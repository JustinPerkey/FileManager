//! The Tar Packager's model: everything the Tauri view held in React state for
//! loading, opening, reloading, the "changed on disk" watcher, the manifest's
//! errors, assigning entries by browse, clear and drop, and the build side
//! (the output and format, building with progress, the confirmed overwrite,
//! the result and its report, reveal and copy, the shortcuts), as one entity.
//! The view only renders it, owns focus and layout, and calls the methods
//! here.
//!
//! No backend call runs on the UI thread: each goes through the model's
//! [`Worker`], and a drop's folder walk and a build's write run on threads of
//! their own ([`spawn_thread`]). Results come back through weak-entity updates, so a
//! result that lands after the model is gone is dropped. For commands that
//! return a session only the newest request applies: an older response
//! arriving late would show a state the backend no longer holds.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::time::Duration;

use fm_core::watch::FileWatcher;
use fm_tarpack::archive::{BuildPhase, BuildSummary, Progress};
use fm_tarpack::format::ArchiveFormat;
use fm_tarpack::session::{
    coalescing, DroppedAssignment, SessionEntry, TarpackError, TarpackErrorKind, TarpackSession,
};
use fm_tarpack::sources::DropOutcome;
use gpui::{App, AsyncApp, Context, Task, WeakEntity, Window};

use crate::app::dev_mode::DevMode;
use crate::app::text::PICK_FAILED;
use crate::jobs::{spawn_thread, JobPanicked, Worker};
use crate::platform::{platform, FileFilter, OpenError, OpenSpec, PickError, PickKind, SaveSpec};

use super::backend::{BuildWork, TarpackBackend};
use super::text::{self, DropBlocker, DropLine};

/// How long loading lasts before the skeleton shows.
pub const SKELETON_DELAY: Duration = Duration::from_millis(150);

/// How long a drop is matched before its "matching" line shows.
pub const PENDING_DELAY: Duration = Duration::from_millis(150);

/// How long a drop is matched before the model announces it.
pub const PENDING_ANNOUNCE_DELAY: Duration = Duration::from_millis(1000);

/// A drop whose session changed while its files were matched is tried this
/// many times in all.
pub const DROP_ATTEMPTS: usize = 3;

/// What caused a session to be applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// The first load, or a manifest opened (dialog, recent, example).
    Load,
    /// The loaded manifest was read again.
    Reload,
    /// Anything else: an assignment, a drop, a refresh.
    Other,
}

/// What the error banner shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewError {
    /// The backend refused or reported an error. The copy is
    /// [`text::error_message`] of its kind, naming the entry's file when
    /// the error names an entry (`source`); `error.message` is the detail.
    Tarpack {
        error: TarpackError,
        source: Option<String>,
    },
    /// A backend call panicked or stopped; nothing is known about what
    /// happened. Copy: [`text::UNEXPECTED`].
    Unexpected { message: String },
    /// The file dialog could not be shown. Copy: the app's `PICK_FAILED`.
    Dialog { detail: String },
    /// A recent manifest was chosen from a list that had changed meanwhile;
    /// nothing was opened and the list was refreshed. Copy:
    /// [`text::RECENT_CHANGED`]. It has no detail.
    RecentChanged,
}

impl ViewError {
    /// The user-facing sentence.
    #[must_use]
    pub fn copy(&self) -> String {
        match self {
            ViewError::Tarpack { error, source } => {
                text::error_message(error.kind, source.as_deref())
            }
            ViewError::Unexpected { .. } => text::UNEXPECTED.to_string(),
            ViewError::Dialog { .. } => PICK_FAILED.to_string(),
            ViewError::RecentChanged => text::RECENT_CHANGED.to_string(),
        }
    }

    /// The technical detail shown under "Details"; empty when there is none.
    #[must_use]
    pub fn detail(&self) -> &str {
        match self {
            ViewError::Tarpack { error, .. } => &error.message,
            ViewError::Unexpected { message } => message,
            ViewError::Dialog { detail } => detail,
            ViewError::RecentChanged => "",
        }
    }
}

impl From<JobPanicked> for ViewError {
    fn from(e: JobPanicked) -> Self {
        ViewError::Unexpected { message: e.0 }
    }
}

impl From<OpenError> for ViewError {
    /// The system could not open a file or show it: the `OpenerFailed` copy,
    /// with what the system said as the detail.
    fn from(e: OpenError) -> Self {
        ViewError::Tarpack {
            error: TarpackError::new(TarpackErrorKind::OpenerFailed, e.detail),
            source: None,
        }
    }
}

impl From<TarpackError> for ViewError {
    fn from(error: TarpackError) -> Self {
        ViewError::Tarpack {
            error,
            source: None,
        }
    }
}

/// Where the view moves focus, read once with
/// [`TarpackModel::take_focus_hint`]. The model does not know where focus is:
/// the view acts on a hint only when focus sits where the hint says it would
/// be lost.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusHint {
    /// The errors report ([`TarpackModel::show_errors`]).
    Report,
    /// The Reload button: a reload removed a result, and focus inside it
    /// would be lost.
    Reload,
    /// The first control after the drop result: the result was dismissed or
    /// a new drop replaced it, and focus inside it would be lost.
    AfterResult,
    /// The Create archive button: a build ended, or its result was
    /// dismissed, and focus inside the result or on the button (disabled
    /// while the build ran) would be lost. Act only if focus was lost.
    CreateArchive,
    /// The Create archive button, whatever has focus: the overwrite
    /// confirmation was cancelled and the dialog's closing leaves focus
    /// nowhere useful.
    CreateArchiveAfterDialog,
}

/// What a build came to, as the result shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[expect(
    clippy::large_enum_variant,
    reason = "the summary is the result; it is held once, never copied in a loop"
)]
pub enum BuildOutcome {
    /// The archive was written and verified.
    Ok(BuildSummary),
    /// The build failed. Its sentence is [`text::error_copy`], with the
    /// error's message as the detail.
    Err(TarpackError),
    /// The build job panicked. Its sentence is [`text::UNEXPECTED`], with the
    /// panic message as the detail.
    Unexpected {
        /// The panic message.
        detail: String,
    },
}

/// A key the Tar Packager acts on (`useTarpackShortcuts.ts`). The view maps
/// the chords to these and passes fresh presses only (no auto-repeat).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shortcut {
    /// Ctrl+O.
    Open,
    /// F5 or Ctrl+R.
    Reload,
    /// Ctrl+E.
    Edit,
    /// Ctrl+Enter.
    Build,
    /// F8.
    ShowErrors,
    /// Escape.
    Escape,
}

/// What the last drop did, with the entries it was matched against.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DropReport {
    pub outcome: DropOutcome,
    /// The entries of the session the drop produced.
    pub entries: Vec<SessionEntry>,
    /// Kept as it was when the result appeared, whatever the session does
    /// later.
    pub has_failed_entries: bool,
    /// The result's lines ([`text::drop_lines`]).
    pub lines: Vec<DropLine>,
}

