use super::*;
use crate::rules::{FieldError, FieldIssueCode};
use crate::session::{FieldChange, FieldInput, SaveRequest, SaveResult};
use rusqlite::Connection;

pub(super) fn text(s: &str) -> FieldInput {
    FieldInput::Value {
        value: DbValue::Text(s.into()),
    }
}

pub(super) fn int(n: i64) -> FieldInput {
    FieldInput::Value {
        value: DbValue::Integer(n.to_string()),
    }
}

pub(super) fn dur(s: &str) -> FieldInput {
    FieldInput::Value {
        value: DbValue::Duration(s.into()),
    }
}

pub(super) fn reference(n: i64) -> FieldInput {
    FieldInput::Reference { key: Some(rk(n)) }
}

pub(super) fn request(
    s: &Session,
    table: &str,
    key: Option<ObjectKey>,
    changes: Vec<(&str, FieldInput)>,
) -> SaveRequest {
    let version = key
        .as_ref()
        .map(|k| s.object(table, k).unwrap().version.clone());
    SaveRequest {
        table: table.into(),
        key,
        version,
        config_hash: s.config().hash().to_owned(),
        links: Vec::new(),
        changes: changes
            .into_iter()
            .map(|(column, input)| FieldChange {
                column: column.into(),
                input,
            })
            .collect(),
    }
}

fn update(
    s: &mut Session,
    table: &str,
    id: i64,
    changes: Vec<(&str, FieldInput)>,
) -> Result<SaveResult, SessionError> {
    let r = request(s, table, Some(rk(id)), changes);
    s.save(&r)
}

fn create(
    s: &mut Session,
    table: &str,
    changes: Vec<(&str, FieldInput)>,
) -> Result<SaveResult, SessionError> {
    let r = request(s, table, None, changes);
    s.save(&r)
}

pub(super) fn fields_of(e: SessionError) -> Vec<FieldError> {
    match e {
        SessionError::Validation { fields, .. } => fields,
        other => panic!("expected Validation, got {other:?}"),
    }
}

pub(super) fn codes(fields: &[FieldError], column: &str) -> Vec<FieldIssueCode> {
    fields
        .iter()
        .find(|f| f.column == column)
        .unwrap_or_else(|| panic!("no issues on {column}: {fields:?}"))
        .issues
        .iter()
        .map(|i| i.code)
        .collect()
}

pub(super) fn raw(fx: &Fixture) -> Connection {
    Connection::open(&fx.db).unwrap()
}

pub(super) fn one<T: rusqlite::types::FromSql>(fx: &Fixture, sql: &str) -> T {
    raw(fx).query_row(sql, [], |r| r.get(0)).unwrap()
}

pub(super) fn baks(fx: &Fixture) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(fx.db.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| {
            std::path::Path::new(n)
                .extension()
                .is_some_and(|e| e == "bak")
        })
        .collect();
    v.sort();
    v
}

fn dir_names(fx: &Fixture) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(fx.db.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

// ---- updates and conflicts ----

#[test]
fn save_updates_only_changed_columns() {
    let fx = Fixture::example();
    // a legacy invalid value in an untouched column, and a trigger that
    // records any write to first_name or born
    fx.exec(
        "UPDATE author SET born = 'abc' WHERE id = 1;\
         CREATE TRIGGER rec AFTER UPDATE OF first_name, born ON author \
           BEGIN INSERT INTO audit_log (message) VALUES ('touched'); END;",
    );
    let mut s = fx.session();
    let r = update(&mut s, "author", 1, vec![("last_name", text("Lind"))]).unwrap();
    assert_eq!(
        field(&r.object, "last_name").value,
        DbValue::Text("Lind".into())
    );
    assert_eq!(
        one::<String>(&fx, "SELECT last_name FROM author WHERE id = 1"),
        "Lind"
    );
    assert_eq!(
        one::<String>(&fx, "SELECT typeof(born) || born FROM author WHERE id = 1"),
        "textabc"
    );
    assert_eq!(
        one::<i64>(
            &fx,
            "SELECT count(*) FROM audit_log WHERE message = 'touched'"
        ),
        0
    );
}

#[test]
fn save_returns_new_version() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let before = s.object("author", &rk(2)).unwrap().version;
    let r = update(
        &mut s,
        "author",
        2,
        vec![("last_name", text("Okafor-Smith"))],
    )
    .unwrap();
    assert_ne!(r.object.version, before);
    assert_eq!(
        r.object.version,
        s.object("author", &rk(2)).unwrap().version
    );
    assert_eq!(r.object.label, "Okafor-Smith, Tomas");
}

