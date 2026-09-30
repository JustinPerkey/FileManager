//! Where each manifest entry's bytes come from.

mod assignments;
mod matching;
mod remembered;

#[cfg(test)]
mod tests;

pub use assignments::{Assignments, EntryStatus};
pub use matching::{apply, match_dropped, Ambiguity, DropOutcome, Unmatched, MAX_DEPTH};
pub use remembered::{ManifestMemory, RememberedState, MAX_RECENT};
