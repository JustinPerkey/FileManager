//! Fixture backends for the Schedule Creator: tests and the screenshot
//! harness. They never touch the disk; paths are fake display strings.
//!
//! **Timing** (the harness's `Step::WaitIdle` is a short fixed settle, not idle
//! detection). Every method answers synchronously, so every state a scenario
//! shows resolves inside the settle and no `Step::Wait` is needed. `apply`
//! answers at once too: by default it ends `ParseNotImplemented` (the parse
//! hook is the real stub that fails first; `schedule-notimpl`).

use std::path::{Path, PathBuf};

use fm_schedule::session::{
    ApplySummary, FileSlot, ScheduleError, ScheduleErrorKind, ScheduleSession,
};

use super::backend::ScheduleBackend;

/// Which screenshot state a fixture starts in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScheduleFixture {
    /// `schedule-empty`: no files chosen.
    Empty,
    /// `schedule-ready` and `schedule-confirm` (reached by the Add click), and
    /// `schedule-notimpl` (after Update): both files valid.
    Ready,
    /// `schedule-bad`: a relative schedule path, with its error.
    Bad,
}

pub const TEXT: &str = "C:\\Schedules\\week-44.txt";
pub const XML: &str = "C:\\Config\\schedule.xml";

/// A scripted [`ScheduleBackend`] that mirrors the real session's rules
/// (full paths, one file per slot on a drop, the confirmed paths checked)
/// without the disk.
pub struct FixtureSchedule {
    text: Option<FileSlot>,
    xml: Option<FileSlot>,
    outcome: Result<ApplySummary, ScheduleError>,
}

fn not_implemented() -> ScheduleError {
    ScheduleError::new(
        ScheduleErrorKind::ParseNotImplemented,
        "schedule parsing is not implemented yet",
    )
}

fn ok_slot(path: &str) -> FileSlot {
    FileSlot {
        path: path.to_string(),
        error: None,
    }
}

fn is_xml(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("xml"))
}

/// Any path that starts at a drive or a share is a file; anything else is
/// relative.
fn check(path: &Path) -> FileSlot {
    let shown = path.to_string_lossy().into_owned();
    let b = shown.as_bytes();
    let full = (b.len() > 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\')
        || shown.starts_with("\\\\");
    let error = (!full).then(|| {
        ScheduleError::new(
            ScheduleErrorKind::NotAbsolute,
            format!("{shown} is not a full path"),
        )
    });
    FileSlot { path: shown, error }
}

impl FixtureSchedule {
    /// No files; Add ends `ParseNotImplemented`.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            text: None,
            xml: None,
            outcome: Err(not_implemented()),
        }
    }

    /// Both files valid.
    #[must_use]
    pub fn ready() -> Self {
        Self {
            text: Some(ok_slot(TEXT)),
            xml: Some(ok_slot(XML)),
            outcome: Err(not_implemented()),
        }
    }

    /// A relative schedule path (error `NotAbsolute`) and a valid XML file.
    #[must_use]
    pub fn bad() -> Self {
        Self {
            text: Some(check(Path::new("schedules\\week-44.txt"))),
            xml: Some(ok_slot(XML)),
            outcome: Err(not_implemented()),
        }
    }

    /// The fixture a screenshot state names.
    #[must_use]
    pub fn from_fixture(fixture: ScheduleFixture) -> Self {
        match fixture {
            ScheduleFixture::Empty => Self::empty(),
            ScheduleFixture::Ready => Self::ready(),
            ScheduleFixture::Bad => Self::bad(),
        }
    }

    /// What `apply` answers when the paths match.
    #[must_use]
    pub fn with_outcome(mut self, outcome: Result<ApplySummary, ScheduleError>) -> Self {
        self.outcome = outcome;
        self
    }

    fn snapshot(&self) -> ScheduleSession {
        let ok = |s: &Option<FileSlot>| s.as_ref().is_some_and(|s| s.error.is_none());
        ScheduleSession {
            text: self.text.clone(),
            xml: self.xml.clone(),
            can_apply: ok(&self.text) && ok(&self.xml),
        }
    }
}

impl ScheduleBackend for FixtureSchedule {
    fn session(&mut self) -> ScheduleSession {
        self.snapshot()
    }

    fn set_text(&mut self, path: Option<PathBuf>) -> ScheduleSession {
        self.text = path.as_deref().map(check);
        self.snapshot()
    }

    fn set_xml(&mut self, path: Option<PathBuf>) -> ScheduleSession {
        self.xml = path.as_deref().map(check);
        self.snapshot()
    }

    fn set_dropped(&mut self, paths: Vec<PathBuf>) -> Result<ScheduleSession, ScheduleError> {
        let (xml, text): (Vec<&PathBuf>, Vec<&PathBuf>) = paths.iter().partition(|p| is_xml(p));
        if xml.len() > 1 || text.len() > 1 {
            return Err(ScheduleError::new(
                ScheduleErrorKind::DropAmbiguous,
                format!(
                    "{} XML and {} other files were dropped; drop at most one of each",
                    xml.len(),
                    text.len()
                ),
            ));
        }
        if let Some(p) = text.first() {
            self.text = Some(check(p));
        }
        if let Some(p) = xml.first() {
            self.xml = Some(check(p));
        }
        Ok(self.snapshot())
    }

    fn apply(
        &mut self,
        expected_text: &str,
        expected_xml: &str,
    ) -> Result<ApplySummary, ScheduleError> {
        let usable = |slot: &Option<FileSlot>, missing, what: &str| match slot {
            None => Err(ScheduleError::new(missing, format!("no {what} is chosen"))),
            Some(FileSlot { error: Some(e), .. }) => Err(e.clone()),
            Some(FileSlot { path, .. }) => Ok(path.clone()),
        };
        let text = usable(&self.text, ScheduleErrorKind::NoText, "schedule file")?;
        let xml = usable(&self.xml, ScheduleErrorKind::NoXml, "XML file")?;
        if text != expected_text || xml != expected_xml {
            return Err(ScheduleError::new(
                ScheduleErrorKind::FilesChanged,
                "the chosen files changed after the update was confirmed",
            ));
        }
        self.outcome.clone()
    }
}
