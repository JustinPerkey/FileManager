//! Tool core tests. Every test builds its databases, config and state in its
//! own temp dir.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use fm_core::AppDirs;

use super::*;
use crate::config::{ConfigStatus, LoadedConfig, TableMode};
use crate::db::OpenError;
use crate::session::{
    DeleteImpact, FieldChange, FieldInput, ListQuery, M2mQuery, ObjectKey, SaveRequest,
    SessionError,
};
use crate::value::DbValue;

const SCHEMA: &str = include_str!("../../../../examples/dbmanager/schema.sql");
const EXAMPLE: &str = include_str!("../../../../examples/dbmanager/example.toml");

struct Fx {
    dir: tempfile::TempDir,
    db: PathBuf,
    cfg: PathBuf,
}

impl Fx {
    fn new(config: Option<&str>) -> Fx {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("t.db");
        make_db(&db);
        let cfg = dir.path().join("dbmanager.toml");
        if let Some(c) = config {
            fs::write(&cfg, c).unwrap();
        }
        Fx { dir, db, cfg }
    }

    fn example() -> Fx {
        Fx::new(Some(EXAMPLE))
    }

    fn dirs(&self) -> AppDirs {
        AppDirs::at(self.dir.path().join("app"))
    }

    fn core(&self) -> Core {
        Core::new(&self.dirs(), Some(self.cfg.clone()))
    }

    fn state_file(&self) -> PathBuf {
        self.dir
            .path()
            .join("app")
            .join("dbmanager")
            .join("state.json")
    }
}

fn make_db(path: &Path) {
    rusqlite::Connection::open(path)
        .unwrap()
        .execute_batch(SCHEMA)
        .unwrap();
}

fn mtime(path: &Path) -> SystemTime {
    fs::metadata(path).unwrap().modified().unwrap()
}

fn lq() -> ListQuery {
    ListQuery {
        search: None,
        offset: 0,
        limit: 25,
        referencing: None,
    }
}

#[test]
fn open_builds_snapshot() {
    let fx = Fx::example();
    let mut core = fx.core();
    assert!(!core.session().open);
    core.open(&fx.db).unwrap();
    let s = core.session();
    assert!(s.open);
    assert_eq!(s.database.as_ref().unwrap().path, fx.db);
    assert_eq!(s.config.status, ConfigStatus::Loaded);
    let publisher = s.tables.iter().find(|t| t.name == "publisher").unwrap();
    assert_eq!(publisher.mode, TableMode::Pickable);
    assert!(s.tables.iter().any(|t| t.name == "author"));
    assert_eq!(
        s.error_count + s.warning_count,
        s.diagnostics.len(),
        "counts cover the diagnostics"
    );
    assert_eq!(s.recent, vec![fx.db.to_string_lossy().into_owned()]);
    assert!(s.backup_path.is_none());
    assert!(s.config_fail_closed.is_none());
}

#[test]
fn session_is_cheap_and_side_effect_free() {
    let fx = Fx::example();
    let mut core = fx.core();
    core.open(&fx.db).unwrap();
    let before_addr = Core::session_opens();
    let files = |dir: &Path| {
        let mut v: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        v.sort();
        v
    };
    let listing = files(fx.dir.path());
    let db_bytes = fs::read(&fx.db).unwrap();
    let db_time = mtime(&fx.db);
    let state_bytes = fs::read(fx.state_file()).unwrap();
    let state_time = mtime(&fx.state_file());
    let first = core.session();
    for _ in 0..3 {
        assert_eq!(core.session(), first);
    }
    assert_eq!(
        Core::session_opens(),
        before_addr,
        "same Session instance, none opened"
    );
    assert_eq!(fs::read(&fx.db).unwrap(), db_bytes);
    assert_eq!(mtime(&fx.db), db_time);
    assert_eq!(fs::read(fx.state_file()).unwrap(), state_bytes);
    assert_eq!(mtime(&fx.state_file()), state_time);
    assert_eq!(files(fx.dir.path()), listing, "no backup or other file");
    assert!(first.backup_path.is_none());
}

