//! Tar Packager domain library. Depends on `fm-core` only; never on tauri.

pub mod archive;
pub mod format;
pub mod manifest;
pub mod sources;

pub use archive::{
    write_archive, ArchivePlan, BuildError, BuildPhase, BuildSummary, NormalizedEntry, PlanError,
    PlannedEntry, Progress,
};
pub use format::ArchiveFormat;
pub use manifest::{
    load, parse, Diagnostic, Entry, EntryFailure, LoadError, LoadedManifest, Manifest,
    ManifestView, Owner, ParseReport, Severity,
};
pub use sources::{
    apply, match_dropped, Ambiguity, Assignments, DropOutcome, EntryStatus, ManifestMemory,
    RememberedState, Unmatched,
};
