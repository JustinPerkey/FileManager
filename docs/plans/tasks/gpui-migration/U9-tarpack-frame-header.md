# U9 — Tar Packager: view frame, states, header, Recent, banners, empty state, errors region, drop result

Status: approved (2026-11-21)
Project: gpui-migration   Depends on: U4 (landed), M7a (landed)

## Goal

Build the Tar Packager's GPUI view frame and everything above the entry
table: loading skeleton, restore failure, the empty state, the manifest
header with its actions and Recent menu, the banners, the manifest errors
region and the drop result, rendering `TarpackModel`'s manifest side.

## Context

FileManager is moving from Tauri + React to GPUI + gpui-component. The UI
looks like today: same layout and token colours (±1 per channel, geometry
within about 2 px of `docs/manual/images/tarpack-*.png` at 1366×800); text
widths may differ. Views render the model and call its methods; behaviour,
copy and predicates are in `model.rs`/`text.rs` (backend implementer). A
missing model need is a gap to report. Every interactive element is
keyboard reachable and labelled.

The Tar Packager builds a Linux tar archive from Windows files listed in a
TOML manifest; one row per manifest file shows its status, source, Linux
target, mode and owner; the user assigns files by drop or Browse, picks the
output and format, and builds.

### Model API (`apps/filemanager/src/tools/tarpack/model.rs`, M7a)

```rust
pub enum DropState { Pending, Result(DropReport) }   // DropReport: outcome, entries, has_failed_entries, lines
impl TarpackModel {
    pub fn session(&self) -> Option<&TarpackSession>;  // manifest: Option<SessionManifest> (name, path display, entries,
                                                        //   errors, warnings), ready_count, total_count, …
    pub fn loading(&self) -> bool;  pub fn show_skeleton(&self) -> bool;  pub fn load_failed(&self) -> bool;
    pub fn error(&self) -> Option<&ViewError>;  pub fn error_generation(&self) -> u64;  pub fn dismiss_error(&mut self, cx);
    pub fn state_warning(&self) -> Option<&str>;  pub fn dismiss_state_warning(&mut self, cx);
    pub fn changed_on_disk(&self) -> bool;
    pub fn errors_expanded(&self) -> bool;  pub fn set_errors_expanded(&mut self, on: bool, cx);  pub fn show_errors(&mut self, cx);
    pub fn recent(&self) -> &[String];  pub fn open_recent(&mut self, index: usize, cx);
    pub fn open(&mut self, cx);  pub fn reload(&mut self, cx);
    pub fn can_edit(&self, cx: &App) -> bool;  pub fn edit(&mut self, cx);
    pub fn create_from_example(&mut self, cx);
    pub fn drop_state(&self) -> Option<&DropState>;  pub fn dismiss_drop_result(&mut self, cx);  pub fn drop_pending_shown(&self) -> bool;
    pub fn announcement(&self) -> Option<&str>;        // not rendered
    pub fn take_focus_hint(&mut self) -> Option<FocusHint>; // Report | Reload | AfterResult; cleared on read
}
```

`text.rs` (M7a) holds every string: the title "Tar Packager", the empty
state sentence, hint and buttons ("Open manifest…", "Create from
example…"), header buttons ("Open…", "Recent", "Reload", "Edit in
editor"), the manifest sentence, "The manifest changed on disk." and its
Reload action, the errors heading, consequence and "Show errors"/"Hide
errors", diagnostics wording, the drop result's lines (`buildLines`
equivalent, with tones), its "paths" disclosure, error messages. Drop
itself is the Shell's (overlay from the model's drop target).

**Focus hints**: `take_focus_hint()` (consumable; built in M7a). `Report` → the errors region's
report; `Reload` → the Reload button; `AfterResult` → the first tabbable
element after the drop result (the build bar), as today, not the header. Announcements are not rendered.

### Widgets and tokens

`text::{h1, h2, muted, small_muted, mono_small_muted}`, `button`
(primary, quiet, `.icon`, `.icon_only`, `.shortcut`), `MenuButton`,
`ShortcutsHelp` (with the `SHORTCUT_ROWS` this task creates; see Files),
`Banner`, `Disclosure`, `skeleton_bar`, `MiddlePath`, `icon`/`status`.
Tokens: `bg`, `surface`, `surface_sunken`, `border`, `text`, `text_muted`,
`ok`, `warn`, `danger`, `accent`.

