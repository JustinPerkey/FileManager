//! Hook: step 3, the schedule step.

use crate::{StepError, StepInput, StepOutcome};

/// The schedule step, run over each of `input.subdirectories`. The parsing
/// and merging can call into `fm-schedule` (add it to this crate's
/// dependencies).
///
/// Anything that overwrites a file must keep a backup or be otherwise
/// recoverable (see `fm_core::atomic_write`), and a failure part way through
/// must say which subdirectories were already done.
pub fn schedule(input: StepInput<'_>) -> Result<StepOutcome, StepError> {
    // TODO: do the schedule step for each subdirectory.
    let _ = input;
    Err(StepError::NotImplemented)
}
