//! The Schedule Creator's session state and operations, with no UI types. Used
//! by the Tauri shell now and the GPUI app later.

mod core;
mod types;

#[cfg(test)]
mod tests;

pub use self::core::{Core, Hooks, MergeFn, ParseFn};
pub use self::types::{ApplySummary, FileSlot, ScheduleError, ScheduleErrorKind, ScheduleSession};