#[test]
fn session_recomputes_config_changed() {
    let fx = Fx::example();
    let mut core = fx.core();
    core.open(&fx.db).unwrap();
    assert!(!core.session().config_changed);
    fs::write(&fx.cfg, format!("{EXAMPLE}\n# edited\n")).unwrap();
    let s = core.session();
    assert!(s.config_changed);
    assert!(s.open, "the config is not reloaded by a snapshot");
    fs::write(&fx.cfg, EXAMPLE).unwrap();
    assert!(!core.session().config_changed);
}

#[test]
fn config_missing_is_editable_defaults() {
    let fx = Fx::new(None);
    let mut core = fx.core();
    core.open(&fx.db).unwrap();
    let s = core.session();
    assert_eq!(s.config.status, ConfigStatus::Missing);
    assert_eq!(s.config.hash, "missing");
    assert!(s.config_fail_closed.is_none());
    let author = s.tables.iter().find(|t| t.name == "author").unwrap();
    assert_eq!(author.mode, TableMode::Editable);
    assert!(author.can_create);
    assert!(!s.config_changed);
}

#[test]
fn config_loaded_status_and_hash() {
    let fx = Fx::example();
    let mut core = fx.core();
    let s = core.session();
    assert_eq!(s.config.status, ConfigStatus::Loaded);
    assert_eq!(s.config.hash, LoadedConfig::load(&fx.cfg).hash());
    assert_eq!(s.config.hash.len(), 64);
    assert_eq!(s.config.path.as_deref(), Some(fx.cfg.as_path()));
    assert_eq!(
        s.config.display.as_deref(),
        Some(fx.cfg.to_string_lossy().as_ref())
    );
}

#[test]
fn config_fail_closed_in_snapshot() {
    for case in ["syntax", "version", "directory"] {
        let fx = Fx::new(None);
        match case {
            "syntax" => fs::write(&fx.cfg, "version = = 1\n[").unwrap(),
            "version" => fs::write(&fx.cfg, "version = 2\n").unwrap(),
            _ => fs::create_dir(&fx.cfg).unwrap(),
        }
        let mut core = fx.core();
        core.open(&fx.db).unwrap();
        let s = core.session();
        assert_eq!(s.config.status, ConfigStatus::FailClosed, "{case}");
        assert!(s.config_fail_closed.is_some(), "{case}");
        assert!(s.error_count >= 1, "{case}");
        assert!(
            s.tables.iter().all(|t| !t.can_create && !t.can_delete),
            "{case}: everything is read-only"
        );
    }
}

#[test]
fn config_location_unknown_fails_closed() {
    let fx = Fx::new(None);
    let mut core = Core::new(&fx.dirs(), None);
    core.open(&fx.db).unwrap();
    let s = core.session();
    assert_eq!(s.config.status, ConfigStatus::FailClosed);
    assert!(s.config.path.is_none() && s.config.display.is_none());
    assert!(s
        .config_fail_closed
        .as_deref()
        .unwrap()
        .contains("could not be determined"));
    assert!(!s.config_changed);
    assert!(s.tables.iter().all(|t| !t.can_create && !t.can_delete));
}

#[test]
fn config_shown_without_database() {
    let fx = Fx::new(Some("version = 2\n"));
    let mut core = fx.core();
    let s = core.session();
    assert!(!s.open && s.database.is_none() && s.tables.is_empty());
    assert_eq!(s.config.status, ConfigStatus::FailClosed);
    assert_ne!(s.diagnostics.len(), 0);
    assert!(s.error_count >= 1);
    assert!(s.config_fail_closed.is_some());
}

#[test]
fn recent_databases_persisted_and_restored() {
    let fx = Fx::example();
    let db2 = fx.dir.path().join("two.db");
    make_db(&db2);
    let mut core = fx.core();
    core.open(&fx.db).unwrap();
    core.open(&db2).unwrap();
    assert_eq!(core.recent_paths(), vec![db2.clone(), fx.db.clone()]);

    let mut again = fx.core();
    let s = again.session();
    assert!(s.open);
    assert_eq!(s.database.unwrap().path, db2);
    assert_eq!(s.recent.len(), 2);
    assert!(s.state_warning.is_none());
}

