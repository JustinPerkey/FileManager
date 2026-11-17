# M5 — The model: overlay the config on the schema

Status: awaiting approval (revised 2026-11-16: editable defaults, durations,
link fields; then the `pickable` table mode)
Project: dbmanager   Depends on: M2 (landed), M3 (landed), M4 (landed)

## Goal

Combine the introspected schema and the parsed config into a resolved
`Model`. The model holds every table's mode, every field's kind, mode and
rule, labels, ordering, search columns, FK links, reverse relations, and
many-to-many relations with their link fields. It carries a diagnostic for
every mismatch and exposes the per-table view types the UI will render.

## Context

The Database Manager (`dbmanager`) shows a SQLite database as tables of
objects. The structure comes from the database, and the TOML config
overlays modes, bounds, durations, labels and many-to-many relations.
**Anything the config does not mention is editable**: the built-in defaults
are `editable` for tables, fields and relations, `allow_create = true` and
`allow_delete = false`. The forced read-only rules still apply.

**The rules to implement are in `docs/dbmanager-config.md`** (written by
M3). Implement these sections, plus every row of "Diagnostics" that needs
the schema:

- "Defaults", "Tables", "Fields";
- "Kinds", "Durations", "Forced read-only";
- "Labels", "Foreign keys and reverse relations", "Many-to-many".

Where this task plan adds detail, it agrees with the doc. If you find a
conflict, stop and report it rather than choosing.

Builds on:

- M2, `crates/fm-dbmanager/src/schema/`:
  - `introspect`, `Schema`, `Table`;
  - `Column`, with `affinity`, `declared_type`, `not_null`, `default_sql`,
    `generated`, `pk_position` and `is_rowid_alias`;
  - `Identity`, `ForeignKey` (`from`, `to_table`, `to_columns`, `on_delete`,
    `target`), `unique_keys`, `has_triggers`, `skipped`;
  - `Schema::table`, `Table::column`, `Schema::referencing`;
  - `Ident`, the only way a name reaches SQL.
- M3, `src/config/`:
  - `parse`, `ConfigReport`, `Config`;
  - `Defaults { table_mode, field_mode, relation_mode }`;
  - `TableEntry`/`FieldEntry`/`M2mEntry` (`Valid` | `Rejected { hidden }`; a
    rejected table also carries `fields: Vec<FieldEntry>`),
    with explicit-vs-inherited flags on `mode`, `allow_create` and
    `allow_delete`;
  - `Config::fail_closed()` (all defaults read-only, used after a syntax
    error or an unsupported `version`);
  - `LabelTemplate`, `Bound` (`Int` | `Float` | `Duration(text)`),
    `AllowedValues`;
  - `TableMode`, `FieldMode`, `RelationMode`, `DurationUnit`;
  - `ConfigDiagnostic`, and entry positions.
- M4: `src/value.rs` `DbValue`, plus `src/rules/`, which provides:
  - `FieldKind` (incl. `Duration`), `FieldRule` (incl. `unit`) and
    `AllowedValue`;
  - `duration::{parse, format}`.

Rules that bind this task:

- No tauri. fmt and clippy pedantic clean, with only a scoped `#[expect]`.
  Tests run in temp dirs, on databases built from SQL text or from
  `examples/dbmanager/schema.sql`.
- **Fail closed.** An error never leaves anything more editable than its
  entry would have.
  - A rejected or erroneous field is read-only, even though the default is
    editable. It is hidden only if its own valid entry asked for hidden.
  - A rejected table entry makes the table read-only (or hidden, if it
    validly asked for hidden). **Its field entries, kept in
    `TableEntry::Rejected.fields`, still hide their fields:** a field whose
    entry (valid or rejected) asks for `hidden` stays hidden, and every other
    field of the table is read-only. A table-level typo must never reveal a
    hidden column.
  - Diagnostics that narrow the defaults (an unknown top-level key, a
    wrong-typed `defaults`/`tables`/`many_to_many`) arrive as read-only
    `Defaults`. Apply them like any defaults.
  - A rejected relation entry makes the relation read-only.
- **Hidden never leaks.**
  - Hidden fields are absent from every view.
  - Hidden tables are absent from the table list and are never link targets
    (`navigable: false`).
  - Hidden tables never contribute reverse relations or checklist sides.
- Boundary types:
  - derive `Clone, Debug, PartialEq, Serialize, ts_rs::TS`, with camelCase;
  - no `bigint`;
  - no name already used in `apps/desktop/src/lib/generated/`;
  - exported through `EXPORTERS` in
    `apps/desktop/src-tauri/src/generated_types.rs`, then regenerated.

