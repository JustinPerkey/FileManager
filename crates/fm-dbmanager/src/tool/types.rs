//! The tool core's plain Rust types (not exported to TypeScript).

use std::fmt;
use std::path::PathBuf;

use crate::config::{ConfigDiagnostic, ConfigStatus};
use crate::db::OpenError;
use crate::model::TableView;
use crate::rules::FieldError;
use crate::session::{DeleteImpact, LinkError, LinkRef, ObjectView, SessionError};

/// The database the tool holds a path for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DbFileSlot {
    pub path: PathBuf,
    /// For display only; never parsed back into a path.
    pub display: String,
    /// Why it is not open, when it is not.
    pub error: Option<DbmanagerError>,
}

/// The app-wide config file as last read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DbConfigView {
    /// `None` when the location could not be determined.
    pub path: Option<PathBuf>,
    pub display: Option<String>,
    pub status: ConfigStatus,
    /// `LoadedConfig::hash`.
    pub hash: String,
}

/// Everything the UI shows about the tool, in one value.
#[derive(Clone, Debug, PartialEq)]
pub struct DbmanagerSnapshot {
    pub database: Option<DbFileSlot>,
    pub config: DbConfigView,
    /// A database is open.
    pub open: bool,
    pub read_only_reason: Option<String>,
    pub config_changed: bool,
    /// `Some` iff the config is fail-closed; never inferred from diagnostics.
    pub config_fail_closed: Option<String>,
    /// The session's with a database, the config's alone without one.
    pub diagnostics: Vec<ConfigDiagnostic>,
    pub error_count: usize,
    pub warning_count: usize,
    /// Includes pickable tables (which are not for the table list).
    pub tables: Vec<TableView>,
    pub backup_path: Option<PathBuf>,
    /// Display strings, most recent first.
    pub recent: Vec<String>,
    pub state_warning: Option<String>,
}

/// What went wrong, as the UI tells the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DbmanagerErrorKind {
    NoDatabase,
    NotAbsolute,
    NotAFile,
    DatabaseUnreadable,
    OpenerFailed,
    ConfigNotDroppable,
    DropAmbiguous,
    UnknownTable,
    UnknownObject,
    UnknownRelation,
    NotEditable,
    ValidationFailed,
    Conflict,
    SchemaChanged,
    ConfigChangedOnDisk,
    DatabaseReadOnly,
    Busy,
    BackupFailed,
    ImpactChanged,
    DeleteBlocked,
    ConstraintFailed,
    Io,
}

impl DbmanagerErrorKind {
    /// Every kind, in declaration order.
    pub const ALL: [DbmanagerErrorKind; 22] = [
        Self::NoDatabase,
        Self::NotAbsolute,
        Self::NotAFile,
        Self::DatabaseUnreadable,
        Self::OpenerFailed,
        Self::ConfigNotDroppable,
        Self::DropAmbiguous,
        Self::UnknownTable,
        Self::UnknownObject,
        Self::UnknownRelation,
        Self::NotEditable,
        Self::ValidationFailed,
        Self::Conflict,
        Self::SchemaChanged,
        Self::ConfigChangedOnDisk,
        Self::DatabaseReadOnly,
        Self::Busy,
        Self::BackupFailed,
        Self::ImpactChanged,
        Self::DeleteBlocked,
        Self::ConstraintFailed,
        Self::Io,
    ];

    /// A stable name. The match is exhaustive, so a new kind cannot be
    /// forgotten here.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::NoDatabase => "NoDatabase",
            Self::NotAbsolute => "NotAbsolute",
            Self::NotAFile => "NotAFile",
            Self::DatabaseUnreadable => "DatabaseUnreadable",
            Self::OpenerFailed => "OpenerFailed",
            Self::ConfigNotDroppable => "ConfigNotDroppable",
            Self::DropAmbiguous => "DropAmbiguous",
            Self::UnknownTable => "UnknownTable",
            Self::UnknownObject => "UnknownObject",
            Self::UnknownRelation => "UnknownRelation",
            Self::NotEditable => "NotEditable",
            Self::ValidationFailed => "ValidationFailed",
            Self::Conflict => "Conflict",
            Self::SchemaChanged => "SchemaChanged",
            Self::ConfigChangedOnDisk => "ConfigChangedOnDisk",
            Self::DatabaseReadOnly => "DatabaseReadOnly",
            Self::Busy => "Busy",
            Self::BackupFailed => "BackupFailed",
            Self::ImpactChanged => "ImpactChanged",
            Self::DeleteBlocked => "DeleteBlocked",
            Self::ConstraintFailed => "ConstraintFailed",
            Self::Io => "Io",
        }
    }
}

