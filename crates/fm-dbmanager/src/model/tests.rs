use super::*;
use crate::config::{
    parse as parse_config, ConfigReport, DiagnosticSeverity, DurationUnit, FieldConfig, FieldEntry,
    Located, TableConfig, TableEntry,
};
use crate::db::Database;
use crate::rules::{infer_kind, FieldKind};
use crate::schema::introspect;
use std::fmt::Write;

const EXAMPLE: &str = include_str!("../../../../examples/dbmanager/example.toml");
const SCHEMA: &str = include_str!("../../../../examples/dbmanager/schema.sql");

fn schema_of(sql: &str) -> Schema {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("t.db");
    rusqlite::Connection::open(&p)
        .unwrap()
        .execute_batch(sql)
        .unwrap();
    introspect(&Database::open(&p).unwrap()).unwrap()
}

fn model(sql: &str, toml: &str) -> Model {
    resolve(&schema_of(sql), &parse_config(toml))
}

fn example(toml: &str) -> Model {
    model(SCHEMA, toml)
}

fn views(m: &Model) -> Vec<TableView> {
    m.table_views()
}

fn tv<'a>(v: &'a [TableView], name: &str) -> &'a TableView {
    v.iter()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("no table view {name}"))
}

fn fv<'a>(t: &'a TableView, col: &str) -> &'a FieldView {
    t.fields
        .iter()
        .find(|f| f.column == col)
        .unwrap_or_else(|| panic!("no field {}.{col}", t.name))
}

fn column_names(m: &Model, table: &str) -> Vec<String> {
    tv(&views(m), table)
        .fields
        .iter()
        .map(|f| f.column.clone())
        .collect()
}

fn has_field(t: &TableView, col: &str) -> bool {
    t.fields.iter().any(|f| f.column == col)
}

fn diags(m: &Model, sev: DiagnosticSeverity) -> Vec<&ConfigDiagnostic> {
    m.diagnostics()
        .iter()
        .filter(|d| d.severity == sev)
        .collect()
}

/// A diagnostic of `sev` on `table` (and `column`, when given) whose message
/// contains `text`.
fn find<'a>(
    m: &'a Model,
    sev: DiagnosticSeverity,
    table: &str,
    column: Option<&str>,
    text: &str,
) -> &'a ConfigDiagnostic {
    m.diagnostics()
        .iter()
        .find(|d| {
            d.severity == sev
                && d.table.as_deref() == Some(table)
                && (column.is_none() || d.column.as_deref() == column)
                && d.message.contains(text)
        })
        .unwrap_or_else(|| {
            panic!(
                "no {sev:?} on {table}.{column:?} with {text:?}: {:#?}",
                m.diagnostics()
            )
        })
}

use DiagnosticSeverity::{Error, Warning};
use FieldMode::{CreateOnly, Editable, ReadOnly};

#[test]
fn example_resolves_clean() {
    let m = example(EXAMPLE);
    assert_eq!(m.diagnostics(), &[]);
    let v = views(&m);
    let names: Vec<&str> = v.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(
        names,
        ["publisher", "author", "book", "genre", "member", "loan"]
    );
    for gone in ["audit_log", "book_genre", "contribution"] {
        assert!(m.table(gone).is_none(), "{gone}");
    }
    assert!(!has_field(tv(&v, "author"), "internal_note"));

    let book = tv(&v, "book");
    assert_eq!(fv(book, "revision").mode, ReadOnly);
    assert_eq!(fv(book, "isbn").mode, CreateOnly);
    assert_eq!(fv(book, "id").mode, ReadOnly);
    assert!(fv(book, "id").is_key);
    assert_eq!(fv(book, "title").mode, Editable);
    assert_eq!(fv(book, "pages").mode, Editable);
    assert_eq!(fv(tv(&v, "member"), "name").mode, Editable);
    assert_eq!(fv(book, "in_print").kind, FieldKind::Bool);
    assert_eq!(fv(tv(&v, "loan"), "returned").kind, FieldKind::Bool);
    let audio = fv(book, "audio_length");
    assert_eq!(audio.kind, FieldKind::Duration);
    assert_eq!(audio.rule.unit, Some(DurationUnit::Seconds));
    assert_eq!(audio.rule.min.as_deref(), Some("1m"));
    // 100 hours, canonically: days first.
    assert_eq!(audio.rule.max.as_deref(), Some("4d4h"));
    let period = fv(tv(&v, "loan"), "loan_period");
    assert_eq!(period.default_value, Some(DbValue::Duration("14d".into())));
    assert_eq!(period.rule.min.as_deref(), Some("1d"));

    let author_fk = fv(book, "author_id").foreign_key.as_ref().unwrap();
    assert_eq!(author_fk.table, "author");
    assert!(author_fk.navigable);
    let publisher = tv(&v, "publisher");
    assert_eq!(publisher.mode, TableMode::Pickable);
    assert!(publisher.fields.iter().all(|f| f.mode == ReadOnly));
    let pfk = fv(book, "publisher_id");
    assert_eq!(pfk.mode, Editable);
    assert!(pfk.foreign_key.as_ref().unwrap().navigable);

    let genre_side = &book.many_to_many[0];
    assert_eq!(genre_side.relation, "m2m:book_genre");
    assert_eq!(genre_side.heading, "Genres");
    assert_eq!(genre_side.mode, RelationMode::Editable);
    assert_eq!(
        genre_side.link_fields,
        [] as [crate::model::views::FieldView; 0]
    );
    let genre = tv(&v, "genre");
    assert_eq!(genre.many_to_many[0].heading, "Books");
    assert_eq!(genre.many_to_many[0].mode, RelationMode::Editable);
    for t in [book, tv(&v, "author")] {
        let c = t
            .many_to_many
            .iter()
            .find(|s| s.relation == "m2m:contribution")
            .unwrap();
        assert_eq!(c.mode, RelationMode::Editable);
        let cols: Vec<&str> = c.link_fields.iter().map(|f| f.column.as_str()).collect();
        assert_eq!(cols, ["role", "position"]);
        assert_eq!(c.link_fields[0].rule.allowed.as_ref().unwrap().len(), 3);
        assert!(c.link_fields[0].has_default);
        assert_eq!(c.link_fields[1].rule.min.as_deref(), Some("1"));
        assert_eq!(c.link_fields[1].rule.max.as_deref(), Some("99"));
    }
    let author = tv(&v, "author");
    let from: Vec<&str> = author
        .referenced_by
        .iter()
        .map(|r| r.table.as_str())
        .collect();
    assert_eq!(from, ["book"]);
    assert_eq!(author.referenced_by[0].via, "Author");
    assert!(book.referenced_by.iter().all(|r| r.table != "book_genre"));

    let label = |t: &str| m.table(t).unwrap().label.clone();
    let genre_name = m.schema().table("genre").unwrap().column("name").unwrap();
    assert_eq!(
        label("genre").parts,
        [ResolvedLabelPart::Column(genre_name)]
    );
    assert_eq!(author.object_name.as_deref(), Some("author"));
    assert_eq!(
        tv(&v, "publisher").object_name.as_deref(),
        Some("publisher")
    );
    for (t, create, delete) in [
        ("author", true, true),
        ("book", true, true),
        ("genre", true, false),
        ("member", true, true),
        ("loan", true, true),
        ("publisher", false, false),
    ] {
        assert_eq!(tv(&v, t).can_create, create, "{t} create");
        assert_eq!(tv(&v, t).can_delete, delete, "{t} delete");
    }
}

#[test]
fn no_config_is_editable() {
    let m = example("");
    // Only the missing version is reported.
    assert_eq!(m.diagnostics().len(), 1);
    let v = views(&m);
    assert!(m.table("book_genre").is_none());
    for t in &v {
        assert_eq!(t.mode, TableMode::Editable, "{}", t.name);
        assert!(!t.can_delete);
        assert_eq!(t.object_name, None);
    }
    let book = tv(&v, "book");
    assert_eq!(fv(book, "id").mode, ReadOnly);
    assert_eq!(fv(book, "title").mode, Editable);
    assert_eq!(fv(book, "isbn").mode, Editable);
    assert_eq!(fv(tv(&v, "genre"), "code").mode, CreateOnly);
    for t in [
        "author",
        "book",
        "genre",
        "member",
        "loan",
        "publisher",
        "contribution",
        "audit_log",
    ] {
        assert!(tv(&v, t).can_create, "{t}");
    }
    for (t, other) in [("book", "genre"), ("genre", "book")] {
        let side = &tv(&v, t).many_to_many[0];
        assert_eq!(side.other_table, other);
        assert_eq!(side.mode, RelationMode::Editable);
    }
    // contribution has extra columns: an ordinary table with reverse relations.
    assert!(tv(&v, "book")
        .referenced_by
        .iter()
        .any(|r| r.table == "contribution"));
    assert!(tv(&v, "author")
        .referenced_by
        .iter()
        .any(|r| r.table == "contribution"));
    assert!(tv(&v, "book")
        .many_to_many
        .iter()
        .all(|s| s.relation != "m2m:contribution"));
    assert_eq!(fv(tv(&v, "contribution"), "role").mode, Editable);
}

#[test]
fn pickable_table_semantics() {
    let m = example(
        r#"version = 1
[tables.publisher]
mode = "pickable"
allow_create = true
allow_delete = true
[tables.publisher.fields.name]
mode = "editable"
[tables.book_genre_dummy]
mode = "pickable"
[tables.genre]
mode = "pickable"
"#,
    );
    let v = views(&m);
    let p = tv(&v, "publisher");
    assert_eq!(p.mode, TableMode::Pickable);
    assert!(!p.can_create && !p.can_delete);
    assert!(p.fields.iter().all(|f| f.mode == ReadOnly));
    find(&m, Warning, "publisher", Some("name"), "stays read-only");
    let dc = find(&m, Warning, "publisher", None, "allow_create");
    assert!(dc.line.is_some());
    find(&m, Warning, "publisher", None, "allow_delete");
    // Picking from it, and following the link, are allowed.
    let fk = fv(tv(&v, "book"), "publisher_id");
    assert_eq!(fk.mode, Editable);
    assert!(fk.foreign_key.as_ref().unwrap().navigable);
    // A pickable genre is still the other side of book's checklist, and its
    // own checklist is read-only.
    let book_side = &tv(&v, "book").many_to_many[0];
    assert_eq!(book_side.other_table, "genre");
    assert_eq!(book_side.mode, RelationMode::Editable);
    let genre_side = &tv(&v, "genre").many_to_many[0];
    assert_eq!(genre_side.mode, RelationMode::ReadOnly);
    // It is a reverse-relation source too.
    assert!(tv(&v, "publisher")
        .referenced_by
        .iter()
        .any(|r| r.table == "book"));

    // Not a valid default.
    let m = example("version = 1\n[defaults]\ntable_mode = \"pickable\"\n");
    assert_ne!(
        diags(&m, Error),
        [] as [&crate::config::ConfigDiagnostic; 0]
    );
    assert!(views(&m).iter().all(|t| t.mode == TableMode::ReadOnly));

    // On a junction: a warning, and the junction stays unlisted.
    let m = example("version = 1\n[tables.book_genre]\nmode = \"pickable\"\n");
    find(&m, Warning, "book_genre", None, "pickable");
    assert!(m.table("book_genre").is_none());
    assert_eq!(
        tv(&views(&m), "book").many_to_many[0].mode,
        RelationMode::Editable
    );
}