#[test]
fn failed_restore_warns_and_keeps_entry() {
    let fx = Fx::example();
    {
        let mut core = fx.core();
        core.open(&fx.db).unwrap();
    }
    fs::remove_file(&fx.db).unwrap();

    let mut again = fx.core();
    let s = again.session();
    assert!(!s.open);
    let warning = s.state_warning.unwrap();
    assert!(warning.contains(&*fx.db.to_string_lossy()), "{warning}");
    assert_eq!(s.recent.len(), 1);
    let slot = s.database.unwrap();
    assert_eq!(slot.path, fx.db);
    assert_eq!(slot.error.unwrap().kind, DbmanagerErrorKind::NotAFile);
    assert_eq!(again.recent_paths(), vec![fx.db.clone()]);
}

#[test]
fn open_recent_uses_stored_path() {
    let fx = Fx::example();
    let db2 = fx.dir.path().join("two.db");
    make_db(&db2);
    let mut core = fx.core();
    core.open(&fx.db).unwrap();
    core.open(&db2).unwrap();
    let path = core.recent_paths()[1].clone();
    core.open(&path).unwrap();
    assert_eq!(core.session().database.unwrap().path, fx.db);
    assert_eq!(core.recent_paths(), vec![fx.db.clone(), db2]);
}

#[test]
fn open_dropped_single() {
    let fx = Fx::example();
    let mut core = fx.core();
    core.open_dropped(std::slice::from_ref(&fx.db)).unwrap();
    assert!(core.session().open);
}

#[test]
fn open_dropped_ambiguous() {
    let fx = Fx::example();
    let mut core = fx.core();
    core.open(&fx.db).unwrap();
    let before = core.session();
    let other = fx.dir.path().join("other.db");
    make_db(&other);
    for paths in [vec![fx.db.clone(), other], Vec::new()] {
        let e = core.open_dropped(&paths).unwrap_err();
        assert_eq!(e.kind, DbmanagerErrorKind::DropAmbiguous);
    }
    assert_eq!(core.session(), before, "nothing changed");
}

#[test]
fn open_dropped_toml_is_config_not_droppable() {
    let fx = Fx::example();
    let mut core = fx.core();
    core.open(&fx.db).unwrap();
    let before = core.session();
    let upper = fx.dir.path().join("Other.TOML");
    let e = core
        .open_dropped(std::slice::from_ref(&fx.cfg))
        .unwrap_err();
    assert_eq!(e.kind, DbmanagerErrorKind::ConfigNotDroppable);
    let e = core.open_dropped(&[upper]).unwrap_err();
    assert_eq!(
        e.kind,
        DbmanagerErrorKind::ConfigNotDroppable,
        "before any open"
    );
    assert_eq!(core.session(), before, "the database stays open");
}

#[test]
fn open_rejects_relative_and_directory() {
    let fx = Fx::example();
    let mut core = fx.core();
    let e = core.open(Path::new("t.db")).unwrap_err();
    assert_eq!(e.kind, DbmanagerErrorKind::NotAbsolute);
    let e = core.open(fx.dir.path()).unwrap_err();
    assert_eq!(e.kind, DbmanagerErrorKind::NotAFile);
    let e = core.open(&fx.dir.path().join("missing.db")).unwrap_err();
    assert_eq!(e.kind, DbmanagerErrorKind::NotAFile);
    assert_eq!(core.recent_paths().len(), 0);
}

#[test]
fn non_database_is_unreadable() {
    let fx = Fx::example();
    let text = fx.dir.path().join("notes.db");
    fs::write(
        &text,
        "this is plainly not a database, but long enough to look like one",
    )
    .unwrap();
    let mut core = fx.core();
    core.open(&fx.db).unwrap();
    let e = core.open(&text).unwrap_err();
    assert_eq!(e.kind, DbmanagerErrorKind::DatabaseUnreadable);
    assert_eq!(core.session().database.unwrap().path, fx.db, "still open");
    assert_eq!(core.recent_paths(), vec![fx.db.clone()]);
}

#[test]
fn reads_without_session_fail() {
    let fx = Fx::example();
    let mut core = fx.core();
    core.session();
    let key = ObjectKey::Rowid { value: "1".into() };
    let q = M2mQuery {
        search: None,
        offset: 0,
        limit: 25,
        linked_only: false,
        only: None,
    };
    assert_eq!(
        core.list("author", &lq()).unwrap_err().kind,
        DbmanagerErrorKind::NoDatabase
    );
    assert_eq!(
        core.object("author", &key).unwrap_err().kind,
        DbmanagerErrorKind::NoDatabase
    );
    assert_eq!(
        core.m2m_options("book", Some(&key), "m2m:book_genre", &q)
            .unwrap_err()
            .kind,
        DbmanagerErrorKind::NoDatabase
    );
}

