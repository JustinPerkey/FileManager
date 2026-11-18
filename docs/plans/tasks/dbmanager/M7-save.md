# M7 — Saving objects: backup, update, create, conflicts

Status: awaiting approval. Revised 2026-11-16: link writes were split out
into M8, durations were added, and the defaults are now editable. Then:
writes carry the config hash (the config now reloads automatically), and
`pickable` tables are never written.
Project: dbmanager   Depends on: M6 (landed)

## Goal

Let a `Session` save one object's field changes, for an existing object or a
new one, in a single transaction after a one-time backup. Stale, invalid or
not-permitted writes are refused with precise errors. The checking and
reference-resolving steps are built so that link fields (the next milestone)
reuse them.

## Context

The Database Manager (`dbmanager`) edits a user's SQLite file. Saves are
**explicit** (the user presses Save) and **transactional**. Anything the
config does not mention is editable by default, except the forced read-only
columns.

`docs/dbmanager-config.md` holds the modes and bounds that decide what may be
written. The relevant sections are "Defaults", "Fields", "Kinds",
"Durations" and "Forced read-only".

The next milestone adds many-to-many link changes to the same save request
and the same transaction. This milestone saves only the object's own fields,
but it must leave the clear seams described under "Reuse".

Builds on:

- M1 `src/db.rs`: `Database`, `read_only_reason`, `conn_mut`.
- M2 `src/schema/`: `Ident::quoted`, `Identity`, `ForeignKey` targets.
- M4:
  - `src/value.rs`: `DbValue` incl. `Duration`, and `to_sql(&FieldRule)`
    (a duration binds its integer count).
  - `src/rules/`: `validate`, `FieldIssue`, `FieldError`, and
    `FieldIssueCode` incl. `Unique`, `ReferenceMissing`, `NotEditable`,
    `UnknownField`.
- M5 `src/model/`: effective field modes (`editable`, `create-only`,
  `read-only`, `hidden`), `can_create`, `required_on_create`, FK fields with
  their targets.
- M6 `src/session/`:
  - `Session` and `SessionError`;
  - `ObjectKey`, `ObjectView`, version hashing, `object()`;
  - freshness checks, `config()` (`LoadedConfig` with `hash()`) and
    `config_changed()`;
  - `src/session/sql.rs`, the only module that assembles SQL text.

Rules that bind this task:

- **SQL safety:** values are bound parameters, and names reach SQL only via
  `Ident` through `src/session/sql.rs`. Never use `INSERT OR IGNORE` or
  `OR REPLACE`: they hide constraint failures or delete rows.
- **Destructive operations are never silent.** The first write of every
  session is preceded by a backup. Any failure rolls the whole transaction
  back and is reported.
- One save is one `BEGIN IMMEDIATE` transaction. There are no
  multi-transaction batches, so no partial result can exist.
- No tauri. fmt and clippy pedantic must pass; silence a lint only with a
  scoped `#[expect]`. Tests run in temp dirs.
- Boundary types use serde camelCase and `ts_rs::TS`, contain no `bigint`,
  and take no name already used in `apps/desktop/src/lib/generated/`. Append
  them to `EXPORTERS` in `apps/desktop/src-tauri/src/generated_types.rs` and
  regenerate.

### Request and result

```rust
pub struct SaveRequest {
    pub table: String,
    pub key: Option<ObjectKey>,        // None: create
    pub version: Option<String>,       // required when key is Some
    /// The config hash the client's draft was checked against
    /// (`DbmanagerSession.config.hash`). The config reloads automatically when its
    /// file changes, so a save prepared under old rules must not slip through.
    pub config_hash: String,
    pub changes: Vec<FieldChange>,
}
pub struct FieldChange { pub column: String, pub input: FieldInput }
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FieldInput {
    Value { value: DbValue },             // plain fields: bool as integer "0"/"1",
                                          // duration as {"type":"duration","value":"1h30m"}
    Reference { key: Option<ObjectKey> }, // FK fields: the picked object, or None for NULL
}
pub struct SaveResult { pub object: ObjectView, pub backup_created: Option<String> } // display path
impl Session { pub fn save(&mut self, request: &SaveRequest) -> Result<SaveResult, SessionError>; }
```

