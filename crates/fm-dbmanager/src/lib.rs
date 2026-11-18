//! Database Manager domain library. Must never depend on tauri.

pub mod config;
pub mod db;
pub mod rules;
pub mod schema;
pub mod state;
pub mod value;

pub use config::{
    parse as parse_config, Config, ConfigDiagnostic, ConfigReport, DiagnosticSeverity,
};
pub use db::{Database, OpenError};
pub use rules::{
    validate, AllowedValue, FieldError, FieldIssue, FieldIssueCode, FieldKind, FieldRule,
};
pub use state::{RememberedState, MAX_RECENT};
pub use value::DbValue;