### Resolution details

- **Kind inference** (`rules::infer_kind(&Column, strict: bool) ->
  FieldKind`) applies the doc's seven ordered rules. There is no date rule:
  NUMERIC affinity is `real`. `type` then overrides the kind according to
  the doc's compatibility table, STRICT tables included. `duration` comes
  only from `type`.
- **Field rule:**
  - `nullable = config.nullable.unwrap_or(!column.not_null)`.
  - Bounds are applied when they are valid for the kind:
    - integer bounds must be TOML integers;
    - real bounds accept both forms and are stored as canonical decimal
      text;
    - duration bounds accept a duration string or an integer count, and are
      parsed with the field's unit (default `seconds`). Not a whole number of
      units is an error. They are stored as canonical duration text;
    - lengths and pattern apply only to text;
    - `allowed` applies only to integer and text, with matching value types,
      and labels default to the value text.
  - `empty_is_null` defaults to `nullable && kind == Text`. Setting it on a
    non-nullable or non-text field is an error.
  - `unit` is `Some` iff the kind is duration.
  - Read-only fields still get a rule (kind, unit, nullable, any valid
    bounds), which the read layer later uses to flag legacy values.
- **Table modes:** the valid entry's `mode`. Otherwise not listed, for a
  junction used as many-to-many. Otherwise `defaults.table_mode`. A
  rejected entry makes the table read-only (or hidden).