### Layout (today's `tarpack.css`; reference `tarpack-ready.png`, `-errors`, `-dropped`, `-empty`)

The view is a column filling the tool area: padding 16/24; regions 12 px
apart. **Top region** (this task), then the entry table (U10), the build
result and the build bar (U11). The table takes its content height and
shrinks to scroll inside itself when space runs out, so the build bar stays
visible under it (today's sticky bar). Build this column now with empty
slots for U10/U11 (`table_slot`, `result_slot`, `bar_slot` functions in
`view/mod.rs` that return nothing yet).

1. **Loading**: nothing until `show_skeleton()`; then two skeleton bars
   (60% wide 24 px, 40% 16 px, 8 px apart).
2. **Restore failure** (`load_failed`): the error banner, then the empty
   state.
3. **No manifest**: a title bar (h1 "Tar Packager", right: the shortcuts
   help button), the empty state (max 546 px, 12 px gaps): the sentence;
   **Open manifest…** (primary, shortcut Ctrl+O) and **Create from
   example…**; the muted hint; the Recent menu button when `recent()` is
   not empty.
4. **Header** (manifest loaded): left, h1 = manifest name and under it
   (4 px) the manifest path: `MiddlePath`, mono 13 px muted, tooltip full
   path; right (8 px gaps, wrapping under the title when narrow):
   **Open…** (shortcut Ctrl+O), **Recent** (`MenuButton`: items = `recent()`
   display paths, label the file name, detail the folder; select →
   `open_recent(i)`), **Reload** (shortcut "F5 or Ctrl+R"), **Edit in
   editor** (only when `can_edit(cx)`; shortcut Ctrl+E), the shortcuts help
   button (quiet, last). Debug selector of the header box: `tarpack-header`.
5. **Banners** (8 px apart, in this order): command error (`error()`,
   `Tone::Error`, Details, dismissible, re-shown on a new generation);
   changed on disk (`Tone::Warn`, "The manifest changed on disk.", action
   **Reload**); state warning (`Tone::Info`, as today, dismissible →
   `dismiss_state_warning`).
6. **Errors region** (manifest has errors): `surface`, 1 px `danger`
   border, radius 6, padding 8/12; a row (8/12 gaps): `x-circle` in danger,
   the heading (16 px 600) and consequence line, and **Show errors** /
   **Hide errors** (`set_errors_expanded`); when expanded, under a 1 px
   rule, the report: groups (14 px 600) of diagnostics, each row = the
   position (mono muted, min 5 characters) and the message (wrapping); the
   report scrolls inside itself at 40% of the view height. **Warnings**
   (manifest has warnings): a bordered `surface` disclosure, `warn`
   `alert-triangle` before its summary, the same diagnostics list inside.
7. **Drop result** (`drop_state()`): `Pending` and `drop_pending_shown()`:
   a muted line "Matching dropped files…" (copy from `text.rs`). `Result`:
   bordered `surface` box, padding 8/12, the lines (4 px apart, each a tone
   icon — ok `check-circle`, warn `alert-triangle`, muted `info` — and the
   text, wrapping; hint lines muted), a "paths" `Disclosure` (13 px muted)
   listing the dropped paths in groups, and a quiet icon-only **Dismiss**
   (`x`, tooltip "Dismiss") → `dismiss_drop_result`.

Debug selectors: `tarpack-open`, `tarpack-recent`, `tarpack-reload`,
`tarpack-edit`, `tarpack-shortcuts`, `tarpack-create-example`,
`tarpack-error`, `tarpack-changed`, `tarpack-errors`,
`tarpack-errors-toggle`, `tarpack-drop-result`, `tarpack-header`.

**Shortcuts table overflow check (2026-11-23).** This task renders the
Tar Packager's real shortcuts table (`SHORTCUT_ROWS` in the shared
`ShortcutsHelp`). The shared grid uses `grid_cols_max_content(3)`, which is
`minmax(0, max-content)`. Under the 576 px cap, a wide chord in a row with a
long action can shrink the keys column below its content, and the chord then
overflows into the actions column. U3's alignment test cannot catch this,
because grid cells stretch.

