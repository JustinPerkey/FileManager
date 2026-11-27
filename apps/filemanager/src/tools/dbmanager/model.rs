//! The Database Manager's model: the snapshot, the places the user has been
//! (history), the table lists, one object with its "referenced by" and
//! many-to-many pages, and the edit side: one draft at a time, the reference
//! picker, save, create, the conflict panel and the leave confirmation, the
//! staging of many-to-many link changes (saved with the draft, with the link
//! conflict rebase), and the confirmed delete with its impact. The view only
//! renders it and calls the methods here.
//!
//! No backend call runs on the UI thread: each goes through the model's
//! [`Worker`]. Results come back through weak-entity updates, so a result
//! that lands after the model is gone is dropped. Every list, object and page
//! request carries a sequence number, and only the newest request for the
//! same list or page applies.

use std::collections::HashMap;
use std::path::PathBuf;

use fm_core::watch::FileWatcher;
use fm_dbmanager::config::{RelationMode, TableMode};
use fm_dbmanager::model::{FieldView, TableView};
use fm_dbmanager::session::{
    DeleteImpact, DeleteResult, ImpactGroup, LinkRef, ListQuery, M2mOption, M2mPage, M2mQuery,
    ObjectKey, ObjectPage, ObjectSummary, ObjectView, ReferencingFilter, SaveRequest, SaveResult,
};
use fm_dbmanager::tool::{DbmanagerError, DbmanagerErrorKind, DbmanagerSnapshot};
use gpui::{Context, Task};

use crate::app::dev_mode::DevMode;
use crate::app::text::PICK_FAILED;
use crate::jobs::{JobPanicked, Worker};
use crate::platform::{platform, FileFilter, OpenError, OpenSpec, PickError, PickKind};

use super::backend::DbmanagerBackend;
pub use super::draft::EditorInput;
use super::draft::{
    rebase_link, ConflictDiff, Draft, LinkStage, Pick, RebaseOutcome, Rebased, StagedLink,
};
use super::text;

/// Rows per page, in lists, "referenced by" and many-to-many pages.
pub const PAGE_SIZE: u32 = 25;
/// Places kept in the history; the oldest is dropped beyond it.
pub const MAX_HISTORY: usize = 50;

/// What the error banner shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewError {
    /// The backend refused or reported an error; the copy is
    /// [`text::error_message`] and `error.message` is the detail.
    Dbmanager {
        error: DbmanagerError,
        /// The config's display path, for the `ConfigNotDroppable` sentence.
        config_location: Option<String>,
    },
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
            ViewError::Dbmanager {
                error,
                config_location,
            } => text::error_message(error, config_location.as_deref()),
            ViewError::Unexpected { .. } => text::UNEXPECTED.to_string(),
            ViewError::Dialog { .. } => PICK_FAILED.to_string(),
        }
    }

    /// The technical detail shown under "Details".
    #[must_use]
    pub fn detail(&self) -> &str {
        match self {
            ViewError::Dbmanager { error, .. } => &error.message,
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

/// Where the user is. Every place is an entry in the history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    /// The table list (the first place when nothing is listed).
    Tables,
    /// One table's list of objects.
    Table { table: String },
    /// One object. `label` is its title as last known (empty until read).
    Object {
        table: String,
        key: ObjectKey,
        label: String,
    },
    /// A new object of `table`.
    New { table: String },
}

/// Where the view moves focus after a change, read once with
/// [`DbmanagerModel::take_focus_hint`]. A hint is returned once, so a repeated
/// hint is distinct from an old one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FocusHint {
    /// The heading of an object's sheet (after a place change to an object or
    /// a new object).
    SheetHeading,
    /// The heading of a table's list (after a place change to a table).
    ListHeading,
    /// The error summary of a refused save.
    ErrorSummary,
    /// The error banner.
    ErrorBanner,
    /// The reference picker's trigger for this field, after the picker
    /// closed or picked.
    PickerTrigger(String),
    /// The delete notice (impact changed, or delete done).
    DeleteNotice,
}

/// The current object's read.
#[derive(Clone, Debug, PartialEq)]
pub enum ObjectLoad {
    Loading,
    Loaded(ObjectView),
    /// The object no longer exists (for instance it was deleted). A history
    /// entry showing it is kept.
    Gone,
    /// The read failed for another reason; the banner has it.
    Failed,
}

/// One table's list: kept for the app run (until another database opens), so
/// returning shows the list as left.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListState {
    /// The search text; empty is no search.
    pub search: String,
    pub offset: u64,
    /// The last page read.
    pub page: Option<ObjectPage>,
    pub loading: bool,
    seq: u64,
}

/// One "referenced by" entry of the current object.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RefList {
    pub open: bool,
    /// Every row read so far.
    pub items: Vec<ObjectSummary>,
    pub total: u64,
    pub loading: bool,
    seq: u64,
}

/// One many-to-many relation of the current object.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct M2mState {
    pub search: String,
    pub linked_only: bool,
    pub offset: u64,
    pub page: Option<M2mPage>,
    pub loading: bool,
    seq: u64,
}

/// A message under the sheet that stays until dismissed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    /// The first write of the session took a backup (display path).
    Backup { path: String },
    /// The config was reloaded; the edits that no longer apply are listed.
    ConfigReloaded { dropped: Vec<String> },
    /// A save was refused because the config changed; it was reloaded and
    /// the draft checked again. Nothing was saved.
    ConfigChangedWhileEditing { dropped: Vec<String> },
    /// The config was reloaded and the edited object is gone.
    ConfigReloadedGone { label: String },
    /// Re-applying a set-aside link change found the link already there.
    LinkAlreadyLinked,
    /// Re-applying a set-aside removal found the link already gone.
    LinkAlreadyGone,
    /// Re-applying a set-aside edit found the link already holding it.
    LinkAlreadyApplied,
    /// Edit config created the starter file (display path).
    StarterCreated { display: String },
    /// An object was deleted (`related` rows went with it; `at_least` when
    /// the count was cut short). `backup` is the display path when this
    /// delete took the session's backup.
    Deleted {
        label: String,
        related: u64,
        at_least: bool,
        backup: Option<String>,
    },
    /// A delete found the object changed by another program; nothing was
    /// deleted.
    DeleteConflict { label: String },
}

impl Notice {
    /// The user-facing sentence.
    #[must_use]
    pub fn copy(&self) -> String {
        match self {
            Notice::Backup { path } => text::backup_notice(path),
            Notice::ConfigReloaded { dropped } => text::config_reloaded(dropped),
            Notice::ConfigChangedWhileEditing { dropped } => {
                text::config_changed_while_editing(dropped)
            }
            Notice::ConfigReloadedGone { label } => text::config_reloaded_gone(label),
            Notice::LinkAlreadyLinked => text::LINK_ALREADY_LINKED.to_owned(),
            Notice::LinkAlreadyGone => text::LINK_ALREADY_GONE.to_owned(),
            Notice::LinkAlreadyApplied => text::LINK_ALREADY_APPLIED.to_owned(),
            Notice::StarterCreated { display } => text::starter_created(display),
            Notice::Deleted {
                label,
                related,
                at_least,
                backup,
            } => text::deleted_notice(label, *related, *at_least, backup.as_deref()),
            Notice::DeleteConflict { label } => text::delete_conflict(label),
        }
    }
}

/// The object changed under the draft: what the conflict panel shows.
#[derive(Clone, Debug, PartialEq)]
pub struct ConflictPanel {
    /// The object's label as the user knew it.
    pub label: String,
    /// The object as it is now.
    pub current: ObjectView,
    pub diff: ConflictDiff,
}

/// One staged link change as the many-to-many section lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StagedRow {
    pub other: ObjectKey,
    pub label: String,
    pub stage: LinkStage,
    /// The row is on the relation's current page (drawn in place). Off the
    /// page, it is drawn in the "Not saved yet" group, so a staged change is
    /// never off screen.
    pub on_page: bool,
    /// For a removal of a link that has fields: "Will be removed with its
    /// fields: ...".
    pub removal_note: Option<String>,
}

impl StagedRow {
    /// `!on_page`.
    #[must_use]
    pub fn off_page(&self) -> bool {
        !self.on_page
    }
}

/// The counts line of a many-to-many section.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkCounts {
    /// Links stored now.
    pub linked: u64,
    pub to_add: usize,
    pub to_remove: usize,
}

impl LinkCounts {
    /// "3 linked", or "3 linked \u{b7} 2 to add \u{b7} 1 to remove".
    #[must_use]
    pub fn line(&self) -> String {
        text::link_counts(self.linked, self.to_add, self.to_remove)
    }
}

/// A save met a link that another program changed: the user's change to it
/// is set aside, and this is what the section offers.
#[derive(Clone, Debug)]
pub struct LinkConflict {
    pub relation: String,
    pub other: ObjectKey,
    /// The other object's label.
    pub label: String,
    /// The link as it is now (`None`: the other object is gone).
    pub now: Option<M2mOption>,
    /// The user's change as it was.
    pub set_aside: StagedLink,
    /// What "Re-apply my change" does.
    pub rebased: Rebased,
}

impl LinkConflict {
    /// The section notice.
    #[must_use]
    pub fn notice(&self) -> String {
        text::link_conflict(&self.label)
    }
}

/// How far a delete is.
#[derive(Clone, Debug, PartialEq)]
pub enum DeleteState {
    /// The impact is being read ("Checking...").
    Checking { label: String },
    /// The confirmation, starting on Cancel.
    Confirm(DeleteDialog),
    /// The delete cannot happen; the dialog offers only Close.
    Blocked(DeleteDialog),
    /// The confirmed delete is running.
    Deleting(DeleteDialog),
}

/// What changed since the dialog first opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeleteDialogNotice {
    ImpactChanged,
    ConfigReloaded,
}

impl DeleteDialogNotice {
    /// The user-facing sentence.
    #[must_use]
    pub fn copy(self) -> &'static str {
        match self {
            DeleteDialogNotice::ImpactChanged => text::DELETE_IMPACT_CHANGED,
            DeleteDialogNotice::ConfigReloaded => text::DELETE_CONFIG_RELOADED,
        }
    }
}

/// The delete confirmation's content.
#[derive(Clone, Debug, PartialEq)]
pub struct DeleteDialog {
    pub table: String,
    pub key: ObjectKey,
    pub label: String,
    pub impact: DeleteImpact,
    /// The object has a dirty draft that the delete discards.
    pub discards_draft: bool,
    pub notice: Option<DeleteDialogNotice>,
}

impl DeleteDialog {
    /// The groups that make the delete fail, first.
    #[must_use]
    pub fn blocking_groups(&self) -> Vec<&ImpactGroup> {
        self.impact.groups.iter().filter(|g| g.blocks).collect()
    }

    /// The groups that do not block, in the impact's order.
    #[must_use]
    pub fn other_groups(&self) -> Vec<&ImpactGroup> {
        self.impact.groups.iter().filter(|g| !g.blocks).collect()
    }

    /// Blocked with no blocking group to name: show
    /// [`text::DELETE_BLOCKED_GENERIC`].
    #[must_use]
    pub fn generic_block(&self) -> bool {
        self.impact.blocked && !self.impact.groups.iter().any(|g| g.blocks)
    }
}

/// The object a delete is about.
#[derive(Clone, Debug)]
struct DeleteTarget {
    table: String,
    key: ObjectKey,
    label: String,
    /// It is the open object (the place is replaced on success).
    open: bool,
    discards_draft: bool,
}

