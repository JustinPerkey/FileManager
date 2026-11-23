# U11 — Tar Packager: build bar, progress, overwrite, result, report, developer extras, shortcuts

Status: approved (2026-11-21)
Project: gpui-migration   Depends on: U10 (landed), M7b (landed)

## Goal

Render the Tar Packager's build side from `TarpackModel`: the build bar
(output, format, reason, progress, Create archive), the overwrite
confirmation, the build result with its report and copy actions, the
developer-mode extraction command, and the tool's keyboard shortcuts with
their help.

## Context

FileManager is moving from Tauri + React to GPUI + gpui-component. The UI
looks like today: same layout and token colours (±1 per channel, geometry
within about 2 px of `docs/manual/images/tarpack-*.png`); text widths may
differ. Views render the model and call its methods; copy is in
`tools/tarpack/text.rs`. Every interactive element is keyboard reachable
and labelled. Destructive operations are confirmed: an existing archive is
replaced only after the dialog that names it.

### Model API (`apps/filemanager/src/tools/tarpack/model.rs`, M7b; plus M7a's)

```rust
pub enum BuildOutcome { Ok(BuildSummary), Err(TarpackError), Unexpected { detail: String } } // Unexpected: a panicked build; copy text::UNEXPECTED
pub enum Shortcut { Open, Reload, Edit, Build, ShowErrors, Escape }
impl TarpackModel {
    pub fn choose_output(&mut self, cx);  pub fn format_notice(&self) -> Option<&str>;
    pub fn set_format(&mut self, format: ArchiveFormat, cx);
    pub fn build_reason(&self) -> Option<String>;  pub fn left_out_note(&self) -> Option<String>;
    pub fn can_build(&self) -> bool;  pub fn building(&self) -> bool;
    pub fn progress(&self) -> Option<&Progress>;  // no progress_unavailable: removed (M7b follow-up)
    pub fn build(&mut self, cx);
    pub fn confirming_overwrite(&self) -> Option<&str>;   // output display path for the dialog
    pub fn confirm_overwrite(&mut self, cx);  pub fn cancel_overwrite(&mut self, cx);
    pub fn result(&self) -> Option<&BuildOutcome>;  pub fn dismiss_result(&mut self, cx);
    pub fn reveal_output(&mut self, cx);  pub fn copy_report(&mut self, cx);  pub fn copy_extraction_command(&mut self, cx);
    pub fn shortcut(&self, s: Shortcut, cx: &App) -> bool;    // whether the key acts now
    pub fn run_shortcut(&mut self, s: Shortcut, window: &mut Window, cx);
    pub fn set_shortcuts_open(&mut self, open: bool);          // Escape is ignored while the help is open
}
```

`TarpackSession` gives `format`, `formats: Vec<ArchiveFormatOption>` (name,
extension), `output_path` display, `suggested_output_name`, `ready_count`,
`total_count`. `BuildSummary` (`fm_tarpack::archive`): output path, sizes,
entry count, SHA-256, failures, warnings. `text.rs` (M7b) has every string:
"Output:", "none", "Choose…", "Format", the format names, "Create
archive", reason texts, left-out note, format notice, progress phase
labels, the overwrite title, `overwriteBody`, "Replace", result headings
(`resultHeading`), size formatting, copy labels and "Copied", the
extraction-command label, the report text and its group titles, failure
list wording, "Reveal in Explorer", the shortcuts list. Developer mode:
`crate::app::dev_mode::DevMode::is_on(cx)`, observe with
`cx.observe_global::<DevMode>`.

**Focus hints**: `take_focus_hint()` (consumable; built in M7a/M7b). After a build the result's heading takes focus as
today (hint from the model).

### Widgets and tokens

`button` (primary, quiet, `.icon(Folder|Copy)`, `.busy`, `.shortcut`),
`select`, `MiddlePath`, `status`/`icon` (`check-circle` ok,
`alert-triangle` warn, `x-circle` danger), `Disclosure`, `text::*`,
`sync_confirm`, `ShortcutsHelp` with `view/shortcuts.rs::SHORTCUT_ROWS`
(U9). Tokens: `surface`, `surface_sunken`, `border`, `text`, `text_muted`,
`accent`, `ok`, `warn`, `danger`, `SPACE_1..5`, `RADIUS`.

### Look (today's `tarpack.css`; references `tarpack-buildbar.png`, `-building`, `-result`, `-result-partial`, `-replace`, `-dev-result`)

**Build bar** (`bar_slot`, the last item of the view's column; full width of
the tool area, i.e. it cancels the view's 24 px side padding and 16 px
bottom padding): `surface`, a 1 px `border` rule above, padding 12/24,
items in a wrapping row (8/16 gaps), two groups:

- Settings (grows): "Output:" (13 px muted), the output `MiddlePath` (mono
  13 px; "none" muted when unset), **Choose…** (folder icon →
  `choose_output`), then "Format" (13 px muted) and the format `select`
  (options from `formats`; disabled while building), and the format notice
  (13 px) when set.
- Action (right-aligned): the status — the reason (13 px muted), or the
  left-out note (13 px, warn `alert-triangle` before it), or while building
  the progress: phase label 13 px, a row of a 6 px track (`surface_sunken`,
  radius 6) with an `accent` fill of the done fraction and the percent
  (13 px, tabular, min 4 characters); no "progress unavailable" state (the
  model has none) — and **Create archive** (primary, busy while
  building, disabled unless `can_build`, shortcut Ctrl+Enter), with **Show
  errors** (shortcut F8) beside it when the manifest has errors.