#[test]
fn reads_forward_to_the_session() {
    let fx = Fx::example();
    let mut core = fx.core();
    core.open(&fx.db).unwrap();
    let page = core.list("author", &lq()).unwrap();
    assert_eq!(page.total, 3);
    let obj = core.object("author", &page.items[0].key).unwrap();
    assert_eq!(obj.table, "author");
    let e = core.list("nope", &lq()).unwrap_err();
    assert_eq!(e.kind, DbmanagerErrorKind::UnknownTable);
}

#[test]
#[expect(clippy::too_many_lines, reason = "one table of every mapping")]
fn errors_map_to_kinds() {
    use DbmanagerErrorKind as K;
    let p = || PathBuf::from("/x.db");
    let open = |e: OpenError| DbmanagerError::from(e).kind;
    assert_eq!(open(OpenError::NotAbsolute(p())), K::NotAbsolute);
    assert_eq!(open(OpenError::NotAFile(p())), K::NotAFile);
    assert_eq!(open(OpenError::Busy(p())), K::Busy);
    assert_eq!(open(OpenError::NotUnicode(p())), K::DatabaseUnreadable);
    assert_eq!(
        open(OpenError::NotADatabase(p(), "m".into())),
        K::DatabaseUnreadable
    );
    assert_eq!(open(OpenError::Io(p(), "m".into())), K::DatabaseUnreadable);
    let unicode = DbmanagerError::from(OpenError::NotUnicode(p()));
    assert!(unicode.message.contains("not valid Unicode"));

    let fx = Fx::example();
    let mut core = fx.core();
    core.open(&fx.db).unwrap();
    let page = core.list("author", &lq()).unwrap();
    let current = core.object("author", &page.items[0].key).unwrap();
    let impact = DeleteImpact {
        table: "author".into(),
        key: page.items[0].key.clone(),
        label: "a".into(),
        version: "v".into(),
        groups: Vec::new(),
        blocked: true,
        truncated: false,
        has_triggers: false,
        token: "t".into(),
    };
    let cases: Vec<(SessionError, K)> = vec![
        (SessionError::Open(OpenError::Busy(p())), K::Busy),
        (SessionError::UnknownTable("t".into()), K::UnknownTable),
        (SessionError::UnknownObject, K::UnknownObject),
        (
            SessionError::UnknownRelation("r".into()),
            K::UnknownRelation,
        ),
        (SessionError::SchemaChanged, K::SchemaChanged),
        (SessionError::Busy, K::Busy),
        (SessionError::Sqlite("s".into()), K::Io),
        (SessionError::ReadOnlyDatabase, K::DatabaseReadOnly),
        (SessionError::ConfigChangedOnDisk, K::ConfigChangedOnDisk),
        (
            SessionError::NotEditable { reason: "r".into() },
            K::NotEditable,
        ),
        (
            SessionError::Validation {
                fields: Vec::new(),
                links: Vec::new(),
            },
            K::ValidationFailed,
        ),
        (
            SessionError::Conflict {
                current: Box::new(current.clone()),
                link: None,
            },
            K::Conflict,
        ),
        (
            SessionError::BackupFailed { detail: "d".into() },
            K::BackupFailed,
        ),
        (
            SessionError::Constraint {
                message: "m".into(),
            },
            K::ConstraintFailed,
        ),
        (
            SessionError::ImpactChanged(Box::new(impact.clone())),
            K::ImpactChanged,
        ),
        (
            SessionError::DeleteBlocked(Box::new(impact.clone())),
            K::DeleteBlocked,
        ),
    ];
    for (error, kind) in cases {
        let write = matches!(
            kind,
            K::DatabaseReadOnly
                | K::ConfigChangedOnDisk
                | K::NotEditable
                | K::ValidationFailed
                | K::Conflict
                | K::BackupFailed
                | K::ConstraintFailed
                | K::ImpactChanged
                | K::DeleteBlocked
        );
        let mapped = DbmanagerError::from(error);
        assert_eq!(mapped.kind, kind);
        if write {
            assert!(
                mapped.message.ends_with("Nothing was changed."),
                "{kind:?}: {}",
                mapped.message
            );
        }
        assert_eq!(mapped.current.is_some(), kind == K::Conflict);
        assert_eq!(
            mapped.impact.is_some(),
            matches!(kind, K::ImpactChanged | K::DeleteBlocked)
        );
    }
}

