# M6 — Session and reads: lists, objects, relations, checklists

Status: implemented; the reviewer requested changes. **Run "Review round 1"
(end of this plan).** (Approved 2026-11-19; revised 2026-11-16: editable defaults, durations,
link values; then the human's final answers: one fixed config file loaded
separately and swappable for automatic reload, `pickable` tables)
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

- M1 `src/db.rs`: `Database::open` (`OpenError`, whose tuple-like variants
  carry the `PathBuf`, incl. `Busy(path)`), `conn()`,
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
pub struct Session { /* Database, Schema, Model, LoadedConfig, backup path (M7) */ }

// src/config/load.rs — the app has ONE config file at a fixed path (the shell
// passes it in; tests pass a temp path). Reading it is separate from opening a
// database, because its status is shown even when no database is open.
pub enum ConfigStatus { Loaded, Missing, FailClosed }          // ts: "loaded" | "missing" | "failClosed"
pub struct LoadedConfig { /* path, status, report: ConfigReport, hash, fail_closed reason */ }
impl LoadedConfig {
    /// Reads and parses `path`:
    /// - file absent (NotFound) → `Missing`: `Config::default()` (editable
    ///   built-in defaults), no diagnostics, hash `"missing"`;
    /// - unreadable (other I/O error, not UTF-8) → `FailClosed`:
    ///   `Config::fail_closed()`, reason "the config file could not be read: …";
    /// - parsed with `ConfigReport.fail_closed` set (syntax error, unsupported
    ///   version) → `FailClosed` with that reason;
    /// - otherwise `Loaded` (entry-level errors are diagnostics, not fail-closed).
    /// The hash is the SHA-256 (hex) of the exact bytes.
    pub fn load(path: &Path) -> LoadedConfig;
    pub fn path(&self) -> &Path;
    pub fn status(&self) -> ConfigStatus;
    pub fn hash(&self) -> &str;
    pub fn fail_closed_reason(&self) -> Option<&str>;
    /// Config-only diagnostics (no schema needed); shown when no database is open.
    pub fn diagnostics(&self) -> &[ConfigDiagnostic];
    /// True if the file now differs from what was loaded (re-reads and hashes it;
    /// appearing, disappearing and becoming unreadable all count).
    pub fn changed_on_disk(&self) -> bool;
}

