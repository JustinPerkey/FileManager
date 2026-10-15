//! Hook: step 1, check every subdirectory has its required files.

use crate::{StepError, StepInput, StepOutcome};

/// Checks that each of `input.subdirectories` holds the files it needs.
///
/// Report a missing or unusable file as an [`Issue`](crate::Issue) naming its
/// subdirectory, and keep checking the rest, so the user sees every problem at
/// once. Return [`StepError::Failed`] only when the check itself cannot go
/// on. This step must not change anything on disk.
pub fn verify(input: StepInput<'_>) -> Result<StepOutcome, StepError> {
    // TODO: decide the required files and check each subdirectory for them.
    let _ = input;
    Err(StepError::NotImplemented)
}