/// A drop's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DropState {
    /// Files are being matched.
    Pending,
    Result(DropReport),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Load {
    Loading,
    Loaded,
    Failed,
}

/// Which delayed pieces of the view have come due.
#[derive(Default)]
struct Shown {
    skeleton: bool,
    pending: bool,
}

/// What starting a watcher on the Worker came to.
enum Watch {
    /// No manifest is loaded.
    Off,
    /// The running watcher already watches the loaded path.
    Keep,
    On(FileWatcher),
    Failed(PathBuf, String),
}

type Backend = Box<dyn TarpackBackend>;

/// The Tar Packager's state and behaviour.
pub struct TarpackModel {
    worker: Worker<Backend>,
    session: Option<TarpackSession>,
    load: Load,
    shown: Shown,
    error: Option<ViewError>,
    error_generation: u64,
    /// Sequence number of the newest session command; older responses are
    /// discarded.
    command_seq: u64,
    recent: Vec<String>,
    recent_seq: u64,
    changed_on_disk: bool,
    /// The state warning the user dismissed; a different text shows again.
    state_warning_dismissed: Option<String>,
    errors_expanded: bool,
    drop_state: Option<DropState>,
    /// Identifies the drop whose timers may still fire.
    drop_generation: u64,
    blocker: Option<DropBlocker>,
    /// The running watcher and the id its callback reports.
    watcher: Option<(u64, FileWatcher)>,
    /// Identifies the newest request to (re)arm the watcher; an older result
    /// is dropped.
    arm_seq: u64,
    next_watcher_id: u64,
    changes: async_channel::Sender<u64>,
    announcement: Option<String>,
    announcement_generation: u64,
    focus_hint: Option<FocusHint>,
    progress: Option<Progress>,
    /// The last file the progress named, with its phase: the label keeps it
    /// while directory and long-name records name none.
    progress_last: Option<(BuildPhase, Option<String>)>,
    result: Option<BuildOutcome>,
    /// The output named in the open overwrite confirmation.
    confirming: Option<String>,
    format_notice: Option<String>,
    /// A chosen output whose answer decides the format notice: the command
    /// it belongs to and the format before.
    notice_probe: Option<(u64, ArchiveFormat)>,
    shortcuts_open: bool,
}

fn manifest_filter() -> FileFilter {
    FileFilter {
        name: "Manifest".to_string(),
        patterns: vec!["*.toml".to_string()],
    }
}

/// The folder a dialog starts in for the file at `path` (a hint only).
fn folder_of(path: &Path) -> Option<PathBuf> {
    path.parent()
        .filter(|d| !d.as_os_str().is_empty())
        .map(Path::to_path_buf)
}

/// The threads [`default_start_dir`] ran on, newest last (tests check that
/// none of them is the UI thread).
#[cfg(test)]
pub(super) static LOOKUP_THREADS: std::sync::Mutex<Vec<std::thread::ThreadId>> =
    std::sync::Mutex::new(Vec::new());

/// Where "Create from example" starts when no manifest is loaded: the user's
/// Documents folder, else their home folder.
///
/// The lookup asks the system and may touch the disk, so the model runs it
/// on a thread of its own, never on the UI thread.
#[must_use]
pub fn default_start_dir() -> Option<PathBuf> {
    #[cfg(test)]
    LOOKUP_THREADS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push(std::thread::current().id());
    crate::app::dirs::user_documents_dir()
}

impl TarpackModel {
    /// A model over `backend`; it starts loading the session (which restores
    /// the most recent manifest) at once.
    pub fn new(backend: Box<dyn TarpackBackend>, cx: &mut Context<Self>) -> Self {
        let mut model = Self::unstarted(backend, cx);
        Self::arm_skeleton(cx);
        model.fetch(Origin::Load, true, cx);
        model.fetch_recent(cx);
        model
    }

