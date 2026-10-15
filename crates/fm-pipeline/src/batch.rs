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
pub fn run_batch(input: StepInput<'_>) -> Result<StepOutcome, StepError> {
    // TODO: run each subdirectory's batch file (for example with `std::process::Command` and `cmd /C`) and collect failures.
    let _ = input;
    Err(StepError::NotImplemented)
}
