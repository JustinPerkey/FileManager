# M10 — Tauri commands, session snapshot, errors, and the typed client

Status: awaiting approval. Renumbered from M9 and revised 2026-11-16 for
link errors and conflicts, the duration helper and the fail-closed config.
Then the ui-designer's contract requests were added: `configFailClosed`,
pinned client names, a cheap `session()`, `objectName` and `M2mQuery.only`.
Then the human's final answers were folded in: one fixed app-wide config, so
`setConfig` and config pairs are gone; recent databases; the config hash on
writes; `pickable` tables; and the `OpenerFailed` kind.
Project: dbmanager   Depends on: M9 (landed)

## Goal

Expose the Database Manager to the frontend. That means:

- the `dbmanager_*` commands;
- the session snapshot, including the app-wide config's location and status;
- the error types;
- the list of recent databases;
- the typed client `apps/desktop/src/lib/dbmanager.ts`.

This is **the UI contract**: no UI task starts before it lands. A later
milestone adds the config file watcher, its event and the developer-mode
"Edit config" command on top of this contract.

## Context

The Database Manager (`dbmanager`) is a tool in the FileManager Tauri app.
**There is one config for the whole app**, at a fixed location the user never
chooses:

- `fm_core::system_config_dir()?.join("dbmanager.toml")`;
- on Windows that is `%ProgramData%\FileManager\dbmanager.toml`;
- on other platforms it is `/etc/filemanager/dbmanager.toml`.

The app only reads it. The user picks only databases.

All the database logic lives in `crates/fm-dbmanager`:

- `db.rs`: `Database::open` → `OpenError` (tuple-like variants that carry
  the `PathBuf`: `NotAbsolute(path)`, …, `Busy(path)`). Its variants are `{NotAbsolute, NotAFile, Busy,
  NotUnicode, NotADatabase, Io}`.
- `state.rs`:
  - `RememberedState { recent: Vec<PathBuf> }` (recent databases);
  - `touch(database)`, `forget(database)`;
  - `StateData` (tool `"dbmanager"`, name `"state"`).
- `config/load.rs`:
  - `LoadedConfig::load(path)` reads and parses the config;
  - `status()` is `ConfigStatus::{Loaded, Missing, FailClosed}`; `Missing`
    means the editable built-in defaults;
  - also `hash()` (`"missing"` when absent), `fail_closed_reason()`,
    `diagnostics()` (config-only) and `changed_on_disk()`.
- `session/`, the `Session` type:
  - `Session::open(database, LoadedConfig)` and `set_config(LoadedConfig)`
    (re-resolves without reopening);
  - `config()`, `database_path`, `read_only_reason`, `config_fail_closed`,
    `diagnostics`, `table_views`, `config_changed`, `backup_path`;
  - `list`, `object`, `m2m_options`, `save` (the request carries
    `config_hash`), `delete_preview`, and `delete(…, config_hash)`.
- `SessionError` variants:
  - `Open(OpenError)`, `UnknownTable`, `UnknownObject`, `UnknownRelation`;
  - `SchemaChanged`, `Busy`, `Sqlite`, `ReadOnlyDatabase`;
  - `ConfigChangedOnDisk` (stale config hash, or the file changed);
  - `NotEditable`;
  - `Validation { fields: Vec<FieldError>, links: Vec<LinkError> }`;
  - `Conflict { current: Box<ObjectView>, link: Option<LinkRef> }`;
  - `BackupFailed`, `Constraint`;
  - `ImpactChanged(Box<DeleteImpact>)`, `DeleteBlocked(Box<DeleteImpact>)`.
