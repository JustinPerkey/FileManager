# Project plan: the Database Manager tool

Status: **awaiting approval.** First written 2026-11-13 by the planner.
Revised 2026-11-16 after the human answered four open questions (§8.1).
Those answers changed the defaults (editable, not read-only), removed dates,
added a `duration` field kind, and made junctions with extra columns editable
together with their fields. Link editing became its own milestone (M8), so
the later milestones were renumbered. The same day, the ui-designer's
contract requests (§8.3) were applied to M3, M5, M6 and M10. Decisions marked
*(proposed)* stand unless the human overrides them in answer to §8.2.

The UI is not designed here. §7 hands it to the `ui-designer`, who writes
[`dbmanager-ui.md`](dbmanager-ui.md) and the `U<n>` task plans.

Task plans: [`../tasks/dbmanager/`](../tasks/dbmanager/).

## 1. Request

From the user, with their answers to the clarifying questions:

- Add a **Database Manager** tool (`dbmanager`) to FileManager.
- The engine is a **SQLite file** the user picks. The app stays a portable
  Windows exe with no server, so SQLite is bundled and statically linked.
- A **TOML config file** sets the bounds of each field. Only some fields are
  configurable.
- The TOML classifies **tables and fields by mode**: at least *editable*,
  *read-only* and *hidden*. Editable fields carry bounds. Table structure,
  column types and foreign keys come from introspecting the SQLite file; the
  TOML is an overlay on it. **Anything the TOML does not mention is
  editable** (answer to Q1), apart from the columns that can never be edited.
- **Foreign keys are links.** Following one opens the referenced object, with
  back navigation. Editing a FK field picks from the referenced table's
  objects. Objects that reference this one are shown too.
- **Many-to-many** relations (junction tables, detected from FKs,
  confirmable in the TOML) show, on an object, a checklist of **every object
  of the other table**, with search and paging and the linked ones ticked
  (answer to Q3). Ticking adds a junction row, unticking removes it.
  **Junction extra columns are editable in v1** (answer to Q4): adding a link
  sets them, an existing link's fields can be edited, and a link can be
  removed.
- It feels like an **object editor**, not a grid: pick a table, see its
  objects by label, open an object as a form with typed fields.
- **Booleans render as checkboxes.**
- **No dates. Durations may occur** (answer to Q8).
- v1 writes: **edit, create and delete objects, and edit many-to-many links
  with their fields.** Delete is confirmed and shows its FK impact. Saves are
  explicit and transactional. Validation runs in Rust and is mirrored in the
  UI for inline feedback. Concurrent modification of the file and a recovery
  story are handled.

## 2. Architecture

### 2.1 Crate and module layout *(proposed)*

```
crates/fm-dbmanager/              domain library; no tauri dependency
  Cargo.toml                      rusqlite (bundled, backup), toml, regex, sha2,
                                  serde, ts-rs, tempfile, fm-core; [lints] workspace = true
  src/lib.rs
  src/db.rs                       Database: open (never create), pragmas,
                                  read-only detection, schema_version
  src/state.rs                    RememberedState (recent database/config pairs),
                                  persisted as dbmanager/state in fm-core's Store
  src/schema/                     introspection: Schema, Table, Column,
                                  ForeignKey, UniqueKey, Affinity, Identity;
                                  ident.rs (Ident: the only way to put a name in SQL)
  src/config/                     TOML parse + structural validation with line/col
                                  diagnostics: Config, TableConfig, FieldConfig, ...
  src/value.rs                    DbValue (boundary), rusqlite conversions
  src/rules/                      FieldKind, FieldRule, duration.rs (parse/format),
                                  validate(rule, input) -> normalised value | issues
  src/model/                      overlay: resolve(schema, config) -> Model +
                                  Diagnostics; kinds, labels, m2m + link fields, views
  src/session/                    Session: one open database + model
    read.rs                       list, object, m2m options (+ link values), versions
    save.rs                       object update / create, conflict check
    links.rs                      link add / update / remove with link fields
    backup.rs                     once-per-session online backup
    delete.rs                     impact preview + confirmed delete
    sql.rs                        the only module that builds SQL text
examples/dbmanager/
  schema.sql                      example schema + seed rows (tests include_str! it)
  example.toml                    example config; resolves with zero diagnostics
  validation-cases.json           shared validation vectors: Rust and TS both run them
docs/dbmanager-config.md          the config reference (M3); later plans point here
docs/dbmanager.md                 the tool's flow, contract and recovery story (M12)
apps/desktop/src-tauri/src/tools/dbmanager/
  mod.rs                          dbmanager_* commands; work runs on a blocking worker
  core.rs                         session slots, recent store, error mapping, snapshot
  types.rs                        DbmanagerSession, DbmanagerError(+Kind)
  tests.rs
apps/desktop/src/lib/dbmanager.ts             typed client (one fn per command) + helpers
apps/desktop/src/lib/dbmanagerValidation.ts   TS mirror of rules::validate
apps/desktop/src/tools/dbmanager/             views (ui-implementer; not in this plan)
```