#[test]
fn error_kinds_are_pinned() {
    let names: Vec<_> = DbmanagerErrorKind::ALL.iter().map(|k| k.name()).collect();
    assert_eq!(
        names,
        [
            "NoDatabase",
            "NotAbsolute",
            "NotAFile",
            "DatabaseUnreadable",
            "OpenerFailed",
            "ConfigNotDroppable",
            "DropAmbiguous",
            "UnknownTable",
            "UnknownObject",
            "UnknownRelation",
            "NotEditable",
            "ValidationFailed",
            "Conflict",
            "SchemaChanged",
            "ConfigChangedOnDisk",
            "DatabaseReadOnly",
            "Busy",
            "BackupFailed",
            "ImpactChanged",
            "DeleteBlocked",
            "ConstraintFailed",
            "Io",
        ]
    );
}

#[test]
fn reload_with_and_without_database() {
    let fx = Fx::example();
    let mut core = fx.core();

    // No database: only the config is reloaded.
    core.session();
    fs::write(&fx.cfg, "version = 2\n").unwrap();
    assert!(core.session().config_changed);
    core.reload().unwrap();
    let s = core.session();
    assert!(!s.config_changed && !s.open);
    assert_eq!(s.config.status, ConfigStatus::FailClosed);
    fs::write(&fx.cfg, EXAMPLE).unwrap();
    core.reload().unwrap();

    // With a database: reopened with the new config, backup kept.
    core.open(&fx.db).unwrap();
    let backup = fx.dir.path().join("t.db.backup");
    core.adopt_backup_for_tests(backup.clone());
    assert!(core.session().tables.iter().any(|t| t.can_create));
    fs::write(
        &fx.cfg,
        "version = 1\n[defaults]\ntable_mode = \"read-only\"\n",
    )
    .unwrap();
    assert!(core.session().config_changed);
    let old = Core::session_opens();
    core.reload().unwrap();
    let s = core.session();
    assert!(!s.config_changed);
    assert_eq!(Core::session_opens(), old + 1, "reopened once");
    assert!(s.tables.iter().all(|t| !t.can_create));
    assert_eq!(s.backup_path, Some(backup), "backup kept, no second one");
    assert!(core.list("author", &lq()).is_ok());
}

#[test]
fn reload_config_applies_without_reopening() {
    let fx = Fx::example();
    let mut core = fx.core();
    core.open(&fx.db).unwrap();
    let addr = Core::session_opens();
    fs::write(
        &fx.cfg,
        "version = 1\n[defaults]\ntable_mode = \"read-only\"\n",
    )
    .unwrap();
    core.reload_config();
    assert_eq!(Core::session_opens(), addr);
    let s = core.session();
    assert!(!s.config_changed);
    assert!(s.tables.iter().all(|t| !t.can_create));
}

#[test]
fn close_keeps_config() {
    let fx = Fx::example();
    let mut core = fx.core();
    core.open(&fx.db).unwrap();
    core.close();
    let s = core.session();
    assert!(!s.open && s.database.is_none() && s.tables.is_empty());
    assert_eq!(s.config.status, ConfigStatus::Loaded);
    assert_eq!(s.recent.len(), 1, "the recent list is kept");
    assert_eq!(
        core.list("author", &lq()).unwrap_err().kind,
        DbmanagerErrorKind::NoDatabase
    );
}

#[test]
fn display_root_maps_paths() {
    let fx = Fx::example();
    let mut core = fx
        .core()
        .with_display_root(fx.dir.path(), Path::new("/fixture"));
    core.open(&fx.db).unwrap();
    let s = core.session();
    // The root is kept as given; only the appended part uses the platform separator.
    assert_eq!(
        s.database.unwrap().display,
        format!("/fixture{}t.db", std::path::MAIN_SEPARATOR)
    );
    assert_eq!(s.recent.len(), 1);
}

