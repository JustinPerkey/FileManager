use super::*;

#[test]
fn list_orders_and_labels() {
    let fx = Fixture::example();
    let sess = fx.session();
    let p = sess.list("author", &lq(None, 0, 50)).unwrap();
    assert_eq!(
        labels(&p),
        ["Castellan, Mira", "Lindqvist, Ada", "Okafor, Tomas"]
    );
    assert_eq!(p.total, 3);
    assert_eq!(p.items[0].key, rk(3));
    // default label is the `name` column
    let g = sess.list("genre", &lq(None, 0, 50)).unwrap();
    assert_eq!(
        labels(&g),
        ["Fiction", "History", "Poetry", "Science fiction"]
    );
}

#[test]
fn list_paging_is_complete() {
    let fx = Fixture::example();
    let sess = fx.session();
    let mut seen = Vec::new();
    for offset in [0, 2, 4] {
        let p = sess.list("book", &lq(None, offset, 2)).unwrap();
        assert_eq!(p.total, 5);
        assert_eq!(p.offset, offset);
        seen.extend(p.items.into_iter().map(|i| i.key));
    }
    assert_eq!(seen.len(), 5);
    for n in 1..=5 {
        assert!(seen.contains(&rk(n)));
    }
}

#[test]
fn paging_stable_with_duplicate_labels() {
    let fx = Fixture::example();
    fx.exec(&"INSERT INTO author (first_name, last_name) VALUES ('Same', 'Name');".repeat(20));
    let sess = fx.session();
    let mut keys = Vec::new();
    for offset in [0, 7, 14, 21] {
        let p = sess.list("author", &lq(None, offset, 7)).unwrap();
        assert_eq!(p.total, 23);
        keys.extend(p.items.into_iter().map(|i| i.key));
    }
    assert_eq!(keys.len(), 23);
    let mut unique = keys.clone();
    unique.sort_by_key(|k| format!("{k:?}"));
    unique.dedup();
    assert_eq!(unique.len(), 23, "every row exactly once");
}

#[test]
fn list_search_case_insensitive() {
    let fx = Fixture::example();
    let sess = fx.session();
    for needle in ["AS", "as", " as "] {
        let p = sess.list("author", &lq(Some(needle), 0, 50)).unwrap();
        assert_eq!(labels(&p), ["Castellan, Mira", "Okafor, Tomas"], "{needle}");
        assert_eq!(p.total, 2);
    }
    // empty search means none
    assert_eq!(
        sess.list("author", &lq(Some("  "), 0, 50)).unwrap().total,
        3
    );
}

