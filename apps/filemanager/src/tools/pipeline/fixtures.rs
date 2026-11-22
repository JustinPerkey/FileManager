//! Fixture backends for the Pipeline: tests and the screenshot harness. They
//! never touch the disk; paths are fake display strings such as
//! `C:\Deliveries\2026-10`.
//!
//! **Timing** (the harness's `Step::WaitIdle` is a short fixed settle, not idle
//! detection). Every fixture answers `session`, `set_directory`, `begin_step`
//! and `finish_step` synchronously, so those resolve inside the settle. A step
//! job runs on its own thread: it returns at once unless the fixture holds it,
//! so a scenario that clicks Run should add a `Step::Wait` of about 200 ms for
//! it; [`PipelineFixture::Slow`] holds the job for 30 s (the screenshot of a
//! running step) and ends with the process.

use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::Duration;

use fm_pipeline::session::{
    DirectorySlot, Finished, PipelineError, PipelineErrorKind, PipelineSession, StepId, StepIssue,
    StepReport, StepView,
};

use super::backend::{PipelineBackend, StepJob};
use super::text::STEP_IDS;

/// Which screenshot state a fixture starts in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipelineFixture {
    /// `pipeline-empty`: no folder chosen.
    Empty,
    /// `pipeline-chosen` and `pipeline-runall`: a valid folder with 12
    /// subfolders, nothing run. A step ends "not implemented", like the real
    /// stubs (`pipeline-ran` after Run next; add a `Wait` of about 200 ms).
    Chosen,
    /// `pipeline-bad`: a folder with an error ("not found").
    Bad,
    /// `pipeline-ran` without clicking: the first step already ran and is not
    /// implemented.
    Ran,
    /// The manual's demo state: the first step passed, the second found two
    /// problems.
    Demo,
    /// `pipeline-running`: [`PipelineFixture::Chosen`] whose steps take 30 s.
    Slow,
}

/// A closed gate holds a step job until opened, so a test can look at the
/// model while a step runs.
#[derive(Clone, Default)]
pub struct StepGate(Arc<(Mutex<bool>, Condvar)>);

impl StepGate {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Lets every job waiting on this gate go on.
    pub fn open(&self) {
        let (open, changed) = &*self.0;
        *open.lock().unwrap_or_else(PoisonError::into_inner) = true;
        changed.notify_all();
    }

    fn wait(&self, at_most: Duration) {
        let (open, changed) = &*self.0;
        let guard = open.lock().unwrap_or_else(PoisonError::into_inner);
        let _ = changed
            .wait_timeout_while(guard, at_most, |open| !*open)
            .unwrap_or_else(PoisonError::into_inner);
    }
}

/// What a step does when it runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Behavior {
    /// Passes with this summary.
    Pass(String),
    /// Runs to the end and finds problems: `(subfolder, message)`.
    Problems(String, Vec<(Option<String>, String)>),
    /// The hook is a stub.
    NotImplemented,
    /// Stops part way with this detail.
    Fail(String),
    /// The job panics.
    Panic,
}

type Result_ = Result<StepReport, PipelineError>;

/// A scripted [`PipelineBackend`] that mirrors the real session's rules
/// (steps in order, one at a time, the folder checked at the click) without
/// the disk.
pub struct FixturePipeline {
    directory: Option<DirectorySlot>,
    results: [Option<Result_>; 4],
    running: Option<StepId>,
    subfolders: u32,
    behaviors: [Behavior; 4],
    delay: Duration,
    hold: Option<(StepGate, Duration)>,
}

fn index(step: StepId) -> usize {
    STEP_IDS.iter().position(|s| *s == step).expect("a step")
}

fn slot(path: &str, error: Option<PipelineError>, subfolders: u32) -> DirectorySlot {
    DirectorySlot {
        path: path.to_string(),
        error,
        subdirectories: subfolders,
    }
}