Reasons:

- The **domain crate holds all database logic**, including the `Session` that
  runs queries and writes. Every rule is then testable with `cargo test -p
  fm-dbmanager` in a temp dir, with no Tauri runtime. The shell module only
  holds the two file slots, the recent list, error mapping and threading.
  This is the tarpack split: a domain crate plus a thin shell `core.rs`.
- **Boundary types that describe data** (`DbValue`, `FieldRule`,
  `TableView`, `ObjectView`, `SaveRequest`, `DeleteImpact`, ...) derive
  `ts_rs::TS` in the crate. **The session snapshot and the error** live in
  the shell's `types.rs`.
- Each milestone that adds crate boundary types also appends them to
  `EXPORTERS` and regenerates `lib/generated/`. The TS mirror (M11) can then
  start as soon as the rule types exist (M4).

### 2.2 SQL safety *(proposed; binding on every milestone that touches SQL)*

- **Values are always bound parameters.** No value is ever formatted into SQL.
- **Identifiers come only from introspection.** `schema::Ident` is a newtype
  whose constructor is `pub(crate)` and is called only by the introspection
  code. `Ident::quoted()` wraps the name in `"` and doubles every embedded
  `"`. `session/sql.rs` is the only module that assembles SQL text, and it
  accepts names only as `&Ident`, never as `&str`. Config names are looked up
  in the introspected schema, never passed through.
- **Introspection interpolates nothing.** It uses the table-valued pragma
  functions with bound parameters.
- Never `INSERT OR IGNORE` / `OR REPLACE`. They hide constraint failures or
  delete rows.
- Tests run every read and write against names with embedded quotes, spaces,
  SQL keywords and non-ASCII text.
- The config never contains SQL.

### 2.3 Opening a database *(proposed)*

- The path must be absolute and name an existing file. It is opened
  read-write, **never created**, and never interpreted as a URI. If the file
  or its folder is read-only, the database opens read-only, the session
  carries the reason, and every write is refused with `DatabaseReadOnly`.
- After open: `foreign_keys = ON`, `busy_timeout` 3000 ms,
  `trusted_schema = OFF`, and `SQLITE_DBCONFIG_DEFENSIVE`. The journal mode
  is never changed.
- A file that is not a SQLite database fails with `DatabaseUnreadable`. So
  does a path that is not valid Unicode, because SQLite on Windows needs a
  UTF-8 path.

### 2.4 The model: introspection plus overlay *(proposed)*

The full rules are in M3's task plan. Once M3 lands they live in
`docs/dbmanager-config.md`. The decisions:

- **Object tables:** ordinary tables only, excluding `sqlite_*`. Views,
  virtual tables and shadow tables are ignored, and naming one in the config
  is a warning.
- **Object identity (`ObjectKey`)** is the rowid for rowid tables and the
  primary-key values for `WITHOUT ROWID` tables. It is opaque to the UI.
- **Modes:** tables `editable | read-only | hidden`; fields `editable |
  read-only | hidden | create-only`; many-to-many relations `editable |
  read-only | hidden`.