#[test]
fn empty_save_writes_nothing() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let r = update(&mut s, "author", 1, vec![]).unwrap();
    assert!(r.backup_created.is_none());
    assert!(s.backup_path().is_none());
    assert_eq!(baks(&fx).len(), 0);
    // a stale version is still a conflict
    let mut stale = request(&s, "author", Some(rk(1)), vec![]);
    stale.version = Some("stale".into());
    assert!(matches!(s.save(&stale), Err(SessionError::Conflict { .. })));
    assert_eq!(baks(&fx).len(), 0);
}

#[test]
fn conflict_on_stale_version() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let r = request(&s, "author", Some(rk(1)), vec![("last_name", text("Mine"))]);
    fx.exec("UPDATE author SET first_name = 'Changed' WHERE id = 1;");
    match s.save(&r) {
        Err(SessionError::Conflict { current, .. }) => {
            assert_eq!(
                field(&current, "first_name").value,
                DbValue::Text("Changed".into())
            );
        }
        other => panic!("expected Conflict, got {other:?}"),
    }
    assert_eq!(
        one::<String>(&fx, "SELECT last_name FROM author WHERE id = 1"),
        "Lindqvist"
    );
    // a missing version never matches
    let mut none = request(&s, "author", Some(rk(1)), vec![("last_name", text("Mine"))]);
    none.version = None;
    assert!(matches!(s.save(&none), Err(SessionError::Conflict { .. })));
    // an object that is gone
    let gone = request(&s, "author", Some(rk(1)), vec![("last_name", text("x"))]);
    fx.exec("DELETE FROM book WHERE author_id = 1; DELETE FROM contribution WHERE author_id = 1; DELETE FROM author WHERE id = 1;");
    assert!(matches!(s.save(&gone), Err(SessionError::UnknownObject)));
}

// ---- backup ----

#[test]
fn first_write_backs_up_once() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let r = update(&mut s, "author", 1, vec![("last_name", text("One"))]).unwrap();
    let shown = r.backup_created.expect("first save backs up");
    let names = baks(&fx);
    assert_eq!(names.len(), 1);
    let name = &names[0];
    assert!(
        name.starts_with("t.db.") && name.ends_with("Z.bak"),
        "{name}"
    );
    let stamp = &name["t.db.".len()..name.len() - ".bak".len()];
    assert_eq!(stamp.len(), 16, "{stamp}");
    assert_eq!(stamp.as_bytes()[8], b'T');
    assert_eq!(s.backup_path().unwrap(), fx.db.parent().unwrap().join(name));
    assert_eq!(shown, s.backup_path().unwrap().display().to_string());
    // the backup is the pre-save content
    let bak = Connection::open(s.backup_path().unwrap()).unwrap();
    let last: String = bak
        .query_row("SELECT last_name FROM author WHERE id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(last, "Lindqvist");
    assert_eq!(
        bak.query_row("SELECT count(*) FROM book", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        5
    );
    // the second save makes none
    let r2 = update(&mut s, "author", 1, vec![("last_name", text("Two"))]).unwrap();
    assert!(r2.backup_created.is_none());
    assert_eq!(baks(&fx).len(), 1);
    // no temp file is left behind
    assert!(dir_names(&fx).iter().all(|n| !n.contains("fm-backup")));
}

#[test]
fn adopted_backup_is_not_retaken() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let adopted = fx.db.with_file_name("adopted.bak");
    s.adopt_backup(adopted.clone(), true);
    let r = update(&mut s, "author", 1, vec![("last_name", text("X"))]).unwrap();
    assert!(r.backup_created.is_none());
    assert_eq!(s.backup_path(), Some(adopted.as_path()));
    assert_eq!(baks(&fx).len(), 0);
}

#[test]
fn backup_name_never_replaces() {
    let fx = Fixture::example();
    let mut s = fx.session();
    s.backup_stamp = Some("20261113T101500Z".into());
    let taken = fx.db.with_file_name("t.db.20261113T101500Z.bak");
    std::fs::write(&taken, "keep me").unwrap();
    update(&mut s, "author", 1, vec![("last_name", text("X"))]).unwrap();
    assert_eq!(std::fs::read_to_string(&taken).unwrap(), "keep me");
    assert_eq!(
        s.backup_path().unwrap(),
        fx.db.with_file_name("t.db.20261113T101500Z-1.bak")
    );
}

