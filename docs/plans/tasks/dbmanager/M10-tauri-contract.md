# M10 — Tauri commands, session snapshot, errors, and the typed client

Status: awaiting approval (renumbered from M9 and revised 2026-11-16: link
errors and conflicts, duration helper, fail-closed config; then the
ui-designer's contract requests: `configFailClosed`, pinned client names, a
cheap `session()`, `objectName`, `M2mQuery.only`)
Project: dbmanager   Depends on: M9 (landed)

## Goal

Expose the Database Manager to the frontend: the `dbmanager_*` commands, the
session snapshot and error types, the recent list, and the typed client
`apps/desktop/src/lib/dbmanager.ts`. This is **the UI contract**: no UI task
starts before it lands.

## Context

The Database Manager (`dbmanager`) is a tool in the FileManager Tauri app.
All its database logic lives in `crates/fm-dbmanager`:

- `db.rs`: `Database::open` → `OpenError::{NotAbsolute, NotAFile,
  NotUnicode, NotADatabase, Io}`.
- `state.rs`: `RememberedState { recent: Vec<RecentPair { database, config
  }> }`, `touch`, `forget`, `StateData` (tool `"dbmanager"`, name
  `"state"`).
- `config/`: `ConfigDiagnostic`; `session/`: `Session::open(database,
  config) -> Result<(Session, Option<ConfigLoadError>), SessionError>`,
  `database_path`, `config_path`, `read_only_reason`, `diagnostics`,
  `table_views`, `config_changed`, `config_fail_closed`, `backup_path`, `list`, `object`,
  `m2m_options`, `save`, `delete_preview`, `delete`.
- `SessionError::{Open(OpenError), UnknownTable, UnknownObject,
  UnknownRelation, SchemaChanged, Busy, Sqlite, ReadOnlyDatabase,
  ConfigChangedOnDisk, NotEditable, Validation { fields: Vec<FieldError>,
  links: Vec<LinkError> }, Conflict { current: Box<ObjectView>, link:
  Option<LinkRef> }, BackupFailed, Constraint, ImpactChanged(Box<
  DeleteImpact>), DeleteBlocked(Box<DeleteImpact>)}`.
- An unreadable config opens the session **fail-closed** (everything
  read-only) and is returned as `ConfigLoadError`; no config at all means the
  editable built-in defaults.
- Boundary types already exported to `apps/desktop/src/lib/generated/`:
  `DbValue` (incl. `duration`), `FieldKind`, `FieldRule`, `DurationUnit`,
  `FieldIssue`, `FieldError`, `FieldIssueCode`, `ConfigDiagnostic`,
  `TableMode`/`FieldMode`/`RelationMode`, `TableView` (with `objectName`; +
  `FieldView`,
  `ForeignKeyView`, `ReverseRelationView`, `M2mView` with `linkFields`),
  `ObjectKey`, `ListQuery`, `ObjectPage`, `ObjectView`, `M2mQuery` (with an
  optional `only` key),
  `M2mPage` (options with `linkValues`, `linkVersion`), `SaveRequest` (with
  `links: LinkChange[]` of `add`/`update`/`remove`), `LinkError`, `LinkRef`,
  `SaveResult`, `DeleteImpact` (groups with `links`, `linkData`),
  `DeleteResult`.

Follow `CLAUDE.md` "Adding a tool", steps 2–5, and the existing tools as
patterns: `apps/desktop/src-tauri/src/tools/schedule/` (slots, drop sorting,
error type) and `tools/pipeline/` (work on a blocking worker),
`tools/tarpack/core.rs` (store, restoring the most recent file on first
session call, `stateWarning`). Rules that bind this task:

- `tools/dbmanager/mod.rs` contributes only `#[tauri::command]` functions
  named `dbmanager_*` and `pub fn state()`. It never touches the `Builder`.
  Only `tools/mod.rs::register` calls `.invoke_handler(` / `.setup(` (a test
  enforces it): add `.manage(dbmanager::state())` and the commands to the
  **single** `generate_handler!`. Dbmanager needs no `setup`.
