# U8 — Database Manager: creating an object

Status: awaiting approval (rewritten 2026-11-21 for GPUI; the React version is in git history)
Project: dbmanager (GPUI: gpui-migration)   Depends on: U7 (landed), gpui-migration M12b (landed)

## Goal

Add **New** to the object list (and Ctrl+N), render the create form the
model prepares (defaults prefilled, Required tags, "Set automatically",
"Database default"), and the create variant of the save bar (Cancel,
Create).

## Context

FileManager (Windows 11, GPUI + gpui-component). In the Database Manager a
new object is a place in history (`Place::New { table }`); its form is the
model's draft (gpui-migration M12b): editable and create-only fields are
editors, read-only fields show "Set automatically" (or their known
default), fields with a `default_value` are prefilled, `has_default`
without a value shows "Database default"; Cancel discards and goes Back;
Create saves and **replaces** the place with the created object.

Rules: views render and call; copy from `text.rs`; keyboard reachable and
labelled.

### Model (M12b)

```rust
impl DbmanagerModel {
    pub fn new_object(&mut self, table: &str, cx);   // asks first when the draft is dirty (leave dialog, U6)
    pub fn cancel_new(&mut self, cx);
    pub fn save(&mut self, cx);  pub fn saving(&self) -> bool;  pub fn dirty(&self) -> bool;
    pub fn draft(&self) -> Option<&Draft>;           // includes required-on-create and missing-required issues
    pub fn place(&self) -> &Place;
}
```

`TableView`: `can_create`, `object_name` (e.g. "author"), `display_name`.
Copy (`text.rs`): "New <objectName>" or "New" (tooltip "New object in
<table>"), the title "New <objectName>" or "New object", "Not saved yet",
"Set automatically", "Database default", "Required", "Create",
"Cancel", "Created <label>." (notice, model).

### Widgets and tokens

`button` (secondary with `plus` icon for New; primary "Create"; "Cancel"),
`tag` ("Not saved yet", "Required"), the U5 editors, U7's "Choose…".
Tokens: as U5/U6.

### Look

- **New** in the object list toolbar (U3), right of the search, only when
  `can_create`; `plus` icon; tooltip "New object in <table> (Ctrl+N)".
- **Create form** in the sheet: eyebrow = table; h2 = "New <objectName>" /
  "New object"; tag **Not saved yet**; fields as U5 with: **Required** tag
  on required-on-create fields; read-only fields as read-only text "Set
  automatically" (or the default) in muted; placeholder "Database default"
  in inputs with `has_default` and no known value; a required reference
  shows **Choose…** instead of a link. Referenced by is not shown;
  checklists are (U9).
- **Save bar** in create mode: status "Not saved yet", **Cancel**
  (secondary → `cancel_new`) and **Create** (primary, busy "Creating…",
  shortcut Ctrl+S → `save`).

### Keys

Ctrl+N → `new_object(current table)` when the table allows creating
(works inside inputs). Add the help row "Ctrl+N · New object · The table
allows creating".

Debug selectors: `db-new`, `db-create`, `db-cancel-new`.

## Files

Edit: `tools/dbmanager/view/{object_list.rs,sheet.rs,sheet_header.rs,field_row.rs,save_bar.rs,keys.rs,shortcuts.rs,tests.rs}`.
Do not touch: model, draft, text, fixtures, other tools, `crates/`,
`docs/plans/`.

## Skill

None here; U14 critiques `dbmanager-create`.

## Acceptance criteria

- New shows only when `can_create`, with the noun rule; Ctrl+N works.
- The create form shows defaults, Required, Set automatically and Database
  default as the model provides; Create and Cancel work by keyboard.
- Labels/tooltips; no colour literal outside `theme`; fmt; clippy pedantic
  `-D warnings`; `cargo test --workspace`.

## Tests proving completion

`tools/dbmanager/view/tests.rs`: `new_button_noun_and_visibility`,
`ctrl_n_opens_create_form`, `create_form_prefills_and_tags_required`,
`set_automatically_and_database_default_render`,
`create_saves_and_place_becomes_object`, `cancel_goes_back`.

## States covered

Create form clean, edited, missing required (after Create), creating,
created.

## Out of scope

Checklists on the create form (U9), link fields (U10).