#[test]
fn backup_failure_blocks_write() {
    let fx = Fixture::example();
    let mut s = fx.session();
    s.backup_stamp = Some("20261113T101500Z".into());
    std::fs::create_dir(fx.db.with_file_name("t.db.20261113T101500Z.bak")).unwrap();
    for n in 1..crate::session::backup::MAX_ATTEMPTS + 100 {
        std::fs::create_dir(
            fx.db
                .with_file_name(format!("t.db.20261113T101500Z-{n}.bak")),
        )
        .unwrap();
    }
    let err = update(&mut s, "author", 1, vec![("last_name", text("X"))]).unwrap_err();
    assert!(matches!(err, SessionError::BackupFailed { .. }), "{err:?}");
    assert_eq!(
        one::<String>(&fx, "SELECT last_name FROM author WHERE id = 1"),
        "Lindqvist"
    );
    assert!(s.backup_path().is_none());
    assert!(dir_names(&fx).iter().all(|n| !n.contains("fm-backup")));
}

// ---- refusals ----

#[test]
fn rejects_read_only_database() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let r = request(&s, "author", Some(rk(1)), vec![("last_name", text("X"))]);
    s.database.force_read_only("test");
    assert!(matches!(s.save(&r), Err(SessionError::ReadOnlyDatabase)));
    assert_eq!(baks(&fx).len(), 0);
}

#[test]
fn rejects_config_changed() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let stale_hash = s.config().hash().to_owned();
    std::fs::write(&fx.cfg, format!("{EXAMPLE}\n# edited\n")).unwrap();
    // edited, not yet reloaded
    let r = request(&s, "author", Some(rk(1)), vec![("last_name", text("X"))]);
    assert!(matches!(s.save(&r), Err(SessionError::ConfigChangedOnDisk)));
    // reloaded: the old hash is refused, the current one succeeds
    s.set_config(LoadedConfig::load(&fx.cfg));
    let mut old = r.clone();
    old.config_hash = stale_hash;
    assert!(matches!(
        s.save(&old),
        Err(SessionError::ConfigChangedOnDisk)
    ));
    let fresh = request(&s, "author", Some(rk(1)), vec![("last_name", text("X"))]);
    assert!(s.save(&fresh).is_ok());
    assert_eq!(baks(&fx).len(), 1);
}

#[test]
fn rejects_schema_changed() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let r = request(&s, "author", Some(rk(1)), vec![("last_name", text("X"))]);
    fx.exec("ALTER TABLE member ADD COLUMN extra TEXT;");
    assert!(matches!(s.save(&r), Err(SessionError::SchemaChanged)));
    assert_eq!(
        one::<String>(&fx, "SELECT last_name FROM author WHERE id = 1"),
        "Lindqvist"
    );
}

#[test]
fn busy_when_locked() {
    let fx = Fixture::example();
    let mut s = fx.session();
    s.adopt_backup(fx.db.with_file_name("adopted.bak"), true);
    s.database
        .conn()
        .busy_timeout(std::time::Duration::from_millis(50))
        .unwrap();
    let r = request(&s, "author", Some(rk(1)), vec![("last_name", text("X"))]);
    let other = raw(&fx);
    other.execute_batch("BEGIN EXCLUSIVE;").unwrap();
    assert!(matches!(s.save(&r), Err(SessionError::Busy)));
    other.execute_batch("ROLLBACK;").unwrap();
    assert!(s.save(&r).is_ok());
}

// ---- field checks ----