- Tools never import each other.
- `capabilities/default.json` and the CSP in `tauri.conf.json` do **not**
  change (the UI needs only the Open dialog and drag-drop, already granted;
  `capabilities_are_minimal` must still pass).
- A boundary type is defined once in Rust and generated; `lib/generated/` is
  never hand-edited; no `bigint`; no name already in `lib/generated/` (note
  `FileSlot` belongs to the Schedule Creator).
- The implementer owns everything under `apps/desktop/src/lib/`; do not touch
  the rest of `apps/desktop/src/` (views, registry, styles belong to the
  ui-implementer).
- fmt, clippy pedantic, scoped `#[expect]`; tests never touch real user files
  (temp dirs, `AppDirs::at(tempdir)`).
- Paths arrive as `PathBuf`; display strings (`to_string_lossy`) only in
  snapshot fields meant for display.

### Threading

State: `pub struct DbmanagerState { core: Arc<Mutex<Core>> }`. Every command is
`async` and runs its work with `tauri::async_runtime::spawn_blocking`,
cloning the `Arc` into the closure (opening, introspecting, backups and big
deletes can take seconds; sync commands would run on the main thread). Lock
poisoning recovers with `PoisonError::into_inner`, as the other tools do. A
`JoinError` maps to `Io`.

### Core (`tools/dbmanager/core.rs`, no Tauri types)

Holds: `Option<Store<RememberedState>>` (or an "unpersisted" reason, as
tarpack does when `AppDirs::from_system` fails), the database slot and config
slot (path + optional error), the `Option<Session>`, whether the first-call
restore ran, and a `state_warning`.

- `session()` → snapshot; on the first call, reopen the most recent pair; on
  failure nothing is open, `state_warning` says which pair could not be
  reopened and why, and the pair stays in the list.
- **`session()` after the first call is cheap and side-effect free** (the view
  calls it on every window focus): it opens nothing, runs no query against
  user tables, writes nothing (not the database, not the recent store), takes
  no backup, and does not re-introspect. Its only I/O is re-reading and
  hashing the config file so `config_changed` is **recomputed on every call**
  (and `false` with no config). Everything else in the snapshot comes from
  state already held.
- `open(database, config)`: path must be absolute (`NotAbsolute`) and a file
  (`NotAFile`) — checked for both before opening; on success the pair is
  touched in the recent list and saved; a config that cannot be read leaves
  the database open, **all read-only**, with the config slot's error.
- `set_config(config?)`: reopens the session with the current database and the
  new config (or none).
- `open_dropped(paths)`: a path ending `.toml` (ASCII case-insensitive) is the
  config, any other file the database; more than one of either →
  `DropAmbiguous`, nothing changes; a config alone keeps the current database.
- `reload()`: reopens with the same pair (re-introspects, re-reads config).
- `close()`: drops the session; slots cleared.
- `list`, `object`, `m2m_options`, `save`, `delete_preview`, `delete`: forward
  to the session (`NoDatabase` if none).

### Types (`tools/dbmanager/types.rs`)