fn assert_all_closed(m: &Model) {
    assert_ne!(m.table_views(), [] as [crate::model::views::TableView; 0]);
    for t in m.tables() {
        assert_eq!(t.mode, TableMode::ReadOnly, "{}", t.name);
        assert!(!t.can_create && !t.can_delete);
        assert_eq!(t.hidden_columns, [] as [usize; 0]);
        assert!(t.fields.iter().all(|f| f.mode == ReadOnly), "{}", t.name);
        for s in &t.many_to_many {
            assert_eq!(s.mode, RelationMode::ReadOnly);
            assert!(s.link_fields.iter().all(|f| f.mode == ReadOnly));
        }
    }
    assert!(m.table("audit_log").is_some());
}

#[test]
fn syntax_error_config_is_read_only() {
    let m = example("version = 1\n[tables.book\n");
    assert_all_closed(&m);
    let m = example("version = 2\n[tables.audit_log]\nmode = \"hidden\"\n");
    assert_all_closed(&m);
    assert!(m.table("book_genre").is_none(), "still unlisted");
    assert!(m.table("contribution").is_some());
}

#[test]
fn read_only_default_policy() {
    let m = example("version = 1\n[defaults]\ntable_mode = \"read-only\"\n");
    assert_eq!(m.diagnostics(), &[]);
    for t in m.tables() {
        assert_eq!(t.mode, TableMode::ReadOnly);
        assert!(!t.can_create && !t.can_delete);
    }
    // field_mode is still editable, but a read-only table narrows it.
    let v = views(&m);
    assert!(tv(&v, "book").fields.iter().all(|f| f.mode == ReadOnly));
    // The m2m checklists are read-only because both tables are.
    assert_eq!(tv(&v, "book").many_to_many[0].mode, RelationMode::ReadOnly);
    let m = example(
        "version = 1\n[defaults]\ntable_mode = \"read-only\"\n[tables.book.fields.title]\nmode = \"editable\"\n[tables.book]\nallow_delete = true\n[tables.author]\nmode = \"editable\"\n",
    );
    find(&m, Warning, "book", Some("title"), "not editable");
    find(&m, Warning, "book", None, "allow_delete");
    let v = views(&m);
    assert_eq!(fv(tv(&v, "author"), "first_name").mode, Editable);
    assert_eq!(fv(tv(&v, "book"), "title").mode, ReadOnly);
}

// ---- kinds ---------------------------------------------------------------

fn column_of(decl: &str) -> crate::schema::Column {
    let s = schema_of(&format!(
        "CREATE TABLE t (id INTEGER PRIMARY KEY, c {decl})"
    ));
    s.tables[0].columns[1].clone()
}

#[test]
fn kind_inference_rules() {
    use FieldKind::{Any, Blob, Bool, Integer, Real, Text};
    for (decl, kind) in [
        ("BOOLEAN", Bool),
        ("bool", Bool),
        ("", Any),
        ("ANY", Any),
        ("INT", Integer),
        ("UNSIGNED BIG INT", Integer),
        ("VARCHAR(20)", Text),
        ("NCHAR(5)", Text),
        ("CLOB", Text),
        ("TEXT", Text),
        ("BLOB", Blob),
        ("REAL", Real),
        ("FLOAT", Real),
        ("DOUBLE PRECISION", Real),
        ("NUMERIC", Real),
        ("DECIMAL(10, 5)", Real),
        ("DATETIME", Real),
    ] {
        assert_eq!(infer_kind(&column_of(decl), false), kind, "{decl:?}");
    }
}

fn kind_cfg(table: &str, ty: &str) -> ConfigReport {
    parse_config(&format!(
        "version = 1\n[tables.t.fields.c]\ntype = \"{ty}\"\n{table}"
    ))
}

fn resolved_kind(decl: &str, strict: bool, ty: &str) -> (FieldKind, bool) {
    let sql = format!(
        "CREATE TABLE t (id INTEGER PRIMARY KEY, c {decl}){}",
        if strict { " STRICT" } else { "" }
    );
    let s = schema_of(&sql);
    let m = resolve(&s, &kind_cfg("", ty));
    let f = m.table("t").unwrap().field(1).unwrap().clone();
    (f.kind, !diags(&m, Error).is_empty())
}

#[test]
fn type_override_compatibility() {
    use FieldKind::{Bool, Duration, Integer, Real, Text};
    // (declared, type, allowed)
    for (decl, ty, ok) in [
        ("INTEGER", "integer", true),
        ("INTEGER", "bool", true),
        ("INTEGER", "duration", true),
        ("INTEGER", "text", false),
        ("INTEGER", "real", false),
        ("TEXT", "integer", false),
        ("TEXT", "text", true),
        ("REAL", "real", true),
        ("REAL", "integer", false),
        ("NUMERIC", "integer", true),
        ("NUMERIC", "real", true),
        ("NUMERIC", "text", true),
        ("NUMERIC", "duration", true),
        ("", "integer", true),
        ("", "text", true),
        ("", "duration", true),
        ("BLOB", "integer", false),
        ("BLOB", "text", false),
    ] {
        let (kind, errored) = resolved_kind(decl, false, ty);
        assert_eq!(!errored, ok, "{decl:?} + {ty}");
        if !ok {
            assert_ne!(
                kind,
                match ty {
                    "integer" => Integer,
                    "real" => Real,
                    "text" => Text,
                    "bool" => Bool,
                    _ => Duration,
                }
            );
        }
    }
    assert_eq!(resolved_kind("INTEGER", false, "bool").0, Bool);
    assert_eq!(resolved_kind("", false, "duration").0, Duration);
}

#[test]
fn strict_table_kinds() {
    use FieldKind::{Any, Bool, Duration, Integer};
    for (decl, ty, ok) in [
        ("INT", "integer", true),
        ("INTEGER", "bool", true),
        ("INTEGER", "duration", true),
        ("INTEGER", "real", false),
        ("REAL", "real", true),
        ("REAL", "integer", false),
        ("TEXT", "text", true),
        ("TEXT", "integer", false),
        ("ANY", "integer", true),
        ("ANY", "real", true),
        ("ANY", "text", true),
        ("ANY", "duration", true),
        ("BLOB", "integer", false),
        ("BLOB", "text", false),
    ] {
        assert_eq!(!resolved_kind(decl, true, ty).1, ok, "STRICT {decl} + {ty}");
    }
    assert_eq!(resolved_kind("ANY", true, "bool").0, Bool);
    assert_eq!(resolved_kind("INTEGER", true, "duration").0, Duration);
    // Without `type`, ANY is any and read-only.
    let m = model(
        "CREATE TABLE t (id INTEGER PRIMARY KEY, c ANY, d INTEGER) STRICT",
        "version = 1\n",
    );
    let t = m.table("t").unwrap();
    assert_eq!(t.field(1).unwrap().kind, Any);
    assert_eq!(t.field(1).unwrap().mode, ReadOnly);
    assert_eq!(t.field(2).unwrap().kind, Integer);
}

#[test]
fn duration_fields_resolve() {
    let m = model(
        "CREATE TABLE t (id INTEGER PRIMARY KEY, d INTEGER NOT NULL DEFAULT 90, e INTEGER)",
        r#"version = 1
[tables.t.fields.d]
type = "duration"
unit = "minutes"
min = "30m"
max = 5000
[tables.t.fields.e]
type = "duration"
"#,
    );
    assert_eq!(m.diagnostics(), &[]);
    let t = m.table("t").unwrap();
    let d = t.field(1).unwrap();
    assert_eq!(d.kind, FieldKind::Duration);
    assert_eq!(d.rule.unit, Some(DurationUnit::Minutes));
    assert_eq!(d.rule.min.as_deref(), Some("30m"));
    assert_eq!(d.rule.max.as_deref(), Some("3d11h20m"));
    assert_eq!(d.default_value, Some(DbValue::Duration("1h30m".into())));
    assert!(!d.rule.nullable);
    let e = t.field(2).unwrap();
    assert_eq!(e.rule.unit, Some(DurationUnit::Seconds));
    assert!(e.rule.nullable);
    // Only durations carry a unit.
    let m = model(
        "CREATE TABLE t (id INTEGER PRIMARY KEY, n INTEGER)",
        "version = 1\n",
    );
    assert_eq!(m.table("t").unwrap().field(1).unwrap().rule.unit, None);
}

#[test]
fn duration_bound_precision() {
    let cfg = |min: &str| {
        format!(
            "version = 1\n[tables.t.fields.d]\ntype = \"duration\"\nunit = \"minutes\"\nmin = {min}\n"
        )
    };
    let sql = "CREATE TABLE t (id INTEGER PRIMARY KEY, d INTEGER)";
    let m = model(sql, &cfg("\"90s\""));
    let e = find(&m, Error, "t", Some("d"), "whole number of minutes");
    assert_eq!((e.line, e.col.is_some()), (Some(5), true));
    let f = m.table("t").unwrap().field(1).unwrap();
    assert_eq!(f.mode, ReadOnly);
    assert_eq!(f.rule.min, None);
    // A whole number of units, or a count, is fine.
    let m = model(sql, &cfg("\"2m\""));
    assert_eq!(m.diagnostics(), &[]);
    let m = model(sql, &cfg("45"));
    assert_eq!(
        m.table("t").unwrap().field(1).unwrap().rule.min.as_deref(),
        Some("45m")
    );
    // Mixed forms compare after resolution.
    let m = model(
        sql,
        "version = 1\n[tables.t.fields.d]\ntype = \"duration\"\nunit = \"minutes\"\nmin = \"2h\"\nmax = 100\n",
    );
    find(&m, Error, "t", Some("d"), "greater than");
}

// ---- modes and fail closed ----------------------------------------------

const FORCED: &str = "CREATE TABLE p (a INTEGER, b INTEGER, UNIQUE (a, b));
CREATE TABLE h (id INTEGER PRIMARY KEY);
CREATE TABLE t (
  id INTEGER PRIMARY KEY,
  g INTEGER GENERATED ALWAYS AS (id + 1) VIRTUAL,
  bl BLOB,
  u,
  ma INTEGER, mb INTEGER,
  hid INTEGER REFERENCES h(id),
  ok TEXT,
  FOREIGN KEY (ma, mb) REFERENCES p(a, b));";

#[test]
fn forced_read_only_explicit_is_error() {
    let mut cfg = String::from("version = 1\n[tables.h]\nmode = \"hidden\"\n");
    for c in ["id", "g", "bl", "u", "ma", "hid"] {
        let _ = writeln!(cfg, "[tables.t.fields.{c}]\nmode = \"editable\"");
    }
    let m = model(FORCED, &cfg);
    let t = m.table("t").unwrap();
    for c in ["id", "g", "bl", "u", "ma", "hid"] {
        assert_eq!(t.field(t_col(&m, c)).unwrap().mode, ReadOnly, "{c}");
        let e = find(&m, Error, "t", Some(c), "read-only");
        assert!(e.line.is_some(), "{c}");
    }
    assert_eq!(diags(&m, Error).len(), 6);
    // create-only on them is the same error.
    let m = model(
        FORCED,
        "version = 1\n[tables.t.fields.id]\nmode = \"create-only\"\n",
    );
    find(&m, Error, "t", Some("id"), "rowid alias");
}

fn t_col(m: &Model, c: &str) -> usize {
    m.schema().table("t").unwrap().column(c).unwrap()
}