New `SessionError` variants. They are struct variants, so the next milestone
can add link details without breaking callers:

- `ReadOnlyDatabase`
- `ConfigChangedOnDisk`
- `NotEditable(String)`
- `Validation { fields: Vec<FieldError> }`
- `Conflict { current: Box<ObjectView> }`
- `BackupFailed(String)`
- `Constraint(String)`, carrying SQLite's message.

### Order of checks

Nothing is written until step 6.

1. If the database is read-only → `ReadOnlyDatabase`. If
   `request.config_hash` differs from `self.config().hash()`, **or**
   `config_changed()` (the file changed and the automatic reload has not run
   yet) → `ConfigChangedOnDisk`: the caller reloads the config (the shell does
   this itself) and the client re-checks its draft and retries. If the table is
   hidden or unknown → `UnknownTable`.
2. Permissions: an update needs the table to be `editable`, and a create
   needs `can_create`. Otherwise → `NotEditable`.
3. Check each change:
   - an unknown or hidden column → issue `unknownField`;
   - a duplicate column → `unknownField` on the second;
   - the mode must be `editable` for an update, or `editable`/`create-only`
     for a create; otherwise → `notEditable`;
   - `Value` on a FK field, or `Reference` on a plain field → `wrongType`.
4. Run `validate(rule, value)` for each `Value` change, and keep the
   normalised value. **Only changed fields are validated**, so legacy values
   in untouched fields never block a save.
5. On a create, every `required_on_create` field that is not supplied →
   `required`. All issues are collected and returned together as
   `Validation { fields }`. If there are any, stop.
   - **No-op:** if there is nothing to write (an update with no changes),
     re-read the object. If its version differs → `Conflict`. Otherwise
     return it. No backup and no write transaction happen.
6. **Backup.** Taken once per session, before the first write (see below).
   If it fails → `BackupFailed`, and nothing is written.
7. `BEGIN IMMEDIATE`, then re-check `schema_version` → `SchemaChanged`.
8. On an update, re-read the row:
   - missing → `UnknownObject`;
   - version differs → `Conflict { current }`, and roll back.
9. Resolve every `Reference`. The picked key must exist in the FK's target
   table; otherwise the issue is `referenceMissing`, collected, the
   transaction rolls back, and the result is `Validation`. The stored value
   is the target row's referenced column.
10. Write.
    - Update: `UPDATE <t> SET <changed cols> = ? … WHERE <identity> = ?`,
      with only the changed columns.
    - Create: `INSERT INTO <t> (<supplied cols>) VALUES (…)`. The other
      columns take their defaults. An omitted rowid alias is assigned by
      SQLite. A rowid alias is exactly `Column::is_rowid_alias` from the
      introspection milestone, which excludes `INTEGER PRIMARY KEY DESC`.
      Such a column is an ordinary nullable column: it is never assumed to
      be assigned, and the new object's key is always the rowid.
    - Values bind through `to_sql(rule)`, so durations store their integer
      count in the field's unit. The new key comes from `last_insert_rowid()`
      for rowid tables, or from the supplied PK values.
11. *(The next milestone inserts link writes here, in the same transaction.)*
12. `COMMIT`. Re-read the object and return it, because triggers may have
    changed it.

**Constraint mapping.** Any SQLite error rolls back.

| SQLite error | Result |
| --- | --- |
| `SQLITE_CONSTRAINT_UNIQUE` / `_PRIMARYKEY` naming exactly one changed column (`UNIQUE constraint failed: t.c`) | `Validation`, issue `unique` on that column |
| `_NOTNULL` on one column | `required` on that column |
| Anything else (`CHECK`, `FOREIGNKEY`, `TRIGGER`, a multi-column unique) | `Constraint(message)` |
| `SQLITE_BUSY` | `Busy` |

### Reuse (the link milestone depends on these)

Keep these `pub(crate)` in `src/session/save.rs`, with no assumption that the
fields belong to an object table:

- `check_changes(fields: &[ResolvedField], changes: &[FieldChange], phase:
  Phase::{Create, Update}) -> (Vec<Checked>, Vec<FieldError>)`. This covers
  steps 3–5 over any resolved field set, including the `required` check for
  `Phase::Create`.
- `resolve_references(tx, checked) -> Result<Vec<Bound>, Vec<FieldError>>`,
  which is step 9.
- `map_constraint(err, changed_columns) -> SessionError`.
- `ensure_backup(&mut self) -> Result<Option<PathBuf>, SessionError>`
  (below).

### Backup (`src/session/backup.rs`)

- The backup is taken before the **first** write of a `Session`, whether a
  save, link changes or a delete, and never again for that session. Its path
  is kept on the session (`pub fn backup_path(&self) -> Option<&Path>`) and
  returned once in `backup_created`.
- It uses SQLite's online backup API (`rusqlite::backup`), so the copy is
  consistent even with other connections or WAL.
- The target is beside the database: `<file name>.<UTC yyyymmddThhmmssZ>.bak`
  (`shop.db` → `shop.db.20261113T101500Z.bak`). If that name is taken, add
  `-1`, `-2`, … before `.bak`.
  - Write into a temp file in the same folder, then move it into place
    **without replacing** an existing file (`tempfile`'s
    `persist_noclobber`).
  - On failure, remove the temp file and refuse the write.
- Build the name with `OsString` operations, not by formatting display
  strings, so non-UTF-8 names never panic.
- The app never deletes backups.
- `pub(crate) fn ensure_backup(&mut self) -> Result<Option<PathBuf>,
  SessionError>` returns the path only when it made one.
- **A reload does not reset the backup.** "Session" here means "this
  database, opened once in this app run, until it is closed or another
  database is opened". The shell's reload reopens the same file as a new
  `Session` value, so add `pub fn adopt_backup(&mut self, backup: PathBuf)`.
  It marks the backup as already taken and sets `backup_path()` to it,
  without writing anything. The caller uses it only for the same database
  path. Test `adopted_backup_is_not_retaken`: after `adopt_backup`, the next
  save creates no backup, returns `backup_created: None`, and `backup_path()`
  is the adopted path.

Exports: `SaveRequest`, `SaveResult`, and the types they reference.

### From the M4 review (2026-11-18)

**Never call `to_sql` on an unvalidated value.**

- `DbValue::to_sql` panics on a blob or on an unvalidated value; M4
  documents this.
- Bind only values that `validate` returned. `validate` and `to_sql` happen
  in the same function, so the value passed to `to_sql` is the `Ok` value
  `validate` returned for that change, never the raw input.
- A blob input is rejected as `wrongType` at step 2, before anything binds.
  So a panic cannot occur and lose the user's form.
- Test `blob_input_is_wrong_type_not_panic`.

**Unchanged non-finite reals stay saveable.**

- A row whose `price` is `+Inf` (read as `"inf"` with `WrongType`) can still
  be saved with other changes. The untouched `price` is not validated and not
  written, and stays `+Inf` byte-for-byte.
- Editing `price` requires a finite value.
- Test `unchanged_infinite_real_saves`.

## Files

Create `crates/fm-dbmanager/src/session/{save.rs, backup.rs}` and their
tests.

Edit:

- `src/session/mod.rs` and `src/session/sql.rs`;
- `crates/fm-dbmanager/Cargo.toml`: `tempfile` as a normal dependency,
  workspace version;
- `apps/desktop/src-tauri/src/generated_types.rs` and the generated files;
- `Cargo.lock`;
- `CLAUDE.md`: the layout line for `src/session/`.

Do not touch the frontend outside `lib/generated/`, other crates, or
`docs/plans/`.

## Acceptance criteria

Unless a criterion says otherwise, it runs against
`examples/dbmanager/schema.sql` + `example.toml` in a temp dir. That config
leaves most fields editable by default.

**Updates and conflicts**

- Updating `author.last_name` writes only that column and returns the new
  object and version. Prove the first part with a trigger that records the
  updated columns, or by checking that an untouched column holding a legacy
  invalid value stays byte-identical.
