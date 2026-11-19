use super::*;
use crate::config::DiagnosticSeverity;

#[test]
fn no_config_is_editable() {
    let fx = Fixture::new(SCHEMA, None);
    let sess = fx.session();
    assert_eq!(sess.config().status(), ConfigStatus::Missing);
    assert!(sess.config_fail_closed().is_none());
    let views = sess.table_views();
    let names: Vec<&str> = views.iter().map(|t| t.name.as_str()).collect();
    assert!(!names.contains(&"book_genre"));
    for n in [
        "publisher",
        "author",
        "book",
        "genre",
        "contribution",
        "member",
        "loan",
        "audit_log",
    ] {
        assert!(names.contains(&n), "{n}");
    }
    assert!(views.iter().all(|t| t.mode == TableMode::Editable));
    assert!(sess.list("audit_log", &lq(None, 0, 10)).is_ok());
    // hidden columns are not hidden without config
    assert!(sess
        .object("author", &rk(1))
        .unwrap()
        .fields
        .iter()
        .any(|f| f.column == "internal_note"));
}

#[test]
fn loaded_config_statuses() {
    let fx = Fixture::new("CREATE TABLE t (id INTEGER PRIMARY KEY);", None);
    let missing = LoadedConfig::load(&fx.cfg);
    assert_eq!(missing.status(), ConfigStatus::Missing);
    assert_eq!(missing.hash(), "missing");
    assert_eq!(missing.diagnostics(), []);
    assert!(missing.fail_closed_reason().is_none());
    assert!(!missing.changed_on_disk());

    std::fs::write(&fx.cfg, EXAMPLE).unwrap();
    let ok = LoadedConfig::load(&fx.cfg);
    assert_eq!(ok.status(), ConfigStatus::Loaded);
    assert!(ok.fail_closed_reason().is_none());
    assert_eq!(ok.path(), fx.cfg);
    assert_eq!(ok.hash().len(), 64);
    assert!(ok.hash().bytes().all(|b| b.is_ascii_hexdigit()));
    assert!(!ok.changed_on_disk());
    assert!(missing.changed_on_disk(), "appearing counts");

    // a BOM is hashed but not parsed
    std::fs::write(&fx.cfg, format!("\u{feff}{EXAMPLE}")).unwrap();
    let bom = LoadedConfig::load(&fx.cfg);
    assert_eq!(bom.status(), ConfigStatus::Loaded);
    assert_ne!(bom.hash(), ok.hash());

    // entry-level errors are diagnostics, not fail-closed
    std::fs::write(&fx.cfg, "version = 1\n[tables.t]\nmode = \"bogus\"\n").unwrap();
    let entry = LoadedConfig::load(&fx.cfg);
    assert_eq!(entry.status(), ConfigStatus::Loaded);
    assert!(entry
        .diagnostics()
        .iter()
        .any(|d| d.severity == DiagnosticSeverity::Error));

    std::fs::write(&fx.cfg, "version = 2\n").unwrap();
    let v2 = LoadedConfig::load(&fx.cfg);
    assert_eq!(v2.status(), ConfigStatus::FailClosed);
    assert!(v2.fail_closed_reason().unwrap().contains("version 2"));

    std::fs::remove_file(&fx.cfg).unwrap();
    assert!(v2.changed_on_disk(), "disappearing counts");
    assert_eq!(status(&fx), ConfigStatus::Missing);
}

#[test]
fn config_fail_closed_reason() {
    let fx = Fixture::new(SCHEMA, Some("version = 1\n[tables\n"));
    let sess = fx.session();
    assert_eq!(sess.config().status(), ConfigStatus::FailClosed);
    assert!(sess.config_fail_closed().unwrap().contains("syntax error"));
    assert_ne!(sess.diagnostics(), []);
    let views = sess.table_views();
    assert!(views.iter().all(|t| t.mode == TableMode::ReadOnly));
    assert!(views
        .iter()
        .flat_map(|t| &t.fields)
        .all(|f| f.mode == crate::config::FieldMode::ReadOnly));
    assert!(sess.list("author", &lq(None, 0, 10)).is_ok());

    let v2 = Fixture::new(SCHEMA, Some("version = 2\n"));
    assert!(v2
        .session()
        .config_fail_closed()
        .unwrap()
        .contains("version 2"));
    let ok = Fixture::example();
    assert!(ok.session().config_fail_closed().is_none());
}