/// A navigation that waits for the answer to "Discard unsaved changes?".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Nav {
    Back,
    Forward,
    Go(Place),
    OpenPath(PathBuf),
    OpenRecent(usize),
    Dropped(Vec<PathBuf>),
    Reload,
    CloseDatabase,
}

/// The pending navigation the leave dialog asks about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeaveRequest {
    /// The edited object's title.
    pub label: String,
    pub action: Nav,
}

/// The reference picker of one field: its own list query, apart from the
/// table lists.
#[derive(Clone, Debug, PartialEq)]
pub struct PickerState {
    pub field: String,
    /// The table listed (the field's target).
    pub table: String,
    pub search: String,
    /// Every row read so far.
    pub items: Vec<ObjectSummary>,
    pub total: u64,
    pub loading: bool,
    pub error: Option<ViewError>,
    seq: u64,
}

/// What a recheck reads: the state, and the open object when there is one.
type RecheckRead = (
    DbmanagerSnapshot,
    Option<Result<ObjectView, DbmanagerError>>,
);

/// Why a recheck runs.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Recheck {
    /// The config was reloaded (by the watcher, or an edit).
    Reload,
    /// A save was refused because the config changed.
    Save,
}

/// What a finished session command did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Opened,
    Reloaded,
    Closed,
}

type Backend = Box<dyn DbmanagerBackend>;
type JobResult<R> = Result<Result<R, DbmanagerError>, JobPanicked>;

fn filter(name: &str, patterns: &[&str]) -> FileFilter {
    FileFilter {
        name: name.to_string(),
        patterns: patterns.iter().map(|p| (*p).to_string()).collect(),
    }
}

/// An offset as the session takes it.
fn offset_u32(offset: u64) -> u32 {
    u32::try_from(offset).unwrap_or(u32::MAX)
}

fn non_empty(s: &str) -> Option<String> {
    (!s.is_empty()).then(|| s.to_owned())
}

/// The Database Manager's state and behaviour.
pub struct DbmanagerModel {
    worker: Worker<Backend>,
    snapshot: Option<DbmanagerSnapshot>,
    error: Option<ViewError>,
    error_generation: u64,
    announcement: Option<String>,
    announcement_generation: u64,
    focus_hint: Option<FocusHint>,
    /// Sequence number of the newest session command; older responses are
    /// discarded.
    command_seq: u64,
    pending_commands: usize,
    /// Every request in flight, of any kind.
    inflight: usize,
    /// Source of the sequence numbers of lists, objects and pages.
    next_seq: u64,
    /// The order number of the snapshot applied last. Results can arrive
    /// out of order, so an older snapshot than this is dropped.
    applied_state: u64,
    history: Vec<Place>,
    cursor: usize,
    list_states: HashMap<String, ListState>,
    object: Option<ObjectLoad>,
    object_seq: u64,
    refs: HashMap<String, RefList>,
    m2ms: HashMap<String, M2mState>,
    /// The folder of the database last held, where the open dialog starts.
    last_dir: Option<PathBuf>,
    /// A save or the confirmed delete is running; the drop is refused
    /// meanwhile.
    write_in_progress: bool,
    draft: Option<Draft>,
    /// The config hash the draft was last checked against.
    checked_hash: String,
    picker: Option<PickerState>,
    conflict: Option<ConflictPanel>,
    leave: Option<LeaveRequest>,
    notice: Option<Notice>,
    /// The request a `Busy` answer refused, for "Try again".
    retry: Option<SaveRequest>,
    /// Counts every start-over for a database (see `command_done`).
    reset_count: u64,
    recheck_seq: u64,
    /// Link conflicts waiting for an answer, by relation.
    link_conflicts: Vec<LinkConflict>,
    link_conflict_seq: u64,
    /// A link conflict's one-option re-read is in flight.
    link_reread: bool,
    delete: Option<DeleteState>,
    delete_target: Option<DeleteTarget>,
    delete_seq: u64,
    /// The watcher of the config file, once armed.
    config_watcher: Option<FileWatcher>,
    /// Whether the first snapshot arms the watcher, and whether it did.
    watch: WatchState,
    /// How many times a watcher was started (tests count them).
    #[cfg(test)]
    pub(super) watch_arms: usize,
    /// The watcher's callback sends here; a foreground task reloads.
    config_changes: async_channel::Sender<()>,
}

/// Arming of the config watcher (once per model).
#[derive(Clone, Copy, PartialEq, Eq)]
enum WatchState {
    /// Never armed (fixtures, tests).
    Off,
    /// The first snapshot arms it.
    Wanted,
    /// Arming was started.
    Armed,
}

impl DbmanagerModel {
    /// A model over `backend`; it starts loading the state at once.
    pub fn new(backend: Box<dyn DbmanagerBackend>, cx: &mut Context<Self>) -> Self {
        let worker = Worker::new(backend, cx);
        let (config_changes, changed) = async_channel::unbounded::<()>();
        cx.spawn(async move |this, cx| {
            while changed.recv().await.is_ok() {
                if this.update(cx, Self::config_file_changed).is_err() {
                    break;
                }
            }
        })
        .detach();
        // Edit config shows only in developer mode: a change redraws at once.
        cx.observe_global::<DevMode>(|_, cx| cx.notify()).detach();
        let mut model = Self {
            worker,
            snapshot: None,
            error: None,
            error_generation: 0,
            announcement: None,
            announcement_generation: 0,
            focus_hint: None,
            command_seq: 0,
            pending_commands: 0,
            inflight: 0,
            next_seq: 0,
            applied_state: 0,
            history: vec![Place::Tables],
            cursor: 0,
            list_states: HashMap::new(),
            object: None,
            object_seq: 0,
            refs: HashMap::new(),
            m2ms: HashMap::new(),
            last_dir: None,
            write_in_progress: false,
            draft: None,
            checked_hash: String::new(),
            picker: None,
            conflict: None,
            leave: None,
            notice: None,
            retry: None,
            reset_count: 0,
            recheck_seq: 0,
            link_conflicts: Vec::new(),
            link_conflict_seq: 0,
            link_reread: false,
            delete: None,
            delete_target: None,
            delete_seq: 0,
            config_watcher: None,
            watch: WatchState::Off,
            #[cfg(test)]
            watch_arms: 0,
            config_changes,
        };
        model.fetch_snapshot(cx);
        model
    }

    /// Makes the first snapshot arm the config watcher. The real app sets
    /// this; fixtures and tests leave it off unless they test the watcher.
    #[must_use]
    pub fn with_config_watcher(mut self) -> Self {
        self.watch = WatchState::Wanted;
        self
    }

    // ---- reading ----

    /// The state; `None` until the first load answers.
    #[must_use]
    pub fn snapshot(&self) -> Option<&DbmanagerSnapshot> {
        self.snapshot.as_ref()
    }

    /// The first load, or an open, reload or close, has not answered yet.
    #[must_use]
    pub fn loading(&self) -> bool {
        self.snapshot.is_none() || self.pending_commands > 0
    }

    /// Any request is still in flight (tests wait on this).
    #[must_use]
    pub fn busy(&self) -> bool {
        self.inflight > 0
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

    /// The tables for the table list: not hidden, not pickable, in display
    /// order.
    #[must_use]
    pub fn listed_tables(&self) -> Vec<&TableView> {
        self.snapshot.as_ref().map_or_else(Vec::new, |s| {
            s.tables
                .iter()
                .filter(|t| !matches!(t.mode, TableMode::Hidden | TableMode::Pickable))
                .collect()
        })
    }

    /// One table's view (pickable tables included).
    #[must_use]
    pub fn table_view(&self, table: &str) -> Option<&TableView> {
        self.snapshot
            .as_ref()?
            .tables
            .iter()
            .find(|t| t.name == table)
    }

    /// Why nothing in `table` can be changed, if so: the database's own
    /// reason, the unusable config, or the table's mode (a pickable table is
    /// opened read-only).
    #[must_use]
    pub fn table_read_only_reason(&self, table: &str) -> Option<String> {
        text::table_read_only_reason(self.snapshot.as_ref()?, self.table_view(table)?)
    }

    /// The config's status line (missing, or the fail-closed reason); `None`
    /// for a config that loaded.
    #[must_use]
    pub fn config_notice(&self) -> Option<String> {
        text::config_notice(self.snapshot.as_ref()?)
    }

    /// Whether the window's drop is taken now, or why not.
    ///
    /// # Errors
    /// The reason it is refused.
    pub fn drop_availability(&self) -> Result<(), &'static str> {
        if self.write_in_progress {
            Err(text::REASON_SAVING)
        } else {
            Ok(())
        }
    }

    // ---- loading ----

    /// The tool became the current one: forgets the banner and the
    /// announcement and refreshes the state (cheap). The place, the history
    /// and the lists are kept.
    pub fn activated(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        self.announcement = None;
        self.fetch_snapshot(cx);
        cx.notify();
    }