- An update with no changes writes nothing and takes no backup.
- A version mismatch → `Conflict` carrying the current object, and nothing is
  written.

**Backup**

- The first save creates exactly one backup beside the database, named per
  the rule. It opens as a database equal to the pre-save content.
- The second save creates no backup, and `backup_created` is `Some` only the
  first time.
- A blocked backup fails with `BackupFailed` and leaves the database
  unchanged. Block it by making the folder read-only, or by pre-creating a
  directory at the target name and at every `-n` variant in the test.

**Field checks**

Each check in steps 1–5 has a test, including:

- `NotEditable` for an update of `publisher` (`mode = "pickable"`), and for
  a create in it;
- `notEditable` for `book.isbn` (create-only) and for `book.revision`
  (read-only) on update;
- `unknownField` for `author.internal_note` (hidden);
- `wrongType` for a `Value` on `book.author_id`;
- `required` for a missing `book.isbn` on create;
- several issues returned together.

**Durations**

- Saving `book.audio_length` as `"1h 30m"` stores the integer `5400`, and
  the returned object shows `"1h30m"`.
- `"90ms"` → `durationPrecision`.
- `"30s"` → `belowMin` with limit `"1m"`.

**Creates and constraints**

- Creating a `book` with `author_id` by `Reference` returns the new object
  with its link.
- Creating a `genre` works, because `allow_create` is inherited as `true`.
- An `isbn` that duplicates another book's → `Validation` with `unique` on
  `isbn`.
- A `CHECK` constraint failure (test schema) → `Constraint`, and the
  transaction is rolled back.

**Refusals**

- `ReadOnlyDatabase` for a read-only file.
- `ConfigChangedOnDisk` when `config_hash` is stale (after `set_config` with a
  different config), and when the file was edited but not yet reloaded; a
  retry with the current hash after `set_config` succeeds.
- `SchemaChanged` after DDL from another connection.
- `Busy` when a second connection holds `BEGIN EXCLUSIVE`, after the
  timeout. Set a short timeout in the test via `conn().busy_timeout`.

**General**

- Awkward identifiers (`a"b`, `select`, `with space`, `ünï`) save and create
  correctly.
- Generated types are current, and fmt, clippy pedantic and `cargo test
  --workspace` pass.

## Tests proving completion

Also, from the M4 review: `blob_input_is_wrong_type_not_panic`,
`unchanged_infinite_real_saves`.

Updates and conflicts:

- `save_updates_only_changed_columns`
- `save_returns_new_version`
- `empty_save_writes_nothing`
- `conflict_on_stale_version`

Backup:

- `first_write_backs_up_once`
- `backup_name_never_replaces`
- `backup_failure_blocks_write`

Refusals:

- `rejects_read_only_database`
- `rejects_config_changed`
- `rejects_schema_changed`
- `busy_when_locked`

Field checks and validation:

- `permission_checks`
- `validation_collects_all_issues`
- `legacy_values_do_not_block`
- `duration_save_and_issues`

Creates and constraints:

- `create_with_reference`
- `create_in_integer_primary_key_desc_table` (the new key is the rowid;
  the omitted `id` stays NULL and is not used to address the object)
- `create_inherits_allow_create`
- `create_requires_required_fields`
- `reference_missing`
- `unique_maps_to_field`
- `check_maps_to_constraint_and_rolls_back`

General:

- `awkward_identifiers_save`

Run `cargo test -p fm-dbmanager`, then regenerate, then `cargo test
--workspace`, then fmt and clippy as in `CLAUDE.md`.

## Out of scope

- Many-to-many link changes, which come in the next milestone.
- Deleting objects and the impact preview, which come later.
- Tauri commands and the mapping to the UI error type.
- Composite FKs and blobs, which are not in v1.

## Risks

- A backup of a large database blocks the calling thread. The Tauri layer
  runs it on a blocking worker.
- Parsing SQLite's constraint message for the column is best effort. When it
  does not name exactly one changed column, fall back to `Constraint`.
- Triggers may make the saved object differ from the request. The returned
  object is re-read, so the UI shows the truth.
