# M8 — Many-to-many link writes with link fields

Status: awaiting approval (new 2026-11-16, split from the save milestone;
revised the same day for `pickable` tables and the config hash on saves)
Project: dbmanager   Depends on: M7 (landed)

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

Extend `Session::save` so a save can also add, edit and remove the object's
many-to-many links, with the junction's extra columns ("link fields"). Link
writes run in the same transaction as the object's field changes and follow
strict conflict rules.

## Context

The Database Manager (`dbmanager`) shows a many-to-many relation on an object
as a checklist of every object of the other table, with the linked ones
ticked. A link is one junction row. A junction may have extra columns, and
the human decided these are **editable in v1, together with the link**:

- ticking an object adds a link, and the user can set its link fields;
- an existing link's fields can be edited;
- unticking removes the link, and its field values with it.

The user stages these changes in the form, and they are committed by an
explicit Save, together with the object's own field changes, in one
transaction.

`docs/dbmanager-config.md` holds the semantics, in the sections
"Many-to-many", "Fields", "Durations" and "Forced read-only". In short:

- Link fields are configured as `[tables.<junction>.fields.<column>]`, with
  the same modes, kinds and bounds as object fields.
- `create-only` link fields are set when the link is added.
- Hidden link fields take their column default on add and are never
  accepted as input.

Builds on:

- M5 `src/model/`. For each many-to-many side, the model has:
  - the junction table and its identity;
  - this side's column, the other side's column, and the other table;
  - whether the side is editable, and whether links can be added;
  - the resolved link fields (`ResolvedField`, hidden ones flagged), and
    `M2mView.link_fields`.
- M6 `src/session/`:
  - `m2m_options`, which reads each link's `link_values` and
    `link_version`;
  - version hashing over junction rows;
  - `ObjectKey` lookups;
  - `src/session/sql.rs`, the only module that assembles SQL text.
- M7 `src/session/save.rs` and `backup.rs`:
  - `Session::save` and `SaveRequest`;
  - `FieldChange` / `FieldInput` (`Value` | `Reference`);
  - the check order (steps 1–12, with step 11 reserved for links);
  - `check_changes(fields, changes, Phase::{Create, Update})`,
    `resolve_references`, `map_constraint` and `ensure_backup`;
  - the `SessionError` struct variants `Validation { fields }` and
    `Conflict { current }`.

Rules that bind this task:

- **SQL safety.** Values are bound parameters. Names reach SQL only via
  `Ident` in `src/session/sql.rs`. Never use `INSERT OR IGNORE` /
  `OR REPLACE`.
- **Destructive operations are never silent.**
  - A removal is an explicit, staged change.
  - A link whose data changed since the UI read it is never overwritten or
    removed silently. That case is a `Conflict`.
  - The backup precedes the session's first write.
  - Any failure rolls the whole save back, the object's field changes
    included.
- No tauri. fmt and clippy pedantic must pass, with only a scoped
  `#[expect]`. Tests run in temp dirs.
- Boundary types: serde camelCase, `ts_rs::TS`, no `bigint`, and no name
  already used in `apps/desktop/src/lib/generated/`. Update `EXPORTERS` if
  needed, then regenerate.

### Contract additions

```rust
pub struct SaveRequest { /* M7 fields */ #[serde(default)] pub links: Vec<LinkChange> }
pub struct LinkChange {
    pub relation: String,                // "m2m:<junction>", a many-to-many side of `table`
    #[serde(default)] pub add: Vec<LinkAdd>,
    #[serde(default)] pub update: Vec<LinkUpdate>,
    #[serde(default)] pub remove: Vec<LinkRemove>,
}
pub struct LinkAdd    { pub other: ObjectKey, #[serde(default)] pub fields: Vec<FieldChange> }
pub struct LinkUpdate { pub other: ObjectKey, pub version: String, pub fields: Vec<FieldChange> }
pub struct LinkRemove { pub other: ObjectKey, pub version: Option<String> }
pub struct LinkRef    { pub relation: String, pub other: ObjectKey }
pub struct LinkError  {
    pub relation: String, pub other: ObjectKey,
    pub fields: Vec<FieldError>,          // issues on link fields
    pub issues: Vec<FieldIssue>,          // issues on the link itself (referenceMissing, notAllowed)
}
```

