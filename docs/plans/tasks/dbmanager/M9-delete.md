# M9 — Delete: impact preview and confirmed delete

Status: awaiting approval (renumbered from M8 and revised 2026-11-16: link
fields, editable defaults; then the config hash on writes and `pickable`
tables)
Project: dbmanager   Depends on: M8 (landed)

**Note (2026-11-21): the UI is moving to GPUI.** This domain milestone is
unchanged, with one condition. If `apps/desktop/src-tauri/src/generated_types.rs`
still exists when you start, derive `ts_rs::TS` and update `EXPORTERS` and
the generated files as written below. If it no longer exists (the migration
removed Tauri and ts-rs), skip every ts-rs step: no `ts_rs::TS` derive, no
`#[ts(...)]` attribute, no export, no regeneration, and keep `serde` only
where this plan needs it for something other than the TypeScript boundary.
Wherever this plan says the Tauri commands or the UI error type come later,
read "the GPUI app's Database Manager model comes later".

## Goal

Let a `Session` describe exactly what deleting an object would do to every
table that references it, and delete the object only when the caller confirms
that same impact, in one transaction after the session's backup.

## Context

The Database Manager (`dbmanager`) edits a user's SQLite file. Deleting an
object is **destructive**, so by the repository's invariants it is confirmed
and recoverable: the UI shows a confirmation with the impact this milestone
computes, and the write is preceded by the session's one-time backup (M7).
Delete is offered only where the config allows it (`can_delete`: table
`editable` and `allow_delete = true`, see `docs/dbmanager-config.md`). Tables
are editable by default, but `allow_delete` is **off by default**: a table
opts in.

Many-to-many links are junction rows. A junction may carry **link fields**
(extra columns with user data); when a delete cascades into such rows, those
values are lost, and the impact must say so.

FK enforcement is on for every connection (`PRAGMA foreign_keys = ON`, M1), so
SQLite applies `ON DELETE` actions itself: `CASCADE` deletes referencing rows
(recursively), `SET NULL`/`SET DEFAULT` update them, `RESTRICT`/`NO ACTION`
make the delete fail if referencing rows exist. A `SET NULL` on a `NOT NULL`
column also fails.

Builds on:

- M2 `src/schema/`: `Schema::referencing(table)` (every FK in **any** table,
  hidden ones included, targeting a table), `ForeignKey { from, target,
  on_delete: FkAction }`, `has_triggers`, `Ident::quoted`.
- M5 `src/model/`: which tables are hidden, display names, labels,
  many-to-many junctions (relation mode per side, the two relation columns,
  whether the junction has link fields), `can_delete`.
- M6 `src/session/`: `Session`, `ObjectKey`, version hashing, label
  rendering, freshness checks, `src/session/sql.rs` (the only module that
  assembles SQL text).
- M7/M8 `src/session/`: `ensure_backup` (once per session; `BackupFailed`),
  `SessionError::{ReadOnlyDatabase, ConfigChangedOnDisk, NotEditable,
  Conflict { current, link }, BackupFailed, Constraint}` (a delete conflict
  has `link: None`), `Busy` mapping, `backup_path()`.

Rules that bind this task:

- **Impact includes hidden tables.** Hidden tables are counted (display name or
  SQL name, row count) but no labels from them are shown. Hiding data loss
  would make the delete silent.
- SQL safety: bound values; names only via `Ident` in `src/session/sql.rs`.
- One delete = one `BEGIN IMMEDIATE` transaction; failure rolls back
  entirely and is reported.
- No tauri; fmt, clippy pedantic; scoped `#[expect]`; tests in temp dirs.
- Boundary types: serde camelCase, `ts_rs::TS`, no `bigint` (counts `u64` with
  `#[ts(type = "number")]`), names not already used in
  `apps/desktop/src/lib/generated/`; append to `EXPORTERS` in
  `apps/desktop/src-tauri/src/generated_types.rs`; regenerate.

### Types