    /// Runs `job` on the worker; `done` gets its result on the UI thread.
    fn request<R: Send + 'static>(
        &mut self,
        job: impl FnOnce(&mut Backend) -> R + Send + 'static,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, Result<R, JobPanicked>, &mut Context<Self>) + 'static,
    ) {
        self.inflight += 1;
        let task = self.worker.run(job, cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |m, cx| {
                m.inflight -= 1;
                done(m, result, cx);
            })
            .ok();
        })
        .detach();
    }

    fn next_seq(&mut self) -> u64 {
        self.next_seq += 1;
        self.next_seq
    }

    pub(super) fn fetch_snapshot(&mut self, cx: &mut Context<Self>) {
        let order = self.next_seq();
        self.request(
            |b| b.session(),
            cx,
            move |m, result, cx| match result {
                Ok(snapshot) => m.snapshot_arrived(order, snapshot, cx),
                Err(e) => m.fail(e.into(), cx),
            },
        );
    }

    fn snapshot_arrived(
        &mut self,
        order: u64,
        snapshot: DbmanagerSnapshot,
        cx: &mut Context<Self>,
    ) {
        if order < self.applied_state {
            return;
        }
        self.applied_state = order;
        // A different database (or none, then one) than the model's: the
        // restored database, or the backend's state after commands that did
        // not all show. Shown without taking focus.
        let changed = self.differs(&snapshot);
        self.remember_dir(&snapshot);
        self.snapshot = Some(snapshot);
        if changed {
            self.reset_for_database(false, cx);
        }
        // Only a model set up with `with_config_watcher` watches (a real
        // watcher on every fixture would reload behind unrelated tests).
        if self.watch == WatchState::Wanted {
            self.arm_config_watcher(cx);
        }
        cx.notify();
    }

    /// `snapshot` names another database, or another open state, than the
    /// model's (or the model has none yet).
    fn differs(&self, snapshot: &DbmanagerSnapshot) -> bool {
        self.snapshot.as_ref().is_none_or(|old| {
            old.open != snapshot.open
                || old.database.as_ref().map(|d| &d.path)
                    != snapshot.database.as_ref().map(|d| &d.path)
        })
    }

    fn remember_dir(&mut self, snapshot: &DbmanagerSnapshot) {
        let dir = snapshot
            .database
            .as_ref()
            .and_then(|slot| slot.path.parent())
            .filter(|d| !d.as_os_str().is_empty());
        if let Some(dir) = dir {
            self.last_dir = Some(dir.to_path_buf());
        }
    }

    fn fail(&mut self, error: ViewError, cx: &mut Context<Self>) {
        self.error = Some(error);
        self.error_generation += 1;
        self.focus_hint = Some(FocusHint::ErrorBanner);
        cx.notify();
    }

    fn fail_db(&mut self, error: DbmanagerError, cx: &mut Context<Self>) {
        let config_location = self
            .snapshot
            .as_ref()
            .and_then(|s| s.config.display.clone());
        self.fail(
            ViewError::Dbmanager {
                error,
                config_location,
            },
            cx,
        );
    }

    fn announce(&mut self, sentence: String, cx: &mut Context<Self>) {
        self.announcement = Some(sentence);
        self.announcement_generation += 1;
        cx.notify();
    }

    // ---- opening ----

    /// Shows the file picker (SQLite databases first, then all files,
    /// starting in the folder of the database last held) and opens the pick.
    /// A cancel changes nothing; a dialog failure is the banner.
    pub fn open(&mut self, cx: &mut Context<Self>) {
        let spec = OpenSpec {
            kind: PickKind::File,
            filters: vec![
                filter(
                    text::FILTER_DATABASE,
                    &["*.db", "*.sqlite", "*.sqlite3", "*.db3"],
                ),
                filter(text::FILTER_ALL, &["*.*"]),
            ],
            start_dir: self.last_dir.clone(),
            title: None,
        };
        let pick = platform(cx).pick(spec, cx);
        cx.spawn(async move |this, cx| {
            let result = pick.await;
            this.update(cx, |m, cx| match result {
                Ok(Some(path)) => m.open_path(path, cx),
                Ok(None) => {}
                Err(PickError::Failed { detail }) => m.fail(ViewError::Dialog { detail }, cx),
            })
            .ok();
        })
        .detach();
    }

    /// Opens the database at `path` (the picked file).
    pub fn open_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.intercept(Nav::OpenPath(path.clone()), cx) {
            return;
        }
        self.command(move |b| b.open(&path), Outcome::Opened, cx);
    }

    /// Opens the recent database at `index` of the snapshot's `recent` list.
    pub fn open_recent(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.intercept(Nav::OpenRecent(index), cx) {
            return;
        }
        self.command(move |b| b.open_recent(index), Outcome::Opened, cx);
    }

    /// Files dropped on the window. Ignored while a write runs.
    pub fn dropped(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if self.drop_availability().is_err() || self.intercept(Nav::Dropped(paths.clone()), cx) {
            return;
        }
        self.command(move |b| b.open_dropped(paths), Outcome::Opened, cx);
    }

    /// Reloads the config and the database.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        if self.intercept(Nav::Reload, cx) {
            return;
        }
        self.command(|b| b.reload(), Outcome::Reloaded, cx);
    }

    /// Closes the database; the config stays.
    pub fn close_database(&mut self, cx: &mut Context<Self>) {
        if self.intercept(Nav::CloseDatabase, cx) {
            return;
        }
        self.command(|b| Ok(b.close()), Outcome::Closed, cx);
    }

    /// Runs a session command. Only the newest command's response is shown.
    fn command(
        &mut self,
        job: impl FnOnce(&mut Backend) -> Result<DbmanagerSnapshot, DbmanagerError> + Send + 'static,
        outcome: Outcome,
        cx: &mut Context<Self>,
    ) {
        self.command_seq += 1;
        let seq = self.command_seq;
        let order = self.next_seq();
        let resets = self.reset_count;
        self.pending_commands += 1;
        self.request(job, cx, move |m, result, cx| {
            m.pending_commands -= 1;
            m.command_done(seq, order, result, outcome, resets, cx);
        });
        cx.notify();
    }

    fn command_done(
        &mut self,
        seq: u64,
        order: u64,
        result: JobResult<DbmanagerSnapshot>,
        outcome: Outcome,
        resets_at_start: u64,
        cx: &mut Context<Self>,
    ) {
        if seq != self.command_seq {
            // Superseded: nothing of it is shown, but the worker ran it in
            // order, so the backend's state is already changed. A failure or
            // panic here changes nothing, and the newest result follows.
            if let Ok(Ok(snapshot)) = result {
                self.snapshot_arrived(order, snapshot, cx);
            }
            return;
        }
        match result {
            Ok(Ok(snapshot)) => {
                let sentence = match outcome {
                    Outcome::Opened => snapshot
                        .database
                        .as_ref()
                        .map(|d| text::opened_sentence(&d.display)),
                    Outcome::Reloaded => Some(text::RELOADED_SENTENCE.to_owned()),
                    Outcome::Closed => Some(text::CLOSED_SENTENCE.to_owned()),
                };
                self.error = None;
                // A snapshot older than the applied one (a later fetch
                // answered first) is not applied; the answer still shows.
                if order >= self.applied_state {
                    self.applied_state = order;
                    let changed = self.differs(&snapshot);
                    self.remember_dir(&snapshot);
                    self.snapshot = Some(snapshot);
                    match outcome {
                        Outcome::Opened | Outcome::Closed => self.reset_for_database(true, cx),
                        Outcome::Reloaded if changed => self.reset_for_database(false, cx),
                        Outcome::Reloaded => self.enter(false, cx),
                    }
                } else if outcome != Outcome::Reloaded && self.reset_count != resets_at_start {
                    // A later fetch was applied first and already started the
                    // lists over for this open; it stands in for the open's own
                    // reset, so the focus the open would have raised is raised
                    // here.
                    self.focus_hint = Some(match self.place() {
                        Place::Object { .. } | Place::New { .. } => FocusHint::SheetHeading,
                        Place::Tables | Place::Table { .. } => FocusHint::ListHeading,
                    });
                }
                if let Some(sentence) = sentence {
                    self.announce(sentence, cx);
                }
                cx.notify();
            }
            Ok(Err(e)) => {
                // Superseded commands before this one may have taken effect:
                // fetch the backend's state rather than keep the old copy.
                self.fail_db(e, cx);
                self.fetch_snapshot(cx);
            }
            Err(e) => {
                // The backend's state is unknown: fetch it again, keep the
                // banner.
                self.fail(e.into(), cx);
                self.fetch_snapshot(cx);
            }
        }
    }

    // ---- history ----

    /// Every place, oldest first, ending at the newest.
    #[must_use]
    pub fn history(&self) -> &[Place] {
        &self.history
    }

    /// The current place.
    #[must_use]
    pub fn place(&self) -> &Place {
        &self.history[self.cursor]
    }

    /// There is an earlier place.
    #[must_use]
    pub fn can_back(&self) -> bool {
        self.cursor > 0
    }

    /// There is a later place.
    #[must_use]
    pub fn can_forward(&self) -> bool {
        self.cursor + 1 < self.history.len()
    }

    /// Goes to the previous place. Asks first when a draft has changes.
    pub fn back(&mut self, cx: &mut Context<Self>) {
        if self.can_back() && !self.intercept(Nav::Back, cx) {
            self.back_now(cx);
        }
    }

    fn back_now(&mut self, cx: &mut Context<Self>) {
        if self.can_back() {
            self.cursor -= 1;
            self.enter(true, cx);
        }
    }

    /// Goes to the next place. Asks first when a draft has changes.
    pub fn forward(&mut self, cx: &mut Context<Self>) {
        if self.can_forward() && !self.intercept(Nav::Forward, cx) {
            self.forward_now(cx);
        }
    }

    fn forward_now(&mut self, cx: &mut Context<Self>) {
        if self.can_forward() {
            self.cursor += 1;
            self.enter(true, cx);
        }
    }

    /// Shows `table`'s list (a push). Ignored for a table that is not in the
    /// snapshot, and when it is already the place.
    pub fn choose_table(&mut self, table: &str, cx: &mut Context<Self>) {
        if self.table_view(table).is_some() {
            self.go(
                Place::Table {
                    table: table.to_owned(),
                },
                cx,
            );
        }
    }

    /// Opens one object (a push). A pickable table's object opens read-only
    /// (see [`DbmanagerModel::table_read_only_reason`]).
    pub fn open_object(&mut self, table: &str, key: ObjectKey, cx: &mut Context<Self>) {
        if self.table_view(table).is_none() {
            return;
        }
        let label = self
            .list_states
            .get(table)
            .and_then(|l| l.page.as_ref())
            .and_then(|p| p.items.iter().find(|i| i.key == key))
            .map(|i| i.label.clone())
            .unwrap_or_default();
        self.go(
            Place::Object {
                table: table.to_owned(),
                key,
                label,
            },
            cx,
        );
    }

    /// Follows the reference in the current object's field `fk_field` to the
    /// object it names (a push). Does nothing if the field has no reference
    /// that can be followed.
    pub fn follow(&mut self, fk_field: &str, cx: &mut Context<Self>) {
        let Some(ObjectLoad::Loaded(view)) = &self.object else {
            return;
        };
        let Some(link) = view
            .fields
            .iter()
            .find(|f| f.column == fk_field)
            .and_then(|f| f.link.as_ref())
        else {
            return;
        };
        let (true, Some(key)) = (link.navigable, link.key.clone()) else {
            return;
        };
        let place = Place::Object {
            table: link.table.clone(),
            key,
            label: link.label.clone().unwrap_or_default(),
        };
        if self.table_view(&link.table).is_some() {
            self.go(place, cx);
        }
    }

    /// Pushes `place` unless it is the current one, asking first when a draft
    /// has changes.
    fn go(&mut self, place: Place, cx: &mut Context<Self>) {
        if *self.place() == place || self.intercept(Nav::Go(place.clone()), cx) {
            return;
        }
        self.push(place, cx);
    }

    /// Pushes `place`: forward entries are dropped, the history is capped
    /// (the oldest goes), and a place equal to the current one is not pushed
    /// again.
    fn push(&mut self, place: Place, cx: &mut Context<Self>) {
        if *self.place() == place {
            return;
        }
        self.history.truncate(self.cursor + 1);
        self.history.push(place);
        if self.history.len() > MAX_HISTORY {
            self.history.remove(0);
        }
        self.cursor = self.history.len() - 1;
        self.enter(true, cx);
    }

    /// A database was opened or closed: the lists and history start over at
    /// the first listed table, or the table list if none.
    fn reset_for_database(&mut self, hint: bool, cx: &mut Context<Self>) {
        self.reset_count += 1;
        self.list_states.clear();
        let first = self.listed_tables().first().map(|t| t.name.clone());
        self.history = vec![first.map_or(Place::Tables, |table| Place::Table { table })];
        self.cursor = 0;
        self.enter(hint, cx);
    }

    /// Shows the current place: forgets the previous object's pages, reads
    /// what the place shows, and raises the focus hint when `hint`.
    fn enter(&mut self, hint: bool, cx: &mut Context<Self>) {
        self.object = None;
        self.object_seq = self.next_seq();
        self.refs.clear();
        self.m2ms.clear();
        self.draft = None;
        self.picker = None;
        self.conflict = None;
        self.link_conflicts.clear();
        // A link conflict's one-option re-read still in flight belongs to the
        // place being left: its answer is stale and is dropped.
        self.link_conflict_seq += 1;
        self.link_reread = false;
        self.delete = None;
        self.delete_target = None;
        self.leave = None;
        self.retry = None;
        let focus = match self.place().clone() {
            Place::Tables => FocusHint::ListHeading,
            Place::Table { table } => {
                self.fetch_list(&table, cx);
                FocusHint::ListHeading
            }
            Place::Object { table, key, .. } => {
                self.fetch_object(table, key, cx);
                FocusHint::SheetHeading
            }
            Place::New { table } => {
                self.start_new_draft(&table, cx);
                FocusHint::SheetHeading
            }
        };
        if hint {
            self.focus_hint = Some(focus);
        }
        cx.notify();
    }

    // ---- lists ----

    /// A table's list state, once it has been shown.
    #[must_use]
    pub fn list_state(&self, table: &str) -> Option<&ListState> {
        self.list_states.get(table)
    }

    /// Searches `table`'s list (an empty text clears the search) and goes to
    /// its first page.
    pub fn set_search(&mut self, table: &str, text: &str, cx: &mut Context<Self>) {
        let state = self.list_states.entry(table.to_owned()).or_default();
        if state.search == text && state.offset == 0 && state.page.is_some() {
            return;
        }
        text.clone_into(&mut state.search);
        state.offset = 0;
        self.fetch_list(table, cx);
    }

    /// Shows the page of `table`'s list starting at `offset`.
    pub fn set_offset(&mut self, table: &str, offset: u64, cx: &mut Context<Self>) {
        let state = self.list_states.entry(table.to_owned()).or_default();
        state.offset = offset;
        self.fetch_list(table, cx);
    }

    fn fetch_list(&mut self, table: &str, cx: &mut Context<Self>) {
        let seq = self.next_seq();
        let state = self.list_states.entry(table.to_owned()).or_default();
        state.seq = seq;
        state.loading = true;
        let query = ListQuery {
            search: non_empty(&state.search),
            offset: offset_u32(state.offset),
            limit: PAGE_SIZE,
            referencing: None,
        };
        let name = table.to_owned();
        let asked = name.clone();
        self.request(
            move |b| b.list(&asked, &query),
            cx,
            move |m, result, cx| m.list_done(&name, seq, result, cx),
        );
        cx.notify();
    }

    fn list_done(
        &mut self,
        table: &str,
        seq: u64,
        result: JobResult<ObjectPage>,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.list_states.get_mut(table) else {
            return;
        };
        if state.seq != seq {
            return;
        }
        state.loading = false;
        match result {
            Ok(Ok(page)) => state.page = Some(page),
            Ok(Err(e)) => self.fail_db(e, cx),
            Err(e) => self.fail(e.into(), cx),
        }
        cx.notify();
    }

    // ---- the object ----

    /// The current place's object, when the place is one.
    #[must_use]
    pub fn object(&self) -> Option<&ObjectLoad> {
        self.object.as_ref()
    }

    fn fetch_object(&mut self, table: String, key: ObjectKey, cx: &mut Context<Self>) {
        let seq = self.next_seq();
        self.object_seq = seq;
        self.object = Some(ObjectLoad::Loading);
        let asked = table.clone();
        self.request(
            move |b| b.object(&asked, &key),
            cx,
            move |m, result, cx| m.object_done(&table, seq, result, cx),
        );
    }

    fn object_done(
        &mut self,
        table: &str,
        seq: u64,
        result: JobResult<ObjectView>,
        cx: &mut Context<Self>,
    ) {
        if self.object_seq != seq {
            return;
        }
        match result {
            Ok(Ok(view)) => self.apply_view(table, view, false, cx),
            Ok(Err(e)) if e.kind == DbmanagerErrorKind::UnknownObject => {
                self.object = Some(ObjectLoad::Gone);
            }
            Ok(Err(e)) => {
                self.object = Some(ObjectLoad::Failed);
                self.fail_db(e, cx);
            }
            Err(e) => {
                self.object = Some(ObjectLoad::Failed);
                self.fail(e.into(), cx);
            }
        }
        cx.notify();
    }

    /// Shows `view` as the current object: fills in the place's label,
    /// starts a clean draft when the table can be changed, and reads the
    /// many-to-many pages (`keep_m2m`: the open ones keep their query and
    /// are read again).
    fn apply_view(
        &mut self,
        table: &str,
        view: ObjectView,
        keep_m2m: bool,
        cx: &mut Context<Self>,
    ) {
        if let Place::Object {
            table: t,
            key: k,
            label,
        } = &mut self.history[self.cursor]
        {
            if t == table && *k == view.key {
                label.clone_from(&view.label);
            }
        }
        self.draft = None;
        self.conflict = None;
        self.link_conflicts.clear();
        if self.table_read_only_reason(table).is_none() {
            if let Some(tv) = self.table_view(table) {
                self.draft = Some(Draft::for_object(tv, &view));
                self.checked_hash = self.config_hash();
            }
        }
        self.object = Some(ObjectLoad::Loaded(view));
        self.init_m2m(table, keep_m2m, cx);
    }

    /// Starts the many-to-many states of `table`'s relations and reads
    /// their first pages (`keep`: the open ones keep their query).
    fn init_m2m(&mut self, table: &str, keep_m2m: bool, cx: &mut Context<Self>) {
        let relations: Vec<(String, bool)> = self
            .table_view(table)
            .map(|t| {
                t.many_to_many
                    .iter()
                    .map(|m| {
                        (
                            m.relation.clone(),
                            m.mode == fm_dbmanager::config::RelationMode::ReadOnly,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        for (relation, linked_only) in relations {
            if !keep_m2m || !self.m2ms.contains_key(&relation) {
                self.m2ms.insert(
                    relation.clone(),
                    M2mState {
                        linked_only,
                        ..M2mState::default()
                    },
                );
            }
            self.fetch_m2m(&relation, cx);
        }
    }

    fn config_hash(&self) -> String {
        self.snapshot
            .as_ref()
            .map(|s| s.config.hash.clone())
            .unwrap_or_default()
    }

    // ---- referenced by ----

    /// The "referenced by" entry `relation` of the current object; `None`
    /// until it has been opened.
    #[must_use]
    pub fn referenced_by(&self, relation: &str) -> Option<&RefList> {
        self.refs.get(relation)
    }

    /// Opens the entry and reads its first page. Does nothing when the
    /// object is not read yet, the relation is unknown, or its count is 0.
    pub fn open_referenced_by(&mut self, relation: &str, cx: &mut Context<Self>) {
        let Some(ObjectLoad::Loaded(view)) = &self.object else {
            return;
        };
        let count = view
            .referenced_by
            .iter()
            .find(|c| c.relation == relation)
            .map_or(0, |c| c.count);
        let referencing_table = self
            .table_view(&view.table)
            .and_then(|t| t.referenced_by.iter().find(|r| r.relation == relation))
            .map(|r| r.table.clone());
        let (true, Some(referencing_table)) = (count > 0, referencing_table) else {
            return;
        };
        let key = view.key.clone();
        self.refs.insert(
            relation.to_owned(),
            RefList {
                open: true,
                ..RefList::default()
            },
        );
        self.fetch_refs(relation, referencing_table, key, 0, cx);
    }

    /// Closes the entry, forgetting what it read.
    pub fn close_referenced_by(&mut self, relation: &str, cx: &mut Context<Self>) {
        if self.refs.remove(relation).is_some() {
            cx.notify();
        }
    }

    /// Appends the next page to an open entry.
    pub fn show_more_referenced_by(&mut self, relation: &str, cx: &mut Context<Self>) {
        let Some(list) = self.refs.get(relation) else {
            return;
        };
        let loaded = list.items.len() as u64;
        if !list.open || list.loading || loaded >= list.total {
            return;
        }
        let Some(ObjectLoad::Loaded(view)) = &self.object else {
            return;
        };
        let key = view.key.clone();
        let Some(table) = self
            .table_view(&view.table)
            .and_then(|t| t.referenced_by.iter().find(|r| r.relation == relation))
            .map(|r| r.table.clone())
        else {
            return;
        };
        self.fetch_refs(relation, table, key, loaded, cx);
    }

    fn fetch_refs(
        &mut self,
        relation: &str,
        table: String,
        key: ObjectKey,
        offset: u64,
        cx: &mut Context<Self>,
    ) {
        let seq = self.next_seq();
        let Some(list) = self.refs.get_mut(relation) else {
            return;
        };
        list.seq = seq;
        list.loading = true;
        let query = ListQuery {
            search: None,
            offset: offset_u32(offset),
            limit: PAGE_SIZE,
            referencing: Some(ReferencingFilter {
                relation: relation.to_owned(),
                key,
            }),
        };
        let name = relation.to_owned();
        self.request(
            move |b| b.list(&table, &query),
            cx,
            move |m, result, cx| m.refs_done(&name, seq, offset, result, cx),
        );
        cx.notify();
    }

    fn refs_done(
        &mut self,
        relation: &str,
        seq: u64,
        offset: u64,
        result: JobResult<ObjectPage>,
        cx: &mut Context<Self>,
    ) {
        let Some(list) = self.refs.get_mut(relation) else {
            return;
        };
        if list.seq != seq {
            return;
        }
        list.loading = false;
        match result {
            Ok(Ok(page)) => {
                if offset == 0 {
                    list.items = page.items;
                } else {
                    list.items.extend(page.items);
                }
                list.total = page.total;
            }
            Ok(Err(e)) => self.fail_db(e, cx),
            Err(e) => self.fail(e.into(), cx),
        }
        cx.notify();
    }

    // ---- many-to-many ----

    /// The many-to-many relation `relation` of the current object: its query
    /// and page. `linked_only` starts on for read-only relations and off for
    /// editable ones.
    #[must_use]
    pub fn m2m(&self, relation: &str) -> Option<&M2mState> {
        self.m2ms.get(relation)
    }

    /// Changes the relation's query and reads the page it names.
    pub fn set_m2m_query(
        &mut self,
        relation: &str,
        search: &str,
        linked_only: bool,
        offset: u64,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.m2ms.get_mut(relation) else {
            return;
        };
        search.clone_into(&mut state.search);
        state.linked_only = linked_only;
        state.offset = offset;
        self.fetch_m2m(relation, cx);
    }

    fn fetch_m2m(&mut self, relation: &str, cx: &mut Context<Self>) {
        let (table, key) = match self.place().clone() {
            Place::Object { table, key, .. } => (table, Some(key)),
            Place::New { table } => (table, None),
            _ => return,
        };
        let seq = self.next_seq();
        let Some(state) = self.m2ms.get_mut(relation) else {
            return;
        };
        state.seq = seq;
        state.loading = true;
        let query = M2mQuery {
            search: non_empty(&state.search),
            offset: offset_u32(state.offset),
            limit: PAGE_SIZE,
            linked_only: state.linked_only,
            only: None,
        };
        let name = relation.to_owned();
        let asked = name.clone();
        self.request(
            move |b| b.m2m_options(&table, key.as_ref(), &asked, &query),
            cx,
            move |m, result, cx| m.m2m_done(&name, seq, result, cx),
        );
        cx.notify();
    }

    fn m2m_done(
        &mut self,
        relation: &str,
        seq: u64,
        result: JobResult<M2mPage>,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.m2ms.get_mut(relation) else {
            return;
        };
        if state.seq != seq {
            return;
        }
        state.loading = false;
        match result {
            Ok(Ok(page)) => state.page = Some(page),
            Ok(Err(e)) => self.fail_db(e, cx),
            Err(e) => self.fail(e.into(), cx),
        }
        cx.notify();
    }
}

// ---- editing ----

impl DbmanagerModel {
    /// The draft of the open or new object; `None` when the place has none
    /// (a list, a read-only table, an object not read yet).
    #[must_use]
    pub fn draft(&self) -> Option<&Draft> {
        self.draft.as_ref()
    }

    /// The draft has changed fields or staged link changes.
    #[must_use]
    pub fn dirty(&self) -> bool {
        self.draft.as_ref().is_some_and(Draft::dirty)
    }

    /// How many fields are changed plus how many link changes are staged.
    #[must_use]
    pub fn dirty_count(&self) -> usize {
        self.draft.as_ref().map_or(0, Draft::dirty_count)
    }

    /// A save is running: editors are read-only, navigation and other writes
    /// are refused, and the drop is disabled.
    #[must_use]
    pub fn saving(&self) -> bool {
        self.write_in_progress
    }

    /// The title of the object being edited: the object's label, or the new
    /// object's title.
    #[must_use]
    pub fn draft_label(&self) -> String {
        match self.place() {
            Place::Object { label, .. } => match (&self.object, label.is_empty()) {
                (Some(ObjectLoad::Loaded(v)), true) => v.label.clone(),
                _ => label.clone(),
            },
            Place::New { table } => text::new_title(
                self.table_view(table)
                    .and_then(|t| t.object_name.as_deref()),
            ),
            _ => String::new(),
        }
    }

    /// The close guard's item while the draft has changes.
    #[must_use]
    pub fn unsaved_description(&self) -> Option<String> {
        self.dirty()
            .then(|| text::unsaved_item(&self.draft_label()))
    }

    /// Why Save does nothing now, if so: the conflict panel is up (resolve it
    /// first) or a save is running.
    #[must_use]
    pub fn save_blocked_reason(&self) -> Option<&'static str> {
        if self.conflict.is_some() {
            Some(text::REASON_CONFLICT)
        } else if self.delete.is_some() {
            Some(text::REASON_DELETE)
        } else if self.write_in_progress {
            Some(text::REASON_SAVING)
        } else if self.link_reread {
            Some(text::REASON_LINK_CHECK)
        } else {
            None
        }
    }

    /// The message under the sheet (backup, config reloaded, delete done).
    /// It stays until the user dismisses it, the next successful save clears
    /// it (and sets the backup notice again only when that save made the
    /// backup), or anything else that has a notice replaces it.
    #[must_use]
    pub fn notice(&self) -> Option<&Notice> {
        self.notice.as_ref()
    }

    /// Hides the notice.
    pub fn dismiss_notice(&mut self, cx: &mut Context<Self>) {
        if self.notice.take().is_some() {
            cx.notify();
        }
    }

    /// The conflict panel's content.
    #[must_use]
    pub fn conflict(&self) -> Option<&ConflictPanel> {
        self.conflict.as_ref()
    }

    /// The pending navigation the leave dialog asks about.
    #[must_use]
    pub fn leave_request(&self) -> Option<&LeaveRequest> {
        self.leave.as_ref()
    }

    /// A save was refused as busy and can be sent again unchanged.
    #[must_use]
    pub fn can_try_again(&self) -> bool {
        self.retry.is_some() && !self.write_in_progress
    }

    /// Asks before leaving a dirty draft. Returns whether `action` was held
    /// back: for the question, or because a save is running.
    fn intercept(&mut self, action: Nav, cx: &mut Context<Self>) -> bool {
        if self.write_in_progress || self.delete.is_some() {
            return true;
        }
        if !self.dirty() {
            return false;
        }
        self.leave = Some(LeaveRequest {
            label: self.draft_label(),
            action,
        });
        cx.notify();
        true
    }

    /// The user chose "Discard changes" in the leave dialog: the draft goes
    /// and the navigation happens.
    pub fn confirm_leave(&mut self, cx: &mut Context<Self>) {
        let Some(request) = self.leave.take() else {
            return;
        };
        self.draft = None;
        self.conflict = None;
        match request.action {
            Nav::Back => self.back_now(cx),
            Nav::Forward => self.forward_now(cx),
            Nav::Go(place) => self.push(place, cx),
            Nav::OpenPath(path) => self.open_path(path, cx),
            Nav::OpenRecent(index) => self.open_recent(index, cx),
            Nav::Dropped(paths) => self.dropped(paths, cx),
            Nav::Reload => self.reload(cx),
            Nav::CloseDatabase => self.close_database(cx),
        }
        cx.notify();
    }

    /// The user chose "Keep editing": nothing changes.
    pub fn cancel_leave(&mut self, cx: &mut Context<Self>) {
        if self.leave.take().is_some() {
            cx.notify();
        }
    }

    // ---- fields ----

    /// Sets a field from its editor. Ignored while a save runs.
    pub fn edit(&mut self, field: &str, input: EditorInput, cx: &mut Context<Self>) {
        if self.write_in_progress {
            return;
        }
        if let Some(draft) = self.draft.as_mut() {
            draft.set(field, input);
            cx.notify();
        }
    }

    /// The field lost focus: its issues show, and a valid duration takes its
    /// canonical form.
    pub fn blur(&mut self, field: &str, cx: &mut Context<Self>) {
        if let Some(draft) = self.draft.as_mut() {
            draft.blur(field);
            cx.notify();
        }
    }

    /// Puts one field back to its stored value (Escape in a changed field).
    pub fn revert_field(&mut self, field: &str, cx: &mut Context<Self>) {
        if self.write_in_progress {
            return;
        }
        if let Some(draft) = self.draft.as_mut() {
            draft.revert(field);
            cx.notify();
        }
    }

    /// Reverts every field without asking.
    pub fn discard(&mut self, cx: &mut Context<Self>) {
        if self.write_in_progress {
            return;
        }
        if let Some(draft) = self.draft.as_mut() {
            draft.revert_all();
            self.retry = None;
            cx.notify();
        }
    }

    // ---- the reference picker ----

    /// The open picker.
    #[must_use]
    pub fn picker(&self) -> Option<&PickerState> {
        self.picker.as_ref()
    }

    /// Opens the picker of reference field `field` and reads its first page.
    pub fn open_picker(&mut self, field: &str, cx: &mut Context<Self>) {
        if self.write_in_progress {
            return;
        }
        let Some(target) = self
            .draft
            .as_ref()
            .and_then(|d| d.field_view(field))
            .and_then(|f| f.foreign_key.as_ref())
            .map(|fk| fk.table.clone())
        else {
            return;
        };
        if self.table_view(&target).is_none() {
            return;
        }
        self.picker = Some(PickerState {
            field: field.to_owned(),
            table: target,
            search: String::new(),
            items: Vec::new(),
            total: 0,
            loading: false,
            error: None,
            seq: 0,
        });
        self.fetch_picker(0, cx);
    }

    /// Searches the picker's list from its first page.
    pub fn picker_search(&mut self, text: &str, cx: &mut Context<Self>) {
        let Some(picker) = self.picker.as_mut() else {
            return;
        };
        text.clone_into(&mut picker.search);
        self.fetch_picker(0, cx);
    }

    /// Appends 25 more rows to the picker.
    pub fn picker_more(&mut self, cx: &mut Context<Self>) {
        let Some(picker) = self.picker.as_ref() else {
            return;
        };
        let loaded = picker.items.len() as u64;
        if picker.loading || loaded >= picker.total {
            return;
        }
        self.fetch_picker(loaded, cx);
    }

    fn fetch_picker(&mut self, offset: u64, cx: &mut Context<Self>) {
        let seq = self.next_seq();
        let Some(picker) = self.picker.as_mut() else {
            return;
        };
        picker.seq = seq;
        picker.loading = true;
        picker.error = None;
        let query = ListQuery {
            search: non_empty(&picker.search),
            offset: offset_u32(offset),
            limit: PAGE_SIZE,
            referencing: None,
        };
        let table = picker.table.clone();
        self.request(
            move |b| b.list(&table, &query),
            cx,
            move |m, result, cx| m.picker_done(seq, offset, result, cx),
        );
        cx.notify();
    }

    fn picker_done(
        &mut self,
        seq: u64,
        offset: u64,
        result: JobResult<ObjectPage>,
        cx: &mut Context<Self>,
    ) {
        let config_location = self
            .snapshot
            .as_ref()
            .and_then(|s| s.config.display.clone());
        let Some(picker) = self.picker.as_mut() else {
            return;
        };
        if picker.seq != seq {
            return;
        }
        picker.loading = false;
        match result {
            Ok(Ok(page)) => {
                if offset == 0 {
                    picker.items = page.items;
                } else {
                    picker.items.extend(page.items);
                }
                picker.total = page.total;
            }
            Ok(Err(error)) => {
                picker.error = Some(ViewError::Dbmanager {
                    error,
                    config_location,
                });
            }
            Err(e) => picker.error = Some(e.into()),
        }
        cx.notify();
    }

    /// Stages the picked object in the picker's field, closes the picker and
    /// hints its trigger.
    pub fn pick(&mut self, key: ObjectKey, cx: &mut Context<Self>) {
        let Some(picker) = self.picker.take() else {
            return;
        };
        let label = picker
            .items
            .iter()
            .find(|i| i.key == key)
            .map(|i| i.label.clone())
            .unwrap_or_default();
        if let Some(draft) = self.draft.as_mut() {
            draft.set_reference(&picker.field, Some(Pick { key, label }));
        }
        self.focus_hint = Some(FocusHint::PickerTrigger(picker.field));
        cx.notify();
    }

    /// Stages NULL in a nullable reference field.
    pub fn clear_reference(&mut self, field: &str, cx: &mut Context<Self>) {
        if self.write_in_progress {
            return;
        }
        let Some(draft) = self.draft.as_mut() else {
            return;
        };
        if draft.field_view(field).is_some_and(|f| f.rule.nullable) {
            draft.set_reference(field, None);
            if self.picker.as_ref().is_some_and(|p| p.field == field) {
                self.picker = None;
            }
            cx.notify();
        }
    }

    /// Closes the picker without picking and hints its trigger.
    pub fn close_picker(&mut self, cx: &mut Context<Self>) {
        if let Some(picker) = self.picker.take() {
            self.focus_hint = Some(FocusHint::PickerTrigger(picker.field));
            cx.notify();
        }
    }

    // ---- save ----

    /// Saves the draft (Ctrl+S). Refused with every issue shown when a
    /// changed field has issues or a required field is missing; nothing is
    /// sent then.
    pub fn save(&mut self, cx: &mut Context<Self>) {
        if self.save_blocked_reason().is_some() {
            return;
        }
        let Some(draft) = self.draft.as_mut() else {
            return;
        };
        draft.attempt();
        if !draft.can_save() {
            self.focus_hint = Some(FocusHint::ErrorSummary);
            cx.notify();
            return;
        }
        if !draft.is_new() && !draft.dirty() {
            cx.notify();
            return;
        }
        let request = draft.to_request(&self.checked_hash);
        self.send_save(request, cx);
    }

    /// Sends the request a `Busy` answer refused again, unchanged.
    pub fn try_again(&mut self, cx: &mut Context<Self>) {
        if self.write_in_progress {
            return;
        }
        if let Some(request) = self.retry.take() {
            self.error = None;
            self.send_save(request, cx);
        }
    }

    fn send_save(&mut self, request: SaveRequest, cx: &mut Context<Self>) {
        self.write_in_progress = true;
        self.picker = None;
        self.retry = None;
        self.error = None;
        let sent = request.clone();
        self.request(
            move |b| b.save(request),
            cx,
            move |m, result, cx| {
                m.write_in_progress = false;
                m.save_done(sent, result, cx);
            },
        );
        cx.notify();
    }

    fn save_done(
        &mut self,
        request: SaveRequest,
        result: JobResult<SaveResult>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(Ok(saved)) => self.saved(request, saved, cx),
            Ok(Err(e)) => self.save_failed(request, e, cx),
            Err(e) => {
                // What happened to the write is unknown: say so, and read
                // the backend's state again.
                self.fail(e.into(), cx);
                self.fetch_snapshot(cx);
            }
        }
        cx.notify();
    }

    fn saved(&mut self, request: SaveRequest, saved: SaveResult, cx: &mut Context<Self>) {
        let table = request.table;
        let created = request.key.is_none();
        let label = saved.object.label.clone();
        self.error = None;
        self.conflict = None;
        self.notice = None;
        self.object_seq = self.next_seq();
        if created {
            self.history[self.cursor] = Place::Object {
                table: table.clone(),
                key: saved.object.key.clone(),
                label: label.clone(),
            };
            self.focus_hint = Some(FocusHint::SheetHeading);
            self.announce(text::created_sentence(&label), cx);
        } else {
            self.announce(text::SAVED_SENTENCE.to_owned(), cx);
        }
        self.apply_view(&table, saved.object, !created, cx);
        if self
            .list_states
            .get(&table)
            .is_some_and(|l| l.page.is_some())
        {
            self.fetch_list(&table, cx);
        }
        if let Some(path) = saved.backup_created {
            self.notice = Some(Notice::Backup { path });
            self.fetch_snapshot(cx);
        }
    }

    fn save_failed(&mut self, request: SaveRequest, e: DbmanagerError, cx: &mut Context<Self>) {
        match e.kind {
            DbmanagerErrorKind::ValidationFailed
                if self.draft.as_mut().is_some_and(|d| {
                    let fields = d.set_server_issues(&e.field_errors);
                    let links = d.links_mut().set_errors(&e.link_errors);
                    fields || links
                }) =>
            {
                self.focus_hint = Some(FocusHint::ErrorSummary);
            }
            DbmanagerErrorKind::Conflict
                if e.conflict_link
                    .as_deref()
                    .is_some_and(|l| self.is_staged(l)) =>
            {
                if let Some(link) = e.conflict_link {
                    self.begin_link_conflict(*link, cx);
                }
            }
            DbmanagerErrorKind::Conflict if e.conflict_link.is_none() && e.current.is_some() => {
                if let (Some(current), Some(draft)) = (e.current, self.draft.as_ref()) {
                    let diff = draft.conflict_diff(&current);
                    let label = match self.draft_label() {
                        l if l.is_empty() => current.label.clone(),
                        l => l,
                    };
                    self.conflict = Some(ConflictPanel {
                        label,
                        current: *current,
                        diff,
                    });
                }
            }
            DbmanagerErrorKind::ConfigChangedOnDisk => {
                // The backend has reloaded the config. Nothing is retried:
                // the draft is checked against the new rules first.
                self.recheck(Recheck::Save, cx);
            }
            DbmanagerErrorKind::Busy => {
                self.retry = Some(request);
                self.fail_db(e, cx);
            }
            _ => {
                self.fail_db(e, cx);
                if matches!(
                    self.error,
                    Some(ViewError::Dbmanager { ref error, .. })
                        if error.kind == DbmanagerErrorKind::BackupFailed
                ) {
                    self.fetch_snapshot(cx);
                }
            }
        }
    }

    // ---- the conflict panel ----

    /// "Re-apply my changes": the draft is laid over the object as it is now;
    /// the user saves again.
    pub fn reapply(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = self.conflict.take() else {
            return;
        };
        let Place::Object { table, .. } = self.place().clone() else {
            return;
        };
        let tv = self.table_view(&table).cloned();
        if let (Some(draft), Some(tv)) = (self.draft.as_mut(), tv) {
            draft.rebase(&tv, Some(&panel.current));
        }
        self.object = Some(ObjectLoad::Loaded(panel.current));
        cx.notify();
    }

    /// "Discard my changes": the object as it is now replaces the draft.
    pub fn discard_mine(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = self.conflict.take() else {
            return;
        };
        let Place::Object { table, .. } = self.place().clone() else {
            return;
        };
        self.object_seq = self.next_seq();
        self.apply_view(&table, panel.current, true, cx);
        cx.notify();
    }

    // ---- create ----

    /// Opens the form for a new object of `table`. Does nothing when the
    /// table cannot create.
    pub fn new_object(&mut self, table: &str, cx: &mut Context<Self>) {
        if self.can_create(table) {
            self.go(
                Place::New {
                    table: table.to_owned(),
                },
                cx,
            );
        }
    }

    /// New objects can be created in `table` now.
    #[must_use]
    pub fn can_create(&self, table: &str) -> bool {
        self.table_view(table).is_some_and(|t| t.can_create)
            && self.table_read_only_reason(table).is_none()
    }

    fn start_new_draft(&mut self, table: &str, cx: &mut Context<Self>) {
        if !self.can_create(table) {
            return;
        }
        if let Some(tv) = self.table_view(table) {
            self.draft = Some(Draft::for_new(tv));
            self.checked_hash = self.config_hash();
            // A new object stages link adds too: its relations list every
            // other-side object, none linked.
            self.init_m2m(table, false, cx);
        }
    }

    /// Cancel on the new-object form: discards the draft and goes Back.
    pub fn cancel_new(&mut self, cx: &mut Context<Self>) {
        if self.write_in_progress || !matches!(self.place(), Place::New { .. }) {
            return;
        }
        self.draft = None;
        if self.can_back() {
            self.back_now(cx);
        } else {
            let first = self.listed_tables().first().map(|t| t.name.clone());
            self.history[self.cursor] = first.map_or(Place::Tables, |table| Place::Table { table });
            self.enter(true, cx);
        }
    }

    // ---- re-checking after a reload ----

    /// The config was reloaded: reads the state and the object again and
    /// checks the draft against them. The same version rebases the draft
    /// (edits to fields that are gone or no longer editable are dropped and
    /// listed); another version raises the conflict panel; a gone object or
    /// table drops the draft with a notice. Part D's watcher calls this.
    pub fn recheck_after_reload(&mut self, cx: &mut Context<Self>) {
        self.recheck(Recheck::Reload, cx);
    }

    fn recheck(&mut self, why: Recheck, cx: &mut Context<Self>) {
        let order = self.next_seq();
        self.recheck_seq += 1;
        let seq = self.recheck_seq;
        let target = match self.place() {
            Place::Object { table, key, .. } => Some((table.clone(), key.clone())),
            _ => None,
        };
        self.request(
            move |b| {
                let snapshot = b.session();
                let object = target.map(|(table, key)| b.object(&table, &key));
                (snapshot, object)
            },
            cx,
            move |m, result, cx| m.recheck_done(seq, order, why, result, cx),
        );
    }

    fn recheck_done(
        &mut self,
        seq: u64,
        order: u64,
        why: Recheck,
        result: Result<RecheckRead, JobPanicked>,
        cx: &mut Context<Self>,
    ) {
        if seq != self.recheck_seq {
            return;
        }
        let (snapshot, object) = match result {
            Ok(pair) => pair,
            Err(e) => {
                self.fail(e.into(), cx);
                return;
            }
        };
        let place_before = self.place().clone();
        self.snapshot_arrived(order, snapshot, cx);
        if *self.place() != place_before || self.snapshot.is_none() {
            // Another database: everything started over already.
            return;
        }
        self.checked_hash = self.config_hash();
        let mut dropped = Vec::new();
        let mut gone: Option<String> = None;
        match self.place().clone() {
            Place::New { table } => {
                let label = self.draft_label();
                let editable = self.can_create(&table);
                match (self.draft.take(), self.table_view(&table).cloned()) {
                    (Some(mut draft), Some(tv)) if editable => {
                        dropped = draft.rebase(&tv, None).dropped;
                        self.draft = Some(draft);
                    }
                    (Some(draft), _) if draft.dirty() => gone = Some(label),
                    _ => {}
                }
            }
            Place::Object { table, label, .. } => match object {
                Some(Ok(view)) => {
                    self.recheck_object(&table, view, &mut dropped, cx);
                }
                Some(Err(e))
                    if matches!(
                        e.kind,
                        DbmanagerErrorKind::UnknownObject | DbmanagerErrorKind::UnknownTable
                    ) =>
                {
                    if self.dirty() {
                        gone = Some(label);
                    }
                    self.draft = None;
                    self.conflict = None;
                    self.object = Some(ObjectLoad::Gone);
                }
                Some(Err(e)) => self.fail_db(e, cx),
                None => {}
            },
            Place::Tables | Place::Table { .. } => {}
        }
        self.notice = Some(match (gone, why) {
            (Some(label), _) => Notice::ConfigReloadedGone { label },
            (None, Recheck::Reload) => Notice::ConfigReloaded { dropped },
            (None, Recheck::Save) => Notice::ConfigChangedWhileEditing { dropped },
        });
        cx.notify();
    }

    /// The object as read again, against the draft.
    fn recheck_object(
        &mut self,
        table: &str,
        view: ObjectView,
        dropped: &mut Vec<String>,
        cx: &mut Context<Self>,
    ) {
        let editable = self.table_read_only_reason(table).is_none();
        let dirty = self.dirty();
        let same_version = self
            .draft
            .as_ref()
            .is_some_and(|d| d.version() == Some(view.version.as_str()));
        match (self.draft.take(), self.table_view(table).cloned()) {
            (Some(draft), Some(_)) if dirty && !editable => {
                *dropped = draft
                    .changed_columns()
                    .iter()
                    .filter_map(|c| draft.field_view(c).map(|f| f.display_name.clone()))
                    .collect();
                self.object = Some(ObjectLoad::Loaded(view));
            }
            (Some(mut draft), Some(tv)) if dirty && same_version => {
                *dropped = draft.rebase(&tv, Some(&view)).dropped;
                self.draft = Some(draft);
                self.object = Some(ObjectLoad::Loaded(view));
            }
            (Some(draft), Some(_)) if dirty => {
                let diff = draft.conflict_diff(&view);
                let label = match self.draft_label() {
                    l if l.is_empty() => view.label.clone(),
                    l => l,
                };
                self.draft = Some(draft);
                self.conflict = Some(ConflictPanel {
                    label,
                    current: view,
                    diff,
                });
            }
            _ => {
                // Nothing edited: show the fresh object with a fresh draft.
                self.object_seq = self.next_seq();
                self.apply_view(table, view, true, cx);
            }
        }
    }

    // ---- many-to-many staging ----

    /// The current table's many-to-many side `relation` with its link
    /// fields, when the draft may change it.
    fn editable_relation(&self, relation: &str) -> Option<Vec<FieldView>> {
        if self.write_in_progress || self.delete.is_some() {
            return None;
        }
        let (Place::Object { table, .. } | Place::New { table }) = self.place() else {
            return None;
        };
        self.draft.as_ref()?;
        let side = self
            .table_view(table)?
            .many_to_many
            .iter()
            .find(|m| m.relation == relation)?;
        (side.mode != RelationMode::ReadOnly).then(|| side.link_fields.clone())
    }

    /// `other`'s row of the relation's current page.
    fn page_option(&self, relation: &str, other: &ObjectKey) -> Option<M2mOption> {
        self.m2ms
            .get(relation)?
            .page
            .as_ref()?
            .items
            .iter()
            .find(|o| &o.key == other)
            .cloned()
    }

    /// Ticks or unticks a row: stages an add (link fields start at their
    /// defaults) or a removal; toggling again undoes it exactly. Nothing is
    /// written until Save.
    pub fn toggle_link(&mut self, relation: &str, other: &ObjectKey, cx: &mut Context<Self>) {
        let Some(views) = self.editable_relation(relation) else {
            return;
        };
        let option = self.page_option(relation, other);
        let Some(draft) = self.draft.as_mut() else {
            return;
        };
        match option {
            Some(option) => draft.links_mut().toggle(relation, &option, &views),
            None => {
                draft.links_mut().flip(relation, other);
            }
        }
        cx.notify();
    }

    /// Edits a link field of a row: on a linked row it stages an update
    /// with the link's version; on a staged add it edits the add.
    pub fn edit_link_field(
        &mut self,
        relation: &str,
        other: &ObjectKey,
        field: &str,
        input: EditorInput,
        cx: &mut Context<Self>,
    ) {
        let Some(views) = self.editable_relation(relation) else {
            return;
        };
        let option = self.page_option(relation, other).or_else(|| {
            // A staged row off the page: its own state is the base.
            self.draft
                .as_ref()?
                .links()
                .get(relation, other)
                .map(|l| (l.label().to_owned(), l.version().is_some()))
                .map(|(label, linked)| M2mOption {
                    key: other.clone(),
                    label,
                    linked,
                    link_values: None,
                    link_version: None,
                })
        });
        let (Some(option), Some(draft)) = (option, self.draft.as_mut()) else {
            return;
        };
        draft
            .links_mut()
            .edit(relation, &option, &views, field, input);
        cx.notify();
    }

    /// A link field lost focus: its issues show.
    pub fn blur_link_field(
        &mut self,
        relation: &str,
        other: &ObjectKey,
        field: &str,
        cx: &mut Context<Self>,
    ) {
        if let Some(draft) = self.draft.as_mut() {
            draft.links_mut().blur(relation, other, field);
            cx.notify();
        }
    }

    /// Drops the staged change to a row ("Undo").
    pub fn undo_link(&mut self, relation: &str, other: &ObjectKey, cx: &mut Context<Self>) {
        if self.editable_relation(relation).is_none() {
            return;
        }
        if let Some(draft) = self.draft.as_mut() {
            if draft.links_mut().undo(relation, other).is_some() {
                cx.notify();
            }
        }
    }

    /// The staged change to one row, with its link-field editors.
    #[must_use]
    pub fn staged_link(&self, relation: &str, other: &ObjectKey) -> Option<&StagedLink> {
        self.draft.as_ref()?.links().get(relation, other)
    }

    /// Every staged change of `relation`, once each: those on the current
    /// page are drawn in place, the others in the "Not saved yet" group.
    #[must_use]
    pub fn staged_rows(&self, relation: &str) -> Vec<StagedRow> {
        let Some(draft) = &self.draft else {
            return Vec::new();
        };
        draft
            .links()
            .for_relation(relation)
            .map(|l| {
                let values = l.removal_values();
                StagedRow {
                    other: l.other().clone(),
                    label: l.label().to_owned(),
                    stage: l.stage(),
                    on_page: self.page_option(relation, l.other()).is_some(),
                    removal_note: (!values.is_empty()).then(|| text::removal_with_fields(&values)),
                }
            })
            .collect()
    }

    /// The counts line's numbers: links stored now, and staged adds and
    /// removals.
    #[must_use]
    pub fn link_counts(&self, relation: &str) -> LinkCounts {
        let linked = self
            .m2ms
            .get(relation)
            .and_then(|s| s.page.as_ref())
            .map(|p| p.linked_total)
            .or_else(|| match &self.object {
                Some(ObjectLoad::Loaded(v)) => v
                    .many_to_many
                    .iter()
                    .find(|c| c.relation == relation)
                    .map(|c| c.count),
                _ => None,
            })
            .unwrap_or(0);
        let (to_add, to_remove) = self
            .draft
            .as_ref()
            .map_or((0, 0), |d| d.links().counts(relation));
        LinkCounts {
            linked,
            to_add,
            to_remove,
        }
    }

    // ---- link conflicts ----

    fn is_staged(&self, link: &LinkRef) -> bool {
        self.draft
            .as_ref()
            .is_some_and(|d| d.links().get(&link.relation, &link.other).is_some())
    }

    /// The section's pending link conflict.
    #[must_use]
    pub fn link_conflict(&self, relation: &str) -> Option<&LinkConflict> {
        self.link_conflicts.iter().find(|c| c.relation == relation)
    }

    /// A save met a link another program changed: reads that one option and
    /// the relation's page again, then sets the user's change to it aside.
    /// Every other edit stays.
    pub(super) fn begin_link_conflict(&mut self, link: LinkRef, cx: &mut Context<Self>) {
        let Place::Object { table, key, .. } = self.place().clone() else {
            return;
        };
        self.link_conflict_seq += 1;
        let seq = self.link_conflict_seq;
        self.link_reread = true;
        let query = M2mQuery {
            search: None,
            offset: 0,
            limit: 1,
            linked_only: false,
            only: Some(link.other.clone()),
        };
        let relation = link.relation.clone();
        self.request(
            move |b| b.m2m_options(&table, Some(&key), &link.relation, &query),
            cx,
            {
                let relation = relation.clone();
                let other = link.other;
                move |m, result, cx| m.link_conflict_read(seq, &relation, &other, result, cx)
            },
        );
        if self.m2ms.contains_key(&relation) {
            self.fetch_m2m(&relation, cx);
        }
    }

    fn link_conflict_read(
        &mut self,
        seq: u64,
        relation: &str,
        other: &ObjectKey,
        result: JobResult<M2mPage>,
        cx: &mut Context<Self>,
    ) {
        if seq != self.link_conflict_seq {
            return;
        }
        self.link_reread = false;
        match result {
            Ok(Ok(page)) => {
                let now = page.items.into_iter().find(|o| &o.key == other);
                let Some(set_aside) = self
                    .draft
                    .as_mut()
                    .and_then(|d| d.links_mut().undo(relation, other))
                else {
                    return;
                };
                let rebased = rebase_link(&set_aside, now.as_ref());
                self.link_conflicts.retain(|c| c.relation != relation);
                self.link_conflicts.push(LinkConflict {
                    relation: relation.to_owned(),
                    other: other.clone(),
                    label: now
                        .as_ref()
                        .map_or_else(|| set_aside.label().to_owned(), |o| o.label.clone()),
                    now,
                    set_aside,
                    rebased,
                });
            }
            Ok(Err(e)) => self.fail_db(e, cx),
            Err(e) => self.fail(e.into(), cx),
        }
        cx.notify();
    }

    /// "Re-apply my change": stages the set-aside change again on the link as
    /// it is now (see [`rebase_link`]). When nothing is left to do a notice
    /// says why.
    pub fn reapply_link(&mut self, relation: &str, cx: &mut Context<Self>) {
        let Some(at) = self
            .link_conflicts
            .iter()
            .position(|c| c.relation == relation)
        else {
            return;
        };
        let conflict = self.link_conflicts.remove(at);
        match conflict.rebased.outcome {
            RebaseOutcome::Staged => {}
            RebaseOutcome::AlreadyLinked => self.notice = Some(Notice::LinkAlreadyLinked),
            RebaseOutcome::AlreadyGone => self.notice = Some(Notice::LinkAlreadyGone),
            RebaseOutcome::AlreadyApplied => self.notice = Some(Notice::LinkAlreadyApplied),
        }
        if let (Some(staged), Some(draft)) = (conflict.rebased.staged, self.draft.as_mut()) {
            draft.links_mut().insert(staged);
        }
        cx.notify();
    }

    /// "Discard it": the set-aside change is dropped.
    pub fn discard_link(&mut self, relation: &str, cx: &mut Context<Self>) {
        let before = self.link_conflicts.len();
        self.link_conflicts.retain(|c| c.relation != relation);
        if self.link_conflicts.len() != before {
            cx.notify();
        }
    }

    // ---- delete ----

    /// Objects of `table` can be deleted now: the config allows it and
    /// nothing makes the table read-only. A pickable table never can.
    #[must_use]
    pub fn can_delete(&self, table: &str) -> bool {
        self.table_view(table)
            .is_some_and(|t| t.can_delete && t.mode == TableMode::Editable)
            && self.table_read_only_reason(table).is_none()
    }

    /// The delete confirmation's state; `None` when no delete is going on.
    #[must_use]
    pub fn delete_state(&self) -> Option<&DeleteState> {
        self.delete.as_ref()
    }

    /// Starts deleting the open object: reads the impact ("Checking..."),
    /// then opens the confirmation. A dirty draft is named in it and
    /// discarded when the delete happens.
    pub fn request_delete(&mut self, cx: &mut Context<Self>) {
        let (Place::Object { table, key, .. }, Some(ObjectLoad::Loaded(view))) =
            (self.place().clone(), &self.object)
        else {
            return;
        };
        let label = match self.draft_label() {
            l if l.is_empty() => view.label.clone(),
            l => l,
        };
        let discards = self.dirty();
        self.begin_delete(
            DeleteTarget {
                table,
                key,
                label,
                open: true,
                discards_draft: discards,
            },
            cx,
        );
    }

    /// Starts deleting a list item. When it is the open object this is
    /// [`DbmanagerModel::request_delete`]; otherwise there is no draft to
    /// discard, and on success the list is read again and the place stays.
    pub fn request_delete_of(
        &mut self,
        table: &str,
        key: ObjectKey,
        label: &str,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.place(), Place::Object { table: t, key: k, .. } if t == table && *k == key)
        {
            self.request_delete(cx);
            return;
        }
        self.begin_delete(
            DeleteTarget {
                table: table.to_owned(),
                key,
                label: label.to_owned(),
                open: false,
                discards_draft: false,
            },
            cx,
        );
    }

    fn begin_delete(&mut self, target: DeleteTarget, cx: &mut Context<Self>) {
        if self.write_in_progress || self.delete.is_some() || !self.can_delete(&target.table) {
            return;
        }
        self.delete_seq += 1;
        let seq = self.delete_seq;
        self.delete = Some(DeleteState::Checking {
            label: target.label.clone(),
        });
        let (table, key) = (target.table.clone(), target.key.clone());
        self.delete_target = Some(target);
        self.request(
            move |b| b.delete_preview(&table, &key),
            cx,
            move |m, result, cx| m.preview_done(seq, None, result, cx),
        );
        cx.notify();
    }

    fn preview_done(
        &mut self,
        seq: u64,
        notice: Option<DeleteDialogNotice>,
        result: JobResult<DeleteImpact>,
        cx: &mut Context<Self>,
    ) {
        if seq != self.delete_seq || self.delete.is_none() {
            return;
        }
        match result {
            Ok(Ok(impact)) => self.show_dialog(impact, notice),
            Ok(Err(e)) => {
                self.delete = None;
                self.delete_target = None;
                self.fail_db(e, cx);
            }
            Err(e) => {
                self.delete = None;
                self.delete_target = None;
                self.fail(e.into(), cx);
            }
        }
        cx.notify();
    }

    /// Shows `impact`: the confirmation, or the blocked state.
    fn show_dialog(&mut self, impact: DeleteImpact, notice: Option<DeleteDialogNotice>) {
        let Some(target) = &self.delete_target else {
            return;
        };
        let blocked = impact.blocked;
        let dialog = DeleteDialog {
            table: target.table.clone(),
            key: target.key.clone(),
            label: target.label.clone(),
            impact,
            discards_draft: target.discards_draft,
            notice,
        };
        self.delete = Some(if blocked {
            DeleteState::Blocked(dialog)
        } else {
            DeleteState::Confirm(dialog)
        });
    }

    /// Cancel or Close: the dialog goes and nothing is deleted. Ignored while
    /// the delete itself runs.
    pub fn cancel_delete(&mut self, cx: &mut Context<Self>) {
        if matches!(self.delete, None | Some(DeleteState::Deleting(_))) {
            return;
        }
        self.delete = None;
        self.delete_target = None;
        self.delete_seq += 1;
        cx.notify();
    }

    /// Confirm: deletes with the impact's version and token and the config
    /// hash the user was shown.
    pub fn confirm_delete(&mut self, cx: &mut Context<Self>) {
        let Some(DeleteState::Confirm(dialog)) = self.delete.clone() else {
            return;
        };
        let hash = self.config_hash();
        self.write_in_progress = true;
        self.delete = Some(DeleteState::Deleting(dialog.clone()));
        self.request(
            move |b| {
                b.delete(
                    &dialog.table,
                    &dialog.key,
                    &dialog.impact.version,
                    &dialog.impact.token,
                    &hash,
                )
            },
            cx,
            |m, result, cx| {
                m.write_in_progress = false;
                m.delete_done(result, cx);
            },
        );
        cx.notify();
    }

    fn delete_done(&mut self, result: JobResult<DeleteResult>, cx: &mut Context<Self>) {
        let Some(DeleteState::Deleting(dialog)) = self.delete.clone() else {
            return;
        };
        match result {
            Ok(Ok(done)) => self.deleted(&done, cx),
            Ok(Err(e)) => self.delete_failed(dialog, e, cx),
            Err(e) => {
                self.delete = None;
                self.delete_target = None;
                self.fail(e.into(), cx);
                self.fetch_snapshot(cx);
            }
        }
        cx.notify();
    }

    fn deleted(&mut self, done: &DeleteResult, cx: &mut Context<Self>) {
        let target = self.delete_target.take();
        self.delete = None;
        let groups = &done.impact.groups;
        let related = groups.iter().map(|g| g.count).sum();
        let at_least = done.impact.truncated || groups.iter().any(|g| g.truncated);
        let notice = Notice::Deleted {
            label: done.label.clone(),
            related,
            at_least,
            backup: done.backup_created.clone(),
        };
        let open = target.as_ref().is_some_and(|t| t.open);
        if open {
            // The object is gone: the place is replaced by its table.
            self.history[self.cursor] = Place::Table {
                table: done.table.clone(),
            };
            self.enter(true, cx);
        } else if self
            .list_states
            .get(&done.table)
            .is_some_and(|l| l.page.is_some())
        {
            self.fetch_list(&done.table, cx);
        }
        self.announce(notice.copy(), cx);
        self.notice = Some(notice);
        self.focus_hint = Some(FocusHint::DeleteNotice);
        if done.backup_created.is_some() {
            self.fetch_snapshot(cx);
        }
    }

    fn delete_failed(&mut self, dialog: DeleteDialog, e: DbmanagerError, cx: &mut Context<Self>) {
        match e.kind {
            DbmanagerErrorKind::ImpactChanged => {
                let impact = e.impact.map_or(dialog.impact, |i| *i);
                self.show_dialog(impact, Some(DeleteDialogNotice::ImpactChanged));
                self.focus_hint = Some(FocusHint::DeleteNotice);
            }
            DbmanagerErrorKind::DeleteBlocked => {
                let mut impact = e.impact.map_or(dialog.impact, |i| *i);
                impact.blocked = true;
                self.show_dialog(impact, None);
            }
            DbmanagerErrorKind::ConfigChangedOnDisk => {
                // The backend has reloaded the config. Nothing is retried:
                // the impact is read again and shown first.
                self.delete_seq += 1;
                let seq = self.delete_seq;
                let order = self.next_seq();
                self.delete = Some(DeleteState::Checking {
                    label: dialog.label.clone(),
                });
                // The draft's inline issues follow the new rules at once.
                self.recheck(Recheck::Reload, cx);
                let (table, key) = (dialog.table, dialog.key);
                self.request(
                    move |b| {
                        let snapshot = b.session();
                        let impact = b.delete_preview(&table, &key);
                        (snapshot, impact)
                    },
                    cx,
                    move |m, result, cx| match result {
                        Ok((snapshot, impact)) => {
                            m.snapshot_arrived(order, snapshot, cx);
                            m.preview_done(
                                seq,
                                Some(DeleteDialogNotice::ConfigReloaded),
                                Ok(impact),
                                cx,
                            );
                        }
                        Err(e) => m.preview_done(seq, None, Err(e), cx),
                    },
                );
            }
            DbmanagerErrorKind::Conflict => {
                let open = self.delete_target.take().is_some_and(|t| t.open);
                self.delete = None;
                match e.current {
                    Some(current) if open => {
                        let table = dialog.table.clone();
                        self.object_seq = self.next_seq();
                        if self.dirty() {
                            // Nothing was deleted: the edits stay, and the
                            // conflict panel is raised as on a save.
                            if let Some(draft) = self.draft.as_ref() {
                                let diff = draft.conflict_diff(&current);
                                let label = match self.draft_label() {
                                    l if l.is_empty() => current.label.clone(),
                                    l => l,
                                };
                                self.conflict = Some(ConflictPanel {
                                    label,
                                    current: *current,
                                    diff,
                                });
                            }
                        } else {
                            self.apply_view(&table, *current, true, cx);
                        }
                        self.notice = Some(Notice::DeleteConflict {
                            label: dialog.label,
                        });
                        self.focus_hint = Some(FocusHint::DeleteNotice);
                    }
                    _ => {
                        self.fail_db(e, cx);
                        if self
                            .list_states
                            .get(&dialog.table)
                            .is_some_and(|l| l.page.is_some())
                        {
                            self.fetch_list(&dialog.table, cx);
                        }
                    }
                }
            }
            _ => {
                self.delete = None;
                self.delete_target = None;
                let backup_failed = e.kind == DbmanagerErrorKind::BackupFailed;
                self.fail_db(e, cx);
                if backup_failed {
                    self.fetch_snapshot(cx);
                }
            }
        }
    }

    /// Tests: the race `command_done` handles. An open was sent, a later fetch
    /// answered first (it was applied and started the lists over), then the
    /// open's own, older, snapshot arrives.
    #[cfg(test)]
    pub(super) fn race_open_for_tests(
        &mut self,
        snapshot: &DbmanagerSnapshot,
        cx: &mut Context<Self>,
    ) {
        self.command_seq += 1;
        let seq = self.command_seq;
        let resets = self.reset_count;
        let open_order = self.next_seq();
        let fetch_order = self.next_seq();
        self.snapshot_arrived(fetch_order, snapshot.clone(), cx);
        self.command_done(
            seq,
            open_order,
            Ok(Ok(snapshot.clone())),
            Outcome::Opened,
            resets,
            cx,
        );
    }

    // ---- the config file ----

    /// Starts watching the config file, once per model, after the first
    /// snapshot, when the config has a location. The watcher starts on the
    /// background executor (it touches the disk); one that cannot start is
    /// logged, and the snapshot's `config_changed` and the write path's
    /// reload cover it.
    pub(super) fn arm_config_watcher(&mut self, cx: &mut Context<Self>) {
        if self.watch == WatchState::Armed {
            return;
        }
        let Some(path) = self.snapshot.as_ref().and_then(|s| s.config.path.clone()) else {
            return;
        };
        self.watch = WatchState::Armed;
        #[cfg(test)]
        {
            self.watch_arms += 1;
        }
        let changes = self.config_changes.clone();
        let target = path.clone();
        let task = cx.background_executor().spawn(async move {
            FileWatcher::start(&target, move || {
                let _ = changes.try_send(());
            })
        });
        cx.spawn(async move |this, cx| match task.await {
            Ok(watcher) => {
                this.update(cx, |m, _| m.config_watcher = Some(watcher))
                    .ok();
            }
            Err(e) => log::warn!("dbmanager: cannot watch {}: {e}", path.display()),
        })
        .detach();
    }

    /// Whether the config file is being watched (tests wait on this).
    #[cfg(test)]
    pub(super) fn config_watched(&self) -> bool {
        self.config_watcher.is_some()
    }

    /// The config file changed on disk (the watcher, or the harness): asks
    /// the worker to read it again, after any write already queued, so a
    /// reload never interrupts one. When its hash differs from the one shown,
    /// the draft is checked against the new rules ([`Self::recheck_after_reload`]);
    /// the database is not reopened.
    pub(super) fn config_file_changed(&mut self, cx: &mut Context<Self>) {
        let order = self.next_seq();
        self.request(
            |b| b.reload_config(),
            cx,
            move |m, result, cx| match result {
                Ok(snapshot) => {
                    let shown = m.snapshot.as_ref().map(|s| s.config.hash.as_str());
                    if shown.is_some_and(|h| h != snapshot.config.hash) {
                        m.recheck_after_reload(cx);
                    } else {
                        m.snapshot_arrived(order, snapshot, cx);
                    }
                }
                Err(e) => m.fail(e.into(), cx),
            },
        );
    }

    /// Edit config can be used: developer mode is on.
    #[must_use]
    pub fn can_edit_config(&self, cx: &gpui::App) -> bool {
        DevMode::is_on(cx)
    }

    /// Opens the config in the system editor, first creating a commented
    /// starter file when there is none (the notice says so). A system that
    /// cannot open it is the `OpenerFailed` banner. Does nothing outside
    /// developer mode.
    pub fn edit_config(&mut self, cx: &mut Context<Self>) {
        if !self.can_edit_config(cx) {
            return;
        }
        let task = self.worker.run(|b| b.edit_config_target(), cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |m, cx| match result {
                Ok(Ok((path, created))) => {
                    if created {
                        let display = m
                            .snapshot
                            .as_ref()
                            .and_then(|s| s.config.display.clone())
                            .unwrap_or_else(|| path.display().to_string());
                        m.notice = Some(Notice::StarterCreated { display });
                        cx.notify();
                    }
                    let opened = platform(cx).open_with_system(&path, cx);
                    Self::report_open(opened, cx);
                }
                Ok(Err(e)) => m.fail_db(e, cx),
                Err(e) => m.fail(e.into(), cx),
            })
            .ok();
        })
        .detach();
    }

    /// Waits for the system's answer to an open and shows its failure as the
    /// banner (`OpenerFailed`, the system's words as the detail).
    fn report_open(task: Task<Result<(), OpenError>>, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            if let Err(e) = task.await {
                this.update(cx, |m, cx| {
                    m.fail_db(
                        DbmanagerError::new(DbmanagerErrorKind::OpenerFailed, e.detail),
                        cx,
                    );
                })
                .ok();
            }
        })
        .detach();
    }

    /// The copy of the current notice when it is one of the three
    /// config-reload notices ("The config was reloaded.", "The config was
    /// reloaded. These changes no longer apply and were removed: ...", and
    /// the save-refused-after-reload notice). `None` for any other notice,
    /// or none.
    #[must_use]
    pub fn config_reloaded_notice(&self) -> Option<String> {
        self.notice
            .as_ref()
            .filter(|n| {
                matches!(
                    n,
                    Notice::ConfigReloaded { .. }
                        | Notice::ConfigChangedWhileEditing { .. }
                        | Notice::ConfigReloadedGone { .. }
                )
            })
            .map(Notice::copy)
    }

    /// Screenshot harness and tests: edits the config as a user would, then
    /// takes the watcher's path: reload the config, and re-check the draft
    /// when its hash changed.
    #[cfg(any(test, feature = "shoot"))]
    pub fn inject_file_changed(&mut self, cx: &mut Context<Self>) {
        self.request(
            |b| b.simulate_config_edit(),
            cx,
            |m, result, cx| {
                if let Err(e) = result {
                    m.fail(e.into(), cx);
                }
                m.config_file_changed(cx);
            },
        );
    }
}
