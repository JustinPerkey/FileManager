# Project plan: the Database Manager tool

Status: **awaiting approval; UI-facing milestones re-planned for GPUI on 2026-11-21** (see the migration note below). First written by the planner on 2026-11-13,
then revised three times on 2026-11-16:

1. The human answered four open questions (§8.1). Defaults became editable,
   dates were removed, a `duration` kind was added, and junction extra
   columns became editable. Link editing became its own milestone (M8).
2. The ui-designer's contract requests were applied (§8.4).
3. The human answered every remaining question (§8.1):
   - a `pickable` table mode;
   - **one fixed app-wide config** in `%ProgramData%`, which the user does not
     choose and which reloads automatically;
   - a developer-mode "Edit config" command;
   - recent databases instead of database/config pairs;
   - a prompt when closing the window with unsaved edits.

   Two milestones were added for these: M13 (config watcher and Edit config)
   and M14 (the close guard, a capability change). They are numbered after
   M12 so that the milestone numbers the UI plans cite do not change. §5
   gives the order.

No open questions remain (§8.2). The UI changes caused by the last round are
listed for the ui-designer in §7.1.

The UI is not designed here. §7 hands it to the `ui-designer`, who writes
[`dbmanager-ui.md`](dbmanager-ui.md) and the `U<n>` task plans.

Task plans: [`../tasks/dbmanager/`](../tasks/dbmanager/).

**Migration to GPUI (2026-11-21).** The human decided to replace the Tauri +
React UI with GPUI ([`gpui-migration.md`](gpui-migration.md), §11 there).
For this tool that means:

- **Unchanged:** M1–M6 (done) and M7–M9 (save, links, delete) in
  `fm-dbmanager`. If M7–M9 land before the migration's cutover (its M9),
  they keep deriving `ts_rs::TS` and appending to `EXPORTERS`; after it,
  they skip both. Their task plans say so.
- **Superseded:** M10 (Tauri contract) by gpui-migration **M12a–M12c**
  (tool core in `fm_dbmanager::tool`, backend trait, view-model with drafts,
  save, links, delete); M13 (config watch, Edit config) by gpui-migration
  **M2b** (`FileWatcher` moves to `fm-core`) and **M12d**; M12 (docs and
  release) by gpui-migration **M13**.
- **Dropped:** M11 (TS validation mirror): the model calls
  `rules::validate` directly; M14 (window-close guard): the GPUI app's close
  guard (gpui-migration M3b), with dbmanager registering its drafts (M12b).