```rust
pub enum OnDeleteAction { Cascade, SetNull, SetDefault, Restrict, NoAction }  // camelCase
pub struct ImpactGroup {
    pub table: String,                 // SQL name
    pub table_display_name: String,    // display name; the SQL name for hidden tables without one
    pub hidden: bool,
    pub via: String,                   // FK column display name(s) (SQL names for hidden tables), joined ", "
    pub action: OnDeleteAction,
    pub count: u64,                    // rows affected through this FK
    pub depth: u32,                    // 1 = references the object directly; 2+ = through cascades
    pub truncated: bool,               // count is a lower bound
    pub sample: Vec<String>,           // up to 10 labels; empty when hidden
    pub links: bool,                   // rows of a junction used as many-to-many
    pub link_data: bool,               // links whose junction has extra columns: their values are lost
    pub blocks: bool,                  // this group makes the delete fail
}
pub struct DeleteImpact {
    pub table: String, pub key: ObjectKey, pub label: String, pub version: String,
    pub groups: Vec<ImpactGroup>,      // breadth-first: depth, then table, then via
    pub blocked: bool,                 // any group blocks
    pub truncated: bool,
    pub has_triggers: bool,            // the object's table or any affected table has triggers
    pub token: String,                 // hash of everything above except labels
}
pub struct DeleteResult { pub table: String, pub label: String, pub impact: DeleteImpact,
                          pub backup_created: Option<String> }
impl Session {
    pub fn delete_preview(&self, table: &str, key: &ObjectKey) -> Result<DeleteImpact, SessionError>;
    /// `config_hash`: the config hash the caller's preview was shown under
    /// (the config reloads automatically; a stale hash is refused).
    pub fn delete(&mut self, table: &str, key: &ObjectKey, version: &str, token: &str,
                  config_hash: &str) -> Result<DeleteResult, SessionError>;
}
```

New `SessionError` variants: `ImpactChanged(Box<DeleteImpact>)` (the fresh
impact) and `DeleteBlocked(Box<DeleteImpact>)`.

### Computing the impact

Breadth-first over rows, starting from the object:

- For each row set R in table T being deleted, and each FK `f` (from any
  table C) whose target is T: the affected rows of C are those whose `f.from`
  columns equal R's `f.target` columns. Group per (C, f).
  - `CASCADE`: those rows are deleted too; add them to the next level
    (depth + 1) and continue from them.
  - `SET NULL`: rows are updated; `blocks` if any `f.from` column is `NOT
    NULL` and count > 0.
  - `SET DEFAULT`: rows are updated; not predicted to block (SQLite will
    report it if the default violates a constraint).
  - `RESTRICT` / `NO ACTION`: `blocks` if count > 0.
- Rows already scheduled for deletion are not counted twice (track keys per
  table). A row of a self-referencing table that cascades to itself is
  handled the same way.
- Stop enumerating after **10 000** affected rows in total: mark the groups
  touched afterwards and the impact `truncated`, counts as lower bounds.
- Groups with count 0 are omitted.
- `links` is true for groups whose table is a junction used as many-to-many;
  `link_data` is also true when that junction has any column besides the two
  relation columns (and an optional rowid alias), visible or hidden.
- `hidden` is true when the group's table is hidden, or, for a junction used
  as many-to-many (which is not listed as a table), when the relation is
  hidden. A visible link group's `sample` holds the labels of the objects on
  the **other side** of each removed link (the side not being deleted), and
  its `table_display_name` is the relation's heading on the deleted row's
  side; other groups' samples are the affected rows' labels.
- `token`: SHA-256 (hex) over the object's version and, for every group in
  order, table, via, action, count, depth, truncated, blocks, links and
  link_data.

### Deleting

1. `ReadOnlyDatabase`; `ConfigChangedOnDisk` when `config_hash` differs from
   `self.config().hash()` or `config_changed()` is true; `UnknownTable`;
   `NotEditable` (unless `can_delete`; a `pickable` table never can) — checked
   first, nothing written.
2. `BEGIN IMMEDIATE`; `schema_version` check (`SchemaChanged`); re-read the
   row (`UnknownObject` if gone); version differs → `Conflict { current,
   link: None }`.