impl Session {
    /// Opens the database and resolves its model with `config` (already loaded).
    pub fn open(database: &Path, config: LoadedConfig) -> Result<Session, SessionError>;
    /// Replaces the config and re-resolves the model against the already
    /// introspected schema. No database I/O except a `schema_version` check.
    /// Used by the automatic reload when the config file changes.
    pub fn set_config(&mut self, config: LoadedConfig);
    pub fn config(&self) -> &LoadedConfig;
    pub fn database_path(&self) -> &Path;
    /// Some(short reason) iff the model was resolved fail-closed (everything
    /// read-only): `self.config().fail_closed_reason()`. None otherwise,
    /// including a missing config file.
    pub fn config_fail_closed(&self) -> Option<&str>;
    pub fn read_only_reason(&self) -> Option<&str>;
    pub fn diagnostics(&self) -> &[ConfigDiagnostic];
    pub fn table_views(&self) -> Vec<TableView>;
    /// `self.config().changed_on_disk()`.
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
hashing. The hash is how writes (later milestones) prove they were checked
against the config now in force.

**Pickable tables** (`TableView.mode == "pickable"`) are readable like any
visible table: `list` (FK pickers), `object` (following a link opens it; every
field is read-only), and as the other side of `m2m_options`.

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
the PK values joined by `, `. **Exception:** if any identity column is hidden, the fallback is
the display name alone, so the hidden value never shows (Review round 1, R1).

### Queries

```rust
pub struct ListQuery { pub search: Option<String>, pub offset: u32, pub limit: u32,
                       pub referencing: Option<ReferencingFilter> }
pub struct ReferencingFilter { pub relation: String, pub key: ObjectKey }
pub struct ObjectSummary { pub key: ObjectKey, pub label: String }
pub struct ObjectPage { pub items: Vec<ObjectSummary>, pub total: u64, pub offset: u32 }
```

- `limit` is clamped to 1..=500. Order: the table's resolved order
  (`ResolvedTable.order`, which holds only the explicit or label terms),
  then **append the row identity as the final `ORDER BY` tie-break**: the
  rowid accessor, or every primary-key column in key order. Paging is then
  deterministic even when labels repeat. `total` counts the same filter. The
  same tie-break applies to `m2m_options` and to referencing lists. Test
  `paging_stable_with_duplicate_labels` (20 authors with the same name, limit
  7: every row exactly once).
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
  None`, problem `DanglingReference`. A FK into a hidden table or an
  unlisted junction has no `foreign_key` view (the model omits it), so its
  value gets **no** `link` at all and is shown as a plain value. No table
  name reaches the UI.

```rust
pub struct M2mQuery {
    pub search: Option<String>, pub offset: u32, pub limit: u32, pub linked_only: bool,
    /// Fetch exactly this one other-side object (e.g. to refresh a single link after a
    /// conflict): the page holds it, or nothing if it no longer exists. `search`,
    /// `offset` and `linked_only` are ignored; `total` is 0 or 1; `linked_total` is
    /// still the relation's full linked count. `#[serde(default)]`, `#[ts(optional)]`.
    pub only: Option<ObjectKey>,
}
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
`ObjectView`, `M2mQuery`, `M2mPage`, `ConfigStatus` (referenced types follow).

### From the M4 review (2026-11-18): non-finite reals

SQLite can store `+Inf`/`-Inf` in a REAL column. M4's `DbValue` reads them
as `"inf"`/`"-inf"`, which `validate` rejects (`notReal`).

- Show such a value as stored, with problem **`WrongType`**: the stored value
  cannot be edited as a real.
- Do not add a new `ValueProblem` variant, because the UI already handles
  `wrongType`.
- The field stays editable if its mode allows. Saving the object without
  touching it keeps the value (the save milestone validates only changed
  fields).
- Test `non_finite_real_is_wrong_type`: insert `9e999` (= `+Inf`) into
  `book.price` and read it back as `{"type":"real","value":"inf"}` with
  `WrongType`.

## Files

Create `crates/fm-dbmanager/src/session/{mod.rs, read.rs, sql.rs, key.rs,
version.rs, label.rs (or reuse model/labels.rs), tests/…}` and
`src/config/load.rs`. Edit
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
  search `"AS"` and `"as"` both match Castellan and Tomas (case-insensitive); a search containing `%` or `_`
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
- `only: Some(key)` returns exactly that author (with `linked`,
  `link_values`, `link_version` as in the full listing), `total` 1 and the
  relation's full `linked_total`; for a deleted author it returns an empty
  page with `total` 0; `search`/`offset`/`linked_only` are ignored with it.
- With **no config**, every table is editable and listed except detected
  junctions.
- `version` changes when any column changes, including a hidden one, and is
  stable otherwise.
- `UnknownTable` for a hidden or missing table; `UnknownObject` for a wrong
  key; `UnknownRelation` for a relation of another table.
- Altering the schema from a second connection makes the next read fail with
  `SchemaChanged`. Editing the config file makes `config_changed()` true.
- `LoadedConfig::load`: a missing file is `Missing` (editable defaults, hash
  `"missing"`); an unreadable file (a directory at the path, invalid UTF-8),
  a syntax error and `version = 2` are `FailClosed` with a reason, and the
  session is all read-only; a valid config and one with only entry-level
  errors are `Loaded`. `config_fail_closed()` follows the status.
  `changed_on_disk()` turns true when the file is created, edited, deleted or
  made unreadable.
- `set_config` with a new `LoadedConfig` re-resolves the model in place:
  `table_views()` reflects the new modes and bounds, the database is not
  reopened, and reads keep working.
- `publisher` (pickable in `example.toml`) is listable, openable (all fields
  read-only) and appears in `book`'s FK picker list.
- **Awkward identifiers:** a schema whose tables and columns are named `a"b`,
  `select`, `with space` and `ünï`, with FKs and a junction between them, is
  listed, searched, opened, and checklisted correctly.
- Only `src/session/sql.rs` assembles SQL text containing names (reviewer
  checks).
- Generated types current, no `bigint`; fmt, clippy pedantic, `cargo test
  --workspace` pass.

## Tests proving completion

Also: `non_finite_real_is_wrong_type` (from the M4 review).

`src/session/tests/`: `list_orders_and_labels`, `list_paging_is_complete`,
`list_search_case_insensitive`, `list_search_escapes_wildcards`,
`list_referencing`, `object_fields_and_links`, `object_hides_hidden`,
`value_problems`, `duration_values_and_labels`, `relation_counts`,
`m2m_options_all_and_linked`, `m2m_options_new_object`,
`m2m_options_link_values`, `m2m_options_only`, `link_version_tracks_junction_row`,
`config_fail_closed_reason`, `loaded_config_statuses`, `set_config_reresolves`,
`pickable_table_reads`,
`no_config_is_editable`, `version_tracks_every_column`,
`unknown_table_object_relation`, `schema_change_detected`,
`config_change_detected`, `unreadable_config_is_fail_closed`,
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
- Text that is not valid UTF-8 in the database is shown lossily. Editing such
  a field replaces it, and the version check still protects concurrent
  edits. **Lookups are lossy too:** a key or FK value stored as non-UTF-8
  text crosses as lossy text and is bound back as that text, so it no longer
  matches. The object then cannot be opened (`UnknownObject`), and links to it
  show as dangling. This is accepted for v1 (see Review round 1, R3); it must
  fail gracefully and never panic.

## Review round 1

The reviewer requested changes. Fix the following in one run.

**R1 — the fallback label must not show a hidden identity value.**

`session/label.rs` (around lines 36–37) renders an empty or missing label as
`"<display name> #<key>"` from the key text. An identity column can be
hidden: a hidden rowid alias (`id INTEGER PRIMARY KEY`), or a hidden
primary-key column of a `WITHOUT ROWID` table. Its value then appears as
display text in lists, FK link labels, pickers and checklist options, which
breaks "hidden never reaches the UI".

**Decision: keep hiding an identity column legal, and change the label.**
Hiding ids is a natural thing to configure, and the fallback is the only
place that leaks them.

- When **any identity column is hidden**, the fallback label (empty or
  missing template) is the table's **display name alone**, e.g. `"Authors"`.
  This covers the alias column of a rowid table, and any primary-key column
  of a `WITHOUT ROWID` table.
  - Objects stay distinguishable through their keys. The UI never shows
    keys.
  - The config author is told to add a `label` (doc change below).
- Without a hidden identity column, the fallback is unchanged:
  `"<display name> #<key>"`. A rowid table without an alias always shows its
  rowid, because the rowid is not a column and cannot be hidden.
- Update `docs/dbmanager-config.md`, "Labels": state this rule, and
  recommend a `label` for any table whose id is hidden.

Tests:

- `fallback_label_hides_hidden_rowid_alias`: `t(id INTEGER PRIMARY KEY, x)`
  with `id` hidden, no label, and `x` NULL. Check list items, the object
  label, an FK link label from another table, a picker, and an m2m option.
  Each label is exactly the display name, and no label contains the id.
- `fallback_label_hides_hidden_without_rowid_pk`: the same for a
  `WITHOUT ROWID` table with a hidden text PK.
- `fallback_label_shows_visible_key`: unchanged behaviour when the key is
  visible.

**R2 — record: `ObjectKey` may carry identity values.**

An `ObjectKey` may contain hidden identity values, either the rowid or
primary-key values. That is accepted: it is opaque technical data the UI
echoes back and never displays.

Add a doc comment on `ObjectKey`:

> Opaque. May carry hidden identity values. Never render it.

**R3 — limits of keys, accepted for v1.**

- **BLOB primary-key values.** Such rows show in lists, but `object` cannot
  open them: `DbValue::Blob` carries only a size, so the key cannot
  round-trip, and the result is `UnknownObject`. Accepted for v1.
  - Test `blob_key_row_is_listed_but_unknown`: it must not panic, and the
    list labels must not contain blob bytes.
- **Non-UTF-8 text keys and FK values** are lossy (see Risks), also accepted
  for v1.
  - Test `non_utf8_text_key_fails_gracefully`: open gives `UnknownObject`, a
    link to it shows `DanglingReference`, and nothing panics.

**R4 — corrected acceptance line.** The search criterion now uses `"AS"` and
`"as"` (Castellan, Tomas). The example has no author matching `"ar"`.

### Accepted implementer choices (record only; do not change)

- Reads use one read transaction after the `schema_version` check.
  `BUSY`/`LOCKED` map to `Busy`.
- `LIKE` escaping, as implemented.
- The config hash sentinels and `changed_on_disk` semantics.
- The `report()` accessor on `LoadedConfig`.
- `Model::table_at`.
- A referencing list for a row that does not exist gives `UnknownObject`.
- Many-to-many counts include only other-side rows that exist.

### Acceptance criteria (review round 1)

- These tests pass, together with every existing M6 test:
  - `fallback_label_hides_hidden_rowid_alias`;
  - `fallback_label_hides_hidden_without_rowid_pk`;
  - `fallback_label_shows_visible_key`;
  - `blob_key_row_is_listed_but_unknown`;
  - `non_utf8_text_key_fails_gracefully`.
- No label, value, link or option anywhere contains a hidden column's value.
  Assert on the serialized JSON in the two hidden-key tests.
- `ObjectKey` carries the doc comment.
- `docs/dbmanager-config.md`, "Labels", states the hidden-identity fallback.
- No generated type changes shape, except for doc comments. Generated types
  are current.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D
  warnings` and `cargo test --workspace` pass.
- The diff goes back to the reviewer.

