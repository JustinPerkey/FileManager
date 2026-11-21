# U9 — Database Manager: many-to-many checklists

Status: awaiting approval (rewritten 2026-11-21 for GPUI; the React version is in git history)
Project: dbmanager (GPUI: gpui-migration)   Depends on: U8 (landed), gpui-migration M12a and M12c (landed)

## Goal

Render each many-to-many relation of the open object (or create form) as a
checklist of every object on the other side: heading and counts, search,
Linked only, paging, staged adds and removes in place, the "Not saved yet"
group for staged rows off the page, Open on linked rows, and the link
conflict notice.

## Context

FileManager (Windows 11, GPUI + gpui-component). In the Database Manager a
many-to-many relation is a checklist, not a join table. Ticking stages an
**add**, unticking a linked row stages a **remove**; nothing is written
until Save (gpui-migration M12c: staging lives in the model, keyed by
relation and the other object's key). **Staged rows are never off screen.**

Rules: views render and call; copy from `text.rs`; keyboard reachable and
labelled; status words always beside any icon.

### Model (M12a reads, M12c staging)

```rust
impl DbmanagerModel {
    pub fn m2m(&self, relation: &str) -> Option<&M2mState>;   // query (search, linked_only, offset), page (options: key, label, linked), total, loading, error
    pub fn set_m2m_query(&mut self, relation: &str, search: &str, linked_only: bool, offset: u64, cx);
    pub fn toggle_link(&mut self, relation: &str, other: &ObjectKey, cx);
    pub fn undo_link(&mut self, relation: &str, other: &ObjectKey, cx);
    pub fn staged_rows(&self, relation: &str) -> Vec<StagedRow>;  // kind add/remove/update, on_page or off_page, label
    pub fn link_counts(&self, relation: &str) -> LinkCounts;       // linked, to_add, to_remove
    pub fn link_conflict(&self, relation: &str) -> Option<&LinkConflict>;
    pub fn reapply_link(&mut self, relation: &str, cx);  pub fn discard_link(&mut self, relation: &str, cx);
    pub fn open_object(&mut self, table: &str, key: ObjectKey, cx);
}
```

`M2mView` (in the table view): `heading`, other table, mode
(editable / read-only), searchable flag, `link_fields` (U10). Copy
(`text.rs`): "3 linked · 2 to add · 1 to remove", "Linked only", "Search
<table>", the pager "26–50 of 312", "Will be added", "Will be removed",
"Not saved yet", "Undo", "Open" (tooltip "Open <label> in <table>"),
"Linked", "Read-only", "<Table> has no objects.", "Nothing is linked
yet.", "No <table> match", the error + "Try again", the link conflict
notice ("<label>'s link was changed by another program. Nothing was saved.
Your change to it was set aside.") with "Re-apply my change" and "Discard
it".

### Widgets and tokens

`checkbox`, `tag`, `button` (quiet, `.icon_only(ChevronLeft|ChevronRight)`,
`.icon(Undo)`), `text_input` (search), `Banner` (Warn, actions) for the
conflict notice, `icon(Check)`, `skeleton_bar`, `text::*`. Tokens:
`surface`, `surface_sunken`, `border`, `text`, `text_muted`, `warn`.

### Look (after the fields, one section per relation, 16 px apart)

- **Header**: h3 (16 px 600) = heading; the counts line (13 px muted,
  tabular) from `link_counts`; **Read-only** tag (lock) for read-only
  relations.
- **Toolbar** (8 px gaps): search (only when searchable), **Linked only**
  checkbox (its default comes from the model), the pager (range text +
  icon-only Previous/Next, tooltips "Previous page"/"Next page").
- **Not saved yet** group (when any staged row is `off_page`): a muted
  subheading, then those rows, each with its tag and **Undo**.
- **Rows** (32 px min; 1 px rule under each): editable relation → a
  checkbox labelled by the option's label; linked rows add a quiet
  **Open**; read-only relation → no checkbox: linked rows show `check` +
  "Linked" + Open, unlinked rows only the label. Staged add → tag **Will be
  added**; staged remove → tag **Will be removed** and the label struck
  through; both with **Undo**.
- **States**: skeleton rows (first load), empty, nothing linked (Linked
  only), no match, error + Try again.
- **Link conflict**: a warn banner on the section with **Re-apply my
  change** and **Discard it**.

### Keys

Each checklist's rows are Tab-reachable checkboxes; Space toggles
(`toggle_link`); PageUp/PageDown page when focus is in the list.

Debug selectors: `db-m2m-<relation>`, `db-m2m-<relation>-row-<n>`,
`db-m2m-<relation>-linked-only`, `db-m2m-<relation>-undo-<n>`.

## Files

Create: `tools/dbmanager/view/checklist.rs`.
Edit: `tools/dbmanager/view/{sheet.rs,mod.rs,tests.rs}`.
Do not touch: model, draft, text, fixtures, other tools, `crates/`,
`docs/plans/`.

## Skill

None here; U14 critiques `dbmanager-m2m`.

## Acceptance criteria

- Rows, counts, tags and the Not saved yet group follow the model; each
  staged row renders exactly once.
- Keyboard: search, Linked only, pager, each checkbox, Open and Undo are
  reachable and operable.
- Labels/tooltips; no colour literal outside `theme`; fmt; clippy pedantic
  `-D warnings`; `cargo test --workspace`.

## Tests proving completion

`tools/dbmanager/view/tests.rs` (fixtures over a temp database):
`checklist_renders_page_and_counts`, `space_toggles_and_tags_staged`,
`toggle_twice_clears_tag`, `off_page_staged_row_in_not_saved_group_once`,
`linked_only_and_search_query_model`, `read_only_relation_has_no_checkboxes`,
`link_conflict_banner_actions`.

## States covered

Loading, empty, nothing linked, no match, error, read-only, page, staged
add/remove, off-page staged, link conflict; on a create form (adds only).

## Out of scope

Link-field editors and removal-with-data lines (U10).