impl FixturePipeline {
    /// No folder; steps are "not implemented", like the real stubs.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            directory: None,
            results: Default::default(),
            running: None,
            subfolders: 12,
            behaviors: std::array::from_fn(|_| Behavior::NotImplemented),
            delay: Duration::ZERO,
            hold: None,
        }
    }

    /// A valid folder with 12 subfolders, nothing run.
    #[must_use]
    pub fn chosen() -> Self {
        let mut f = Self::empty();
        f.directory = Some(slot("C:\\Deliveries\\2026-10", None, 12));
        f
    }

    /// A folder that was not found.
    #[must_use]
    pub fn bad() -> Self {
        let mut f = Self::empty();
        let error = PipelineError::new(PipelineErrorKind::NotADirectory, "not found");
        f.directory = Some(slot("C:\\Deliveries\\2026-13", Some(error), 0));
        f
    }

    /// [`FixturePipeline::chosen`] after its first step ran and was not
    /// implemented.
    #[must_use]
    pub fn ran() -> Self {
        let mut f = Self::chosen();
        f.results[0] = Some(Err(PipelineError::new(
            PipelineErrorKind::StepNotImplemented,
            "verify is not implemented",
        )));
        f
    }

    /// The first step passed, the second found two problems.
    #[must_use]
    pub fn demo() -> Self {
        let mut f = Self::chosen();
        f.results[0] = Some(Ok(report("12 folders checked", &[])));
        f.results[1] = Some(Ok(report(
            "12 batch files run, 2 failed",
            &[
                (Some("site-07"), "build.bat exited with code 1"),
                (Some("site-11"), "build.bat was not found"),
            ],
        )));
        f
    }

    /// [`FixturePipeline::chosen`] whose steps take 30 s.
    #[must_use]
    pub fn slow() -> Self {
        Self::chosen().with_delay(Duration::from_secs(30))
    }

    /// The fixture for a screenshot state.
    #[must_use]
    pub fn from_fixture(fixture: PipelineFixture) -> Self {
        match fixture {
            PipelineFixture::Empty => Self::empty(),
            PipelineFixture::Chosen => Self::chosen(),
            PipelineFixture::Bad => Self::bad(),
            PipelineFixture::Ran => Self::ran(),
            PipelineFixture::Demo => Self::demo(),
            PipelineFixture::Slow => Self::slow(),
        }
    }

    /// What `step` does when it runs.
    #[must_use]
    pub fn with_behavior(mut self, step: StepId, behavior: Behavior) -> Self {
        self.behaviors[index(step)] = behavior;
        self
    }

    /// Every step passes with the summary "done".
    #[must_use]
    pub fn all_pass(mut self) -> Self {
        self.behaviors = std::array::from_fn(|_| Behavior::Pass("done".to_string()));
        self
    }

    /// Every job takes this long (a sleep on its thread).
    #[must_use]
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// Every job waits on `gate`, for at most `at_most`.
    #[must_use]
    pub fn with_gate(mut self, gate: &StepGate, at_most: Duration) -> Self {
        self.hold = Some((gate.clone(), at_most));
        self
    }

    /// The subfolder count a folder set later reports.
    #[must_use]
    pub fn with_subfolders(mut self, n: u32) -> Self {
        self.subfolders = n;
        self
    }

    fn passed(&self, i: usize) -> bool {
        matches!(&self.results[i], Some(Ok(r)) if r.passed)
    }

    fn next(&self) -> Option<usize> {
        (0..STEP_IDS.len()).find(|&i| !self.passed(i))
    }

    fn snapshot(&self) -> PipelineSession {
        let next = self.next();
        let usable = self.directory.as_ref().is_some_and(|d| d.error.is_none());
        PipelineSession {
            directory: self.directory.clone(),
            steps: STEP_IDS
                .iter()
                .enumerate()
                .map(|(i, id)| StepView {
                    id: *id,
                    report: self.results[i]
                        .as_ref()
                        .and_then(|r| r.as_ref().ok())
                        .cloned(),
                    error: self.results[i]
                        .as_ref()
                        .and_then(|r| r.as_ref().err())
                        .cloned(),
                    unlocked: next.is_none_or(|n| i <= n),
                })
                .collect(),
            next_step: next.map(|i| STEP_IDS[i]),
            running: self.running,
            can_run: usable && self.running.is_none(),
        }
    }
}

fn report(summary: &str, issues: &[(Option<&str>, &str)]) -> StepReport {
    StepReport {
        summary: summary.to_string(),
        passed: issues.is_empty(),
        issues: issues
            .iter()
            .map(|(sub, message)| StepIssue {
                subdirectory: sub.map(str::to_string),
                message: (*message).to_string(),
            })
            .collect(),
    }
}