    /// The model before any load is requested (tests start one part at a
    /// time).
    pub(super) fn unstarted(backend: Box<dyn TarpackBackend>, cx: &mut Context<Self>) -> Self {
        let worker = Worker::new(backend, cx);
        let (change_tx, change_rx) = async_channel::unbounded::<u64>();
        cx.spawn(async move |this, cx| {
            while let Ok(id) = change_rx.recv().await {
                if this.update(cx, |m, cx| m.watcher_fired(id, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        // The extraction command shows only in developer mode: a change
        // redraws at once.
        cx.observe_global::<DevMode>(|_, cx| cx.notify()).detach();
        Self {
            worker,
            session: None,
            load: Load::Loading,
            shown: Shown::default(),
            error: None,
            error_generation: 0,
            command_seq: 0,
            recent: Vec::new(),
            recent_seq: 0,
            changed_on_disk: false,
            state_warning_dismissed: None,
            errors_expanded: true,
            drop_state: None,
            drop_generation: 0,
            blocker: None,
            watcher: None,
            arm_seq: 0,
            next_watcher_id: 0,
            changes: change_tx,
            announcement: None,
            announcement_generation: 0,
            focus_hint: None,
            progress: None,
            progress_last: None,
            result: None,
            confirming: None,
            format_notice: None,
            notice_probe: None,
            shortcuts_open: false,
        }
    }

    // ---- reading ----

    /// The session; `None` while loading or after a failed load.
    #[must_use]
    pub fn session(&self) -> Option<&TarpackSession> {
        self.session.as_ref()
    }

    /// The first load has not answered yet.
    #[must_use]
    pub fn loading(&self) -> bool {
        self.load == Load::Loading
    }

    /// Loading has lasted [`SKELETON_DELAY`]: the view shows its skeleton.
    #[must_use]
    pub fn show_skeleton(&self) -> bool {
        self.shown.skeleton && self.loading()
    }

    /// The first load failed (the backend panicked). The view shows the
    /// empty state with the error banner; opening a manifest still works.
    #[must_use]
    pub fn load_failed(&self) -> bool {
        self.load == Load::Failed
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

    /// Hides the error banner.
    pub fn dismiss_error(&mut self, cx: &mut Context<Self>) {
        if self.error.take().is_some() {
            cx.notify();
        }
    }

    /// The state warning (store problems, a manifest that could not be
    /// reopened), unless the user dismissed this text.
    #[must_use]
    pub fn state_warning(&self) -> Option<&str> {
        let warning = self.session.as_ref()?.state_warning.as_deref()?;
        (self.state_warning_dismissed.as_deref() != Some(warning)).then_some(warning)
    }

    /// Hides the current state warning. A different one shows again.
    pub fn dismiss_state_warning(&mut self, cx: &mut Context<Self>) {
        self.state_warning_dismissed = self.session.as_ref().and_then(|s| s.state_warning.clone());
        cx.notify();
    }

    /// The manifest file changed since it was loaded (the view shows the
    /// banner with a Reload action while a manifest is loaded).
    #[must_use]
    pub fn changed_on_disk(&self) -> bool {
        self.changed_on_disk
    }

    /// The errors report is open. It starts open, and a load or reload of a
    /// manifest with errors opens it again.
    #[must_use]
    pub fn errors_expanded(&self) -> bool {
        self.errors_expanded
    }

    /// Opens or closes the errors report.
    pub fn set_errors_expanded(&mut self, on: bool, cx: &mut Context<Self>) {
        self.errors_expanded = on;
        cx.notify();
    }

    /// Opens the errors report and hints [`FocusHint::Report`]. Does nothing
    /// when the manifest has no errors.
    pub fn show_errors(&mut self, cx: &mut Context<Self>) {
        let errors = self
            .session
            .as_ref()
            .and_then(|s| s.manifest.as_ref())
            .map_or(0, |m| m.error_count);
        if errors == 0 {
            return;
        }
        self.errors_expanded = true;
        self.focus_hint = Some(FocusHint::Report);
        cx.notify();
    }

    /// The recent manifests as display strings, most recent first.
    #[must_use]
    pub fn recent(&self) -> &[String] {
        &self.recent
    }

    /// The drop's state: pending, or the result of the last one.
    #[must_use]
    pub fn drop_state(&self) -> Option<&DropState> {
        self.drop_state.as_ref()
    }

    /// A drop has been matching for [`PENDING_DELAY`]: the view shows its
    /// "matching" line.
    #[must_use]
    pub fn drop_pending_shown(&self) -> bool {
        self.shown.pending && matches!(self.drop_state, Some(DropState::Pending))
    }

    /// Hides the drop result. Hints [`FocusHint::AfterResult`].
    pub fn dismiss_drop_result(&mut self, cx: &mut Context<Self>) {
        if matches!(self.drop_state, Some(DropState::Result(_))) {
            self.drop_state = None;
            self.focus_hint = Some(FocusHint::AfterResult);
            cx.notify();
        }
    }

    /// Whether the window's drop is taken now, or the reason it is not
    /// ([`text::drop_availability`]).
    ///
    /// # Errors
    /// The reason, a sentence for the overlay.
    pub fn drop_availability(&self) -> Result<(), &'static str> {
        text::drop_availability(
            self.session.as_ref().and_then(|s| s.manifest.as_ref()),
            self.blocker,
            matches!(self.drop_state, Some(DropState::Pending)),
        )
    }

    /// Sets what blocks drops and the controls, as a running build and its
    /// open confirmation do (tests set it directly).
    #[cfg(test)]
    pub fn set_drop_blocker(&mut self, blocker: Option<DropBlocker>, cx: &mut Context<Self>) {
        self.blocker = blocker;
        cx.notify();
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

    /// Where focus should go after the latest change; cleared by reading.
    pub fn take_focus_hint(&mut self) -> Option<FocusHint> {
        self.focus_hint.take()
    }

    /// Whether "Edit in editor" is offered: developer mode is on and a
    /// manifest is loaded.
    #[must_use]
    pub fn can_edit(&self, cx: &App) -> bool {
        DevMode::is_on(cx) && self.session.as_ref().is_some_and(|s| s.manifest.is_some())
    }

    /// A build runs or is being confirmed: the controls are disabled.
    fn locked(&self) -> bool {
        self.blocker.is_some()
    }

    // ---- loading ----

    /// The tool became the current one: forgets the banner, the format
    /// notice, an open overwrite confirmation, the drop result (a pending
    /// drop continues), the announcement and any focus hint, keeps the
    /// session, `changed_on_disk` (that is about the file), a running build
    /// and the build result, and refreshes the session.
    pub fn activated(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        self.format_notice = None;
        // A confirmation cannot be open while a build runs, so closing it
        // leaves nothing blocked. A running build continues, and a result
        // (even one that arrived while the tool was away) stays until it is
        // dismissed.
        if self.confirming.take().is_some() {
            self.blocker = None;
        }
        if !matches!(self.drop_state, Some(DropState::Pending)) {
            self.drop_state = None;
        }
        self.announcement = None;
        self.focus_hint = None;
        self.fetch(Origin::Other, true, cx);
        cx.notify();
    }

    pub(super) fn arm_skeleton(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SKELETON_DELAY).await;
            this.update(cx, |m, cx| {
                m.shown.skeleton = true;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Fetches the session. A success clears the banner only when
    /// `clear_error`, and never one raised after the fetch was queued.
    pub(super) fn fetch(&mut self, origin: Origin, clear_error: bool, cx: &mut Context<Self>) {
        let generation = self.error_generation;
        let task = self.worker.run(|b| b.session(), cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |m, cx| match result {
                Ok(session) => {
                    // The worker answers in order, so this fetch can land after
                    // a command queued before it: an error that command
                    // reported meanwhile is newer and must stay.
                    let keep =
                        (!clear_error || m.error_generation != generation).then(|| m.error.clone());
                    m.apply(session, origin, cx);
                    if let Some(error) = keep {
                        m.error = error;
                    }
                    m.arm_watcher(false, cx);
                }
                Err(e) => {
                    if m.session.is_none() {
                        m.load = Load::Failed;
                    }
                    m.fail(e.into(), cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn fetch_recent(&mut self, cx: &mut Context<Self>) {
        self.recent_seq += 1;
        let seq = self.recent_seq;
        let task = self.worker.run(|b| b.recent_manifests(), cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |m, cx| {
                if let (true, Ok(list)) = (seq == m.recent_seq, result) {
                    m.recent = list;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn fail(&mut self, error: ViewError, cx: &mut Context<Self>) {
        let error = match error {
            ViewError::Tarpack {
                error,
                source: None,
            } => {
                let source = self.session.as_ref().and_then(|s| {
                    let entries = s.manifest.as_ref().map_or(&[][..], |m| &m.entries[..]);
                    text::error_source(&error, entries).map(str::to_string)
                });
                ViewError::Tarpack { error, source }
            }
            other => other,
        };
        self.error = Some(error);
        self.error_generation += 1;
        cx.notify();
    }

    fn announce(&mut self, sentence: String, cx: &mut Context<Self>) {
        self.announcement = Some(sentence);
        self.announcement_generation += 1;
        cx.notify();
    }

    /// Makes `next` the session. Every success clears the error. A load or
    /// reload also clears `changed_on_disk` and a drop result, and opens the
    /// errors report when the manifest has errors. One announcement per
    /// result: a new state warning, then the manifest sentence.
    fn apply(&mut self, next: TarpackSession, origin: Origin, cx: &mut Context<Self>) {
        let (prev_errors, prev_warning) = self.session.as_ref().map_or((0, None), |s| {
            (
                s.manifest.as_ref().map_or(0, |m| m.error_count),
                s.state_warning.clone(),
            )
        });
        let next_errors = next.manifest.as_ref().map_or(0, |m| m.error_count);
        let mut parts = Vec::new();
        if let Some(warning) = next.state_warning.as_deref() {
            if prev_warning.as_deref() != Some(warning)
                && self.state_warning_dismissed.as_deref() != Some(warning)
            {
                parts.push(warning.to_string());
            }
        }
        if let Some(sentence) = text::manifest_sentence(&next, prev_errors, origin) {
            parts.push(sentence);
        }
        self.session = Some(next);
        self.load = Load::Loaded;
        self.error = None;
        if origin != Origin::Other {
            self.changed_on_disk = false;
            if matches!(self.drop_state, Some(DropState::Result(_))) {
                self.drop_state = None;
                if origin == Origin::Reload {
                    self.focus_hint = Some(FocusHint::Reload);
                }
            }
            if next_errors > 0 {
                self.errors_expanded = true;
            }
        }
        if origin != Origin::Other && self.result.take().is_some() && origin == Origin::Reload {
            // Loading starts clean: the build result goes, so Show in folder
            // never points at another manifest's archive. After a reload,
            // focus inside it moves to Reload.
            self.focus_hint = Some(FocusHint::Reload);
        }
        if origin == Origin::Reload {
            self.format_notice = None;
        }
        if !parts.is_empty() {
            self.announce(parts.join(" "), cx);
        }
        cx.notify();
    }

    // ---- the watcher ----

    /// Watches the loaded manifest, on the Worker. `force` replaces the
    /// running watcher even when it watches the same path (open, reload and
    /// create); otherwise it is kept. With no manifest loaded there is none.
    fn arm_watcher(&mut self, force: bool, cx: &mut Context<Self>) {
        self.arm_seq += 1;
        let seq = self.arm_seq;
        self.next_watcher_id += 1;
        let id = self.next_watcher_id;
        let running = if force {
            None
        } else {
            self.watcher.as_ref().map(|(_, w)| w.path().to_path_buf())
        };
        let changes = self.changes.clone();
        let task = self.worker.run(
            move |b| {
                let Some(path) = b.manifest_path() else {
                    return Watch::Off;
                };
                if running.as_deref() == Some(path.as_path()) {
                    return Watch::Keep;
                }
                match FileWatcher::start(&path, move || {
                    let _ = changes.try_send(id);
                }) {
                    Ok(w) => Watch::On(w),
                    Err(e) => Watch::Failed(path, e),
                }
            },
            cx,
        );
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |m, _| {
                if seq != m.arm_seq {
                    return;
                }
                match result {
                    Ok(Watch::Off) => m.watcher = None,
                    Ok(Watch::Keep) => {}
                    Ok(Watch::On(w)) => m.watcher = Some((id, w)),
                    Ok(Watch::Failed(path, e)) => {
                        log::warn!("tarpack: cannot watch {}: {e}", path.display());
                        m.watcher = None;
                    }
                    Err(e) => {
                        log::warn!("tarpack: {e}");
                        m.watcher = None;
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    /// The id of the running watcher; it changes whenever the watcher is
    /// replaced.
    #[cfg(test)]
    pub(super) fn watcher_id(&self) -> Option<u64> {
        self.watcher.as_ref().map(|(id, _)| *id)
    }

    /// The watcher `id` reported a change; one that is no longer the running
    /// watcher is ignored.
    pub(super) fn watcher_fired(&mut self, id: u64, cx: &mut Context<Self>) {
        if self
            .watcher
            .as_ref()
            .is_some_and(|(current, _)| *current == id)
        {
            self.file_changed(cx);
        }
    }

    /// The manifest file changed on disk: raises the banner and announces it,
    /// once until the next load or reload. The file watcher calls it; the
    /// screenshot harness and tests call it to inject the signal.
    pub fn file_changed(&mut self, cx: &mut Context<Self>) {
        if !self.changed_on_disk {
            self.announce(text::CHANGED_ON_DISK.to_string(), cx);
        }
        self.changed_on_disk = true;
        cx.notify();
    }

    // ---- session commands ----

    /// Runs a command that answers with a session. Only the newest
    /// command's response is shown.
    fn command(
        &mut self,
        f: impl FnOnce(&mut Backend) -> Result<TarpackSession, TarpackError> + Send + 'static,
        origin: Origin,
        cx: &mut Context<Self>,
    ) {
        self.command_or_refresh(move |b| f(b).map(Some), origin, cx);
    }

    /// Like [`TarpackModel::command`] for a command that may decline with
    /// `Ok(None)`: the recent list the model showed was out of date, so
    /// nothing was done. The model refreshes its list and says so.
    fn command_or_refresh(
        &mut self,
        f: impl FnOnce(&mut Backend) -> Result<Option<TarpackSession>, TarpackError> + Send + 'static,
        origin: Origin,
        cx: &mut Context<Self>,
    ) {
        self.command_seq += 1;
        let seq = self.command_seq;
        let task = self.worker.run(f, cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |m, cx| m.command_done(seq, result, origin, cx))
                .ok();
        })
        .detach();
    }

    fn command_done(
        &mut self,
        seq: u64,
        result: Result<Result<Option<TarpackSession>, TarpackError>, JobPanicked>,
        origin: Origin,
        cx: &mut Context<Self>,
    ) {
        if seq != self.command_seq {
            return;
        }
        let probe = self.notice_probe.take().filter(|(s, _)| *s == seq);
        match result {
            Ok(Ok(None)) => {
                self.fetch_recent(cx);
                self.fail(ViewError::RecentChanged, cx);
            }
            Ok(Ok(Some(session))) => {
                let format = session.format;
                self.apply(session, origin, cx);
                if let Some((_, before)) = probe {
                    self.format_notice =
                        (format != before).then(|| text::format_changed_notice(format));
                }
                if origin != Origin::Other {
                    self.arm_watcher(true, cx);
                    self.fetch_recent(cx);
                }
            }
            Ok(Err(e)) => self.fail(e.into(), cx),
            Err(e) => {
                // The backend's state is unknown: fetch it again, keep the
                // banner.
                self.fail(e.into(), cx);
                self.fetch(Origin::Other, false, cx);
            }
        }
    }

    /// Runs `f` with the answer of a dialog: a cancel does nothing, a dialog
    /// failure is the banner.
    fn after_dialog(
        pick: gpui::Task<crate::platform::PickResult>,
        then: impl FnOnce(&mut Self, PathBuf, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let result = pick.await;
            this.update(cx, |m, cx| match result {
                Ok(Some(path)) => then(m, path, cx),
                Ok(None) => {}
                Err(PickError::Failed { detail }) => m.fail(ViewError::Dialog { detail }, cx),
            })
            .ok();
        })
        .detach();
    }

    /// Asks the platform for a manifest (filter "Manifest", `*.toml`) and
    /// opens it. A cancel changes nothing.
    pub fn open(&mut self, cx: &mut Context<Self>) {
        if self.locked() {
            return;
        }
        let spec = OpenSpec {
            kind: PickKind::File,
            filters: vec![manifest_filter()],
            start_dir: None,
            title: None,
        };
        let pick = platform(cx).pick(spec, cx);
        Self::after_dialog(
            pick,
            |m, path, cx| m.command(move |b| b.open(&path), Origin::Load, cx),
            cx,
        );
    }

    /// Opens the recent manifest at `index` of [`TarpackModel::recent`], by
    /// the path the backend stored for it. The backend opens it only if its
    /// own list still shows the same text at `index`; otherwise nothing is
    /// opened, the list is refreshed and the banner is
    /// [`ViewError::RecentChanged`].
    pub fn open_recent(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.locked() {
            return;
        }
        let Some(expected) = self.recent.get(index).cloned() else {
            return;
        };
        self.command_or_refresh(move |b| b.open_recent(index, &expected), Origin::Load, cx);
    }

    /// Reads the loaded manifest again, keeping assignments.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        if self.locked() {
            return;
        }
        self.command(|b| b.reload(), Origin::Reload, cx);
    }

    /// Waits for the system's answer to an open or reveal and shows its
    /// failure as the banner (`OpenerFailed`).
    pub(super) fn report_open(task: Task<Result<(), OpenError>>, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            if let Err(e) = task.await {
                this.update(cx, |m, cx| m.fail(e.into(), cx)).ok();
            }
        })
        .detach();
    }

    /// Opens the loaded manifest with the system's editor. A system that
    /// cannot open it (no `.toml` handler) is the `OpenerFailed` banner.
    pub fn edit(&mut self, cx: &mut Context<Self>) {
        if self.locked() {
            return;
        }
        let task = self.worker.run(|b| b.manifest_path_for_open(), cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |m, cx| match result {
                Ok(Ok(path)) => {
                    let opened = platform(cx).open_with_system(&path, cx);
                    Self::report_open(opened, cx);
                }
                Ok(Err(e)) => m.fail(e.into(), cx),
                Err(e) => m.fail(e.into(), cx),
            })
            .ok();
        })
        .detach();
    }

    /// Asks the platform where to save the bundled example (a save dialog,
    /// filter "Manifest", default extension `toml`, suggested name
    /// `manifest.toml`), writes it there and opens it. The dialog starts in
    /// the loaded manifest's folder, else Documents, else the home folder.
    /// The backend never replaces a file: an existing one is `PathExists`.
    pub fn create_from_example(&mut self, cx: &mut Context<Self>) {
        if self.locked() {
            return;
        }
        let task = self.worker.run(|b| b.manifest_path(), cx);
        cx.spawn(async move |this, cx| {
            let loaded = task.await;
            let loaded_dir = match &loaded {
                Ok(Some(path)) => folder_of(path),
                _ => None,
            };
            let start_dir = if let Some(dir) = loaded_dir {
                Some(dir)
            } else {
                Self::documents_dir(&this, cx).await
            };
            let spec = SaveSpec {
                filters: vec![manifest_filter()],
                default_extension: Some("toml".to_string()),
                start_dir,
                suggested_name: Some("manifest.toml".into()),
                title: None,
            };
            let Ok(pick) = this.update(cx, |_, cx| platform(cx).pick_save(spec, cx)) else {
                return;
            };
            let result = pick.await;
            this.update(cx, |m, cx| match result {
                Ok(Some(path)) => {
                    m.command(move |b| b.create_from_example(&path), Origin::Load, cx);
                }
                Ok(None) => {}
                Err(PickError::Failed { detail }) => m.fail(ViewError::Dialog { detail }, cx),
            })
            .ok();
        })
        .detach();
    }

    /// Where a dialog starts when nothing better is known: the lookup asks
    /// the system, so it runs on a thread of its own, never on the UI thread.
    async fn documents_dir(this: &WeakEntity<Self>, cx: &mut AsyncApp) -> Option<PathBuf> {
        let (_, lookup) = this
            .update(cx, |_, cx| {
                spawn_thread(
                    "tarpack-start-dir",
                    |_: async_channel::Sender<()>| default_start_dir(),
                    cx,
                )
            })
            .ok()?;
        lookup.await.ok().flatten()
    }

    /// Asks the platform for the source file of the entry `entry_id` (no
    /// filter, starting in the folder of its assigned file) and assigns it.
    /// The folder comes from the path the backend stored, never from the
    /// display string, so a name that is not valid Unicode still works. An
    /// entry with no assignment, or a file with no parent, has no start
    /// folder.
    pub fn browse(&mut self, entry_id: &str, cx: &mut Context<Self>) {
        if self.locked() {
            return;
        }
        let id = entry_id.to_string();
        let lookup_id = id.clone();
        let assigned = self.worker.run(move |b| b.assigned_path(&lookup_id), cx);
        cx.spawn(async move |this, cx| {
            // A panicking lookup only costs the start folder.
            let start_dir = assigned.await.ok().flatten().as_deref().and_then(folder_of);
            let spec = OpenSpec {
                kind: PickKind::File,
                filters: Vec::new(),
                start_dir,
                title: None,
            };
            let Ok(pick) = this.update(cx, |_, cx| platform(cx).pick(spec, cx)) else {
                return;
            };
            let result = pick.await;
            this.update(cx, |m, cx| match result {
                Ok(Some(path)) => {
                    m.command(move |b| b.assign(&id, &path), Origin::Other, cx);
                }
                Ok(None) => {}
                Err(PickError::Failed { detail }) => m.fail(ViewError::Dialog { detail }, cx),
            })
            .ok();
        })
        .detach();
    }

    /// Clears the assignment of the entry `entry_id`.
    pub fn clear(&mut self, entry_id: &str, cx: &mut Context<Self>) {
        if self.locked() {
            return;
        }
        let id = entry_id.to_string();
        self.command(move |b| b.clear(&id), Origin::Other, cx);
    }

    // ---- dropping ----

    /// Files or folders dropped on the window: matched to the entries on a
    /// thread, then assigned. Ignored while a drop is pending or when
    /// [`TarpackModel::drop_availability`] says no. The result is
    /// [`DropState::Result`] and is announced; an error clears the drop
    /// state and shows the banner.
    pub fn dropped(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if self.drop_availability().is_err() {
            return;
        }
        if matches!(self.drop_state, Some(DropState::Result(_))) {
            self.focus_hint = Some(FocusHint::AfterResult);
        }
        self.drop_generation += 1;
        let generation = self.drop_generation;
        self.drop_state = Some(DropState::Pending);
        self.shown.pending = false;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(PENDING_DELAY).await;
            let still = this.update(cx, |m, cx| {
                let current = m.drop_generation == generation
                    && matches!(m.drop_state, Some(DropState::Pending));
                if current {
                    m.shown.pending = true;
                    cx.notify();
                }
                current
            });
            if !matches!(still, Ok(true)) {
                return;
            }
            cx.background_executor()
                .timer(PENDING_ANNOUNCE_DELAY.saturating_sub(PENDING_DELAY))
                .await;
            this.update(cx, |m, cx| {
                if m.drop_generation == generation
                    && matches!(m.drop_state, Some(DropState::Pending))
                {
                    m.announce(text::MATCHING.to_string(), cx);
                }
            })
            .ok();
        })
        .detach();
        cx.spawn(async move |this, cx| {
            let Some(result) = Self::match_drop(this.clone(), paths, cx).await else {
                return;
            };
            this.update(cx, |m, cx| m.drop_done(generation, result, cx))
                .ok();
        })
        .detach();
    }

    /// The three steps of a drop, up to [`DROP_ATTEMPTS`] times while the
    /// session changes under the walk. `None`: the model is gone.
    async fn match_drop(
        this: WeakEntity<Self>,
        paths: Vec<PathBuf>,
        cx: &mut AsyncApp,
    ) -> Option<Result<DroppedAssignment, ViewError>> {
        for _ in 0..DROP_ATTEMPTS {
            let begin = this
                .update(cx, |m, cx| m.worker.run(|b| b.begin_drop(), cx))
                .ok()?;
            let mut work = match begin.await {
                Ok(Ok(work)) => work,
                Ok(Err(e)) => return Some(Err(e.into())),
                Err(e) => return Some(Err(e.into())),
            };
            let walked = paths.clone();
            // The folder walk runs on a thread with no lock held.
            let (_progress, walk) = this
                .update(cx, |_, cx| {
                    spawn_thread(
                        "tarpack-drop",
                        move |_: async_channel::Sender<()>| {
                            work.run(&walked);
                            work
                        },
                        cx,
                    )
                })
                .ok()?;
            let work = match walk.await {
                Ok(work) => work,
                Err(e) => return Some(Err(e.into())),
            };
            let finish = this
                .update(cx, |m, cx| m.worker.run(move |b| b.finish_drop(work), cx))
                .ok()?;
            match finish.await {
                Ok(Ok(Some(done))) => return Some(Ok(done)),
                Ok(Ok(None)) => {}
                Ok(Err(e)) => return Some(Err(e.into())),
                Err(e) => return Some(Err(e.into())),
            }
        }
        Some(Err(TarpackError::new(
            TarpackErrorKind::Io,
            text::DROP_STALE,
        )
        .into()))
    }

    fn drop_done(
        &mut self,
        generation: u64,
        result: Result<DroppedAssignment, ViewError>,
        cx: &mut Context<Self>,
    ) {
        if generation != self.drop_generation {
            return;
        }
        self.shown.pending = false;
        match result {
            Ok(DroppedAssignment { session, outcome }) => {
                self.apply(session, Origin::Other, cx);
                let manifest = self.session.as_ref().and_then(|s| s.manifest.as_ref());
                let entries = manifest.map(|m| m.entries.clone()).unwrap_or_default();
                let has_failed_entries = manifest.is_some_and(|m| !m.failed_entries.is_empty());
                let lines = text::drop_lines(&outcome, &entries, has_failed_entries);
                self.announce(text::drop_result_text(&lines), cx);
                self.drop_state = Some(DropState::Result(DropReport {
                    outcome,
                    entries,
                    has_failed_entries,
                    lines,
                }));
            }
            Err(e) => {
                self.drop_state = None;
                // A panic leaves the backend's state unknown: fetch it again,
                // keep the banner.
                let panicked = matches!(e, ViewError::Unexpected { .. });
                self.fail(e, cx);
                if panicked {
                    self.fetch(Origin::Other, false, cx);
                }
            }
        }
        cx.notify();
    }
}

/// What a build that never settled normally is reported as.
fn stopped(panic: &JobPanicked) -> BuildFailure {
    BuildFailure::Panicked(panic.0.clone())
}

/// Why a build did not write an archive.
enum BuildFailure {
    /// The backend answered with an error.
    Tarpack(TarpackError),
    /// A call panicked, with the panic message.
    Panicked(String),
}

/// A build, as the model's task reports it: what it came to, and whether the
/// backend's state is unknown (a call panicked), so the session is fetched
/// again.
type Settled = (Result<BuildSummary, BuildFailure>, bool);

/// The Worker's answer to `begin_build`.
type Begun = Task<Result<Result<Box<dyn BuildWork>, TarpackError>, JobPanicked>>;

// ---- the build side ----

impl TarpackModel {
    // ---- reading ----

    /// A build runs: the controls are disabled and the progress shows.
    #[must_use]
    pub fn building(&self) -> bool {
        self.blocker == Some(DropBlocker::Building)
    }

    /// Whether Create archive is offered: the session says it can build and
    /// neither a build runs nor a confirmation is open.
    #[must_use]
    pub fn can_build(&self) -> bool {
        self.session.as_ref().is_some_and(|s| s.can_build) && !self.locked()
    }

    /// Why Create archive is disabled ([`text::build_reason`]).
    #[must_use]
    pub fn build_reason(&self) -> Option<String> {
        text::build_reason(self.session.as_ref()?)
    }

    /// The note about files left out of the build ([`text::left_out_note`]).
    #[must_use]
    pub fn left_out_note(&self) -> Option<String> {
        text::left_out_note(self.session.as_ref()?)
    }

    /// Set when the chosen file name switched the format: "Format changed to
    /// zstd to match the file name." The view reads it out in a live region.
    /// Any later format or output change, a reload, a new build and
    /// [`TarpackModel::activated`] clear it.
    #[must_use]
    pub fn format_notice(&self) -> Option<&str> {
        self.format_notice.as_deref()
    }

    /// The latest progress event of the running build; `None` until the first
    /// arrives (the view shows the writing phase at 0%).
    #[must_use]
    pub fn progress(&self) -> Option<&Progress> {
        self.progress.as_ref()
    }

    fn source_file(&self, id: &str) -> Option<&str> {
        let manifest = self.session.as_ref()?.manifest.as_ref()?;
        let entry = manifest.entries.iter().find(|e| e.view.id == id)?;
        Some(entry.view.source.as_str())
    }

    /// The source file the progress label names: the file of the current
    /// event, else the last one named in the same phase.
    #[must_use]
    pub fn progress_file(&self) -> Option<&str> {
        let p = self.progress.as_ref()?;
        let id = p.entry_id.as_deref().or_else(|| {
            self.progress_last
                .as_ref()
                .filter(|(phase, _)| *phase == p.phase)
                .and_then(|(_, id)| id.as_deref())
        })?;
        self.source_file(id)
    }

    /// The label above the progress bar ([`text::progress_label`]).
    #[must_use]
    pub fn progress_label(&self) -> String {
        text::progress_label(self.progress.as_ref(), self.progress_file())
    }

    /// The progress bar's accessible value: the current event's file only.
    #[must_use]
    pub fn progress_value_text(&self) -> String {
        let current = self
            .progress
            .as_ref()
            .and_then(|p| p.entry_id.as_deref())
            .and_then(|id| self.source_file(id));
        text::progress_value_text(self.progress.as_ref(), current)
    }

    /// The output the open overwrite confirmation names, as a display path
    /// (title [`text::overwrite_title`], body [`text::overwrite_body`]).
    #[must_use]
    pub fn confirming_overwrite(&self) -> Option<&str> {
        self.confirming.as_deref()
    }

    /// What the last build came to, until it is dismissed, a reload, or the
    /// next build.
    #[must_use]
    pub fn result(&self) -> Option<&BuildOutcome> {
        self.result.as_ref()
    }

    /// The command that extracts the archive on the target; only while
    /// developer mode is on and the last build succeeded.
    #[must_use]
    pub fn extraction_command(&self, cx: &App) -> Option<&str> {
        match &self.result {
            Some(BuildOutcome::Ok(s)) if DevMode::is_on(cx) => Some(s.extract_command.as_str()),
            _ => None,
        }
    }

    // ---- output and format ----

    /// Asks the platform where to save the archive (a save dialog) and sets
    /// it. The dialog starts in the folder of the current output, else of the
    /// loaded manifest, else Documents; the suggested name is the current
    /// output's file name, else the session's suggestion. The filter is
    /// "<format> archive (<extension>)" over the format's last suffix, which
    /// is also the default extension; the backend normalises the full
    /// extension. A name that changes the format sets
    /// [`TarpackModel::format_notice`]. A cancel changes nothing.
    pub fn choose_output(&mut self, cx: &mut Context<Self>) {
        if self.locked() {
            return;
        }
        let Some(session) = &self.session else {
            return;
        };
        let Some(option) = session.formats.iter().find(|f| f.format == session.format) else {
            return;
        };
        let filter = FileFilter {
            name: text::format_filter_name(option),
            patterns: vec![format!("*.{}", option.filter_extension)],
        };
        let extension = option.filter_extension.clone();
        let suggested = session.suggested_output_name.clone();
        let held = self
            .worker
            .run(|b| (b.output_path(), b.manifest_path()), cx);
        cx.spawn(async move |this, cx| {
            // A panicking lookup only costs the start folder and the name.
            let (output, manifest) = held.await.unwrap_or((None, None));
            let name = output
                .as_deref()
                .and_then(Path::file_name)
                .map(OsStr::to_os_string)
                .or_else(|| suggested.map(OsString::from));
            if name.is_none() {
                return;
            }
            let near = output
                .as_deref()
                .and_then(folder_of)
                .or_else(|| manifest.as_deref().and_then(folder_of));
            let start_dir = if let Some(dir) = near {
                Some(dir)
            } else {
                Self::documents_dir(&this, cx).await
            };
            let spec = SaveSpec {
                filters: vec![filter],
                default_extension: Some(extension),
                start_dir,
                suggested_name: name,
                title: None,
            };
            let Ok(pick) = this.update(cx, |_, cx| platform(cx).pick_save(spec, cx)) else {
                return;
            };
            let result = pick.await;
            this.update(cx, |m, cx| match result {
                Ok(Some(path)) => m.output_chosen(path, cx),
                Ok(None) => {}
                Err(PickError::Failed { detail }) => m.fail(ViewError::Dialog { detail }, cx),
            })
            .ok();
        })
        .detach();
    }

    fn output_chosen(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.locked() {
            return;
        }
        let before = self.session.as_ref().map(|s| s.format);
        self.command(move |b| b.set_output(&path), Origin::Other, cx);
        self.notice_probe = before.map(|format| (self.command_seq, format));
    }

    /// Sets the archive format; the output's extension follows it. Clears the
    /// format notice.
    pub fn set_format(&mut self, format: ArchiveFormat, cx: &mut Context<Self>) {
        if self.locked() {
            return;
        }
        self.format_notice = None;
        self.command(move |b| b.set_format(format), Origin::Other, cx);
        cx.notify();
    }

    // ---- building ----

    /// Builds the archive without replacing a file. Does nothing unless
    /// [`TarpackModel::can_build`]. An existing output opens the overwrite
    /// confirmation ([`TarpackModel::confirming_overwrite`]) instead of
    /// building.
    pub fn build(&mut self, cx: &mut Context<Self>) {
        self.start_build(false, cx);
    }

    /// Replaces the existing output: builds again with `overwrite`.
    pub fn confirm_overwrite(&mut self, cx: &mut Context<Self>) {
        if self.confirming.take().is_none() {
            return;
        }
        self.blocker = None;
        self.start_build(true, cx);
    }

    /// Closes the overwrite confirmation; nothing is written. Hints
    /// [`FocusHint::CreateArchiveAfterDialog`].
    pub fn cancel_overwrite(&mut self, cx: &mut Context<Self>) {
        if self.confirming.take().is_none() {
            return;
        }
        self.blocker = None;
        self.focus_hint = Some(FocusHint::CreateArchiveAfterDialog);
        cx.notify();
    }

    fn start_build(&mut self, overwrite: bool, cx: &mut Context<Self>) {
        if !self.can_build() {
            return;
        }
        self.result = None;
        self.progress = None;
        self.progress_last = None;
        self.format_notice = None;
        self.blocker = Some(DropBlocker::Building);
        cx.notify();
        let begin = self.worker.run(|b| b.begin_build(), cx);
        cx.spawn(async move |this, cx| {
            let Some((outcome, refetch)) = Self::run_build(&this, begin, overwrite, cx).await
            else {
                return;
            };
            this.update(cx, |m, cx| m.build_done(overwrite, outcome, refetch, cx))
                .ok();
        })
        .detach();
    }

    /// Releases the backend's session after a build that did not report back.
    async fn abort(this: &WeakEntity<Self>, cx: &mut AsyncApp) {
        if let Ok(task) = this.update(cx, |m, cx| m.worker.run(|b| b.abort_build(), cx)) {
            let _ = task.await;
        }
    }

    /// The three steps of a build: check and mark on the Worker, write on a
    /// thread of its own with the progress forwarded as it arrives, then end
    /// it on the Worker. `None`: the model is gone.
    async fn run_build(
        this: &WeakEntity<Self>,
        begin: Begun,
        overwrite: bool,
        cx: &mut AsyncApp,
    ) -> Option<Settled> {
        let work = match begin.await {
            Ok(Ok(work)) => work,
            Ok(Err(e)) => return Some((Err(BuildFailure::Tarpack(e)), false)),
            Err(panicked) => {
                Self::abort(this, cx).await;
                return Some((Err(stopped(&panicked)), true));
            }
        };
        let (progress, job) = this
            .update(cx, |_, cx| {
                spawn_thread(
                    "tarpack-build",
                    move |tx: async_channel::Sender<Progress>| {
                        let mut work = work;
                        let mut sink = coalescing(move |event| {
                            let _ = tx.try_send(event);
                        });
                        work.run(overwrite, &mut sink);
                        work
                    },
                    cx,
                )
            })
            .ok()?;
        match Self::watch_build(this.clone(), progress, job, cx).await? {
            Ok(work) => {
                let finish = this
                    .update(cx, |m, cx| m.worker.run(move |b| b.finish_build(work), cx))
                    .ok()?;
                match finish.await {
                    Ok(result) => Some((result.map_err(BuildFailure::Tarpack), false)),
                    Err(panicked) => {
                        Self::abort(this, cx).await;
                        Some((Err(stopped(&panicked)), true))
                    }
                }
            }
            Err(panicked) => {
                Self::abort(this, cx).await;
                Some((Err(stopped(&panicked)), true))
            }
        }
    }

    /// Takes the progress events into the model as they arrive, then the
    /// job's result. The job owns the progress sender, so the events end
    /// only with the job. `None`: the model is gone.
    pub(super) async fn watch_build<W: 'static>(
        this: WeakEntity<Self>,
        progress: async_channel::Receiver<Progress>,
        job: Task<Result<W, JobPanicked>>,
        cx: &mut AsyncApp,
    ) -> Option<Result<W, JobPanicked>> {
        while let Ok(event) = progress.recv().await {
            this.update(cx, |m, cx| m.progress_event(event, cx)).ok()?;
        }
        Some(job.await)
    }

    fn progress_event(&mut self, event: Progress, cx: &mut Context<Self>) {
        if !self.building() {
            return;
        }
        match &mut self.progress_last {
            Some((phase, id)) if *phase == event.phase => {
                if event.entry_id.is_some() && event.entry_id != *id {
                    id.clone_from(&event.entry_id);
                }
            }
            last => *last = Some((event.phase, event.entry_id.clone())),
        }
        self.progress = Some(event);
        cx.notify();
    }

    /// A build settled: the progress goes, and the outcome is a result, the
    /// overwrite confirmation, the changed-on-disk state, or a failure.
    fn build_done(
        &mut self,
        overwrite: bool,
        outcome: Result<BuildSummary, BuildFailure>,
        refetch: bool,
        cx: &mut Context<Self>,
    ) {
        self.blocker = None;
        self.progress = None;
        self.progress_last = None;
        self.focus_hint = Some(FocusHint::CreateArchive);
        match outcome {
            Ok(summary) => {
                let sentence = text::result_announcement(&summary);
                self.result = Some(BuildOutcome::Ok(summary));
                self.announce(sentence, cx);
            }
            Err(BuildFailure::Panicked(detail)) => {
                self.result = Some(BuildOutcome::Unexpected { detail });
                self.announce(text::UNEXPECTED.to_string(), cx);
            }
            Err(BuildFailure::Tarpack(e))
                if e.kind == TarpackErrorKind::OutputExists && !overwrite =>
            {
                let output = self
                    .session
                    .as_ref()
                    .and_then(|s| s.output_path.clone())
                    .unwrap_or_default();
                self.confirming = Some(output);
                self.blocker = Some(DropBlocker::Confirming);
                self.focus_hint = None;
            }
            Err(BuildFailure::Tarpack(e)) if e.kind == TarpackErrorKind::ManifestChangedOnDisk => {
                self.file_changed(cx);
            }
            Err(BuildFailure::Tarpack(e)) => {
                let entries = self
                    .session
                    .as_ref()
                    .and_then(|s| s.manifest.as_ref())
                    .map_or(&[][..], |m| &m.entries[..]);
                let sentence = text::error_copy(&e, entries);
                self.result = Some(BuildOutcome::Err(e));
                self.announce(sentence, cx);
            }
        }
        if refetch {
            // The backend's state is unknown: fetch it again.
            self.fetch(Origin::Other, false, cx);
        }
        cx.notify();
    }

    // ---- the result ----

    /// Hides the build result. Hints [`FocusHint::CreateArchive`].
    pub fn dismiss_result(&mut self, cx: &mut Context<Self>) {
        if self.result.take().is_some() {
            self.focus_hint = Some(FocusHint::CreateArchive);
            cx.notify();
        }
    }

    /// Shows the archive the last build wrote in the system file manager,
    /// by the path the backend holds, never a display string. A system that
    /// cannot show it is the `OpenerFailed` banner.
    pub fn reveal_output(&mut self, cx: &mut Context<Self>) {
        let task = self.worker.run(|b| b.reveal_target(), cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |m, cx| match result {
                Ok(Ok(path)) => {
                    let shown = platform(cx).reveal(&path, cx);
                    Self::report_open(shown, cx);
                }
                Ok(Err(e)) => m.fail(e.into(), cx),
                Err(e) => m.fail(e.into(), cx),
            })
            .ok();
        })
        .detach();
    }

    /// Copies the build report ([`text::report_text`]) to the clipboard.
    pub fn copy_report(&mut self, cx: &mut Context<Self>) {
        if let Some(BuildOutcome::Ok(summary)) = &self.result {
            let report = text::report_text(summary);
            platform(cx).copy_text(&report, cx);
        }
    }

    /// Copies the archive's SHA-256 to the clipboard.
    pub fn copy_sha256(&mut self, cx: &mut Context<Self>) {
        if let Some(BuildOutcome::Ok(summary)) = &self.result {
            let hash = summary.sha256_hex.clone();
            platform(cx).copy_text(&hash, cx);
        }
    }

    /// Copies the extraction command to the clipboard; only while developer
    /// mode is on.
    pub fn copy_extraction_command(&mut self, cx: &mut Context<Self>) {
        if let Some(command) = self.extraction_command(cx).map(str::to_string) {
            platform(cx).copy_text(&command, cx);
        }
    }

    // ---- shortcuts ----

    /// The shortcuts popover is open or closed: Escape belongs to it while
    /// it is open.
    pub fn set_shortcuts_open(&mut self, open: bool) {
        self.shortcuts_open = open;
    }

    fn drop_result_visible(&self) -> bool {
        matches!(self.drop_state, Some(DropState::Result(_)))
            && self.session.as_ref().is_some_and(|s| s.manifest.is_some())
    }

    /// Whether `s` acts now. All are off while a build runs or the
    /// confirmation is open.
    #[must_use]
    pub fn shortcut(&self, s: Shortcut, cx: &App) -> bool {
        if self.locked() {
            return false;
        }
        let manifest = self
            .session
            .as_ref()
            .and_then(|s| s.manifest.as_ref())
            .is_some();
        match s {
            Shortcut::Open => true,
            Shortcut::Reload => manifest,
            Shortcut::Edit => self.can_edit(cx),
            Shortcut::Build => self.can_build(),
            Shortcut::ShowErrors => self
                .session
                .as_ref()
                .and_then(|s| s.manifest.as_ref())
                .is_some_and(|m| m.error_count > 0),
            Shortcut::Escape => {
                !self.shortcuts_open && (self.drop_result_visible() || self.result.is_some())
            }
        }
    }

    /// Runs `s` if [`TarpackModel::shortcut`] says it acts. Escape dismisses
    /// the drop result first, else the build result; it never touches the
    /// errors report.
    pub fn run_shortcut(&mut self, s: Shortcut, _window: &mut Window, cx: &mut Context<Self>) {
        if !self.shortcut(s, cx) {
            return;
        }
        match s {
            Shortcut::Open => self.open(cx),
            Shortcut::Reload => self.reload(cx),
            Shortcut::Edit => self.edit(cx),
            Shortcut::Build => self.build(cx),
            Shortcut::ShowErrors => self.show_errors(cx),
            Shortcut::Escape => {
                if self.drop_result_visible() {
                    self.dismiss_drop_result(cx);
                } else {
                    self.dismiss_result(cx);
                }
            }
        }
    }
}
