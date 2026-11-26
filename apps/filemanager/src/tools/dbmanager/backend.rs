//! The Database Manager's backend as the model sees it: a trait over
//! [`fm_dbmanager::tool::Core`], so the model runs against the real core or a
//! fixture. Every method may block (opening, introspecting and querying a
//! database on a slow share can take seconds), so the model calls them only
//! through its [`crate::jobs::Worker`].

use std::path::{Path, PathBuf};

use fm_dbmanager::session::{ListQuery, M2mPage, M2mQuery, ObjectKey, ObjectPage, ObjectView};
use fm_dbmanager::tool::{Core, DbmanagerError, DbmanagerErrorKind, DbmanagerSnapshot};

/// What the Database Manager model needs from its core: one method per
/// [`Core`] operation. Writes come with later parts.
pub trait DbmanagerBackend: Send + 'static {
    /// The current state. The first call loads the config and reopens the
    /// most recent database.
    fn session(&mut self) -> DbmanagerSnapshot;

    /// Opens the database at `path`. The current one stays open on failure.
    ///
    /// # Errors
    /// `NotAbsolute`, `NotAFile`, `Busy` or `DatabaseUnreadable`.
    fn open(&mut self, path: &Path) -> Result<DbmanagerSnapshot, DbmanagerError>;

    /// Opens the recent database at position `index` of the snapshot's
    /// `recent` list, by its stored path.
    ///
    /// # Errors
    /// As [`DbmanagerBackend::open`]; `Io` when there is no such position.
    fn open_recent(&mut self, index: usize) -> Result<DbmanagerSnapshot, DbmanagerError>;

    /// Opens a database from dropped paths.
    ///
    /// # Errors
    /// `DropAmbiguous`, `ConfigNotDroppable`, or as `open`.
    fn open_dropped(&mut self, paths: Vec<PathBuf>) -> Result<DbmanagerSnapshot, DbmanagerError>;

    /// Reloads the config and reopens the database (keeping the session's
    /// backup).
    ///
    /// # Errors
    /// The database cannot be reopened.
    fn reload(&mut self) -> Result<DbmanagerSnapshot, DbmanagerError>;

    /// Closes the database; the config stays.
    fn close(&mut self) -> DbmanagerSnapshot;

    /// One page of `table`'s objects.
    ///
    /// # Errors
    /// `NoDatabase`, or the session's error.
    fn list(&mut self, table: &str, query: &ListQuery) -> Result<ObjectPage, DbmanagerError>;

    /// One object.
    ///
    /// # Errors
    /// `NoDatabase`, or the session's error.
    fn object(&mut self, table: &str, key: &ObjectKey) -> Result<ObjectView, DbmanagerError>;

    /// One page of a many-to-many relation's other side.
    ///
    /// # Errors
    /// `NoDatabase`, or the session's error.
    fn m2m_options(
        &mut self,
        table: &str,
        key: Option<&ObjectKey>,
        relation: &str,
        query: &M2mQuery,
    ) -> Result<M2mPage, DbmanagerError>;
}

/// The real core.
pub struct RealDbmanager {
    core: Core,
}

impl RealDbmanager {
    /// The core over the system's app data directory and the app-wide config
    /// file. Without a data directory it remembers nothing and says so as its
    /// state warning; without a config location it fails closed.
    #[must_use]
    pub fn new() -> Self {
        let config = crate::app::dirs::system_config_path()
            .ok()
            .map(|dir| dir.join("dbmanager.toml"));
        let core = match crate::app::dirs::user_app_dirs() {
            Ok(dirs) => Core::new(&dirs, config),
            Err(e) => Core::unpersisted(format!("saved state is unavailable: {e}."), config),
        };
        Self { core }
    }

    /// Over an existing core (tests and fixtures pass temp-dir state).
    #[must_use]
    pub fn with_core(core: Core) -> Self {
        Self { core }
    }
}

impl Default for RealDbmanager {
    fn default() -> Self {
        Self::new()
    }
}

impl DbmanagerBackend for RealDbmanager {
    fn session(&mut self) -> DbmanagerSnapshot {
        self.core.session()
    }

    fn open(&mut self, path: &Path) -> Result<DbmanagerSnapshot, DbmanagerError> {
        self.core.open(path)?;
        Ok(self.core.session())
    }

    fn open_recent(&mut self, index: usize) -> Result<DbmanagerSnapshot, DbmanagerError> {
        let path = self
            .core
            .recent_paths()
            .into_iter()
            .nth(index)
            .ok_or_else(|| {
                DbmanagerError::new(
                    DbmanagerErrorKind::Io,
                    "that entry is no longer in the recent list",
                )
            })?;
        self.open(&path)
    }

    fn open_dropped(&mut self, paths: Vec<PathBuf>) -> Result<DbmanagerSnapshot, DbmanagerError> {
        self.core.open_dropped(&paths)?;
        Ok(self.core.session())
    }

    fn reload(&mut self) -> Result<DbmanagerSnapshot, DbmanagerError> {
        self.core.reload()?;
        Ok(self.core.session())
    }

    fn close(&mut self) -> DbmanagerSnapshot {
        self.core.close();
        self.core.session()
    }

    fn list(&mut self, table: &str, query: &ListQuery) -> Result<ObjectPage, DbmanagerError> {
        self.core.list(table, query)
    }

    fn object(&mut self, table: &str, key: &ObjectKey) -> Result<ObjectView, DbmanagerError> {
        self.core.object(table, key)
    }

    fn m2m_options(
        &mut self,
        table: &str,
        key: Option<&ObjectKey>,
        relation: &str,
        query: &M2mQuery,
    ) -> Result<M2mPage, DbmanagerError> {
        self.core.m2m_options(table, key, relation, query)
    }
}