#[test]
fn forced_read_only_inherited_is_silent() {
    let m = model(FORCED, "version = 1\n[tables.h]\nmode = \"hidden\"\n");
    assert_eq!(m.diagnostics(), &[]);
    let t = m.table("t").unwrap();
    for c in ["id", "g", "bl", "u", "ma", "mb", "hid"] {
        assert_eq!(t.field(t_col(&m, c)).unwrap().mode, ReadOnly, "{c}");
    }
    assert_eq!(t.field(t_col(&m, "ok")).unwrap().mode, Editable);
    // The FK into the hidden table has no link: a plain field.
    let hid = t.field(t_col(&m, "hid")).unwrap();
    assert!(hid.foreign_key.is_none());
    // Multi-column FK: only the first column carries the link.
    assert!(t.field(t_col(&m, "ma")).unwrap().foreign_key.is_some());
    assert!(t.field(t_col(&m, "mb")).unwrap().foreign_key.is_none());
}

#[test]
fn primary_key_inherited_becomes_create_only() {
    let sql = "CREATE TABLE t (code TEXT PRIMARY KEY, n INTEGER);";
    let m = model(sql, "version = 1\n");
    assert_eq!(m.diagnostics(), &[]);
    assert_eq!(m.table("t").unwrap().field(0).unwrap().mode, CreateOnly);
    let m = model(
        sql,
        "version = 1\n[tables.t.fields.code]\nmode = \"create-only\"\n",
    );
    assert_eq!(m.table("t").unwrap().field(0).unwrap().mode, CreateOnly);
    assert_eq!(m.diagnostics(), &[]);
    let m = model(
        sql,
        "version = 1\n[tables.t.fields.code]\nmode = \"editable\"\n",
    );
    find(&m, Error, "t", Some("code"), "create-only");
    assert_eq!(m.table("t").unwrap().field(0).unwrap().mode, ReadOnly);
}

#[test]
fn fail_closed_rejected_table() {
    let m = example("version = 1\n[tables.book]\nbogus = 1\n");
    assert_eq!(diags(&m, Error).len(), 1);
    let v = views(&m);
    let book = tv(&v, "book");
    assert_eq!(book.mode, TableMode::ReadOnly);
    assert!(!book.can_create && !book.can_delete);
    assert!(book.fields.iter().all(|f| f.mode == ReadOnly));
    assert_eq!(tv(&v, "author").mode, TableMode::Editable);
    // A rejected table that asked for hidden is hidden.
    let m = example("version = 1\n[tables.book]\nmode = \"hidden\"\nbogus = 1\n");
    assert!(m.table("book").is_none());
}

#[test]
fn rejected_table_keeps_hidden_fields() {
    let m = example(
        "version = 1\n[tables.author]\nbogus = 1\n[tables.author.fields.internal_note]\nmode = \"hidden\"\n[tables.author.fields.born]\nmin = 5\nnullable = true\nbogus = 2\n",
    );
    let v = views(&m);
    let a = tv(&v, "author");
    assert_eq!(a.mode, TableMode::ReadOnly);
    assert!(!has_field(a, "internal_note"));
    assert_eq!(a.fields.len(), 4);
    assert!(a.fields.iter().all(|f| f.mode == ReadOnly));
    // A rejected field entry that asked for hidden is hidden too.
    let m = example(
        "version = 1\n[tables.author]\nbogus = 1\n[tables.author.fields.born]\nmode = \"hidden\"\nbogus = 2\n",
    );
    assert!(!has_field(tv(&views(&m), "author"), "born"));
}

#[test]
fn unknown_top_level_key_is_read_only_policy() {
    let m = example("version = 1\n[table.secrets]\nx = 1\n[tables.author]\nmode = \"editable\"\n");
    let v = views(&m);
    assert_eq!(tv(&v, "author").mode, TableMode::Editable);
    for t in ["book", "genre", "member", "loan", "publisher"] {
        assert_eq!(tv(&v, t).mode, TableMode::ReadOnly, "{t}");
        assert!(tv(&v, t).fields.iter().all(|f| f.mode == ReadOnly));
    }
}

#[test]
fn fail_closed_rejected_field() {
    let m = example("version = 1\n[tables.book.fields.title]\nmax_length = -1\n[tables.book.fields.pages]\nmin = 1\n");
    let v = views(&m);
    let book = tv(&v, "book");
    assert_eq!(book.mode, TableMode::Editable);
    assert_eq!(fv(book, "title").mode, ReadOnly);
    assert_eq!(fv(book, "pages").mode, Editable);
    // Rejected entries drop everything except hidden.
    let m = example("version = 1\n[tables.book.fields.title]\ndisplay_name = \"T\"\nbogus = 1\n");
    assert_eq!(fv(tv(&views(&m), "book"), "title").display_name, "title");
    // A resolution error also makes the field read-only.
    let m = example("version = 1\n[tables.book.fields.title]\nmin = 1\n");
    find(&m, Error, "book", Some("title"), "applies to integer");
    assert_eq!(fv(tv(&views(&m), "book"), "title").mode, ReadOnly);
}

#[test]
fn field_in_read_only_table_warns() {
    let m = example(
        "version = 1\n[tables.book]\nmode = \"read-only\"\n[tables.book.fields.title]\nmode = \"editable\"\n[tables.book.fields.isbn]\nmode = \"create-only\"\n[tables.book.fields.pages]\nmode = \"read-only\"\n",
    );
    for c in ["title", "isbn"] {
        let d = find(&m, Warning, "book", Some(c), "stays read-only");
        assert!(d.line.is_some());
    }
    assert_eq!(diags(&m, Warning).len(), 2);
    assert!(tv(&views(&m), "book")
        .fields
        .iter()
        .all(|f| f.mode == ReadOnly));
}

// ---- rules ----------------------------------------------------------------

#[test]
fn nullable_cannot_loosen() {
    let sql =
        "CREATE TABLE t (id INTEGER PRIMARY KEY, a TEXT NOT NULL, b TEXT, c INTEGER NOT NULL)";
    let m = model(
        sql,
        "version = 1\n[tables.t.fields.a]\nnullable = true\n[tables.t.fields.b]\nnullable = false\n",
    );
    let e = find(&m, Error, "t", Some("a"), "NOT NULL");
    assert_eq!(e.line, Some(3));
    let t = m.table("t").unwrap();
    assert_eq!(t.field(1).unwrap().mode, ReadOnly);
    assert!(!t.field(1).unwrap().rule.nullable);
    assert!(!t.field(2).unwrap().rule.nullable);
    assert!(!t.field(2).unwrap().rule.empty_is_null);
    assert!(!t.field(3).unwrap().rule.nullable);
    // empty_is_null defaults for nullable text only.
    let m = model(sql, "version = 1\n");
    let t = m.table("t").unwrap();
    assert!(t.field(2).unwrap().rule.empty_is_null);
    assert!(!t.field(1).unwrap().rule.empty_is_null);
    let m = model(sql, "version = 1\n[tables.t.fields.a]\nempty_is_null = true\n[tables.t.fields.c]\nempty_is_null = true\n");
    assert_eq!(diags(&m, Error).len(), 2);
    let m = model(
        sql,
        "version = 1\n[tables.t.fields.b]\nempty_is_null = false\n",
    );
    assert!(!m.table("t").unwrap().field(2).unwrap().rule.empty_is_null);
}

#[test]
fn bounds_must_fit_kind() {
    let sql =
        "CREATE TABLE t (id INTEGER PRIMARY KEY, i INTEGER, r REAL, s TEXT, b BOOLEAN, n NUMERIC)";
    let cases = [
        ("i", "min = 1.5", true),
        ("i", "min = \"1h\"", true),
        ("i", "min = 1\nmax = 9", false),
        ("r", "min = 1\nmax = 2.5", false),
        ("r", "min = \"1h\"", true),
        ("s", "min = 1", true),
        ("s", "min_length = 1\nmax_length = 5\npattern = 'a+'", false),
        ("i", "min_length = 1", true),
        ("i", "pattern = 'a'", true),
        ("b", "min = 0", true),
        ("i", "allowed = [\"x\"]", true),
        ("s", "allowed = [1]", true),
        ("r", "allowed = [1]", true),
        ("i", "allowed = [1, 2]", false),
        ("s", "allowed = [\"a\"]", false),
        ("n", "type = \"duration\"\nmin = \"1h\"", false),
    ];
    for (col, body, bad) in cases {
        let m = model(
            sql,
            &format!("version = 1\n[tables.t.fields.{col}]\n{body}\n"),
        );
        let errs = diags(&m, Error);
        assert_eq!(!errs.is_empty(), bad, "{col}: {body}: {errs:?}");
        let f = m.table("t").unwrap();
        let col_i = m.schema().table("t").unwrap().column(col).unwrap();
        assert_eq!(
            f.field(col_i).unwrap().mode == ReadOnly,
            bad,
            "{col}: {body}"
        );
    }
    // Real bounds are canonical decimal text.
    let m = model(
        sql,
        "version = 1\n[tables.t.fields.r]\nmin = 0.5\nmax = 100\n",
    );
    let r = m.table("t").unwrap().field(2).unwrap().rule.clone();
    assert_eq!(
        (r.min.as_deref(), r.max.as_deref()),
        (Some("0.5"), Some("100"))
    );
    // Allowed labels default to the value text.
    let m = model(
        sql,
        "version = 1\n[tables.t.fields.i]\nallowed = [1, { value = 2, label = \"Two\" }]\n",
    );
    let a = m
        .table("t")
        .unwrap()
        .field(1)
        .unwrap()
        .rule
        .allowed
        .clone()
        .unwrap();
    assert_eq!(a[0].label, "1");
    assert_eq!(a[1].label, "Two");
    assert_eq!(a[1].value, DbValue::Integer("2".into()));
}

#[test]
fn fk_field_bounds_rejected() {
    for body in [
        "min = 1",
        "type = \"integer\"",
        "allowed = [1]",
        "pattern = 'a'",
        "max_length = 3",
    ] {
        let m = example(&format!(
            "version = 1\n[tables.book.fields.author_id]\n{body}\n"
        ));
        find(&m, Error, "book", Some("author_id"), "foreign key");
        let book = tv(&views(&m), "book").clone();
        assert_eq!(fv(&book, "author_id").mode, ReadOnly, "{body}");
    }
    let m = example(
        "version = 1\n[tables.book.fields.author_id]\ndisplay_name = \"A\"\nnullable = false\nhelp = \"x\"\n",
    );
    assert_eq!(m.diagnostics(), &[]);
    assert_eq!(fv(tv(&views(&m), "book"), "author_id").display_name, "A");
}

// ---- patterns ---------------------------------------------------------------

fn pattern_model(pattern: &str) -> Model {
    model(
        "CREATE TABLE t (id INTEGER PRIMARY KEY, s TEXT)",
        &format!("version = 1\n[tables.t.fields.s]\npattern = '{pattern}'\n"),
    )
}

#[test]
fn pattern_shorthand_rejected() {
    for p in [
        r"\d+", r"\D", r"\w+", r"\W", r"\s", r"\S+", r"\bx", r"\B", r"[\d.]", r"a[^\s]b", r"x\\\d",
    ] {
        let m = pattern_model(p);
        let e = find(&m, Error, "t", Some("s"), "explicit class");
        assert_eq!(e.line, Some(3), "{p}");
        let f = m.table("t").unwrap().field(1).unwrap().clone();
        assert_eq!(f.rule.pattern, None, "{p}");
        assert_eq!(f.mode, ReadOnly, "{p}");
    }
    // Escaped forms are not flagged.
    for p in [r"\\d", r"\.", r"[0-9]+", r"a\\\\d", r"\[d\]"] {
        let m = pattern_model(p);
        assert_eq!(diags(&m, Error), Vec::<&ConfigDiagnostic>::new(), "{p}");
        assert_eq!(
            m.table("t")
                .unwrap()
                .field(1)
                .unwrap()
                .rule
                .pattern
                .as_deref(),
            Some(p)
        );
    }
}

