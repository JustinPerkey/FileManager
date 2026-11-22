//! The Pipeline's model: everything the Tauri view held in React state, as one
//! entity. The view only renders it, maps [`StepStatus`] to icons and colours,
//! owns focus and layout, and calls the methods here.
//!
//! No backend call runs on the UI thread: each goes through the model's
//! [`Worker`], and a step's job runs on its own thread
//! ([`spawn_thread`]). Results come back through weak-entity updates, so a
//! result that lands after the model is gone is dropped.

use std::path::PathBuf;

use fm_pipeline::session::{PipelineError, PipelineErrorKind, PipelineSession, StepId};
use gpui::{AsyncApp, Context, WeakEntity};

use crate::app::text::PICK_FAILED;
use crate::jobs::{spawn_thread, JobPanicked, Worker};
use crate::platform::{platform, OpenSpec, PickError, PickKind};

use super::backend::{panicked_step, PipelineBackend, StepJob};
use super::text;

/// What the error banner shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewError {
    /// The backend refused or reported an error; the copy is
    /// [`text::error_message`] of its kind and `message` is the detail.
    Pipeline(PipelineError),
    /// A backend call panicked or stopped; nothing is known about what
    /// happened. Copy: [`text::UNEXPECTED`].
    Unexpected { message: String },
    /// The folder dialog could not be shown. Copy: the app's `PICK_FAILED`.
    /// (An addition to the planned enum: the dialog's copy differs from
    /// `Unexpected`'s.)
    Dialog { detail: String },
}

impl ViewError {
    /// The user-facing sentence.
    #[must_use]
    pub fn copy(&self) -> &'static str {
        match self {
            ViewError::Pipeline(e) => text::error_message(e.kind),
            ViewError::Unexpected { .. } => text::UNEXPECTED,
            ViewError::Dialog { .. } => PICK_FAILED,
        }
    }

    /// The technical detail shown under "Details".
    #[must_use]
    pub fn detail(&self) -> &str {
        match self {
            ViewError::Pipeline(e) => &e.message,
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

/// What a confirmation is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pending {
    /// One step; `via_next` when "Run next" asked for it.
    Step { id: StepId, via_next: bool },
    /// Every step in order.
    All,
}

/// A step's status. The view maps it to an icon and a tone;
/// [`text::status_label`] gives the label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepStatus {
    Running,
    NotImplemented,
    Failed,
    Passed,
    /// Ran and found this many problems.
    Problems(u32),
    /// Locked until the step before passes.
    Waiting,
    NotRun,
}

/// Where the view moves focus after a run, read once with
/// [`PipelineModel::take_focus_hint`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusHint {
    /// The step's row.
    Step(StepId),
    /// The "Run next" button, which another step can use now.
    RunNext,
}

type Backend = Box<dyn PipelineBackend>;

/// The Pipeline tool's state and behaviour.
pub struct PipelineModel {
    worker: Worker<Backend>,
    session: Option<PipelineSession>,
    load_failed: bool,
    error: Option<ViewError>,
    error_generation: u64,
    /// Sequence number of the newest folder change; older responses are
    /// discarded.
    folder_seq: u64,
    /// The step this model started and waits for.
    active: Option<StepId>,
    running_all: bool,
    via_next: Option<StepId>,
    pending: Option<(Pending, String)>,
    announcement: Option<String>,
    announcement_generation: u64,
    focus_hint: Option<FocusHint>,
}

impl PipelineModel {
    /// A model over `backend`; it starts loading the session at once.
    pub fn new(backend: Box<dyn PipelineBackend>, cx: &mut Context<Self>) -> Self {
        let worker = Worker::new(backend, cx);
        let mut model = Self {
            worker,
            session: None,
            load_failed: false,
            error: None,
            error_generation: 0,
            folder_seq: 0,
            active: None,
            running_all: false,
            via_next: None,
            pending: None,
            announcement: None,
            announcement_generation: 0,
            focus_hint: None,
        };
        model.load(cx);
        model
    }

    // ---- reading ----

    /// The session; `None` while loading or after a failed load.
    #[must_use]
    pub fn session(&self) -> Option<&PipelineSession> {
        self.session.as_ref()
    }

    /// The first load has not answered yet.
    #[must_use]
    pub fn loading(&self) -> bool {
        self.session.is_none() && !self.load_failed
    }

    /// The first load failed; [`PipelineModel::retry_load`] tries again.
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

    /// A step runs (this model's or one the session reports) or Run all is
    /// between steps.
    #[must_use]
    pub fn busy(&self) -> bool {
        self.running().is_some() || self.running_all
    }

