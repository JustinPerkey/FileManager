//! The Database Manager as one tool: the app-wide config, the recent
//! databases, the open database, and the errors the UI shows. No UI types and
//! no SQL: reads and writes go through [`crate::session::Session`].
//!
//! These types never reach TypeScript, so they do not derive `ts_rs::TS`.

mod core;
#[cfg(test)]
mod tests;
mod types;

pub use self::core::Core;
pub use self::types::{
    DbConfigView, DbFileSlot, DbmanagerError, DbmanagerErrorKind, DbmanagerSnapshot,
};