#[test]
fn pattern_dot_warns() {
    let m = pattern_model("a.b");
    let w = find(&m, Warning, "t", Some("s"), "[^\\n]");
    assert_eq!(w.line, Some(3));
    let f = m.table("t").unwrap().field(1).unwrap().clone();
    assert_eq!(f.rule.pattern.as_deref(), Some("a.b"));
    assert_eq!(f.mode, Editable);
    for p in [r"a\.b", "[.]", r"[^.]x", r"a[.\]]b"] {
        assert_eq!(pattern_model(p).diagnostics(), &[], "{p}");
    }
}

#[test]
fn pattern_compile_failure_is_read_only() {
    // The config parser already rejects a bad pattern; build the entry by
    // hand so the resolver's own check is what is tested.
    let mut report = parse_config("version = 1\n");
    let at = crate::config::At { line: 7, col: 1 };
    let field = FieldConfig {
        name: "s".into(),
        at,
        mode: None,
        display_name: None,
        help: None,
        field_type: None,
        unit: None,
        nullable: None,
        min: None,
        max: None,
        min_length: None,
        max_length: None,
        pattern: Some(Located {
            value: "(unclosed".into(),
            at,
        }),
        allowed: None,
        empty_is_null: None,
        multiline: None,
    };
    report.config.tables.push(TableEntry::Valid(TableConfig {
        name: "t".into(),
        at,
        mode: None,
        display_name: None,
        description: None,
        object_name: None,
        label: None,
        order_by: vec![],
        field_order: vec![],
        allow_create: None,
        allow_delete: None,
        fields: vec![FieldEntry::Valid(field.clone())],
    }));
    let s = schema_of("CREATE TABLE t (id INTEGER PRIMARY KEY, s TEXT)");
    let m = resolve(&s, &report);
    let e = find(&m, Error, "t", Some("s"), "does not compile");
    assert_eq!(e.line, Some(7));
    let f = m.table("t").unwrap().field(1).unwrap().clone();
    assert_eq!((f.rule.pattern, f.mode), (None, ReadOnly));
}

// ---- create and delete -------------------------------------------------------

#[test]
fn allow_create_blocked_explicit_error() {
    let sql = "CREATE TABLE t (id INTEGER PRIMARY KEY, a TEXT NOT NULL, b TEXT)";
    let m = model(
        sql,
        "version = 1\n[tables.t]\nallow_create = true\n[tables.t.fields.a]\nmode = \"read-only\"\n",
    );
    let e = find(&m, Error, "t", Some("a"), "creating");
    assert_eq!(e.line, Some(3));
    assert!(!m.table("t").unwrap().can_create);
    // Explicit false needs no diagnostic.
    let m = model(
        sql,
        "version = 1\n[tables.t]\nallow_create = false\n[tables.t.fields.a]\nmode = \"read-only\"\n",
    );
    assert_eq!(m.diagnostics(), &[]);
    assert!(!m.table("t").unwrap().can_create);
}

#[test]
fn allow_create_blocked_inherited_warning() {
    let sql = "CREATE TABLE t (id INTEGER PRIMARY KEY, a TEXT NOT NULL, b TEXT NOT NULL DEFAULT 'x', c TEXT NOT NULL)";
    let m = model(
        sql,
        "version = 1\n[tables.t.fields.a]\nmode = \"hidden\"\n[tables.t.fields.b]\nmode = \"read-only\"\n[tables.t.fields.c]\nmode = \"create-only\"\n",
    );
    let w = find(&m, Warning, "t", Some("a"), "creating");
    assert_eq!(w.line, Some(2));
    // Diagnostics may name a hidden column; views never do.
    assert!(w.message.contains("`a`"), "{}", w.message);
    assert_eq!(
        diags(&m, Error),
        [] as [&crate::config::ConfigDiagnostic; 0]
    );
    assert!(!m.table("t").unwrap().can_create);
    // A default, or an editable field, does not block.
    let m = model(
        sql,
        "version = 1\n[tables.t.fields.b]\nmode = \"read-only\"\n",
    );
    assert!(m.table("t").unwrap().can_create);
    // With no entry for the table the warning has no position.
    let m = model(sql, "version = 1\n[defaults]\nfield_mode = \"read-only\"\n");
    let w = find(&m, Warning, "t", Some("a"), "creating");
    assert_eq!(w.line, None);
}

#[test]
fn delete_off_by_default() {
    let m = example(
        "version = 1\n[tables.book]\nallow_delete = true\n[tables.author]\nallow_delete = false\n",
    );
    let v = views(&m);
    assert!(tv(&v, "book").can_delete);
    assert!(!tv(&v, "author").can_delete);
    assert!(!tv(&v, "member").can_delete);
    let m = example("version = 1\n[tables.book]\nmode = \"read-only\"\nallow_delete = true\nallow_create = true\n");
    find(&m, Warning, "book", None, "allow_delete");
    find(&m, Warning, "book", None, "allow_create");
    assert!(!tv(&views(&m), "book").can_delete);
    let m = example("version = 1\n[tables.audit_log]\nmode = \"hidden\"\nallow_delete = true\n");
    find(&m, Warning, "audit_log", None, "allow_delete");
}

fn t_col_of(m: &Model, table: &str, c: &str) -> usize {
    m.schema().table(table).unwrap().column(c).unwrap()
}

/// Every produced default passes `validate` for its field's rule, ignoring
/// bounds (a default may violate them).
fn assert_defaults_validate(t: &ResolvedTable) {
    for f in &t.fields {
        if let Some(v) = &f.default_value {
            let mut rule = f.rule.clone();
            rule.min = None;
            rule.max = None;
            rule.nullable = true;
            assert!(
                crate::rules::validate(&rule, v).is_ok(),
                "{}: {v:?} fails validate",
                f.name
            );
        }
    }
}

#[test]
fn default_literals_parsed() {
    let m = model(
        "CREATE TABLE t (id INTEGER PRIMARY KEY,
          a INTEGER DEFAULT -5, b REAL DEFAULT 1.5, c TEXT DEFAULT 'it''s', d TEXT DEFAULT NULL,
          e BOOLEAN DEFAULT TRUE, f BOOLEAN DEFAULT false, g TEXT DEFAULT CURRENT_TIMESTAMP,
          h INTEGER DEFAULT (1 + 2), i INTEGER, j INTEGER NOT NULL DEFAULT 90, k INTEGER DEFAULT +7)",
        "version = 1\n[tables.t.fields.j]\ntype = \"duration\"\nunit = \"minutes\"\n",
    );
    let t = m.table("t").unwrap();
    let d = |c: usize| {
        (
            t.field(c).unwrap().has_default,
            t.field(c).unwrap().default_value.clone(),
        )
    };
    assert_eq!(d(1), (true, Some(DbValue::Integer("-5".into()))));
    assert_eq!(d(2), (true, Some(DbValue::Real("1.5".into()))));
    assert_eq!(d(3), (true, Some(DbValue::Text("it's".into()))));
    assert_eq!(d(4), (true, Some(DbValue::Null)));
    assert_eq!(d(5), (true, Some(DbValue::Integer("1".into()))));
    assert_eq!(d(6), (true, Some(DbValue::Integer("0".into()))));
    assert_eq!(d(7), (true, None));
    assert_eq!(d(8), (true, None));
    assert_eq!(d(9), (false, None));
    assert_eq!(d(10), (true, Some(DbValue::Duration("1h30m".into()))));
    assert_eq!(d(11), (true, Some(DbValue::Integer("7".into()))));
    assert!(!t.field(10).unwrap().required_on_create);
    assert_defaults_validate(t);
    // Converted by the field's kind, not by the literal's spelling.
    let m = model(
        "CREATE TABLE t (id INTEGER PRIMARY KEY,
          r REAL DEFAULT 0, r2 REAL DEFAULT -3, r3 REAL DEFAULT 'x', t1 TEXT DEFAULT 5, t2 TEXT DEFAULT 2.50,
          i1 INTEGER DEFAULT 'x', i2 INTEGER DEFAULT 1.5, d1 INTEGER DEFAULT 90, d2 INTEGER DEFAULT 'x',
          d3 INTEGER DEFAULT 1.5, b1 BOOLEAN DEFAULT 5, b2 BOOLEAN DEFAULT TRUE, n1 REAL DEFAULT NULL,
          t3 TEXT DEFAULT TRUE)",
        "version = 1\n[tables.t.fields.d1]\ntype = \"duration\"\n[tables.t.fields.d2]\ntype = \"duration\"\n[tables.t.fields.d3]\ntype = \"duration\"\n",
    );
    let t = m.table("t").unwrap();
    let d = |name: &str| {
        let f = t.field(t_col_of(&m, "t", name)).unwrap();
        (f.has_default, f.default_value.clone())
    };
    assert_eq!(d("r"), (true, Some(DbValue::Real("0".into()))));
    assert_eq!(d("r2"), (true, Some(DbValue::Real("-3".into()))));
    assert_eq!(d("r3"), (true, None));
    assert_eq!(d("t1"), (true, Some(DbValue::Text("5".into()))));
    assert_eq!(d("t2"), (true, Some(DbValue::Text("2.50".into()))));
    assert_eq!(d("i1"), (true, None));
    assert_eq!(d("i2"), (true, None));
    assert_eq!(d("d1"), (true, Some(DbValue::Duration("1m30s".into()))));
    assert_eq!(d("d2"), (true, None));
    assert_eq!(d("d3"), (true, None));
    assert_eq!(d("b1"), (true, None));
    assert_eq!(d("b2"), (true, Some(DbValue::Integer("1".into()))));
    assert_eq!(d("n1"), (true, Some(DbValue::Null)));
    assert_eq!(d("t3"), (true, Some(DbValue::Text("1".into()))));
    assert_defaults_validate(t);
    // Not NULL, no default, not an alias.
    let m = model(
        "CREATE TABLE t (id INTEGER PRIMARY KEY, a TEXT NOT NULL)",
        "version = 1\n",
    );
    let t = m.table("t").unwrap();
    assert!(t.field(1).unwrap().required_on_create);
    assert!(!t.field(0).unwrap().required_on_create);
}

// ---- labels and ordering -------------------------------------------------------

fn names(m: &Model, table: &str, cols: &[usize]) -> Vec<String> {
    let t = m.schema().table(table).unwrap();
    cols.iter()
        .map(|&c| t.columns[c].name.as_str().to_owned())
        .collect()
}

#[test]
fn labels_default_and_template() {
    let sql = "CREATE TABLE a (id INTEGER PRIMARY KEY, Title TEXT, name TEXT, label TEXT);
CREATE TABLE b (id INTEGER PRIMARY KEY, n INTEGER, label TEXT);
CREATE TABLE c (id INTEGER PRIMARY KEY, name INTEGER, x TEXT);
CREATE TABLE d (id INTEGER PRIMARY KEY, name TEXT, x TEXT);";
    let m = model(
        sql,
        "version = 1\n[tables.d]\nlabel = \"{x} ({name}) {{ok}}\"\n[tables.d.fields.x]\nmode = \"read-only\"\n",
    );
    assert_eq!(m.diagnostics(), &[]);
    // `name` outranks `title` and `label`.
    assert_eq!(
        names(&m, "a", &m.table("a").unwrap().label.columns()),
        ["name"]
    );
    assert_eq!(
        names(&m, "b", &m.table("b").unwrap().label.columns()),
        ["label"]
    );
    // `name` is not text in c: the key label.
    assert_eq!(
        m.table("c").unwrap().label.parts,
        [] as [crate::model::ResolvedLabelPart; 0]
    );
    assert!(!tv(&views(&m), "c").searchable);
    let d = m.table("d").unwrap();
    assert_eq!(names(&m, "d", &d.label.columns()), ["x", "name"]);
    assert_eq!(d.label.parts[1], ResolvedLabelPart::Literal(" (".into()));
    assert_eq!(
        d.label.parts[3],
        ResolvedLabelPart::Literal(") {ok}".into())
    );
}