- Boundary types already exported to `apps/desktop/src/lib/generated/`:
  - values and rules: `DbValue` (incl. `duration`), `FieldKind`,
    `FieldRule`, `DurationUnit`, `FieldIssue`, `FieldError`,
    `FieldIssueCode`;
  - config: `ConfigDiagnostic`, `ConfigStatus`;
  - modes: `TableMode` (incl. `pickable`), `FieldMode`, `RelationMode`;
  - tables: `TableView` (with `objectName`), `FieldView`, `ForeignKeyView`,
    `ReverseRelationView`, `M2mView` (with `linkFields`);
  - reads: `ObjectKey`, `ListQuery`, `ObjectPage`, `ObjectView`, `M2mQuery`
    (with an optional `only` key), `M2mPage` (options with `linkValues` and
    `linkVersion`);
  - writes: `SaveRequest` (with `configHash`, and `links: LinkChange[]` of
    `add`/`update`/`remove`), `LinkError`, `LinkRef`, `SaveResult`;
  - delete: `DeleteImpact` (groups with `links` and `linkData`),
    `DeleteResult`.

Follow `CLAUDE.md` "Adding a tool", steps 2–5. Use the existing tools as
patterns:

- `apps/desktop/src-tauri/src/tools/schedule/`: slots, drops, the error type.
- `tools/pipeline/`: work on a blocking worker.
- `tools/tarpack/core.rs`: the store, restoring the most recent file on the
  first session call, and `stateWarning`.

Rules that bind this task:

- `tools/dbmanager/mod.rs` contributes only `#[tauri::command]` functions
  named `dbmanager_*` and `pub fn state()`. It never touches the `Builder`.
  Only `tools/mod.rs::register` calls `.invoke_handler(` / `.setup(`, and a
  test enforces it. Add `.manage(dbmanager::state())` and the commands to the
  **single** `generate_handler!`. Dbmanager needs no `setup`.
- Tools never import each other.
- `capabilities/default.json` and the CSP in `tauri.conf.json` do **not**
  change in this milestone. The UI needs only the Open dialog and drag-drop,
  which are already granted. `capabilities_are_minimal` must still pass. A
  later milestone makes the reviewed window-close capability change.
- A boundary type is defined once in Rust and generated. `lib/generated/` is
  never hand-edited, and no type uses `bigint`. No name may already exist in
  `lib/generated/`; note that `FileSlot` belongs to the Schedule Creator.
- The implementer owns everything under `apps/desktop/src/lib/`. Do not touch
  the rest of `apps/desktop/src/`: views, the registry and styles belong to
  the ui-implementer.
- fmt and clippy pedantic must pass, with only a scoped `#[expect]`.
- **Tests never touch real user files or the real config location.**
  - `Core` takes the config path as a parameter, and tests pass a path inside
    their temp dir.
  - `AppDirs::at(tempdir)` is used for the store.
  - Only `state()` resolves the real path.
- Paths arrive as `PathBuf`. Display strings (`to_string_lossy`) appear only
  in snapshot fields meant for display.

### Threading

State: `pub struct DbmanagerState { core: Arc<Mutex<Core>> }`. Every command
is `async` and runs its work with `tauri::async_runtime::spawn_blocking`,
cloning the `Arc` into the closure. Opening, introspecting, backups and big
deletes can take seconds, and sync commands would run on the main thread.
Lock poisoning recovers with `PoisonError::into_inner`, as the other tools
do. A `JoinError` maps to `Io`.

### Core (`tools/dbmanager/core.rs`, no Tauri types)

`Core::new(dirs: &AppDirs, config_path: Option<PathBuf>)`. `state()` passes
`fm_core::system_config_dir().ok().map(|d| d.join("dbmanager.toml"))`.

`Core` holds:

- `Option<Store<RememberedState>>`, or an "unpersisted" reason, as tarpack
  does when `AppDirs::from_system` fails;
- the config path;
- the current `LoadedConfig`, always loaded, even with no database open;
- the database slot (path plus an optional error);
- the `Option<Session>`;
- whether the first-call restore ran;
- a `state_warning`.

**If the config path is `None`** (the config directory could not be
determined), the config is treated as fail-closed with the reason "the
config location could not be determined", and everything is read-only. It
never falls back to the editable defaults.

Operations:

- `session()` returns the snapshot. On the first call it loads the config and
  reopens the most recent database. If the database is missing or fails to
  open, nothing is open, `state_warning` says which database and why, the
  entry stays in the list, and the view shows its empty state.
