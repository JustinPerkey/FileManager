use super::*;
use std::fmt::Write;
use std::path::Path;

fn open(dir: &Path, sql: &str) -> Database {
    let p = dir.join("t.db");
    rusqlite::Connection::open(&p)
        .unwrap()
        .execute_batch(sql)
        .unwrap();
    Database::open(&p).unwrap()
}

fn schema_of(sql: &str) -> Schema {
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path(), sql);
    introspect(&db).unwrap()
}

fn skipped(s: &Schema, name: &str) -> Option<SkipReason> {
    s.skipped.iter().find(|k| k.name == name).map(|k| k.reason)
}

#[test]
fn object_tables_and_skipped() {
    let s = schema_of(
        "CREATE TABLE a (id INTEGER PRIMARY KEY, v TEXT);
         CREATE TABLE b (x, y);
         CREATE TABLE c (p, q, PRIMARY KEY (q, p)) WITHOUT ROWID;
         CREATE TABLE d (n INT) STRICT;
         CREATE VIEW v AS SELECT * FROM a;
         CREATE VIRTUAL TABLE f USING fts5(body);
         CREATE TABLE g (w INT, x INT GENERATED ALWAYS AS (w + 1) VIRTUAL, y INT AS (w) STORED);",
    );
    let names: Vec<&str> = s.tables.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["a", "b", "c", "d", "g"]);
    assert_eq!(skipped(&s, "v"), Some(SkipReason::View));
    assert_eq!(skipped(&s, "f"), Some(SkipReason::Virtual));
    assert_eq!(skipped(&s, "f_data"), Some(SkipReason::Shadow));
    assert_eq!(skipped(&s, "sqlite_schema"), Some(SkipReason::Internal));
    let c = s.table("c").unwrap();
    assert!(c.without_rowid);
    assert_eq!(c.primary_key, [1, 0]);
    assert_eq!(
        c.identity,
        Identity::PrimaryKey {
            columns: vec![1, 0]
        }
    );
    assert!(s.table("d").unwrap().strict);
    let g = s.table("g").unwrap();
    assert_eq!(g.columns.len(), 3);
    assert!(!g.columns[0].generated && g.columns[1].generated && g.columns[2].generated);
    assert_eq!(s.table("A").unwrap().name.as_str(), "a");
}

#[test]
fn affinity_rules() {
    let cases = [
        ("INT", Affinity::Integer),
        ("BIGINT", Affinity::Integer),
        ("VARCHAR(20)", Affinity::Text),
        ("CLOB", Affinity::Text),
        ("BLOB", Affinity::Blob),
        ("", Affinity::Blob),
        ("DOUBLE PRECISION", Affinity::Real),
        ("FLOAT", Affinity::Real),
        ("NUMERIC", Affinity::Numeric),
        ("DECIMAL(10,2)", Affinity::Numeric),
        ("BOOLEAN", Affinity::Numeric),
        ("DATETIME", Affinity::Numeric),
    ];
    let cols: Vec<String> = cases
        .iter()
        .enumerate()
        .map(|(i, (t, _))| format!("c{i} {t}"))
        .collect();
    let s = schema_of(&format!("CREATE TABLE t ({});", cols.join(", ")));
    let t = s.table("t").unwrap();
    for (i, (ty, want)) in cases.iter().enumerate() {
        assert_eq!(t.columns[i].affinity, *want, "{ty}");
        assert_eq!(t.columns[i].declared_type, *ty);
    }
}

#[test]
fn rowid_alias_detection() {
    let s = schema_of(
        "CREATE TABLE a (id INTEGER PRIMARY KEY, v);
         CREATE TABLE b (id INT PRIMARY KEY, v);
         CREATE TABLE c (x INTEGER, y INTEGER, PRIMARY KEY (x, y));
         CREATE TABLE d (id INTEGER PRIMARY KEY) WITHOUT ROWID;",
    );
    assert!(s.table("a").unwrap().columns[0].is_rowid_alias);
    assert!(!s.table("b").unwrap().columns[0].is_rowid_alias);
    assert!(s
        .table("c")
        .unwrap()
        .columns
        .iter()
        .all(|c| !c.is_rowid_alias));
    assert!(!s.table("d").unwrap().columns[0].is_rowid_alias);
    assert_eq!(
        s.table("a").unwrap().identity,
        Identity::Rowid {
            accessor: s.table("a").unwrap().columns[0].name.clone(),
            alias_column: Some(0)
        }
    );
    assert_eq!(s.table("a").unwrap().columns[0].pk_position, Some(1));
}

#[test]
fn identity_accessor_shadowing() {
    let s = schema_of(
        "CREATE TABLE a (rowid, v);
         CREATE TABLE b (rowid, _rowid_, oid);
         CREATE TABLE c (id INTEGER PRIMARY KEY, rowid);
         CREATE TABLE d (v);",
    );
    let acc = |n: &str| match &s.table(n).unwrap().identity {
        Identity::Rowid { accessor, .. } => accessor.as_str().to_owned(),
        Identity::PrimaryKey { .. } => panic!(),
    };
    assert_eq!(acc("a"), "_rowid_");
    assert_eq!(acc("c"), "id");
    assert_eq!(acc("d"), "rowid");
    assert!(s.table("b").is_none());
    assert_eq!(skipped(&s, "b"), Some(SkipReason::NoIdentity));
}

