//! Fixture backends for the Database Manager: tests and the screenshot
//! harness. An in-memory fixture is not practical for `SQLite` views, so each
//! fixture is a real [`Core`] over a database built from the example schema in
//! a temp dir **the fixture creates** (never a user path), with the example
//! config as the config in the same temp dir. The dir is removed when the
//! backend (and every [`FixtureEnv`] clone) is dropped.
//!
//! Display paths are fake: the temp dir is shown as `C:\Fixture` (as
//! `/fixture` off Windows), through [`Core::with_display_root`].
//!
//! **Edits.** Saves run on the `Worker` against the real database: a save
//! (and its first-write backup) takes a few milliseconds, like a read, so a
//! scenario that shows a saved sheet, a backup notice, a refused save or the
//! conflict panel adds `Step::Wait(200)` after the Save click. Typing and the
//! picker's first page behave like reads (`Wait(200)` for the picker's rows).
//! [`DbmanagerFixture::ConflictOnSave`] makes the first `book` save conflict.
//! The `dbmanager-config-reloaded` scenario injects the file-changed signal
//! (`ToolHooks::inject_file_changed`), which rewrites the temp config so book
//! `pages` is read-only, reloads and re-checks the draft: add `Wait(300)`.
//!
//! **Timing** (the harness's `Step::WaitIdle` is a short fixed settle, not
//! idle detection). The first `session` call opens a tiny database and
//! introspects it, and every list or object read queries it, all on the
//! `Worker`: a few milliseconds each, not instant. A scenario that shows a
//! loaded list or object adds `Step::Wait(200)` after the step that causes
//! the read. [`DbmanagerFixture::Empty`] opens nothing and answers within the
//! settle.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use fm_core::AppDirs;
use fm_dbmanager::session::{
    ListQuery, M2mPage, M2mQuery, ObjectKey, ObjectPage, ObjectView, SaveRequest, SaveResult,
};
use fm_dbmanager::tool::{Core, DbmanagerError, DbmanagerSnapshot};

use super::backend::{DbmanagerBackend, RealDbmanager};

const SCHEMA: &str = include_str!("../../../../../examples/dbmanager/schema.sql");
const EXAMPLE_CONFIG: &str = include_str!("../../../../../examples/dbmanager/example.toml");

/// Which screenshot state a fixture starts in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DbmanagerFixture {
    /// No database, the example config: the empty state.
    Empty,
    /// The example database was the most recent one and is restored open
    /// (`loaded`).
    Loaded,
    /// A restored database and no config file: everything editable, with the
    /// notice.
    ConfigMissing,
    /// A restored database and a config that cannot be used: everything
    /// read-only, with the reason.
    ConfigFailClosed,
    /// The most recent database is gone: nothing is open, the state warning
    /// says which and why.
    StateWarning,
    /// As `Loaded`, but another program changes the saved book (its revision)
    /// just before the first save reaches the database, so that save ends in
    /// the conflict panel. Only a `book` save is disturbed, once.
    ConflictOnSave,
}

/// A temp dir with a database, a config and a state folder, shared by the
/// backends made from it.
#[derive(Clone)]
pub struct FixtureEnv {
    dir: Arc<tempfile::TempDir>,
    /// The example database (it exists except in `StateWarning`).
    pub db: PathBuf,
    /// The app-wide config file (absent in `ConfigMissing`).
    pub config: PathBuf,
    dirs: AppDirs,
    /// Test hook: an `open` of this path on any backend of this env panics.
    panic_open_of: Arc<Mutex<Option<PathBuf>>>,
}

fn shown_root() -> PathBuf {
    PathBuf::from(if cfg!(windows) {
        r"C:\Fixture"
    } else {
        "/fixture"
    })
}

impl FixtureEnv {
    /// Builds the temp dir for `fixture`: the database from the example
    /// schema, the config, and (for the states that restore a database) the
    /// recent list naming it.
    ///
    /// # Panics
    /// If the temp dir or the database cannot be created.
    #[must_use]
    pub fn new(fixture: DbmanagerFixture) -> Self {
        let dir = tempfile::tempdir().expect("a temp dir for the fixture");
        let db = dir.path().join("library.db");
        let config = dir.path().join("dbmanager.toml");
        let dirs = AppDirs::at(dir.path().join("state"));
        rusqlite::Connection::open(&db)
            .and_then(|c| c.execute_batch(SCHEMA))
            .expect("the example database");
        let text = match fixture {
            DbmanagerFixture::Empty
            | DbmanagerFixture::Loaded
            | DbmanagerFixture::StateWarning
            | DbmanagerFixture::ConflictOnSave => Some(EXAMPLE_CONFIG),
            DbmanagerFixture::ConfigMissing => None,
            DbmanagerFixture::ConfigFailClosed => Some("version = 2\n"),
        };
        if let Some(text) = text {
            std::fs::write(&config, text).expect("the fixture config");
        }
        let env = Self {
            dir: Arc::new(dir),
            db,
            config,
            dirs,
            panic_open_of: Arc::new(Mutex::new(None)),
        };
        if fixture != DbmanagerFixture::Empty {
            // Remember the database as an earlier run would have.
            env.core().open(&env.db).expect("open the example database");
        }
        if fixture == DbmanagerFixture::StateWarning {
            std::fs::remove_file(&env.db).expect("remove the example database");
        }
        env
    }