- **`session()` after the first call is cheap and side-effect free.** The
  view calls it on every window focus.
  - It opens nothing, runs no query against user tables, writes nothing (not
    the database, not the recent store), takes no backup, does not
    re-introspect, and does **not** reload the config.
  - Its only I/O is re-reading and hashing the config file, so
    `config_changed` is **recomputed on every call**.
  - Everything else in the snapshot comes from state already held.
- `open(database)`:
  - The path must be absolute (`NotAbsolute`) and a file (`NotAFile`). Both
    are checked before opening.
  - The database opens with the current `LoadedConfig`.
  - On success, the database is touched in the recent list and saved.
- `open_dropped(paths)`:
  - Exactly one path is opened like `open`.
  - More than one path gives `DropAmbiguous`, and nothing changes.
  - A dropped `.toml` is not special. It fails as `DatabaseUnreadable`, and
    the message adds that the config is not chosen by dropping and names its
    fixed path.
- `reload()` reloads the config and, if a database is open, reopens it
  (re-introspects) with the new config. It works with no database open: it
  then only reloads the config.
- `reload_config()` (`pub(crate)`, used by the write path here and by the
  watcher in a later milestone) runs `LoadedConfig::load(path)`, then
  `session.set_config(…)` if a database is open.
- `close()` drops the session and clears the database slot. The config stays
  loaded.
- `list`, `object`, `m2m_options`, `save`, `delete_preview` and `delete`
  forward to the session, giving `NoDatabase` if there is none.
  - When `save`/`delete` returns `ConfigChangedOnDisk`, `Core` calls
    `reload_config()` **before** returning the error. The client re-fetches
    the session, re-checks its draft against the new rules, and retries with
    the new hash. That retry succeeds unless the rules now reject the draft.

### Types (`tools/dbmanager/types.rs`)

