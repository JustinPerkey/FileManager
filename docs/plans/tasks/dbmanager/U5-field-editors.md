# U5 — Database Manager: field editors over the model's draft

Status: approved (2026-11-21) (rewritten 2026-11-21 for GPUI; the React version is in git history)
Project: dbmanager (GPUI: gpui-migration)   Depends on: U4 (landed), gpui-migration M12b (landed, with the editor API of planner request R4)

## Goal

Turn editable fields of the open object into typed editors bound to the
model's draft: text, integer, real, duration, allowed values and booleans
(including "not set"), with Edited tags, Escape to revert a field, and
inline issues shown after the field loses focus or a save attempt.

## Context

FileManager (Windows 11, GPUI + gpui-component). The Database Manager's
**draft lives in the model** (`model.rs` + `draft.rs`, gpui-migration
M12b): one draft at a time, keyed by column; the model validates with
`fm_dbmanager::rules::validate` on every change. Views only send input and
render the draft. A field is **changed** when its value (normalised when
valid) differs from the stored one: "1h 30m" over "1h30m" is not a change.

Rules: no validation, conversion or copy in the view; no filesystem
access; every editor keyboard reachable and labelled by the field's display
name.

### Model (M12b, with R4)

```rust
pub enum EditorInput { Text(String), Bool(Option<bool>), Choice(Option<String>) }
impl DbmanagerModel {
    pub fn draft(&self) -> Option<&Draft>;   // per field: editor text, changed(), issues(), shown(), stored value
    pub fn dirty(&self) -> bool;  pub fn dirty_count(&self) -> usize;
    pub fn edit(&mut self, field: &str, input: EditorInput, cx);
    pub fn blur(&mut self, field: &str, cx);           // a duration is shown canonical after a valid entry
    pub fn revert_field(&mut self, field: &str, cx);   // Escape in a changed field
    pub fn saving(&self) -> bool;                      // editors read-only while saving
}
```

(R4 asked the planner for `EditorInput`; M12b's text says `edit(field,
text)` "also bool/choice/null variants as needed". **Use the landed names
with the same meaning; if the model cannot express bool, choice or null,
stop and report.**)

Copy from `text.rs`: issue messages per code (`required`, `notInteger`,
`notReal`, `notDuration`, `durationPrecision`, `belowMin`, `aboveMax`,
`tooShort`, `tooLong`, `patternMismatch` with the pattern in mono,
`notAllowed`, …), the duration hint "Like 1h30m or 1h 30m. A plain number
counts <unit>.", "Not set", "Stored: X (not allowed)", the counter
"12 / 200", "Edited", "Clear" and its tooltip "Clear <field>".

### Widgets and tokens

`text_input` (`.mono`, `.invalid`, `.read_only`, `.multiline`), `select`,
`checkbox` + `CheckState::{Unchecked, Checked, NotSet}`, `tag` ("Edited"),
`button` (quiet), `icon(XCircle)`. Tokens: `surface`, `border`, `danger`,
`text`, `text_muted`.

### Editors (in U4's field row; the value column becomes the editor when the field is editable and not saving)

| Kind / rule | Editor | Input sent |
|---|---|---|
| integer | text input, mono, tabular | `Text` (model trims; empty = null when nullable) |
| real | text input, mono | `Text` |
| text | text input, or multi-line (4 rows) when `multiline`; counter "n / max" (13 px muted, tabular) right of the hint when `max_length` | `Text`, untrimmed |
| allowed values | `select` of the allowed labels; "Not set" first when nullable; a stored value outside the list is shown as an extra selected option "Stored: X (not allowed)" | `Choice` |
| bool | `checkbox` labelled by the field; nullable and null → `NotSet` with "Not set"; clicking/Space cycles per `CheckState::next` restricted to Unchecked/Checked for non-nullable; a quiet **Clear** (tooltip "Clear <field>") sets null, shown only when nullable and not null | `Bool` |
| duration | text input, mono; hint as above | `Text`; `blur` on focus loss |
| FK | U7 | — |
| blob, never-editable | read-only as U4 | — |

- **Edited** tag on each changed field (beside the label).
- Issues are shown when the draft says `shown()` (after the first blur or
  a save attempt): `x-circle` + message in `danger`, 13 px, under the hint,
  and the input `.invalid(true)`.
- **Escape** in a changed field → `revert_field`; Escape in an unchanged
  field does nothing here.
- While `saving()`: editors are read-only (focus stays where it is).
- Stored-value problems stay as U4 draws them until the field is edited.

Debug selectors: `db-edit-<field>`, `db-clear-<field>`, `db-issue-<field>`.

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

Create: `tools/dbmanager/view/editors.rs`.
Edit: `tools/dbmanager/view/{field_row.rs,mod.rs,tests.rs}`.
Do not touch: model, draft, text, fixtures, other tools, `crates/`,
`docs/plans/`.

## Skill

None here; U14 critiques the screenshots.

## Acceptance criteria

- Each kind has the editor above and sends the input named; nothing is
  validated or converted in the view.
- Issues appear only after blur or a save attempt; Edited tags follow
  `changed()`.
- Keyboard: every editor and Clear reachable; Space cycles a checkbox;
  Escape reverts a changed field.
- Labels: each editor is labelled by the field name (visible label).
- No colour literal outside `theme`; fmt; clippy pedantic `-D warnings`;
  `cargo test --workspace`.

## Tests proving completion

`tools/dbmanager/view/tests.rs` (`#[gpui::test]`, fixture over a temp
database): `typing_edits_draft_and_tags_edited`,
`issue_shown_after_blur_only`, `escape_reverts_changed_field`,
`duration_canonical_after_blur`, `nullable_bool_cycles_and_clear`,
`allowed_values_select_with_not_set_and_stored_extra`,
`text_counter_shows_length`, `editors_read_only_while_saving`.

## States covered

Unchanged, changed (Edited), invalid shown, invalid not yet shown, not set,
stored value outside the allowed list, saving (read-only).

## Out of scope

Save, discard, conflicts, leave dialog (U6); references (U7); create (U8).