- **New:** [M15](../tasks/dbmanager/M15-pattern-syntax.md) allows the full
  Rust `regex` syntax in config patterns, lifting every old Rust/JavaScript
  restriction (the human's decision, 2026-11-21; any time).
- **UI:** `dbmanager-ui.md`'s design decisions carry over; its implementation
  parts and U1–U13 are superseded and are rewritten by the ui-designer for
  GPUI.
- Sections below that describe Tauri commands, `lib/*.ts`, generated types,
  capabilities or the CSP (§2.1 shell lines, §2.6, §2.8, §2.11, the
  "Boundary types" and "Security settings" rules of §3, §4's last sentence)
  are **historical**; the replacements are in the gpui-migration task plans
  named above.


## 1. Request

From the user, with their answers to the clarifying questions:

- Add a **Database Manager** tool (`dbmanager`) to FileManager.
- **The engine is a SQLite file the user picks.** The app stays a portable
  Windows exe with no server, so SQLite is bundled and statically linked.
- **A TOML config sets the bounds of each field.** Only some fields are
  configurable.
  - There is **one config for the whole app**, at
    `%ProgramData%\FileManager\dbmanager.toml`.
  - The user does not choose it, and it is mostly hidden. The app only reads
    it.
  - When it changes, the app reloads it automatically.
  - When it is missing, everything is editable by default, and a notice says
    that no config is in use.
  - In developer mode, an **Edit config** action opens it in the system
    editor, creating a commented starter file if there is none.
- **The config classifies tables and fields by mode:** *editable*,
  *read-only* and *hidden*, plus *pickable* for lookup tables. A pickable
  table is not listed, but its objects can be picked, linked to and opened
  read-only.
  - Table structure, column types and foreign keys come from introspecting
    the SQLite file. The config is an overlay on it.
  - **Anything the config does not mention is editable**, apart from columns
    that can never be edited.
- **Foreign keys are links.** Following one opens the referenced object,
  with back navigation. Editing a FK field picks from the referenced table's
  objects. Objects that reference this one are shown too.
- **Many-to-many relations** show a checklist of **every object of the other
  table**, with search and paging, and with the linked objects ticked.
  - These relations are junction tables, detected from FKs and confirmable
    in the config.
  - **Junction extra columns ("link fields") are editable in v1.**
  - Self-referential many-to-many is not in v1.
- It feels like an **object editor**, not a grid.
- **Booleans render as checkboxes.**
- **No dates.** **Durations** are stored as an INTEGER count of a declared
  unit and typed and shown as `1h30m`.
- **v1 writes:** edit, create and delete objects, and edit many-to-many links
  with their fields.
  - Delete is confirmed and shows its FK impact.
  - Saves are explicit and transactional.
  - Validation runs in Rust and is mirrored in the UI.
  - Concurrent modification and recovery are handled.
- **Closing the window with unsaved edits asks first.**
- Views are ignored. Composite FKs are not editable. BLOBs are shown as
  "N bytes" and never edited.

## 2. Architecture

### 2.1 Layout *(decided)*

```
crates/fm-core/src/dirs.rs        + system_config_dir(): %ProgramData%\FileManager
                                  (/etc/filemanager elsewhere) (M1)
crates/fm-core/src/watch.rs       FileWatcher: debounced, parent-dir watch, moved from
                                  tarpack's watch.rs and shared by both tools (M13)
crates/fm-dbmanager/              domain library; no tauri dependency
  src/db.rs                       Database: open (never create), pragmas, read-only
  src/state.rs                    RememberedState: recent databases (dbmanager/state)
  src/schema/                     introspection; ident.rs (Ident: the only way to put a name in SQL)
  src/config/                     TOML parse + structural validation (line/col);
                                  load.rs: LoadedConfig (status, hash, change detection)
  src/value.rs                    DbValue (boundary), rusqlite conversions
  src/rules/                      FieldKind, FieldRule, duration.rs, validate
  src/model/                      overlay: resolve(schema, config) -> Model + diagnostics
  src/session/                    Session: read.rs, save.rs, links.rs, backup.rs,
                                  delete.rs, sql.rs (the only module that builds SQL text)
examples/dbmanager/
  schema.sql, example.toml        example schema + config (zero diagnostics)
  starter.toml                    commented, inert starter written by "Edit config"
  validation-cases.json           shared validation vectors (Rust + TS)
docs/dbmanager-config.md          the config reference (M3)
docs/dbmanager.md                 the tool's flow, contract and recovery story (M12)
apps/desktop/src-tauri/
  capabilities/default.json       + core:window:allow-destroy (M14, reviewed)
  src/tools/dbmanager/            mod.rs (commands, config watcher, event), core.rs
                                  (config path injected; recent; snapshot; reload),
                                  types.rs, tests.rs
apps/desktop/src/lib/dbmanager.ts             typed client + helpers + onConfigChanged
apps/desktop/src/lib/dbmanagerValidation.ts   TS mirror of rules::validate
apps/desktop/src/lib/tauri.ts                 + guardWindowClose, closeWindow (M14)
apps/desktop/src/tools/dbmanager/             views (ui-implementer)
```

Reasons:

- **The domain crate holds all database logic,** so every rule is testable
  without Tauri. The shell holds the config path, the recent list, the
  watcher, error mapping and threading.
- **The file watcher moves into `fm-core`** because two tools now need it.
  Tools never import each other.
- **Boundary types are generated by ts-rs.**

### 2.2 SQL safety *(decided; binding on every milestone that touches SQL)*

- Values are always bound parameters.
- Identifiers come only from introspection, as `schema::Ident`, and are
  quoted by `Ident::quoted()`. Only `session/sql.rs` assembles SQL text.
- Introspection uses the pragma table functions with bound parameters.
- No `INSERT OR IGNORE` / `OR REPLACE`.
- Tests use names with quotes, spaces, keywords and non-ASCII characters.
- The config never contains SQL.

### 2.3 Opening a database *(decided)*

- **Path rules.** The path must be absolute and name an existing file. It is
  never created, and never treated as a URI.
- **Busy at open.** `busy_timeout` (3 s) is set right after the connection
  opens, before the first statement, so a database locked by another program
  waits and then fails with `Busy` (`OpenError::Busy`), not `DatabaseUnreadable`.
- **Read-only files.** A read-only file, or a writable file in a read-only
  folder (found by a transient temp-file probe; SQLite's journal and the
  app's backup both need the folder), opens read-only, with a
  reason, and writes are refused with `DatabaseReadOnly`.
- **Settings.** The connection sets `foreign_keys = ON`, `busy_timeout`
  3 s, `trusted_schema = OFF` and `SQLITE_DBCONFIG_DEFENSIVE`. The journal
  mode is never changed.
- **Unreadable input.** A file that is not a SQLite database, or a path that
  is not valid Unicode, fails with `DatabaseUnreadable`.

### 2.4 The model: introspection plus overlay *(decided)*

The full rules are in M3's task plan. Once M3 lands, they live in
`docs/dbmanager-config.md`.

**Tables and identity**

- **Object tables** are ordinary tables. Views, virtual tables and shadow
  tables are ignored.
- **`ObjectKey`** is the rowid for rowid tables and the primary-key values
  for `WITHOUT ROWID` tables. It is opaque to the UI.

**Modes**

- **Tables:** `editable | read-only | hidden | pickable`.
- **Fields:** `editable | read-only | hidden | create-only`.
- **Relations:** `editable | read-only | hidden`.

**Pickable** *(new; the human's answer to Q5)* is for lookup tables:

- A pickable table is not in the table list.
- Its objects appear in FK pickers, as navigable links and as the other side
  of many-to-many checklists.
- All of its fields are read-only. It is never created in or deleted from.
- **Following a link to a pickable object opens it read-only.** The user
  must be able to see what they picked, and a table kept out of the list
  must not become editable through a link.
- A pickable table is not hidden, so delete impact labels its rows.
- `pickable` is not valid in `[defaults]`.

**Defaults** *(confirmed)*

- Anything the config does not mention is **editable**.
- `allow_create` is on and `allow_delete` is off by default.
- Relations default to `editable`.

**Forced read-only:**

- the rowid alias;
- generated columns;
- BLOBs (shown as "N bytes");
- untyped columns without a `type`;
- multi-column FKs (*confirmed*);
- FKs into hidden tables.

An inherited `editable` on a primary key becomes `create-only`. An explicit
`editable` on a forced column is an error.

**Fail closed.** These make **everything read-only** until they are fixed:

- a rejected entry makes its subject read-only;
- a TOML syntax error, an unsupported `version` or an unreadable file makes
  everything read-only;
- so does a config location that cannot be determined;
- an invalid `[defaults]` makes the defaults read-only.

A missing config file is not an error. It means the editable built-in
defaults.

**Hidden** means never shown. The exception is delete-impact counts.

**Field kinds** are `integer | real | text | bool | duration | blob | any`.
Bools are stored as `0`/`1`.

**Bounds** are `min`, `max`, `min_length`, `max_length`, `pattern`,
`allowed`, `nullable` and `empty_is_null`. `object_name` is a table's
singular noun (§8.4).

**Create availability** is an error when it was configured explicitly, and a
warning when inherited.

**Diagnostics** carry line/col for config errors, and table/column for
database mismatches. The database always opens.

**Labels** come from a template, or the default label, and are computed in
Rust. Search uses `LIKE` over the label's text and integer columns.

**Many-to-many**

- **Detection.** Strict junctions are auto-detected. Junctions with extra
  columns must be declared.
- **Link fields** are configured as `[tables.<junction>.fields.*]`.
- **Not in v1:** self-referential relations (*confirmed*).
- **Where the checklist appears.** The checklist shows on a side when the
  other side is not hidden; a pickable other side counts as not hidden. It is
  editable when the relation is editable, the side's table is editable, and
  links can be added.

### 2.5 Durations *(decided; confirmed)*

- **Storage.** A duration is stored as an INTEGER count of a declared `unit`:
  `milliseconds`, `seconds` (the default), `minutes`, `hours` or `days`. A
  day is 24 h.
- **Declaration.** A field is a duration only through `type = "duration"`.
  It is never inferred. Its bounds are written `"30s"`/`"1h30m"` or as
  counts, and must be whole units.
- **Input and display.** Input accepts `1h30m`, `1h 30m` or a bare count of
  the unit. Values are displayed in canonical form (`1h30m`). There is no
  `h:mm:ss` form (*confirmed*).
- **Boundary.** Durations cross the boundary as
  `{"type":"duration","value":"1h30m"}`.
- **Validation codes:** `notDuration` and `durationPrecision`.
- **Search** skips duration columns.

### 2.6 Validation, mirrored *(decided; mirror dropped 2026-11-21: only Rust validates, gpui-migration M12b)*

- `rules::validate` is pure and authoritative. Integers, reals and durations
  cross the boundary as strings.
- `lib/dbmanagerValidation.ts` mirrors it. Parity is proven by
  `examples/dbmanager/validation-cases.json`, which both sides run.
- `pattern`: Rust's `regex` is authoritative; JS `RegExp` with the `u` flag
  is used while typing.
- Saving validates only the fields that changed.

### 2.7 Writes, concurrency and recovery *(decided)*

**Transactions**

- One write is one `BEGIN IMMEDIATE` transaction: a save (fields and links)
  or a delete. Any failure rolls everything back and says nothing changed.

**Concurrency**

- Objects carry a `version` and links a `linkVersion`. Link edits are
  strict. A conflict carries the current object and names the link.
- `schema_version` is checked on every command.
- A busy database gives `Busy` after 3 s.

**Config changes** *(changed by the human's answer to Q9: automatic reload)*

- The app watches the config file and reloads it when it changes, then emits
  `dbmanager://config-changed`. The UI keeps its drafts and re-checks them
  against the new rules.
- Writes carry the `configHash` they were checked against. If it is stale,
  or the file changed but has not been reloaded yet, the write is refused
  with `ConfigChangedOnDisk`. The backend has already reloaded by then, so
  the client re-checks and retries with the new hash.
- So a write never applies rules the user has not seen, and never applies
  rules that no longer hold.

**Backup** *(confirmed)*

- The first write of each opened session first writes `<file>.<UTC
  stamp>.bak` beside the database, using the online backup API. It never
  replaces an existing file, and the app never deletes it.
- If the backup fails, the write is refused.

**Delete**

- Delete is a preview followed by a confirmed delete with the version, the
  impact token and `configHash`.
- The impact includes hidden tables (as counts only), link rows, and
  `linkData`.

**Removing a link with fields** is staged and committed only by Save. The
backup covers it.

### 2.8 Boundary contract *(historical: superseded 2026-11-21 by gpui-migration M12a–M12d; no Tauri commands are built)*

All commands are `async`, and the work runs under `spawn_blocking`.

| Command | Returns |
| --- | --- |
| `dbmanager_session()` | `DbmanagerSession`. The first call loads the config and reopens the most recent database. Later calls are cheap and side-effect free, and recompute `configChanged`. |
| `dbmanager_open(database)` | `DbmanagerSession` |
| `dbmanager_open_dropped(paths)` | `DbmanagerSession`. Exactly one database file. A `.toml` is refused with `ConfigNotDroppable` (the config is fixed). |
| `dbmanager_reload()` | `DbmanagerSession`. Reloads the config and re-introspects. It **keeps** the session's backup (`backupPath` unchanged, no second backup). `close`/`open` start a fresh session. |
| `dbmanager_close()` | `DbmanagerSession` |
| `dbmanager_list(table, query)` | `ObjectPage` |
| `dbmanager_object(table, key)` | `ObjectView` |
| `dbmanager_m2m_options(table, key?, relation, query)` | `M2mPage`. `query.only` fetches a single object. |
| `dbmanager_save(request)` | `SaveResult`. The request carries `configHash` and `links`. |
| `dbmanager_delete_preview(table, key)` | `DeleteImpact` |
| `dbmanager_delete(table, key, version, impactToken, configHash)` | `DeleteResult` |
| `dbmanager_edit_config()` (M13) | `EditConfigOutcome { created, session }`. Creates the starter if missing, then opens the system editor. Developer mode only, gated by the UI. |
| event `dbmanager://config-changed` (M13) | `DbmanagerConfigChanged { hash, status }`, emitted after the reload |

`DbmanagerSession` fields:

- `database`;
- `config: { path, status: "loaded" | "missing" | "failClosed", hash }`;
- `open`, `readOnlyReason`, `configChanged`;
- `configFailClosed`, the reason, or null;
- `diagnostics`, with config-only diagnostics when no database is open;
- `errorCount`, `warningCount`;
- `tables`, which include pickable ones, not for the table list;
- `backupPath`;
- `recent`, the recent database paths;
- `stateWarning`.

`DbmanagerErrorKind` has 22 kinds, pinned by a test:

- `NoDatabase`, `NotAbsolute`, `NotAFile`, `DatabaseUnreadable`;
- `OpenerFailed`, which replaces `ConfigUnreadable`;
- `DropAmbiguous`, `ConfigNotDroppable` (a dropped `.toml`);
- `UnknownTable`, `UnknownObject`, `UnknownRelation`, `NotEditable`;
- `ValidationFailed`, `Conflict`;
- `SchemaChanged`, `ConfigChangedOnDisk`, `DatabaseReadOnly`, `Busy`;
- `BackupFailed`, `ImpactChanged`, `DeleteBlocked`, `ConstraintFailed`,
  `Io`.

`lib/dbmanager.ts` exports these pinned names:

- commands: `session`, `open`, `openDropped`, `reload`, `close`, `list`,
  `object`, `m2mOptions`, `save`, `deletePreview`, `deleteObject`;
- M13 additions: `editConfig`, `onConfigChanged`, `CONFIG_CHANGED_EVENT`;
- helpers: `boolValue`, `durationValue`, `linkKey`,
  `isConfirmDeleteAllowed`, `isDbmanagerError`.

`setConfig` is removed. `lib/tauri.ts` gains `guardWindowClose` and
`closeWindow` (M14).

**Capabilities:** M14 adds exactly one permission, `core:window:allow-destroy`
(or the narrowest equivalent its manual check proves). This is a reviewed
decision. **The CSP does not change.**

### 2.9 Persisted state *(decided)*

`RememberedState` holds recent **databases**: at most 10, most recent first,
unique by path. The config is not remembered, because it is fixed.

On start, the last database reopens. If it is missing, the app shows the
empty state and `stateWarning` says why, and the entry is kept (*confirmed*).

### 2.10 The app-wide config file *(decided; the human's answer to Q11)*

- **Location.** The path is `fm_core::system_config_dir()` plus
  `dbmanager.toml`.
  - On Windows this is `%ProgramData%\FileManager\dbmanager.toml`.
    `ProgramData` comes from the environment variable, which must be
    absolute.
  - On other platforms, which are development builds only, it is
    `/etc/filemanager/dbmanager.toml`.
  - It is injectable: `Core::new(dirs, config_path)`. Tests always pass a
    temp path.
  - If the location cannot be determined, the config fails closed.
- **The app only reads it.** It is the same file for every database, and
  names a database does not have are warnings.
- **Statuses:**

  | Status | Meaning | What the user sees |
  | --- | --- | --- |
  | `loaded` | The file was read and parsed. | Entry-level problems are diagnostics. |
  | `missing` | There is no file. | The editable defaults, and a "no config" notice. There is no prompt. |
  | `failClosed` | A syntax error, an unsupported version, an unreadable file, or an unknown location. | Everything is read-only, with a reason. |

- **Watching.** `fm_core::watch::FileWatcher` watches the parent directory,
  or the deepest existing ancestor if the directory does not exist yet. It is
  never recursive. It is armed once, on the first `dbmanager_session`.
- **Reloading.** A reload re-resolves the model without reopening the
  database. It runs after any write in progress. Then the event is emitted.
- **Edit config** is for developer mode only (the app-wide
  Ctrl+Alt+Shift+D, `useDevMode()`), wired like tarpack's "Edit in editor".
  - If the file is missing, it first creates the directory and
    `examples/dbmanager/starter.toml`'s text, with `create_new`: never
    overwriting, and inert, so creating it changes no behaviour.
  - It then opens the file with the opener plugin from Rust. That needs no
    webview capability.
- **Removed:** choosing a config, opening without a config, config files in
  the recent list or in drops, and a config that names its database.

### 2.11 Window-close prompt *(the human's answer stands; the mechanism below is historical: the GPUI shell's close guard, gpui-migration M3b and M12b)*

- **Where it is intercepted.** The **frontend** intercepts the close through
  `getCurrentWindow().onCloseRequested`. Only the UI knows about unsaved
  edits.
- **Why not the backend.** Intercepting in Rust with `on_window_event` was
  rejected. It needs a new builder hook outside the tool registry, a
  dirty-state round trip, and app-wide behaviour inside one tool's backend.
- **Wrappers.** `lib/tauri.ts` gains:
  - `guardWindowClose(shouldKeepOpen)`, which prevents the close only when
    the callback returns true, and closes the window if the callback throws;
  - `closeWindow()`, which calls `destroy`.
- **Capability.** Closing after the guard needs `destroy` permission, so M14
  adds `core:window:allow-destroy` to `capabilities/default.json` and to its
  pinning test. M14 verifies this by hand against the installed Tauri
  versions. The reviewer must approve the change explicitly. The CSP does not
  change.
- **Where the guard is registered** (the app shell or the tool view) and the
  prompt itself are the ui-designer's.

## 3. Cross-cutting rules (every task plan restates the ones that bind it)

**Build and lint**

- fmt and clippy pedantic with `-D warnings` must pass. Silence a lint only
  with a scoped `#[expect]`.

**Crate boundaries**

- `fm-dbmanager` and `fm-core` have no tauri dependency. Tools never import
  each other; shared code goes in `fm-core`.

**Boundary types**

- They are defined once, generated, use no `bigint`, and take no name
  already used in `lib/generated/`.

**Tests**

- Tests use temp dirs. **Tests never touch the real config location or real
  user files.**

**SQL**

- SQL safety follows §2.2.

**Destructive operations**

- A delete is confirmed together with its impact.
- Removing a link is staged in the form, and Save commits it.
- A backup precedes the first write.
- A failure rolls everything back and is reported.
- Closing the window with unsaved edits asks first.

**Security settings**

- Capability and CSP changes are reviewed decisions. M14 is the only
  capability change.

**Paths**

- Paths are `PathBuf`; display strings are used only for display.

**Accessibility**

- The backend supplies every user-facing string the UI needs.

## 4. Static linking of SQLite *(decided)*

`rusqlite` is built with `bundled` and `+crt-static` (`/MT`). The
`dumpbin` check forbids `sqlite3*.dll` (from M1). That check becomes
meaningful once the shell links the crate, in M10.

## 5. Milestones

| # | Task plan | Goal | Depends on |
| --- | --- | --- | --- |
| M1 | [`M1-foundation.md`](../tasks/dbmanager/M1-foundation.md) | Crate, bundled SQLite, `Database::open`, recent **databases**, `fm_core::system_config_dir`, CI and `CLAUDE.md` | none |
| M2 | [`M2-introspection.md`](../tasks/dbmanager/M2-introspection.md) | Schema introspection and `Ident` quoting | M1 |
| M3 | [`M3-config-format.md`](../tasks/dbmanager/M3-config-format.md) | Config parsing and validation; the doc, including the fixed location and `pickable`; examples and `starter.toml` | M1 |
| M4 | [`M4-values-rules.md`](../tasks/dbmanager/M4-values-rules.md) | `DbValue`, kinds incl. duration, `FieldRule`, `validate`, shared vectors | M1, M3 |
| M5 | [`M5-model-overlay.md`](../tasks/dbmanager/M5-model-overlay.md) | Overlay: modes incl. `pickable`, defaults, kinds, rules, labels, links, m2m with link fields, diagnostics | M2, M3, M4 |
| M6 | [`M6-session-reads.md`](../tasks/dbmanager/M6-session-reads.md) | `LoadedConfig` (status, hash), `Session` with `set_config`, reads (incl. pickable) | M5 |
| M7 | [`M7-save.md`](../tasks/dbmanager/M7-save.md) | Backup, object save with `configHash`, conflicts | M6 |
| M8 | [`M8-links.md`](../tasks/dbmanager/M8-links.md) | Link add, update and remove with link fields | M7 |
| M9 | [`M9-delete.md`](../tasks/dbmanager/M9-delete.md) | Delete impact and confirmed delete with `configHash` | M8 |
| M10 | ~~[`M10-tauri-contract.md`](../tasks/dbmanager/M10-tauri-contract.md)~~ **superseded** by gpui-migration M12a–M12c | — | — |
| M11 | ~~[`M11-validation-mirror.md`](../tasks/dbmanager/M11-validation-mirror.md)~~ **dropped** (only Rust validates) | — | — |
| M12 | ~~[`M12-docs-release.md`](../tasks/dbmanager/M12-docs-release.md)~~ **superseded** by gpui-migration M13 | — | — |
| M13 | ~~[`M13-config-watch-and-edit.md`](../tasks/dbmanager/M13-config-watch-and-edit.md)~~ **superseded** by gpui-migration M2b (watcher move) and M12d | — | — |
| M14 | ~~[`M14-window-close-guard.md`](../tasks/dbmanager/M14-window-close-guard.md)~~ **dropped** (GPUI close guard: gpui-migration M3b, M12b) | — | — |
| M15 | [`M15-pattern-syntax.md`](../tasks/dbmanager/M15-pattern-syntax.md) | **New 2026-11-21:** the full Rust `regex` syntax in patterns; the common-subset scanner and the `.` warning go (only Rust validates) | M5 |

**Execution order (revised 2026-11-21 for the GPUI migration):**

1. M1–M6: done.
2. M7 → M8 → M9 (domain), any time. M15 any time.
3. gpui-migration M12a → M12b → M12c → M12d (after M9 and the migration's
   M7b), each followed by the dbmanager U tasks that consume it.
4. gpui-migration M13 (docs, manual, Windows check) last.

The original order, interleaving and M10–M14 are historical.

## 6. Risks

- **rusqlite MSRV:** pick a version with MSRV ≤ 1.90, and commit
  `Cargo.lock`.
- **Pattern dialects** can diverge between Rust and JS. The save result is
  authoritative.
- **Large tables:** paging, `COUNT(*)` and `LIKE` are linear.
- **Delete impact cannot see what triggers do.** It flags their presence.
- **Backups** of large databases take time and disk.
- **`foreign_keys = ON`** may expose FK violations in existing data.
- **Editable by default:** mitigated by the explicit Save, the first-write
  backup, delete being off by default, and the `[defaults]` override.
- **Strict link conflicts** cost the user a round trip, and protect link
  data.
- **The config in `%ProgramData%`** is shared by every user of the machine.
  - Administrators deploy it, and the app only reads it.
  - Edit config, in developer mode, may fail with a permission error if
    another user's account created the file. The error says so.
- **Watcher reliability** varies, for example on redirected folders. The
  fallbacks are `configChanged` on `session()` and the write-path reload.
- **Moving the watcher into `fm-core`** touches the Tar Packager. Its tests
  must stay green.
- **Tauri API drift** could change the close-guard permission. M14's manual
  check fixes the exact permission.

## 7. Handoff to ui-designer

```
HANDOFF: ui-designer — Database Manager (dbmanager)

Request
  Design the UI for a new FileManager tool, the Database Manager. The user
  opens a SQLite file. ONE app-wide TOML config (fixed location
  %ProgramData%\FileManager\dbmanager.toml; the user never chooses it; the
  app only reads it and reloads it automatically) marks tables (editable /
  read-only / hidden / pickable), fields (editable / read-only / hidden /
  create-only) and many-to-many relations, and sets field bounds. Anything
  it does not mention is EDITABLE (except columns that can never be edited).
  It must feel like an OBJECT EDITOR, not a grid: pick a table -> a list of
  its objects by label (search, paging) -> an object form with typed fields.
  Write docs/plans/project/dbmanager-ui.md and docs/plans/tasks/dbmanager/U<n>-*.md.
  Also update PRODUCT.md (the tool list and users) and DESIGN.md if shared
  components or tokens change.

Backend milestones the UI depends on
  - M10 (docs/plans/tasks/dbmanager/M10-tauri-contract.md): lib/dbmanager.ts,
    DbmanagerSession (with config: {path, status, hash}), DbmanagerError /
    DbmanagerErrorKind and every view type in lib/generated/. No U task
    starts before M10 lands.
  - M13: onConfigChanged / CONFIG_CHANGED_EVENT (automatic reload) and
    editConfig (developer mode). Needed by the task showing config status,
    re-checking drafts on reload, and the dev-mode Edit config action.
  - M14: guardWindowClose / closeWindow in lib/tauri.ts. Needed by the task
    that prompts on window close with unsaved edits.
  - M11 (lib/dbmanagerValidation.ts): inline validation in the object form
    and in link fields, including durations.
  - M12 runs after the last U task (manual chapter, screenshots via
    docs/manual/screenshots/mock.js, Windows check). Give M12 the list of
    screenshot scenarios you want.

Contract facts already fixed (do not redesign them; ask the planner to change them)
  - The UI renders backend snapshots and keeps no second copy of the truth.
    Labels, display names, headings, help text, kinds, modes, bounds and
    duration units all come from TableView / FieldView / M2mView /
    ObjectView / M2mPage. The UI does no label, kind, unit or SQL logic.
  - ObjectKey, relation ids, versions, link versions and the config hash are
    opaque: echo them back verbatim.
  - Integers, reals and durations arrive and leave as strings (DbValue).
    Durations use {"type":"duration","value":"1h30m"}: canonical text on
    read; on input the user's text ("1h30m", "1h 30m", or a bare number of
    the field's unit). No h:mm:ss.
  - Hidden tables and fields never reach the UI. A FK into a hidden table
    arrives as a plain value with link.navigable = false.
  - Pickable tables arrive in session.tables with mode "pickable": leave them
    out of the table list; their objects appear in FK pickers, as navigable
    links (the object opens READ-ONLY: all fields read-only, no create /
    delete) and as the other side of checklists.
  - Forced read-only fields (rowid alias, generated, BLOB, composite FK,
    untyped) arrive with mode "read-only"; primary keys and create-only
    fields are editable only in the create form (create-only link fields only
    when the link is added).
  - Field values may carry a `problem` (wrongType, notBool, outOfBounds,
    danglingReference) for legacy data: show it, do not block other edits.
  - Saving validates only changed fields. Every save and delete carries the
    session's config.hash; ConfigChangedOnDisk means the backend already
    reloaded: re-fetch the session, re-check the draft, let the user save
    again.
  - Creating is offered when TableView.canCreate (on by default); deleting
    only when TableView.canDelete (off by default, opted into per table).
  - DbmanagerSession.configFailClosed carries the fail-closed reason, or
    null. TableView.objectName is the optional singular noun. M2mQuery.only
    fetches one other-side object. dbmanager_session() is cheap after the
    first call and recomputes configChanged. The lib/dbmanager.ts names are
    pinned in M10 and M13.

Constraints and features the design must cover
  - Object editor feel; the table list shows display names and whether each
    table is editable (pickable tables are not listed); read-only tables and
    fields are visibly read-only (not merely disabled-looking) and still
    readable and focusable. Because everything is editable by default, make
    the editable state calm and the read-only state clearly marked.
  - FK fields render as links to the referenced object (label). Following a
    link navigates to that object (a pickable one opens read-only); there is
    BACK navigation (history), by mouse and keyboard. A dangling reference is
    shown as such.
  - Editing a FK field picks from the referenced table's objects (searchable,
    paged picker; dbmanager_list on the target table); nullable FKs can be
    cleared.
  - Reverse relations ("referenced by"): each with heading and count, listing
    the referencing objects (paged), each a link.
  - Many-to-many: on an object, a checklist of EVERY object of the other
    table (search + paging; optional "linked only" filter), linked ones
    ticked. Ticks are staged in the form and committed with Save; unsaved
    changes are visible. Read-only relations show the checklist read-only.
    Available on the create form too.
  - Link fields (junction extra columns, M2mView.linkFields): ticking an
    object adds a link whose fields the user can set before saving (required
    ones must be filled; defaults prefilled); a linked entry shows its link
    fields and lets the user edit them (per field mode; create-only link
    fields are settable only when adding); unticking a link that has link
    fields discards those values: make that visible before Save (and
    reversible until Save). Save errors may target a specific link
    (linkErrors); a Conflict may name a link (conflictLink): reload that
    checklist and keep the user's other edits.
  - Booleans render as checkboxes (nullable bools can be "not set"); a
    notBool problem value must be visible.
  - Durations: text input ("1h30m" / "1h 30m" / bare number of the unit),
    unit and bounds as hints, canonical display, own copy for notDuration and
    durationPrecision.
  - Typed inputs per kind: integer, real, text (single line or multiline),
    enum (allowed values with labels), bool, duration, FK.
  - Inline validation from lib/dbmanagerValidation.ts; the save result is
    authoritative (per-field and per-link errors).
  - Save is explicit and transactional; dirty state is visible; leaving a
    dirty form (navigate, follow a link, switch table, close the database)
    asks first; CLOSING THE WINDOW with unsaved edits asks too
    (guardWindowClose / closeWindow).
  - Conflict shows the current object and lets the user re-apply or discard.
  - Create: "New <objectName or table>" only when TableView.canCreate;
    defaults prefilled; after save, the new object opens.
  - Delete (only when canDelete) is DESTRUCTIVE: a confirmation showing the
    DeleteImpact (per table and FK, action, counts, "at least" when
    truncated, sample labels, hidden tables as counts only, link rows and
    lost link data, triggers warning, blocked deletes not confirmable).
    ImpactChanged re-shows the new impact. The Delete key opens it.
  - Recovery is visible: the backup path after the first write; every write
    failure says nothing was changed.
  - The config is NOT chosen in the UI: show its status from session.config
    (loaded: path, with diagnostics and line/col; missing: a calm "no config:
    everything is editable unless stated" notice, no prompt; failClosed:
    EVERYTHING read-only with configFailClosed as the reason). It reloads by
    itself (onConfigChanged): keep drafts, re-check them, tell the user what
    changed for them. In developer mode (useDevMode()), offer "Edit config"
    (editConfig): it may report that a starter file was created.
  - Session states: no database (empty state with recent DATABASES and a
    drop hint for ONE database file), opening, open (read-only database
    banner with reason), SchemaChanged prompts to reload, Busy.
  - Loading / empty / error / success states for every list, checklist and
    form.
  - Accessibility is a requirement: keyboard reachable and labelled controls
    (field labels from displayName, help as description, errors associated
    with their field, link-field controls labelled with both the linked
    object and the field), focus management on navigation, back, save,
    error, and dialog open/close; checklists, link-field editors and pickers
    fully keyboard operable; no information by colour alone; jsx-a11y lint
    passes.
  - CSP unchanged. Capabilities: only the Open dialog, drag-drop, events,
    and (M14) destroying the window after an intercepted close.
```

### 7.1 UI impact of the final answers (for the ui-designer)

These change things the committed `dbmanager-ui.md` and U plans rely on.

**Removed from the contract** (U2 especially):

- `setConfig`, and the Config menu items "Choose config…" and "Open without
  config";
- the config file slot (`session.config` as a `DbFileSlot`) and
  `ConfigUnreadable`, which is replaced by `OpenerFailed` (copy needed);
- `RecentPairView`: `recent` is now `string[]` of database paths;
- `.toml` in drops and in the drop hint;
- the `DropAmbiguous` copy "one of each".

**Changed signatures:**

- `open(database)` takes no config.
- `deleteObject(table, key, version, impactToken, configHash)`.
- `SaveRequest` gains `configHash`.

**New in `session`:** `config: { path, status, hash }`. `configFailClosed`
now also covers "location unknown". Diagnostics are present with no database
open.

**Config reload is automatic** (M13, `onConfigChanged`):

- The "Config changed — Reload" banner and its "saving is paused" copy no
  longer fit.
- Replace them with a notice about the automatic reload and a re-check of
  the drafts.
- `ConfigChangedOnDisk` copy changes meaning to "the rules changed; check
  and save again".
- `configChanged` remains only as a fallback signal.

**New dev-mode action:** "Edit config" calls `editConfig`, with an outcome
message when a starter file was created.

**New "no config" notice** for `config.status === "missing"`.

**Pickable tables:** they are not listed, a link opens a read-only object,
and the navigator must handle landing on an object whose table is not in the
list.

**Window close:** a prompt through `guardWindowClose` / `closeWindow` (M14).
The leave guard now covers the window as well.

**Confirmed UI-only items** (no backend change): the Delete key opens the
confirmation; three panes with history; arrow keys move and Enter opens.

**Screenshots** (M12): add `dbmanager-no-config`; drop any "choose config"
shot.

## 8. Questions

### 8.1 Answered

First round (2026-11-16):

- **All matching objects:** every object of the other table, with search
  and paging, and no filter.
- **Defaults:** anything not mentioned is editable. Forced read-only still
  applies.
- **Dates:** there are none. Durations were added.
- **Junction extra columns:** editable together with their fields in v1.

Final round (2026-11-16):

1. **Create/delete defaults:** confirmed. Create is on, delete is off, and
   relations are editable.
2. **Backups:** confirmed. Beside the database, one per session, never
   deleted automatically, and a failed backup blocks the write.
3. **Durations:** confirmed. An INTEGER count of a declared unit; `1h30m`
   style input and display; no `h:mm:ss`.
4. **Self-referential many-to-many:** not in v1.
5. **Lookup tables:** **changed**. The new `pickable` table mode applies to
   M3, M5, M6 and M10 (§2.4).
6. **Views:** ignored in v1.
7. **Composite FKs** are not editable, and **BLOBs** show "N bytes" and are
   never editable (this also answers the original Q8). Confirmed.
8. **Config reload:** **changed** to an automatic file watcher, with writes
   carrying the config hash. This affects M6, M7, M9, M10 and the new M13.
9. **Startup:** confirmed. The last database reopens, and if it is missing
   the empty state shows.
10. **Config location:** **changed**. One fixed, read-only app-wide config
    in `%ProgramData%`, with no config selection, recent databases only, and
    a developer-mode Edit config with a starter file. This affects M1, M3,
    M6, M10, M13 and M12. The "`database = …`" idea is dropped.
11. **UI: closing the window with unsaved edits prompts.** That needs a
    capability change (M14, reviewed).
12. **UI: the Delete key and the three-pane navigation** are UI-only.

### 8.2 Open

None.

### 8.3 Planner decisions from the final round

These decisions were not asked of the human. Each is recorded with its
reason.

- **`reload()` keeps the backup** (asked by the ui-designer). A reload
  re-reads the same file. A session is "this database, opened, until
  closed", so a second backup would only add clutter. `close`/`open` start
  a new session. Pinned in M7 (`adopt_backup`) and M10.
- **The config-changed event fires only after the reload is complete**, and
  is emitted without holding the core lock. A `session()` call from the
  handler sees the new hash, diagnostics and tables. Pinned in M13.
- **A dropped `.toml` gets its own kind, `ConfigNotDroppable`** (taken; it
  was cheap). The UI copy can say that the config is fixed and show
  `session.config.path`. The list now has 22 kinds.

- **A pickable link opens the object read-only.** The user sees what they
  picked, and the table cannot be edited through a link.
- **Fail closed on an unknown config location.** It is never treated as
  editable defaults.
- **Writes carry `configHash`.** The backend reloads before it refuses, so a
  retry with the new hash works.
- **The starter file** is created only by developer mode, never overwrites,
  and is inert.
- **The watcher moves to `fm-core`.** The new milestones are numbered M13 and
  M14 so the UI plans' references to M10–M12 stay valid.
- **The window close is intercepted in the frontend**, with the narrowest
  capability.

### 8.4 UI contract requests (applied 2026-11-16)

The ui-designer's plan (`dbmanager-ui.md`, U1–U12) requested five changes.
All five are applied:

1. `DbmanagerSession.configFailClosed`, required by U2. It is pinned in
   M10's snapshot tests.
2. The client export names. They are pinned in M10, and the final round
   changed them as described in §7.1.
3. `dbmanager_session()` is cheap and side-effect free after the first call,
   and recomputes `configChanged`.
4. *(optional, taken)* An `object_name` table key that becomes
   `TableView.objectName`.
5. *(optional, taken)* `M2mQuery.only`.