- **Pickable tables** (`mode = "pickable"`, a lookup table the human asked
  for): not in the table list, but otherwise visible. In detail:
  - every field is read-only (an explicit `editable`/`create-only` field entry
    is a warning, as in a read-only table); `can_create` and `can_delete` are
    false (an explicit `allow_create`/`allow_delete` is a warning);
  - it **is** a valid FK target: FK fields into it are editable (picking from
    it) and **navigable**; following such a link opens the object
    **read-only**. Reason: the user must be able to see what they picked, and
    a lookup table hidden from the list must not become editable by the back
    door of a link;
  - it is a valid other side of a many-to-many checklist (the checklist shows
    on the other table's objects); on a pickable object's own (read-only) view,
    its checklists and reverse relations show read-only;
  - it is **not** hidden: its rows are labelled in delete impact like any
    visible table;
  - `mode = "pickable"` on a junction used as many-to-many is a warning and
    means "not listed".
  - `TableView` is produced for it (the UI needs its fields to show an object
    opened through a link) with `mode: "pickable"`; the UI leaves it out of
    the table list by that mode. `Model::table(name)` returns it.
- **Field modes:**
  - The entry's `mode`, otherwise `defaults.field_mode`.
  - A read-only table makes every field read-only. An explicit
    `editable`/`create-only` there is a warning.
  - Forced read-only follows the doc: an explicit request is an error, an
    inherited one is silently narrowed.
  - "Rowid alias" always means `Column::is_rowid_alias` from M2. That
    excludes `INTEGER PRIMARY KEY DESC`, which SQLite does not treat as an
    alias.
    - Such a column is an ordinary primary-key column: nullable unless
      declared `NOT NULL`, and create-only by inheritance.
    - Its table's identity is the rowid.
    - Its `is_key` is true, because it is part of the primary key, but it is
      not the identity.
  - **A primary-key column that is not a rowid alias:** an inherited
    `editable` becomes `create-only`; an explicit `editable` is an error
    (suggest `create-only`); an explicit `create-only` is kept.
  - The effective mode is `editable`, `read-only`, `create-only` or
    `hidden`.
- **Create/delete:**
  - `can_create = table editable && allow_create && no blocking column`,
    where `allow_create` defaults to `true`. A blocking column is a `NOT
    NULL` column that is neither editable nor create-only, has no default,
    and is not a rowid alias.
  - If a column blocks, the diagnostic names it: an error when
    `allow_create` was explicit, a warning when it was inherited.
  - `can_delete = table editable && allow_delete`, where `allow_delete`
    defaults to `false`.
  - An explicit `allow_create`/`allow_delete` on a non-editable table is a
    warning.
- **Column defaults:** `default_sql` is parsed as a literal when it is a
  signed integer, a real, a single-quoted string (`''` escapes), `NULL`,
  `TRUE` or `FALSE` (→ `1`/`0`). For a duration field, an integer literal
  becomes `DbValue::Duration(format(n, unit))`. Anything else
  (`CURRENT_TIMESTAMP`, `(expr)`) means "has a default, value unknown".
- **Labels:**
  - Resolve template placeholders to column indexes. An unknown or hidden
    column is an error, and the default label is used.
  - The default label follows the doc.
  - Search columns are the label's columns **minus durations**.
  - `order_by` is resolved. An unknown or hidden column is an error, and the
    default order is used.
  - The default order is the label columns ascending, then the identity.
- **Foreign keys:**
  - A single-column FK whose `target` exists becomes a link field
    (`ForeignKeyView`). It is navigable iff the target table is not hidden,
    and editable only if the target is not hidden.
  - For a multi-column FK, each column is read-only, and only the first
    column carries the link.
  - A FK with a missing target is a warning, and the column is a plain
    field.
  - A FK whose target is not the parent's primary key or a unique key
    (`FkTarget.is_key == false`, compared as sets) is a **warning**, and the
    column is a plain read-only field.
    - The warning says: "the foreign key from `t.c` to `p(x)` does not
      reference a primary or unique key; SQLite will refuse changes that
      touch it".
    - The table's `can_create` and `can_delete` are unaffected. SQLite
      reports such writes itself as `ConstraintFailed`.
- **Reverse relations** of table T: every FK from a non-hidden table (T
  itself included, for self-references) to T, except FKs of junctions used
  as many-to-many.
- **Many-to-many:** detection and declaration work exactly as the doc says.
  Only declared junctions may have extra columns. Self-referential
  junctions are not many-to-many, and declaring one is a warning.
  - **Relation mode:** the entry's `mode`, otherwise
    `defaults.relation_mode`. A rejected entry makes the relation read-only.
  - **Link fields:** every junction column other than the two relation
    columns. Each is resolved exactly like an ordinary field of the junction
    table, from `[tables.<junction>.fields.*]` and `defaults.field_mode`,
    with the forced read-only rules. Extra FK columns are link fields with a
    `ForeignKeyView`.

    A junction table entry that was rejected makes every link field
    read-only. A junction listed as `read-only` makes them read-only too.
    The default (not listed) does not.
  - **Adding links:** possible iff every `NOT NULL` junction column without a
    default is a relation column, or an editable/create-only link field.
    Otherwise the relation is read-only, with an error if its `mode` was
    explicit and a warning if it was inherited.
  - **Per side:** take side S of relation R, with other side O.
    - The checklist exists iff O is not hidden and R's mode is not hidden.
    - It is editable iff R is effectively `editable`, S's table is editable,
      and links can be added.
    - Link fields are shown on both sides. Their modes are as resolved, but
      they are read-only on a side whose checklist is read-only.
- **Diagnostics** combine the config diagnostics (from `ConfigReport`) with
  the schema-dependent ones.
  - Each has `table`/`column` set, plus the line/col of its config entry
    when it comes from one.
  - These are warnings: unknown tables, fields and junctions, and names of
    skipped tables (views, virtual, shadow, internal, no identity).
  - Sort by line (config first), then table, then column.
- **Relation ids** are strings, opaque to the UI and stable for a given
  schema: `fk:<table>:<fk id>` and `m2m:<junction>`.

### Types

The internal shapes are up to you, but M6–M9 need all of this per table:

- the schema index and `Identity`;
- the visible fields, with effective modes and rules;
- the label template as column indexes, plus the search and order columns;
- FK fields with their target table and target columns;
- the reverse relations;
- the many-to-many relations per side: junction table, this side's column,
  the other side's column, the other table, the mode on this side, whether
  links can be added, the resolved link fields (hidden ones included,
  flagged), and the junction's identity.

```rust
pub fn resolve(schema: &Schema, config: &ConfigReport) -> Model;
impl Model {
    pub fn diagnostics(&self) -> &[ConfigDiagnostic];
    pub fn table_views(&self) -> Vec<TableView>;   // listed and pickable tables, schema order
    pub fn table(&self, name: &str) -> Option<&ResolvedTable>;  // exact SQL name; hidden/unlisted junction → None
}
```

The boundary views are exported:

```rust
pub struct TableView {
    pub name: String, pub display_name: String, pub description: Option<String>,
    pub object_name: Option<String>,     // config `object_name`; None when not configured (no guessing)
    pub mode: TableMode,                 // editable | read-only | pickable (never hidden here);
                                         // pickable tables are not shown in the table list
    pub can_create: bool, pub can_delete: bool, pub searchable: bool,
    pub has_triggers: bool,
    pub fields: Vec<FieldView>,          // display order (field_order, then column order)
    pub referenced_by: Vec<ReverseRelationView>,
    pub many_to_many: Vec<M2mView>,
}
pub struct FieldView {
    pub column: String, pub display_name: String, pub help: Option<String>,
    pub mode: FieldMode,                 // editable | read-only | create-only (never hidden)
    pub kind: FieldKind, pub declared_type: String, pub rule: FieldRule,
    pub is_key: bool,                    // part of the identity / primary key
    pub required_on_create: bool,        // NOT NULL, no default, not a rowid alias
    pub has_default: bool, pub default_value: Option<DbValue>,
    pub multiline: bool,
    pub foreign_key: Option<ForeignKeyView>,
}
pub struct ForeignKeyView { pub table: String, pub table_display_name: Option<String>, pub navigable: bool }
pub struct ReverseRelationView { pub relation: String, pub table: String, pub table_display_name: String, pub via: String }
pub struct M2mView {
    pub relation: String, pub other_table: String, pub heading: String,
    pub mode: RelationMode,              // this side: editable | read-only
    pub link_fields: Vec<FieldView>,     // visible link fields, junction column order;
                                         // required_on_create means "required when adding a link"
}
```

`ForeignKeyView.table_display_name` is `None` for a hidden target. `via` is
the referencing field's display name(s), joined by `, `.

Append `TableView` to `EXPORTERS` (its referenced types follow), then
regenerate.

## Files

Create `crates/fm-dbmanager/src/model/{mod.rs, resolve.rs, kinds.rs (or in
rules/), labels.rs, m2m.rs, views.rs, tests.rs}`.

Edit:

- `src/lib.rs`;
- `src/rules/` (for `infer_kind`, if it lives there);
- `apps/desktop/src-tauri/src/generated_types.rs`, and the generated files;
- `CLAUDE.md` (layout: `src/model/`).

Do not touch the frontend outside `lib/generated/`, other crates, or
`docs/plans/`. If the doc needs a correction, make it in
`docs/dbmanager-config.md` and say so in the hand-off to the reviewer.

## Acceptance criteria

**`examples/dbmanager/schema.sql` + `examples/dbmanager/example.toml`
resolve with zero diagnostics, and:**

- Listing and hiding:
  - `audit_log`, `book_genre` and `contribution` are absent from
    `table_views()`;
  - `author.internal_note` is absent.
- Field modes:
  - `book.revision` is read-only;
  - `book.isbn` is create-only;
  - `book.id` is read-only with `is_key`;
  - `book.title`, `book.pages` and `member.name` are editable by default.
- Kinds:
  - `book.in_print` and `loan.returned` are bool;
  - `book.audio_length` is a duration with unit seconds and rule bounds
    `"1m"`/`"100h"`;
  - `loan.loan_period` is a duration whose default value is `"14d"`.
- Links:
  - `book.author_id` links to `author` and is navigable;
  - `publisher` is a **pickable** table (`TableView.mode == "pickable"`,
    every field read-only), and `book.publisher_id` is still editable
    (picking from a pickable table is allowed) and navigable.
- Many-to-many:
  - `book` and `genre` each have an editable `M2mView` for `m2m:book_genre`,
    with headings "Genres" and "Books" and no link fields;
  - `book` and `author` each have an editable `M2mView` for
    `m2m:contribution`, with link fields `role` (enum) and `position`
    (integer 1–99), and `role` has a default;
  - `author` has a reverse relation from `book` and none from
    `contribution`;
  - `book` has no reverse relation from `book_genre`.
- Labels and permissions:
  - genre's label is `{name}`;
  - `author`'s `object_name` is `"author"`; a table without the key (for
    example any table with no config) has `object_name: None`;
  - `can_create` is true for author, book, genre, member and loan, and false
    for publisher;
  - `can_delete` is true for author, book, member and loan, and false for
    genre.