`other` is the key of the object on the other side. `version` is the
`link_version` from `m2m_options`.

`SessionError` grows:

- `Validation { fields: Vec<FieldError>, links: Vec<LinkError> }`
- `Conflict { current: Box<ObjectView>, link: Option<LinkRef> }`

Update M7's call sites, which pass an empty `links` and `link: None`.
`SaveRequest`, `LinkChange` and the rest derive `Deserialize` as well. The
TS type gets `links?: LinkChange[]` with `#[ts(optional)]`, or a required
array; pick one and document it in the doc comment.

### Checks before writing

These extend M7's steps 3–5. Every issue is collected before anything is
written.

- Each `relation` must be a many-to-many side of `table` with a checklist,
  otherwise `UnknownRelation`. It must be editable on this side, otherwise
  `NotEditable`. Adding needs "links can be added", otherwise `NotEditable`.
- Within one relation, an `other` may appear only once across `add`,
  `update` and `remove`. A repeat gives the link issue `notAllowed`, with the
  message "listed more than once".
- `add.fields` go through `check_changes(link_fields, …, Phase::Create)`:
  - `editable` and `create-only` fields are accepted;
  - hidden fields, unknown fields and the two relation columns get
    `unknownField`;
  - read-only fields get `notEditable`;
  - a `required_on_create` link field that is missing gets `required`;
  - values are validated and normalised by `validate`, durations included.
- `update.fields` go through `Phase::Update`, where only `editable` is
  accepted. An update with no fields is a no-op apart from its version
  check.
- On a create (`key: None`), only `add` is allowed. `update` and `remove`
  give the link issue `notAllowed`.
- Issues are attached to the right `LinkError`. If there are any, the result
  is `Validation { fields, links }` and nothing is written.

### Writing (M7's step 11, inside the same transaction)

The object itself was updated or inserted in step 10, so this side's key is
known. For each relation, apply removes, then updates, then adds:

