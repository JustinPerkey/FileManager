# M12a — Database Manager, part A: tool core, snapshot, errors, backend trait, and the reading model

Status: approved (2026-11-21). M12a landed as 0ec70dc; the next run does only the section "Review follow-up" below.
Project: gpui-migration   Depends on: dbmanager M9 (delete, landed), gpui-migration M7b (landed)

## Goal

Make the Database Manager a tool of the GPUI app with a read-only,
fully tested view-model: the tool-level core in `fm_dbmanager::tool`
(the fixed app-wide config, recent databases, restore, open, drop, reload,
close, snapshot, error kinds), a backend trait over it, and a model for
places, history, lists, objects, "referenced by" and many-to-many pages,
with copy and fixtures. Writes come in parts B and C, config watching in
part D.

## Windows CI follow-up (2026-11-27)

PR #44's Windows CI fails two tests in `crates/fm-dbmanager/src/tool/tests.rs`.
**This run fixes only these tests,** and test code only. The M12c
implementer works in the same crate, so touch nothing else.

1. **`display_root_maps_paths`** (around line 626). Left is
   `"/fixture\t.db"`, right is `"\fixture\t.db"`. The test builds its
   expectation by replacing every `/` with `MAIN_SEPARATOR`, but the code
   keeps the display root **as given**.
   - **Decision: the display root is kept as given.** It is a fixture's fake
     display path, written in the form the screenshots should show. Only
     the part appended to it uses the platform separator.
   - Fix the **test**: build the expectation as the root string unchanged,
     followed by the appended components joined with `MAIN_SEPARATOR`. The
     code is unchanged. It must pass on both Linux and Windows.
2. **`failed_restore_warns_and_keeps_entry`** (around line 258).
   `fs::remove_file(&fx.db)` fails on Windows with OS error 32 (the file is
   in use), because the first `Core` still has the database open.
   - Drop the first `Core` (and anything holding its `Session`) before
     deleting the file, for example by scoping it in a block or calling
     `drop(core)`.
   - **Check the others:** grep `crates/fm-dbmanager/src/tool/tests.rs` and
     `apps/filemanager/src/tools/dbmanager/tests.rs` for
     `remove_file`, `rename`, `remove_dir` and `fs::write` over a database
     or config file that a live `Core`, `Session` or model may still hold
     open. Fix each the same way, and list them in the PR. (Rewriting a
     config file while it is open for reading is fine on Windows; deleting
     or renaming an open database is not.)

Acceptance: both tests, and any others found, pass on the Windows CI job;
the Linux job stays green; no non-test code changes.

## Review follow-up (2026-11-26)

The review of 0ec70dc found one P2. **The next run does only this section.**
Read `apps/filemanager/src/tools/dbmanager/model.rs` and its tests first.

### P2: a superseded command's effect is dropped

- `command_done` drops every result whose sequence number is not the
  newest. But the Worker runs jobs in order, so a superseded command has
  **already taken effect in the backend**. For example: open A succeeds,
  open B (queued behind it) fails, and the backend keeps A. The model then
  discards A's snapshot and keeps the pre-A snapshot and history.
- The `JobPanicked` branch has the same gap: `snapshot_arrived` resets
  nothing once a snapshot exists.
- **Fix:**
  - When a command fails (`Ok(Err)`) or panics, re-fetch the snapshot
    through the Worker. Do not keep the model's old copy.
  - Whenever a snapshot arrives (from a command or a fetch), compare its
    database path and `open` state with the model's. If they differ, call
    `reset_for_database` (place, history, list states, object) before
    applying it.
  - The "newest result wins" rule still applies to what is **shown**
    (errors, list pages). It must never leave the model on a snapshot
    older than the backend's.
- **Test:** `superseded_open_keeps_the_backend_state`.
  1. Install a closed `WorkerGate` (`jobs.rs`).
  2. Queue `open(A, valid)`, then `open(B, missing)`.
  3. Release the gate, then `run_until_parked`.
  4. Assert that the place, the snapshot (database A, open) and the listed
     tables are database A's, and that B's error banner shows.
- Add `panicked_command_refetches_snapshot` too, using the fixture's panic
  hook if it has one; otherwise add a minimal hook to the dbmanager fixture.