#[test]
fn permission_checks() {
    let fx = Fixture::example();
    let mut s = fx.session();
    // pickable table: neither update nor create
    let e = update(&mut s, "publisher", 1, vec![("name", text("X"))]).unwrap_err();
    assert!(matches!(e, SessionError::NotEditable { .. }), "{e:?}");
    let e = create(&mut s, "publisher", vec![("name", text("X"))]).unwrap_err();
    assert!(matches!(e, SessionError::NotEditable { .. }), "{e:?}");
    // hidden table
    let e = create(&mut s, "audit_log", vec![]).unwrap_err();
    assert!(matches!(e, SessionError::UnknownTable(_)), "{e:?}");
    // create-only and read-only on update
    let f = fields_of(
        update(
            &mut s,
            "book",
            1,
            vec![("isbn", text("9780000000999")), ("revision", int(9))],
        )
        .unwrap_err(),
    );
    assert_eq!(codes(&f, "isbn"), [FieldIssueCode::NotEditable]);
    assert_eq!(codes(&f, "revision"), [FieldIssueCode::NotEditable]);
    // read-only is not creatable either; hidden is unknown
    let f = fields_of(update(&mut s, "author", 1, vec![("internal_note", text("x"))]).unwrap_err());
    assert_eq!(codes(&f, "internal_note"), [FieldIssueCode::UnknownField]);
    let f = fields_of(update(&mut s, "author", 1, vec![("nope", text("x"))]).unwrap_err());
    assert_eq!(codes(&f, "nope"), [FieldIssueCode::UnknownField]);
    // value on a foreign key, reference on a plain field
    let f = fields_of(update(&mut s, "book", 1, vec![("author_id", int(2))]).unwrap_err());
    assert_eq!(codes(&f, "author_id"), [FieldIssueCode::WrongType]);
    let f = fields_of(update(&mut s, "book", 1, vec![("title", reference(2))]).unwrap_err());
    assert_eq!(codes(&f, "title"), [FieldIssueCode::WrongType]);
    // a required reference cannot be NULL
    let f = fields_of(
        update(
            &mut s,
            "book",
            1,
            vec![("author_id", FieldInput::Reference { key: None })],
        )
        .unwrap_err(),
    );
    assert_eq!(codes(&f, "author_id"), [FieldIssueCode::Required]);
    // a duplicate column
    let f = fields_of(
        update(
            &mut s,
            "author",
            1,
            vec![("born", int(1970)), ("born", int(1971))],
        )
        .unwrap_err(),
    );
    assert_eq!(codes(&f, "born"), [FieldIssueCode::UnknownField]);
    assert_eq!(baks(&fx).len(), 0, "no check failure writes a backup");
    // create-only is allowed on create
    assert!(create(
        &mut s,
        "book",
        vec![
            ("title", text("T")),
            ("isbn", text("9780000000066")),
            ("author_id", reference(1))
        ]
    )
    .is_ok());
}

#[test]
fn validation_collects_all_issues() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let f = fields_of(
        update(
            &mut s,
            "author",
            1,
            vec![
                ("first_name", text("")),
                ("born", int(5)),
                ("nope", text("x")),
            ],
        )
        .unwrap_err(),
    );
    assert_eq!(codes(&f, "first_name"), [FieldIssueCode::TooShort]);
    assert_eq!(codes(&f, "born"), [FieldIssueCode::BelowMin]);
    assert_eq!(codes(&f, "nope"), [FieldIssueCode::UnknownField]);
    assert_eq!(f.len(), 3);
}

#[test]
fn legacy_values_do_not_block() {
    let fx = Fixture::example();
    fx.exec("UPDATE author SET born = 'abc', first_name = '' WHERE id = 1;");
    let mut s = fx.session();
    update(&mut s, "author", 1, vec![("last_name", text("Fine"))]).unwrap();
    assert_eq!(
        one::<String>(
            &fx,
            "SELECT typeof(born) || born || '|' || first_name FROM author WHERE id = 1"
        ),
        "textabc|"
    );
}

#[test]
fn blob_input_is_wrong_type_not_panic() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let blob = FieldInput::Value {
        value: DbValue::Blob { size: 3 },
    };
    let f = fields_of(update(&mut s, "book", 1, vec![("summary", blob)]).unwrap_err());
    assert_eq!(codes(&f, "summary"), [FieldIssueCode::WrongType]);
}

#[test]
fn unchanged_infinite_real_saves() {
    let fx = Fixture::example();
    fx.exec("UPDATE book SET price = 9e999 WHERE id = 1;");
    let mut s = fx.session();
    let o = s.object("book", &rk(1)).unwrap();
    assert_eq!(field(&o, "price").problem, Some(ValueProblem::WrongType));
    let r = update(&mut s, "book", 1, vec![("pages", int(321))]).unwrap();
    assert_eq!(
        field(&r.object, "pages").value,
        DbValue::Integer("321".into())
    );
    let price: f64 = one(&fx, "SELECT price FROM book WHERE id = 1");
    assert!(price.is_infinite() && price > 0.0);
    // editing it needs a finite value
    let bad = FieldInput::Value {
        value: DbValue::Real("inf".into()),
    };
    assert!(update(&mut s, "book", 1, vec![("price", bad)]).is_err());
}

