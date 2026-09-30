//! The Tar Packager's contract types: the session snapshot the UI renders, the
//! error every command returns, and the event payloads. All are exported to
//! `lib/generated/` (see `generated_types.rs`).

use std::fmt;

use fm_tarpack::format::ArchiveFormat;
use fm_tarpack::manifest::{Diagnostic, EntryFailure, EntryView};
use fm_tarpack::sources::EntryStatus;
use serde::Serialize;

/// One entry that passed validation, with where its bytes come from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionEntry {
    #[serde(flatten)]
    #[ts(flatten)]
    pub view: EntryView,
    /// The assigned Windows file, as a display string.
    pub assigned: Option<String>,
    pub status: EntryStatus,
}

/// The loaded manifest, as the UI sees it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionManifest {
    /// Display string.
    pub path: String,
    /// The manifest's name, or the file stem when the name is in error.
    pub name: String,
    pub output_name: Option<String>,
    /// SHA-256 of the loaded file bytes, hex.
    pub hash: String,
    /// Passed entries only, in manifest order.
    pub entries: Vec<SessionEntry>,
    /// A manifest-level error hides every entry.
    pub entries_withheld: bool,
    /// Manifest-level errors (outside every `[[file]]` table).
    pub errors: Vec<Diagnostic>,
    /// One per `[[file]]` table with errors, in manifest order.
    pub failed_entries: Vec<EntryFailure>,
    /// `errors.len()` plus every failed entry's errors.
    pub error_count: u32,
    pub warnings: Vec<Diagnostic>,
}

/// One archive format, with the extension forms the UI needs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveFormatOption {
    pub format: ArchiveFormat,
    /// For example `.tar.zst`.
    pub extension: String,
    /// The last suffix without its dot, for a Save-dialog filter: `zst`.
    pub filter_extension: String,
}

impl ArchiveFormatOption {
    pub fn all() -> Vec<ArchiveFormatOption> {
        ArchiveFormat::ALL
            .iter()
            .map(|f| ArchiveFormatOption {
                format: *f,
                extension: f.extension().to_owned(),
                filter_extension: f.filter_extension().to_owned(),
            })
            .collect()
    }
}

/// The first failing condition of a build, in this order. Errors in the
/// manifest never block a build.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub enum BuildBlockedReason {
    NoManifest,
    NoEntries,
    EntriesNotReady,
    NoOutput,
}

/// The whole session state. Every command returns a fresh snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct TarpackSession {
    pub manifest: Option<SessionManifest>,
    /// Display string. Always ends with the extension of `format`.
    pub output_path: Option<String>,
    pub format: ArchiveFormat,
    /// All four formats, in `ArchiveFormat::ALL` order.
    pub formats: Vec<ArchiveFormatOption>,
    pub suggested_output_name: Option<String>,
    pub ready_count: u32,
    pub total_count: u32,
    pub can_build: bool,
    pub build_blocked_reason: Option<BuildBlockedReason>,
    pub state_warning: Option<String>,
}

/// Result of `tarpack_assign_dropped`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct DroppedAssignment {
    pub session: TarpackSession,
    pub outcome: fm_tarpack::sources::DropOutcome,
}

/// Payload of `tarpack://manifest-changed`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct ManifestChanged {
    /// Display string.
    pub path: String,
}

/// Every error a command can return. A closed set: the UI switches over it
/// exhaustively, so adding, removing or renaming a variant is a UI contract
/// change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
pub enum TarpackErrorKind {
    NoManifest,
    ManifestUnreadable,
    NoEntries,
    ManifestChangedOnDisk,
    UnknownEntry,
    NotAFile,
    NoOutput,
    EntriesNotReady,
    OutputExists,
    PathExists,
    SourceMissing,
    SourceUnreadable,
    SourceChanged,
    VerifyFailed,
    BuildInProgress,
    OpenerFailed,
    Io,
}

/// `{ kind, message, entryId? }`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct TarpackError {
    pub kind: TarpackErrorKind,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub entry_id: Option<String>,
}

impl TarpackError {
    pub fn new(kind: TarpackErrorKind, message: impl Into<String>) -> Self {
        TarpackError {
            kind,
            message: message.into(),
            entry_id: None,
        }
    }

    pub fn for_entry(
        kind: TarpackErrorKind,
        message: impl Into<String>,
        id: impl Into<String>,
    ) -> Self {
        TarpackError {
            kind,
            message: message.into(),
            entry_id: Some(id.into()),
        }
    }
}

impl fmt::Display for TarpackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for TarpackError {}

/// The one place `BuildError`, `PlanError` and `FmError` become error kinds.
impl From<fm_tarpack::BuildError> for TarpackError {
    fn from(e: fm_tarpack::BuildError) -> Self {
        use fm_tarpack::BuildError as B;
        use TarpackErrorKind as K;
        let message = e.to_string();
        match e {
            B::OutputExists { .. } => TarpackError::new(K::OutputExists, message),
            B::SourceMissing { id, .. } => TarpackError::for_entry(K::SourceMissing, message, id),
            B::SourceUnreadable { id, .. } => {
                TarpackError::for_entry(K::SourceUnreadable, message, id)
            }
            B::SourceChanged { id } => TarpackError::for_entry(K::SourceChanged, message, id),
            B::Io { id: Some(id), .. } => TarpackError::for_entry(K::Io, message, id),
            B::Io { id: None, .. } => TarpackError::new(K::Io, message),
            B::VerifyFailed(_) => TarpackError::new(K::VerifyFailed, message),
        }
    }
}

impl From<fm_tarpack::PlanError> for TarpackError {
    fn from(e: fm_tarpack::PlanError) -> Self {
        use fm_tarpack::PlanError as P;
        let message = e.to_string();
        match e {
            P::NoEntries => TarpackError::new(TarpackErrorKind::NoEntries, message),
            P::Unassigned(ids) => match ids.into_iter().next() {
                Some(id) => TarpackError::for_entry(TarpackErrorKind::EntriesNotReady, message, id),
                None => TarpackError::new(TarpackErrorKind::EntriesNotReady, message),
            },
        }
    }
}

impl From<fm_core::FmError> for TarpackError {
    fn from(e: fm_core::FmError) -> Self {
        TarpackError::new(TarpackErrorKind::Io, e.to_string())
    }
}
