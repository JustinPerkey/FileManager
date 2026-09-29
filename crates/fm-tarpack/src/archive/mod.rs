//! The archive writer: plan, write through a format encoder, verify, persist.
//! The layout is documented in `docs/tarpack-manifest.md`.

mod encode;
mod eol;
mod header;
mod plan;
mod verify;
mod write;

pub use plan::{ArchivePlan, PlanError, PlannedEntry};
pub use write::{write_archive, BuildError, BuildPhase, BuildSummary, NormalizedEntry, Progress};

#[cfg(test)]
mod tests;