// ---- durations ----

#[test]
fn duration_save_and_issues() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let r = update(&mut s, "book", 1, vec![("audio_length", dur("1h 30m"))]).unwrap();
    assert_eq!(
        one::<i64>(&fx, "SELECT audio_length FROM book WHERE id = 1"),
        5400
    );
    assert_eq!(
        field(&r.object, "audio_length").value,
        DbValue::Duration("1h30m".into())
    );
    let f = fields_of(update(&mut s, "book", 1, vec![("audio_length", dur("90ms"))]).unwrap_err());
    assert_eq!(
        codes(&f, "audio_length"),
        [FieldIssueCode::DurationPrecision]
    );
    let f = fields_of(update(&mut s, "book", 1, vec![("audio_length", dur("30s"))]).unwrap_err());
    assert_eq!(codes(&f, "audio_length"), [FieldIssueCode::BelowMin]);
    let limit = f[0].issues[0].limit.as_deref();
    assert_eq!(limit, Some("1m"));
}

// ---- creates and constraints ----

#[test]
fn create_with_reference() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let r = create(
        &mut s,
        "book",
        vec![
            ("title", text("New Book")),
            ("isbn", text("9780000000066")),
            ("author_id", reference(2)),
        ],
    )
    .unwrap();
    assert_eq!(r.object.key, rk(6));
    assert_eq!(r.object.label, "New Book");
    let link = field(&r.object, "author_id").link.as_ref().unwrap();
    assert_eq!(link.key, Some(rk(2)));
    assert_eq!(link.label.as_deref(), Some("Okafor, Tomas"));
    assert_eq!(
        field(&r.object, "format").value,
        DbValue::Text("paperback".into())
    );
    assert!(r.backup_created.is_some());
}

#[test]
fn create_in_integer_primary_key_desc_table() {
    let fx = Fixture::new(
        "CREATE TABLE d (id INTEGER PRIMARY KEY DESC, name TEXT);\
         INSERT INTO d (id, name) VALUES (7, 'seven');",
        None,
    );
    let mut s = fx.session();
    let r = create(&mut s, "d", vec![("name", text("new"))]).unwrap();
    let rowid: i64 = one(&fx, "SELECT rowid FROM d WHERE name = 'new'");
    assert_eq!(
        r.object.key,
        ObjectKey::Rowid {
            value: rowid.to_string()
        }
    );
    assert_eq!(
        one::<Option<i64>>(&fx, "SELECT id FROM d WHERE name = 'new'"),
        None
    );
    assert_eq!(field(&r.object, "name").value, DbValue::Text("new".into()));
}

#[test]
fn create_inherits_allow_create() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let r = create(
        &mut s,
        "genre",
        vec![("code", text("drama")), ("name", text("Drama"))],
    )
    .unwrap();
    assert_eq!(r.object.key, pk("drama"));
    assert_eq!(r.object.label, "Drama");
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM genre"), 5);
}

#[test]
fn create_requires_required_fields() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let f = fields_of(create(&mut s, "author", vec![("born", int(1990))]).unwrap_err());
    assert_eq!(codes(&f, "first_name"), [FieldIssueCode::Required]);
    assert_eq!(codes(&f, "last_name"), [FieldIssueCode::Required]);
    let f = fields_of(
        create(
            &mut s,
            "book",
            vec![("title", text("T")), ("author_id", reference(1))],
        )
        .unwrap_err(),
    );
    assert_eq!(codes(&f, "isbn"), [FieldIssueCode::Required]);
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM book"), 5);
    assert_eq!(baks(&fx).len(), 0);
}

#[test]
fn reference_missing() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let f = fields_of(
        create(
            &mut s,
            "book",
            vec![
                ("title", text("T")),
                ("isbn", text("9780000000066")),
                ("author_id", reference(999)),
            ],
        )
        .unwrap_err(),
    );
    assert_eq!(codes(&f, "author_id"), [FieldIssueCode::ReferenceMissing]);
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM book"), 5);
}