```rust
pub struct DbFileSlot { pub path: String, pub error: Option<DbmanagerError> }
pub struct RecentPairView { pub database: String, pub config: Option<String> }
pub struct DbmanagerSession {
    pub database: Option<DbFileSlot>,
    pub config: Option<DbFileSlot>,
    pub open: bool,
    pub read_only_reason: Option<String>,
    pub config_changed: bool,
    /// Some(short reason) iff a config syntax error, an unsupported `version`,
    /// or an unreadable config file made everything read-only (from
    /// `Session::config_fail_closed()`). None otherwise, including no config
    /// and no open database. The UI shows this; it never infers fail-closed
    /// from diagnostic text.
    pub config_fail_closed: Option<String>,
    pub diagnostics: Vec<ConfigDiagnostic>,
    pub error_count: u32, pub warning_count: u32,
    pub tables: Vec<TableView>,
    pub backup_path: Option<String>,
    pub recent: Vec<RecentPairView>,
    pub state_warning: Option<String>,
}
pub enum DbmanagerErrorKind {
    NoDatabase, NotAbsolute, NotAFile, DatabaseUnreadable, ConfigUnreadable,
    DropAmbiguous, UnknownTable, UnknownObject, UnknownRelation, NotEditable,
    ValidationFailed, Conflict, SchemaChanged, ConfigChangedOnDisk,
    DatabaseReadOnly, Busy, BackupFailed, ImpactChanged, DeleteBlocked,
    ConstraintFailed, Io,
}
pub struct DbmanagerError {
    pub kind: DbmanagerErrorKind,
    pub message: String,                                  // technical detail
    #[serde(skip_serializing_if = "Option::is_none")] #[ts(optional)]
    pub field_errors: Option<Vec<FieldError>>,            // ValidationFailed
    #[serde(skip_serializing_if = "Option::is_none")] #[ts(optional)]
    pub link_errors: Option<Vec<LinkError>>,              // ValidationFailed (when any)
    #[serde(skip_serializing_if = "Option::is_none")] #[ts(optional)]
    pub current: Option<ObjectView>,                      // Conflict
    #[serde(skip_serializing_if = "Option::is_none")] #[ts(optional)]
    pub conflict_link: Option<LinkRef>,                   // Conflict on a link
    #[serde(skip_serializing_if = "Option::is_none")] #[ts(optional)]
    pub impact: Option<DeleteImpact>,                     // ImpactChanged, DeleteBlocked
}
```

`DbmanagerErrorKind` serialises as the variant name (like
`ScheduleErrorKind`). Mapping: `OpenError::NotAbsolute`/`NotAFile` → same
kinds; `NotUnicode`/`NotADatabase`/`Io` → `DatabaseUnreadable`;
`ConfigLoadError` → `ConfigUnreadable` (in the config slot); `Validation` →
`ValidationFailed` + `field_errors` (always present, possibly empty) +
`link_errors` (present when non-empty); `Conflict` → + `current` (+
`conflict_link` when the conflict is on a link);
`ImpactChanged`/`DeleteBlocked` → + `impact`; `ReadOnlyDatabase` →
`DatabaseReadOnly`; `Constraint` → `ConstraintFailed`; `Sqlite` → `Io`; the
rest by name. Every message for a failed write ends by saying nothing was
changed.

### Commands (`tools/dbmanager/mod.rs`)

| Command | Arguments (camelCase on the JS side) | Returns |
| --- | --- | --- |
| `dbmanager_session` | — | `DbmanagerSession` |
| `dbmanager_open` | `database: PathBuf, config: Option<PathBuf>` | `DbmanagerSession` |
| `dbmanager_set_config` | `config: Option<PathBuf>` | `DbmanagerSession` |
| `dbmanager_open_dropped` | `paths: Vec<PathBuf>` | `DbmanagerSession` |
| `dbmanager_reload` | — | `DbmanagerSession` |
| `dbmanager_close` | — | `DbmanagerSession` |
| `dbmanager_list` | `table: String, query: ListQuery` | `ObjectPage` |
| `dbmanager_object` | `table: String, key: ObjectKey` | `ObjectView` |
| `dbmanager_m2m_options` | `table: String, key: Option<ObjectKey>, relation: String, query: M2mQuery` | `M2mPage` |
| `dbmanager_save` | `request: SaveRequest` | `SaveResult` |
| `dbmanager_delete_preview` | `table: String, key: ObjectKey` | `DeleteImpact` |
| `dbmanager_delete` | `table: String, key: ObjectKey, version: String, impactToken: String` | `DeleteResult` |

