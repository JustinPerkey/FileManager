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
//! **Timing** (the harness's `Step::WaitIdle` is a short fixed settle, not
//! idle detection). The first `session` call opens a tiny database and
//! introspects it, and every list or object read queries it, all on the
//! `Worker`: a few milliseconds each, not instant. A scenario that shows a
//! loaded list or object adds `Step::Wait(200)` after the step that causes
//! the read. [`DbmanagerFixture::Empty`] opens nothing and answers within the
//! settle.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fm_core::AppDirs;
use fm_dbmanager::session::{ListQuery, M2mPage, M2mQuery, ObjectKey, ObjectPage, ObjectView};
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
            DbmanagerFixture::Empty | DbmanagerFixture::Loaded | DbmanagerFixture::StateWarning => {
                Some(EXAMPLE_CONFIG)
            }
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
            _env: self.clone(),
        }
    }

    /// The temp dir.
    #[must_use]
    pub fn dir(&self) -> &Path {
        self.dir.path()
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
    _env: FixtureEnv,
}

impl FixtureDbmanager {
    /// The fixture a screenshot state names.
    #[must_use]
    pub fn from_fixture(fixture: DbmanagerFixture) -> Self {
        FixtureEnv::new(fixture).backend()
    }
}

impl DbmanagerBackend for FixtureDbmanager {
    fn session(&mut self) -> DbmanagerSnapshot {
        self.real.session()
    }

    fn open(&mut self, path: &Path) -> Result<DbmanagerSnapshot, DbmanagerError> {
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
}
