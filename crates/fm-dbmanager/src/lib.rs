//! Database Manager domain library. Must never depend on tauri.

pub mod config;
pub mod db;
pub mod model;
pub mod rules;
pub mod schema;
pub mod session;
pub mod state;
pub mod value;

pub use config::{
    parse as parse_config, Config, ConfigDiagnostic, ConfigReport, ConfigStatus,
    DiagnosticSeverity, LoadedConfig,
};
pub use db::{Database, OpenError};
pub use model::{resolve, Model, TableView};
pub use rules::{
    validate, AllowedValue, FieldError, FieldIssue, FieldIssueCode, FieldKind, FieldRule,
};
pub use session::{Session, SessionError};
pub use state::{RememberedState, MAX_RECENT};
pub use value::DbValue;
