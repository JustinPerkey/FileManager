//! A `Session` owns one open database and its resolved model, notices schema
//! and config changes, and answers every read the UI needs.

mod cell;
mod key;
mod label;
mod read;
mod sql;
#[cfg(test)]
mod tests;
mod types;
mod version;

use std::fmt;
use std::path::Path;

use rusqlite::ErrorCode;

use crate::config::{ConfigDiagnostic, LoadedConfig};
use crate::db::{Database, OpenError};
use crate::model::{resolve, Model, TableView};
use crate::schema::introspect;

pub use types::{
    FieldValue, LinkTarget, ListQuery, M2mOption, M2mPage, M2mQuery, ObjectKey, ObjectPage,
    ObjectSummary, ObjectView, ReferencingFilter, RelationCount, ValueProblem,
};

/// Why a session operation failed. Later milestones add variants.
#[derive(Debug)]
pub enum SessionError {
    Open(OpenError),
    UnknownTable(String),
    UnknownObject,
    UnknownRelation(String),
    /// The database's schema changed since it was read; open a new session.
    SchemaChanged,
    /// Locked by another program.
    Busy,
    Sqlite(String),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionError::Open(e) => e.fmt(f),
            SessionError::UnknownTable(t) => write!(f, "There is no table `{t}`."),
            SessionError::UnknownObject => f.write_str("That object does not exist."),
            SessionError::UnknownRelation(r) => write!(f, "There is no relation `{r}` here."),
            SessionError::SchemaChanged => {
                f.write_str("The database's structure changed; reload it.")
            }
            SessionError::Busy => f.write_str("The database is in use by another program."),
            SessionError::Sqlite(m) => write!(f, "Database error: {m}"),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<OpenError> for SessionError {
    fn from(e: OpenError) -> Self {
        SessionError::Open(e)
    }
}

impl From<rusqlite::Error> for SessionError {
    fn from(e: rusqlite::Error) -> Self {
        match &e {
            rusqlite::Error::SqliteFailure(f, _)
                if matches!(f.code, ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) =>
            {
                SessionError::Busy
            }
            _ => SessionError::Sqlite(e.to_string()),
        }
    }
}

pub struct Session {
    database: Database,
    model: Model,
    config: LoadedConfig,
}

impl Session {
    /// Opens the database and resolves its model with `config` (already
    /// loaded).
    ///
    /// # Errors
    ///
    /// The database cannot be opened or introspected.
    pub fn open(database: &Path, config: LoadedConfig) -> Result<Session, SessionError> {
        let database = Database::open(database)?;
        let schema = introspect(&database)?;
        let model = resolve(&schema, config.report());
        Ok(Session {
            database,
            model,
            config,
        })
    }

    /// Replaces the config and re-resolves the model against the schema
    /// already read. The database is not reopened.
    pub fn set_config(&mut self, config: LoadedConfig) {
        self.model = resolve(self.model.schema(), config.report());
        self.config = config;
    }

    #[must_use]
    pub fn config(&self) -> &LoadedConfig {
        &self.config
    }

    #[must_use]
    pub fn database_path(&self) -> &Path {
        self.database.path()
    }

    /// Some(short reason) iff the model was resolved fail-closed.
    #[must_use]
    pub fn config_fail_closed(&self) -> Option<&str> {
        self.config.fail_closed_reason()
    }

    #[must_use]
    pub fn read_only_reason(&self) -> Option<&str> {
        self.database.read_only_reason()
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[ConfigDiagnostic] {
        self.model.diagnostics()
    }

    #[must_use]
    pub fn table_views(&self) -> Vec<TableView> {
        self.model.table_views()
    }

    /// The config file differs from what was loaded.
    #[must_use]
    pub fn config_changed(&self) -> bool {
        self.config.changed_on_disk()
    }

    /// Runs `f` in one read transaction, after checking the schema has not
    /// changed.
    fn read<T>(
        &self,
        f: impl FnOnce(&read::Reader<'_>) -> Result<T, SessionError>,
    ) -> Result<T, SessionError> {
        let conn = self.database.conn();
        let tx = conn.unchecked_transaction()?;
        if self.database.schema_version()? != self.model.schema().schema_version {
            return Err(SessionError::SchemaChanged);
        }
        let out = f(&read::Reader {
            conn,
            model: &self.model,
        })?;
        tx.commit()?;
        Ok(out)
    }

    /// One page of `table`'s objects.
    ///
    /// # Errors
    ///
    /// Unknown table or relation, a changed schema, or a database error.
    pub fn list(&self, table: &str, query: &ListQuery) -> Result<ObjectPage, SessionError> {
        self.read(|r| r.list(table, query))
    }

    /// One object with typed values.
    ///
    /// # Errors
    ///
    /// Unknown table or object, a changed schema, or a database error.
    pub fn object(&self, table: &str, key: &ObjectKey) -> Result<ObjectView, SessionError> {
        self.read(|r| r.object(table, key))
    }

    /// One page of the other side of a many-to-many relation, with the
    /// linked objects flagged.
    ///
    /// # Errors
    ///
    /// Unknown table, object or relation, a changed schema, or a database
    /// error.
    pub fn m2m_options(
        &self,
        table: &str,
        key: Option<&ObjectKey>,
        relation: &str,
        query: &M2mQuery,
    ) -> Result<M2mPage, SessionError> {
        self.read(|r| r.m2m_options(table, key, relation, query))
    }
}