- **Defaults (changed by the human's answer to Q1): anything the TOML does
  not mention is editable.** `[defaults]` has `table_mode`, `field_mode` and
  `relation_mode`, all `editable` by default, so a stricter policy is one
  line (`table_mode = "read-only"`).
- **`allow_create` defaults to `true`, `allow_delete` to `false`**
  *(proposed; kept open as Q1 in §8.2)*. The reasoning:
  - Creating is additive and easy to undo (delete the new object), so it
    follows "editable by default".
  - Deleting destroys data and cascades through foreign keys, possibly into
    tables the user cannot see. Even with the mandatory impact confirmation, a
    default that offers Delete on every table of an unconfigured database
    makes a destructive operation available without anyone having decided it
    should be. Opting in per table is one line.
- **Forced read-only, whatever the config says:**
  - the rowid alias;
  - generated columns;
  - `BLOB` columns;
  - untyped columns without a `type`;
  - multi-column FK fields;
  - FK fields into a hidden table.

  A primary-key column of an existing object is never editable. An
  inherited `editable` on one becomes `create-only`, so it can be set when
  creating. An explicit `editable` on a forced column is an error, while an
  inherited one is silently narrowed.
- **Fail closed.** A config entry with an error is rejected, and its subject
  becomes **read-only**, not the editable default (or hidden, if the entry
  validly asked for hidden). An error never makes anything more editable
  than its entry would have.
- **Hidden** means never shown. Hidden columns never leave Rust. A hidden
  table's objects are never listed, labelled or linked, and a FK into one
  shows the raw value and is not navigable. The one exception is delete
  impact (§2.6), which counts rows in hidden tables without showing them.
- **Field kinds:** `integer | real | text | bool | duration | blob | any`.
  They are inferred from the declared type by SQLite's affinity rules. A
  declared type `BOOLEAN`/`BOOL` gives `bool`, and the date/time special
  case is dropped because there are no dates. The config's `type` can
  override the kind within an affinity compatibility table.
- **Booleans** are stored as INTEGER `0`/`1`. `NULL` means "not set"
  (nullable fields only), and any other stored value is shown as a problem.
- **Durations** (new, §2.5): `type = "duration"` with a `unit`.
- **Bounds:**
  - integer, real and duration: `min`, `max`;
  - text: `min_length`, `max_length`, `pattern`, and `empty_is_null`
    (default true on nullable text);
  - integer and text: `allowed`, an enum with optional labels;
  - every kind: `nullable`, which may tighten the column's `NOT NULL` but
    never loosen it.

  Display-only keys: `display_name`, `help`, `multiline`.
- **Create availability:** creating needs every `NOT NULL` column without a
  default to be settable. If one is not, create is disabled: an **error**
  when `allow_create` was set explicitly, a **warning** when it was
  inherited (so the user can see why "New" is missing).
- **Mismatches are diagnostics.**
  - A TOML syntax error, an unsupported `version`, or an unreadable config
    file ignores the whole config, and **everything is read-only** until it
    is fixed. Falling back to the editable built-in defaults would let a
    typo expose and unlock tables the user meant to hide. An invalid
    `[defaults]` makes the defaults read-only. No config at all means the
    editable built-in defaults.
  - An invalid entry is rejected, and its subject is read-only.
  - An unknown table or column is a warning.
  - Each of these is an error, and the field becomes read-only: a bound that
    does not fit the kind, a bound or `type` on a FK field, `min > max`,
    `nullable = true` on a `NOT NULL` column, and an explicit `editable` on a
    forced-read-only column.
  - The database still opens and can be browsed.
- **Labels:** `label = "{last_name}, {first_name}"`, or by default the first
  visible text column named `name`/`title`/`label`, else `"<display name>
  #<key>"`. Labels are computed in Rust. Search matches the label's text and
  integer columns with `LIKE`.
- **Object noun:** an optional table key `object_name = "author"` reaches
  `TableView.objectName`, so copy can say "New author". It is never guessed
  (no singularising of display names); without it the UI uses neutral copy.
- **Many-to-many:**
  - **Auto-detection** needs a table with exactly two single-column FKs to
    two *different* tables, where the pair is the primary key or a unique
    index and the table has no other column except an optional rowid alias.
  - **Declaring** `[many_to_many.<junction>]` confirms a relation. It also
    sets the mode and headings, picks two FK columns of a wider junction, or
    disables the relation (`enabled = false`). Only a declared junction may
    have extra columns, because a table such as `loan(book_id, member_id,
    …)` is an entity, not a junction.
  - **Link fields** (new, Q4): a junction's extra columns are its **link
    fields**. They are configured as `[tables.<junction>.fields.<column>]`
    with the same modes, kinds and bounds as any field. `create-only` means
    "set when the link is added".
  - A junction used as many-to-many is left out of the table list unless its
    `[tables.<junction>]` entry gives a `mode`. For a junction, `mode` only
    controls that listing. The links are governed by the relation's mode.
  - **Self-referential** junctions are not many-to-many in v1.
  - The checklist shows on a side only if the other side is not hidden. It
    is editable there only if the relation is `editable` and that side's
    table is `editable`.
  - If a junction `NOT NULL` column without a default cannot be supplied
    (hidden, read-only, or forced), links cannot be added. The relation is
    then read-only: an error if its mode was explicit, a warning if
    inherited.

### 2.5 Durations *(proposed; storage format kept open as Q3 in §8.2)*

The human ruled out dates and said durations may occur. Decisions:

- **Storage:** an INTEGER count of a unit the config declares: `unit =
  "milliseconds" | "seconds" | "minutes" | "hours" | "days"`, default
  `seconds`. A day is exactly 24 hours, since durations are not calendar
  arithmetic.
  - Why: integers sort and compare in SQL, bounds are exact, nothing needs
    parsing on the database side, and the unit is explicit in the config.
    Text storage (`"01:30:00"`, ISO 8601 `PT1H30M`) and REAL seconds were
    rejected for v1. Text needs a parser in every reader and cannot be
    compared in SQL. REAL brings rounding.
  - If the human's existing data uses another format, this is the question
    to answer (§8.2 Q3).
- **Declaring:** `type = "duration"` (allowed on INTEGER/NUMERIC affinity,
  untyped columns, and STRICT `INT`/`INTEGER`/`ANY`), with `unit`. It is
  never inferred from a declared type. `min`/`max` are human durations
  (`"30s"`, `"1h30m"`) or integers counted in the unit. A bound that is not a
  whole number of units is an error. `allowed` does not apply.
- **Human format:**
  - **Grammar:** an optional `-`, then either a bare integer (a count of the
    field's unit) or one or more components `<digits><suffix>`, with
    suffixes in strictly decreasing order from `d`, `h`, `m`, `s`, `ms`,
    optionally separated by single spaces (`"1h30m"`, `"1h 30m"`, `"90"`).
  - **Canonical form:** the sign, then the non-zero components from `d` down
    to the field's unit, with no spaces (`5400` seconds → `"1h30m"`), and
    `"0<unit suffix>"` for zero.
  - Labels show the canonical form.
- **Boundary:** a new `DbValue` variant `{"type":"duration","value":"1h30m"}`
  carries the canonical text. Reads of a duration field holding an integer
  produce it; anything else arrives as stored, with a `wrongType` problem.
  Inputs send the user's text in the same variant.
- **Validation** adds two issue codes:
  - `notDuration`: bad grammar, or the count overflows i64;
  - `durationPrecision`: not a whole number of the unit; `limit` is the unit
    name.

  Bounds compare counts. The normalised value is the canonical text, and it
  is stored as the integer count. The shared vectors cover durations, and
  the TS mirror implements them with `BigInt`.
- Search skips duration columns: matching text against a stored count
  would mislead.

### 2.6 Validation, mirrored *(proposed)*

- `rules::validate(&FieldRule, &DbValue) -> Result<DbValue, Vec<FieldIssue>>`
  is pure and authoritative. It returns the normalised value to store.
- Integers, reals and durations cross the boundary as **strings**. i64 does
  not fit a JS number, generated types may not use `bigint`, and inputs are
  strings anyway.
- `lib/dbmanagerValidation.ts` mirrors the pure rules. **Parity is enforced
  by data:** `examples/dbmanager/validation-cases.json` runs in Rust and in
  vitest. The rule data (`FieldRule`) is generated, so it is defined once.
  The same validation applies to link fields.
- `pattern`: Rust `regex` is authoritative, and JS `RegExp` with the `u` flag
  is used while typing, both anchored. The doc says to stay in the common
  subset.
- Not mirrored, because they depend on the database: `unique`,
  `referenceMissing`, `notEditable`, `unknownField`.
- **Saving validates only changed fields.** Legacy values that break a rule
  are flagged in the view, but they do not block saving other fields.

### 2.7 Writes, concurrency and recovery *(proposed)*

- **One write = one transaction** (`BEGIN IMMEDIATE`). A save carries the
  object's field changes and its link changes (adds with link fields,
  link-field updates, removals). A delete removes one object, and SQLite
  applies the cascades. On any failure everything rolls back and the error
  says nothing was changed. There are no multi-transaction batches.
- **Optimistic concurrency.** Every object read returns a `version`
  (SHA-256 over all its columns, hidden ones included), and every linked
  checklist entry returns a `linkVersion` (the same hash over the junction
  row). The save re-reads inside the transaction:
  - The object's version differs → `Conflict`, carrying the current object.
  - **Link edits are strict:**
    - adding a link that already exists → `Conflict`;
    - updating or removing a link that is gone → `Conflict`;
    - updating a link whose version differs → `Conflict`;
    - removing a link whose version is given and differs → `Conflict`.

    The error names the link, so the UI can reload that checklist. Strict
    links replace the earlier idempotent design, because a link can now
    carry data that a silent no-op would overwrite or lose.
- **Schema changes:** `PRAGMA schema_version` is checked on every command. A
  change fails with `SchemaChanged`.
- **Config changes:** the session reports `configChanged`, and writes are
  refused with `ConfigChangedOnDisk` until a reload.
- **Locks:** `busy_timeout` 3 s, then `Busy`.
- **Backup:** before the **first write of each opened session**, the
  database is copied with SQLite's online backup API to `<file name>.<UTC
  yyyymmddThhmmssZ>.bak` beside it. The copy is moved into place without
  replacing anything. If the backup fails, the write is refused and nothing
  is written. Backups are never deleted by the app.
- **Delete** is two steps:
  1. A preview returns the `DeleteImpact`. For every FK in any table
     (hidden ones counted only), it gives the ON DELETE action and the
     affected rows, walked through cascades and capped at 10 000. Link rows
     are flagged `links`, and `linkData` is set when their junction has
     extra columns (those values are lost). It also says whether the delete
     is blocked and whether triggers may do more, and carries a token.
  2. The delete takes the version and the token, recomputes the impact
     inside the transaction, and refuses on any difference.
- **Removing a link with link fields** loses those values. It is staged in
  the form and committed only by an explicit Save, and it is covered by the
  backup.

### 2.8 Boundary contract *(proposed; built in M10, types from M3–M9)*

All commands are `async`. Their work runs under `spawn_blocking` on an
`Arc<Mutex<Core>>`.

| Command | Returns |
| --- | --- |
| `dbmanager_session()` | `DbmanagerSession` (restores the most recent pair on the first call) |
| `dbmanager_open(database, config?)` | `DbmanagerSession` |
| `dbmanager_set_config(config?)` | `DbmanagerSession` |
| `dbmanager_open_dropped(paths)` | `DbmanagerSession` (`.toml` = config, any other file = database) |
| `dbmanager_reload()` | `DbmanagerSession` |
| `dbmanager_close()` | `DbmanagerSession` |
| `dbmanager_list(table, query)` | `ObjectPage` |
| `dbmanager_object(table, key)` | `ObjectView` |
| `dbmanager_m2m_options(table, key?, relation, query)` | `M2mPage` (every object of the other side, `linked`, and for linked ones `linkValues` + `linkVersion`; `query.only` fetches one other-side object, e.g. to refresh a conflicted link) |
| `dbmanager_save(request)` | `SaveResult` (create when `key` is null; `links` carry add / update / remove with link fields) |
| `dbmanager_delete_preview(table, key)` | `DeleteImpact` |
| `dbmanager_delete(table, key, version, impactToken)` | `DeleteResult` |

`DbmanagerErrorKind` (pinned by a test) has these kinds: `NoDatabase`,
`NotAbsolute`, `NotAFile`, `DatabaseUnreadable`, `ConfigUnreadable`,
`DropAmbiguous`, `UnknownTable`, `UnknownObject`, `UnknownRelation`,
`NotEditable`, `ValidationFailed` (with `fieldErrors` and `linkErrors`),
`Conflict` (with `current` and an optional `conflictLink`),
`SchemaChanged`, `ConfigChangedOnDisk`, `DatabaseReadOnly`, `Busy`,
`BackupFailed`, `ImpactChanged`, `DeleteBlocked`, `ConstraintFailed`, `Io`.

`DbmanagerSession.configFailClosed` is a short reason when a config syntax
error, unsupported `version` or unreadable file made everything read-only,
and `null` otherwise (including no config). The UI never infers it from
diagnostic text.

`dbmanager_session()` is cheap and side-effect free after the first call:
no reopen, no query on user tables, no writes. It recomputes `configChanged`
on every call, so the view calls it on window focus.

The `lib/dbmanager.ts` export names are pinned in M10: `session`, `open`,
`setConfig`, `openDropped`, `reload`, `close`, `list`, `object`,
`m2mOptions`, `save`, `deletePreview`, `deleteObject`, plus the helpers
`boolValue`, `durationValue`, `linkKey`, `isConfirmDeleteAllowed` and
`isDbmanagerError`.

Capabilities and the CSP do not change.

### 2.9 Persisted state *(proposed)*

`fm_dbmanager::state::RememberedState` is stored at `<app
data>/dbmanager/state.json`. It holds the recent `(database, config?)` pairs,
most recent first, unique by database path, at most 10. The first
`dbmanager_session` reopens the most recent pair. If that fails, the snapshot
says why and the pair stays in the list.

## 3. Cross-cutting rules (every task plan restates the ones that bind it)

- `cargo fmt --all --check` and clippy pedantic with `-D warnings` pass.
  Silence a lint only with a scoped `#[expect(clippy::…, reason = "…")]`.
- `fm-dbmanager` has `[lints] workspace = true` and no tauri dependency, and
  CI checks it. It never depends on another tool crate.
- Boundary types are defined once in Rust and generated. No `bigint`, and no
  name already present in `lib/generated/`.
- Filesystem tests run in temp dirs they create, and each builds its own
  database from SQL text.
- SQL safety, §2.2.
- Destructive operations:
  - delete is confirmed with its impact;
  - removing a link (with or without fields) is staged and committed by an
    explicit Save;
  - every first write is preceded by a backup;
  - every failure rolls back and is reported.
- Paths are `PathBuf`. Display strings are only for display.
- Accessibility: the backend supplies every user-facing string the UI needs
  to label controls.

## 4. Static linking of SQLite *(proposed)*

`rusqlite` with `bundled` compiles SQLite through `cc`. With `+crt-static`
already set for `x86_64-pc-windows-msvc`, it is built `/MT` and linked
statically. M1 adds `sqlite3*.dll` to the forbidden `dumpbin` list. The check
becomes meaningful once the shell links the crate (M10). `bundled` is used on
Linux too.

## 5. Milestones

Each milestone is specified in full in its task plan.

| # | Task plan | Goal | Depends on |
| --- | --- | --- | --- |
| M1 | [`M1-foundation.md`](../tasks/dbmanager/M1-foundation.md) | `fm-dbmanager` crate, bundled SQLite, `Database::open`, `RememberedState`, CI and `CLAUDE.md` wiring | none |
| M2 | [`M2-introspection.md`](../tasks/dbmanager/M2-introspection.md) | Schema introspection and `Ident` quoting | M1 |
| M3 | [`M3-config-format.md`](../tasks/dbmanager/M3-config-format.md) | TOML config parse/structure validation, `docs/dbmanager-config.md`, `examples/dbmanager/` | M1 |
| M4 | [`M4-values-rules.md`](../tasks/dbmanager/M4-values-rules.md) | `DbValue`, field kinds incl. duration, `FieldRule`, `validate`, shared vectors; first exports | M1, M3 |
| M5 | [`M5-model-overlay.md`](../tasks/dbmanager/M5-model-overlay.md) | Overlay: modes and defaults, kinds, rules, labels, FK links, m2m with link fields, diagnostics, `TableView` | M2, M3, M4 |
| M6 | [`M6-session-reads.md`](../tasks/dbmanager/M6-session-reads.md) | `Session`: freshness, lists, object view, reverse relations, m2m options with link values | M5 |
| M7 | [`M7-save.md`](../tasks/dbmanager/M7-save.md) | Backup, object save (update, create), conflicts, constraint mapping | M6 |
| M8 | [`M8-links.md`](../tasks/dbmanager/M8-links.md) | **New (split from M7):** link add / update / remove with link fields, in the save transaction | M7 |
| M9 | [`M9-delete.md`](../tasks/dbmanager/M9-delete.md) | Delete impact preview and confirmed delete (was M8) | M8 |
| M10 | [`M10-tauri-contract.md`](../tasks/dbmanager/M10-tauri-contract.md) | Tauri commands, session snapshot, errors, `lib/dbmanager.ts`: **the UI contract** (was M9) | M9 |
| M11 | [`M11-validation-mirror.md`](../tasks/dbmanager/M11-validation-mirror.md) | `lib/dbmanagerValidation.ts`, parity with the shared vectors incl. durations (was M10) | M4 |
| M12 | [`M12-docs-release.md`](../tasks/dbmanager/M12-docs-release.md) | `docs/dbmanager.md`, user manual, screenshots, Windows check: **depends on UI plan** (was M11) | M10, M11, all U tasks |

M2 and M3 can run in parallel. M11 depends only on M4, so it can run in
parallel with M5–M10.

**Interleaving with the UI pipeline:**

- No UI task starts before **M10** lands.
- Inline validation, including link fields and durations, needs **M11**,
  which should land before or with M10.
- **M12** runs after the last U task.

## 6. Risks

- **rusqlite MSRV and `--locked`:** pick a version whose `rust-version` is
  at most 1.90, and commit `Cargo.lock`.
- **Pattern dialects** can diverge. The save result is authoritative.
- **Large tables:** lists are paged (limit ≤ 500). `COUNT(*)`/`OFFSET` and
  `LIKE` are linear.
- **Delete impact** cannot see trigger effects. It flags them instead of
  hiding them.
- **Backups of large databases** take time and disk, once per session, on a
  blocking worker.
- **`foreign_keys = ON`** may make edits fail on files whose other users never
  enforced FKs. These are reported and rolled back.
- **Editable by default** means an unconfigured database can be changed as
  soon as it is opened. The mitigations are a confirmation-free but
  explicit Save, the first-write backup, `allow_delete = false` by default,
  and the one-line `[defaults]` override.
- **Strict link semantics** turn a stale checklist into a `Conflict`, which
  is an extra round trip for the user, and safer than silently overwriting
  link data.

## 7. Handoff to ui-designer

```
HANDOFF: ui-designer — Database Manager (dbmanager)

Request
  Design the UI for a new FileManager tool, the Database Manager. The user
  opens a SQLite file and, optionally, a TOML config that marks tables,
  fields and many-to-many relations editable / read-only / hidden (fields
  also create-only) and sets field bounds. Anything the config does not
  mention is EDITABLE (except columns that can never be edited). It must feel
  like an OBJECT EDITOR, not a grid: pick a table -> a list of its objects by
  label (search, paging) -> an object form with typed fields.
  Write docs/plans/project/dbmanager-ui.md and docs/plans/tasks/dbmanager/U<n>-*.md.
  Also update PRODUCT.md (the tool list and users) and DESIGN.md if shared
  components or tokens change.

Backend milestones the UI depends on
  - M10 (docs/plans/tasks/dbmanager/M10-tauri-contract.md): lib/dbmanager.ts,
    DbmanagerSession, DbmanagerError/DbmanagerErrorKind and every view type in
    lib/generated/. No U task starts before M10 lands.
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
  - ObjectKey, relation ids, versions and link versions are opaque: echo
    them back verbatim.
  - Integers, reals and durations arrive and leave as strings (DbValue).
    Durations use {"type":"duration","value":"1h30m"}: canonical text on
    read; on input the user's text ("1h30m", "1h 30m", or a bare number of
    the field's unit).
  - Hidden tables and fields never reach the UI. A FK into a hidden table
    arrives as a plain value with link.navigable = false.
  - Forced read-only fields (rowid alias, generated, BLOB, composite FK,
    untyped) arrive with mode "read-only"; primary keys and create-only
    fields are editable only in the create form (create-only link fields only
    when the link is added).
  - Field values may carry a `problem` (wrongType, notBool, outOfBounds,
    danglingReference) for legacy data: show it, do not block other edits.
  - Saving validates only changed fields.
  - Creating is offered when TableView.canCreate (on by default); deleting
    only when TableView.canDelete (off by default, opted into per table).
  - (Added after the UI plan's contract requests.) DbmanagerSession.
    configFailClosed carries the fail-closed reason, or null.
    TableView.objectName is the optional singular noun. M2mQuery.only
    fetches one other-side object. dbmanager_session() is cheap after the
    first call and recomputes configChanged. The lib/dbmanager.ts names are
    pinned in M10.

Constraints and features the design must cover
  - Object editor feel; the table list shows display names and whether each
    table is editable; read-only tables and fields are visibly read-only (not
    merely disabled-looking) and still readable and focusable. Because
    everything is editable by default, make the editable state calm and the
    read-only state clearly marked, not the reverse.
  - FK fields render as links to the referenced object (label). Following a
    link navigates to that object; there is BACK navigation (history), by
    mouse and keyboard. A dangling reference is shown as such.
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
    reversible until Save). Link fields use the same typed inputs, bounds and
    inline validation as object fields. Save errors may target a specific
    link (linkErrors with relation + other key + per-field errors); a
    Conflict may name a link (conflictLink): reload that checklist and keep
    the user's other edits.
  - Booleans render as checkboxes. A nullable bool can be null: design how
    "not set" is shown and chosen. A notBool problem value must be visible.
  - Durations: a text input accepting "1h30m" / "1h 30m" / a bare number of
    the field's unit; show the unit and bounds (as durations) as hints;
    display values in canonical form; issues notDuration and
    durationPrecision ("must be whole minutes") have their own copy.
  - Typed inputs per kind: integer, real, text (single line or multiline),
    enum (allowed values with labels), bool, duration, FK. Show bounds/help
    where useful.
  - Inline validation from lib/dbmanagerValidation.ts while editing; the save
    result is authoritative (ValidationFailed carries per-field and per-link
    errors, including unique/referenceMissing that only the backend can
    detect).
  - Save is explicit and transactional; dirty state is visible; leaving a
    dirty form (navigate, follow a link, switch table, close) asks first.
    Conflict (object changed elsewhere) shows the current object and lets the
    user re-apply or discard their edits.
  - Create: "New <table>" only when TableView.canCreate; defaults prefilled
    from FieldView.defaultValue; after save, the new object opens.
  - Delete (only when canDelete) is DESTRUCTIVE: always a confirmation that
    shows the DeleteImpact: what references the object, per table and FK, the
    ON DELETE action (cascade delete / set null / set default / blocked), row
    counts ("at least" when truncated), sample labels, hidden tables as counts
    only, link rows as links (and, when linkData is set, that their link
    field values are lost), a triggers warning, and blocked deletes as not
    confirmable with the reason. ImpactChanged re-shows the new impact.
  - Recovery is visible: the backup path from the session/save/delete result
    is shown after the first write ("A backup was saved to ..."). Every write
    failure says nothing was changed.
  - Session states: no database, opening, open (read-only database banner
    with reason), config diagnostics (errors/warnings with line/col; the db is
    still browsable), a config with a syntax error / unsupported version /
    unreadable file making EVERYTHING read-only (say so prominently, with the
    reason and a way to fix and reload), configChanged / SchemaChanged prompts
    to reload, Busy,
    recent pairs list, drag-and-drop of a .db and/or .toml.
  - Loading / empty / error / success states for every list, checklist and
    form.
  - Accessibility is a requirement: every control keyboard reachable and
    labelled (field labels from displayName, help as description, errors
    associated with their field, link-field controls labelled with both the
    linked object and the field), focus management on navigation, back, save,
    error, and dialog open/close; checklists, link-field editors and pickers
    fully keyboard operable; no information by colour alone; jsx-a11y lint
    passes.
  - CSP and capabilities do not change: only the Open dialog and drag-drop
    are available.
```

## 8. Questions

### 8.1 Answered (2026-11-16)

- **All matching objects (was Q3):** every object of the other table, with
  search and paging. No filter in the TOML. Already the design; now
  confirmed.
- **Defaults (was Q1):** changed. Anything the TOML does not mention is
  **editable**, and forced read-only still applies. Folded into §2.4, M3
  (spec and example), M5 (semantics, diagnostics, tests), and the tests of
  M6–M9. The `allow_create`/`allow_delete` defaults are the planner's
  proposal (§2.4) and stay open as Q1 below.
- **Dates (was Q8):** there are none. Durations may occur. The date/time
  inference rule is dropped and the `duration` kind is added (§2.5) in M3,
  M4, M5, M6, M11 and the handoff. The storage format stays open as Q3 below.
- **Junction extra columns (was Q4):** changed to **editable with their
  fields in v1** (§2.4, §2.7). M5 adds link fields, M6 link values, new M8
  the link writes, M9 `linkData` in the impact, M10 the payload and errors,
  and M11 applies the vectors to link fields.

### 8.2 Open

1. **Create/delete defaults:** `allow_create` defaults to `true` (creating is
   additive and undoable) and `allow_delete` to `false` (destructive and
   cascading, so opt in per table). Many-to-many relations default to
   `editable` like everything else. Confirm, or should delete also default
   to on (the impact confirmation is shown either way)?
2. **Backups:** one per opened session, beside the database, never deleted by
   the app, and a failed backup blocks the write. Is beside the database right
   (vs. `%APPDATA%\FileManager\dbmanager\backups\`), and do you want a
   retention limit?
3. **Duration storage:** proposed as an INTEGER count of a unit the config
   declares (milliseconds, seconds, minutes, hours or days; default seconds;
   a day = 24 h), entered and shown as `1h30m`. Does existing data use
   another format, such as text `01:30:00`, ISO 8601 `PT1H30M`, or REAL
   seconds? Do you want an `h:mm:ss` input or display too?
4. **Self-referential many-to-many** (for example `related_books(a, b)`) is
   not treated as m2m in v1. Needed?
5. **Lookup tables:** with editable defaults, a lookup table must be marked
   `read-only` to stay pickable but unchanged. Do you also want "pickable but
   not in the table list"?
6. **Views:** ignored in v1. Should they be browsable read-only?
7. **Composite foreign keys** are shown as links but are not editable in v1.
   OK?
8. **BLOB columns** are shown as "N bytes" and are never editable. OK?
9. **Config reload:** v1 detects config edits and asks for a reload (writes
   are refused until then). Do you want automatic reload with a file watcher?
10. **Startup:** the most recent database and config are reopened
    automatically. Confirm.
11. **Config naming its database:** should the TOML be able to name its
    database file (`database = "shop.db"`)?

### 8.3 UI contract requests (applied 2026-11-16)

The ui-designer's plan (`dbmanager-ui.md`, U1–U12) requested five changes.
All five are applied:

1. `DbmanagerSession.configFailClosed` (required by U2). The source is a new
   `ConfigReport.fail_closed` (M3) and `Session::config_fail_closed()`
   (M6). It is pinned in M10's snapshot tests.
2. The client export names are adopted exactly as the U plans assume, and
   pinned in M10.
3. `dbmanager_session()` is cheap and side-effect free after the first call,
   and recomputes `configChanged`. Stated and tested in M10.
4. *(optional, taken)* `object_name` table key (M3) → `TableView.objectName`
   (M5). It is not guessed when absent, and the example config sets it.
5. *(optional, taken)* `M2mQuery.only: ObjectKey` (M6) returns exactly one
   other-side object with its link state, so U9 can refresh one link after a
   conflict.