All return `Result<_, DbmanagerError>`. After `save` and `delete`, the
session's `backup_path` reflects the backup.

### Exports and tests in `generated_types.rs`

- Append `DbmanagerSession`, `DbmanagerError`, `DbmanagerErrorKind` to
  `EXPORTERS`; regenerate.
- Add a test like `error_kind_is_string_union` for `DbmanagerErrorKind.ts`
  with its own pinned list of the 21 kinds (changing it is a UI contract
  change).

### Client (`apps/desktop/src/lib/dbmanager.ts`)

One exported function per command, built on `call` from `./tauri`, typed
with the generated types, in the style of `lib/schedule.ts` (doc comment per
function stating what it does and which errors matter). **The names are
pinned** (the UI task plans import them):

| Export | Command | Arguments passed |
| --- | --- | --- |
| `session()` | `dbmanager_session` | none |
| `open(database, config)` | `dbmanager_open` | `{ database, config }` (`config: string \| null`) |
| `setConfig(config)` | `dbmanager_set_config` | `{ config }` |
| `openDropped(paths)` | `dbmanager_open_dropped` | `{ paths }` |
| `reload()` | `dbmanager_reload` | none |
| `close()` | `dbmanager_close` | none |
| `list(table, query)` | `dbmanager_list` | `{ table, query }` |
| `object(table, key)` | `dbmanager_object` | `{ table, key }` |
| `m2mOptions(table, key, relation, query)` | `dbmanager_m2m_options` | `{ table, key, relation, query }` (`key: ObjectKey \| null`) |
| `save(request)` | `dbmanager_save` | `{ request }` |
| `deletePreview(table, key)` | `dbmanager_delete_preview` | `{ table, key }` |
| `deleteObject(table, key, version, impactToken)` | `dbmanager_delete` | `{ table, key, version, impactToken }` |

Plus helpers the UI needs so it holds no storage logic (names pinned too):

