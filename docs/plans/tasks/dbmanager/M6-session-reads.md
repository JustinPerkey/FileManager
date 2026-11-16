# M6 — Session and reads: lists, objects, relations, checklists

Status: awaiting approval (revised 2026-11-16: editable defaults, durations,
link values)
Project: dbmanager   Depends on: M5 (landed)

## Goal

Add `Session`, which owns one open database and its resolved model, detects
schema and config changes, and answers every read the UI needs: paged and
searchable object lists, objects with typed values (durations included), FK
links, reverse relations, and many-to-many checklists with each link's field
values.

## Context

The Database Manager (`dbmanager`) presents a SQLite database as an object
editor: pick a table, see its objects by label, open an object as a form.
Following a FK field opens the referenced object; an object also lists who
references it; a many-to-many relation shows every object on the other side
with the linked ones ticked; a link may carry **link fields** (the
junction's extra columns), shown and edited with the link. Anything the config
does not mention is editable by default. The config semantics (defaults,
modes, hidden, labels, search, ordering, kinds, booleans, durations, link
fields) are in `docs/dbmanager-config.md`; this milestone reads data according
to them.

Builds on:

- M1 `src/db.rs`: `Database::open` (`OpenError`), `conn()`,
  `read_only_reason()`, `schema_version()`.
- M2 `src/schema/`: `introspect`, `Schema`, `Identity` (`Rowid { accessor,
  alias_column }` | `PrimaryKey { columns }`), `ForeignKey`, `Ident` with
  `quoted()`.
- M3 `src/config/`: `parse` → `ConfigReport`.
- M3 `src/config/`: also `Config::fail_closed()` (every default read-only).
- M4 `src/value.rs` (`DbValue` incl. `Duration`, from `ValueRef`),
  `src/rules/` (`FieldKind` incl. `Duration`, `FieldRule` incl. `unit`,
  `validate`, `FieldIssueCode`, `duration::format`).
- M5 `src/model/`: `resolve(&Schema, &ConfigReport) -> Model`,
  `Model::table`, `table_views`, `diagnostics`, the per-table resolved data
  (identity, visible fields and modes, label template as column indexes,
  search and order columns, FK fields with targets, reverse relations,
  many-to-many sides with their resolved link fields and junction identity),
  `M2mView.link_fields`, relation ids `fk:<table>:<id>` / `m2m:<junction>`.

Rules that bind this task:

- **SQL safety.** Values are always bound parameters. Names reach SQL only as
  `Ident::quoted()`. Create `src/session/sql.rs` as **the only module that
  assembles SQL text**; its builders take `&Ident` (never `&str`) for names.
  Search text is a bound `LIKE` pattern with `%`, `_` and `\` escaped and
  `ESCAPE '\'`.
- **Hidden never leaks**: hidden columns are never selected into a view
  (they are read only for the version hash); hidden tables are never listed,
  labelled, or used as checklist sides.
- No tauri; fmt, clippy pedantic; scoped `#[expect]`; every test builds its
  database in its own temp dir.
- Boundary types: `#[derive(..., Serialize, ts_rs::TS)]` (plus `Deserialize`
  for anything the UI sends), camelCase, no `bigint` (counts: `u64` with
  `#[ts(type = "number")]` or `u32`), names not already used in
  `apps/desktop/src/lib/generated/`; append to `EXPORTERS` in
  `apps/desktop/src-tauri/src/generated_types.rs` and regenerate.

### Session

```rust
// src/session/mod.rs
pub struct Session { /* Database, Schema, Model, config source (path, sha256), backup path (M7) */ }
pub enum ConfigLoadError { Unreadable { path: PathBuf, message: String } } // I/O or not UTF-8
impl Session {
    /// Opens the database and, if given, reads and parses the config. An unreadable
    /// config does not fail the open: the session resolves with
    /// `Config::fail_closed()` (everything read-only; the user meant a config to
    /// apply, so the editable built-in defaults must not) and returns the config
    /// error beside it. With no config path, the built-in (editable) defaults apply.
    pub fn open(database: &Path, config: Option<&Path>)
        -> Result<(Session, Option<ConfigLoadError>), SessionError>;
    pub fn database_path(&self) -> &Path;
    pub fn config_path(&self) -> Option<&Path>;
    pub fn read_only_reason(&self) -> Option<&str>;
    pub fn diagnostics(&self) -> &[ConfigDiagnostic];
    pub fn table_views(&self) -> Vec<TableView>;
    /// True if the config file's bytes no longer hash to what was loaded (or it
    /// is gone/unreadable). Always false without a config.
    pub fn config_changed(&self) -> bool;
    pub fn list(&self, table: &str, query: &ListQuery) -> Result<ObjectPage, SessionError>;
    pub fn object(&self, table: &str, key: &ObjectKey) -> Result<ObjectView, SessionError>;
    pub fn m2m_options(&self, table: &str, key: Option<&ObjectKey>, relation: &str,
                       query: &M2mQuery) -> Result<M2mPage, SessionError>;
}
```

`SessionError` (domain error, `Display` + `Error`; not a boundary type; later
milestones add variants): `Open(OpenError)`, `UnknownTable(String)`,
`UnknownObject`, `UnknownRelation(String)`, `SchemaChanged`, `Busy`,
`Sqlite(String)`. `SQLITE_BUSY`/`SQLITE_LOCKED` map to `Busy`.

**Freshness:** every `list`/`object`/`m2m_options` first compares `PRAGMA
schema_version` with the value recorded at introspection; a difference is
`SchemaChanged` (the caller reloads by opening a new session). Each read
runs in one read transaction (`BEGIN DEFERRED … COMMIT`) so counts and pages
agree.

Config bytes are hashed with SHA-256 (`sha2`, as `fm-tarpack` does; hex via
`fm_core::to_hex`). A leading BOM is stripped before parsing, not before
hashing.

### Keys and values

```rust
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ObjectKey { Rowid { value: String }, Primary { values: Vec<DbValue> } }
```

**Values of duration fields:** a stored integer becomes
`DbValue::Duration(duration::format(n, unit))`; anything else is returned as
stored, with problem `WrongType`. Other kinds use `DbValue` from storage
unchanged.

Rowid tables use `Rowid` (the rowid as a decimal string, read through the
identity's accessor); `WITHOUT ROWID` tables use `Primary` with the PK values
in PK order. A key of the wrong shape for its table, or one that matches no
row, is `UnknownObject`. Key values bind by their own variant (integer →
`i64`, real → `f64`, text → text). The UI treats keys as opaque.

**Version:** SHA-256 over every column of the row (hidden ones included), in
column order, each encoded as a type tag byte then its bytes (integer as
8-byte big-endian, real as its bit pattern, text and blob as length-prefixed
bytes, null as the tag alone), hex-encoded. Any change to any column changes
it.

**Labels:** rendered in Rust from the table's resolved template (doc
"Labels"): NULL → empty, integer/real → decimal text, bool → `true`/`false`,
duration → canonical text;
empty result or no template → `"<display name> #<key>"` with the rowid or
the PK values joined by `, `.

### Queries

```rust
pub struct ListQuery { pub search: Option<String>, pub offset: u32, pub limit: u32,
                       pub referencing: Option<ReferencingFilter> }
pub struct ReferencingFilter { pub relation: String, pub key: ObjectKey }
pub struct ObjectSummary { pub key: ObjectKey, pub label: String }
pub struct ObjectPage { pub items: Vec<ObjectSummary>, pub total: u64, pub offset: u32 }
```

- `limit` is clamped to 1..=500. Order: the table's resolved order, then the
  identity (so paging is deterministic). `total` counts the same filter.
- `search` (trimmed; empty means none) matches any search column with
  `LIKE`; on a table that is not searchable it is ignored.
- `referencing`: `relation` must be one of `table`'s FKs (`fk:<table>:<id>`)
  that appears as a reverse relation of the key's table; rows whose FK
  columns equal the referenced row's target columns. Otherwise
  `UnknownRelation`. This one query serves plain lists, FK pickers (list the
  target table), and "referenced by" lists.

```rust
pub struct ObjectView {
    pub table: String, pub key: ObjectKey, pub label: String, pub version: String,
    pub fields: Vec<FieldValue>,            // TableView.fields order; visible only
    pub referenced_by: Vec<RelationCount>,  // one per ReverseRelationView, same order
    pub many_to_many: Vec<RelationCount>,   // one per M2mView, linked count
}
pub struct FieldValue {
    pub column: String, pub value: DbValue,
    pub checked: Option<bool>,              // bool kind: Some for 0/1; None for NULL or a problem
    pub link: Option<LinkTarget>,           // FK fields with a non-NULL value
    pub problem: Option<ValueProblem>,
}
pub struct LinkTarget { pub table: String, pub key: Option<ObjectKey>, pub label: Option<String>, pub navigable: bool }
pub enum ValueProblem { WrongType, NotBool, OutOfBounds, DanglingReference }
pub struct RelationCount { pub relation: String, pub count: u64 }
```

- `WrongType`: the storage class does not fit the kind (integer kind:
  integer; real: integer or real; text: text; bool: integer; duration:
  integer). `NotBool`: a
  bool field holding an integer other than 0/1. `OutOfBounds`: `validate`
  of the stored value against the field's rule reports a bound issue
  (`belowMin`, `aboveMax`, `tooShort`, `tooLong`, `patternMismatch`,
  `notAllowed`, or `required` for a NULL in a field the config made
  non-nullable). One problem per field, in that priority order.
- `LinkTarget`: navigable target → look the row up by the FK's target
  columns; found → `key` and `label`; not found → `key: None`, `label:
  None`, problem `DanglingReference`. Hidden target → `navigable: false`,
  `key` and `label` `None`, no lookup.

```rust
pub struct M2mQuery { pub search: Option<String>, pub offset: u32, pub limit: u32, pub linked_only: bool }
pub struct M2mOption {
    pub key: ObjectKey, pub label: String, pub linked: bool,
    /// Linked options only: the link's visible link fields, in M2mView.link_fields
    /// order, read exactly like object fields (FieldValue: problems, links, durations).
    pub link_values: Option<Vec<FieldValue>>,
    /// Linked options only: the junction row's version (same hash as objects,
    /// over every junction column, hidden ones included).
    pub link_version: Option<String>,
}
pub struct M2mPage { pub items: Vec<M2mOption>, pub total: u64, pub linked_total: u64, pub offset: u32 }
```

`m2m_options` lists **every object of the other table** (in its order, with
its search and paging; no other filter), each flagged `linked` if a junction
row joins it to `key`; linked entries carry `link_values` and `link_version`
(both `None` when not linked; `link_values` is `Some(vec![])` for a relation
without link fields).
`key: None` (a new object): nothing is linked, `linked_only` yields an empty
page. `relation` must be an `m2m:` relation with a checklist on `table`'s
side, else `UnknownRelation`.

Exports: `ObjectKey`, `ListQuery`, `ReferencingFilter`, `ObjectPage`,
`ObjectView`, `M2mQuery`, `M2mPage` (referenced types follow).

## Files

Create `crates/fm-dbmanager/src/session/{mod.rs, read.rs, sql.rs, key.rs,
version.rs, label.rs (or reuse model/labels.rs), tests/…}`. Edit
`src/lib.rs`, `crates/fm-dbmanager/Cargo.toml` (`sha2`),
`apps/desktop/src-tauri/src/generated_types.rs`, generated files, `Cargo.lock`,
`CLAUDE.md` (layout: `src/session/`).

Do not touch the frontend outside `lib/generated/`, other crates, or
`docs/plans/`.

## Acceptance criteria

Against `examples/dbmanager/schema.sql` + `example.toml` in a temp dir unless
stated:

- `list("author", …)` returns labels `"Last, First"` ordered by last then
  first name; paging with limit 2 walks every author once; `total` is right;
  search `"ar"` matches case-insensitively; a search containing `%` or `_`
  matches them literally.
- `list("book", referencing: fk from book to author, key of an author)`
  returns exactly that author's books.
- `object("book", key)`: fields in `field_order` order; `in_print` has
  `checked: Some(true)`; `author_id` has a navigable link with the author's
  key and label; a book with a NULL publisher has no link; `revision` is
  present; `audio_length` stored as `5400` reads as
  `{"type":"duration","value":"1h30m"}`; `author.internal_note` never appears
  in any view.
- A loan's `loan_period` of `1209600` reads as `"14d"`; a text value in a
  duration column is `WrongType`; `audio_length = 30` (below `"1m"`) is
  `OutOfBounds`.
- A stored `in_print = 2` is `NotBool`; text in `pages` is `WrongType`;
  `pages = 0` is `OutOfBounds`; a FK to a deleted row (inserted with
  `foreign_keys` off) is `DanglingReference`.
- `referenced_by` counts and `many_to_many` linked counts are right.
- `m2m_options("book", key, "m2m:book_genre", …)` lists all genres with the
  book's genres `linked`; `linked_only` lists only those; `linked_total` is
  right; with `key: None` nothing is linked; every linked option has
  `link_values: Some([])` and a `link_version`.
- `m2m_options("book", key, "m2m:contribution", …)` lists every author;
  linked ones carry `link_values` for `role` and `position` (in that order)
  and a `link_version` that changes when the junction row changes (including a
  hidden link field, if one is configured hidden in the test config); from the
  author side the same links carry the same values.
- With **no config**, every table is editable and listed except detected
  junctions.
- `version` changes when any column changes, including a hidden one, and is
  stable otherwise.
- `UnknownTable` for a hidden or missing table; `UnknownObject` for a wrong
  key; `UnknownRelation` for a relation of another table.
- Altering the schema from a second connection makes the next read fail with
  `SchemaChanged`. Editing the config file makes `config_changed()` true.
- A config that cannot be read opens the session fail-closed (everything
  read-only) and returns a `ConfigLoadError`.
- **Awkward identifiers:** a schema whose tables and columns are named `a"b`,
  `select`, `with space` and `ünï`, with FKs and a junction between them, is
  listed, searched, opened, and checklisted correctly.
- Only `src/session/sql.rs` assembles SQL text containing names (reviewer
  checks).
- Generated types current, no `bigint`; fmt, clippy pedantic, `cargo test
  --workspace` pass.

## Tests proving completion

`src/session/tests/`: `list_orders_and_labels`, `list_paging_is_complete`,
`list_search_case_insensitive`, `list_search_escapes_wildcards`,
`list_referencing`, `object_fields_and_links`, `object_hides_hidden`,
`value_problems`, `duration_values_and_labels`, `relation_counts`,
`m2m_options_all_and_linked`, `m2m_options_new_object`,
`m2m_options_link_values`, `link_version_tracks_junction_row`,
`no_config_is_editable`, `version_tracks_every_column`,
`unknown_table_object_relation`, `schema_change_detected`,
`config_change_detected`, `unreadable_config_opens_read_only`,
`awkward_identifiers_end_to_end`, `rowid_accessor_shadowed`,
`without_rowid_keys`.

Run: `cargo test -p fm-dbmanager`; regenerate; `cargo test --workspace`;
fmt and clippy per `CLAUDE.md`.

## Out of scope

- Any write, the backup, conflict handling: M7. Link writes: M8. Delete: M9.
- Tauri commands and the session snapshot: M10.

## Risks

- `LIKE` is case-insensitive only for ASCII; documented in the config doc.
- Large tables: `COUNT(*)` and `OFFSET` are linear; acceptable for v1.
- Text that is not valid UTF-8 in the database is shown lossily; editing such
  a field replaces it (the version check still protects concurrent edits).