1. **Remove.** Locate the junction row by the pair (this side's referenced
   value, other side's referenced value).
   - Missing → `Conflict { current, link }`.
   - `version` is given and differs → `Conflict { current, link }`.
   - Otherwise `DELETE FROM <junction> WHERE <this> = ? AND <other> = ?`.
2. **Update.** Locate the row.
   - Missing or version differs → `Conflict { current, link }`.
   - Otherwise resolve `Reference` inputs (`resolve_references`), then
     `UPDATE <junction> SET <changed link fields> … WHERE <pair>`.
3. **Add.**
   - The other object must exist, otherwise the link issue
     `referenceMissing`, which rolls back and returns `Validation`.
   - A row for the pair already exists → `Conflict { current, link }`.
     Adding is strict, because silently keeping the old row would discard
     the fields the user just entered.
   - Otherwise `INSERT INTO <junction> (<this>, <other>, <supplied link
     fields>) VALUES (…)`. Other columns take their defaults.

The stored relation values are the referenced columns of each side's FK. Use
the referenced columns, not the key: a FK may reference a unique column that
is not the primary key.

`current` in a link conflict is the object re-read inside the transaction
before the rollback. On a create, where the object did not exist before, a
link conflict cannot happen.

**Constraint mapping for junction writes:**

| SQLite error | Result |
| --- | --- |
| `UNIQUE` or `NOT NULL` naming exactly one changed link field | `Validation` with a `LinkError` for that link (`unique` / `required`) |
| A `UNIQUE` failure on the pair itself | `Conflict { link }` |
| Anything else | `Constraint(message)` |

All of them roll back everything, the object's field changes included.

## Files

Create `crates/fm-dbmanager/src/session/links.rs` and its tests.

Edit:

- `src/session/save.rs` (hook in step 11 and extend the errors)
- `src/session/mod.rs`
- `src/session/sql.rs`
- `apps/desktop/src-tauri/src/generated_types.rs`, plus the generated files
- `CLAUDE.md` (the layout line for `src/session/`)

Do not touch the frontend outside `lib/generated/`, other crates, or
`docs/plans/`.

## Acceptance criteria

Against `examples/dbmanager/schema.sql` and `example.toml` in a temp dir.
`contribution(book_id, author_id, role NOT NULL DEFAULT 'editor', position)`
is a declared many-to-many with link fields `role` (allowed:
editor/translator/illustrator) and `position` (1–99). `book_genre` is a
detected many-to-many with no link fields.

**Adding links**

- Add a contribution to a book with `role = "translator"`, `position = 1`.
  `m2m_options` then shows the link with those `link_values`. The author's
  side shows the same link.
- An add without `role` stores the default `'editor'`.
- An add with `role = "author"` gives `notAllowed`. `position = 0` gives
  `belowMin`. Both are returned together under one `LinkError`, and nothing
  is written.
- Adding an existing link gives `Conflict` with `link` set, for both
  `contribution` and `book_genre`.

**Editing and removing links**

- Updating `position` with the current `link_version` works. With a stale
  version, the result is `Conflict` with `link` set and nothing changes.
- Removing with the current version deletes the row. Removing a missing
  link, or one with a stale version, gives `Conflict`. Removing without a
  version deletes it.

**Combined saves**

- A save that changes `book.title`, adds two genres, removes one, and edits
  one contribution is one transaction. A failure in its last step leaves
  the title and every link unchanged.
- Creating a book with `author_id`, two genres and one contribution in one
  request works, and the result shows the links. `update`/`remove` in a
  create gives `notAllowed`.

**Field and relation restrictions**

- With a test config where `position` is `create-only`, updating it gives
  `notEditable` and setting it on add works.
- With a test config where a `NOT NULL`, no-default link field is hidden,
  adding gives `NotEditable` (the relation cannot add links).
- With `mode = "read-only"` on the relation, any link change gives
  `NotEditable`. The same holds on a side whose table is read-only or
  `pickable`; a pickable table is still a valid **other** side (linking a
  book to a pickable table's object works from the book).
- A save with only link changes and a stale `config_hash` gives
  `ConfigChangedOnDisk` (M7's step 1 covers the whole request), nothing
  written.
- A relation that is not this table's gives `UnknownRelation`.
- A link field that is a duration validates and stores like an object
  duration field (test schema).

**Backup, identifiers and tooling**

- The backup is taken for a save that only changes links, if it is the
  session's first write. It follows M7's order as built: the backup is
  taken **before** `BEGIN IMMEDIATE`, and the `data_version` race guard
  applies (after `BEGIN`, a changed `data_version` means roll back, drop
  the backup and retry, at most 3 times, then `Busy`). Reuse M7's helper;
  do not write a second backup path.
  `backup_created` is reported on the first **successful** write after the
  backup (M7's announced flag), so a failed first save does not lose it.
  Test: `link_only_first_save_fails_then_announces_backup`.
- `SessionError::{NotEditable, BackupFailed, Constraint}` are struct variants
  since M7's follow-up (`{ reason }`, `{ detail }`, `{ message }`).
- Awkward identifiers (`a"b`, `select`, `with space`, `ünï`) on the junction
  and its link fields work.
- Generated types are current. fmt, clippy pedantic and
  `cargo test --workspace` pass.

## Tests proving completion

Adding links:

- `link_add_with_fields`
- `link_add_uses_defaults`
- `link_add_validation_collects`
- `link_add_existing_conflicts`
- `link_add_other_missing`

Editing and removing links:

- `link_update_fields`
- `link_update_stale_conflicts`
- `link_remove_with_and_without_version`
- `link_remove_missing_conflicts`

Combined saves:

- `links_and_fields_one_transaction`
- `create_with_links`
- `create_rejects_update_remove`

Field and relation restrictions:

- `create_only_link_field`
- `hidden_required_link_field_blocks_add`
- `read_only_relation_refused`
- `read_only_side_refused`
- `pickable_other_side_links`
- `links_only_stale_config_hash`
- `unknown_relation`
- `duplicate_other_rejected`
- `link_duration_field`
- `link_constraint_mapping`

Backup and identifiers:

- `links_only_save_backs_up`
- `awkward_identifiers_links`

Run `cargo test -p fm-dbmanager`, regenerate, then `cargo test
--workspace`, then fmt and clippy per `CLAUDE.md`.

## Out of scope

- Delete and its impact (next milestone).
- Tauri commands and the UI error type.
- Self-referential many-to-many (not in v1).

## Risks

- Strict link semantics turn a stale checklist into a `Conflict`. That is
  intended: the UI reloads the checklist and keeps the user's other edits.
- The pair lookup must use the FK target columns, not the `ObjectKey`, when
  they differ.