#[test]
fn unique_maps_to_field() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let f = fields_of(
        create(
            &mut s,
            "book",
            vec![
                ("title", text("T")),
                ("isbn", text("9780000000011")),
                ("author_id", reference(1)),
            ],
        )
        .unwrap_err(),
    );
    assert_eq!(codes(&f, "isbn"), [FieldIssueCode::Unique]);
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM book"), 5);
}

#[test]
fn check_maps_to_constraint_and_rolls_back() {
    let fx = Fixture::new(
        "CREATE TABLE c (id INTEGER PRIMARY KEY, a INTEGER, b INTEGER CHECK (b > 0));\
         INSERT INTO c (id, a, b) VALUES (1, 1, 1);",
        None,
    );
    let mut s = fx.session();
    let e = create(&mut s, "c", vec![("a", int(2)), ("b", int(-1))]).unwrap_err();
    assert!(
        matches!(&e, SessionError::Constraint { message: m } if m.contains("CHECK")),
        "{e:?}"
    );
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM c"), 1);
    let e = update(&mut s, "c", 1, vec![("a", int(5)), ("b", int(-1))]).unwrap_err();
    assert!(matches!(e, SessionError::Constraint { .. }), "{e:?}");
    assert_eq!(one::<i64>(&fx, "SELECT a FROM c WHERE id = 1"), 1);
    // the session is usable afterwards
    assert!(update(&mut s, "c", 1, vec![("b", int(3))]).is_ok());
}

#[test]
fn awkward_identifiers_save() {
    let fx = Fixture::new(
        "CREATE TABLE \"select\" (id INTEGER PRIMARY KEY, \"a\"\"b\" TEXT, \"with space\" TEXT, \"\u{fc}n\u{ef}\" TEXT);",
        None,
    );
    let mut s = fx.session();
    let r = create(
        &mut s,
        "select",
        vec![
            ("a\"b", text("one")),
            ("with space", text("two")),
            ("\u{fc}n\u{ef}", text("three")),
        ],
    )
    .unwrap();
    assert_eq!(field(&r.object, "a\"b").value, DbValue::Text("one".into()));
    assert_eq!(
        field(&r.object, "\u{fc}n\u{ef}").value,
        DbValue::Text("three".into())
    );
    let key = r.object.key.clone();
    let req = request(
        &s,
        "select",
        Some(key.clone()),
        vec![("with space", text("changed"))],
    );
    let r = s.save(&req).unwrap();
    assert_eq!(
        field(&r.object, "with space").value,
        DbValue::Text("changed".into())
    );
    assert_eq!(field(&r.object, "a\"b").value, DbValue::Text("one".into()));
}

#[test]
fn busy_when_locked_on_first_save() {
    let fx = Fixture::example();
    let mut s = fx.session();
    s.database
        .conn()
        .busy_timeout(std::time::Duration::from_millis(50))
        .unwrap();
    let r = request(&s, "author", Some(rk(1)), vec![("last_name", text("X"))]);
    let other = raw(&fx);
    other.execute_batch("BEGIN EXCLUSIVE;").unwrap();
    let start = std::time::Instant::now();
    assert!(matches!(s.save(&r), Err(SessionError::Busy)));
    assert!(start.elapsed() < std::time::Duration::from_secs(10));
    assert_eq!(baks(&fx).len(), 0);
    assert!(s.backup_path().is_none());
    assert!(dir_names(&fx).iter().all(|n| !n.contains("fm-backup")));
    other.execute_batch("ROLLBACK;").unwrap();
    assert!(s.save(&r).unwrap().backup_created.is_some());
}

#[test]
fn backup_announced_on_first_successful_save() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let dup = create(
        &mut s,
        "book",
        vec![
            ("title", text("T")),
            ("isbn", text("9780000000011")),
            ("author_id", reference(1)),
        ],
    );
    assert!(matches!(dup, Err(SessionError::Validation { .. })));
    assert!(s.backup_path().is_some(), "the backup was taken");
    let path = s.backup_path().unwrap().display().to_string();
    let second = update(&mut s, "author", 1, vec![("last_name", text("Two"))]).unwrap();
    assert_eq!(second.backup_created, Some(path));
    let third = update(&mut s, "author", 1, vec![("last_name", text("Three"))]).unwrap();
    assert!(third.backup_created.is_none());
    assert_eq!(baks(&fx).len(), 1);
}