#[test]
fn list_search_escapes_wildcards() {
    let fx = Fixture::example();
    fx.exec(
        "INSERT INTO author (first_name, last_name) VALUES ('P', 'a%b'), ('P', 'a_b'), \
         ('P', 'axb'), ('P', 'a\\b');",
    );
    let sess = fx.session();
    let find = |t: &str| {
        labels(&sess.list("author", &lq(Some(t), 0, 50)).unwrap())
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    assert_eq!(find("%"), ["a%b, P"]);
    assert_eq!(find("_"), ["a_b, P"]);
    assert_eq!(find("\\"), ["a\\b, P"]);
    assert_eq!(find("a_b"), ["a_b, P"]);
}

#[test]
fn list_referencing() {
    let fx = Fixture::example();
    let sess = fx.session();
    let relation = relation_to(&sess, "author", "book");
    let mut q = lq(None, 0, 50);
    q.referencing = Some(ReferencingFilter {
        relation,
        key: rk(1),
    });
    let p = sess.list("book", &q).unwrap();
    assert_eq!(labels(&p), ["A Short Winter", "The Quiet Harbour"]);
    assert_eq!(p.total, 2);
    // search combines with the filter
    q.search = Some("harbour".into());
    assert_eq!(
        labels(&sess.list("book", &q).unwrap()),
        ["The Quiet Harbour"]
    );
    // an author that does not exist
    q.referencing.as_mut().unwrap().key = rk(99);
    assert!(matches!(
        sess.list("book", &q),
        Err(SessionError::UnknownObject)
    ));
}

#[test]
fn pickable_table_reads() {
    let fx = Fixture::example();
    let sess = fx.session();
    assert_eq!(mode_of(&sess, "publisher"), TableMode::Pickable);
    let book = sess
        .table_views()
        .into_iter()
        .find(|t| t.name == "book")
        .unwrap();
    let fk = book
        .fields
        .iter()
        .find(|v| v.column == "publisher_id")
        .unwrap()
        .foreign_key
        .as_ref()
        .unwrap();
    assert_eq!(fk.table, "publisher");
    let p = sess.list("publisher", &lq(None, 0, 50)).unwrap();
    assert_eq!(labels(&p), ["Harbour Books", "Northwind Press"]);
    let o = sess.object("publisher", &rk(1)).unwrap();
    assert_eq!(o.label, "Northwind Press");
    let view = sess
        .table_views()
        .into_iter()
        .find(|t| t.name == "publisher")
        .unwrap();
    assert!(view
        .fields
        .iter()
        .all(|v| v.mode == crate::config::FieldMode::ReadOnly));
    // following the link from a book opens it
    let b = sess.object("book", &rk(1)).unwrap();
    let link = field(&b, "publisher_id").link.as_ref().unwrap();
    assert_eq!(link.table, "publisher");
    assert_eq!(link.key, Some(rk(1)));
}

#[test]
fn awkward_identifiers_end_to_end() {
    let sql = r#"
CREATE TABLE "a""b" (id INTEGER PRIMARY KEY, "select" TEXT NOT NULL, "with space" TEXT);
CREATE TABLE "select" (id INTEGER PRIMARY KEY, "ünï" TEXT NOT NULL,
                       "a""b_id" INTEGER REFERENCES "a""b"(id));
CREATE TABLE "with space" (id INTEGER PRIMARY KEY, name TEXT);
CREATE TABLE "ünï" ("a""b" INTEGER NOT NULL REFERENCES "a""b"(id),
                    "with space" INTEGER NOT NULL REFERENCES "with space"(id),
                    PRIMARY KEY ("a""b", "with space")) WITHOUT ROWID;
INSERT INTO "a""b" VALUES (1, 'alpha', 'x'), (2, 'beta', 'y');
INSERT INTO "select" VALUES (1, 'one', 1), (2, 'two', 1), (3, 'three', 2);
INSERT INTO "with space" VALUES (1, 'ws one'), (2, 'ws two');
INSERT INTO "ünï" VALUES (1, 2);
"#;
    let toml = r#"
version = 1
[tables."a\"b"]
label = "{select}"
"#;
    let fx = Fixture::new(sql, Some(toml));
    let sess = fx.session();
    assert!(
        sess.diagnostics().is_empty(),
        "{:?}",
        sess.diagnostics()
            .iter()
            .map(|d| &d.message)
            .collect::<Vec<_>>()
    );
    let names: Vec<String> = sess.table_views().into_iter().map(|t| t.name).collect();
    assert_eq!(names, ["a\"b", "select", "with space"]);

    let p = sess.list("a\"b", &lq(Some("ALP"), 0, 10)).unwrap();
    assert_eq!(labels(&p), ["alpha"]);
    let sel = sess.list("select", &lq(None, 0, 10)).unwrap();
    assert_eq!(sel.total, 3);
    assert_eq!(sel.items[0].label, "select #1");

    // reverse relation through the quoted FK
    let relation = relation_to(&sess, "a\"b", "select");
    let mut q = lq(None, 0, 10);
    q.referencing = Some(ReferencingFilter {
        relation,
        key: rk(1),
    });
    assert_eq!(sess.list("select", &q).unwrap().total, 2);

    let o = sess.object("select", &rk(3)).unwrap();
    let link = field(&o, "a\"b_id").link.as_ref().unwrap();
    assert_eq!(link.label.as_deref(), Some("beta"));
    let a = sess.object("a\"b", &rk(1)).unwrap();
    assert_eq!(a.referenced_by[0].count, 2);
    assert_eq!(a.fields.len(), 3);

    // checklist across the junction
    let page = sess
        .m2m_options(
            "a\"b",
            Some(&rk(1)),
            "m2m:ünï",
            &M2mQuery {
                linked_only: false,
                ..mq(None, 0, 10)
            },
        )
        .unwrap();
    assert_eq!(page.total, 2);
    assert_eq!(page.linked_total, 1);
    let linked: Vec<_> = page
        .items
        .iter()
        .filter(|i| i.linked)
        .map(|i| &i.label)
        .collect();
    assert_eq!(linked, ["ws two"]);
    let ws = sess
        .m2m_options(
            "with space",
            Some(&rk(2)),
            "m2m:ünï",
            &mq(Some("ALP"), 0, 10),
        )
        .unwrap();
    assert_eq!(ws.total, 1);
    assert!(ws.items[0].linked);
}

#[test]
fn rowid_accessor_shadowed() {
    let fx = Fixture::new(
        "CREATE TABLE t (rowid TEXT, _rowid_ TEXT, x TEXT);\
         INSERT INTO t VALUES ('r1', 'u1', 'a'), ('r2', 'u2', 'b');",
        None,
    );
    let sess = fx.session();
    let p = sess.list("t", &lq(None, 0, 10)).unwrap();
    assert_eq!(p.total, 2);
    assert_eq!(p.items[0].key, rk(1));
    assert_eq!(p.items[1].label, "t #2");
    let o = sess.object("t", &rk(2)).unwrap();
    assert_eq!(o.key, rk(2));
    assert_eq!(field(&o, "rowid").value, DbValue::Text("r2".into()));
    assert!(matches!(
        sess.object("t", &rk(3)),
        Err(SessionError::UnknownObject)
    ));
}

#[test]
fn without_rowid_keys() {
    let fx = Fixture::new(
        "CREATE TABLE t (a INTEGER NOT NULL, b TEXT NOT NULL, c REAL, PRIMARY KEY (a, b)) WITHOUT ROWID;\
         INSERT INTO t VALUES (1, 'x', 1.5), (1, 'y', 2.5), (2, 'x', NULL);",
        None,
    );
    let sess = fx.session();
    let p = sess.list("t", &lq(None, 0, 10)).unwrap();
    assert_eq!(labels(&p), ["t #1, x", "t #1, y", "t #2, x"]);
    let key = p.items[1].key.clone();
    assert_eq!(
        key,
        ObjectKey::Primary {
            values: vec![DbValue::Integer("1".into()), DbValue::Text("y".into())]
        }
    );
    let o = sess.object("t", &key).unwrap();
    assert_eq!(o.key, key);
    assert_eq!(field(&o, "c").value, DbValue::Real("2.5".into()));
    // wrong shapes
    assert!(matches!(
        sess.object("t", &rk(1)),
        Err(SessionError::UnknownObject)
    ));
    assert!(matches!(
        sess.object("t", &pk("x")),
        Err(SessionError::UnknownObject)
    ));
    // genre: text key
    let g = Fixture::example();
    let gs = g.session();
    let o = gs.object("genre", &pk("poetry")).unwrap();
    assert_eq!(o.label, "Poetry");
    assert!(matches!(
        gs.object("genre", &pk("nope")),
        Err(SessionError::UnknownObject)
    ));
}
