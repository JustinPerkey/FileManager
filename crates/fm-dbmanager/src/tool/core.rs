//! [`Core`]: the tool's session logic with no UI types.

use std::path::{Path, PathBuf};

use fm_core::{AppDirs, Store};

use super::types::{
    DbConfigView, DbFileSlot, DbmanagerError, DbmanagerErrorKind as K, DbmanagerSnapshot,
};
use crate::config::{ConfigDiagnostic, DiagnosticSeverity, LoadedConfig};
use crate::session::{
    DeleteImpact, DeleteResult, ListQuery, M2mPage, M2mQuery, ObjectKey, ObjectPage, ObjectView,
    SaveRequest, SaveResult, Session, SessionError,
};
use crate::state::RememberedState;

/// The commented, inert starter config Edit config creates.
const STARTER_CONFIG: &str = include_str!("../../../../examples/dbmanager/starter.toml");

/// The Database Manager's state: the one config, the recent databases and
/// the open database.
pub struct Core {
    store: Option<Store<RememberedState>>,
    remembered: RememberedState,
    /// `None`: the location could not be determined, so the config is
    /// fail-closed.
    config_path: Option<PathBuf>,
    /// Loaded by the first call that needs it.
    config: Option<LoadedConfig>,
    slot: Option<DbFileSlot>,
    session: Option<Session>,
    /// The first call has loaded the config and restored the last database.
    started: bool,
    state_warning: Option<String>,
    /// Fixtures: paths under `.0` are shown as under `.1`.
    display_root: Option<(PathBuf, PathBuf)>,
}