**With no config** (`ConfigReport` of an empty string):

- every table is editable;
- fields are editable except the forced ones, and PK columns are
  create-only;
- `can_create` is true where no column blocks it, and `can_delete` is false;
- `book_genre` is detected, unlisted, with editable checklists on both
  sides;
- `contribution` is not detected (extra columns): it is an ordinary editable
  table, with reverse relations on `book` and `author`.

**Fail closed for the whole config:**

- A `ConfigReport` from text with a TOML syntax error (which M3 turns into
  `Config::fail_closed()`) resolves with every table, field and relation
  read-only, and hidden nothing. The same holds for `version = 2`.

**Diagnostics:**

- With `[defaults] table_mode = "read-only"`, everything is read-only
  unless stated, and explicit field `editable` entries in read-only tables
  warn.
- There is one test per diagnostics-table row that needs the schema,
  asserting severity, position (or table/column) and effect. That includes
  explicit-vs-inherited create blocking and link-adding blocking.

**Kinds and forced read-only:**

- Kind inference covers all seven rules and the `type` compatibility table,
  STRICT tables and `duration` included.
- An explicit `editable` on a rowid alias, a generated column, a blob, an
  untyped column, a multi-column FK or a FK into a hidden table is an error,
  and the field is read-only. The same through the editable default is
  silent and read-only. An inherited `editable` on a text PK becomes
  `create-only`.

