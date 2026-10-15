//! The Pipeline's session logic as plain functions, with no Tauri types.
//!
//! A run is split so the step's work happens without the session lock held:
//! [`Core::begin`] checks the request and marks the step running,
//! [`Job::run`] lists the subdirectories and calls the step's hook (on a worker
//! thread), and [`Core::finish`] records the result. The hooks default to
//! `fm_pipeline::Step::hook`; tests swap in fakes.

use std::path::{Path, PathBuf};

use fm_pipeline::{Issue, Step, StepFn, StepInput, StepOutcome};

use super::types::{
    DirectorySlot, PipelineError, PipelineErrorKind as K, PipelineSession, StepIssue, StepReport,
    StepView,
};

/// Picks the hook for a step.
pub(super) type HookFor = fn(Step) -> StepFn;

/// The chosen directory and, if it cannot be used, why.
struct Dir {
    path: PathBuf,
    error: Option<PipelineError>,
    subdirectories: u32,
}

impl Dir {
    fn check(path: &Path) -> Dir {
        let (error, subdirectories) = match count_subdirectories(path) {
            Ok(n) => (None, n),
            Err(e) => (Some(e), 0),
        };
        Dir {
            path: path.to_path_buf(),
            error,
            subdirectories,
        }
    }
}

type StepResult = Result<StepReport, PipelineError>;

pub(super) struct Core {
    hook_for: HookFor,
    directory: Option<Dir>,
    /// The last result of each step, indexed like `Step::ALL`.
    results: [Option<StepResult>; Step::ALL.len()],
    running: Option<Step>,
}

impl Default for Core {
    fn default() -> Self {
        Core {
            hook_for: Step::hook,
            directory: None,
            results: Default::default(),
            running: None,
        }
    }
}

/// A step ready to run, holding everything it needs and no lock.
pub(super) struct Job {
    root: PathBuf,
    hook: StepFn,
}

fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn index(step: Step) -> usize {
    Step::ALL
        .iter()
        .position(|s| *s == step)
        .expect("every step is in Step::ALL")
}

fn count_subdirectories(path: &Path) -> Result<u32, PipelineError> {
    // A relative path would resolve against the process's working directory,
    // which the user never sees.
    if !path.is_absolute() {
        return Err(PipelineError::new(
            K::NotAbsolute,
            format!("{} is not a full path", path.display()),
        ));
    }
    if !path.is_dir() {
        return Err(PipelineError::new(
            K::NotADirectory,
            format!("{} is not a folder", path.display()),
        ));
    }
    Ok(count(&list(path)?))
}

fn count(subdirectories: &[PathBuf]) -> u32 {
    u32::try_from(subdirectories.len()).unwrap_or(u32::MAX)
}

fn list(path: &Path) -> Result<Vec<PathBuf>, PipelineError> {
    fm_pipeline::subdirectories(path).map_err(|e| {
        PipelineError::new(
            K::DirectoryUnreadable,
            format!("listing {}: {e}", path.display()),
        )
    })
}

fn issue_view(issue: Issue) -> StepIssue {
    StepIssue {
        subdirectory: issue.subdirectory.map(|p| match p.file_name() {
            Some(name) => name.to_string_lossy().into_owned(),
            None => display(&p),
        }),
        message: issue.message,
    }
}

fn report(outcome: StepOutcome) -> StepReport {
    StepReport {
        summary: outcome.summary,
        passed: outcome.issues.is_empty(),
        issues: outcome.issues.into_iter().map(issue_view).collect(),
    }
}

impl Job {
    /// Lists the subdirectories and runs the step. May block for a long time.
    pub fn run(self) -> StepResult {
        // Listed again now: folders may have been added since the directory
        // was chosen.
        let subdirectories = list(&self.root)?;
        let outcome = (self.hook)(StepInput {
            root: &self.root,
            subdirectories: &subdirectories,
        })?;
        Ok(report(outcome))
    }
}

impl Core {
    #[cfg(test)]
    pub fn with_hooks(hook_for: HookFor) -> Core {
        Core {
            hook_for,
            ..Core::default()
        }
    }

    pub fn snapshot(&self) -> PipelineSession {
        let usable = self.directory.as_ref().is_some_and(|d| d.error.is_none());
        PipelineSession {
            directory: self.directory.as_ref().map(|d| DirectorySlot {
                path: display(&d.path),
                error: d.error.clone(),
                subdirectories: d.subdirectories,
            }),
            steps: Step::ALL
                .iter()
                .zip(&self.results)
                .map(|(step, result)| StepView {
                    id: (*step).into(),
                    report: result.as_ref().and_then(|r| r.as_ref().ok()).cloned(),
                    error: result.as_ref().and_then(|r| r.as_ref().err()).cloned(),
                })
                .collect(),
            running: self.running.map(Into::into),
            can_run: usable && self.running.is_none(),
        }
    }

    fn not_running(&self) -> Result<(), PipelineError> {
        match self.running {
            Some(step) => Err(PipelineError::new(
                K::StepInProgress,
                format!("{step:?} is still running"),
            )),
            None => Ok(()),
        }
    }

    /// Sets or clears (`None`) the directory and forgets every step's result.
    /// It is checked to be a readable directory. Refused while a step runs.
    pub fn set_directory(&mut self, path: Option<&Path>) -> Result<(), PipelineError> {
        self.not_running()?;
        self.directory = path.map(Dir::check);
        self.results = Default::default();
        Ok(())
    }

    /// Marks `step` running and returns the work to do. `expected` is the
    /// display path of the directory the UI showed; if the session holds
    /// another, nothing runs.
    pub fn begin(&mut self, step: Step, expected: &str) -> Result<Job, PipelineError> {
        self.not_running()?;
        let dir = match &self.directory {
            None => return Err(PipelineError::new(K::NoDirectory, "no folder is chosen")),
            Some(Dir { error: Some(e), .. }) => return Err(e.clone()),
            Some(dir) => dir,
        };
        if display(&dir.path) != expected {
            return Err(PipelineError::new(
                K::DirectoryChanged,
                "the chosen folder changed before the step started",
            ));
        }
        let job = Job {
            root: dir.path.clone(),
            hook: (self.hook_for)(step),
        };
        self.running = Some(step);
        self.results[index(step)] = None;
        Ok(job)
    }

    /// Records the result of the step [`Core::begin`] started.
    pub fn finish(&mut self, step: Step, result: StepResult) {
        debug_assert_eq!(self.running, Some(step));
        self.running = None;
        // The folder may have gained or lost subdirectories during the step.
        if let Some(dir) = &mut self.directory {
            if let Ok(subdirectories) = list(&dir.path) {
                dir.subdirectories = count(&subdirectories);
            }
        }
        self.results[index(step)] = Some(result);
    }
}