#[test]
fn create_declared_pk_with_default_reads_key_by_rowid() {
    let fx = Fixture::new(
        "CREATE TABLE p (code TEXT PRIMARY KEY DEFAULT 'dflt', name TEXT);",
        None,
    );
    let mut s = fx.session();
    let r = create(&mut s, "p", vec![("name", text("n"))]).unwrap();
    assert_eq!(r.object.key, rk(1));
    assert_eq!(field(&r.object, "code").value, DbValue::Text("dflt".into()));
}

#[test]
fn create_declared_pk_without_default_requires_it() {
    let fx = Fixture::new("CREATE TABLE q (code TEXT PRIMARY KEY, name TEXT);", None);
    let mut s = fx.session();
    let f = fields_of(create(&mut s, "q", vec![("name", text("n"))]).unwrap_err());
    assert_eq!(codes(&f, "code"), [FieldIssueCode::Required]);
    let null = FieldInput::Value {
        value: DbValue::Null,
    };
    let f = fields_of(create(&mut s, "q", vec![("code", null), ("name", text("n"))]).unwrap_err());
    assert_eq!(codes(&f, "code"), [FieldIssueCode::Required]);
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM q"), 0);
    assert!(create(&mut s, "q", vec![("code", text("c"))]).is_ok());
}

#[test]
fn create_without_rowid_requires_pk() {
    let fx = Fixture::new(
        "CREATE TABLE w (code TEXT PRIMARY KEY DEFAULT 'd', name TEXT) WITHOUT ROWID;",
        None,
    );
    let mut s = fx.session();
    let f = fields_of(create(&mut s, "w", vec![("name", text("n"))]).unwrap_err());
    assert_eq!(codes(&f, "code"), [FieldIssueCode::Required]);
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM w"), 0);
}

#[test]
fn notnull_violation_is_required() {
    let fx = Fixture::new(
        "CREATE TABLE n (id INTEGER PRIMARY KEY, a TEXT NOT NULL DEFAULT 'x', b TEXT);\
         INSERT INTO n (id) VALUES (1);\
         CREATE TRIGGER nn AFTER UPDATE OF b ON n \
           BEGIN UPDATE n SET a = NULL WHERE id = new.id; END;",
        None,
    );
    let mut s = fx.session();
    let f =
        fields_of(update(&mut s, "n", 1, vec![("a", text("y")), ("b", text("z"))]).unwrap_err());
    assert_eq!(codes(&f, "a"), [FieldIssueCode::Required]);
    assert_eq!(one::<String>(&fx, "SELECT a FROM n WHERE id = 1"), "x");
}

#[test]
fn primary_key_violation_is_unique() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let f = fields_of(
        create(
            &mut s,
            "genre",
            vec![("code", text("fiction")), ("name", text("Again"))],
        )
        .unwrap_err(),
    );
    assert_eq!(codes(&f, "code"), [FieldIssueCode::Unique]);
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM genre"), 4);
}

#[test]
fn map_constraint_unit() {
    use crate::session::save::map_constraint;
    let failure = |extended: i32, msg: &str| {
        rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(extended), Some(msg.to_owned()))
    };
    let unique = rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE;
    let one = map_constraint(
        failure(unique, "UNIQUE constraint failed: t.a"),
        "t",
        &["a", "b"],
    );
    assert_eq!(codes(&fields_of(one), "a"), [FieldIssueCode::Unique]);
    let two = map_constraint(
        failure(unique, "UNIQUE constraint failed: t.a, t.b"),
        "t",
        &["a", "b"],
    );
    assert!(matches!(two, SessionError::Constraint { .. }));
    let unchanged = map_constraint(
        failure(unique, "UNIQUE constraint failed: t.c"),
        "t",
        &["a"],
    );
    assert!(matches!(unchanged, SessionError::Constraint { .. }));
    let check = map_constraint(
        failure(
            rusqlite::ffi::SQLITE_CONSTRAINT_CHECK,
            "CHECK constraint failed: x",
        ),
        "t",
        &["a"],
    );
    assert!(matches!(check, SessionError::Constraint { message } if message.contains("CHECK")));
    let busy = map_constraint(failure(rusqlite::ffi::SQLITE_BUSY, "locked"), "t", &["a"]);
    assert!(matches!(busy, SessionError::Busy));
}

