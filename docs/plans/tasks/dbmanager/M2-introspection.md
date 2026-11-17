# M2 — Schema introspection and identifier quoting

Status: implemented; the reviewer requested changes. **Run "Review round 1"
(end of this plan)** on this task plan; the rules below are already corrected
for it. (Approved 2026-11-17.)
Project: dbmanager   Depends on: M1 (landed)

## Goal

Read a SQLite database's structure (object tables, columns, keys, foreign
keys, unique keys, triggers, identity) into a `Schema` model, and introduce
`Ident`, the only way a table or column name can reach SQL text.

## Context

The Database Manager (`dbmanager`) shows a user's SQLite tables as lists of
objects and their rows as forms. Table structure, column types and foreign
keys come **from the database**; a TOML config (M3) is later overlaid on this
schema (M5). This milestone only reads structure; it reads no user rows.

Builds on M1: `crates/fm-dbmanager/src/db.rs` (`Database::open`, `conn()`,
`schema_version()`), with `foreign_keys = ON` and `trusted_schema = OFF`.

Rules that bind this task:

- **No identifier is ever interpolated into SQL by introspection.** Use the
  table-valued pragma functions with bound parameters only:
  `SELECT … FROM pragma_table_list`, `pragma_table_xinfo(?1)`,
  `pragma_foreign_key_list(?1)`, `pragma_index_list(?1)`,
  `pragma_index_info(?1)`, and `SELECT type, name, tbl_name FROM
  sqlite_schema`.
- **`Ident`** (`src/schema/ident.rs`): `pub struct Ident(String)` with a
  `pub(crate)` constructor called only from `src/schema/`. Public API:
  `as_str()`, `quoted() -> String` (wrap in `"`, double every embedded `"`),
  `Display` (the raw name). No `From<&str>`, no `Deserialize`, no public
  constructor. Every later module that builds SQL takes `&Ident`.
- The crate rules: no tauri, `[lints] workspace = true`, fmt/clippy pedantic
  clean, tests in temp dirs (build each test database in a temp dir from SQL
  text).

### What is introspected

`Schema { schema_version: i64, tables: Vec<Table>, skipped: Vec<Skipped> }`,
tables in `sqlite_schema` order.

**Object tables:** rows of `pragma_table_list` for schema `main` with `type =
'table'`, name not starting with `sqlite_`. Everything else (`view`,
`virtual`, `shadow`, other schemas) goes to `skipped` with a
`SkipReason::{View, Virtual, Shadow, Internal, NoIdentity}` so a later
milestone can report a config that names one.

`Table`:

- `name: Ident`
- `without_rowid: bool`, `strict: bool` (from `pragma_table_list`'s `wr`,
  `strict`)
- `columns: Vec<Column>` in declaration order (from `pragma_table_xinfo`,
  which includes generated columns)
- `primary_key: Vec<usize>` column indexes ordered by `pk` position
- `identity: Identity`
- `foreign_keys: Vec<ForeignKey>`
- `unique_keys: Vec<Vec<usize>>` — from `pragma_index_list` rows with
  `unique = 1` and `partial = 0`, columns from `pragma_index_info` (skip any
  index containing an expression, `cid < 0`); the primary key counts as one
- `has_triggers: bool` — any `sqlite_schema` row with `type = 'trigger'` and
  `tbl_name` equal to this table

`Column`:

- `name: Ident`, `declared_type: String` (as written, may be empty)
- `affinity: Affinity` — `Integer | Text | Blob | Real | Numeric`, by SQLite's
  rules in this order on the upper-cased declared type: contains `INT` →
  Integer; contains `CHAR`, `CLOB` or `TEXT` → Text; contains `BLOB` or is
  empty → Blob; contains `REAL`, `FLOA` or `DOUB` → Real; otherwise Numeric.
  (STRICT `ANY` falls to Numeric by these rules; record `declared_type` so M5
  can tell.)
- `not_null: bool`, `default_sql: Option<String>` (`dflt_value` text as
  stored)
