//! Hook: step 4, pack each subdirectory into a tar archive.

use crate::{StepError, StepInput, StepOutcome};

/// The tar packing step, run over each of `input.subdirectories`. The
/// archive writing can call into `fm-tarpack` (add it to this crate's
/// dependencies).
///
/// Never replace an existing archive silently, and report which
/// subdirectories were packed if this stops part way.
///
/// # Errors
///
/// [`StepError::NotImplemented`] until written; [`StepError::Failed`] when the
/// step cannot go on.
pub fn tarpack(input: StepInput<'_>) -> Result<StepOutcome, StepError> {
    // TODO: build the archive(s) for each subdirectory.
    let _ = input;
    Err(StepError::NotImplemented)
}