#[test]
fn label_hidden_column_rejected() {
    let m = example("version = 1\n[tables.author]\nlabel = \"{last_name} {internal_note}\"\n");
    assert_eq!(m.diagnostics(), &[]);
    let m = example(
        "version = 1\n[tables.author]\nlabel = \"{last_name} {internal_note}\"\n[tables.author.fields.internal_note]\nmode = \"hidden\"\n",
    );
    find(&m, Error, "author", None, "not a visible column");
    // The default label is used: author has no name/title/label column.
    assert_eq!(
        m.table("author").unwrap().label.parts,
        [] as [crate::model::ResolvedLabelPart; 0]
    );
    let m = example("version = 1\n[tables.author]\nlabel = \"{nope}\"\n");
    find(&m, Error, "author", None, "`nope`");
}

#[test]
fn search_skips_durations() {
    let m = model(
        "CREATE TABLE t (id INTEGER PRIMARY KEY, a TEXT, d INTEGER, n INTEGER)",
        "version = 1\n[tables.t]\nlabel = \"{a} {d} {n}\"\n[tables.t.fields.d]\ntype = \"duration\"\n",
    );
    let t = m.table("t").unwrap();
    assert_eq!(names(&m, "t", &t.search), ["a", "n"]);
    assert_eq!(names(&m, "t", &t.label.columns()), ["a", "d", "n"]);
    let m = model(
        "CREATE TABLE t (id INTEGER PRIMARY KEY, d INTEGER)",
        "version = 1\n[tables.t]\nlabel = \"{d}\"\n[tables.t.fields.d]\ntype = \"duration\"\n",
    );
    assert!(!tv(&views(&m), "t").searchable);
}

#[test]
fn order_by_resolution() {
    let sql = "CREATE TABLE t (id INTEGER PRIMARY KEY, a TEXT, b TEXT, c TEXT, h TEXT)";
    let m = model(sql, "version = 1\n[tables.t]\nlabel = \"{b} {a}\"\n");
    let o: Vec<(usize, bool)> = m
        .table("t")
        .unwrap()
        .order
        .iter()
        .map(|o| (o.column, o.descending))
        .collect();
    assert_eq!(o, [(2, false), (1, false)]);
    let m = model(
        sql,
        "version = 1\n[tables.t]\norder_by = [\"c\", { column = \"A\", descending = true }]\n",
    );
    let o: Vec<(usize, bool)> = m
        .table("t")
        .unwrap()
        .order
        .iter()
        .map(|o| (o.column, o.descending))
        .collect();
    assert_eq!(o, [(3, false), (1, true)]);
    // Unknown or hidden: error at the element, default order.
    let m = model(
        sql,
        "version = 1\n[tables.t]\nlabel = \"{b}\"\norder_by = [\"c\", \"h\", \"zz\"]\n[tables.t.fields.h]\nmode = \"hidden\"\n",
    );
    assert_eq!(diags(&m, Error).len(), 2);
    assert_eq!(
        m.table("t")
            .unwrap()
            .order
            .iter()
            .map(|o| o.column)
            .collect::<Vec<_>>(),
        [2]
    );
    // field_order: listed first, rest in column order; a bad name ignores the key.
    let m = model(sql, "version = 1\n[tables.t]\nfield_order = [\"c\", \"a\"]\n[tables.t.fields.b]\nmode = \"hidden\"\n");
    let cols = column_names(&m, "t");
    assert_eq!(cols, ["c", "a", "id", "h"]);
    let m = model(sql, "version = 1\n[tables.t]\nfield_order = [\"c\", \"b\"]\n[tables.t.fields.b]\nmode = \"hidden\"\n");
    find(&m, Error, "t", None, "field_order");
    let cols = column_names(&m, "t");
    assert_eq!(cols, ["id", "a", "c", "h"]);
}

// ---- names and links -----------------------------------------------------------

#[test]
fn unknown_table_and_field_warn() {
    let m = example(
        "version = 1\n[tables.nope]\nmode = \"hidden\"\n[tables.BOOK.fields.nope]\nmin = 1\n[many_to_many.nope]\nenabled = false\n[tables.BOOK.fields.TITLE]\nmax_length = 5\n",
    );
    let w = find(&m, Warning, "nope", None, "unknown table");
    assert_eq!(w.line, Some(2));
    let w = find(&m, Warning, "book", Some("nope"), "no column");
    assert_eq!(w.line, Some(4));
    find(&m, Warning, "nope", None, "unknown junction");
    // Matching is exact, then ASCII case-insensitive.
    assert_eq!(diags(&m, Warning).len(), 3, "{:#?}", m.diagnostics());
    assert_eq!(fv(tv(&views(&m), "book"), "title").rule.max_length, Some(5));
    // Entries on tables that are not managed still resolve nothing.
    assert!(m.table("nope").is_none());
    // Two entries for one table: see `duplicate_table_entries_fail_closed`.
    let m = example(
        "version = 1\n[tables.book]\nmode = \"hidden\"\n[tables.BOOK]\nmode = \"editable\"\n",
    );
    let w = find(&m, Warning, "book", None, "more than once");
    assert_eq!(w.line, Some(4));
}

#[test]
fn skipped_table_named_warns() {
    let sql = "CREATE TABLE a (id INTEGER PRIMARY KEY); CREATE VIEW v AS SELECT * FROM a; CREATE TABLE nokey (rowid, _rowid_, oid);";
    let m = model(
        sql,
        "version = 1\n[tables.v]\nmode = \"hidden\"\n[tables.nokey]\nmode = \"hidden\"\n[tables.sqlite_schema]\nmode = \"hidden\"\n[many_to_many.v]\nenabled = true\n",
    );
    find(&m, Warning, "v", None, "a view");
    find(&m, Warning, "nokey", None, "no usable identity");
    find(&m, Warning, "sqlite_schema", None, "internal");
    assert_eq!(diags(&m, Warning).len(), 4);
    assert!(m.table("a").is_some());
}

#[test]
fn fk_missing_target_warns() {
    let m = model(
        "PRAGMA foreign_keys = OFF;
         CREATE TABLE t (id INTEGER PRIMARY KEY, g INTEGER REFERENCES gone(id), h INTEGER REFERENCES t(nope));",
        "version = 1\n",
    );
    let w = find(&m, Warning, "t", Some("g"), "does not exist");
    assert!(w.line.is_none());
    find(&m, Warning, "t", Some("h"), "does not exist");
    let t = m.table("t").unwrap();
    for c in [1, 2] {
        let f = t.field(c).unwrap();
        assert!(f.foreign_key.is_none());
        assert_eq!(f.mode, Editable);
    }
    assert!(t.can_create);
}

#[test]
fn fk_target_not_a_key_warns() {
    let m = model(
        "CREATE TABLE p (id INTEGER PRIMARY KEY, x INTEGER);
         CREATE TABLE t (id INTEGER PRIMARY KEY, px INTEGER NOT NULL REFERENCES p(x));",
        "version = 1\n[tables.t]\nallow_delete = true\n",
    );
    let w = find(
        &m,
        Warning,
        "t",
        Some("px"),
        "does not reference a primary or unique key",
    );
    assert_eq!(
        w.message,
        "the foreign key from `t.px` to `p(x)` does not reference a primary or unique key; SQLite will refuse changes that touch it"
    );
    let t = m.table("t").unwrap();
    let f = t.field(1).unwrap();
    assert_eq!(f.mode, ReadOnly);
    assert!(f.foreign_key.is_none());
    // create and delete are unaffected by the warning itself; the NOT NULL
    // read-only column blocks create on its own account.
    assert!(t.can_delete);
    let m = model(
        "CREATE TABLE p (id INTEGER PRIMARY KEY, x INTEGER);
         CREATE TABLE t (id INTEGER PRIMARY KEY, px INTEGER REFERENCES p(x));",
        "version = 1\n",
    );
    assert!(m.table("t").unwrap().can_create);
    assert_eq!(diags(&m, Warning).len(), 1);
}

#[test]
fn integer_primary_key_desc_is_not_an_alias() {
    let m = model(
        "CREATE TABLE t (id INTEGER PRIMARY KEY DESC, v TEXT)",
        "version = 1\n",
    );
    assert_eq!(m.diagnostics(), &[]);
    let t = m.table("t").unwrap();
    let id = t.field(0).unwrap();
    assert_eq!(id.mode, CreateOnly);
    assert!(id.is_key);
    assert!(id.rule.nullable);
    assert!(matches!(
        t.identity,
        Identity::Rowid {
            alias_column: None,
            ..
        }
    ));
    assert!(t.can_create);
    // Declared NOT NULL: required on create, and create-only keeps it possible.
    let m = model(
        "CREATE TABLE t (id INTEGER PRIMARY KEY DESC NOT NULL, v TEXT)",
        "version = 1\n",
    );
    let id = m.table("t").unwrap().field(0).unwrap();
    assert!(id.required_on_create && !id.rule.nullable);
    assert!(m.table("t").unwrap().can_create);
}

#[test]
fn reverse_relations() {
    let m = example(EXAMPLE);
    let v = views(&m);
    let book = tv(&v, "book");
    let from: Vec<(&str, &str, &str)> = book
        .referenced_by
        .iter()
        .map(|r| {
            (
                r.table.as_str(),
                r.via.as_str(),
                r.table_display_name.as_str(),
            )
        })
        .collect();
    assert_eq!(from, [("loan", "Book", "loan")]);
    assert!(book.referenced_by[0].relation.starts_with("fk:loan:"));
    // Self reference, and a hidden referencing table.
    let m = model(
        "CREATE TABLE n (id INTEGER PRIMARY KEY, parent INTEGER REFERENCES n(id), also INTEGER REFERENCES n(id));
         CREATE TABLE secret (id INTEGER PRIMARY KEY, n INTEGER REFERENCES n(id));
         CREATE TABLE sneaky (id INTEGER PRIMARY KEY, n INTEGER REFERENCES n(id));",
        "version = 1\n[tables.n.fields.parent]\ndisplay_name = \"Parent\"\n[tables.secret]\nmode = \"hidden\"\n[tables.sneaky.fields.n]\nmode = \"hidden\"\n",
    );
    let n = tv(&views(&m), "n").clone();
    let mut via: Vec<&str> = n.referenced_by.iter().map(|r| r.via.as_str()).collect();
    via.sort_unstable();
    assert_eq!(via, ["Parent", "also"]);
    assert!(n.referenced_by.iter().all(|r| r.table == "n"));
    assert_ne!(n.referenced_by[0].relation, n.referenced_by[1].relation);
    assert!(m.table("secret").is_none());
}

// ---- many-to-many ------------------------------------------------------------------

const M2M: &str = "CREATE TABLE a (id INTEGER PRIMARY KEY, name TEXT);
CREATE TABLE b (id INTEGER PRIMARY KEY, name TEXT);
CREATE TABLE ab (a_id INTEGER NOT NULL REFERENCES a(id), b_id INTEGER NOT NULL REFERENCES b(id), PRIMARY KEY (a_id, b_id));";