// ---- save ----

fn rk(n: i64) -> ObjectKey {
    ObjectKey::Rowid {
        value: n.to_string(),
    }
}

fn author_change(core: &mut Core, hash: &str, last_name: &str) -> SaveRequest {
    SaveRequest {
        table: "author".into(),
        key: Some(rk(1)),
        version: Some(core.object("author", &rk(1)).unwrap().version),
        config_hash: hash.to_owned(),
        changes: vec![FieldChange {
            column: "last_name".into(),
            input: FieldInput::Value {
                value: DbValue::Text(last_name.into()),
            },
        }],
        links: Vec::new(),
    }
}

fn baks(fx: &Fx) -> usize {
    fs::read_dir(fx.dir.path())
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".bak")
        })
        .count()
}

fn last_name(core: &Core) -> String {
    let view = core.object("author", &rk(1)).unwrap();
    match &view
        .fields
        .iter()
        .find(|f| f.column == "last_name")
        .unwrap()
        .value
    {
        DbValue::Text(t) => t.clone(),
        other => panic!("{other:?}"),
    }
}

const READ_ONLY_CONFIG: &str = "version = 1\n[defaults]\ntable_mode = \"read-only\"\n";

#[test]
fn stale_config_hash_reloads_and_refuses() {
    let fx = Fx::example();
    let mut core = fx.core();
    core.open(&fx.db).unwrap();
    let request = author_change(&mut core, "not-the-hash", "Changed");
    let err = core.save(&request).unwrap_err();
    assert_eq!(err.kind, DbmanagerErrorKind::ConfigChangedOnDisk);
    assert_eq!(last_name(&core), "Lindqvist", "nothing was saved");
    assert_eq!(baks(&fx), 0, "refused before the backup");
}

#[test]
fn changed_file_without_reload_refuses_and_reloads() {
    let fx = Fx::example();
    let mut core = fx.core();
    core.open(&fx.db).unwrap();
    let old = core.session().config.hash;
    fs::write(&fx.cfg, READ_ONLY_CONFIG).unwrap();
    assert!(core.session().config_changed);
    let request = author_change(&mut core, &old, "Changed");
    let err = core.save(&request).unwrap_err();
    assert_eq!(err.kind, DbmanagerErrorKind::ConfigChangedOnDisk);
    let s = core.session();
    assert!(!s.config_changed, "the core reloaded before returning");
    assert_ne!(s.config.hash, old);
    assert!(s.tables.iter().all(|t| !t.can_create));
    assert_eq!(last_name(&core), "Lindqvist");
}

#[test]
fn retry_with_new_hash_succeeds() {
    let fx = Fx::example();
    let mut core = fx.core();
    core.open(&fx.db).unwrap();
    let old = core.session().config.hash;
    fs::write(&fx.cfg, format!("{EXAMPLE}\n# touched\n")).unwrap();
    let request = author_change(&mut core, &old, "Changed");
    assert!(core.save(&request).is_err());
    let hash = core.session().config.hash;
    assert_ne!(hash, old);
    let request = author_change(&mut core, &hash, "Changed");
    core.save(&request).unwrap();
    assert_eq!(last_name(&core), "Changed");
}

#[test]
fn backup_path_after_first_save() {
    let fx = Fx::example();
    let mut core = fx.core();
    core.open(&fx.db).unwrap();
    assert!(core.session().backup_path.is_none());
    let hash = core.session().config.hash;
    let request = author_change(&mut core, &hash, "One");
    let first = core.save(&request).unwrap();
    let path = core.session().backup_path.expect("a backup");
    assert!(path.exists());
    assert_eq!(first.backup_created, Some(path.display().to_string()));
    assert_eq!(baks(&fx), 1);
    let request = author_change(&mut core, &hash, "Two");
    assert!(core.save(&request).unwrap().backup_created.is_none());
    assert_eq!(baks(&fx), 1);
}

#[test]
fn reload_keeps_backup() {
    let fx = Fx::example();
    let mut core = fx.core();
    core.open(&fx.db).unwrap();
    let hash = core.session().config.hash;
    let request = author_change(&mut core, &hash, "One");
    core.save(&request).unwrap();
    let path = core.session().backup_path;
    core.reload().unwrap();
    assert_eq!(core.session().backup_path, path);
    let request = author_change(&mut core, &hash, "Two");
    assert!(core.save(&request).unwrap().backup_created.is_none());
    assert_eq!(baks(&fx), 1);
}

