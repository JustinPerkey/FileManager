# U7 — Schedule Creator views

Status: approved (2026-11-21)
Project: gpui-migration   Depends on: U4 (landed), M6 (landed)

## Goal

Replace the Schedule Creator's placeholder view with today's screen,
rendering `ScheduleModel` and calling its methods, keyboard operable and
matching today's layout and colours.

## Context

FileManager is moving from Tauri + React to GPUI + gpui-component. The UI
looks like today: same layout and token colours (±1 per channel, geometry
within about 2 px of `docs/manual/images/schedule-*.png`); text widths may
differ. Views render the model and call its methods; behaviour, copy and
predicates are in `model.rs`/`text.rs` (backend implementer). A missing
model need is a gap to report. Every interactive element is keyboard
reachable and labelled.

The Schedule Creator reads a schedule from a text file and adds it to an
existing XML file (both chosen by typed path, Browse, or drop), after a
confirmation; the XML is backed up first. Its parse and merge are stubs, and
the UI says so through the error copy.

### Model API (`apps/filemanager/src/tools/schedule/model.rs`, M6)

```rust
pub enum Slot { Text, Xml }
impl ScheduleModel {
    pub fn session(&self) -> Option<&ScheduleSession>;   // text and xml FileSlot (display path, error), can_apply
    pub fn loading(&self) -> bool;  pub fn load_failed(&self) -> bool;  pub fn retry_load(&mut self, cx);
    pub fn error(&self) -> Option<&ViewError>;  pub fn error_generation(&self) -> u64;  pub fn dismiss_error(&mut self, cx);
    pub fn set_typed(&mut self, slot: Slot, typed: &str, cx);  pub fn browse(&mut self, slot: Slot, cx);
    pub fn apply_reason(&self) -> Option<&'static str>;  pub fn can_apply(&self) -> bool;  pub fn applying(&self) -> bool;
    pub fn request_apply(&mut self, cx);  pub fn pending(&self) -> Option<(&str, &str)>;   // text and XML paths in the dialog
    pub fn confirm(&mut self, cx);  pub fn cancel(&mut self, cx);
    pub fn result(&self) -> Option<&ApplySummary>;       // includes the backup path
    pub fn dismiss_result(&mut self, cx);                 // the result banner's dismiss
    pub fn already_applied(&self) -> bool;               // duplicate warning, in the confirmation body
    pub fn announcement(&self) -> Option<&str>;          // not rendered
    pub fn take_focus_hint(&mut self) -> Option<FocusHint>; // Outcome | AddButton; cleared on read
}
```

Drop is handled by the Shell (overlay label/reason from the model's
`ToolHooks::drop_target`); the view does nothing for drop.

Copy (`tools/schedule/text.rs`): header "Schedule Creator", intro, the two
step labels and placeholders, browse labels, "Add to XML…"/"Adding…", the
apply reasons, the result sentence and backup line, the duplicate warning,
the confirmation title, body and "Update", "Try again", error messages.

**Focus hints**: `take_focus_hint()` (consumable; built in M6).
Announcements are not rendered.

### Widgets and tokens

`text::{h1, intro, muted, mono}`, `step_number`, `text_input` (`.mono()`,
`.invalid()`, `.disabled()`), `button` (primary, `.busy`), `Banner`
(`Tone::{Info, Warn, Error}`, `.details`, `.dismiss`, `.action`),
`sync_confirm`. Tokens: `bg`, `surface`, `border`, `text`, `text_muted`,
`danger`, `accent`, `SPACE_1..5`.

### Layout (today's `schedule.css`; reference `schedule-ready.png`)

View scrolls as a whole, padding 16/24, regions 16 px apart:

1. Header: h1 + intro (muted, max 546 px), 4 px apart.
2. Load failure: error banner + **Try again**, nothing else.
3. Error banner (`error()`), with Details; re-shown on a new
   `error_generation`; dismissible.
4. Two steps, 16 px apart, each: a title row (8 px gap): `step_number(1|2)`
   and the label (16 px 600); 8 px below, the file row (12 px gap,
   wrapping): mono input (flex, height 32 px; `.invalid` when the slot has an
   error) and **Browse…** (its tooltip from `text.rs`); then the slot's
   error in `danger`. Typing commits on Enter or blur when changed
   (`set_typed`); Escape reverts. Disabled while `applying`.
5. Apply row: 12 px top padding with a 1 px `border` rule above; **Add to
   XML…** (primary; busy "Adding…" while `applying`; disabled unless
   `can_apply`) and the reason in muted.
6. Result: an info banner with the result sentence and, below it, the
   backup line; dismissible (its dismiss calls `dismiss_result`). No
   duplicate banner here: `already_applied()` is true right after an apply,
   so a banner would sit beside the success result.
7. Confirmation (`pending()`): `sync_confirm` key `"schedule-confirm"`,
   title and body from `text.rs` showing both paths, Cancel focused, confirm
   "Update" (primary). When `already_applied()`, the body also carries the
   duplicate warning from `text.rs`, as today.

Focus: `Outcome` → the result banner (or the error banner); `AddButton` →
Add to XML….

Debug selectors: `schedule-text-input`, `schedule-text-browse`,
`schedule-xml-input`, `schedule-xml-browse`, `schedule-apply`,
`schedule-result`, `schedule-error`, `schedule-retry`.

**Hosting rules (after the U3 and U4 reviews, 2026-11-23).**

- **Sizing.** The chrome hosts the tool view in a `flex_1` / `min_h_0`
  column and cannot style it. So this task's root view is `size_full`, adds
  its own padding (16 px vertical, 24 px horizontal: `SPACE_4` / `SPACE_5`
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

Create: `apps/filemanager/src/tools/schedule/view/{steps.rs,apply.rs,tests.rs}`.
Edit: `apps/filemanager/src/tools/schedule/view/mod.rs` (replace the
placeholder), `docs/gpui-test-ledger.md` (Schedule rows marked U).
Do not touch: model, text, backend, fixtures, other tools, `crates/`,
`apps/desktop/`, `docs/plans/`.

## Skill

None here; U8 critiques the screenshots.

## Acceptance criteria

- Every region above renders from the model; copy only from `text.rs`.
- Keyboard: Tab reaches both inputs, both Browse buttons and Add to XML…;
  Enter/Escape in inputs as specified; the dialog starts on Cancel.
- Labels on every control; no colour literal outside `theme`.
- fmt; clippy pedantic `-D warnings`; `cargo test --workspace`.

## Tests proving completion

`tools/schedule/view/tests.rs` (`#[gpui::test]`, fixtures, `FakePlatform`):
`renders_each_fixture_state` (empty, ready, bad), `enter_and_blur_commit_typed_paths`,
`escape_reverts_typing`, `browse_buttons_pick_their_slot`,
`apply_opens_confirmation_and_update_confirms`, `cancel_applies_nothing`,
`result_banner_with_backup_line_dismisses` (calls `dismiss_result`),
`duplicate_warning_shows_in_confirmation_only` (in the confirmation body for
the same pair; absent beside the result after an apply),
`apply_disabled_with_reason`, `focus_follows_hint_after_apply`.

Ledger: settle every `ScheduleView.test.tsx` row M6 marked U.

## States covered

Loading, load failure, empty, ready, slot error, applying, confirmation,
result with backup, duplicate warning, apply error (not implemented).

## Out of scope

Scenarios (U8); drop behaviour (model and shell).