    /// The step running now.
    #[must_use]
    pub fn running(&self) -> Option<StepId> {
        self.active
            .or(self.session.as_ref().and_then(|s| s.running))
    }

    /// The step "Run next" started, so only that button shows busy for it.
    #[must_use]
    pub fn via_next(&self) -> Option<StepId> {
        self.via_next
    }

    /// Run all is in progress.
    #[must_use]
    pub fn running_all(&self) -> bool {
        self.running_all
    }

    /// Why nothing can run now, if so (see [`text`]'s `REASON_*`).
    #[must_use]
    pub fn run_reason(&self) -> Option<&'static str> {
        let session = self.session.as_ref()?;
        if self.busy() {
            Some(text::REASON_RUNNING)
        } else if let Some(dir) = &session.directory {
            dir.error.as_ref().map(|_| text::REASON_BAD_FOLDER)
        } else {
            Some(text::REASON_NO_FOLDER)
        }
    }

    /// A step may start.
    #[must_use]
    pub fn can_run(&self) -> bool {
        self.session.as_ref().is_some_and(|s| s.can_run) && !self.busy()
    }

    /// The first step that has not passed, which "Run next" runs.
    #[must_use]
    pub fn next_step(&self) -> Option<StepId> {
        self.session.as_ref().and_then(|s| s.next_step)
    }

    /// Every step has passed.
    #[must_use]
    pub fn all_passed(&self) -> bool {
        self.session.is_some() && self.next_step().is_none()
    }

    /// The status of `id`; the label is [`text::status_label`].
    #[must_use]
    pub fn step_status(&self, id: StepId) -> StepStatus {
        status_of(self.session.as_ref(), self.running(), id)
    }

    /// The open confirmation and the folder it names.
    #[must_use]
    pub fn pending(&self) -> Option<(&Pending, &str)> {
        self.pending.as_ref().map(|(p, dir)| (p, dir.as_str()))
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

    /// Where focus should go after the latest run; cleared by reading.
    pub fn take_focus_hint(&mut self) -> Option<FocusHint> {
        self.focus_hint.take()
    }

    // ---- loading ----

    /// The tool became the current one: forgets the banner, the open
    /// confirmation, the announcement and any focus hint, and refreshes the
    /// session. A running step or Run all continues and lands its result here.
    pub fn activated(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        self.pending = None;
        self.announcement = None;
        self.focus_hint = None;
        self.load(cx);
        cx.notify();
    }

    /// Fetches the session again after a failed load.
    pub fn retry_load(&mut self, cx: &mut Context<Self>) {
        self.load_failed = false;
        self.load(cx);
        cx.notify();
    }

    /// Hides the error banner.
    pub fn dismiss_error(&mut self, cx: &mut Context<Self>) {
        if self.error.take().is_some() {
            cx.notify();
        }
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let task = self.worker.run(|b| b.session(), cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |m, cx| match result {
                Ok(session) => {
                    m.session = Some(session);
                    m.load_failed = false;
                    m.error = None;
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

    // ---- the folder ----

    /// A typed or pasted path: cleaned with [`clean_path`], then set (empty
    /// clears). Does nothing when it is already the folder, or while a step
    /// runs.
    pub fn set_directory_text(&mut self, typed: &str, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let path = clean_path(typed);
        let current = self
            .session
            .as_ref()
            .and_then(|s| s.directory.as_ref())
            .map(|d| d.path.as_str());
        let unchanged = match (&path, current) {
            (None, None) => true,
            (Some(p), Some(c)) => p.to_string_lossy() == c,
            _ => false,
        };
        if !unchanged {
            self.choose(path, cx);
        }
    }

    /// Clears the folder.
    pub fn clear_directory(&mut self, cx: &mut Context<Self>) {
        if !self.busy() {
            self.choose(None, cx);
        }
    }

    /// Asks the platform for a folder, starting at the current one, and sets
    /// it. A cancel changes nothing; a dialog failure is the banner.
    pub fn browse(&mut self, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let start_dir = self
            .session
            .as_ref()
            .and_then(|s| s.directory.as_ref())
            .map(|d| PathBuf::from(&d.path));
        let spec = OpenSpec {
            kind: PickKind::Folder,
            filters: Vec::new(),
            start_dir,
            title: None,
        };
        let pick = platform(cx).pick(spec, cx);
        cx.spawn(async move |this, cx| {
            let result = pick.await;
            this.update(cx, |m, cx| match result {
                Ok(Some(path)) => m.choose(Some(path), cx),
                Ok(None) => {}
                Err(PickError::Failed { detail }) => m.fail(ViewError::Dialog { detail }, cx),
            })
            .ok();
        })
        .detach();
    }

    fn choose(&mut self, path: Option<PathBuf>, cx: &mut Context<Self>) {
        self.folder_seq += 1;
        let seq = self.folder_seq;
        let task = self.worker.run(move |b| b.set_directory(path), cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |m, cx| {
                // Only the newest folder change is shown: an older answer
                // arriving late would show a folder the backend no longer
                // holds.
                if seq != m.folder_seq {
                    return;
                }
                match result {
                    Ok(Ok(session)) => {
                        // A field already holding focus does not announce its
                        // new error, so say it.
                        let said = session
                            .directory
                            .as_ref()
                            .and_then(|d| d.error.as_ref())
                            .map(|e| text::error_message(e.kind).to_string());
                        m.session = Some(session);
                        m.error = None;
                        cx.notify();
                        if let Some(said) = said {
                            m.announce(said, cx);
                        }
                    }
                    Ok(Err(e)) => m.fail(ViewError::Pipeline(e), cx),
                    Err(e) => m.fail(e.into(), cx),
                }
            })
            .ok();
        })
        .detach();
    }

    // ---- running ----

    /// Runs `id`, after a confirmation when the step has confirm copy. Does
    /// nothing unless [`PipelineModel::can_run`].
    pub fn request_step(&mut self, id: StepId, cx: &mut Context<Self>) {
        self.start(
            Pending::Step {
                id,
                via_next: false,
            },
            cx,
        );
    }

    /// Runs [`PipelineModel::next_step`], after a confirmation when it has
    /// confirm copy.
    pub fn request_next(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.next_step() {
            self.start(Pending::Step { id, via_next: true }, cx);
        }
    }

    /// Runs every step in order, after one confirmation when any step has
    /// confirm copy.
    pub fn request_all(&mut self, cx: &mut Context<Self>) {
        self.start(Pending::All, cx);
    }

    fn start(&mut self, what: Pending, cx: &mut Context<Self>) {
        // A hint left over from an earlier run must not fire on this one.
        self.focus_hint = None;
        if !self.can_run() {
            return;
        }
        let Some(folder) = self
            .session
            .as_ref()
            .and_then(|s| s.directory.as_ref())
            .map(|d| d.path.clone())
        else {
            return;
        };
        let needs_confirm = match what {
            Pending::All => text::STEP_IDS
                .iter()
                .any(|id| text::step_copy(*id).confirm.is_some()),
            Pending::Step { id, .. } => text::step_copy(id).confirm.is_some(),
        };
        if needs_confirm {
            self.pending = Some((what, folder));
            cx.notify();
        } else {
            self.execute(what, folder, cx);
        }
    }

    /// Runs what the open confirmation asks, on exactly the folder it named.
    pub fn confirm(&mut self, cx: &mut Context<Self>) {
        if let Some((what, folder)) = self.pending.take() {
            cx.notify();
            self.execute(what, folder, cx);
        }
    }

    /// Closes the confirmation; nothing runs.
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.pending.take().is_some() {
            cx.notify();
        }
    }

    fn execute(&mut self, what: Pending, folder: String, cx: &mut Context<Self>) {
        match what {
            Pending::Step { id, via_next } => {
                cx.spawn(async move |this, cx| run_single(this, id, folder, via_next, cx).await)
                    .detach();
            }
            Pending::All => {
                self.running_all = true;
                cx.notify();
                cx.spawn(async move |this, cx| run_all(this, folder, cx).await)
                    .detach();
            }
        }
    }
}

/// Runs one step. Resolves to the new session, or `None` when the step could
/// not start (the banner says why) or the model is gone.
async fn run_one(
    this: &WeakEntity<PipelineModel>,
    id: StepId,
    folder: String,
    cx: &mut AsyncApp,
) -> Option<PipelineSession> {
    let begin = this
        .update(cx, |m, cx| {
            m.active = Some(id);
            cx.notify();
            m.worker.run(move |b| b.begin_step(id, &folder), cx)
        })
        .ok()?;
    let session = run_begun(this, id, begin.await, cx).await;
    this.update(cx, |m, cx| {
        m.active = None;
        if let Some(session) = &session {
            m.session = Some(session.clone());
            m.error = None;
        }
        cx.notify();
    })
    .ok()?;
    session
}

type Begun = Result<Result<Box<dyn StepJob>, PipelineError>, JobPanicked>;

/// The part of a run after `begin_step` answered: the job on its own thread,
/// then `finish_step`. `None` when the step did not start.
async fn run_begun(
    this: &WeakEntity<PipelineModel>,
    id: StepId,
    begin: Begun,
    cx: &mut AsyncApp,
) -> Option<PipelineSession> {
    let job = match begin {
        Ok(Ok(job)) => job,
        Ok(Err(e)) => {
            this.update(cx, |m, cx| m.fail(ViewError::Pipeline(e), cx))
                .ok()?;
            return None;
        }
        Err(e) => {
            this.update(cx, |m, cx| m.fail(e.into(), cx)).ok()?;
            return None;
        }
    };
    let (_progress, thread) = this
        .update(cx, |_, cx| {
            spawn_thread(
                "pipeline-step",
                move |_: async_channel::Sender<()>| job.run(),
                cx,
            )
        })
        .ok()?;
    let finished = match thread.await {
        Ok(finished) => finished,
        Err(JobPanicked(message)) => panicked_step(&message),
    };
    let finish = this
        .update(cx, |m, cx| {
            m.worker.run(move |b| b.finish_step(id, finished), cx)
        })
        .ok()?;
    match finish.await {
        Ok(session) => Some(session),
        Err(e) => {
            this.update(cx, |m, cx| m.fail(e.into(), cx)).ok()?;
            None
        }
    }
}

async fn run_single(
    this: WeakEntity<PipelineModel>,
    id: StepId,
    folder: String,
    via_next: bool,
    cx: &mut AsyncApp,
) {
    if via_next {
        this.update(cx, |m, cx| {
            m.via_next = Some(id);
            cx.notify();
        })
        .ok();
    }
    let session = run_one(&this, id, folder, cx).await;
    this.update(cx, |m, cx| {
        m.via_next = None;
        if let Some(session) = session {
            let label = text::status_label(m.step_status(id));
            let sentence = text::step_outcome(text::step_copy(id).title, &label);
            m.announce(sentence, cx);
            m.focus_hint = Some(
                if via_next && session.next_step.is_some() && session.can_run {
                    FocusHint::RunNext
                } else {
                    FocusHint::Step(id)
                },
            );
        }
        cx.notify();
    })
    .ok();
}

/// Every step in order, stopping at the first that does not pass.
async fn run_all(this: WeakEntity<PipelineModel>, folder: String, cx: &mut AsyncApp) {
    let ids: Vec<StepId> = this
        .update(cx, |m, _| {
            m.session
                .as_ref()
                .map(|s| s.steps.iter().map(|v| v.id).collect())
                .unwrap_or_default()
        })
        .unwrap_or_default();
    let mut last = None;
    let mut current = None;
    for id in ids {
        last = Some(id);
        current = run_one(&this, id, folder.clone(), cx).await;
        if !current.as_ref().is_some_and(|s| step_passed(s, id)) {
            break;
        }
    }
    this.update(cx, |m, cx| {
        m.running_all = false;
        if let (Some(session), Some(last)) = (current, last) {
            let all = session
                .steps
                .iter()
                .all(|v| v.report.as_ref().is_some_and(|r| r.passed));
            let sentence = if all {
                text::ALL_PASSED.to_string()
            } else {
                text::stopped_at(text::step_copy(last).title)
            };
            m.announce(sentence, cx);
            m.focus_hint = Some(FocusHint::Step(last));
        }
        cx.notify();
    })
    .ok();
}

fn step_passed(session: &PipelineSession, id: StepId) -> bool {
    session
        .steps
        .iter()
        .find(|v| v.id == id)
        .and_then(|v| v.report.as_ref())
        .is_some_and(|r| r.passed)
}

fn status_of(session: Option<&PipelineSession>, running: Option<StepId>, id: StepId) -> StepStatus {
    if running == Some(id) {
        return StepStatus::Running;
    }
    let Some(view) = session.and_then(|s| s.steps.iter().find(|v| v.id == id)) else {
        return StepStatus::NotRun;
    };
    if let Some(error) = &view.error {
        return if error.kind == PipelineErrorKind::StepNotImplemented {
            StepStatus::NotImplemented
        } else {
            StepStatus::Failed
        };
    }
    match &view.report {
        Some(report) if report.passed => StepStatus::Passed,
        Some(report) => {
            StepStatus::Problems(u32::try_from(report.issues.len()).unwrap_or(u32::MAX))
        }
        None if !view.unlocked => StepStatus::Waiting,
        None => StepStatus::NotRun,
    }
}

/// A typed or pasted path: trimmed, without the quotes Explorer's "Copy as
/// path" adds. Empty means none.
#[must_use]
pub fn clean_path(typed: &str) -> Option<PathBuf> {
    let trimmed = typed.trim();
    let unquoted = trimmed
        .strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .unwrap_or(trimmed)
        .trim();
    if unquoted.is_empty() {
        None
    } else {
        Some(PathBuf::from(unquoted))
    }
}