- `generated: bool` — `hidden` is 2 or 3 in `pragma_table_xinfo`
- `pk_position: Option<u32>`
- `is_rowid_alias: bool`: true only when all of these hold:
  - it is a rowid table;
  - the primary key is a single column;
  - the declared type is exactly `INTEGER` (case-insensitive, trimmed);
  - **no** row of `pragma_index_list(?1, 'main')` has `origin = 'pk'`.

  The last condition is SQLite's documented exception: `INTEGER PRIMARY KEY
  DESC` is **not** a rowid alias. SQLite gives such a table an automatic
  index (`sqlite_autoindex_<t>_1`, origin `pk`). The column is an ordinary,
  nullable column: `insert into t(x) values(1)` leaves it NULL, with rowid 1.
  Such a table's identity is `Rowid { accessor, alias_column: None }`.

`Identity`:

- `Rowid { accessor: Ident, alias_column: Option<usize> }` for rowid tables.
  `accessor` is the alias column's name if there is one; otherwise the first of
  `rowid`, `_rowid_`, `oid` that no column is named (case-insensitive). If all
  three are shadowed and there is no alias, the table goes to `skipped` with
  `NoIdentity` instead of `tables`. (Build the accessor `Ident` through the
  same `pub(crate)` constructor.)
- `PrimaryKey { columns: Vec<usize> }` for `WITHOUT ROWID` tables.

`ForeignKey` (group `pragma_foreign_key_list` rows by `id`, order by `seq`):

- `id: i64`
- `from: Vec<usize>` (column indexes in this table)
- `to_table: String` (as written; may name a table that does not exist)
- `to_columns: Vec<String>`; when the pragma reports `to` as NULL the FK
  references the parent's primary key: resolve to the parent's PK column
  names if the parent exists
- `on_delete`, `on_update: FkAction` — `NoAction | Restrict | SetNull |
  SetDefault | Cascade` (parse the pragma's text)
- `target: Option<FkTarget>`: `Some { table: usize, columns: Vec<usize>,
  is_key: bool }` when the parent table exists in `tables` and every
  referenced column exists in it, and `None` otherwise (a later milestone
  reports it).
  - `is_key` is true when the referenced columns, **as a set**, equal the
    parent's primary key or one of its `unique_keys`.
  - When it is false, SQLite raises "foreign key mismatch" on any write
    that touches the FK, so a later milestone must report it.

`Schema` helpers: `table(&self, name: &str) -> Option<&Table>` (exact match
first, then ASCII case-insensitive, as SQLite resolves names),
`Table::column(&self, name: &str) -> Option<usize>` (same matching), and
`referencing(&self, table: usize) -> impl Iterator<Item = (usize, &ForeignKey)>`
(every FK in any table, including hidden ones later, whose target is
`table`).

Entry point: `pub fn introspect(db: &Database) -> rusqlite::Result<Schema>`
in `src/schema/mod.rs`.

## Files

Create `crates/fm-dbmanager/src/schema/{mod.rs, model.rs, ident.rs}` and
tests (`src/schema/tests.rs`). Edit `src/lib.rs` to add `pub mod schema;`.

Do not touch the shell, the frontend, other crates, or `docs/plans/`.

## Acceptance criteria

- Introspecting a database with a rowid table with `INTEGER PRIMARY KEY`, a
  rowid table without one, a `WITHOUT ROWID` table with a composite PK, a
  STRICT table, a view, an FTS5 virtual table (the bundled build enables
  FTS5; its shadow tables must land in `skipped` as `Shadow`), and a
  generated column
  yields exactly the object tables, with views/virtual/shadow tables in
  `skipped`.
- Affinity follows the five rules for `INT`, `BIGINT`, `VARCHAR(20)`,
  `CLOB`, `BLOB`, empty, `DOUBLE PRECISION`, `FLOAT`, `NUMERIC`,
  `DECIMAL(10,2)`, `BOOLEAN`, `DATETIME`.
- `is_rowid_alias` is true for `id INTEGER PRIMARY KEY`, and false for:
  - `id INT PRIMARY KEY`;
  - **`id INTEGER PRIMARY KEY DESC`**, whose identity is then
    `Rowid { alias_column: None }` with accessor `rowid`;
  - a composite PK;
  - any column of a `WITHOUT ROWID` table.
- A table with columns named `rowid`, `_rowid_` and `oid` and no alias is in
  `skipped` with `NoIdentity`; with only `rowid` shadowed, the accessor is
  `_rowid_`.
- FKs: multi-column FK grouped and ordered; `REFERENCES parent` without
  columns resolves to the parent's PK; ON DELETE actions parsed; a FK to a
  missing table has `target: None`.
- Unique keys include the PK and plain unique indexes, exclude partial and
  expression indexes.
- `has_triggers` is set only on tables with triggers.
- **Quoting:** tables and columns named `a"b`, `select`, `with space`,
  `ünï`, and `"` alone introspect correctly, and `Ident::quoted()` text
  round-trips: `SELECT count(*) FROM {quoted}` succeeds for each.
- No `format!` builds SQL anywhere in `src/schema/` except inside
  `Ident::quoted()` (reviewer checks; tests use `quoted()` only in test code).
- fmt, clippy pedantic, and `cargo test -p fm-dbmanager` pass.

## Tests proving completion