3. Backup if this is the session's first write, **inside** the transaction
   (M7's order, after its follow-up; `BackupFailed` or `Busy` → roll back).
   `backup_created` is reported on the first successful write after the
   backup, M7's announced flag; a failed first delete does not lose it.
   Test: `failed_first_delete_then_success_announces_backup`.
4. Recompute the impact inside the transaction. Token differs →
   `ImpactChanged(fresh)`. `blocked` → `DeleteBlocked(impact)`.
5. `DELETE FROM <t> WHERE <identity> = ?`. An FK failure here (an
   unpredicted block, e.g. via `SET DEFAULT` or deferred constraints) rolls
   back → `DeleteBlocked` with the impact; other constraint errors →
   `Constraint`; busy → `Busy`.
6. `COMMIT`; return `DeleteResult`.

Exports: `DeleteImpact`, `DeleteResult` (and referenced types).

## Files

Create `crates/fm-dbmanager/src/session/delete.rs` and tests. Edit
`src/session/mod.rs`, `src/session/sql.rs`,
`apps/desktop/src-tauri/src/generated_types.rs`, generated files,
`CLAUDE.md` (layout line for `src/session/`).

Do not touch the frontend outside `lib/generated/`, other crates, or
`docs/plans/`.

## Acceptance criteria

Against `examples/dbmanager/schema.sql` + `example.toml` in a temp dir:

- Preview deleting an author with 2 books (each with genres, loans and
  contributions) who is also a contributor to a third book: groups `book`
  (cascade, depth 1, count 2, sample titles), `contribution` (cascade, depth
  1, `links: true`, `link_data: true`, not hidden, sample = the third book's
  title, display name = the author side's heading "Contributions"),
  `book_genre` (cascade, depth 2, `links: true`, `link_data: false`, sample
  genre names), `loan` (cascade, depth 2), `contribution` of the deleted books
  (depth 2); not blocked. Deleting with that token removes all of them and
  creates the backup if it is the session's first write.
- Preview deleting a publisher referenced by a book: `blocks` on the `book`
  group (`RESTRICT`), `blocked: true`; `delete` → `DeleteBlocked`, nothing
  changed. (`publisher` is pickable in the example: test with a config where
  it is editable and `allow_delete = true`.)
- Preview deleting a member with loans: `loan` group `SetNull`, not blocking
  (nullable `member_id`); after delete those loans have `member_id` NULL.
- A `SET NULL` onto a `NOT NULL` column (test schema) blocks.
- Hidden referencing tables (e.g. a test table referencing `member` with
  `mode = "hidden"`) appear with `hidden: true`, a count and no sample; a
  link group of a relation with `mode = "hidden"` is `hidden: true` too.
- Adding a referencing row between preview and delete → `ImpactChanged` with
  the new impact; nothing deleted.
- Editing the object between preview and delete → `Conflict`.
- `ConfigChangedOnDisk` for a stale `config_hash`; nothing deleted.
- `NotEditable` for `publisher` (pickable), and when `can_delete` is false:
  `genre` in the example (editable,
  `allow_delete` left at its default `false`), and every table with no
  config at all.
- A self-referencing table (`parent_id REFERENCES node ON DELETE CASCADE`)
  with a 3-level tree: depths 1 and 2, no double counting.
- A cascade fan-out over 10 000 rows (test schema) sets `truncated`.
- `has_triggers` is true when an affected table has a trigger.
- Awkward identifiers delete correctly.
- Generated types current; fmt, clippy pedantic, `cargo test --workspace`
  pass.

## Tests proving completion

`impact_cascades_through_levels`, `impact_restrict_blocks`,
`impact_set_null_allowed`, `impact_set_null_not_null_blocks`,
`impact_counts_hidden_tables_without_labels`, `impact_link_groups_and_link_data`,
`impact_self_reference_no_double_count`,
`impact_truncates`, `impact_flags_triggers`, `delete_applies_cascade`,
`delete_backs_up_first`, `delete_blocked_changes_nothing`,
`delete_impact_changed`, `delete_conflict`, `delete_not_allowed`,
`delete_off_by_default`, `delete_stale_config_hash`, `delete_pickable_refused`,
`delete_read_only_database`, `delete_awkward_identifiers`.

Run: `cargo test -p fm-dbmanager`; regenerate; `cargo test --workspace`;
fmt and clippy per `CLAUDE.md`.

## Out of scope

- Batch delete (several objects at once): not in v1.
- Predicting trigger effects (flagged only).
- Tauri commands: M10.

## Risks

- `PRAGMA foreign_keys` cannot change inside a transaction; it is already on
  from open (M1).
- Deferred FK constraints fail at `COMMIT`, not at the `DELETE`: handle the
  commit error the same way (rollback, `DeleteBlocked`).