#[test]
fn m2m_detected() {
    let m = model(M2M, "version = 1\n");
    assert_eq!(m.diagnostics(), &[]);
    assert!(m.table("ab").is_none());
    let a = m.table("a").unwrap();
    let s = &a.many_to_many[0];
    assert_eq!(
        (s.relation.as_str(), s.other_table_name.as_str()),
        ("m2m:ab", "b")
    );
    assert_eq!(s.heading, "b");
    assert!(s.can_add && s.mode == RelationMode::Editable);
    assert_eq!(
        s.junction_identity,
        m.schema().table("ab").unwrap().identity
    );
    assert_eq!(
        (s.this_column, s.other_column, s.this_key, s.other_key),
        (0, 1, 0, 0)
    );
    let b = m.table("b").unwrap();
    assert_eq!(
        (
            b.many_to_many[0].this_column,
            b.many_to_many[0].other_column
        ),
        (1, 0)
    );
    assert_eq!(a.referenced_by, [] as [crate::model::ReverseRelation; 0]);
    // An optional rowid alias is allowed; a unique index works as the pair.
    let m = model(
        "CREATE TABLE a (id INTEGER PRIMARY KEY); CREATE TABLE b (id INTEGER PRIMARY KEY);
         CREATE TABLE ab (id INTEGER PRIMARY KEY, x INTEGER REFERENCES a(id), y INTEGER REFERENCES b(id), UNIQUE (y, x));",
        "version = 1\n",
    );
    assert!(m.table("ab").is_none());
    assert_eq!(
        m.table("a").unwrap().many_to_many[0].link_fields,
        [] as [crate::model::ResolvedField; 0]
    );
    // No unique pair: an ordinary table.
    let m = model(
        "CREATE TABLE a (id INTEGER PRIMARY KEY); CREATE TABLE b (id INTEGER PRIMARY KEY);
         CREATE TABLE ab (x INTEGER REFERENCES a(id), y INTEGER REFERENCES b(id));",
        "version = 1\n",
    );
    assert!(m.table("ab").is_some());
    assert_eq!(
        m.table("a").unwrap().many_to_many,
        [] as [crate::model::M2mSide; 0]
    );
}

const WIDE: &str = "CREATE TABLE a (id INTEGER PRIMARY KEY, name TEXT);
CREATE TABLE b (id INTEGER PRIMARY KEY, name TEXT);
CREATE TABLE c (id INTEGER PRIMARY KEY);
CREATE TABLE ab (
  a_id INTEGER NOT NULL REFERENCES a(id), b_id INTEGER NOT NULL REFERENCES b(id),
  c_id INTEGER REFERENCES c(id), note TEXT NOT NULL DEFAULT 'x', len INTEGER, PRIMARY KEY (a_id, b_id));";

#[test]
fn m2m_declared_with_link_fields() {
    // Not detected without an entry: extra columns.
    let m = model(WIDE, "version = 1\n");
    assert!(m.table("ab").is_some());
    let m = model(
        WIDE,
        r#"version = 1
[many_to_many.ab]
columns = ["b_id", "a_id"]
headings = { a = "Bs", b = "As" }
[tables.ab.fields.len]
type = "duration"
unit = "minutes"
min = "5m"
[tables.ab.fields.note]
mode = "create-only"
"#,
    );
    assert_eq!(m.diagnostics(), &[]);
    assert!(m.table("ab").is_none());
    let a = m.table("a").unwrap();
    let s = &a.many_to_many[0];
    // `columns` order picks the sides' order, not which table is which.
    assert_eq!(s.other_table_name, "b");
    assert_eq!(s.heading, "Bs");
    assert_eq!(m.table("b").unwrap().many_to_many[0].heading, "As");
    let cols: Vec<&str> = s.link_fields.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(cols, ["c_id", "note", "len"]);
    assert!(s.link_fields[0].foreign_key.is_some());
    assert_eq!(s.link_fields[1].mode, CreateOnly);
    assert_eq!(s.link_fields[2].kind, FieldKind::Duration);
    assert_eq!(s.link_fields[2].rule.min.as_deref(), Some("5m"));
    assert_eq!(s.mode, RelationMode::Editable);
    let v = views(&m);
    let view = &tv(&v, "a").many_to_many[0];
    assert_eq!(view.link_fields.len(), 3);
    // A bad heading side warns.
    let m = model(
        WIDE,
        "version = 1\n[many_to_many.ab]\ncolumns = [\"a_id\", \"b_id\"]\nheadings = { zz = \"Z\" }\n",
    );
    find(&m, Warning, "ab", None, "zz");
    // Declared without `columns` but with three FKs: not usable.
    let m = model(
        WIDE,
        "version = 1\n[many_to_many.ab]\nmode = \"editable\"\n",
    );
    find(&m, Warning, "ab", None, "columns");
    assert_eq!(
        m.table("a").unwrap().many_to_many,
        [] as [crate::model::M2mSide; 0]
    );
    // Unknown column.
    let m = model(
        WIDE,
        "version = 1\n[many_to_many.ab]\ncolumns = [\"a_id\", \"zz\"]\n",
    );
    find(&m, Warning, "ab", None, "zz");
}

#[test]
fn m2m_link_field_blocks_adding() {
    let cfg = |mode: &str, field: &str| {
        format!(
            "version = 1\n[many_to_many.ab]\ncolumns = [\"a_id\", \"b_id\"]\n{mode}\n[tables.ab.fields.note]\nmode = \"{field}\"\n"
        )
    };
    // `note` is NOT NULL with a default, so it never blocks.
    let m = model(WIDE, &cfg("", "hidden"));
    assert!(m.table("a").unwrap().many_to_many[0].can_add);
    let sql = WIDE.replace("note TEXT NOT NULL DEFAULT 'x'", "note TEXT NOT NULL");
    let m = model(&sql, &cfg("", "hidden"));
    let warn = find(&m, Warning, "ab", Some("note"), "links cannot be added");
    assert_eq!(warn.line, Some(2));
    let side = &m.table("a").unwrap().many_to_many[0];
    assert!(!side.can_add);
    assert_eq!(side.mode, RelationMode::ReadOnly);
    assert!(side
        .link_fields
        .iter()
        .any(|f| f.name == "note" && f.mode == FieldMode::Hidden));
    // Hidden link fields never reach a view.
    let vs = views(&m);
    assert!(tv(&vs, "a").many_to_many[0]
        .link_fields
        .iter()
        .all(|f| f.column != "note"));
    // An explicit relation mode makes it an error at the mode key.
    let m = model(&sql, &cfg("mode = \"editable\"", "read-only"));
    let err = find(&m, Error, "ab", Some("note"), "links cannot be added");
    assert_eq!(err.line, Some(4));
    // Editable or create-only link fields supply the column.
    for field in ["editable", "create-only"] {
        let m = model(&sql, &cfg("", field));
        assert_eq!(m.diagnostics(), &[], "{field}");
        assert!(m.table("a").unwrap().many_to_many[0].can_add);
    }
    // A read-only relation needs no diagnostic.
    let m = model(&sql, &cfg("mode = \"read-only\"", "read-only"));
    assert_eq!(m.diagnostics(), &[]);
    // A rejected junction entry makes the link fields read-only.
    let m = model(&sql, "version = 1\n[many_to_many.ab]\ncolumns = [\"a_id\", \"b_id\"]\n[tables.ab]\nbogus = 1\n[tables.ab.fields.note]\nmode = \"editable\"\n");
    let side = &m.table("a").unwrap().many_to_many[0];
    assert!(side.link_fields.iter().all(|f| f.mode == ReadOnly));
    assert!(!side.can_add);
    // A junction listed read-only does too; the default (not listed) does not.
    let m = model(&sql, "version = 1\n[many_to_many.ab]\ncolumns = [\"a_id\", \"b_id\"]\n[tables.ab]\nmode = \"read-only\"\n");
    assert!(m.table("ab").is_some());
    assert!(m.table("a").unwrap().many_to_many[0]
        .link_fields
        .iter()
        .all(|f| f.mode == ReadOnly));
}

#[test]
fn m2m_self_reference_not_supported() {
    let sql = "CREATE TABLE n (id INTEGER PRIMARY KEY);
CREATE TABLE nn (x INTEGER NOT NULL REFERENCES n(id), y INTEGER NOT NULL REFERENCES n(id), PRIMARY KEY (x, y));";
    let m = model(sql, "version = 1\n");
    assert!(m.table("nn").is_some());
    assert_eq!(
        m.table("n").unwrap().many_to_many,
        [] as [crate::model::M2mSide; 0]
    );
    assert_eq!(m.table("n").unwrap().referenced_by.len(), 2);
    assert_eq!(m.diagnostics().len(), 0);
    let m = model(sql, "version = 1\n[many_to_many.nn]\nmode = \"editable\"\n");
    let w = find(&m, Warning, "nn", None, "self-referential");
    assert_eq!(w.line, Some(2));
    assert!(m.table("nn").is_some());
}

#[test]
fn m2m_disabled() {
    let m = example("version = 1\n[many_to_many.book_genre]\nenabled = false\n");
    assert_eq!(m.diagnostics(), &[]);
    let v = views(&m);
    let bg = tv(&v, "book_genre");
    assert_eq!(bg.mode, TableMode::Editable);
    assert!(tv(&v, "book")
        .many_to_many
        .iter()
        .all(|s| s.relation != "m2m:book_genre"));
    assert!(tv(&v, "book")
        .referenced_by
        .iter()
        .any(|r| r.table == "book_genre"));
    assert!(tv(&v, "genre")
        .referenced_by
        .iter()
        .any(|r| r.table == "book_genre"));
}

#[test]
fn m2m_hidden_side() {
    let m = example("version = 1\n[tables.genre]\nmode = \"hidden\"\n");
    let v = views(&m);
    assert_eq!(
        tv(&v, "book").many_to_many,
        [] as [crate::model::views::M2mView; 0]
    );
    assert!(m.table("genre").is_none());
    // Hidden never contributes a reverse relation.
    assert!(tv(&v, "book")
        .referenced_by
        .iter()
        .all(|r| r.table != "genre"));
    let m = example("version = 1\n[tables.book]\nmode = \"hidden\"\n");
    let v = views(&m);
    assert_eq!(
        tv(&v, "genre").many_to_many,
        [] as [crate::model::views::M2mView; 0]
    );
    assert_eq!(
        tv(&v, "author").many_to_many,
        [] as [crate::model::views::M2mView; 0]
    );
    // A hidden relation shows nowhere.
    let m = example("version = 1\n[many_to_many.book_genre]\nmode = \"hidden\"\n");
    let v = views(&m);
    assert_eq!(
        tv(&v, "book").many_to_many,
        [] as [crate::model::views::M2mView; 0]
    );
    assert_eq!(
        tv(&v, "genre").many_to_many,
        [] as [crate::model::views::M2mView; 0]
    );
    // An FK into a hidden table has no link.
    let m = example("version = 1\n[tables.author]\nmode = \"hidden\"\n");
    let f = fv(tv(&views(&m), "book"), "author_id").clone();
    assert!(f.foreign_key.is_none());
    assert_eq!(f.mode, ReadOnly);
}

#[test]
fn m2m_read_only_relation() {
    let m = example(
        "version = 1\n[many_to_many.contribution]\nmode = \"read-only\"\n[tables.contribution.fields.role]\nmode = \"editable\"\n",
    );
    assert_eq!(m.diagnostics(), &[]);
    let v = views(&m);
    for t in ["book", "author"] {
        let s = tv(&v, t)
            .many_to_many
            .iter()
            .find(|s| s.relation == "m2m:contribution")
            .unwrap()
            .clone();
        assert_eq!(s.mode, RelationMode::ReadOnly);
        assert!(s.link_fields.iter().all(|f| f.mode == ReadOnly), "{t}");
    }
    // The other relation is unaffected.
    assert_eq!(tv(&v, "genre").many_to_many[0].mode, RelationMode::Editable);
    // A read-only table makes its side read-only; the other side stays editable.
    let m = example("version = 1\n[tables.genre]\nmode = \"read-only\"\n");
    let v = views(&m);
    assert_eq!(tv(&v, "genre").many_to_many[0].mode, RelationMode::ReadOnly);
    assert_eq!(tv(&v, "book").many_to_many[0].mode, RelationMode::Editable);
    // A rejected relation entry is read-only.
    let m = example("version = 1\n[many_to_many.book_genre]\nbogus = 1\n");
    assert_eq!(
        tv(&views(&m), "book").many_to_many[0].mode,
        RelationMode::ReadOnly
    );
}

