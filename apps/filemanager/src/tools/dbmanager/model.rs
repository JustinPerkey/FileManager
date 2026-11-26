//! The Database Manager's model, read side: the snapshot, the places the user
//! has been (history), the table lists, one object with its "referenced by"
//! and many-to-many pages. The view only renders it and calls the methods
//! here. Writes (drafts, save, create, delete) come with later parts.
//!
//! No backend call runs on the UI thread: each goes through the model's
//! [`Worker`]. Results come back through weak-entity updates, so a result
//! that lands after the model is gone is dropped. Every list, object and page
//! request carries a sequence number, and only the newest request for the
//! same list or page applies.

use std::collections::HashMap;
use std::path::PathBuf;

use fm_dbmanager::config::TableMode;
use fm_dbmanager::model::TableView;
use fm_dbmanager::session::{
    ListQuery, M2mPage, M2mQuery, ObjectKey, ObjectPage, ObjectSummary, ObjectView,
    ReferencingFilter,
};
use fm_dbmanager::tool::{DbmanagerError, DbmanagerErrorKind, DbmanagerSnapshot};
use gpui::Context;

use crate::app::text::PICK_FAILED;
use crate::jobs::{JobPanicked, Worker};
use crate::platform::{platform, FileFilter, OpenSpec, PickError, PickKind};

use super::backend::DbmanagerBackend;
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
    /// A new object of `table` (raised by a later part).
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
    /// The error summary of a refused save (a later part).
    ErrorSummary,
    /// The error banner.
    ErrorBanner,
    /// The reference picker's trigger for this field, after the picker closed
    /// (a later part).
    PickerTrigger(String),
    /// The delete notice (impact changed, or delete done; a later part).
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

/// What a finished session command did.
#[derive(Clone, Copy, PartialEq, Eq)]
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
    history: Vec<Place>,
    cursor: usize,
    list_states: HashMap<String, ListState>,
    object: Option<ObjectLoad>,
    object_seq: u64,
    refs: HashMap<String, RefList>,
    m2ms: HashMap<String, M2mState>,
    /// The folder of the database last held, where the open dialog starts.
    last_dir: Option<PathBuf>,
    /// A save or delete is running. Set by the write parts; the drop is
    /// refused meanwhile.
    write_in_progress: bool,
}

impl DbmanagerModel {
    /// A model over `backend`; it starts loading the state at once.
    pub fn new(backend: Box<dyn DbmanagerBackend>, cx: &mut Context<Self>) -> Self {
        let worker = Worker::new(backend, cx);
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
            history: vec![Place::Tables],
            cursor: 0,
            list_states: HashMap::new(),
            object: None,
            object_seq: 0,
            refs: HashMap::new(),
            m2ms: HashMap::new(),
            last_dir: None,
            write_in_progress: false,
        };
        model.fetch_snapshot(cx);
        model
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

    fn fetch_snapshot(&mut self, cx: &mut Context<Self>) {
        self.request(
            |b| b.session(),
            cx,
            |m, result, cx| match result {
                Ok(snapshot) => m.snapshot_arrived(snapshot, cx),
                Err(e) => m.fail(e.into(), cx),
            },
        );
    }

    fn snapshot_arrived(&mut self, snapshot: DbmanagerSnapshot, cx: &mut Context<Self>) {
        let first = self.snapshot.is_none();
        self.remember_dir(&snapshot);
        self.snapshot = Some(snapshot);
        if first {
            // The restored database, if any, is shown without taking focus.
            self.reset_for_database(false, cx);
        }
        cx.notify();
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
        self.command(move |b| b.open(&path), Outcome::Opened, cx);
    }

    /// Opens the recent database at `index` of the snapshot's `recent` list.
    pub fn open_recent(&mut self, index: usize, cx: &mut Context<Self>) {
        self.command(move |b| b.open_recent(index), Outcome::Opened, cx);
    }

    /// Files dropped on the window. Ignored while a write runs.
    pub fn dropped(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if self.drop_availability().is_err() {
            return;
        }
        self.command(move |b| b.open_dropped(paths), Outcome::Opened, cx);
    }

    /// Reloads the config and the database.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.command(|b| b.reload(), Outcome::Reloaded, cx);
    }

    /// Closes the database; the config stays.
    pub fn close_database(&mut self, cx: &mut Context<Self>) {
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
        self.pending_commands += 1;
        self.request(job, cx, move |m, result, cx| {
            m.pending_commands -= 1;
            m.command_done(seq, result, outcome, cx);
        });
        cx.notify();
    }

    fn command_done(
        &mut self,
        seq: u64,
        result: JobResult<DbmanagerSnapshot>,
        outcome: Outcome,
        cx: &mut Context<Self>,
    ) {
        if seq != self.command_seq {
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
                self.remember_dir(&snapshot);
                self.snapshot = Some(snapshot);
                self.error = None;
                match outcome {
                    Outcome::Opened | Outcome::Closed => self.reset_for_database(true, cx),
                    Outcome::Reloaded => self.enter(false, cx),
                }
                if let Some(sentence) = sentence {
                    self.announce(sentence, cx);
                }
                cx.notify();
            }
            Ok(Err(e)) => self.fail_db(e, cx),
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

    /// Goes to the previous place.
    pub fn back(&mut self, cx: &mut Context<Self>) {
        if self.can_back() {
            self.cursor -= 1;
            self.enter(true, cx);
        }
    }

    /// Goes to the next place.
    pub fn forward(&mut self, cx: &mut Context<Self>) {
        if self.can_forward() {
            self.cursor += 1;
            self.enter(true, cx);
        }
    }

    /// Shows `table`'s list (a push). Ignored for a table that is not in the
    /// snapshot, and when it is already the place.
    pub fn choose_table(&mut self, table: &str, cx: &mut Context<Self>) {
        if self.table_view(table).is_some() {
            self.push(
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
        self.push(
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
            self.push(place, cx);
        }
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
            Place::New { .. } => FocusHint::SheetHeading,
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
        let asked = (table.clone(), key.clone());
        self.request(
            move |b| b.object(&asked.0, &asked.1),
            cx,
            move |m, result, cx| m.object_done(&table, &key, seq, result, cx),
        );
    }

    fn object_done(
        &mut self,
        table: &str,
        key: &ObjectKey,
        seq: u64,
        result: JobResult<ObjectView>,
        cx: &mut Context<Self>,
    ) {
        if self.object_seq != seq {
            return;
        }
        match result {
            Ok(Ok(view)) => {
                if let Place::Object {
                    table: t,
                    key: k,
                    label,
                } = &mut self.history[self.cursor]
                {
                    if t == table && k == key {
                        label.clone_from(&view.label);
                    }
                }
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
                self.object = Some(ObjectLoad::Loaded(view));
                for (relation, linked_only) in relations {
                    self.m2ms.insert(
                        relation.clone(),
                        M2mState {
                            linked_only,
                            ..M2mState::default()
                        },
                    );
                    self.fetch_m2m(&relation, cx);
                }
            }
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
        let Place::Object { table, key, .. } = self.place().clone() else {
            return;
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
            move |b| b.m2m_options(&table, Some(&key), &asked, &query),
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
