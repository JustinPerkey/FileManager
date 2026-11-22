//! The Pipeline's copy, ported verbatim from the Tauri view
//! (`errorMessages.ts`, `steps.ts`, `PipelineView.tsx`). The views import every
//! string from here.

use fm_pipeline::session::{PipelineErrorKind, StepId};

use super::model::StepStatus;

/// User-facing copy for an error kind. The error's own `message` is technical
/// detail, shown separately.
#[must_use]
pub fn error_message(kind: PipelineErrorKind) -> &'static str {
    match kind {
        PipelineErrorKind::NoDirectory => "Choose a folder first.",
        PipelineErrorKind::NotAbsolute => {
            "Enter the full path, starting with the drive letter, for example C:\\."
        }
        PipelineErrorKind::NotADirectory => "No folder was found at this path.",
        PipelineErrorKind::DirectoryUnreadable => {
            "The folder's contents could not be listed. Check that it still exists and you can open it."
        }
        PipelineErrorKind::DirectoryChanged => {
            "The chosen folder changed before the step started. Nothing ran."
        }
        PipelineErrorKind::StepInProgress => "A step is still running. Wait for it to finish.",
        PipelineErrorKind::PreviousStepNotPassed => {
            "An earlier step has not passed yet. Run the steps in order. Nothing ran."
        }
        PipelineErrorKind::StepNotImplemented => "This step is not implemented yet.",
        PipelineErrorKind::StepFailed => "The step stopped before it finished. See Details.",
    }
}

/// Copy for an error nothing is known about (a panicked backend call).
pub const UNEXPECTED: &str = "Something went wrong. See Details.";

/// One step's copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepCopy {
    pub title: &'static str,
    pub description: &'static str,
    /// Shown in a confirmation before the step runs on its own or as part of
    /// Run all; `None` runs without asking.
    pub confirm: Option<&'static str>,
}

/// The copy of `id`.
#[must_use]
pub fn step_copy(id: StepId) -> StepCopy {
    match id {
        StepId::Verify => StepCopy {
            title: "Check required files",
            description: "Checks that every subfolder has the files it needs. Nothing is changed.",
            confirm: None,
        },
        StepId::RunBatch => StepCopy {
            title: "Run batch files",
            description: "Runs the batch file in each subfolder and reports the ones that fail.",
            confirm: Some("The batch file in each subfolder will be run."),
        },
        StepId::Schedule => StepCopy {
            title: "Schedule",
            description: "Runs the schedule step for each subfolder.",
            confirm: Some("The schedule step may update files in each subfolder."),
        },
        StepId::Tarpack => StepCopy {
            title: "Pack archives",
            description: "Packs each subfolder into a tar archive.",
            confirm: Some("An archive will be written for each subfolder."),
        },
    }
}

/// Every step, in run order.
pub const STEP_IDS: [StepId; 4] = [
    StepId::Verify,
    StepId::RunBatch,
    StepId::Schedule,
    StepId::Tarpack,
];

// Status labels.
pub const STATUS_RUNNING: &str = "Running…";
pub const STATUS_NOT_IMPLEMENTED: &str = "Not implemented";
pub const STATUS_FAILED: &str = "Failed";
pub const STATUS_PASSED: &str = "Passed";
pub const STATUS_WAITING: &str = "Waiting";
pub const STATUS_NOT_RUN: &str = "Not run";

/// "1 problem", "3 problems".
#[must_use]
pub fn problems_label(n: u32) -> String {
    format!("{n} {}", plural(n, "problem", "problems"))
}

/// The label of a step status.
#[must_use]
pub fn status_label(status: StepStatus) -> String {
    match status {
        StepStatus::Running => STATUS_RUNNING.to_string(),
        StepStatus::NotImplemented => STATUS_NOT_IMPLEMENTED.to_string(),
        StepStatus::Failed => STATUS_FAILED.to_string(),
        StepStatus::Passed => STATUS_PASSED.to_string(),
        StepStatus::Problems(n) => problems_label(n),
        StepStatus::Waiting => STATUS_WAITING.to_string(),
        StepStatus::NotRun => STATUS_NOT_RUN.to_string(),
    }
}

fn plural(n: u32, one: &'static str, many: &'static str) -> &'static str {
    if n == 1 {
        one
    } else {
        many
    }
}

