//! Database Manager domain library. Must never depend on tauri.

pub mod db;
pub mod state;

pub use db::{Database, OpenError};
pub use state::{RememberedState, MAX_RECENT};