- `boolValue(checked: boolean): DbValue` → integer `"1"`/`"0"`;
- `durationValue(text: string): DbValue` → `{ type: "duration", value: text }`
  (the user's text as typed; the backend and the validation mirror parse it);
- `linkKey(relation: string, other: ObjectKey): string` → a stable string for
  keying staged link edits in UI state (e.g. `relation + "\u0000" +
  JSON.stringify(other)`), so the UI never inspects keys;
- `isConfirmDeleteAllowed(impact: DeleteImpact): boolean` → `!impact.blocked`;
- `isDbmanagerError(e: unknown): e is DbmanagerError`.

`deleteObject`'s doc comment says: always call `deletePreview` first and show
its impact; pass back its `version` and `token`. `session`'s doc comment says
it is cheap after the first call and safe to call on window focus.

`apps/desktop/src/lib/dbmanager.test.ts`: as `lib/schedule.test.ts`, one
`it.each` row per command asserting the command name and argument object
(including a `save` request with `links` holding an add with fields, an update
with a version, and a remove), plus the helpers.

### Documentation

`CLAUDE.md`: Status (the Database Manager: backend and contract done, UI to
come), Layout (`src-tauri/src/tools/dbmanager/` with its four files,
`src/lib/dbmanager.ts`), and nothing else.

## Files

Create `apps/desktop/src-tauri/src/tools/dbmanager/{mod.rs, core.rs,
types.rs, tests.rs}`, `apps/desktop/src/lib/dbmanager.ts`,
`apps/desktop/src/lib/dbmanager.test.ts`, generated files. Edit
`apps/desktop/src-tauri/src/tools/mod.rs`,
`apps/desktop/src-tauri/src/generated_types.rs`, `CLAUDE.md`.

Do not touch `apps/desktop/src/tools/`, `App.tsx`, styles, capabilities, the
CSP, other tools, or `docs/plans/`.

## Acceptance criteria

- Commands registered in the single `generate_handler!`;
  `builder_hooks_only_in_tool_registry` and `capabilities_are_minimal` pass.
- `tests.rs` (driving `Core` directly, `AppDirs::at(tempdir)`, databases
  built from `examples/dbmanager/schema.sql`): open, snapshot contents
  (tables, diagnostics, counts), recent list persisted and reopened on the
  first `session()` of a new `Core`, failed restore sets `state_warning` and
  keeps the pair, `open_dropped` sorting and `DropAmbiguous`, relative path
  → `NotAbsolute`, directory → `NotAFile`, non-database → in the database
  slot as `DatabaseUnreadable`, unreadable config → config slot
  `ConfigUnreadable` with the database still open and every table read-only,
  no config → tables editable, `NoDatabase` for reads without a session,
  each `SessionError` maps to its kind with the right payload field (incl.
  `link_errors` and `conflict_link`, and their absence from the JSON when
  empty/none), a save through `Core` with link changes on
  `m2m:contribution`, `backup_path` set after the first save.
- **Snapshot pins** (tests in `tests.rs`):
  - `configFailClosed` is a non-empty reason for a config with a TOML syntax
    error, for `version = 2`, and for an unreadable config file (all tables
    read-only in each case); it is `null` with no config, with a valid
    config, with a config that has only entry-level errors, and with no
    database open. The serialised JSON key is `configFailClosed`.
  - `session()` called repeatedly after the first call: the database file's
    bytes and the recent store file's bytes and modification time are
    unchanged, no backup appears, and (with a counting hook or by asserting
    the `Session` is the same instance) no reopen or re-introspection happens;
    editing the config file between two calls flips `configChanged` from
    `false` to `true` without any other call; restoring its old bytes flips it
    back.
  - `TableView.objectName` and `M2mQuery.only` pass through the commands
    (an `object_name` from the config reaches the snapshot; an `only` query
    returns one option).
- `dbmanager.test.ts` asserts every export name in the pinned table and its
  command and argument object.
- The kind-list test passes; generated types current; no `bigint`.
- `npm run typecheck && npm run lint && npm run test` pass, including
  `dbmanager.test.ts`.
- On the Windows CI job, the exe (which now links SQLite) passes the dumpbin
  check, including the `sqlite3*.dll` rule added in M1.
- fmt, clippy pedantic, `cargo test --workspace` pass.

## Tests proving completion

Shell: `open_builds_snapshot`, `session_is_cheap_and_side_effect_free`,
`session_recomputes_config_changed`, `config_fail_closed_in_snapshot`,
`recent_persisted_and_restored`,
`failed_restore_warns_and_keeps_pair`, `open_dropped_sorts_by_extension`,
`open_dropped_ambiguous`, `open_rejects_relative_and_directory`,
`non_database_is_unreadable`, `unreadable_config_keeps_database_open_read_only`,
`no_config_is_editable`, `save_with_links_through_core`,
`link_errors_and_conflict_link_serialised`,
`reads_without_session_fail`, `errors_map_to_kinds`,
`backup_path_after_first_save`, `set_config_and_reload`, `close_clears`;
`generated_types.rs`: the new kind-list test. Frontend: `dbmanager.test.ts`.

Run: `cargo test --workspace`; regenerate as in `CLAUDE.md`;
`npm run typecheck && npm run lint && npm run test`.

## Out of scope

- Any view, the registry entry, styles, or copy: the ui-implementer's U tasks.
- The TypeScript validation mirror: its own milestone
  (`lib/dbmanagerValidation.ts`).
- A file watcher for the config (not in v1: `configChanged` is computed on
  each snapshot and write).

## Risks

- `spawn_blocking` closures need `'static` data: clone the `Arc` and owned
  arguments in.
- Restoring the most recent pair on first call opens a file that may be on a
  slow share; it runs on the blocking worker, so the UI stays responsive.
