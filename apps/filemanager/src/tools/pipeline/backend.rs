//! The Pipeline's backend as the model sees it: a trait over
//! [`fm_pipeline::session::Core`], so the model runs against the real session
//! or a fixture. Every method may block (the real one checks the disk), so the
//! model calls them only through its [`crate::jobs::Worker`]; a step's job runs
//! on its own thread through [`crate::jobs::spawn_thread`].

use std::path::PathBuf;

use fm_pipeline::session::{
    Core, Dir, Finished, Job, PipelineError, PipelineErrorKind, PipelineSession, StepId,
};

/// What the Pipeline model needs from its session. The same four operations as
/// the Tauri commands.
pub trait PipelineBackend: Send + 'static {
    /// The current session.
    fn session(&mut self) -> PipelineSession;

    /// Sets the folder, or clears it with `None`; every step's result is
    /// forgotten. May touch the disk.
    ///
    /// # Errors
    /// `StepInProgress` while a step runs.
    fn set_directory(&mut self, path: Option<PathBuf>) -> Result<PipelineSession, PipelineError>;

    /// Marks `step` running and returns its work. `expected_directory` is the
    /// display path the UI showed; a mismatch runs nothing.
    ///
    /// # Errors
    /// A step runs, no usable folder, the folder changed, or an earlier step
    /// has not passed.
    fn begin_step(
        &mut self,
        step: StepId,
        expected_directory: &str,
    ) -> Result<Box<dyn StepJob>, PipelineError>;

    /// Records the result of the step [`PipelineBackend::begin_step`] started,
    /// and returns the new session.
    fn finish_step(&mut self, step: StepId, finished: Finished) -> PipelineSession;
}

/// A step ready to run. Runs on its own thread and may take a long time.
pub trait StepJob: Send + 'static {
    /// Runs the step to the end.
    fn run(self: Box<Self>) -> Finished;
}

/// The result recorded for a step whose job panicked: the step failed, and the
/// folder is not checked again.
#[must_use]
pub fn panicked_step(message: &str) -> Finished {
    Finished {
        result: Err(PipelineError::new(
            PipelineErrorKind::StepFailed,
            format!("the step stopped unexpectedly: {message}"),
        )),
        directory: None,
    }
}

/// The real session: [`Core`] with the real step hooks.
#[derive(Default)]
pub struct RealPipeline {
    core: Core,
}

impl RealPipeline {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

struct RealJob(Job);

impl StepJob for RealJob {
    fn run(self: Box<Self>) -> Finished {
        self.0.run()
    }
}

impl PipelineBackend for RealPipeline {
    fn session(&mut self) -> PipelineSession {
        self.core.snapshot()
    }

    fn set_directory(&mut self, path: Option<PathBuf>) -> Result<PipelineSession, PipelineError> {
        // Runs on the worker, off the UI thread: the check touches the disk.
        let directory = path.as_deref().map(Dir::check);
        self.core.set_directory(directory)?;
        Ok(self.core.snapshot())
    }

    fn begin_step(
        &mut self,
        step: StepId,
        expected_directory: &str,
    ) -> Result<Box<dyn StepJob>, PipelineError> {
        let job = self.core.begin(step.into(), expected_directory)?;
        Ok(Box::new(RealJob(job)))
    }

    fn finish_step(&mut self, step: StepId, finished: Finished) -> PipelineSession {
        self.core.finish(step.into(), finished);
        self.core.snapshot()
    }
}