#[cfg(test)]
thread_local! {
    /// Tests: how many sessions this thread opened, to tell instances apart.
    static OPENS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl Core {
    /// A core remembering in `dirs`. `config_path` is the app-wide config
    /// file; `None` when its location is unknown, which fails closed.
    #[must_use]
    pub fn new(dirs: &AppDirs, config_path: Option<PathBuf>) -> Core {
        match Store::<RememberedState>::load(dirs, "dbmanager", "state") {
            Ok((store, warning)) => {
                let remembered = store.get().clone();
                Core::with(
                    Some(store),
                    remembered,
                    config_path,
                    warning.map(|w| {
                        format!(
                            "{} (the unusable file was kept as {}).",
                            w.reason,
                            w.kept_file.display()
                        )
                    }),
                )
            }
            Err(e) => {
                Core::unpersisted(format!("saved state could not be read: {e}."), config_path)
            }
        }
    }

    /// A core that remembers nothing between runs, with `warning` shown.
    #[must_use]
    pub fn unpersisted(warning: String, config_path: Option<PathBuf>) -> Core {
        Core::with(None, RememberedState::default(), config_path, Some(warning))
    }

    fn with(
        store: Option<Store<RememberedState>>,
        remembered: RememberedState,
        config_path: Option<PathBuf>,
        state_warning: Option<String>,
    ) -> Core {
        Core {
            store,
            remembered,
            config_path,
            config: None,
            slot: None,
            session: None,
            started: false,
            state_warning,
            display_root: None,
        }
    }

    /// Fixtures: shows every path under `real` as the same path under
    /// `shown`.
    #[must_use]
    pub fn with_display_root(mut self, real: &Path, shown: &Path) -> Core {
        self.display_root = Some((real.to_path_buf(), shown.to_path_buf()));
        self
    }

    fn display(&self, path: &Path) -> String {
        if let Some((real, shown)) = &self.display_root {
            if let Ok(rest) = path.strip_prefix(real) {
                return shown.join(rest).to_string_lossy().into_owned();
            }
        }
        path.to_string_lossy().into_owned()
    }

    fn persist(&mut self) {
        let Some(store) = self.store.as_mut() else {
            return;
        };
        let snapshot = self.remembered.clone();
        store.update(|s| *s = snapshot);
        if let Err(e) = store.save() {
            self.add_warning(&format!("saved state could not be written: {e}."));
        }
    }

    /// Adds a warning sentence after any earlier one, once.
    fn add_warning(&mut self, text: &str) {
        match &mut self.state_warning {
            None => self.state_warning = Some(text.to_owned()),
            Some(existing) => {
                if !existing.contains(text) {
                    existing.push(' ');
                    existing.push_str(text);
                }
            }
        }
    }

    fn new_session(path: &Path, config: LoadedConfig) -> Result<Session, SessionError> {
        #[cfg(test)]
        {
            OPENS.with(|n| n.set(n.get() + 1));
        }
        Session::open(path, config)
    }

    fn read_config(&self) -> LoadedConfig {
        match &self.config_path {
            Some(path) => LoadedConfig::load(path),
            None => LoadedConfig::location_unknown(),
        }
    }

    /// The first call: loads the config and reopens the most recent
    /// database. A database that cannot be reopened stays in the recent
    /// list; the state warning says which and why.
    fn start(&mut self) {
        if self.started {
            return;
        }
        self.started = true;
        let config = self.read_config();
        self.config = Some(config.clone());
        let Some(path) = self.remembered.recent.first().cloned() else {
            return;
        };
        let display = self.display(&path);
        match Self::new_session(&path, config) {
            Ok(session) => {
                self.session = Some(session);
                self.slot = Some(DbFileSlot {
                    path,
                    display,
                    error: None,
                });
            }
            Err(e) => {
                let error = DbmanagerError::from(e);
                self.add_warning(&format!(
                    "The last database, {display}, could not be opened: {}",
                    error.message
                ));
                self.slot = Some(DbFileSlot {
                    path,
                    display,
                    error: Some(error),
                });
            }
        }
    }

    /// The current state. The first call loads the config and reopens the
    /// most recent database; later calls open nothing, query nothing and
    /// write nothing: they only re-read and hash the config file, so
    /// `config_changed` is current.
    pub fn session(&mut self) -> DbmanagerSnapshot {
        self.start();
        self.snapshot()
    }

    fn snapshot(&self) -> DbmanagerSnapshot {
        let config = self.config.clone().unwrap_or_else(|| self.read_config());
        let diagnostics: Vec<ConfigDiagnostic> = match &self.session {
            Some(s) => s.diagnostics().to_vec(),
            None => config.diagnostics().to_vec(),
        };
        let count = |severity| {
            diagnostics
                .iter()
                .filter(|d| d.severity == severity)
                .count()
        };
        DbmanagerSnapshot {
            database: self.slot.clone(),
            config: DbConfigView {
                path: self.config_path.clone(),
                display: self.config_path.as_deref().map(|p| self.display(p)),
                status: config.status(),
                hash: config.hash().to_owned(),
            },
            open: self.session.is_some(),
            read_only_reason: self
                .session
                .as_ref()
                .and_then(|s| s.read_only_reason())
                .map(str::to_owned),
            config_changed: config.changed_on_disk(),
            config_fail_closed: config.fail_closed_reason().map(str::to_owned),
            error_count: count(DiagnosticSeverity::Error),
            warning_count: count(DiagnosticSeverity::Warning),
            tables: self
                .session
                .as_ref()
                .map(Session::table_views)
                .unwrap_or_default(),
            backup_path: self
                .session
                .as_ref()
                .and_then(|s| s.backup_path())
                .map(Path::to_path_buf),
            recent: self
                .remembered
                .recent
                .iter()
                .map(|p| self.display(p))
                .collect(),
            diagnostics,
            state_warning: self.state_warning.clone(),
        }
    }

    /// The recent databases, most recent first. Open one by its position
    /// here, never by a display string.
    #[must_use]
    pub fn recent_paths(&self) -> Vec<PathBuf> {
        self.remembered.recent.clone()
    }

    /// Opens the database at `path` with the current config, and puts it
    /// first in the recent list. Even the same path starts a fresh session
    /// (no backup taken yet). On failure the current database stays open.
    ///
    /// # Errors
    /// `NotAbsolute`, `NotAFile`, `Busy`, or `DatabaseUnreadable`.
    pub fn open(&mut self, path: &Path) -> Result<(), DbmanagerError> {
        self.start();
        if !path.is_absolute() {
            return Err(DbmanagerError::new(
                K::NotAbsolute,
                format!("{} is not an absolute path.", path.display()),
            ));
        }
        if !path.is_file() {
            return Err(DbmanagerError::new(
                K::NotAFile,
                format!("{} is not an existing file.", path.display()),
            ));
        }
        let config = self.config.clone().unwrap_or_else(|| self.read_config());
        let session = Self::new_session(path, config).map_err(DbmanagerError::from)?;
        self.session = Some(session);
        self.slot = Some(DbFileSlot {
            path: path.to_path_buf(),
            display: self.display(path),
            error: None,
        });
        self.remembered.touch(path);
        self.persist();
        Ok(())
    }

    /// Opens a database from dropped paths: exactly one is opened like
    /// [`Core::open`].
    ///
    /// # Errors
    /// `DropAmbiguous` for any other count, `ConfigNotDroppable` for a
    /// `.toml` file (the config has one fixed location), else as `open`.
    pub fn open_dropped(&mut self, paths: &[PathBuf]) -> Result<(), DbmanagerError> {
        let [path] = paths else {
            return Err(DbmanagerError::new(
                K::DropAmbiguous,
                format!(
                    "{} items were dropped; drop one database file.",
                    paths.len()
                ),
            ));
        };
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("toml"))
        {
            return Err(DbmanagerError::new(
                K::ConfigNotDroppable,
                format!("{} is a config file, not a database.", path.display()),
            ));
        }
        self.open(path)
    }

    /// Reloads the config and, with a database held, reopens it
    /// (re-introspects) with the new config, keeping the session's backup.
    /// With no database only the config is reloaded. On failure nothing
    /// changes.
    ///
    /// # Errors
    /// The database cannot be reopened.
    pub fn reload(&mut self) -> Result<(), DbmanagerError> {
        self.start();
        let fresh = self.read_config();
        if let Some(slot) = &self.slot {
            let mut session =
                Self::new_session(&slot.path, fresh.clone()).map_err(DbmanagerError::from)?;
            if let Some((backup, announced)) = self
                .session
                .as_ref()
                .and_then(|s| Some((s.backup_path()?.to_path_buf(), s.backup_announced())))
            {
                session.adopt_backup(backup, announced);
            }
            self.session = Some(session);
            if let Some(slot) = &mut self.slot {
                slot.error = None;
            }
        }
        self.config = Some(fresh);
        Ok(())
    }

    /// Reads the config file again and applies it to the open database (the
    /// schema is not read again).
    pub fn reload_config(&mut self) {
        self.start();
        let fresh = self.read_config();
        if let Some(session) = self.session.as_mut() {
            session.set_config(fresh.clone());
        }
        self.config = Some(fresh);
    }

    /// Where Edit config opens the config, and whether this call created it.
    /// When the file is missing its directory is created and the commented
    /// starter is written with `create_new`; an existing file is never
    /// touched (losing a race to create it is fine: `created` is `false`).
    ///
    /// # Errors
    /// `Io` when the location is unknown or the file cannot be created; a
    /// permission failure says that the folder is not writable by this
    /// account.
    pub fn edit_config_target(&self) -> Result<(PathBuf, bool), DbmanagerError> {
        use std::io::{ErrorKind, Write as _};

        let io = |message: String| DbmanagerError::new(K::Io, message);
        let path = self
            .config_path
            .clone()
            .ok_or_else(|| io("The config location could not be determined.".to_owned()))?;
        if path.is_file() {
            return Ok((path, false));
        }
        let denied = |what: &str, e: &std::io::Error| {
            if e.kind() == ErrorKind::PermissionDenied {
                io(format!(
                    "This account may not create {what}. Ask the account that owns the folder to create the config file."
                ))
            } else {
                io(format!("Could not create {what}: {e}."))
            }
        };
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)
                .map_err(|e| denied(&format!("the folder {}", dir.display()), &e))?;
        }
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(STARTER_CONFIG.as_bytes())
                    .and_then(|()| file.flush())
                    .map_err(|e| denied(&format!("the config {}", path.display()), &e))?;
                Ok((path, true))
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => Ok((path, false)),
            Err(e) => Err(denied(&format!("the config {}", path.display()), &e)),
        }
    }

    /// Drops the session and forgets the database. The config stays loaded.
    pub fn close(&mut self) {
        self.start();
        self.session = None;
        self.slot = None;
    }

    fn open_session(&self) -> Result<&Session, DbmanagerError> {
        self.session
            .as_ref()
            .ok_or_else(|| DbmanagerError::new(K::NoDatabase, "no database is open"))
    }

    /// One page of `table`'s objects.
    ///
    /// # Errors
    /// `NoDatabase`, or the session's error.
    pub fn list(&self, table: &str, query: &ListQuery) -> Result<ObjectPage, DbmanagerError> {
        self.open_session()?
            .list(table, query)
            .map_err(DbmanagerError::from)
    }

    /// One object with typed values.
    ///
    /// # Errors
    /// `NoDatabase`, or the session's error.
    pub fn object(&self, table: &str, key: &ObjectKey) -> Result<ObjectView, DbmanagerError> {
        self.open_session()?
            .object(table, key)
            .map_err(DbmanagerError::from)
    }

    /// One page of the other side of a many-to-many relation.
    ///
    /// # Errors
    /// `NoDatabase`, or the session's error.
    pub fn m2m_options(
        &self,
        table: &str,
        key: Option<&ObjectKey>,
        relation: &str,
        query: &M2mQuery,
    ) -> Result<M2mPage, DbmanagerError> {
        self.open_session()?
            .m2m_options(table, key, relation, query)
            .map_err(DbmanagerError::from)
    }

    /// Saves one object's changes (or creates one) in one transaction, after
    /// the session's one backup. When the config changed on disk (the
    /// request's hash is stale, or the file differs from what was loaded) the
    /// config is reloaded **before** the error is returned, and nothing is
    /// saved; the caller checks its draft against the new rules and sends a
    /// new request with the new `config.hash`. Never retried here.
    ///
    /// # Errors
    /// `NoDatabase`, or the session's error (`ConfigChangedOnDisk`,
    /// `ValidationFailed`, `Conflict`, `SchemaChanged`, `Busy`, ...).
    pub fn save(&mut self, request: &SaveRequest) -> Result<SaveResult, DbmanagerError> {
        self.start();
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| DbmanagerError::new(K::NoDatabase, "no database is open"))?;
        match session.save(request) {
            Ok(result) => Ok(result),
            Err(e) => {
                if matches!(e, SessionError::ConfigChangedOnDisk) {
                    self.reload_config();
                }
                Err(DbmanagerError::from(e))
            }
        }
    }

    /// What deleting the object would do to every table that references it,
    /// hidden tables counted. Reads only.
    ///
    /// # Errors
    /// `NoDatabase`, or the session's error (`UnknownTable`, `UnknownObject`,
    /// `SchemaChanged`, ...).
    pub fn delete_preview(
        &self,
        table: &str,
        key: &ObjectKey,
    ) -> Result<DeleteImpact, DbmanagerError> {
        self.open_session()?
            .delete_preview(table, key)
            .map_err(DbmanagerError::from)
    }

    /// Deletes one object in one transaction, after the session's one backup,
    /// when `version` and `token` are still what the caller previewed and
    /// `config_hash` is the config it was shown under. When the config changed
    /// on disk it is reloaded **before** the error is returned, and nothing is
    /// deleted; the caller previews again. Never retried here.
    ///
    /// # Errors
    /// `NoDatabase`, or the session's error (`ConfigChangedOnDisk`,
    /// `Conflict`, `ImpactChanged`, `DeleteBlocked`, `NotEditable`, `Busy`,
    /// ...).
    pub fn delete(
        &mut self,
        table: &str,
        key: &ObjectKey,
        version: &str,
        token: &str,
        config_hash: &str,
    ) -> Result<DeleteResult, DbmanagerError> {
        self.start();
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| DbmanagerError::new(K::NoDatabase, "no database is open"))?;
        match session.delete(table, key, version, token, config_hash) {
            Ok(result) => Ok(result),
            Err(e) => {
                if matches!(e, SessionError::ConfigChangedOnDisk) {
                    self.reload_config();
                }
                Err(DbmanagerError::from(e))
            }
        }
    }

    /// Tests: how many sessions this thread has opened.
    #[cfg(test)]
    pub(super) fn session_opens() -> usize {
        OPENS.with(std::cell::Cell::get)
    }

    /// Tests: marks `backup` as the open session's backup.
    #[cfg(test)]
    pub(super) fn adopt_backup_for_tests(&mut self, backup: PathBuf) {
        if let Some(s) = self.session.as_mut() {
            s.adopt_backup(backup, true);
        }
    }

    /// Tests: the open session's `backup_announced`.
    #[cfg(test)]
    pub(super) fn backup_announced_for_tests(&self) -> bool {
        self.session.as_ref().is_some_and(Session::backup_announced)
    }
}
