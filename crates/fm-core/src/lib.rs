//! Shared, tool-agnostic library. Must never depend on tauri.

pub mod atomic;
pub mod dirs;
pub mod error;
pub mod store;

pub use atomic::atomic_write;
pub use dirs::AppDirs;
pub use error::{FmError, Result};
pub use store::{StateData, Store, StoreWarning};