/// The fixtures treat a path as absolute when it starts at a drive letter or a
/// share, as Windows does, whatever the host.
fn looks_absolute(path: &str) -> bool {
    let b = path.as_bytes();
    path.starts_with("\\\\") || (b.len() >= 3 && b[0].is_ascii_alphabetic() && &b[1..3] == b":\\")
}

impl PipelineBackend for FixturePipeline {
    fn session(&mut self) -> PipelineSession {
        self.snapshot()
    }

    fn set_directory(&mut self, path: Option<PathBuf>) -> Result<PipelineSession, PipelineError> {
        if let Some(step) = self.running {
            return Err(PipelineError::new(
                PipelineErrorKind::StepInProgress,
                format!("{step:?} is still running"),
            ));
        }
        self.directory = path.map(|p| {
            let shown = p.to_string_lossy().into_owned();
            if looks_absolute(&shown) {
                slot(&shown, None, self.subfolders)
            } else {
                let error = PipelineError::new(
                    PipelineErrorKind::NotAbsolute,
                    format!("{shown} is not a full path"),
                );
                slot(&shown, Some(error), 0)
            }
        });
        self.results = Default::default();
        Ok(self.snapshot())
    }

    fn begin_step(
        &mut self,
        step: StepId,
        expected_directory: &str,
    ) -> Result<Box<dyn StepJob>, PipelineError> {
        if let Some(running) = self.running {
            return Err(PipelineError::new(
                PipelineErrorKind::StepInProgress,
                format!("{running:?} is still running"),
            ));
        }
        let dir = match &self.directory {
            None => {
                return Err(PipelineError::new(
                    PipelineErrorKind::NoDirectory,
                    "no folder is chosen",
                ))
            }
            Some(DirectorySlot { error: Some(e), .. }) => return Err(e.clone()),
            Some(dir) => dir,
        };
        if dir.path != expected_directory {
            return Err(PipelineError::new(
                PipelineErrorKind::DirectoryChanged,
                "the chosen folder changed before the step started",
            ));
        }
        let i = index(step);
        if let Some(blocking) = (0..i).find(|&j| !self.passed(j)) {
            return Err(PipelineError::new(
                PipelineErrorKind::PreviousStepNotPassed,
                format!(
                    "{:?} cannot start until {:?} has passed",
                    step, STEP_IDS[blocking]
                ),
            ));
        }
        self.running = Some(step);
        for result in &mut self.results[i..] {
            *result = None;
        }
        Ok(Box::new(FixtureJob {
            behavior: self.behaviors[i].clone(),
            delay: self.delay,
            hold: self.hold.clone(),
        }))
    }

    fn finish_step(&mut self, step: StepId, finished: Finished) -> PipelineSession {
        self.running = None;
        self.results[index(step)] = Some(finished.result);
        self.snapshot()
    }
}

struct FixtureJob {
    behavior: Behavior,
    delay: Duration,
    hold: Option<(StepGate, Duration)>,
}

impl StepJob for FixtureJob {
    fn run(self: Box<Self>) -> Finished {
        if let Some((gate, at_most)) = &self.hold {
            gate.wait(*at_most);
        }
        if !self.delay.is_zero() {
            std::thread::sleep(self.delay);
        }
        let result = match self.behavior {
            Behavior::Pass(summary) => Ok(report(&summary, &[])),
            Behavior::Problems(summary, issues) => {
                let issues: Vec<(Option<&str>, &str)> = issues
                    .iter()
                    .map(|(s, m)| (s.as_deref(), m.as_str()))
                    .collect();
                Ok(report(&summary, &issues))
            }
            Behavior::NotImplemented => Err(PipelineError::new(
                PipelineErrorKind::StepNotImplemented,
                "this step is not implemented yet",
            )),
            Behavior::Fail(detail) => {
                Err(PipelineError::new(PipelineErrorKind::StepFailed, detail))
            }
            Behavior::Panic => panic!("the fixture step panicked"),
        };
        Finished {
            result,
            directory: None,
        }
    }
}