### Recorded (as built and accepted)

- Choosing the **current** place again pushes no history entry and does not
  refresh. Reload refreshes.
- List states (search text, offset) are kept **per database**: cleared when
  a database is opened or closed, kept across a reload. This replaces "for
  the whole app run" below.
- `DbmanagerError::conflict_link` is `Option<Box<LinkRef>>`.

### Later (not this run)

- Perf note: both the Tar Packager's and the Database Manager's `Core::new`
  read the persisted store on the UI thread, at startup. A later backend
  task moves that read onto the Worker.

### Acceptance

- The two tests pass. fmt (linux job), clippy pedantic (with and without
  `--features shoot`), and `cargo test --workspace` pass.

## Context

### The tool

The Database Manager (`dbmanager`) edits a SQLite file the user picks, as
objects rather than a grid. **One config for the whole app** sets what is
editable: `fm_core::system_config_dir()?.join("dbmanager.toml")`
(`%ProgramData%\FileManager\dbmanager.toml` on Windows,
`/etc/filemanager/dbmanager.toml` elsewhere). The user never chooses it and
the app only reads it. Missing config = everything editable by default,
with a notice. A config that cannot be used (syntax error, unsupported
`version`, unreadable, or an unknown location) **fails closed**: everything
is read-only, with a reason. Tables have modes editable / read-only /
hidden / pickable (a pickable lookup table is not listed, but its objects
can be picked, linked to and opened read-only).

### What exists

- `crates/fm-dbmanager` (domain, read the code): `db.rs`
  (`Database::open`, `OpenError::{NotAbsolute, NotAFile, Busy, NotUnicode, NotADatabase, Io}`),
  `state.rs` (`RememberedState { recent }`, `touch`, `forget`, persisted as
  `dbmanager/state` in `fm_core::Store`; at most 10, most recent first,
  unique), `config/load.rs` (`LoadedConfig::load`, `status() -> ConfigStatus::{Loaded, Missing, FailClosed}`,
  `hash()` = SHA-256 hex or `"missing"`, `fail_closed_reason`,
  `diagnostics`, `changed_on_disk`), `session/` (`Session::open(db, LoadedConfig)`,
  `set_config`, `config`, `database_path`, `read_only_reason`,
  `config_fail_closed`, `diagnostics`, `table_views`, `config_changed`,
  `backup_path`, `adopt_backup`, `list`, `object`, `m2m_options`, `save`,
  `delete_preview`, `delete`; `SessionError` with `Open`, `UnknownTable`,
  `UnknownObject`, `UnknownRelation`, `SchemaChanged`, `Busy`, `Sqlite`,
  `ReadOnlyDatabase`, `ConfigChangedOnDisk`, `NotEditable`, `Validation`,
  `Conflict`, `BackupFailed`, `Constraint`, `ImpactChanged`,
  `DeleteBlocked`), `model/views.rs` (`TableView` with `mode`,
  `objectName`, `canCreate`, `canDelete`, `FieldView`, `ForeignKeyView`,
  `ReverseRelationView`, `M2mView`), `session/types.rs` (`ObjectKey`,
  `ListQuery` with `referencing`, `ObjectPage`, `ObjectView`, `M2mQuery`
  with `only`, `M2mPage`). Examples: `examples/dbmanager/schema.sql`,
  `example.toml`, `starter.toml`.
- `apps/filemanager` (lib `fm_app`): the tool pattern of the Pipeline,
  Schedule Creator and Tar Packager (`src/tools/<tool>/{backend,model,text,fixtures,mod,tests}.rs`,
  `ViewError` + `error_generation`, `announcement`, `take_focus_hint` (consumable: cleared on read),
  `activated()`, "only the newest request applies", drop target), and
  `ToolId`, `ToolSlot`, `ToolHooks` (`activated`, `drop_target`, `dropped`,
  `unsaved_work`, `inject_file_changed`), `jobs::Worker`, `Platform`
  (`pick` with `OpenSpec`/`FileFilter`, `PickError`, `FakePlatform`; native
  Windows dialogs with filters), `DevMode`, `BackendSet`/`FixtureId`,
  the harness.