#[test]
fn foreign_keys_grouped_and_resolved() {
    let s = schema_of(
        "CREATE TABLE parent (a, b, PRIMARY KEY (a, b));
         CREATE TABLE pk1 (id INTEGER PRIMARY KEY);
         CREATE TABLE child (
             x, y, z,
             FOREIGN KEY (x, y) REFERENCES parent (a, b) ON DELETE CASCADE ON UPDATE SET NULL,
             FOREIGN KEY (z) REFERENCES pk1 ON DELETE RESTRICT
         );
         CREATE TABLE child2 (p, q, FOREIGN KEY (p, q) REFERENCES parent ON DELETE SET DEFAULT);",
    );
    let child = s.table("child").unwrap();
    assert_eq!(child.foreign_keys.len(), 2);
    let multi = child
        .foreign_keys
        .iter()
        .find(|f| f.to_table == "parent")
        .unwrap();
    assert_eq!(multi.from, [0, 1]);
    assert_eq!(multi.to_columns, ["a", "b"]);
    assert_eq!(multi.on_delete, FkAction::Cascade);
    assert_eq!(multi.on_update, FkAction::SetNull);
    assert_eq!(
        multi.target,
        Some(FkTarget {
            table: 0,
            columns: vec![0, 1]
        })
    );
    let single = child
        .foreign_keys
        .iter()
        .find(|f| f.to_table == "pk1")
        .unwrap();
    assert_eq!(single.to_columns, ["id"]);
    assert_eq!(single.on_delete, FkAction::Restrict);
    assert_eq!(single.on_update, FkAction::NoAction);
    assert!(single.target.is_some());
    let c2 = &s.table("child2").unwrap().foreign_keys[0];
    assert_eq!(c2.to_columns, ["a", "b"]);
    assert_eq!(c2.on_delete, FkAction::SetDefault);
    assert_eq!(s.referencing(0).count(), 2);
}

#[test]
fn fk_to_missing_table_has_no_target() {
    let s = schema_of("CREATE TABLE child (x REFERENCES nowhere (id));");
    let fk = &s.table("child").unwrap().foreign_keys[0];
    assert_eq!(fk.to_table, "nowhere");
    assert_eq!(fk.to_columns, ["id"]);
    assert!(fk.target.is_none());
}

#[test]
fn unique_keys_exclude_partial_and_expression() {
    let s = schema_of(
        "CREATE TABLE t (id INTEGER PRIMARY KEY, a, b, c, d);
         CREATE UNIQUE INDEX i1 ON t (a);
         CREATE UNIQUE INDEX i2 ON t (b, c);
         CREATE UNIQUE INDEX i3 ON t (c) WHERE c > 0;
         CREATE UNIQUE INDEX i4 ON t (lower(d));
         CREATE INDEX i5 ON t (d);
         CREATE TABLE u (k TEXT PRIMARY KEY, v);",
    );
    let t = s.table("t").unwrap();
    let mut keys = t.unique_keys.clone();
    keys.sort();
    assert_eq!(keys, [vec![0], vec![1], vec![2, 3]]);
    assert_eq!(s.table("u").unwrap().unique_keys, [vec![0]]);
}

#[test]
fn triggers_flagged() {
    let s = schema_of(
        "CREATE TABLE a (x); CREATE TABLE b (x);
         CREATE TRIGGER tr AFTER INSERT ON a BEGIN SELECT 1; END;",
    );
    assert!(s.table("a").unwrap().has_triggers);
    assert!(!s.table("b").unwrap().has_triggers);
}

const AWKWARD: [&str; 5] = ["a\"b", "select", "with space", "ünï", "\""];

#[test]
fn awkward_identifiers_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let mut sql = String::new();
    for n in AWKWARD {
        let q = format!("\"{}\"", n.replace('"', "\"\""));
        write!(sql, "CREATE TABLE {q} ({q} INTEGER PRIMARY KEY, x);").unwrap();
    }
    let db = open(tmp.path(), &sql);
    let s = introspect(&db).unwrap();
    assert_eq!(s.tables.len(), AWKWARD.len());
    for (t, n) in s.tables.iter().zip(AWKWARD) {
        assert_eq!(t.name.as_str(), n);
        assert_eq!(t.name.to_string(), n);
        assert_eq!(t.columns[0].name.as_str(), n);
        let count: i64 = db
            .conn()
            .query_row(
                &format!("SELECT count(*) FROM {}", t.name.quoted()),
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
        let col: i64 = db
            .conn()
            .query_row(
                &format!(
                    "SELECT count({}) FROM {}",
                    t.columns[0].name.quoted(),
                    t.name.quoted()
                ),
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(col, 0);
    }
}

#[test]
fn quoted_doubles_quotes() {
    let s = schema_of("CREATE TABLE \"a\"\"b\" (x);");
    assert_eq!(s.tables[0].name.quoted(), "\"a\"\"b\"");
}
