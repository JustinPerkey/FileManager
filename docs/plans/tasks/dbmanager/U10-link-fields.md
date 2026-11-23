# U10 — Database Manager: link-field editors

Status: approved (2026-11-21) (rewritten 2026-11-21 for GPUI; the React version is in git history)
Project: dbmanager (GPUI: gpui-migration)   Depends on: U9 (landed), gpui-migration M12c (landed, with `edit_link_field` taking the `EditorInput` of planner request R4)

## Goal

Show and edit the values stored on many-to-many links ("Role", "Position")
inline on linked and staged-add checklist rows, with the same editors and
validation as fields, the "Will be removed with its fields: …" line before
Save, and link errors attached to their row.

## Context

FileManager (Windows 11, GPUI + gpui-component). Some Database Manager
relations carry fields on the link itself (`M2mView.link_fields`). Editing
a linked row's field stages an **update** (tag Edited); a staged add is
prefilled from each link field's default; create-only link fields are
editable only on a staged add (else read-only "Set when linked");
required-on-create link fields are Required on an add. Unticking a link that
has fields shows, before Save, what will be lost. All of this is the
model's (gpui-migration M12c); validation is the model's.

Rules: views render and call; copy from `text.rs`; keyboard reachable and
labelled.

### Model (M12c + R4)

```rust
impl DbmanagerModel {
    pub fn edit_link_field(&mut self, relation: &str, other: &ObjectKey, field: &str, input: EditorInput, cx);
    pub fn staged_rows(&self, relation: &str) -> Vec<StagedRow>;   // per row: link-field editor text, changed, issues, shown, kind
    pub fn undo_link(&mut self, relation: &str, other: &ObjectKey, cx);
}
```

(If `edit_link_field` takes only text in the landed model, use it for text
kinds and **report** that bool/choice link fields cannot be edited.)
Copy (`text.rs`): field display names, "Edited", "Required", "Set when
linked", "Will be removed with its fields: Role translator, Position 1.",
"Undo", issue messages, link-level issue messages.

### Widgets and tokens

The U5 editors (`text_input`, `select`, `checkbox`), `tag`, `button`
(quiet "Undo"), `icon(XCircle)`. Tokens: `surface`, `border`, `text`,
`text_muted`, `danger`.

### Look

- On a linked or staged-add row, after the label: one compact editor per
  link field, each with its visible short label (13 px muted) before it
  and a tooltip "<Field> for <label>"; inputs 160 px wide (mono for
  numbers); they wrap under the label when the sheet is narrow.
- Tags on the field: **Edited**, **Required** (on adds), **Set when
  linked** (read-only create-only fields on existing links).
- Issues under the row (13 px, `danger`, `x-circle`), shown after blur or a
  save attempt; link-level issues on the row itself.
- Staged remove of a link with fields: a line under the row, 13 px, "Will
  be removed with its fields: …" with **Undo**.

### Keys

Link-field editors are in the Tab order after their row's checkbox; Escape
in a changed link field reverts it if the model offers that (else no-op).

Debug selectors: `db-link-<relation>-<n>-<field>`.

**Hosting rules (after the U3 and U4 reviews, 2026-11-23).**

- **Sizing.** The chrome hosts the tool view in a `flex_1` / `min_h_0`
  column and cannot style it. So this task's root view is `size_full`, adds
  its own padding (16 px vertical, 24 px horizontal: `SPACE_4` / `SPACE_6`
  from `theme::tokens`), and handles its own scrolling (an
  `overflow_y_scroll` region inside the root, not the root's parent).
- **Confirmations.** If this view shows a confirmation, call `sync_confirm`
  only from a model observer (`cx.observe(&model, …)`), never inside
  `render`. Opening or closing a dialog during render re-enters the window.

**Scenario tags (planner rule, 2026-11-22, after M4).** The screenshot
harness resolves `Step::Click` and `Scenario::clip` only by `.tag("<name>")`
names (`apps/filemanager/src/app/probe.rs`); GPUI's debug selectors cannot be
read outside `VisualTestContext`, so they do not work in the harness. Every
element a scenario clicks or clips therefore carries `.tag("<name>")` as the
**outermost (last) call of its builder chain**, using the names this plan
lists (where this plan says "debug selector", read "tag"). If a
`VisualTestContext` view test also clicks the element, add
`.debug_selector(..)` with the same name as well.

## Files

Create: `tools/dbmanager/view/link_fields.rs`.
Edit: `tools/dbmanager/view/{checklist.rs,editors.rs,tests.rs}`.
Do not touch: model, draft, text, fixtures, other tools, `crates/`,
`docs/plans/`.

## Skill

None here; U14 critiques `dbmanager-link-fields` and `-link-remove`.

## Acceptance criteria

- Editors appear on linked and staged-add rows only; prefilled defaults,
  Required, Set when linked and Edited follow the model; the removal line
  shows before Save and Undo restores the row.
- Keyboard and labels as specified; no colour literal outside `theme`.
- fmt; clippy pedantic `-D warnings`; `cargo test --workspace`.

## Tests proving completion

`tools/dbmanager/view/tests.rs`: `link_fields_on_linked_and_added_rows`,
`editing_link_field_tags_edited`, `create_only_link_field_read_only_on_existing`,
`removal_with_fields_line_and_undo`, `link_errors_attach_to_row`.

## States covered

Existing link, edited link, staged add (prefilled, required), staged
remove with data, invalid, link-level error.

## Out of scope

Delete (U11).