#[test]
fn open_again_resets_backup() {
    let fx = Fx::example();
    let mut core = fx.core();
    core.open(&fx.db).unwrap();
    let hash = core.session().config.hash;
    let request = author_change(&mut core, &hash, "One");
    core.save(&request).unwrap();
    core.open(&fx.db).unwrap();
    assert!(core.session().backup_path.is_none());
    let request = author_change(&mut core, &hash, "Two");
    assert!(core.save(&request).unwrap().backup_created.is_some());
    assert_eq!(baks(&fx), 2);
}

#[test]
fn reload_after_failed_first_save_still_announces_backup() {
    let fx = Fx::example();
    let mut core = fx.core();
    core.open(&fx.db).unwrap();
    let hash = core.session().config.hash;
    // Fails after the backup: the isbn is taken.
    let dup = SaveRequest {
        table: "book".into(),
        key: None,
        version: None,
        config_hash: hash.clone(),
        changes: [
            ("title", DbValue::Text("T".into())),
            ("isbn", DbValue::Text("9780000000011".into())),
        ]
        .into_iter()
        .map(|(column, value)| FieldChange {
            column: column.into(),
            input: FieldInput::Value { value },
        })
        .chain([FieldChange {
            column: "author_id".into(),
            input: FieldInput::Reference { key: Some(rk(1)) },
        }])
        .collect(),
        links: Vec::new(),
    };
    assert_eq!(
        core.save(&dup).unwrap_err().kind,
        DbmanagerErrorKind::ValidationFailed
    );
    assert!(core.session().backup_path.is_some());
    assert!(!core.backup_announced_for_tests());
    core.reload().unwrap();
    assert!(!core.backup_announced_for_tests(), "carried across");
    let request = author_change(&mut core, &hash, "One");
    assert!(core.save(&request).unwrap().backup_created.is_some());
    assert_eq!(baks(&fx), 1);
}

#[test]
fn delete_stale_config_hash_reloads_and_refuses() {
    let fx = Fx::example();
    let mut core = fx.core();
    core.open(&fx.db).unwrap();
    let impact = core.delete_preview("member", &rk(2)).unwrap();
    let err = core
        .delete(
            "member",
            &rk(2),
            &impact.version,
            &impact.token,
            "not-the-hash",
        )
        .unwrap_err();
    assert_eq!(err.kind, DbmanagerErrorKind::ConfigChangedOnDisk);
    assert!(core.object("member", &rk(2)).is_ok(), "nothing was deleted");
    assert_eq!(baks(&fx), 0, "refused before the backup");
    // The config was reloaded, so the current hash goes through.
    let hash = core.session().config.hash;
    let done = core
        .delete("member", &rk(2), &impact.version, &impact.token, &hash)
        .unwrap();
    assert_eq!(done.table, "member");
    assert_eq!(
        core.object("member", &rk(2)).unwrap_err().kind,
        DbmanagerErrorKind::UnknownObject
    );
    assert_eq!(baks(&fx), 1);
}

#[test]
fn delete_without_database_fails() {
    let fx = Fx::example();
    let mut core = fx.core();
    core.close();
    assert_eq!(
        core.delete_preview("member", &rk(2)).unwrap_err().kind,
        DbmanagerErrorKind::NoDatabase
    );
    assert_eq!(
        core.delete("member", &rk(2), "v", "t", "h")
            .unwrap_err()
            .kind,
        DbmanagerErrorKind::NoDatabase
    );
}