- If the ts-rs cutover has not happened yet, `fm-dbmanager` types still
  derive `ts_rs::TS`; new types in this milestone **do not** derive it and
  are not added to any export list (they never reach TypeScript).

### What to build

**`crates/fm-dbmanager/src/tool/` (no UI dependency).** The tool core,
testable without the app:

- `Core::new(dirs: &AppDirs, config_path: Option<PathBuf>)`; the app passes
  `fm_core::system_config_dir().ok().map(|d| d.join("dbmanager.toml"))`.
  `Core` holds the store (or an "unpersisted" reason, like the Tar
  Packager's `Core::unpersisted`), the config path, the current
  `LoadedConfig` (always loaded, even with no database), the database slot
  (path and optional error), `Option<Session>`, whether the restore ran, and
  `state_warning`.
- **Config path `None`** → fail-closed with the reason "the config location
  could not be determined"; never the editable defaults.
- `session()` → `DbmanagerSnapshot`. The first call loads the config and
  reopens the most recent database; if it is missing or fails, nothing is
  open, `state_warning` says which database and why, the entry stays in the
  list. **Later calls are cheap and side-effect free**: no open, no user
  query, no write, no backup, no re-introspection, no config reload; the
  only I/O is re-reading and hashing the config file so `config_changed`
  is recomputed.
- `open(path)`: absolute (`NotAbsolute`) and a file (`NotAFile`) checked
  first; opens with the current config; touches the recent list and saves
  it. Opening (even the same path) starts a fresh session (no backup yet).
- `open_dropped(paths)`: exactly one path opens like `open`; more than one →
  `DropAmbiguous`, nothing changes; a single `.toml` (ASCII
  case-insensitive) → `ConfigNotDroppable` before any open, the current
  database stays open.
- `reload()`: reloads the config and, with a database open, reopens it
  (re-introspects) with the new config, **keeping the session's backup**
  (`adopt_backup`: `backup_path` unchanged, no second backup). Works with no
  database (config only).
- `reload_config()` (`pub`): `LoadedConfig::load` then `set_config` if a
  database is open (used by the write path in part B and the watcher in
  part D).
- `close()`: drops the session and clears the slot; the config stays.
- `list`, `object`, `m2m_options` forward to the session; `NoDatabase`
  without one. (`save`, `delete_preview`, `delete` are added in parts B and
  C.)
- `recent_paths() -> Vec<PathBuf>`; open recent by index into it, never by a
  display string.

Types (plain Rust; integers are integers):

```rust
pub struct DbFileSlot { pub path: PathBuf, pub display: String, pub error: Option<DbmanagerError> }
pub struct DbConfigView { pub path: Option<PathBuf>, pub display: Option<String>, pub status: ConfigStatus, pub hash: String }
pub struct DbmanagerSnapshot {
    pub database: Option<DbFileSlot>, pub config: DbConfigView, pub open: bool,
    pub read_only_reason: Option<String>, pub config_changed: bool,
    pub config_fail_closed: Option<String>,       // Some iff fail-closed; never inferred from diagnostics
    pub diagnostics: Vec<ConfigDiagnostic>,       // session's with a database, config-only without
    pub error_count: usize, pub warning_count: usize,
    pub tables: Vec<TableView>,                   // includes pickable ones (not for the table list)
    pub backup_path: Option<PathBuf>,
    pub recent: Vec<String>,                      // display, most recent first
    pub state_warning: Option<String>,
}
pub enum DbmanagerErrorKind { NoDatabase, NotAbsolute, NotAFile, DatabaseUnreadable, OpenerFailed,
    ConfigNotDroppable, DropAmbiguous, UnknownTable, UnknownObject, UnknownRelation, NotEditable,
    ValidationFailed, Conflict, SchemaChanged, ConfigChangedOnDisk, DatabaseReadOnly, Busy,
    BackupFailed, ImpactChanged, DeleteBlocked, ConstraintFailed, Io }   // 22, pinned by a test
pub struct DbmanagerError {
    pub kind: DbmanagerErrorKind, pub message: String,               // technical detail
    pub field_errors: Vec<FieldError>, pub link_errors: Vec<LinkError>,
    pub current: Option<Box<ObjectView>>, pub conflict_link: Option<Box<LinkRef>>,
    pub impact: Option<Box<DeleteImpact>>,
}
```

Mapping: `OpenError::NotAbsolute`/`NotAFile` → same; `OpenError::Busy` →
`Busy`; `OpenError::{NotUnicode, NotADatabase, Io}` → `DatabaseUnreadable`;
`Validation` → `ValidationFailed` with both lists; `Conflict` → `Conflict`
with `current` and `conflict_link`; `ImpactChanged`, `DeleteBlocked` → same
with `impact`; `ReadOnlyDatabase` → `DatabaseReadOnly`; `ConfigChangedOnDisk`
→ same (message: the config was reloaded; check the change and save again);
`Constraint` → `ConstraintFailed`; `Sqlite` → `Io`; the rest by name. Every
message for a failed write ends by saying nothing was changed. If the
`OpenError::NotUnicode` variant reflects a lossy path conversion inside
`Database::open`, keep it as `DatabaseUnreadable` with a message saying the
path is not valid Unicode.

**`apps/filemanager/src/tools/dbmanager/` (new tool).** `ToolId::Dbmanager`
(id `dbmanager`, label "Database Manager", registered last in `build`),
with:

- `backend.rs`: `DbmanagerBackend` trait, one method per `Core` operation
  above (`session`, `open`, `open_recent(index)`, `open_dropped`,
  `reload`, `close`, `list`, `object`, `m2m_options`), and
  `RealDbmanager { core }`. Every call runs on the `Worker` (opening,
  introspection and queries can take seconds on a slow share).
- `text.rs`: the copy for every `DbmanagerErrorKind` (one sentence each, the
  backend's `message` shown as detail), the config status notices ("No
  config is in use: everything is editable." for `missing`; the fail-closed
  reason line; "The config changed on disk." for the fallback), the empty
  state, the drop label and disabled reasons ("Drop one database file";
  "Wait for the save to finish"), the `ConfigNotDroppable` sentence naming
  the config's fixed location, and the read-only reasons. Write the error
  sentences plainly; a later UI task may adjust wording, not meaning.
- `model.rs`: `DbmanagerModel`, read side:

  ```rust
  pub enum Place { Tables, Table { table: String }, Object { table: String, key: ObjectKey, label: String }, New { table: String } }
  impl DbmanagerModel {
      pub fn snapshot(&self) -> Option<&DbmanagerSnapshot>;  pub fn loading(&self) -> bool;
      pub fn error(&self) -> Option<&ViewError>;  pub fn error_generation(&self) -> u64;  pub fn dismiss_error(&mut self, cx);
      pub fn listed_tables(&self) -> Vec<&TableView>;        // not hidden, not pickable
      pub fn open(&mut self, cx);  pub fn open_recent(&mut self, index: usize, cx);   // open: filters below
      pub fn reload(&mut self, cx);  pub fn close_database(&mut self, cx);
      pub fn place(&self) -> &Place;  pub fn can_back(&self) -> bool;  pub fn can_forward(&self) -> bool;
      pub fn back(&mut self, cx);  pub fn forward(&mut self, cx);
      pub fn choose_table(&mut self, table: &str, cx);
      pub fn open_object(&mut self, table: &str, key: ObjectKey, cx);   // push
      pub fn follow(&mut self, fk_field: &str, cx);                      // follow a reference: push
      pub fn list_state(&self, table: &str) -> Option<&ListState>;     // search text, offset, page, loading
      pub fn set_search(&mut self, table: &str, text: &str, cx);  pub fn set_offset(&mut self, table: &str, offset: u64, cx);
      pub fn object(&self) -> Option<&ObjectLoad>;                     // Loading | Loaded(ObjectView) | Gone
      pub fn referenced_by(&self, relation: &str) -> Option<&RefList>; pub fn open_referenced_by(&mut self, relation: &str, cx);
      pub fn show_more_referenced_by(&mut self, relation: &str, cx);
      pub fn m2m(&self, relation: &str) -> Option<&M2mState>;          // query, page
      pub fn set_m2m_query(&mut self, relation: &str, search: &str, linked_only: bool, offset: u64, cx);
      pub fn take_focus_hint(&mut self) -> Option<FocusHint>;          // consumable: cleared on read
  }
  pub enum FocusHint { SheetHeading, ListHeading, ErrorSummary, ErrorBanner, PickerTrigger(String), DeleteNotice }
  ```

  Focus hints (all defined here; later parts raise the ones they own): a
  place change raises `SheetHeading` (object, new) or `ListHeading` (table);
  a banner error raises `ErrorBanner`; parts B and C raise `ErrorSummary`
  (a refused save), `PickerTrigger(field)` (the reference picker closed) and
  `DeleteNotice` (impact changed, or delete done). A hint is returned once,
  so a repeated hint is distinct from an old one.

  Behaviour:
  - **History:** every place is an entry; choosing a table, opening an
    object, following a reference and New push; pushing truncates forward
    entries; capped at 50 (oldest dropped). Opening a database resets
    history to its first listed table, or `Tables` if none. An entry whose
    object was deleted shows `ObjectLoad::Gone`; it is not removed. (Part B
    adds "replace" after create and delete, and the leave-a-dirty-draft
    confirmation.)
  - **Lists:** 25 per page; each table keeps its search text and offset while
    the database stays open (cleared on open and close, kept across a
    reload), so returning shows it as left. Only the newest
    request for a list applies.
  - **Referenced by:** one entry per `ReverseRelationView`, closed by
    default; opening loads `list(rel.table, { referencing: { relation, key }, offset, limit: 25 })`;
    "show more" appends 25. A count of 0 is not openable.
  - **Many-to-many:** `m2m_options(table, Some(key), relation, M2mQuery { search, offset, limit: 25, linked_only, only: None })`;
    `linked_only` defaults off for editable relations and on for read-only
    ones.
  - **Pickable** targets open read-only.
  - **Open** shows the Windows file picker with the filters "SQLite
    database" (`*.db;*.sqlite;*.sqlite3;*.db3`) then "All files" (`*.*`),
    starting in the folder of the open database, else of the most recent
    one; a pick `Err` shows the `PickError` copy.
  - **Model state survives switching tools** (today's design: switching
    away and back keeps place, history and drafts): `activated()` refreshes
    the snapshot (cheap) and clears only `error` and `announcement`.
  - **Drop:** `drop_target` is enabled with the label when no write is in
    progress; `dropped` calls `open_dropped`; `ConfigNotDroppable` and
    `DropAmbiguous` are banners.
- `fixtures.rs`: an in-memory fixture is not practical for SQLite views;
  instead the fixture backend wraps a real `Core` over a database built
  from `examples/dbmanager/schema.sql` **in a temp dir created by the
  fixture** (never a user path), with `example.toml` as the config in the
  same temp dir. Provide states: no database (empty), example open
  (`loaded`), config missing, config fail-closed, a state warning.
  Screenshots show fake display paths: give `Core` a display-path override
  for fixtures (`with_display_root`), or map paths in the fixture backend.

**Fixture timing rule (planner, 2026-11-22, after M4).** The harness's
`Step::WaitIdle` is a short fixed settle, not idle detection: it does not
await `Worker` jobs, `spawn_thread` jobs or async loads. So every fixture
backend state a scenario shows must answer **synchronously** (the backend
method returns at once, so the Worker call resolves inside the settle), or
the scenario that shows it must add an explicit `Step::Wait(ms)` long
enough for the job. Fixtures that simulate long work (progress, a running
step) are timed on purpose; document each fixture's timing in its doc
comment so the scenario author knows which `Wait` it needs.

**Real folders only through `app::dirs` (after M4d, 2026-11-23).** The app
side resolves the store and the config path only through
`apps/filemanager/src/app/dirs.rs` (`user_app_dirs()`,
`system_config_path()`), which panic in test and `shoot` builds. Tests and
fixtures pass temp-dir `AppDirs` and config paths. Register the
Database Manager in `BackendSet::fixtures()` with a fixture backend.

### Rules that bind this task

- `fm-dbmanager` never depends on GPUI or tauri. Tools never import each
  other.
- **SQL safety:** only `session/sql.rs` assembles SQL; values are bound
  parameters; identifiers come from introspection as `Ident`. The tool core
  writes no SQL.
- **Tests never touch the real config location or real user files**:
  `Core` takes the config path; tests and fixtures pass temp-dir paths;
  `AppDirs::at(tempdir)` for the store; only the app's startup resolves the
  real path.
- No blocking I/O on the UI thread; every backend call on the `Worker`.
- Paths stay `PathBuf`; display strings only for display; recent opens by
  stored path.
- Ownership: `crates/fm-dbmanager/src/tool/`, `apps/filemanager/src/tools/dbmanager/{backend,model,text,fixtures,mod,tests}.rs`;
  `view/` is a placeholder (plain text: the database, the place, list
  labels) for the ui-implementer to replace. No visual decisions.
- fmt clean; clippy pedantic `-D warnings`; scoped `#[expect]` only.

## Files

Create: `crates/fm-dbmanager/src/tool/{mod.rs,core.rs,types.rs,tests.rs}`,
`apps/filemanager/src/tools/dbmanager/{mod.rs,backend.rs,model.rs,text.rs,fixtures.rs,tests.rs,view/mod.rs}`.

Edit: `crates/fm-dbmanager/src/lib.rs`,
`apps/filemanager/src/{lib.rs,tools/mod.rs}`, `CLAUDE.md` (Status: the
Database Manager's GPUI model, read side; Layout lines; one line: the
app-wide config is `%ProgramData%\FileManager\dbmanager.toml`, read-only for
the app).

Do not touch: `apps/desktop/` (no Tauri commands, no `lib/dbmanager.ts`,
no generated types), other tools, `docs/plans/`.

## Acceptance criteria

- `DbmanagerErrorKind` has exactly the 22 kinds (pinned by a test).
- The tool appears last in the registry; its model API above exists with
  doc comments.
- On the Windows CI job the GPUI exe passes dumpbin **with SQLite linked**
  (this is the first non-test GPUI code calling `Database::open`); no
  `sqlite3*.dll`.
- fmt, clippy pedantic (with and without `--features shoot`), and
  `cargo test --workspace` pass on both jobs.

## Tests proving completion

`crates/fm-dbmanager/src/tool/tests.rs` (temp dirs, databases from
`schema.sql`):

- `open_builds_snapshot` (tables incl. `publisher` pickable, diagnostics,
  counts), `session_is_cheap_and_side_effect_free` (database and store bytes
  and mtime unchanged, no backup, same `Session` instance),
  `session_recomputes_config_changed` (edit, then restore the config bytes).
- `config_missing_is_editable_defaults`, `config_loaded_status_and_hash`,
  `config_fail_closed_in_snapshot` (syntax error, `version = 2`, a directory
  at the path), `config_location_unknown_fails_closed`,
  `config_shown_without_database`.
- `recent_databases_persisted_and_restored`,
  `failed_restore_warns_and_keeps_entry`, `open_recent_uses_stored_path`.
- `open_dropped_single`, `open_dropped_ambiguous`,
  `open_dropped_toml_is_config_not_droppable`.
- `open_rejects_relative_and_directory`, `non_database_is_unreadable`,
  `reads_without_session_fail`, `errors_map_to_kinds`,
  `error_kinds_are_pinned`, `reload_with_and_without_database`,
  `close_keeps_config`.

`apps/filemanager/src/tools/dbmanager/tests.rs` (`#[gpui::test]`, fixture
backend):

- `restores_and_resets_history_to_first_table`, `history_push_back_forward_truncate`,
  `history_capped_at_50`, `deleted_object_entry_shows_gone`.
- `list_state_kept_per_table`, `only_newest_list_request_applies`.
- `referenced_by_closed_until_opened_and_pages`,
  `m2m_linked_only_defaults_by_relation_mode`.
- `pickable_target_opens_read_only`.
- `drop_opens_single_database`, `dropped_toml_is_banner`.
- `activated_keeps_place_and_history`.
- `open_passes_database_filters`, `open_dialog_error_shows_dialog_failure_copy`.

## Out of scope

- Drafts, validation, save, create, conflicts, close guard (part B); link
  staging and delete (part C); the config watcher, automatic reload and Edit
  config (part D); views and scenarios (UI tasks).

## Risks

- Restoring the most recent database on the first call may hit a slow
  share; it runs on the Worker.
- `%ProgramData%` is shared by every user of the machine; read access is
  enough.
- The fixture backend creates a temp dir per model; screenshots must keep it
  alive for the run and remove it at exit.