#[test]
fn diagnostics_sorted() {
    let m = example(
        "version = 1\n[tables.zzz]\nmode = \"hidden\"\n[tables.book.fields.title]\nmin = 1\n[tables.author]\nbogus = 1\n[tables.aaa]\nmode = \"hidden\"\n",
    );
    let d = m.diagnostics();
    assert!(d.len() >= 4);
    let keys: Vec<_> = d.iter().map(|d| (d.line.is_none(), d.line)).collect();
    assert!(keys.is_sorted(), "{d:#?}");
    // Two diagnostics without a line come out ordered by table, then column.
    let m = model(
        "PRAGMA foreign_keys = OFF;
         CREATE TABLE b (id INTEGER PRIMARY KEY, y INTEGER REFERENCES nope(id), x INTEGER REFERENCES nope(id));
         CREATE TABLE a (id INTEGER PRIMARY KEY, z INTEGER REFERENCES nope(id));",
        "version = 1\n",
    );
    let order: Vec<(&str, &str)> = m
        .diagnostics()
        .iter()
        .map(|d| (d.table.as_deref().unwrap(), d.column.as_deref().unwrap()))
        .collect();
    assert_eq!(order, [("a", "z"), ("b", "x"), ("b", "y")]);
}

// ---- canonical rules ---------------------------------------------------------------

fn assert_canonical(m: &Model) {
    let mut seen = 0;
    let mut check = |f: &ResolvedField| {
        let r = &f.rule;
        for bound in [&r.min, &r.max].into_iter().flatten() {
            seen += 1;
            match r.kind {
                FieldKind::Integer => {
                    let n: i64 = bound.parse().unwrap();
                    assert_eq!(&n.to_string(), bound);
                    assert!(crate::rules::canonical_integer_for_tests(bound).is_some());
                }
                FieldKind::Real => {
                    let x = crate::rules::parse_real_for_tests(bound);
                    assert!(x.is_some(), "{bound}");
                }
                FieldKind::Duration => {
                    let unit = r.unit.unwrap();
                    let n = crate::rules::duration::parse(bound, unit).unwrap();
                    assert_eq!(&crate::rules::duration::format(n, unit), bound);
                }
                other => panic!("{other:?} has a bound"),
            }
        }
        if r.kind == FieldKind::Duration {
            assert!(r.unit.is_some());
        } else {
            assert!(r.unit.is_none());
        }
    };
    for t in m.tables() {
        for f in &t.fields {
            check(f);
        }
        for s in &t.many_to_many {
            for f in &s.link_fields {
                check(f);
            }
        }
    }
    assert!(seen > 0);
}

#[test]
fn resolved_rules_are_canonical() {
    assert_canonical(&example(EXAMPLE));
    let m = model(
        "CREATE TABLE t (id INTEGER PRIMARY KEY, i INTEGER, r REAL, d INTEGER, e INTEGER, f INTEGER, g REAL)",
        r#"version = 1
[tables.t.fields.i]
min = -9223372036854775808
max = 9223372036854775807
[tables.t.fields.r]
min = -1e3
max = 2.50
[tables.t.fields.g]
min = 7
max = 1e21
[tables.t.fields.d]
type = "duration"
unit = "milliseconds"
min = "1d 2h"
max = "1d2h3m4s5ms"
[tables.t.fields.e]
type = "duration"
unit = "days"
min = -3
max = "10d"
[tables.t.fields.f]
type = "duration"
min = "0"
max = "90m"
"#,
    );
    assert_eq!(m.diagnostics(), &[]);
    assert_canonical(&m);
    let t = m.table("t").unwrap();
    assert_eq!(t.field(3).unwrap().rule.min.as_deref(), Some("1d2h"));
    assert_eq!(t.field(5).unwrap().rule.min.as_deref(), Some("0s"));
    assert_eq!(t.field(5).unwrap().rule.max.as_deref(), Some("1h30m"));
    assert_eq!(t.field(2).unwrap().rule.min.as_deref(), Some("-1000"));
}

// ---- review round 1 ---------------------------------------------------------------

#[test]
fn duplicate_table_entries_fail_closed() {
    // One says hidden: hidden, whichever comes first.
    for cfg in [
        "version = 1\n[tables.book]\nmode = \"hidden\"\n[tables.BOOK]\nmode = \"editable\"\n",
        "version = 1\n[tables.book]\nmode = \"editable\"\n[tables.BOOK]\nmode = \"hidden\"\n",
    ] {
        let m = example(cfg);
        assert!(m.table("book").is_none(), "{cfg}");
        let w = find(&m, Warning, "book", None, "more than once");
        assert_eq!(w.line, Some(4));
        let on_book = diags(&m, Warning)
            .into_iter()
            .filter(|d| d.table.as_deref() == Some("book"))
            .count();
        assert_eq!(on_book, 1, "{:#?}", m.diagnostics());
    }
    // Neither says hidden: a rejected entry, so a read-only table.
    let m = example(
        "version = 1\n[tables.member]\nmode = \"editable\"\nallow_delete = true\n[tables.MEMBER]\ndisplay_name = \"Patron\"\n",
    );
    let w = find(&m, Warning, "member", None, "more than once");
    assert_eq!(w.line, Some(5));
    let v = views(&m);
    let member = tv(&v, "member");
    assert_eq!(member.mode, TableMode::ReadOnly);
    assert!(!member.can_create && !member.can_delete);
    assert_eq!(member.display_name, "member");
    assert!(member.fields.iter().all(|f| f.mode == ReadOnly));
    // Hidden fields stay hidden through the merge.
    let m = example(
        "version = 1\n[tables.author]\n[tables.author.fields.internal_note]\nmode = \"hidden\"\n[tables.AUTHOR]\ndisplay_name = \"Writer\"\n",
    );
    let v = views(&m);
    let author = tv(&v, "author");
    assert_eq!(author.mode, TableMode::ReadOnly);
    assert!(!has_field(author, "internal_note"));
    assert!(author.fields.iter().all(|f| f.mode == ReadOnly));
}

#[test]
fn duplicate_field_entries_fail_closed() {
    // One says hidden: hidden, whichever comes first.
    for cfg in [
        "version = 1\n[tables.book.fields.title]\nmax_length = 5\n[tables.book.fields.TITLE]\nmode = \"hidden\"\n",
        "version = 1\n[tables.book.fields.title]\nmode = \"hidden\"\n[tables.book.fields.TITLE]\nmax_length = 5\n",
    ] {
        let m = example(cfg);
        assert!(!has_field(tv(&views(&m), "book"), "title"), "{cfg}");
        let w = find(&m, Warning, "book", Some("title"), "more than once");
        assert_eq!(w.line, Some(4));
    }
    // Neither says hidden: the field is read-only and neither entry applies.
    let m = example(
        "version = 1\n[tables.book.fields.title]\nmax_length = 5\n[tables.book.fields.TITLE]\ndisplay_name = \"Name\"\n",
    );
    let w = find(&m, Warning, "book", Some("title"), "more than once");
    assert_eq!(w.line, Some(4));
    let v = views(&m);
    let f = fv(tv(&v, "book"), "title");
    assert_eq!(f.mode, ReadOnly);
    assert_eq!(f.rule.max_length, None);
    assert_eq!(f.display_name, "title");
    // The other fields are untouched.
    assert_eq!(fv(tv(&v, "book"), "pages").mode, Editable);
}

#[test]
fn duplicate_m2m_entries_fail_closed() {
    // One says hidden: the relation shows nowhere, whichever comes first.
    for cfg in [
        "version = 1\n[many_to_many.book_genre]\nmode = \"hidden\"\n[many_to_many.BOOK_GENRE]\nmode = \"editable\"\n",
        "version = 1\n[many_to_many.book_genre]\nmode = \"editable\"\n[many_to_many.BOOK_GENRE]\nmode = \"hidden\"\n",
    ] {
        let m = example(cfg);
        let v = views(&m);
        assert!(tv(&v, "book").many_to_many.is_empty(), "{cfg}");
        assert!(tv(&v, "genre").many_to_many.is_empty(), "{cfg}");
        let w = find(&m, Warning, "book_genre", None, "more than once");
        assert_eq!(w.line, Some(4));
    }
    // A declaration of the columns survives the merge, so it is still hidden.
    let m = example(
        "version = 1\n[many_to_many.contribution]\ncolumns = [\"book_id\", \"author_id\"]\n[many_to_many.CONTRIBUTION]\nmode = \"hidden\"\n",
    );
    let v = views(&m);
    assert!(tv(&v, "book")
        .many_to_many
        .iter()
        .all(|r| r.relation != "m2m:contribution"));
    // Neither says hidden: a rejected entry, so a read-only relation.
    let m = example(
        "version = 1\n[many_to_many.book_genre]\nmode = \"editable\"\n[many_to_many.BOOK_GENRE]\nenabled = true\n",
    );
    let w = find(&m, Warning, "book_genre", None, "more than once");
    assert_eq!(w.line, Some(4));
    let v = views(&m);
    for (t, other) in [("book", "genre"), ("genre", "book")] {
        let r = &tv(&v, t).many_to_many[0];
        assert_eq!(r.other_table, other);
        assert_eq!(r.mode, RelationMode::ReadOnly);
    }
    // A declared junction stays a read-only relation, not an editable table.
    let m = example(
        "version = 1\n[many_to_many.contribution]\nmode = \"editable\"\n[many_to_many.CONTRIBUTION]\ncolumns = [\"book_id\", \"author_id\"]\n",
    );
    let w = find(&m, Warning, "contribution", None, "more than once");
    assert_eq!(w.line, Some(4));
    let v = views(&m);
    assert!(v.iter().all(|t| t.name != "contribution"));
    for t in ["book", "author"] {
        let r = tv(&v, t)
            .many_to_many
            .iter()
            .find(|r| r.relation == "m2m:contribution")
            .unwrap();
        assert_eq!(r.mode, RelationMode::ReadOnly, "{t}");
        assert!(r.link_fields.iter().all(|f| f.mode == ReadOnly), "{t}");
    }
    // One entry disabling the relation does not make it an editable table.
    for cfg in [
        "version = 1\n[many_to_many.contribution]\nenabled = false\n[many_to_many.CONTRIBUTION]\ncolumns = [\"book_id\", \"author_id\"]\n",
        "version = 1\n[many_to_many.contribution]\nenabled = false\n[many_to_many.CONTRIBUTION]\ncolumns = [\"book_id\", \"author_id\"]\nmode = \"hidden\"\n",
    ] {
        let m = example(cfg);
        let v = views(&m);
        assert!(v.iter().all(|t| t.name != "contribution"), "{cfg}");
    }
    // Hidden merge: the relation is used, and hidden.
    let m = example(
        "version = 1\n[many_to_many.contribution]\nenabled = false\n[many_to_many.CONTRIBUTION]\ncolumns = [\"book_id\", \"author_id\"]\nmode = \"hidden\"\n",
    );
    let v = views(&m);
    assert!(tv(&v, "book")
        .many_to_many
        .iter()
        .all(|r| r.relation != "m2m:contribution"));
}