#[test]
fn unreadable_config_is_fail_closed() {
    // a directory at the path
    let fx = Fixture::new(SCHEMA, None);
    std::fs::create_dir(&fx.cfg).unwrap();
    let c = LoadedConfig::load(&fx.cfg);
    assert_eq!(c.status(), ConfigStatus::FailClosed);
    assert!(c
        .fail_closed_reason()
        .unwrap()
        .starts_with("the config file could not be read"));
    assert!(!c.changed_on_disk());
    let sess = fx.session();
    assert!(sess.config_fail_closed().is_some());
    assert!(sess
        .table_views()
        .iter()
        .all(|t| t.mode == TableMode::ReadOnly));
    std::fs::remove_dir(&fx.cfg).unwrap();
    assert!(c.changed_on_disk());

    // not UTF-8
    std::fs::write(&fx.cfg, [0x76, 0xff, 0xfe]).unwrap();
    let c = LoadedConfig::load(&fx.cfg);
    assert_eq!(c.status(), ConfigStatus::FailClosed);
    assert!(c.fail_closed_reason().unwrap().contains("UTF-8"));
    assert_eq!(c.hash().len(), 64);
    assert!(!c.changed_on_disk());
    std::fs::write(&fx.cfg, "version = 1\n").unwrap();
    assert!(c.changed_on_disk());
}

#[test]
fn config_change_detected() {
    let fx = Fixture::example();
    let sess = fx.session();
    assert!(!sess.config_changed());
    std::fs::write(&fx.cfg, format!("{EXAMPLE}\n# edited\n")).unwrap();
    assert!(sess.config_changed());
    std::fs::write(&fx.cfg, EXAMPLE).unwrap();
    assert!(
        !sess.config_changed(),
        "restoring the bytes restores the hash"
    );
    std::fs::remove_file(&fx.cfg).unwrap();
    assert!(sess.config_changed());
}

#[test]
fn set_config_reresolves() {
    let fx = Fixture::new(SCHEMA, None);
    let mut sess = fx.session();
    assert_eq!(mode_of(&sess, "author"), TableMode::Editable);
    let before = sess.object("author", &rk(1)).unwrap();
    assert!(before.fields.iter().any(|f| f.column == "internal_note"));

    std::fs::write(
        &fx.cfg,
        "version = 1\n[tables.author]\nmode = \"read-only\"\n\
         [tables.author.fields.born]\nmin = 1000\n\
         [tables.author.fields.internal_note]\nmode = \"hidden\"\n",
    )
    .unwrap();
    sess.set_config(LoadedConfig::load(&fx.cfg));
    assert_eq!(sess.config().status(), ConfigStatus::Loaded);
    assert_eq!(mode_of(&sess, "author"), TableMode::ReadOnly);
    let view = sess
        .table_views()
        .into_iter()
        .find(|t| t.name == "author")
        .unwrap();
    let born = view.fields.iter().find(|f| f.column == "born").unwrap();
    assert_eq!(born.rule.min.as_deref(), Some("1000"));
    // reads keep working on the same connection, with the new model
    let after = sess.object("author", &rk(1)).unwrap();
    assert!(after.fields.iter().all(|f| f.column != "internal_note"));
    assert!(!sess.config_changed());

    // and a fail-closed replacement closes everything
    std::fs::write(&fx.cfg, "version = 3\n").unwrap();
    sess.set_config(LoadedConfig::load(&fx.cfg));
    assert!(sess.config_fail_closed().is_some());
    assert!(sess
        .table_views()
        .iter()
        .all(|t| t.mode == TableMode::ReadOnly));
    assert!(sess.list("book", &lq(None, 0, 5)).is_ok());
}
