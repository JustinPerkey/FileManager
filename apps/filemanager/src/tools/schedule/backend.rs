//! The Schedule Creator's backend as the model sees it: a trait over
//! [`fm_schedule::session::Core`], so the model runs against the real session
//! or a fixture. Every method may block (the real one reads files), so the
//! model calls them only through its [`crate::jobs::Worker`].

use std::path::PathBuf;

use fm_schedule::session::{ApplySummary, Core, Hooks, ScheduleError, ScheduleSession};

/// What the Schedule model needs from its session. The same five operations as
/// the Tauri commands.
pub trait ScheduleBackend: Send + 'static {
    /// The current session.
    fn session(&mut self) -> ScheduleSession;

    /// Sets or clears (`None`) the schedule text file. A problem with the file
    /// is in the returned slot, not an error.
    fn set_text(&mut self, path: Option<PathBuf>) -> ScheduleSession;

    /// Sets or clears (`None`) the XML file.
    fn set_xml(&mut self, path: Option<PathBuf>) -> ScheduleSession;

    /// Sets the slots from dropped paths: a `.xml` file is the XML file, any
    /// other the schedule file.
    ///
    /// # Errors
    /// `DropAmbiguous` when more than one path is dropped for a slot.
    fn set_dropped(&mut self, paths: Vec<PathBuf>) -> Result<ScheduleSession, ScheduleError>;

    /// Adds the schedule to the XML file, after saving a backup of it.
    /// `expected_text` and `expected_xml` are the display paths the user
    /// confirmed; a mismatch writes nothing.
    ///
    /// # Errors
    /// Any [`ScheduleError`]; every kind's copy says whether the XML changed.
    fn apply(
        &mut self,
        expected_text: &str,
        expected_xml: &str,
    ) -> Result<ApplySummary, ScheduleError>;
}

/// The real session: [`Core`] with its hooks (the stubs, until they are
/// written).
#[derive(Default)]
pub struct RealSchedule {
    core: Core,
}

impl RealSchedule {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A session with other parse and merge hooks (tests).
    #[must_use]
    pub fn with_hooks(hooks: Hooks) -> Self {
        Self {
            core: Core::with_hooks(hooks),
        }
    }
}

impl ScheduleBackend for RealSchedule {
    fn session(&mut self) -> ScheduleSession {
        self.core.snapshot()
    }

    fn set_text(&mut self, path: Option<PathBuf>) -> ScheduleSession {
        self.core.set_text(path.as_deref());
        self.core.snapshot()
    }

    fn set_xml(&mut self, path: Option<PathBuf>) -> ScheduleSession {
        self.core.set_xml(path.as_deref());
        self.core.snapshot()
    }

    fn set_dropped(&mut self, paths: Vec<PathBuf>) -> Result<ScheduleSession, ScheduleError> {
        self.core.set_dropped(&paths)?;
        Ok(self.core.snapshot())
    }

    fn apply(
        &mut self,
        expected_text: &str,
        expected_xml: &str,
    ) -> Result<ApplySummary, ScheduleError> {
        self.core.apply(expected_text, expected_xml)
    }
}
