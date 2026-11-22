//! The Schedule Creator's model: everything the Tauri view held in React
//! state, as one entity. The view only renders it, owns focus and layout, and
//! calls the methods here.
//!
//! No backend call runs on the UI thread: each goes through the model's
//! [`Worker`]. Results come back through weak-entity updates, so a result
//! that lands after the model is gone is dropped.

use std::path::PathBuf;

use fm_schedule::session::{ApplySummary, ScheduleError, ScheduleSession};
use gpui::Context;

use crate::app::path::clean_path;
use crate::app::text::PICK_FAILED;
use crate::jobs::{JobPanicked, Worker};
use crate::platform::{platform, FileFilter, OpenSpec, PickError, PickKind};

use super::backend::ScheduleBackend;
use super::text;

/// What the error banner shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewError {
    /// The backend refused or reported an error; the copy is
    /// [`text::error_message`] and `message` is the detail.
    Schedule(ScheduleError),
    /// A backend call panicked or stopped; nothing is known about what
    /// happened. Copy: [`text::UNEXPECTED`].
    Unexpected { message: String },
    /// The file dialog could not be shown. Copy: the app's `PICK_FAILED`.
    Dialog { detail: String },
}

impl ViewError {
    /// The user-facing sentence.
    #[must_use]
    pub fn copy(&self) -> String {
        match self {
            ViewError::Schedule(e) => text::error_message(e),
            ViewError::Unexpected { .. } => text::UNEXPECTED.to_string(),
            ViewError::Dialog { .. } => PICK_FAILED.to_string(),
        }
    }

    /// The technical detail shown under "Details".
    #[must_use]
    pub fn detail(&self) -> &str {
        match self {
            ViewError::Schedule(e) => &e.message,
            ViewError::Unexpected { message } => message,
            ViewError::Dialog { detail } => detail,
        }
    }
}

impl From<JobPanicked> for ViewError {
    fn from(e: JobPanicked) -> Self {
        ViewError::Unexpected { message: e.0 }
    }
}

/// One of the two files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    /// The schedule text file.
    Text,
    /// The XML file to update.
    Xml,
}

/// Where the view moves focus after an apply ends, read once with
/// [`ScheduleModel::take_focus_hint`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusHint {
    /// The result or error banner.
    Outcome,
    /// The "Add to XML…" button (nothing to show).
    AddButton,
}

/// What a finished session command announces.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Announce {
    /// The error of this slot, if it has one.
    SlotError(Slot),
    /// The files a drop set.
    Dropped,
    Nothing,
}

type Backend = Box<dyn ScheduleBackend>;

/// The Schedule Creator's state and behaviour.
pub struct ScheduleModel {
    worker: Worker<Backend>,
    session: Option<ScheduleSession>,
    load_failed: bool,
    error: Option<ViewError>,
    error_generation: u64,
    /// Sequence number of the newest session command; older responses are
    /// discarded.
    command_seq: u64,
    applying: bool,
    /// The text and XML display paths named in the open confirmation.
    pending: Option<(String, String)>,
    result: Option<ApplySummary>,
    /// The files of the last successful apply: adding the same schedule
    /// again would duplicate entries.
    applied: Option<(String, String)>,
    announcement: Option<String>,
    announcement_generation: u64,
    focus_hint: Option<FocusHint>,
}

fn filter(name: &str, pattern: &str) -> FileFilter {
    FileFilter {
        name: name.to_string(),
        patterns: vec![pattern.to_string()],
    }
}

impl ScheduleModel {
    /// A model over `backend`; it starts loading the session at once.
    pub fn new(backend: Box<dyn ScheduleBackend>, cx: &mut Context<Self>) -> Self {
        let worker = Worker::new(backend, cx);
        let mut model = Self {
            worker,
            session: None,
            load_failed: false,
            error: None,
            error_generation: 0,
            command_seq: 0,
            applying: false,
            pending: None,
            result: None,
            applied: None,
            announcement: None,
            announcement_generation: 0,
            focus_hint: None,
        };
        model.fetch(true, cx);
        model
    }