    /// A fresh core over this env's state, as a new run would make.
    #[must_use]
    pub fn core(&self) -> Core {
        Core::new(&self.dirs, Some(self.config.clone()))
            .with_display_root(self.dir.path(), &shown_root())
    }

    /// A backend over a fresh core; it keeps the temp dir alive.
    #[must_use]
    pub fn backend(&self) -> FixtureDbmanager {
        FixtureDbmanager {
            real: RealDbmanager::with_core(self.core()),
            env: self.clone(),
            conflict_on_save: false,
        }
    }

    /// Makes an `open` of `path` through a backend of this env panic.
    ///
    /// # Panics
    /// If the hook's lock is poisoned.
    pub fn panic_on_open_of(&self, path: &Path) {
        *self.panic_open_of.lock().unwrap() = Some(path.to_path_buf());
    }

    /// The temp dir.
    #[must_use]
    pub fn dir(&self) -> &Path {
        self.dir.path()
    }

    /// Rewrites the config so the book's `pages` field is read-only (the
    /// `dbmanager-config-reloaded` scenario).
    ///
    /// # Panics
    /// If the config cannot be written.
    pub fn edit_config_book_pages_read_only(&self) {
        let text = EXAMPLE_CONFIG.replacen(
            "[tables.book.fields.pages]\n",
            "[tables.book.fields.pages]\nmode = \"read-only\"\n",
            1,
        );
        assert_ne!(text, EXAMPLE_CONFIG, "the example config has the table");
        std::fs::write(&self.config, text).expect("the edited fixture config");
    }

    /// Runs `sql` on the database from a second connection (tests change
    /// data behind the model's back).
    ///
    /// # Panics
    /// If the SQL fails.
    pub fn exec(&self, sql: &str) {
        let conn = rusqlite::Connection::open(&self.db).expect("open the fixture database");
        conn.execute_batch("PRAGMA foreign_keys = OFF;")
            .expect("pragma");
        conn.execute_batch(sql).expect("fixture SQL");
    }
}

/// A [`DbmanagerBackend`] over a real core and a database in a temp dir of
/// its own.
pub struct FixtureDbmanager {
    real: RealDbmanager,
    env: FixtureEnv,
    conflict_on_save: bool,
}

impl FixtureDbmanager {
    /// The fixture a screenshot state names.
    #[must_use]
    pub fn from_fixture(fixture: DbmanagerFixture) -> Self {
        let mut backend = FixtureEnv::new(fixture).backend();
        backend.conflict_on_save = fixture == DbmanagerFixture::ConflictOnSave;
        backend
    }
}

impl DbmanagerBackend for FixtureDbmanager {
    fn session(&mut self) -> DbmanagerSnapshot {
        self.real.session()
    }

    fn open(&mut self, path: &Path) -> Result<DbmanagerSnapshot, DbmanagerError> {
        let hook = self.env.panic_open_of.lock().ok().and_then(|g| g.clone());
        assert!(hook.as_deref() != Some(path), "fixture open panicked");
        self.real.open(path)
    }

    fn open_recent(&mut self, index: usize) -> Result<DbmanagerSnapshot, DbmanagerError> {
        self.real.open_recent(index)
    }

    fn open_dropped(&mut self, paths: Vec<PathBuf>) -> Result<DbmanagerSnapshot, DbmanagerError> {
        self.real.open_dropped(paths)
    }

    fn reload(&mut self) -> Result<DbmanagerSnapshot, DbmanagerError> {
        self.real.reload()
    }

    fn close(&mut self) -> DbmanagerSnapshot {
        self.real.close()
    }

    fn list(&mut self, table: &str, query: &ListQuery) -> Result<ObjectPage, DbmanagerError> {
        self.real.list(table, query)
    }

    fn object(&mut self, table: &str, key: &ObjectKey) -> Result<ObjectView, DbmanagerError> {
        self.real.object(table, key)
    }

    fn m2m_options(
        &mut self,
        table: &str,
        key: Option<&ObjectKey>,
        relation: &str,
        query: &M2mQuery,
    ) -> Result<M2mPage, DbmanagerError> {
        self.real.m2m_options(table, key, relation, query)
    }

    fn save(&mut self, request: SaveRequest) -> Result<SaveResult, DbmanagerError> {
        if request.table == "book" && std::mem::take(&mut self.conflict_on_save) {
            if let Some(ObjectKey::Rowid { value }) = &request.key {
                self.env.exec(&format!(
                    "UPDATE book SET revision = revision + 1 WHERE id = {}",
                    value.parse::<i64>().unwrap_or(0)
                ));
            }
        }
        self.real.save(request)
    }

    fn simulate_config_edit(&mut self) {
        self.env.edit_config_book_pages_read_only();
    }
}