**Fail closed and link fields:**

- A table entry with an unknown key leaves that table read-only, although
  the default is editable. A field entry with an error leaves that field
  read-only.
- A link field that is `NOT NULL` without a default and `hidden` makes the
  relation read-only: an error if the relation `mode` was explicit, a
  warning otherwise.
- A link field with a duration `type` resolves like any duration field.
- A self-referential junction is not many-to-many.
- `enabled = false` makes a junction an ordinary visible table with reverse
  relations.
- A hidden side removes the checklist from the other side.
- A relation with `mode = "read-only"` gives read-only checklists and link
  fields on both sides.

**Build:**

- Generated types are current, with no `bigint`.
- fmt, clippy pedantic and `cargo test --workspace` pass.

## Tests proving completion

`src/model/tests.rs`:

- Defaults and configs:
  - `example_resolves_clean`
  - `no_config_is_editable`
  - `pickable_table_semantics` (read-only fields, no create/delete, valid FK
    target and navigable, valid m2m other side, warnings for explicit
    editable fields and allow flags, error in `[defaults]`, warning on a
    junction)
  - `syntax_error_config_is_read_only`
  - `read_only_default_policy`
- Kinds:
  - `kind_inference_rules`
  - `type_override_compatibility`
  - `strict_table_kinds`
  - `duration_fields_resolve`
  - `duration_bound_precision`
- Modes and fail closed:
  - `forced_read_only_explicit_is_error`
  - `forced_read_only_inherited_is_silent`
  - `primary_key_inherited_becomes_create_only`
  - `fail_closed_rejected_table`
  - `rejected_table_keeps_hidden_fields`: with `[tables.author]` carrying an
    unknown key and `internal_note` set to `mode = "hidden"`, `author` is
    read-only, `internal_note` is absent from `TableView.fields`, and the
    other fields are read-only
  - `unknown_top_level_key_is_read_only_policy`: `[table.secrets]` makes
    every unconfigured table read-only
  - `fail_closed_rejected_field`
  - `field_in_read_only_table_warns`
- Rules:
  - `nullable_cannot_loosen`
  - `bounds_must_fit_kind`
  - `fk_field_bounds_rejected`
- Create and defaults:
  - `allow_create_blocked_explicit_error`
  - `allow_create_blocked_inherited_warning`
  - `delete_off_by_default`
  - `default_literals_parsed`
- Labels and ordering:
  - `labels_default_and_template`
  - `label_hidden_column_rejected`
  - `search_skips_durations`
  - `order_by_resolution`
- Names and links:
  - `unknown_table_and_field_warn`
  - `skipped_table_named_warns`
  - `fk_missing_target_warns`
  - `fk_target_not_a_key_warns`
  - `integer_primary_key_desc_is_not_an_alias` (create-only PK column;
    identity is the rowid)
  - `reverse_relations`
- Many-to-many:
  - `m2m_detected`
  - `m2m_declared_with_link_fields`
  - `m2m_link_field_blocks_adding`
  - `m2m_self_reference_not_supported`
  - `m2m_disabled`
  - `m2m_hidden_side`
  - `m2m_read_only_relation`
- `diagnostics_sorted`

Run `cargo test -p fm-dbmanager`, then regenerate, then
`cargo test --workspace`, then fmt and clippy per `CLAUDE.md`.

## Out of scope

- Reading rows, the labels of actual objects, list queries: M6.
- Writes: M7–M9.
- The Tauri module: M10.

## Risks

- The doc and this plan are the spec. Report any ambiguity you find here
  rather than deciding it silently.
- Name matching between config and schema is exact first, then ASCII
  case-insensitive (SQLite's rule), via `Schema::table`/`Table::column`.
- Editable defaults make test expectations easy to get backwards. Each test
  states its config explicitly.
