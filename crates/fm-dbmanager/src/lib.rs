//! Database Manager domain library. Must never depend on tauri.

pub mod config;
pub mod db;
pub mod schema;
pub mod state;

pub use config::{
    parse as parse_config, Config, ConfigDiagnostic, ConfigReport, DiagnosticSeverity,
};
pub use db::{Database, OpenError};
pub use state::{RememberedState, MAX_RECENT};