`src/schema/tests.rs`: `object_tables_and_skipped`, `affinity_rules`,
`rowid_alias_detection`, `identity_accessor_shadowing`,
`foreign_keys_grouped_and_resolved`, `fk_to_missing_table_has_no_target`,
`unique_keys_exclude_partial_and_expression`, `triggers_flagged`,
`awkward_identifiers_round_trip`, `quoted_doubles_quotes`.

Run: `cargo test -p fm-dbmanager`; then the workspace fmt/clippy/test commands
from `CLAUDE.md`.

## Out of scope

- Field kinds (`bool`, `duration`), config overlay, labels, m2m
  detection: M4 and M5.
- Reading or writing rows.
- Boundary (ts-rs) types: the schema model is internal.

## Risks

- `pragma_table_list` needs SQLite ≥ 3.37; bundled SQLite is newer. Do not add
  a fallback for older system libraries (none is used).
- Case-insensitive name matching must use ASCII folding only (SQLite's rule).

## Review round 1

The reviewer requested changes on the first implementation. Fix all of the
following in one run. The rules above already describe the target
behaviour.

**R1 — the `INTEGER PRIMARY KEY DESC` exception** (plan error).

The first implementation marks this column a rowid alias
(`src/schema/mod.rs`, around lines 198–207). That would make the save code
insert NULL keys, and the read code address rows by a nullable, repeatable
column.

- Apply the `is_rowid_alias` rule as now written above. Query
  `SELECT origin FROM pragma_index_list(?1, 'main')` with the table name
  bound. If any row has `origin = 'pk'`, the table has no alias.
- `Identity::Rowid.alias_column` is then `None`. The accessor is chosen as
  for any rowid table without an alias: the first of `rowid`, `_rowid_`,
  `oid` that no column shadows.
- Extend `rowid_alias_detection` with a table `t(id INTEGER PRIMARY KEY DESC,
  x)`. Assert:
  - `is_rowid_alias == false`;
  - `alias_column == None`;
  - the accessor is `rowid`.

  As a sanity check, after `insert into t(x) values(1)`, `select id, rowid
  from t` gives `(NULL, 1)`.
- Add `rowid_alias_asc_explicit`: `id INTEGER PRIMARY KEY ASC` **is** an
  alias.

**R2 — tighten `Ident`'s constructor.**

- Make `Ident::new` `pub(in crate::schema)`. Its doc comment should say
  that no code outside `src/schema/` may construct an `Ident`, and that the
  compiler enforces this.
- Later code (`session/sql.rs` and the reads, writes and delete) only
  receives `&Ident` from the schema and never builds one. That includes the
  rowid accessor, which this milestone already builds inside `src/schema/`.
  The tighter scope therefore costs nothing.

**R3 — FK `from` columns.**

A FK whose `from` column name is not found among the table's columns is
currently dropped silently (`src/schema/mod.rs`, around lines 300–322).
SQLite guarantees the column exists, so this means introspection is wrong,
not the database.

- Keep skipping the FK in release builds.
- Add `debug_assert!(false, …)` with the table and column names.
- Add a comment explaining why.

**R4 — unique keys are sets.**

- Deduplicate `unique_keys` as sets: `UNIQUE(a, b)` and `UNIQUE(b, a)`, or
  a PK plus an equal unique index, yield one entry.
- Store each key sorted by column index.
- Document on the field that keys are sets and their order is not
  meaningful.
- Test `unique_keys_deduplicated_as_sets`.

**R5 — index columns by `cid`.**

The `hidden == 1` skip and the column indexes rely on `pragma_table_xinfo`
row order matching `columns` (around line 170). Index columns by the pragma's
`cid` explicitly, or assert that `cid` equals the position, and add a comment
saying why. Hidden (virtual-table) columns never appear here, because
virtual tables are skipped.

**R6 — FK target is a key.**

- Add `FkTarget.is_key` as specified above.
- Test `fk_target_is_key`:
  - a FK to the parent's PK is a key;
  - a FK to a `UNIQUE` column is a key;
  - a FK to a non-unique column is not a key.

  This is SQLite's "foreign key mismatch" case. Create it with
  `foreign_keys` off, then introspect.
- Reporting it is the model milestone's job, not this one's.

### Acceptance criteria (review round 1)

- `rowid_alias_detection` covers `INTEGER PRIMARY KEY DESC` (no alias,
  accessor `rowid`, `alias_column None`), and `rowid_alias_asc_explicit`
  passes.
- `Ident::new` is `pub(in crate::schema)`, and the crate still builds, so
  nothing outside `src/schema/` constructs one.
- A missing FK `from` column hits a `debug_assert!`. There is no silent drop
  in debug builds.
- `unique_keys_deduplicated_as_sets` passes.
- Columns are indexed by `cid`, or `cid` is asserted.
- `fk_target_is_key` passes.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D
  warnings` and `cargo test --workspace` pass.
- The diff goes back to the reviewer.

