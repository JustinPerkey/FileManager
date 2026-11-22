//! The Tar Packager's session state and operations, with no UI types. Used by
//! the Tauri shell now and the GPUI app later.

mod core;
mod progress;
mod types;

#[cfg(test)]
mod tests;

pub use self::core::{BuildJob, Core, DropJob};
pub use self::progress::{coalescing, ProgressCoalescer, PROGRESS_INTERVAL};
pub use self::types::{
    ArchiveFormatOption, BuildBlockedReason, DroppedAssignment, ManifestChanged, SessionEntry,
    SessionManifest, TarpackError, TarpackErrorKind, TarpackSession,
};