// Why running is not possible.
pub const REASON_RUNNING: &str = "A step is running.";
pub const REASON_NO_FOLDER: &str = "Choose a folder.";
pub const REASON_BAD_FOLDER: &str = "Fix the folder path above first.";

// Announcements.
pub const ALL_PASSED: &str = "All steps passed.";

/// "<title>: <status>." after one step ran.
#[must_use]
pub fn step_outcome(title: &str, status_label: &str) -> String {
    format!("{title}: {status_label}.")
}

/// "Stopped at <title>." when Run all stops.
#[must_use]
pub fn stopped_at(title: &str) -> String {
    format!("Stopped at {title}.")
}

// The view's own strings.
pub const HEADER: &str = "Pipeline";
pub const INTRO: &str = "Choose a folder. Each subfolder is processed by the steps below, in order. Run one step at a time, or run them all.";
pub const LOADING: &str = "Loading";
pub const TRY_AGAIN: &str = "Try again";
pub const DETAILS: &str = "Details";
pub const FOLDER_LABEL: &str = "Folder";
pub const FOLDER_PLACEHOLDER: &str = "C:\\path\\to\\jobs";
pub const BROWSE: &str = "Browse…";
pub const BROWSE_ARIA: &str = "Browse for the folder";
pub const STEPS_LABEL: &str = "Steps";
pub const RUN_NEXT_NONE: &str = "Run next step";
pub const RUN_ALL: &str = "Run all…";
pub const RUNNING_ALL: &str = "Running all…";
pub const RUN: &str = "Run";
pub const RUN_ASKS: &str = "Run…";
pub const RUNNING: &str = "Running…";
pub const CONFIRM_RUN: &str = "Run";
pub const CANCEL: &str = "Cancel";
pub const ALL_TITLE: &str = "Run all steps?";

/// "Run next: <title>", with an ellipsis when the step asks first.
#[must_use]
pub fn run_next_label(next: StepId) -> String {
    let copy = step_copy(next);
    let ellipsis = if copy.confirm.is_some() { "…" } else { "" };
    format!("Run next: {}{ellipsis}", copy.title)
}

/// The accessible name of a step's Run button: "Run <title>".
#[must_use]
pub fn run_aria(id: StepId) -> String {
    format!("Run {}", step_copy(id).title)
}

/// The accessible name of a step's list of problems.
#[must_use]
pub fn problems_aria(id: StepId) -> String {
    format!("{} problems", step_copy(id).title)
}

/// "3 subfolders".
#[must_use]
pub fn subfolders(n: u32) -> String {
    format!("{n} {}", plural(n, "subfolder", "subfolders"))
}

/// "Runs after <title> passes." under a locked step.
#[must_use]
pub fn runs_after(previous: StepId) -> String {
    format!("Runs after {} passes.", step_copy(previous).title)
}

/// The confirmation's title for one step.
#[must_use]
pub fn confirm_title(id: StepId) -> String {
    format!("Run {}?", step_copy(id).title)
}

/// A confirmation's body in three parts, so the view can show the folder in
/// the monospace face: `before`, the `folder`, `after`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfirmBody {
    pub before: String,
    pub folder: String,
    pub after: String,
}

impl ConfirmBody {
    /// The body as one string.
    #[must_use]
    pub fn plain(&self) -> String {
        format!("{}{}{}", self.before, self.folder, self.after)
    }
}

/// The body for one step: its confirm sentence and the folder.
#[must_use]
pub fn confirm_step_body(id: StepId, folder: &str) -> ConfirmBody {
    ConfirmBody {
        before: format!("{} Folder: ", step_copy(id).confirm.unwrap_or_default()),
        folder: folder.to_string(),
        after: String::new(),
    }
}

/// The body for Run all: the order, the folder, every confirm sentence.
#[must_use]
pub fn confirm_all_body(folder: &str) -> ConfirmBody {
    let sentences: Vec<&str> = STEP_IDS
        .iter()
        .filter_map(|id| step_copy(*id).confirm)
        .collect();
    ConfirmBody {
        before: "Every step runs in order over ".to_string(),
        folder: folder.to_string(),
        after: format!(
            ", stopping at the first that does not pass. {}",
            sentences.join(" ")
        ),
    }
}