    // ---- reading ----

    /// The session; `None` while loading or after a failed load.
    #[must_use]
    pub fn session(&self) -> Option<&ScheduleSession> {
        self.session.as_ref()
    }

    /// The first load has not answered yet.
    #[must_use]
    pub fn loading(&self) -> bool {
        self.session.is_none() && !self.load_failed
    }

    /// The first load failed; [`ScheduleModel::retry_load`] tries again.
    #[must_use]
    pub fn load_failed(&self) -> bool {
        self.load_failed
    }

    /// The error banner's content.
    #[must_use]
    pub fn error(&self) -> Option<&ViewError> {
        self.error.as_ref()
    }

    /// Increments with every new error, so the view re-shows a banner for a
    /// repeated one.
    #[must_use]
    pub fn error_generation(&self) -> u64 {
        self.error_generation
    }

    /// Why Add is disabled, if so (see [`text`]'s `REASON_*`).
    #[must_use]
    pub fn apply_reason(&self) -> Option<&'static str> {
        text::apply_reason(self.session.as_ref()?)
    }

    /// Add may be asked for.
    #[must_use]
    pub fn can_apply(&self) -> bool {
        self.session.as_ref().is_some_and(|s| s.can_apply) && !self.locked()
    }

    /// An update is running.
    #[must_use]
    pub fn applying(&self) -> bool {
        self.applying
    }

    /// The text and XML display paths the open confirmation names.
    #[must_use]
    pub fn pending(&self) -> Option<(&str, &str)> {
        self.pending.as_ref().map(|(t, x)| (t.as_str(), x.as_str()))
    }

    /// The latest successful update.
    #[must_use]
    pub fn result(&self) -> Option<&ApplySummary> {
        self.result.as_ref()
    }

    /// The current pair of files is the one last applied: the confirmation
    /// warns of duplicate entries.
    #[must_use]
    pub fn already_applied(&self) -> bool {
        match (&self.applied, self.session.as_ref()) {
            (Some((t, x)), Some(s)) => {
                s.text.as_ref().is_some_and(|f| &f.path == t)
                    && s.xml.as_ref().is_some_and(|f| &f.path == x)
            }
            _ => false,
        }
    }

    /// The latest outcome sentence, for a live region.
    #[must_use]
    pub fn announcement(&self) -> Option<&str> {
        self.announcement.as_deref()
    }

    /// Increments with every announcement, so a repeated sentence is
    /// announced again.
    #[must_use]
    pub fn announcement_generation(&self) -> u64 {
        self.announcement_generation
    }

    /// Where focus should go after the latest apply; cleared by reading.
    pub fn take_focus_hint(&mut self) -> Option<FocusHint> {
        self.focus_hint.take()
    }

    /// An update runs or is being confirmed: the files cannot change.
    fn locked(&self) -> bool {
        self.applying || self.pending.is_some()
    }

    /// Whether the window's drop is taken now (see the drop hook).
    pub(super) fn drop_ready(&self) -> bool {
        self.session.is_some() && !self.locked()
    }

    // ---- loading ----

    /// The tool became the current one: forgets the banner, the open
    /// confirmation, the result, the announcement, any focus hint and the
    /// last applied pair, and refreshes the session. An update in progress
    /// continues and lands its result here.
    pub fn activated(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        self.pending = None;
        self.result = None;
        self.applied = None;
        self.announcement = None;
        self.focus_hint = None;
        self.fetch(true, cx);
        cx.notify();
    }

    /// Fetches the session again after a failed load.
    pub fn retry_load(&mut self, cx: &mut Context<Self>) {
        self.load_failed = false;
        self.fetch(true, cx);
        cx.notify();
    }

    /// Hides the error banner.
    pub fn dismiss_error(&mut self, cx: &mut Context<Self>) {
        if self.error.take().is_some() {
            cx.notify();
        }
    }

    /// Fetches the session; a success clears the banner only when
    /// `clear_error`.
    fn fetch(&mut self, clear_error: bool, cx: &mut Context<Self>) {
        let task = self.worker.run(|b| b.session(), cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |m, cx| match result {
                Ok(session) => {
                    m.session = Some(session);
                    m.load_failed = false;
                    if clear_error {
                        m.error = None;
                    }
                    cx.notify();
                }
                Err(e) => {
                    m.load_failed = m.session.is_none();
                    m.fail(e.into(), cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn fail(&mut self, error: ViewError, cx: &mut Context<Self>) {
        self.error = Some(error);
        self.error_generation += 1;
        cx.notify();
    }

    fn announce(&mut self, sentence: String, cx: &mut Context<Self>) {
        self.announcement = Some(sentence);
        self.announcement_generation += 1;
        cx.notify();
    }

    // ---- the files ----

    fn current_path(&self, slot: Slot) -> Option<&str> {
        let s = self.session.as_ref()?;
        match slot {
            Slot::Text => s.text.as_ref(),
            Slot::Xml => s.xml.as_ref(),
        }
        .map(|f| f.path.as_str())
    }

    /// A typed or pasted path: cleaned with [`clean_path`], then set (empty
    /// clears). Does nothing when it is already the file, or while an update
    /// runs or is being confirmed.
    pub fn set_typed(&mut self, slot: Slot, typed: &str, cx: &mut Context<Self>) {
        if self.locked() {
            return;
        }
        let path = clean_path(typed);
        let unchanged = match (&path, self.current_path(slot)) {
            (None, None) => true,
            (Some(p), Some(c)) => p.to_str() == Some(c),
            _ => false,
        };
        if !unchanged {
            self.choose(slot, path, Announce::SlotError(slot), cx);
        }
    }

    /// Asks the platform for a file (filtered by the slot's kind, starting in
    /// the folder of its current file), and sets it. A cancel changes
    /// nothing; a dialog failure is the banner.
    pub fn browse(&mut self, slot: Slot, cx: &mut Context<Self>) {
        if self.locked() {
            return;
        }
        let start_dir = self
            .current_path(slot)
            .and_then(|p| PathBuf::from(p).parent().map(PathBuf::from))
            .filter(|d| !d.as_os_str().is_empty());
        let kind = match slot {
            Slot::Text => filter("Text", "*.txt"),
            Slot::Xml => filter("XML", "*.xml"),
        };
        let spec = OpenSpec {
            kind: PickKind::File,
            filters: vec![kind, filter("All files", "*.*")],
            start_dir,
            title: None,
        };
        let pick = platform(cx).pick(spec, cx);
        cx.spawn(async move |this, cx| {
            let result = pick.await;
            this.update(cx, |m, cx| match result {
                Ok(Some(path)) => m.choose(slot, Some(path), Announce::Nothing, cx),
                Ok(None) => {}
                Err(PickError::Failed { detail }) => m.fail(ViewError::Dialog { detail }, cx),
            })
            .ok();
        })
        .detach();
    }

    /// Files dropped on the window. Ignored while an update runs or is being
    /// confirmed. Announces the files now chosen.
    pub fn set_dropped(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if !self.drop_ready() {
            return;
        }
        self.command(move |b| b.set_dropped(paths), Announce::Dropped, cx);
    }

    fn choose(&mut self, slot: Slot, path: Option<PathBuf>, say: Announce, cx: &mut Context<Self>) {
        self.command(
            move |b| {
                Ok(match slot {
                    Slot::Text => b.set_text(path),
                    Slot::Xml => b.set_xml(path),
                })
            },
            say,
            cx,
        );
    }

    /// Runs a session command. Only the newest command's response is shown:
    /// an older one arriving late would show files the backend no longer
    /// holds.
    fn command(
        &mut self,
        f: impl FnOnce(&mut Backend) -> Result<ScheduleSession, ScheduleError> + Send + 'static,
        say: Announce,
        cx: &mut Context<Self>,
    ) {
        self.command_seq += 1;
        let seq = self.command_seq;
        let task = self.worker.run(f, cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |m, cx| m.command_done(seq, result, say, cx))
                .ok();
        })
        .detach();
    }

    fn command_done(
        &mut self,
        seq: u64,
        result: Result<Result<ScheduleSession, ScheduleError>, JobPanicked>,
        say: Announce,
        cx: &mut Context<Self>,
    ) {
        if seq != self.command_seq {
            return;
        }
        match result {
            Ok(Ok(session)) => {
                // A field already holding focus does not announce its new
                // error, so say it.
                let sentence = match say {
                    Announce::SlotError(Slot::Text) => session
                        .text
                        .as_ref()
                        .and_then(|f| f.error.as_ref())
                        .map(text::error_message),
                    Announce::SlotError(Slot::Xml) => session
                        .xml
                        .as_ref()
                        .and_then(|f| f.error.as_ref())
                        .map(text::error_message),
                    Announce::Dropped => Some(text::drop_sentence(&session)),
                    Announce::Nothing => None,
                };
                self.session = Some(session);
                self.error = None;
                self.result = None;
                cx.notify();
                if let Some(sentence) = sentence {
                    self.announce(sentence, cx);
                }
            }
            Ok(Err(e)) => self.fail(ViewError::Schedule(e), cx),
            Err(e) => {
                // The backend's state is unknown: fetch it again, keep the
                // banner.
                self.fail(e.into(), cx);
                self.fetch(false, cx);
            }
        }
    }

    // ---- applying ----

    /// Opens the confirmation naming both files. Does nothing unless
    /// [`ScheduleModel::can_apply`].
    pub fn request_apply(&mut self, cx: &mut Context<Self>) {
        // A hint left over from an earlier apply must not fire on this one.
        self.focus_hint = None;
        if !self.can_apply() {
            return;
        }
        let paths = self
            .session
            .as_ref()
            .and_then(|s| Some((s.text.as_ref()?.path.clone(), s.xml.as_ref()?.path.clone())));
        if let Some(paths) = paths {
            self.pending = Some(paths);
            cx.notify();
        }
    }

    /// Updates exactly the files the open confirmation named.
    pub fn confirm(&mut self, cx: &mut Context<Self>) {
        let Some(pair) = self.pending.take() else {
            return;
        };
        self.applying = true;
        self.result = None;
        self.error = None;
        // A session command issued before this one must not land on it.
        self.command_seq += 1;
        cx.notify();
        let sent = pair.clone();
        let task = self.worker.run(move |b| b.apply(&sent.0, &sent.1), cx);
        cx.spawn(async move |this, cx| {
            let outcome = task.await;
            this.update(cx, |m, cx| m.apply_done(pair, outcome, cx))
                .ok();
        })
        .detach();
    }

    fn apply_done(
        &mut self,
        pair: (String, String),
        outcome: Result<Result<ApplySummary, ScheduleError>, JobPanicked>,
        cx: &mut Context<Self>,
    ) {
        self.applying = false;
        match outcome {
            Ok(Ok(summary)) => {
                self.announce(text::result_sentence(&summary), cx);
                self.result = Some(summary);
                self.applied = Some(pair);
            }
            // The banner is an alert, so a failure is not also announced.
            Ok(Err(e)) => self.fail(ViewError::Schedule(e), cx),
            Err(e) => {
                self.fail(e.into(), cx);
                self.fetch(false, cx);
            }
        }
        self.focus_hint = Some(if self.result.is_some() || self.error.is_some() {
            FocusHint::Outcome
        } else {
            FocusHint::AddButton
        });
        cx.notify();
    }

    /// Closes the confirmation; nothing is written.
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.pending.take().is_some() {
            cx.notify();
        }
    }
}
