//! The Schedule Creator's contract types: the session snapshot the UI renders,
//! the apply summary, and the error every command returns. All are exported to
//! `lib/generated/` (see `generated_types.rs`).

use std::fmt;

use fm_schedule::{MergeError, ParseError, SchedulePreview};
use serde::Serialize;

/// Everything the UI shows. Every command but `schedule_apply` returns one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleSession {
    /// The chosen schedule text file, as a display string.
    pub text_path: Option<String>,
    /// The chosen XML file, as a display string.
    pub xml_path: Option<String>,
    /// The parsed schedule, when the text file parsed.
    pub preview: Option<SchedulePreview>,
    /// Why the text file did not parse (`ParseFailed` or `ParseNotImplemented`).
    pub parse_error: Option<ScheduleError>,
    /// A text file parsed and an XML file is chosen.
    pub can_apply: bool,
}

/// The result of a successful `schedule_apply`. Paths are display strings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct ApplySummary {
    pub xml_path: String,
    /// The copy of the XML as it was before the update.
    pub backup_path: String,
    pub added: u32,
}

/// What went wrong. The UI switches over it exhaustively.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
pub enum ScheduleErrorKind {
    /// No schedule text file is chosen.
    NoText,
    /// No XML file is chosen.
    NoXml,
    /// The chosen path is not a file.
    NotAFile,
    /// The text file could not be read or is not UTF-8.
    TextUnreadable,
    /// The XML file could not be read or is not UTF-8.
    XmlUnreadable,
    /// The schedule text has a problem; `line` says where, when known.
    ParseFailed,
    /// The parse hook is still a stub.
    ParseNotImplemented,
    /// The schedule could not be added to the XML. Nothing was written.
    MergeFailed,
    /// The merge hook is still a stub. Nothing was written.
    MergeNotImplemented,
    /// Writing the backup or the updated XML failed.
    Io,
}

/// The error every `schedule_*` command returns. `message` is technical
/// detail; the UI shows its own copy per kind.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleError {
    pub kind: ScheduleErrorKind,
    pub message: String,
    /// 1-based line in the text file, for `ParseFailed`.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub line: Option<u32>,
}

impl ScheduleError {
    pub fn new(kind: ScheduleErrorKind, message: impl Into<String>) -> Self {
        ScheduleError {
            kind,
            message: message.into(),
            line: None,
        }
    }
}

impl fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for ScheduleError {}

impl From<ParseError> for ScheduleError {
    fn from(e: ParseError) -> Self {
        let message = e.to_string();
        match e {
            ParseError::NotImplemented => {
                ScheduleError::new(ScheduleErrorKind::ParseNotImplemented, message)
            }
            ParseError::Invalid { line, message } => ScheduleError {
                kind: ScheduleErrorKind::ParseFailed,
                message,
                line,
            },
        }
    }
}

impl From<MergeError> for ScheduleError {
    fn from(e: MergeError) -> Self {
        let kind = match e {
            MergeError::NotImplemented => ScheduleErrorKind::MergeNotImplemented,
            MergeError::InvalidXml(_) | MergeError::Conflict(_) => ScheduleErrorKind::MergeFailed,
        };
        ScheduleError::new(kind, e.to_string())
    }
}