#[test]
fn save_with_links_through_core() {
    use crate::session::{LinkAdd, LinkChange, LinkRemove, LinkUpdate};

    let fx = Fx::example();
    let mut core = fx.core();
    core.open(&fx.db).unwrap();
    let hash = core.session().config.hash;
    let book = core.object("book", &rk(1)).unwrap();
    let query = M2mQuery {
        search: None,
        offset: 0,
        limit: 50,
        linked_only: true,
        only: None,
    };
    let contribution = core
        .m2m_options("book", Some(&rk(1)), "m2m:contribution", &query)
        .unwrap();
    let editor = contribution.items.first().expect("a linked contributor");
    let version = editor.link_version.clone().expect("a link version");
    let poetry = ObjectKey::Primary {
        values: vec![DbValue::Text("poetry".into())],
    };
    let request = SaveRequest {
        table: "book".into(),
        key: Some(rk(1)),
        version: Some(book.version),
        config_hash: hash,
        changes: Vec::new(),
        links: vec![
            LinkChange {
                relation: "m2m:book_genre".into(),
                add: vec![LinkAdd {
                    other: poetry.clone(),
                    fields: Vec::new(),
                }],
                update: Vec::new(),
                remove: vec![LinkRemove {
                    other: ObjectKey::Primary {
                        values: vec![DbValue::Text("fiction".into())],
                    },
                    version: None,
                }],
            },
            LinkChange {
                relation: "m2m:contribution".into(),
                add: Vec::new(),
                update: vec![LinkUpdate {
                    other: editor.key.clone(),
                    version,
                    fields: vec![FieldChange {
                        column: "role".into(),
                        input: FieldInput::Value {
                            value: DbValue::Text("translator".into()),
                        },
                    }],
                }],
                remove: Vec::new(),
            },
        ],
    };
    let saved = core.save(&request).unwrap();
    assert!(saved.backup_created.is_some());
    assert_eq!(baks(&fx), 1);
    let genres = core
        .m2m_options("book", Some(&rk(1)), "m2m:book_genre", &query)
        .unwrap();
    let keys: Vec<_> = genres.items.iter().map(|i| i.key.clone()).collect();
    assert_eq!(keys, vec![poetry], "fiction was removed, poetry added");
    let contribution = core
        .m2m_options("book", Some(&rk(1)), "m2m:contribution", &query)
        .unwrap();
    let values = contribution.items[0].link_values.clone().unwrap();
    assert!(values
        .iter()
        .any(|v| v.column == "role" && v.value == DbValue::Text("translator".into())));
}

// ---- Edit config ----

#[test]
fn edit_config_creates_starter_when_missing() {
    let fx = Fx::new(None);
    let nested = fx.dir.path().join("a").join("b").join("dbmanager.toml");
    let core = Core::new(&fx.dirs(), Some(nested.clone()));
    let (path, created) = core.edit_config_target().unwrap();
    assert_eq!(path, nested);
    assert!(created);
    assert_eq!(
        fs::read_to_string(&nested).unwrap(),
        include_str!("../../../../examples/dbmanager/starter.toml")
    );
}

#[test]
fn edit_config_never_overwrites() {
    let fx = Fx::new(Some("version = 1\n# mine\n"));
    let core = fx.core();
    let (path, created) = core.edit_config_target().unwrap();
    assert_eq!(path, fx.cfg);
    assert!(!created);
    assert_eq!(
        fs::read_to_string(&fx.cfg).unwrap(),
        "version = 1\n# mine\n"
    );
    // A second call after a creation is not a creation either.
    let missing = Fx::new(None);
    let core = missing.core();
    assert!(core.edit_config_target().unwrap().1);
    assert!(!core.edit_config_target().unwrap().1);
}

#[test]
fn edit_config_without_a_location_is_io() {
    let fx = Fx::new(None);
    let core = Core::new(&fx.dirs(), None);
    assert_eq!(
        core.edit_config_target().unwrap_err().kind,
        DbmanagerErrorKind::Io
    );
}

#[test]
fn edit_config_under_a_file_is_io() {
    let fx = Fx::new(None);
    let blocker = fx.dir.path().join("blocker");
    fs::write(&blocker, "x").unwrap();
    let core = Core::new(&fx.dirs(), Some(blocker.join("dbmanager.toml")));
    assert_eq!(
        core.edit_config_target().unwrap_err().kind,
        DbmanagerErrorKind::Io
    );
}

#[test]
fn starter_is_inert() {
    let fx = Fx::new(None);
    fx.core().edit_config_target().unwrap();
    let loaded = LoadedConfig::load(&fx.cfg);
    assert_eq!(loaded.diagnostics(), &[]);
    assert_eq!(loaded.status(), ConfigStatus::Loaded);
    assert_eq!(loaded.fail_closed_reason(), None);
}