**Overwrite** (`confirming_overwrite()`): `sync_confirm` key
`"tarpack-overwrite"`, title and body from `text.rs` naming the file,
Cancel focused, **Replace** primary → `confirm_overwrite`; cancel →
`cancel_overwrite`.

**Build result** (`result_slot`, between the table and the bar):
`surface`, 1 px `border`, radius 6, padding 12, 8 px gaps. Head: tone icon
(ok full success, warn partial, danger failure), heading (16 px 600) and a
quiet icon-only **Dismiss** (`x`, tooltip "Dismiss result") →
`dismiss_result`. Success: lede, the output path (mono 13 px) with
**Reveal in Explorer** and **Copy**, facts (sizes, entries; tabular), the
SHA-256 row (13 px 600 muted label; mono value; **Copy** with a "Copied"
label 13 px muted that appears after copying); in developer mode the
extraction command as a block (`surface_sunken`, radius 6, padding 8, mono
13 px, wrapping) with **Copy** (`copy_extraction_command`); the hint (13 px
muted). Partial: the same plus the failure list. Failure: the message from
`text.rs`. **Report** (warnings and failures): under a 1 px rule, groups
(14 px 600 titles), failure items (name 14 px 600, where 13 px muted,
message), warnings in a `Disclosure`; scrolls inside itself at 40% of the
view height; **Copy report** → `copy_report`.

Debug selectors: `tarpack-build-bar`, `tarpack-choose-output`,
`tarpack-format`, `tarpack-build`, `tarpack-show-errors`,
`tarpack-progress`, `tarpack-build-result`, `tarpack-reveal`,
`tarpack-copy-report`, `tarpack-copy-command`, `tarpack-result-dismiss`.

### Shortcuts

Bind in the view's key context (`Tarpack`) with `gpui::actions!` and
`KeyBinding`: Ctrl+O → `Shortcut::Open`; F5 and Ctrl+R → `Reload`; Ctrl+E
→ `Edit`; Ctrl+Enter → `Build`; F8 → `ShowErrors`; Escape → `Escape`. Each
handler checks `model.shortcut(s, cx)` and calls `run_shortcut`; a held
key's auto-repeat never acts twice (ignore repeats: `KeyDownEvent::is_held`
or the action's repeat flag). Ctrl/Alt chords work with focus inside an
input. Call `set_shortcuts_open` from `ShortcutsHelp::on_open_change`.
Extend `SHORTCUT_ROWS` (U9) only if M7b's `text.rs` holds a different
list; the Ctrl+E row shows only in developer mode; Ctrl+Alt+Shift+D is never
listed.

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

Create: `apps/filemanager/src/tools/tarpack/view/{build_bar.rs,progress.rs,build_result.rs,report.rs,keys.rs}`.
Edit: `apps/filemanager/src/tools/tarpack/view/{mod.rs,shortcuts.rs,tests.rs}`,
`docs/gpui-test-ledger.md`.
Do not touch: model, text, fixtures, other tools, `crates/`, `apps/desktop/`,
`docs/plans/`.

## Skill

None here; U12 critiques the screenshots.

## Acceptance criteria

- The bar, dialog and result render every state from the model with copy
  from `text.rs`; the bar sits directly under the table/result and stays
  visible when the table scrolls.
- Every shortcut acts only when `shortcut()` allows; auto-repeat does not
  repeat; Escape does nothing while the help popover is open.
- Keyboard: every bar control, result button and copy is Tab reachable;
  the overwrite dialog starts on Cancel.
- Labels or tooltips everywhere; no colour literal outside `theme`.
- fmt; clippy pedantic `-D warnings`; `cargo test --workspace`.

## Tests proving completion

In `tools/tarpack/view/tests.rs` (`#[gpui::test]`, fixtures,
`FakePlatform`): `bar_shows_reason_or_note`, `choose_output_calls_model`,
`format_select_by_keyboard`, `build_click_and_ctrl_enter_build`,
`progress_renders_fraction`, `overwrite_dialog_replace_and_cancel`,
`result_success_partial_failure_render`, `copy_buttons_copy_and_show_copied`
(`FakePlatform` logs the copied text), `extraction_command_follows_dev_mode`,
`shortcuts_respect_gating`, `held_key_acts_once`,
`escape_ignored_while_help_open`, `help_lists_rows_without_dev_chord`.

Ledger: rows M7b marked U in `BuildBar.test.tsx`, `BuildResult.test.tsx`,
`BuildReport.test.tsx`, `FailureList.test.tsx`, `FormatPicker.test.tsx`,
`ShortcutsHelp.test.tsx`, `useTarpackShortcuts.test.tsx`,
`TarpackView.build.test.tsx`.

## States covered

Not buildable (reason), left-out note, ready, building with progress,
overwrite confirmation, success, partial success,
failure, developer mode on/off, help open.

**From the M7b review (2026-11-22).** Render `BuildOutcome::Unexpected` as the
error result with `text::UNEXPECTED` and its detail as Details. There is no
"Couldn't copy" state: GPUI's clipboard write reports no failure; the
"Copied" confirmation stays. A build result disappears when a manifest is
opened or created, as well as on Reload (the model clears it).

**Manual check due (M7a re-review, 2026-11-22).** Once this task's Reveal
button exists, the reveal half of Run 4 of `docs/gpui-dialogs-check.md`
(Explorer opens with the archive selected) becomes runnable. Note in the PR
that the hand check is due.

## Out of scope

Scenarios (U12); model behaviour (M7b).
