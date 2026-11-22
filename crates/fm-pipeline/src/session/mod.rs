//! The Pipeline's session state and operations, with no UI types. Used by the
//! Tauri shell now and the GPUI app later.

mod core;
mod types;

#[cfg(test)]
mod tests;

pub use self::core::{Core, Dir, Finished, HookFor, Job};
pub use self::types::{
    DirectorySlot, PipelineError, PipelineErrorKind, PipelineSession, StepId, StepIssue,
    StepReport, StepView,
};
