//! The Pipeline's contract types: the session snapshot the UI renders and the
//! error every command returns. All are exported to `lib/generated/` (see
//! `generated_types.rs`).

use std::fmt;

use fm_pipeline::{Step, StepError};
use serde::{Deserialize, Serialize};

/// One step of the pipeline, in run order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub enum StepId {
    /// Check every subdirectory has its required files.
    Verify,
    /// Run the batch file in each subdirectory.
    RunBatch,
    /// The schedule step.
    Schedule,
    /// Pack each subdirectory into a tar archive.
    Tarpack,
}

impl From<Step> for StepId {
    fn from(s: Step) -> Self {
        match s {
            Step::Verify => StepId::Verify,
            Step::RunBatch => StepId::RunBatch,
            Step::Schedule => StepId::Schedule,
            Step::Tarpack => StepId::Tarpack,
        }
    }
}

impl From<StepId> for Step {
    fn from(s: StepId) -> Self {
        match s {
            StepId::Verify => Step::Verify,
            StepId::RunBatch => Step::RunBatch,
            StepId::Schedule => Step::Schedule,
            StepId::Tarpack => Step::Tarpack,
        }
    }
}

/// The chosen directory. `error` says why it cannot be used; the path is kept
/// so the user sees what they chose or typed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct DirectorySlot {
    /// Display string.
    pub path: String,
    /// `NotAbsolute`, `NotADirectory` or `DirectoryUnreadable`.
    pub error: Option<PipelineError>,
    /// How many subdirectories it had when it was chosen.
    pub subdirectories: u32,
}

/// One problem a step found.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct StepIssue {
    /// The subdirectory's name (its last component, for display), or `null`
    /// for the directory as a whole.
    pub subdirectory: Option<String>,
    pub message: String,
}

/// A step that ran to the end.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct StepReport {
    pub summary: String,
    pub issues: Vec<StepIssue>,
    /// No issues were found.
    pub passed: bool,
}

/// A step and its last result in this session. `report` and `error` are
/// never both set; both `null` means the step has not run since the directory
/// was chosen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct StepView {
    pub id: StepId,
    pub report: Option<StepReport>,
    pub error: Option<PipelineError>,
}

/// Everything the UI shows. Every command returns one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct PipelineSession {
    pub directory: Option<DirectorySlot>,
    /// Every step, in run order.
    pub steps: Vec<StepView>,
    /// The step running now, if any.
    pub running: Option<StepId>,
    /// A directory is chosen without an error and no step is running.
    pub can_run: bool,
}

/// What went wrong. The UI switches over it exhaustively.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
pub enum PipelineErrorKind {
    /// No directory is chosen.
    NoDirectory,
    /// The path is not a full path (it does not start at a drive or share).
    NotAbsolute,
    /// The chosen path is not a directory.
    NotADirectory,
    /// The directory's contents could not be listed.
    DirectoryUnreadable,
    /// The directory changed between the click and the run. Nothing ran.
    DirectoryChanged,
    /// A step is already running.
    StepInProgress,
    /// The step's hook is still a stub.
    StepNotImplemented,
    /// The step stopped part way; `message` says why.
    StepFailed,
}

/// The error every `pipeline_*` command returns, and a step's result when it
/// could not run to the end. `message` is technical detail; the UI shows its
/// own copy per kind.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct PipelineError {
    pub kind: PipelineErrorKind,
    pub message: String,
}

impl PipelineError {
    pub fn new(kind: PipelineErrorKind, message: impl Into<String>) -> Self {
        PipelineError {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for PipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for PipelineError {}

impl From<StepError> for PipelineError {
    fn from(e: StepError) -> Self {
        let kind = match e {
            StepError::NotImplemented => PipelineErrorKind::StepNotImplemented,
            StepError::Failed(_) => PipelineErrorKind::StepFailed,
        };
        PipelineError::new(kind, e.to_string())
    }
}