pub(super) fn config_with(fx: &Fixture, extra: &str) {
    std::fs::write(&fx.cfg, format!("version = 1\n{extra}")).unwrap();
}

#[test]
fn hidden_pk_refuses_create_up_front() {
    let fx = Fixture::new(
        "CREATE TABLE t (code TEXT PRIMARY KEY DEFAULT 'd', n TEXT) WITHOUT ROWID;",
        None,
    );
    config_with(&fx, "[tables.t.fields.code]\nmode = \"hidden\"\n");
    let mut s = fx.session();
    let e = create(&mut s, "t", vec![("n", text("x"))]).unwrap_err();
    assert!(matches!(e, SessionError::NotEditable { .. }), "{e:?}");
    assert_eq!(one::<i64>(&fx, "SELECT count(*) FROM t"), 0);
    assert_eq!(baks(&fx).len(), 0);
}

#[test]
fn hidden_pk_with_default_creates_in_rowid_table() {
    let fx = Fixture::new(
        "CREATE TABLE t (code TEXT PRIMARY KEY DEFAULT 'd', n TEXT);",
        None,
    );
    config_with(&fx, "[tables.t.fields.code]\nmode = \"hidden\"\n");
    let mut s = fx.session();
    let r = create(&mut s, "t", vec![("n", text("x"))]).unwrap();
    assert_eq!(r.object.key, rk(1));
    assert_eq!(one::<String>(&fx, "SELECT code FROM t"), "d");
}

fn interleave(fx: &Fixture, s: &mut Session, times: Option<u32>) {
    let db = fx.db.clone();
    let mut n = 0_u32;
    s.after_backup = Some(Box::new(move || {
        if times.is_none_or(|t| n < t) {
            n += 1;
            Connection::open(&db)
                .unwrap()
                .execute("UPDATE author SET born = ?1 WHERE id = 3", [2000 + n])
                .unwrap();
        }
    }));
}

#[test]
fn commit_between_backup_and_begin_retakes_backup() {
    let fx = Fixture::example();
    let mut s = fx.session();
    interleave(&fx, &mut s, Some(1));
    let r = update(&mut s, "author", 1, vec![("last_name", text("X"))]).unwrap();
    assert!(r.backup_created.is_some());
    let names = baks(&fx);
    assert_eq!(names.len(), 1, "{names:?}");
    assert!(dir_names(&fx).iter().all(|n| !n.contains("fm-backup")));
    let bak = Connection::open(s.backup_path().unwrap()).unwrap();
    let born: i64 = bak
        .query_row("SELECT born FROM author WHERE id = 3", [], |r| r.get(0))
        .unwrap();
    assert_eq!(born, 2001, "the backup holds the interleaved commit");
    assert_eq!(
        one::<String>(&fx, "SELECT last_name FROM author WHERE id = 1"),
        "X"
    );
}

#[test]
fn repeated_interleaved_commits_give_busy() {
    let fx = Fixture::example();
    let mut s = fx.session();
    interleave(&fx, &mut s, None);
    let e = update(&mut s, "author", 1, vec![("last_name", text("X"))]).unwrap_err();
    assert!(matches!(e, SessionError::Busy), "{e:?}");
    assert_eq!(baks(&fx).len(), 0);
    assert!(s.backup_path().is_none());
    assert!(dir_names(&fx).iter().all(|n| !n.contains("fm-backup")));
    assert_eq!(
        one::<String>(&fx, "SELECT last_name FROM author WHERE id = 1"),
        "Lindqvist"
    );
}

#[test]
fn stray_backup_after_conflict_is_announced_later() {
    let fx = Fixture::example();
    let mut s = fx.session();
    let stale = request(&s, "author", Some(rk(1)), vec![("last_name", text("Mine"))]);
    fx.exec("UPDATE author SET first_name = 'Changed' WHERE id = 1;");
    assert!(matches!(s.save(&stale), Err(SessionError::Conflict { .. })));
    let path = s.backup_path().expect("kept").display().to_string();
    assert_eq!(baks(&fx).len(), 1);
    let second = update(&mut s, "author", 1, vec![("last_name", text("Two"))]).unwrap();
    assert_eq!(second.backup_created, Some(path));
    assert_eq!(baks(&fx).len(), 1);
}
