//! Pipeline domain library: the steps that run, in order, over one directory
//! the user chooses. Depends on nothing from tauri.
//!
//! Each subdirectory of the chosen directory is one unit of work. The steps,
//! in the order the shell runs them:
//!
//! 1. [`verify::verify`] checks that every subdirectory has its required files;
//! 2. [`batch::run_batch`] runs the batch file in each subdirectory and
//!    reports the ones that failed;
//! 3. [`schedule::schedule`] is the schedule step;
//! 4. [`tarpack::tarpack`] is the tar packing step.
//!
//! All four are stubs that return [`StepError::NotImplemented`] until they are
//! written. Everything around them (choosing the directory, listing its
//! subdirectories, running one step or all of them, and showing the results)
//! is already wired up in the desktop shell.

pub mod batch;
pub mod schedule;
pub mod tarpack;
pub mod verify;

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The steps, in the order they run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Step {
    Verify,
    RunBatch,
    Schedule,
    Tarpack,
}

impl Step {
    pub const ALL: [Step; 4] = [Step::Verify, Step::RunBatch, Step::Schedule, Step::Tarpack];

    /// The hook that does this step's work.
    pub fn hook(self) -> StepFn {
        match self {
            Step::Verify => verify::verify,
            Step::RunBatch => batch::run_batch,
            Step::Schedule => schedule::schedule,
            Step::Tarpack => tarpack::tarpack,
        }
    }
}

/// What every step receives.
#[derive(Clone, Copy, Debug)]
pub struct StepInput<'a> {
    /// The directory the user chose. Absolute.
    pub root: &'a Path,
    /// Its immediate subdirectories, sorted, as listed by [`subdirectories`]
    /// just before the step started.
    pub subdirectories: &'a [PathBuf],
}

/// The signature every step hook has.
pub type StepFn = fn(StepInput<'_>) -> Result<StepOutcome, StepError>;

/// A step that ran to the end. Problems it found are `issues`; an empty list
/// means the step passed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StepOutcome {
    /// One line for the UI, for example "12 folders checked".
    pub summary: String,
    pub issues: Vec<Issue>,
}

/// One problem a step found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    /// The subdirectory it concerns: one of [`StepInput::subdirectories`],
    /// not a path inside it (the UI shows only its last component). Name a
    /// file inside it in `message`. `None` for the directory as a whole.
    pub subdirectory: Option<PathBuf>,
    pub message: String,
}

/// Why a step could not run to the end. Problems found in the subdirectories
/// are [`Issue`]s in a [`StepOutcome`], not errors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepError {
    /// The step has not been written yet.
    NotImplemented,
    /// The step stopped part way. The message says why and how far it got.
    Failed(String),
}

impl fmt::Display for StepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StepError::NotImplemented => f.write_str("this step is not implemented yet"),
            StepError::Failed(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for StepError {}

/// The immediate subdirectories of `root`, sorted by path. Files, and
/// symlinks to directories, are skipped.
pub fn subdirectories(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            out.push(entry.path());
        }
    }
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subdirectories_lists_only_directories_sorted() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("b")).unwrap();
        fs::create_dir(dir.path().join("a")).unwrap();
        fs::write(dir.path().join("file.txt"), "x").unwrap();
        assert_eq!(
            subdirectories(dir.path()).unwrap(),
            [dir.path().join("a"), dir.path().join("b")]
        );
    }

    /// `Step::ALL` lists every variant, once. Adding a variant without adding
    /// it to `ALL` fails here (the match below stops compiling, or the
    /// assertion fails).
    #[test]
    fn all_lists_every_step() {
        let position = |step: Step| match step {
            Step::Verify | Step::RunBatch | Step::Schedule | Step::Tarpack => {
                Step::ALL.iter().position(|s| *s == step)
            }
        };
        for step in [Step::Verify, Step::RunBatch, Step::Schedule, Step::Tarpack] {
            assert!(
                position(step).is_some(),
                "{step:?} is missing from Step::ALL"
            );
        }
        let unique: std::collections::HashSet<Step> = Step::ALL.into_iter().collect();
        assert_eq!(unique.len(), Step::ALL.len());
    }

    #[test]
    fn every_step_is_a_stub() {
        let dir = tempfile::tempdir().unwrap();
        let input = StepInput {
            root: dir.path(),
            subdirectories: &[],
        };
        for step in Step::ALL {
            assert_eq!((step.hook())(input), Err(StepError::NotImplemented));
        }
    }
}
