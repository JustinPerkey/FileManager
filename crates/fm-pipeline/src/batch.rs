//! Hook: step 2, run the batch file in each subdirectory.

use crate::{StepError, StepInput, StepOutcome};

/// Runs the batch file in each of `input.subdirectories`, with that
/// subdirectory as the working directory.
///
/// A batch file that exits non-zero, or cannot be started, is an
/// [`Issue`](crate::Issue) naming its subdirectory, with the exit code and
/// the useful part of its output in the message. Keep going with the other
/// subdirectories. This runs on a worker thread, so it may block; the UI shows
/// the step as running until it returns.
///
/// Windows notes:
/// - `FileManager` is a GUI-subsystem exe, so give each child
///   `CREATE_NO_WINDOW` (`std::os::windows::process::CommandExt::creation_flags(0x0800_0000)`),
///   or a console window flashes up for every subdirectory.
/// - `cmd.exe` parses a `.bat` file's arguments with its own rules
///   (CVE-2024-24576): pass no arguments built from file or folder names, or
///   quote them deliberately.
///
/// # Errors
///
/// [`StepError::NotImplemented`] until written; [`StepError::Failed`] when the
/// step cannot go on.
pub fn run_batch(input: StepInput<'_>) -> Result<StepOutcome, StepError> {
    // TODO: run each subdirectory's batch file (for example with `std::process::Command` and `cmd /C`) and collect failures.
    let _ = input;
    Err(StepError::NotImplemented)
}
