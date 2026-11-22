# U7 — Database Manager: editing references with the object picker

Status: approved (2026-11-21) (rewritten 2026-11-21 for GPUI; the React version is in git history)
Project: dbmanager (GPUI: gpui-migration)   Depends on: U6 (landed), gpui-migration M12b (landed, with the picker API of planner request R4)

## Goal

Make editable foreign-key fields changeable: **Change…** opens a non-modal
object picker (search, paged results, Show more, Clear for nullable
fields); picking stages a reference change in the model's draft, shown as
the new label with the Edited tag.

## Context

FileManager (Windows 11, GPUI + gpui-component). In the Database Manager a
reference is shown as the referenced object's label, a link you can follow
(U4). Editing it never shows ids: the user searches the target table's
objects by label. Pickable (lookup) tables are offered here even though
they are not listed. The staged change is part of the draft (the model's),
saved with the other fields.

Rules: the picker's state and queries are the model's (it must not disturb
the table's own list state); views render and call; copy from `text.rs`;
pickers are non-modal popovers; keyboard reachable and labelled.

### Model (M12b + R4)

```rust
impl DbmanagerModel {
    pub fn open_picker(&mut self, field: &str, cx);
    pub fn picker(&self) -> Option<&PickerState>;   // field, target table, search text, items (key, label), total, loading, error
    pub fn picker_search(&mut self, text: &str, cx);  pub fn picker_more(&mut self, cx);
    pub fn pick(&mut self, key: ObjectKey, cx);     // stages the reference, closes the picker
    pub fn clear_reference(&mut self, field: &str, cx);
    pub fn close_picker(&mut self, cx);
    pub fn draft(&self) -> Option<&Draft>;          // the staged reference's label, changed()
}
```

**If these names are not in the landed model, stop and report** (request
R4 to the planner). Copy (`text.rs`): "Change…", "Choose…" (a required
reference on a create form), "Clear", "Search <table>", "Loading…", "No
objects match", "<Table> has no objects.", the error + "Try again", "Show
more", "Not set", "Edited".

Focus hints: `PickerTrigger(field)` → the Change… button after the picker
closes (R5; else return focus there yourself on close).

### Widgets and tokens

`button` (secondary "Change…", quiet "Clear"), the gpui-component
`Popover` anchored under Change… (as `widgets::ShortcutsHelp` does),
`text_input` with the `search` icon, `tag("Edited")`, `skeleton_bar`.
Tokens: `surface`, `surface_sunken`, `border`, `text`, `text_muted`,
`accent`.

### Look

- In an editable FK field row: the current label as the reference link
  (U4) or "Not set", then **Change…** (tooltip "Choose another <table
  object>") and, when nullable and set, quiet **Clear** (tooltip "Clear
  <field>"); the **Edited** tag when the draft changed it.
- **Picker popover**: `surface`, 1 px `border`, radius 6, padding 8, width
  384 px, max height 60% of the window; the search input at the top
  (focused on open; typing → `picker_search`); a list of result rows
  (32 px, label with end ellipsis and tooltip; hover/focused
  `surface_sunken`; the current reference marked with the `check` icon);
  "Show more" at the end while more remain; states Loading / no match /
  empty / error + Try again. Shadow is gpui-kit's (accepted).

### Keys

In the picker: Down/Up move between the search input and rows; Enter picks
the focused row (`pick`); Escape closes (`close_picker`) and focus returns
to Change…; Tab cycles inside the popover.

Debug selectors: `db-change-<field>`, `db-picker`, `db-picker-search`,
`db-picker-row-<n>`, `db-picker-more`.

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

Create: `tools/dbmanager/view/picker.rs`.
Edit: `tools/dbmanager/view/{reference.rs,field_row.rs,mod.rs,tests.rs}`.
Do not touch: model, draft, text, fixtures, other tools, `crates/`,
`docs/plans/`.

## Skill

None here; U14 critiques the screenshots (`dbmanager-picker`).

## Acceptance criteria

- Change… opens the picker; typing searches; Enter or a click picks; Clear
  clears; Escape closes and returns focus; the staged label shows with
  Edited.
- Pickable target tables work like any other.
- Labels/tooltips everywhere; no colour literal outside `theme`.
- fmt; clippy pedantic `-D warnings`; `cargo test --workspace`.

## Tests proving completion

`tools/dbmanager/view/tests.rs`: `change_opens_picker_with_focus_in_search`,
`typing_searches_through_model`, `enter_picks_and_tags_edited`,
`escape_closes_and_refocuses_change`, `clear_sets_not_set`,
`show_more_appends`, `picker_states_render` (loading, no match, error).

## States covered

Picker: loading, results, no match, empty, error, more. Field: link, not
set, staged change, required on create ("Choose…").

## Out of scope

Create forms (U8), link fields (U10).
