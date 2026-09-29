//! Tar Packager domain library. Depends on `fm-core` only; never on tauri.

pub mod format;
pub mod manifest;

pub use format::ArchiveFormat;
pub use manifest::{
    load, parse, Diagnostic, Entry, LoadError, LoadedManifest, Manifest, ManifestView, Owner,
    Severity,
};