```rust
pub struct DbFileSlot { pub path: String, pub error: Option<DbmanagerError> }
pub struct DbConfigView {
    /// Display path of the app-wide config; None if its location could not be
    /// determined.
    pub path: Option<String>,
    pub status: ConfigStatus,            // "loaded" | "missing" | "failClosed"
    /// The hash writes must carry (`SaveRequest.configHash`, `deleteObject`'s
    /// `configHash`): SHA-256 hex of the file, or "missing".
    pub hash: String,
}
pub struct DbmanagerSession {
    pub database: Option<DbFileSlot>,
    pub config: DbConfigView,            // always present, even with no database open
    pub open: bool,
    pub read_only_reason: Option<String>,
    /// The file on disk differs from the loaded config (normally only for the
    /// moment before the automatic reload; or if the watcher could not run).
    pub config_changed: bool,
    /// Some(short reason) iff a config syntax error, an unsupported `version`,
    /// an unreadable config file, or an undeterminable config location made
    /// everything read-only. None otherwise, including a missing config (that
    /// means the editable defaults). The UI shows this; it never infers
    /// fail-closed from diagnostic text.
    pub config_fail_closed: Option<String>,
    /// With a database open: the session's diagnostics (config + schema).
    /// Without: the config-only diagnostics. Always with line/col where they
    /// come from the config.
    pub diagnostics: Vec<ConfigDiagnostic>,
    pub error_count: u32, pub warning_count: u32,
    pub tables: Vec<TableView>,          // includes pickable tables (mode "pickable"): not for the table list
    pub backup_path: Option<String>,
    pub recent: Vec<String>,             // recent databases, display paths, most recent first
    pub state_warning: Option<String>,
}
pub enum DbmanagerErrorKind {
    NoDatabase, NotAbsolute, NotAFile, DatabaseUnreadable, OpenerFailed,
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

`ConfigUnreadable` is gone: an unreadable config is a status
(`config.status == "failClosed"` with `configFailClosed`), not a command
error. `OpenerFailed` replaces it. The later "Edit config" command returns it
when the system editor cannot be started. There are still 21 kinds.

`DbmanagerErrorKind` serializes as the variant name, like
`ScheduleErrorKind`. Mapping:

| From | To |
| --- | --- |
| `OpenError::NotAbsolute`, `NotAFile` | the same kinds |
| `OpenError::Busy` | `Busy` (the database is locked by another program) |
| `OpenError::NotUnicode`, `NotADatabase`, `Io` | `DatabaseUnreadable` |
| `Validation` | `ValidationFailed`, with `field_errors` (always present, possibly empty) and `link_errors` (present when non-empty) |
| `Conflict` | `Conflict`, with `current`, plus `conflict_link` when the conflict is on a link |
| `ImpactChanged`, `DeleteBlocked` | the same kinds, with `impact` |
| `ReadOnlyDatabase` | `DatabaseReadOnly` |
| `ConfigChangedOnDisk` | `ConfigChangedOnDisk`; the message says the config was reloaded and the change should be checked and saved again |
| `Constraint` | `ConstraintFailed` |
| `Sqlite` | `Io` |
| the rest | the same name |

Every message for a failed write ends by saying nothing was changed.

### Commands (`tools/dbmanager/mod.rs`)

| Command | Arguments (camelCase on the JS side) | Returns |
| --- | --- | --- |
| `dbmanager_session` | none | `DbmanagerSession` |
| `dbmanager_open` | `database: PathBuf` | `DbmanagerSession` |
| `dbmanager_open_dropped` | `paths: Vec<PathBuf>` | `DbmanagerSession` |
| `dbmanager_reload` | none | `DbmanagerSession` |
| `dbmanager_close` | none | `DbmanagerSession` |
| `dbmanager_list` | `table: String, query: ListQuery` | `ObjectPage` |
| `dbmanager_object` | `table: String, key: ObjectKey` | `ObjectView` |
| `dbmanager_m2m_options` | `table: String, key: Option<ObjectKey>, relation: String, query: M2mQuery` | `M2mPage` |
| `dbmanager_save` | `request: SaveRequest` | `SaveResult` |
| `dbmanager_delete_preview` | `table: String, key: ObjectKey` | `DeleteImpact` |
| `dbmanager_delete` | `table: String, key: ObjectKey, version: String, impactToken: String, configHash: String` | `DeleteResult` |

All return `Result<_, DbmanagerError>`. After `save` and `delete`, the
session's `backup_path` reflects the backup. `dbmanager_set_config` no longer
exists.

### Exports and tests in `generated_types.rs`

- Append `DbmanagerSession`, `DbmanagerError` and `DbmanagerErrorKind` to
  `EXPORTERS`, then regenerate. `DbConfigView` follows from them.
- Add a test like `error_kind_is_string_union` for `DbmanagerErrorKind.ts`,
  with its own pinned list of the 21 kinds above. Changing that list is a UI
  contract change.

### Client (`apps/desktop/src/lib/dbmanager.ts`)

One exported function per command, built on `call` from `./tauri` and typed
with the generated types, in the style of `lib/schedule.ts`. Each function
has a doc comment saying what it does and which errors matter. **The names
are pinned**, because the UI task plans import them:

| Export | Command | Arguments passed |
| --- | --- | --- |
| `session()` | `dbmanager_session` | none |
| `open(database)` | `dbmanager_open` | `{ database }` |
| `openDropped(paths)` | `dbmanager_open_dropped` | `{ paths }` |
| `reload()` | `dbmanager_reload` | none |
| `close()` | `dbmanager_close` | none |
| `list(table, query)` | `dbmanager_list` | `{ table, query }` |
| `object(table, key)` | `dbmanager_object` | `{ table, key }` |
| `m2mOptions(table, key, relation, query)` | `dbmanager_m2m_options` | `{ table, key, relation, query }` (`key: ObjectKey \| null`) |
| `save(request)` | `dbmanager_save` | `{ request }` (`request.configHash` = `session.config.hash` the draft was checked against) |
| `deletePreview(table, key)` | `dbmanager_delete_preview` | `{ table, key }` |
| `deleteObject(table, key, version, impactToken, configHash)` | `dbmanager_delete` | `{ table, key, version, impactToken, configHash }` |

The pinned helpers keep storage logic out of the UI:

- `boolValue(checked: boolean): DbValue` returns integer `"1"`/`"0"`.
- `durationValue(text: string): DbValue` returns
  `{ type: "duration", value: text }`. It passes the user's text as typed;
  the backend and the validation mirror parse it.
- `linkKey(relation: string, other: ObjectKey): string` returns a stable
  string for keying staged link edits in UI state, for example
  `relation + "\u0000" + JSON.stringify(other)`, so the UI never inspects
  keys.
- `isConfirmDeleteAllowed(impact: DeleteImpact): boolean` returns
  `!impact.blocked`.
- `isDbmanagerError(e: unknown): e is DbmanagerError`.

Doc comments to include:

- `deleteObject`: always call `deletePreview` first and show its impact; pass
  back its `version` and `token`, and the `config.hash` in force.
- `session`: cheap after the first call and safe to call on window focus.
- `save` and `deleteObject`: on `ConfigChangedOnDisk`, re-fetch the session,
  re-check the draft, and retry.

`apps/desktop/src/lib/dbmanager.test.ts` follows `lib/schedule.test.ts`: one
`it.each` row per command asserting the command name and argument object,
plus the helpers. The `save` row's request carries `configHash` and `links`
holding an add with fields, an update with a version, and a remove.

### Documentation

Update `CLAUDE.md`:

- Status: the Database Manager's backend and contract are done, and the UI
  is to come.
- Layout: `src-tauri/src/tools/dbmanager/` with its four files, and
  `src/lib/dbmanager.ts`.
- One line saying the app-wide config is
  `%ProgramData%\FileManager\dbmanager.toml` and is read-only for the app.

## Files

Create:

- `apps/desktop/src-tauri/src/tools/dbmanager/{mod.rs, core.rs, types.rs,
  tests.rs}`;
- `apps/desktop/src/lib/dbmanager.ts` and
  `apps/desktop/src/lib/dbmanager.test.ts`;
- the generated files.

Edit:

- `apps/desktop/src-tauri/src/tools/mod.rs`;
- `apps/desktop/src-tauri/src/generated_types.rs`;
- `CLAUDE.md`.

Do not touch `apps/desktop/src/tools/`, `App.tsx`, styles, capabilities, the
CSP, other tools, or `docs/plans/`.

## Acceptance criteria

**Registration**

- The commands are registered in the single `generate_handler!`, and there is
  no `dbmanager_set_config`.
- `builder_hooks_only_in_tool_registry` and `capabilities_are_minimal` pass.

**`tests.rs`** drives `Core` directly, with `AppDirs::at(tempdir)`, the
config path inside the temp dir, and databases built from
`examples/dbmanager/schema.sql`. It covers:

- open, and the snapshot contents (tables incl. `publisher` with mode
  `pickable`, diagnostics, counts);
- the recent databases list, persisted and reopened on the first
  `session()` of a new `Core`;
- a missing recent database on restore: empty state, `state_warning` set,
  entry kept;
- `open_dropped` with one file, `DropAmbiguous` for two, and a `.toml`
  rejected with a message naming the config path;
- a relative path → `NotAbsolute`, a directory → `NotAFile`, and a
  non-database → `DatabaseUnreadable` in the database slot;
- `NoDatabase` for reads without a session;
- each `SessionError` mapping to its kind with the right payload field,
  including `link_errors` and `conflict_link`, and their absence from the
  JSON when empty or none;
- a save through `Core` with link changes on `m2m:contribution`;
- `backup_path` set after the first save;
- `close` keeping the config loaded.

**Config pins**

Each case is tested:

- **Config file absent:** `config.status` is `"missing"`, `config.hash` is
  `"missing"`, `configFailClosed` is `null`, and tables are editable per the
  defaults.
- **Valid file:** `"loaded"`, with the file's hash; `example.toml` resolves
  as specified.
- **Syntax error, `version = 2`, or a directory at the config path:**
  `"failClosed"`, `configFailClosed` is a non-empty reason, and every table
  is read-only.
- **Config path `None`:** `"failClosed"`, with the reason "the config
  location could not be determined".
- **No database open:** the snapshot still carries `config` and the
  config-only diagnostics, with line/col.
- **JSON keys:** `config`, `configFailClosed`, `recent` (strings).

**Writes against a changed config**

- A save with a stale `configHash` gives `ConfigChangedOnDisk`. After it,
  `session().config.hash` is the new hash and `tables` reflect the new
  config.
- The same applies when the file changed but no reload has happened yet: the
  write path reloads.
- A retry with the new hash succeeds.
- The same holds for `deleteObject`.

**`session()` cost**

When `session()` is called repeatedly after the first call:

- the database file's bytes, and the recent store file's bytes and
  modification time, are unchanged;
- no backup appears;
- no reopen, re-introspection or config reload happens (use a counting hook,
  or assert the `Session` is the same instance);
- editing the config file between two calls flips `configChanged` from
  `false` to `true` with no other call, and restoring its old bytes flips it
  back.

**Pass-through**

- `TableView.objectName` and `M2mQuery.only` pass through the commands: an
  `object_name` from the config reaches the snapshot, and an `only` query
  returns one option.

**Frontend and tooling**

- `dbmanager.test.ts` asserts every export name in the pinned table, with its
  command and argument object.
- The kind-list test passes. Generated types are current, with no `bigint`.
- `npm run typecheck && npm run lint && npm run test` pass, including
  `dbmanager.test.ts`.
- On the Windows CI job, the exe passes the dumpbin step **with SQLite
  linked in**: this milestone is the first whose non-test code uses
  `fm-dbmanager`, so check that it does (for example `dbmanager_open` calls
  `Database::open`), and that the step lists no `sqlite3*.dll`, no CRT DLL,
  and no other forbidden import. A green step without SQLite linked proves
  nothing.
- fmt, clippy pedantic and `cargo test --workspace` pass.

## Tests proving completion

Shell (`tests.rs`):

- Snapshot and `session()`:
  - `open_builds_snapshot`
  - `session_is_cheap_and_side_effect_free`
  - `session_recomputes_config_changed`
- Config status:
  - `config_missing_is_editable_defaults`
  - `config_loaded_status_and_hash`
  - `config_fail_closed_in_snapshot`
  - `config_location_unknown_fails_closed`
  - `config_shown_without_database`
- Writes against a changed config:
  - `stale_config_hash_reloads_and_refuses`
  - `retry_with_new_hash_succeeds`
  - `delete_stale_config_hash`
- Recent databases and restore:
  - `recent_databases_persisted_and_restored`
  - `failed_restore_warns_and_keeps_entry`
- Opening and dropping:
  - `open_dropped_single`
  - `open_dropped_ambiguous`
  - `open_dropped_toml_explained`
  - `open_rejects_relative_and_directory`
  - `non_database_is_unreadable`
- Saves and errors:
  - `save_with_links_through_core`
  - `link_errors_and_conflict_link_serialised`
  - `reads_without_session_fail`
  - `errors_map_to_kinds`
  - `backup_path_after_first_save`
- Reload and close:
  - `reload_with_and_without_database`
  - `close_keeps_config`

`generated_types.rs`: the new kind-list test.

Frontend: `dbmanager.test.ts`.

Run `cargo test --workspace`, regenerate as in `CLAUDE.md`, then
`npm run typecheck && npm run lint && npm run test`.

## Out of scope

- Any view, the registry entry, styles, or copy. These belong to the
  ui-implementer's U tasks.
- The TypeScript validation mirror, which has its own milestone
  (`lib/dbmanagerValidation.ts`).
- The config file **watcher**, the `config-changed` event, `onConfigChanged`,
  and the developer-mode **Edit config** command (with its starter file).
  These come in a later milestone. Until then, the config reloads on
  `reload()` and on a write that finds it changed.
- The window-close guard and its capability. These come in a later
  milestone.

## Risks

- `spawn_blocking` closures need `'static` data. Clone the `Arc` and the
  owned arguments into them.
- Restoring the most recent database on the first call opens a file that may
  be on a slow share. It runs on the blocking worker, so the UI stays
  responsive.
- `%ProgramData%` is shared by every user of the machine. The app only reads
  it, so read permission is all it needs.