- Add `shortcuts_keys_do_not_overflow_into_actions`, a view test with
  `VisualTestContext`. Give the **last** `kbd` of each row and each row's
  action cell a debug selector, open the help with the real `SHORTCUT_ROWS`,
  and assert that each row's last `kbd` ends at or before its action cell's
  left edge.
- If it overflows, use U3's fallback: a keys-column width in `REM`
  multiples from `theme::tokens`, wide enough for the widest chord. Do not
  use a pixel literal.

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

Create: `apps/filemanager/src/tools/tarpack/view/{header.rs,banners.rs,empty.rs,errors.rs,drop_result.rs,tests.rs}`.
Edit: `apps/filemanager/src/tools/tarpack/view/mod.rs` (replace the
placeholder with the frame), `docs/gpui-test-ledger.md`.
The shortcuts list itself is U11's (`view/shortcuts.rs`); here render
`ShortcutsHelp` from a `SHORTCUT_ROWS` constant you create in
`view/shortcuts.rs` with the rows below, which U11 extends with the
bindings:

| Keys | Action | When |
|---|---|---|
| Ctrl+O | Open manifest… | Always |
| F5 or Ctrl+R | Reload manifest | A manifest is open |
| Ctrl+E | Edit in editor | A manifest is open (developer mode only: the row shows only then) |
| Ctrl+Enter | Create archive | The archive can be built |
| F8 | Show errors | The manifest has errors |
| Up or Down | Move between files | Focus is in the table |
| Enter | Browse… for the focused file | A file row is focused |
| Delete | Clear the focused file | The row has a file assigned |
| Esc | Dismiss the drop or build result | A result is showing |

(If M7b's `text.rs` already holds this list, use it instead.)

Do not touch: model, text, backend, fixtures, other tools, `crates/`,
`apps/desktop/`, `docs/plans/`.

## Skill

None here; U12 critiques the screenshots.

## Acceptance criteria

- Each state above renders from the model with copy from `text.rs`.
- Keyboard: every header action, the empty-state buttons, banner actions,
  Show errors, the drop result's disclosure and Dismiss are Tab reachable
  and operable; the Recent menu works by keys (U3).
- Labels or tooltips on every control.
- No colour literal outside `theme`; no filesystem or domain-crate call.
- fmt; clippy pedantic `-D warnings`; `cargo test --workspace`.

## Tests proving completion

`tools/tarpack/view/tests.rs` (`#[gpui::test]`, `FixtureTarpack`,
`FakePlatform`): `renders_manifest_states` (empty, loaded, errors, missing,
withheld, changed: expected selectors), `skeleton_only_when_model_says`,
`header_actions_call_model` (Open, Reload, Recent item by keys),
`edit_button_follows_dev_mode`, `changed_banner_reload`,
`errors_toggle_by_keyboard`, `drop_result_lines_and_dismiss`,
`focus_follows_hint` (Report, Reload).

Ledger: rows M7a marked U in `ManifestHeader.test.tsx`,
`ManifestErrors.test.tsx`, `DiagnosticList.test.tsx`, `DropResult.test.tsx`,
`TarpackView.states.test.tsx`, `TarpackView.errors.test.tsx`,
`TarpackView.assign.test.tsx` (rendering parts).

## States covered

Loading (skeleton after 150 ms), restore failure, empty (with and without
recent), loaded, manifest errors (collapsed/expanded), warnings, changed on
disk, state warning, command error, drop pending, drop result.

**Manual check due (M7a re-review, 2026-11-22).** Once this task's Edit in
editor button exists, Run 4 of `docs/gpui-dialogs-check.md` (Edit with no
`.toml` handler shows "Windows could not open it.") becomes runnable. Note
in the PR that the hand check is due, for the human's next Windows 11
session.

## Out of scope

The entry table (U10); output, build, results, shortcut bindings (U11);
scenarios (U12).