#[test]
fn rejected_declared_junction_is_read_only_table() {
    // `contribution` has extra columns, so only a valid entry declares it.
    let m = example("version = 1\n[many_to_many.contribution]\nbogus = 1\n");
    assert!(diags(&m, Error).iter().any(|d| d.message.contains("bogus")));
    let v = views(&m);
    let c = tv(&v, "contribution");
    assert_eq!(c.mode, TableMode::ReadOnly);
    assert!(!c.can_create && !c.can_delete);
    assert!(c.fields.iter().all(|f| f.mode == ReadOnly));
    for t in ["book", "author"] {
        assert!(tv(&v, t)
            .many_to_many
            .iter()
            .all(|r| r.relation != "m2m:contribution"));
    }
}

#[test]
fn rejected_detected_junction_stays_read_only_relation() {
    let m = example("version = 1\n[many_to_many.book_genre]\nbogus = 1\n");
    assert!(diags(&m, Error).iter().any(|d| d.message.contains("bogus")));
    let v = views(&m);
    assert!(v.iter().all(|t| t.name != "book_genre"));
    for t in ["book", "genre"] {
        let r = &tv(&v, t).many_to_many[0];
        assert_eq!(r.relation, "m2m:book_genre");
        assert_eq!(r.mode, RelationMode::ReadOnly);
    }
}

/// Every link names a table that has a view.
fn assert_links_resolve(m: &Model) {
    for t in m.table_views() {
        let all = t
            .fields
            .iter()
            .chain(t.many_to_many.iter().flat_map(|r| r.link_fields.iter()));
        for f in all {
            if let Some(fk) = &f.foreign_key {
                assert!(m.table(&fk.table).is_some(), "{}.{}", t.name, f.column);
                assert!(fk.navigable && fk.table_display_name.is_some());
            }
        }
    }
}

#[test]
fn fk_into_hidden_table_is_plain() {
    let sql = "CREATE TABLE secret_vault (id INTEGER PRIMARY KEY, code TEXT);
         CREATE TABLE t (id INTEGER PRIMARY KEY, sv INTEGER REFERENCES secret_vault(id), name TEXT);
         CREATE TABLE u (id INTEGER PRIMARY KEY, t_id INTEGER REFERENCES t(id));";
    let m = model(
        sql,
        "version = 1\n[tables.secret_vault]\nmode = \"hidden\"\n",
    );
    assert_eq!(m.diagnostics(), &[]);
    let v = views(&m);
    let f = fv(tv(&v, "t"), "sv");
    assert!(f.foreign_key.is_none());
    assert_eq!(f.mode, ReadOnly);
    for t in &v {
        let json = serde_json::to_string(t).unwrap();
        assert!(!json.contains("secret_vault"), "{}: {json}", t.name);
    }
    // Other links are unaffected.
    assert!(fv(tv(&v, "u"), "t_id").foreign_key.is_some());
    assert_links_resolve(&m);
    // An explicit `editable` is still an error.
    let m = model(
        sql,
        "version = 1\n[tables.secret_vault]\nmode = \"hidden\"\n[tables.t.fields.sv]\nmode = \"editable\"\n",
    );
    find(&m, Error, "t", Some("sv"), "always read-only");
    // With nothing hidden it is a link.
    let m = model(sql, "version = 1\n");
    assert!(fv(tv(&views(&m), "t"), "sv").foreign_key.is_some());
    assert_links_resolve(&m);
    // The example: every link resolves to a table that has a view.
    assert_links_resolve(&example(EXAMPLE));
}

#[test]
fn fk_into_unlisted_junction_is_plain() {
    let sql = "CREATE TABLE a (id INTEGER PRIMARY KEY, n TEXT);
         CREATE TABLE b (id INTEGER PRIMARY KEY, n TEXT);
         CREATE TABLE ab (id INTEGER PRIMARY KEY,
           a_id INTEGER NOT NULL REFERENCES a(id), b_id INTEGER NOT NULL REFERENCES b(id),
           UNIQUE (a_id, b_id));
         CREATE TABLE t (id INTEGER PRIMARY KEY, ab_id INTEGER REFERENCES ab(id));";
    let m = model(sql, "version = 1\n");
    assert!(m.table("ab").is_none());
    let v = views(&m);
    let f = fv(tv(&v, "t"), "ab_id");
    assert!(f.foreign_key.is_none());
    assert_eq!(f.mode, ReadOnly);
    for t in &v {
        assert!(
            !serde_json::to_string(t).unwrap().contains("\"ab\""),
            "{}",
            t.name
        );
    }
    assert_links_resolve(&m);
    // Listing the junction gives it a view, and the link appears.
    let m = model(sql, "version = 1\n[tables.ab]\nmode = \"editable\"\n");
    let f = fv(tv(&views(&m), "t"), "ab_id").clone();
    assert_eq!(f.foreign_key.as_ref().unwrap().table, "ab");
    assert_eq!(f.mode, Editable);
    assert_links_resolve(&m);
    // `enabled = false` makes it an ordinary table too.
    let m = model(sql, "version = 1\n[many_to_many.ab]\nenabled = false\n");
    assert!(fv(tv(&views(&m), "t"), "ab_id").foreign_key.is_some());
    assert_links_resolve(&m);
}

#[test]
fn pattern_scanner_leading_bracket_rejected() {
    for p in ["[]a]", "[^]a]", "[]]x", "[^]]x"] {
        let m = pattern_model(p);
        let e = diags(&m, Error);
        assert_eq!(e.len(), 1, "{p}: {:#?}", m.diagnostics());
        assert!(e[0].message.contains("\\]"), "{p}: {}", e[0].message);
        assert_eq!(e[0].line, Some(3), "{p}");
        let f = m.table("t").unwrap().field(1).unwrap().clone();
        assert_eq!((f.rule.pattern, f.mode), (None, ReadOnly), "{p}");
    }
}

#[test]
fn pattern_scanner_rejects_dialect_differences() {
    use super::rule::scan_pattern;
    let cases: [(&str, &str); 32] = [
        (r"[a[bc]]", "nested class"),
        (r"[[:alpha:]]", "POSIX"),
        (r"[a&&b]", "`&&`"),
        (r"[a--b]", "`--`"),
        (r"[a~~b]", "`~~`"),
        (r"(?i)a", "inline flag"),
        (r"(?x:a)", "inline flag"),
        (r"(?=a)", "look-around"),
        (r"(?!a)", "look-around"),
        (r"(?<n>a)", "look-around"),
        (r"\Aa", r"`\A`"),
        (r"a\z", r"`\z`"),
        (r"a\Z", r"`\Z`"),
        (r"\p{L}", r"`\p`"),
        (r"\P{L}", r"`\P`"),
        (r"\pL", r"`\p`"),
        (r"(a)\1", "backreference"),
        (r"(a)\9", "backreference"),
        (r"\@", r"`\@`"),
        (r"\0", r"`\0`"),
        (r"\u{41}", r"`\u`"),
        (r"\x4", "two hex digits"),
        (r"\xZZ", "two hex digits"),
        (r"[\d]", "explicit class"),
        (r"[]a]", r"`\]`"),
        (r"[^]a]", r"`\]`"),
        (r"a\-b", r"`\-`"),
        (r"a]", r"`\]`"),
        (r"a}", r"`\}`"),
        (r"a{", r"`\{`"),
        (r"a{,3}", r"`\{`"),
        (r"a{3,2}", r"`\{`"),
    ];
    for (p, text) in cases {
        let problem = scan_pattern(p).problem;
        assert!(
            problem.as_deref().is_some_and(|m| m.contains(text)),
            "{p}: {problem:?}"
        );
    }
    // Through the resolver: an error at the key, the pattern dropped, the
    // field read-only. (Patterns Rust itself refuses are rejected earlier, by
    // the config parser.)
    for p in [
        r"[a[bc]]",
        r"[[:alpha:]]",
        r"(?i)a",
        r"(?=a)",
        r"\Aa",
        r"\pL",
        r"[a&&b]",
    ] {
        let m = pattern_model(p);
        let e = diags(&m, Error);
        assert_eq!(e.len(), 1, "{p}: {:#?}", m.diagnostics());
        assert_eq!(e[0].line, Some(3), "{p}");
        let f = m.table("t").unwrap().field(1).unwrap().clone();
        assert_eq!((f.rule.pattern, f.mode), (None, ReadOnly), "{p}");
    }
}

#[test]
fn pattern_scanner_allows_common_subset() {
    use super::rule::scan_pattern;
    for p in [
        r"[A-Za-z0-9._%+-]+",
        r"a\.b",
        r"\x41",
        r"(?:ab)+",
        r"[^\]]",
        r"a{2,3}",
        r"\t\n\r\f\v",
        r"\(\)\[\]\{\}\|\/\^\$\*\+\?",
        r"[a\-b]",
        r"\/",
        r"[\]]",
        r"a{2}",
        r"a{2,}",
        r"a|b",
        r"[^@ \t\r\n]+@[^@ \t\r\n]+",
    ] {
        let scan = scan_pattern(p);
        assert_eq!(scan.problem, None, "{p}");
        assert!(!scan.dot, "{p}");
        let m = pattern_model(p);
        assert_eq!(m.diagnostics(), &[], "{p}");
        assert_eq!(
            m.table("t")
                .unwrap()
                .field(1)
                .unwrap()
                .rule
                .pattern
                .as_deref(),
            Some(p)
        );
    }
}

#[test]
fn without_rowid_hidden_pk_disables_create() {
    // Even with a default: a WITHOUT ROWID key cannot be read back.
    let sql = "CREATE TABLE t (code TEXT PRIMARY KEY DEFAULT 'd', n TEXT) WITHOUT ROWID";
    let m = model(
        sql,
        "version = 1\n[tables.t.fields.code]\nmode = \"hidden\"\n",
    );
    assert!(!m.table("t").unwrap().can_create);
    let w = find(&m, Warning, "t", Some("code"), "creating objects is off");
    assert_eq!(w.line, Some(2));
    assert_eq!(
        w.message,
        "table `t`: creating objects is off: primary key column `code` is hidden; make it editable or create-only to allow creating"
    );
    // Editable (the inherited primary-key mode is create-only): no warning.
    let m = model(sql, "version = 1\n");
    assert!(m.table("t").unwrap().can_create);
    assert_eq!(m.diagnostics(), &[]);
}

#[test]
fn rowid_hidden_pk_without_default_disables_create() {
    let sql = "CREATE TABLE t (code TEXT PRIMARY KEY, n TEXT)";
    let m = model(
        sql,
        "version = 1\n[tables.t.fields.code]\nmode = \"read-only\"\n",
    );
    assert!(!m.table("t").unwrap().can_create);
    let w = find(&m, Warning, "t", Some("code"), "is read-only");
    assert_eq!(w.line, Some(2));
    // No entry for the table: the warning has no position.
    let m = model(sql, "version = 1\n[defaults]\nfield_mode = \"read-only\"\n");
    assert!(!m.table("t").unwrap().can_create);
}

#[test]
fn hidden_pk_with_default_in_rowid_table_still_creates() {
    let sql = "CREATE TABLE t (code TEXT PRIMARY KEY DEFAULT 'd', n TEXT)";
    let m = model(
        sql,
        "version = 1\n[tables.t.fields.code]\nmode = \"hidden\"\n",
    );
    assert!(m.table("t").unwrap().can_create);
    assert!(!m
        .diagnostics()
        .iter()
        .any(|d| d.message.contains("creating objects")));
}