/// An error of the tool core.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DbmanagerError {
    pub kind: DbmanagerErrorKind,
    /// Technical detail, shown under the UI's own sentence.
    pub message: String,
    /// `ValidationFailed`: the fields.
    pub field_errors: Vec<FieldError>,
    /// `ValidationFailed`: the staged links.
    pub link_errors: Vec<LinkError>,
    /// `Conflict`: the object as it is now.
    pub current: Option<Box<ObjectView>>,
    /// `Conflict`: the link that conflicts, if it was one.
    pub conflict_link: Option<Box<LinkRef>>,
    /// `ImpactChanged` and `DeleteBlocked`: the fresh impact.
    pub impact: Option<Box<DeleteImpact>>,
}

impl DbmanagerError {
    #[must_use]
    pub fn new(kind: DbmanagerErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            field_errors: Vec::new(),
            link_errors: Vec::new(),
            current: None,
            conflict_link: None,
            impact: None,
        }
    }
}

impl fmt::Display for DbmanagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DbmanagerError {}

/// `message` followed by the sentence every failed write ends with.
fn nothing_changed(message: &str) -> String {
    let m = message.trim_end();
    let m = m.strip_suffix('.').unwrap_or(m);
    format!("{m}. Nothing was changed.")
}

impl From<OpenError> for DbmanagerError {
    fn from(e: OpenError) -> Self {
        let kind = match &e {
            OpenError::NotAbsolute(_) => DbmanagerErrorKind::NotAbsolute,
            OpenError::NotAFile(_) => DbmanagerErrorKind::NotAFile,
            OpenError::Busy(_) => DbmanagerErrorKind::Busy,
            OpenError::NotUnicode(_) | OpenError::NotADatabase(..) | OpenError::Io(..) => {
                DbmanagerErrorKind::DatabaseUnreadable
            }
        };
        DbmanagerError::new(kind, e.to_string())
    }
}

impl From<SessionError> for DbmanagerError {
    fn from(e: SessionError) -> Self {
        use DbmanagerErrorKind as K;
        let message = e.to_string();
        match e {
            SessionError::Open(open) => open.into(),
            SessionError::UnknownTable(_) => DbmanagerError::new(K::UnknownTable, message),
            SessionError::UnknownObject => DbmanagerError::new(K::UnknownObject, message),
            SessionError::UnknownRelation(_) => DbmanagerError::new(K::UnknownRelation, message),
            SessionError::SchemaChanged => DbmanagerError::new(K::SchemaChanged, message),
            SessionError::Busy => DbmanagerError::new(K::Busy, message),
            SessionError::Sqlite(_) => DbmanagerError::new(K::Io, message),
            SessionError::ReadOnlyDatabase => {
                DbmanagerError::new(K::DatabaseReadOnly, nothing_changed(&message))
            }
            SessionError::ConfigChangedOnDisk => DbmanagerError::new(
                K::ConfigChangedOnDisk,
                nothing_changed(
                    "The config was reloaded because it changed on disk; check the change and save again",
                ),
            ),
            SessionError::NotEditable { .. } => {
                DbmanagerError::new(K::NotEditable, nothing_changed(&message))
            }
            SessionError::Validation { fields, links } => DbmanagerError {
                field_errors: fields,
                link_errors: links,
                ..DbmanagerError::new(K::ValidationFailed, nothing_changed(&message))
            },
            SessionError::Conflict { current, link } => DbmanagerError {
                current: Some(current),
                conflict_link: link.map(Box::new),
                ..DbmanagerError::new(K::Conflict, nothing_changed(&message))
            },
            SessionError::BackupFailed { .. } => {
                DbmanagerError::new(K::BackupFailed, nothing_changed(&message))
            }
            SessionError::Constraint { .. } => {
                DbmanagerError::new(K::ConstraintFailed, nothing_changed(&message))
            }
            SessionError::ImpactChanged(impact) => DbmanagerError {
                impact: Some(impact),
                ..DbmanagerError::new(K::ImpactChanged, nothing_changed(&message))
            },
            SessionError::DeleteBlocked(impact) => DbmanagerError {
                impact: Some(impact),
                ..DbmanagerError::new(K::DeleteBlocked, nothing_changed(&message))
            },
        }
    }
}
