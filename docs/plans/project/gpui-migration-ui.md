# UI project plan: porting FileManager's interface to GPUI + gpui-component

Status: **approved (2026-11-21)**, as written, including V1–V8 and 25 objects per page (written 2026-11-21 by the `ui-designer`).
Backend project plan: [`gpui-migration.md`](gpui-migration.md) (its §13 is the
handoff this plan answers; D1–D26 there are binding). Task plans:
[`../tasks/gpui-migration/`](../tasks/gpui-migration/) `U1`–`U12`; the
Database Manager's GPUI tasks are in [`dbmanager-ui.md`](dbmanager-ui.md) §12
and [`../tasks/dbmanager/`](../tasks/dbmanager/).

**Method.** `impeccable` 4.9.0, `shape`, for a *port inside an established
world*. `impeccable context` loaded the root `PRODUCT.md` and `DESIGN.md`
(target `apps/desktop`, the only app it knows today). The structure and the
visual system were fixed by the human (D3, D7), so no world workshop or
concept roll ran: the discovery questions were answered from D1–D26 and the
spike report. No structured question tool reached the human; assumptions are
marked *(assumed)*. `critique` is used later, on generated screenshots only
(D16), by each scenario task.

**Evidence read.** `spike/gpui/REPORT.md`, `tokens.rs`, `theme.rs`,
`widgets.rs`, `pipeline_screen.rs`, `check.rs` on `spike/gpui-migration`;
`apps/desktop/src/styles/*.css`; the React views in `apps/desktop/src/`;
`docs/manual/images/*.png` (30) and `docs/manual/screenshots/shoot.js`,
`mock.js`; the backend task plans M3a–M13.

## 1. Design brief

1. **Job and audience.** Unchanged from `PRODUCT.md`: developers and testers
   who rebuild packages, run the pipeline, merge schedules and fix SQLite
   data many times a day, beside an editor and a terminal. **Mode: Operate.**
2. **Outcome and proof.** The user opens the GPUI exe and finds the same
   tool, in the same place, with the same words, colours and states as
   yesterday. Proof: every manual image regenerates from the GPUI app with
   the same layout and token colours (±1 per channel, geometry within about
   2 px of today's, text widths free: D7), and every action is still reachable
   by keyboard and labelled (D15).
3. **Selected direction.** **Port, not redesign.** "The Packing List"
   (`DESIGN.md`) stays: neutral tonal layers, one working blue, status
   colours only on status, 1 px rules, 6 px radius, Segoe UI with Cascadia
   Mono for data. Each React component is transcribed onto a gpui-component
   control or a small widget of ours in `apps/filemanager/src/widgets/`,
   styled from `theme/tokens.rs`. Where GPUI cannot match, the difference is
   named in §4.6 and accepted, never improvised around.
4. **Scope and boundaries.** Theme, shared widgets, shell views, the views of
   the four tools, and the screenshot scenarios. Untouched: product
   behaviour and copy (they live in each tool's `model.rs`/`text.rs`, owned
   by the backend `implementer`), the token values, the structure of every
   screen. Anti-goals: new chrome, a theme toggle, tinted banner fills,
   shadows at rest, decoration added "because GPUI can", any behaviour in a
   view that the model should own (a missing model need is a request to the
   planner, §8).
5. **States and ranges.** As today (each tool's parity checklist, §4): load,
   restore failure, empty, typed/browsed/dropped input, busy, confirmation,
   success, error with details, partial failure; tables from 0 to 10,000
   rows; Windows paths of any length, possibly with U+FFFD in their display
   form.
6. **Interaction and layout.** The two-column frame (12rem sidebar, the
   tool's view filling the rest); each view owns its scrolling; overlays are
   the confirmation dialog (modal, scrim), menus and the shortcuts popover
   (non-modal), and the drop overlay. Keyboard maps are in §5.
7. **Constraints.** Windows 11 only; light and dark follow the system, live,
   no toggle; min window 800×560; screenshots at scale 1, 1000×700 (1366×800
   for the Tar Packager and most Database Manager shots). Keyboard reachable
   and labelled (visible label, or a tooltip for an icon-only control); no
   screen-reader, axe, high-contrast or text-scaling work (D15, D20).
   Announcements the React views made in visually hidden live regions are
   **not rendered** (nothing a sighted user saw is lost) *(assumed; §9 V2)*.

## 2. Theme mapping

### 2.1 Tokens

`apps/filemanager/src/theme/tokens.rs` becomes the normative source (after
M9, `DESIGN.md` says so). It is the spike's `tokens.rs`, ported: one
`Colors` struct per theme (`LIGHT`, `DARK`), every `tokens.css` custom
property by the same name in `snake_case`, and the scales as constants.

| `tokens.css` | `tokens.rs` | Light | Dark |
|---|---|---|---|
| `--bg` | `Colors::bg` | `#f7f7f8` | `#16171a` |
| `--surface` | `surface` | `#ffffff` | `#1e2024` |
| `--surface-sunken` | `surface_sunken` | `#eef0f2` | `#121316` |
| `--border` | `border` | `#d9dce1` | `#33363d` |
| `--text` | `text` | `#1b1d21` | `#e8e9ec` |
| `--text-muted` | `text_muted` | `#5b616b` | `#a3a8b1` |
| `--accent` | `accent` | `#2457c5` | `#7aa2ff` |
| `--accent-text` | `accent_text` | `#ffffff` | `#0d1530` |
| `--ok` | `ok` | `#1d7a46` | `#5fd08f` |
| `--warn` | `warn` | `#8a5a00` | `#f2c14e` |
| `--danger` | `danger` | `#b3261e` | `#ff8a80` |
| `--focus-ring` | `focus_ring` | `#2457c5` | `#7aa2ff` |
| `--drop-overlay` | `drop_overlay: (Rgb, f32)` | `rgb(36 87 197) / 0.08` | `rgb(122 162 255) / 0.12` |
| `--shadow-overlay` | `shadow_overlay_alpha` (kept, **not applied**: D12) | 0.18 | 0.5 |
| `--scrim` | `scrim_alpha` | 0.4 | 0.6 |
| `--space-1…6` | `SPACE_1…SPACE_6` | 4, 8, 12, 16, 24, 32 px | same |
| `--radius` | `RADIUS` | 6 px | same |
| `--font-size-sm/md/lg/xl` | `FONT_SIZE_SM/MD/LG/XL` (rem) | 0.8125, 0.875, 1, 1.25 | same |
| `--font-ui` | `FONT_UI` | `"Segoe UI"` | same |
| `--font-mono` | `FONT_MONO` | `fm_app::fonts::mono_family(cx)`: Cascadia Mono, else Consolas, from the system (D28; nothing embedded) | same |
| body line height | `LINE_HEIGHT` | 1.4 | same |
| rem | `REM` | 16 px | same |

Derived values, computed once in `theme/mod.rs` (no hex anywhere else in
`src/`): **hover/active fill of primary and danger** =
`color-mix(in srgb, X 88%, --text)` (`Rgb::over(0.88, text)`); **65ch** =
546 px (`65 × 0.6 × 14 px`, the spike's approximation); **disabled button**
= `surface_sunken` fill, `text_muted` label, `border` outline, opacity 0.55;
**disabled quiet button** = transparent, opacity 0.45; **disabled input** =
opacity 0.6.

### 2.2 gpui-component slots

`theme::apply(mode, cx)`: `Theme::change(mode, None, cx)` then
`Theme::update(cx, …)` (a mode change reloads colours, so colours go in the
second call). The spike's mapping, kept slot for slot:

| Slots | Token |
|---|---|
| `font_family` / `mono_font_family` | `FONT_UI` / `fonts::mono_family(cx)` |
| `font_size` | **16 px** (the window rem; gap 8: body size is set on elements) |
| `mono_font_size` | 14 px |
| `radius`, `radius_lg` | 6 px |
| `shadow` | `false` (dialog and popover shadows stay gpui-kit's fixed ones: D12) |
| `background` | `bg` |
| `foreground`, `caret`, `popover_foreground`, `accent_foreground`, `button_foreground`, `secondary_foreground`, `sidebar_foreground`, `sidebar_accent_foreground` | `text` |
| `border`, `input`, `sidebar_border`, `table_row_border`, `window_border`, `title_bar_border`, `scrollbar_thumb` | `border` |
| `muted`, `accent`, `sidebar`, `button_hover`, `button_active`, `secondary_hover`, `secondary_active`, `list_hover`, `list_active`, `table_hover`, `table_active`, `scrollbar`, `title_bar` | `surface_sunken` |
| `muted_foreground`, `table_head_foreground`, `scrollbar_thumb_hover` | `text_muted` |
| `popover`, `button`, `secondary`, `sidebar_accent`, `colors.list`, `list_even`, `table`, `table_even`, `table_head` | `surface` |
| `list_head` | `bg` |
| `primary`, `button_primary`, `sidebar_primary`, `link`, `info`, `ring`, `progress_bar`, `drag_border`, `list_active_border`, `table_active_border` | `accent` (`ring` = `focus_ring`, equal values) |
| `selection` | `accent` at **alpha 0.3** (planner correction after the U1 review, 2026-11-22; confirmed by the ui-designer, 2026-11-23). Selected text keeps `text` (A10), and on full `accent` that is 2.61:1 light and 2.05:1 dark. At 0.3 over `surface` it is 10.55:1 and 7.61:1. gpui-component applies the same 0.3 cap itself, but only when it loads a theme config, which we bypass by writing slots directly. Today's CSS used `accent` with `accent_text` text, which GPUI cannot set |
| `primary_hover/active`, `button_primary_hover/active`, `link_hover/active` | accent mixed 88% with text |
| `primary_foreground`, `button_primary_foreground`, `sidebar_primary_foreground`, `info_foreground`, `success_foreground`, `warning_foreground`, `danger_foreground`, `button_danger_foreground` | `accent_text` |
| `danger`, `button_danger` | `danger`; hover/active: danger mixed 88% with text |
| `success` / `warning` | `ok` / `warn` |
| `overlay` (dialog scrim) | black at `scrim_alpha` |
| `drop_target` | `drop_overlay` |
| any slot not listed | unused: left at gpui-component's value, never shown by our widgets |

Note on the danger button: `DESIGN.md` plans an **outline** danger button
(`danger` label and border on `surface`, no fill; hover `surface_sunken`).
Our `widgets::button` draws that on the element; the `button_danger*` slots
above only cover gpui-component internals.

### 2.3 Set on elements (gap 9)

Input fill (`surface`, not the slot's `bg`); disabled opacities (§2.1);
read-only input (`surface_sunken`, no border, full `text`); text colour of a
selection (not settable: accepted, §4.6); focus-ring thickness and offset
(gpui-component's ring in `focus_ring`; accepted); the 2 px dashed drop
border; `65ch`; tabular numerals (§4.6).

### 2.4 Fonts, rem, live appearance

- Segoe UI is the system font (always present on Windows 11). The mono font
  is Cascadia Mono, falling back to Consolas, resolved from the installed
  fonts by M3a's `fm_app::fonts::mono_family(cx)` (D28, 2026-11-21: the
  earlier decision to embed Cascadia Mono was reversed; nothing is
  embedded).
- `Theme::font_size` stays 16 px; every text element sets its size from the
  four `FONT_SIZE_*` tokens (body = MD = 14 px, line height 1.4).
- `theme::init(window, cx)`, called from the chrome slot: applies the theme
  for `window.appearance()` and follows `window.observe_window_appearance`
  live (`Light`/`VibrantLight` → light, `Dark`/`VibrantDark` → dark).
- `theme::set_override(Option<ThemeMode>, cx)`: a global that, when set, wins
  over the system appearance. Only the screenshot harness calls it (`--dark`,
  request R3). There is no user toggle.

### 2.5 Tests (U1)

- `tokens_match_tokens_css`: parses `apps/desktop/src/styles/tokens.css` and
  compares every token, both themes (the spike's test). When the file is
  absent, the test asserts `apps/desktop/` is absent too (the cutover ran)
  and passes; it fails if the CSS disappeared while the Tauri app remains.
- `design_md_matches_tokens`: the `colors`, `rounded`, `spacing` and type
  sizes in `DESIGN.md`'s front matter equal `tokens.rs`. This keeps
  `DESIGN.md` honest after `tokens.css` is gone.
- `contrast_pairs_meet_4_5` (the cheap guard of `tokens.test.ts`, not an
  accessibility gate): `text` and `text_muted` on `bg`, `surface`,
  `surface_sunken`; `accent_text` on `accent`; `ok`, `warn`, `danger` on
  `surface`; `danger` on `surface_sunken`; both themes.
- `apply_sets_slots_from_tokens`, `override_wins_over_appearance`,
  `appearance_change_reapplies`.

## 3. Component inventory

Every interactive widget is keyboard reachable (Tab, plus Enter/Space for
buttons and checkboxes, with key-down **and** key-up in tests: gap 7) and
labelled. Element names follow `<scope>-<thing>` (`tarpack-build`,
`dialog-confirm`) and are listed in each task plan. Scenarios click and clip
only by `.tag("<name>")` (the harness's probe, `app/probe.rs`), called last
in the builder chain; view tests in `VisualTestContext` use
`.debug_selector(..)` with the same name (planner note, 2026-11-22, after
M4).

### 3.1 Shared widgets (`apps/filemanager/src/widgets/`, U2 and U3)

| Widget | File | Over | API (sketch) | States | Replaces |
|---|---|---|---|---|---|
| Text styles | `text.rs` | `div` | `h1`, `h2` (lg 600), `body`, `muted`, `small` (sm), `small_muted`, `mono`, `mono_small_muted`, `intro` (max 546 px); each returns a `Typo` element (as built, U2) | — | `.tool-view h1`, `*__intro`, `.mono`, `.num` |
| Icon | `icon.rs` | gpui-component `Icon` + our SVGs | `icon(IconName)` at 1em of the surrounding size, `currentColor` | — | `app/icons.tsx` |
| Button | `button.rs` | gpui-component `Button` | `button(id, label)`, `.primary()`, `.quiet()`, `.danger()`, `.icon(IconName)`, `.icon_flip()` (mirrors the icon), `.icon_only(IconName, tooltip)`, `.disabled(b)`, `.busy(b)` (gpui-component `Spinner`, not `.loading()`), `.shortcut(hint)` (tooltip "Shortcut: …"), `.on_click` | default (secondary), hover, active, focus, disabled, busy (the disabled colours at full opacity, plus a spinner; focusable, inert: U2 review, 2026-11-23), primary, quiet, danger | `app/Button.tsx` |
| Text input | `input.rs` | `Input` + `InputState` | `text_input(&state)`, `.mono()`, `.invalid(b)`, `.read_only(b)`, `.disabled(b)`; multiline is a separate `text_area(&Entity<TextareaState>)`, because gpui-component 0.10 has no multiline `InputState` (as built, U2) | default, focus, invalid, read-only (selectable), disabled, placeholder | `.pipeline__input`, `.schedule__input`, `.input` |
| Checkbox | `checkbox.rs` | `Checkbox` | `checkbox(id, label, CheckState)`; `CheckState::{Unchecked, Checked, NotSet}` with `next()`; "Not set" word beside | ticked, clear, not set, disabled, read-only | `.checkbox` |
| Select | `select.rs` | gpui-component `Select` | `select(id, label, options, selected)`; values are `SharedString` ids, the open/closed state is kept per id, `.on_pick(..)`, `.width(..)` (as built, U2) | closed, open, disabled | `FormatPicker`, allowed-values `<select>` |
| Tag | `tag.rs` | `div` | `tag(label, Option<IconName>)` | — | dbmanager tags |
| Banner | `banner.rs` | `div` | `Banner::new(Tone, message)`, `.id(..)`, `.action(button)`, `.dismiss(handler)`, `.details(text)`; `Tone::{Info, Warn, Error}` | info, warn, error, with details open/closed | `app/Banner.tsx` |
| Disclosure | `disclosure.rs` | `div` + `Button` | `Disclosure::new(id, summary).open(b).on_toggle(..)`, `.debug_selector(..)` | open, closed | `<details>` |
| Status | `status.rs` | `div` | `status(StatusTone, IconName, word)`; tones Ok, Warn, Danger, Muted | — | `.pipeline__status--*`, `EntryStatus` |
| Step number | `step_number.rs` | `div` | `step_number(n)` | — | `.pipeline__step-number`, `.schedule__step-number` |
| Skeleton | `skeleton.rs` | `div` | `skeleton_bar(fraction, wide)` | — | `.skeleton__bar` |
| Middle path | `middle_path.rs` | custom element | `MiddlePath::new(display)`; pure `middle_truncate(text, max_chars)`: the result is exactly `max` characters when truncated, at least 3 head characters, the separator stays with the name, and the whole name is kept while it fits (as built, U2); mono advance from the text system; full path as tooltip | fits, truncated | `MiddlePath.tsx`, `pathParts.ts` |
| Key cap | `kbd.rs` | `div` | `kbd(key)`, `chord(&[&str])` | — | `<kbd>` |
| Confirm dialog | `dialog.rs` | `window.open_dialog` | `ConfirmSpec { title, body, extra, cancel_label, confirm: Option<ConfirmButton{label, Danger/Primary, icon}>, busy, wide }`; `sync_confirm(window, cx, Option<ConfirmSpec>, on_confirm, on_cancel)` opens/updates/closes from model state | open, busy (Escape and Cancel inert), single-button, wide, closed | `app/ConfirmDialog.tsx` (+ planned extension) |
| Menu button | `menu_button.rs` | `Button::dropdown_menu` | `MenuButton::new(id, label).items(Vec<MenuItem{label, detail, tooltip, disabled}>).on_select(..)` | closed, open, empty (not rendered), item disabled | Recent menu, planned `MenuButton` |
| Shortcuts help | `shortcuts_help.rs` | `Popover` | `ShortcutsHelp::new(id, rows).on_open_change(..)`; `ShortcutRow { keys: Vec<Vec<&str>>, action, when }` | closed, open | `ShortcutsHelp.tsx` |

Sizes (from `controls.css`): button padding 4/12 px, 1 px border, radius 6,
body size, height 30 px (the spike's measured value); input min height
32 px, padding 4/8; dialog 448 px wide (wide: 640 px), padding 24, title lg
600, actions right-aligned with gap 8; menu list min 256 px, max 512 px,
items sm with a muted second line, right-aligned under its trigger, the
selected row a 2 px `focus_ring` border inside the row on `surface_sunken`;
popover padding 12/16; shortcuts help right-aligned under its button, its
table with no row rules and self-sizing columns (U3 review, 2026-11-23); banner padding
8/12, gap 8/12, 1 px border in the tone colour on `surface`, tone icon, the
message in `text`, quiet "Dismiss" (icon-only, tooltip "Dismiss") at the
right; tag sm, `text_muted`, 1 px border, radius 6, padding 0/4; kbd mono sm
on `surface_sunken`, 1 px border, padding 0/4.

### 3.2 Shell views (`apps/filemanager/src/app/view/`, U4)

| View | File | Renders | States |
|---|---|---|---|
| Chrome | `mod.rs` | `theme::init`; the two-column frame; the notice; the drop overlay; the close prompt | — |
| Sidebar | `sidebar.rs` | one nav item per `Shell::tools()` | current (surface fill, border, 600, 6 px accent dot), hover (surface), focus |
| Notice | `notice.rs` | `Shell::notice()` as a warn `Banner` at the top of the tool area, dismiss → `Shell::dismiss_notice` | none, shown |
| Drop overlay | `drop_overlay.rs` | `Shell::drag_over()`: full-window `drop_overlay` fill, 2 px dashed accent border inset 8 px, a surface plate (lg) with `folder` + label, or `info` + reason | hidden (also when the current tool's target is `DropTarget::None`: no overlay is drawn at all, confirmed and tested in U4, 2026-11-23), enabled, disabled (also while a modal dialog is open: the Shell reports Disabled with "Close the open dialog first." and refuses the drop; the human's decision, 2026-11-21) |
| Close prompt | `close_prompt.rs` | `Shell::close_request()` as a confirm dialog | hidden, shown |

### 3.3 Tool views

Per tool, in its `view/` folder; the inventories are in each task plan and,
for the Database Manager, in `dbmanager-ui.md` §12.2.

| Tool | Views | Task |
|---|---|---|
| Pipeline | `PipelineView` (header, load failure, folder field, error banner, run bar, steps list with results, run confirmation) | U5 |
| Schedule Creator | `ScheduleView` (header, banners, two file steps, apply row, result banner, apply confirmation) | U7 |
| Tar Packager | frame and states, `ManifestHeader` + Recent menu, banners, empty state, `ManifestErrors` region (`DiagnosticList`), `DropResult` | U9 |
| Tar Packager | `EntryTable` (`DataTable`), `EntryStatus`, `EolMarker`, `MiddlePath` cells | U10 |
| Tar Packager | `BuildBar`, `FormatPicker`, `BuildProgress`, overwrite dialog, `BuildResult`, `BuildReport`, `FailureList`, shortcuts and `ShortcutsHelp` | U11 |

## 4. Parity checklists

Geometry is at 1000×700 unless stated; px are logical. "Copy" names the
source the view takes its words from. Every list below is the acceptance
reference for its tool's view and scenario tasks.

### 4.1 Shell

- Window ground `bg`. Sidebar 192 px (12rem, at most 40% of the window),
  `surface_sunken`, 1 px `border` on its right, padding 12/8, items stacked
  with a 4 px gap, full width, padding 8/12, radius 6, 1 px transparent
  border, label in `text` at body size.
- Current item: `surface` fill, `border` outline, weight 600, a 6 px `accent`
  dot 8 px before the label. Hover: `surface` fill. No side stripe.
- Tool area: fills the rest; the view inside pads 16/24 and owns its
  scrolling.
- Order and labels: Pipeline, Tar Packager, Schedule Creator, then Database
  Manager (registry). Copy: the registry labels.
- Keyboard: Tab reaches the current nav item first, then the view; Up/Down
  move focus between nav items and wrap; Home/End jump; Enter/Space select.
  Ctrl+Alt+Shift+D toggles developer mode anywhere (the Shell's binding).
- Drop overlay, notice and close prompt as §3.2; close prompt copy: title
  "Close FileManager with unsaved changes?", one line per
  `CloseRequest` item, **Keep editing** (focused) and **Close without
  saving** (danger). It is the only shell copy the views own (M3b).
- Developer mode has no visible surface of its own, as today (§9 V3).

### 4.2 Pipeline (1000×700; reference `pipeline-*.png`)

- Header: h1 "Pipeline" (xl, 700), intro (muted, max 546 px), 4 px apart.
  Regions 16 px apart.
- Load failure: error banner with **Try again**; nothing else.
- Folder: title "Folder" (lg 600); row: mono input (flex, min 32 px high)
  and **Browse…** 12 px apart; under it the note (muted: "12 subfolders")
  or the field error (`danger`) and the input's `danger` border.
- Error banner (model `error()`), re-shown on a new `error_generation`.
- Run bar: **Run next: <step>** (primary) and **Run all…**, 12 px apart;
  the reason or "All steps passed." in muted beside them.
- Steps: a list with a 1 px rule above and under each row; row padding 12/0;
  head = step number (24 px circle, `surface_sunken`, 1 px border, sm), the
  title (lg 600) and description (muted), the lock line (sm muted "Runs after
  <title> passes."), the status (min 144 px: icon + word in its tone:
  Passed ok/check-circle, Problems warn/alert-triangle, Failed and Not
  implemented danger/x-circle, Running muted with busy, Waiting and Not run
  muted/circle), and **Run** or **Run…** (confirm). Results indented 36 px:
  summary, then the issues list.
- Confirmation: modal, title and body from `text.rs` naming the folder,
  **Cancel** focused, confirm primary.
- Keyboard: Enter in the folder input commits; Escape reverts unsaved
  typing; Tab order header → input → Browse → run bar → each step's Run.
  After a run, focus follows `focus_hint` (the step row, or Run next).
- Copy: `tools/pipeline/text.rs`.

### 4.3 Schedule Creator (1000×700; reference `schedule-*.png`)

- Header and intro as the Pipeline; error banner; load failure with Try
  again.
- Two steps 16 px apart, each: title row (step number + lg 600 label), the
  file row (mono input + **Browse…**), the field error in `danger`.
- Apply row: 12 px top padding and a 1 px rule above; **Add to XML…**
  (primary; busy "Adding…"), the reason in muted beside it.
- Result: info banner with the result sentence and the backup path line,
  dismissible (`dismiss_result`).
- Confirmation: modal naming both paths; confirm "Update"; when the same
  pair was applied before, the duplicate warning is part of its body, as
  today (planner correction, 2026-11-22: not a banner beside the result).
- Drop: overlay label/reason from the model's drop target.
- Keyboard as the Pipeline; focus after apply follows `focus_hint`.
- Copy: `tools/schedule/text.rs`.

### 4.4 Tar Packager (1366×800; reference `tarpack-*.png`)

- View padding 16/24; regions 12 px apart; whole view on `bg`.
- No manifest: title bar (h1 "Tar Packager" + **Keyboard shortcuts**), the
  empty state (max 546 px: sentence, **Open manifest…** primary and
  **Create from example…**, muted hint), the Recent list when there is one.
- Header: h1 = manifest name; under it the manifest path, mono sm muted,
  middle-truncated; actions right: **Open…**, **Recent** (menu button),
  **Reload**, **Edit in editor** (developer mode), **Keyboard shortcuts**
  (quiet, keyboard icon). Actions wrap under the title when narrow.
- Banners in order: command error, changed on disk (warn, **Reload**),
  state warning (info, as today; dismissible). Skeleton (two bars) only after 150 ms
  of loading.
- Errors region: `surface`, 1 px `danger` border; heading (lg 600) with
  x-circle, consequence line, **Show errors**/**Hide errors**; the report
  (diagnostics: position mono muted, message) scrolls within 40% of the view
  height. Warnings: a bordered disclosure with a warn icon.
- Drop result: bordered `surface` box, lines with tone icons, a "paths"
  disclosure, quiet Dismiss.
- Entry table (§4.6 D13): a ruled sheet on `surface`: header row sm 600
  muted with 1 px rules above and below; rows with a 1 px rule under; row
  hover and focus `surface_sunken`; no side borders or radius. Columns:
  Status 116 px, File, Windows location, Linux target, Mode 132 px, Owner
  80 px, Actions 176 px; File/Windows/Linux share the rest equally at table
  widths ≥ 1024 px and keep their 1024 px widths (≈ 173 px each) below,
  where the table **scrolls sideways**. Row height fixed at two text lines
  plus 16 px padding (56 px). Status: icon + word (Ready ok, Missing danger,
  Unassigned muted). File: name, plus the `CRLF → LF` marker (mono, muted,
  `surface_sunken`, bordered). Windows location: `MiddlePath`, mono; a note
  in `danger` under it when missing. Linux target: mono, up to two lines.
  Mode: symbolic over octal (muted), mono. Owner. Actions: quiet **Browse…**
  and **Clear**.
- Build result between the table and the bar: `surface`, 1 px border,
  padding 12; heading lg 600 with a tone icon; path, facts, hint (sm muted);
  copy rows with a **Copy** button and a "Copied" label; the extraction
  command (developer mode) as a `surface_sunken` mono block; the report
  under a 1 px rule, scrolling within 40% of the view height.
- Build bar: full width under the table, `surface`, 1 px rule above,
  padding 12/24; left: "Output:" (sm muted), the output `MiddlePath`, mono
  sm, **Choose…** with folder icon, "Format" + the format select; right: the
  status (reason sm muted, left-out note with warn icon, or progress: label
  sm, a 6 px `surface_sunken` track with an `accent` fill, percent sm) and
  **Create archive** (primary). Wraps onto two lines when narrow.
- Overwrite dialog: title, `overwriteBody` naming the file, **Cancel**
  focused, **Replace** primary.
- Keyboard: §5.3.
- Copy: `tools/tarpack/text.rs`.

### 4.5 Database Manager

`dbmanager-ui.md` §12 (its GPUI parity list) is the reference.

### 4.6 Accepted differences (all screens)

| # | Difference | Source |
|---|---|---|
| A1 | Dialog, menu and popover shadows are gpui-kit's fixed ones, not `--shadow-overlay` | D12 |
| A2 | The Tar Packager table never stacks rows; below 1024 px it scrolls sideways | D13 |
| A3 | Rows have one fixed height (two lines). A focused row no longer expands to show its full Windows path: the full path is the cell's tooltip; a Linux target longer than two lines is middle-truncated with its tooltip | gap 5, D13 |
| A4 | Focus returns to the opener after a dialog only when the opener had focus (opened from the keyboard); a mouse press does not focus a button | gap 7 |
| A5 | Text renders with DirectWrite and Segoe UI (not Chromium/Inter); widths differ, colours and boxes do not | D7, gap 10 |
| A6 | Middle truncation is ours, exact for mono text (all truncated paths are mono) | gap 6 |
| A7 | Plain text is not selectable. Values the user copies have Copy buttons (report, SHA-256, extraction command, paths in the build result); read-only form values are read-only inputs, which are selectable | risk 6 |
| A8 | `tabular-nums`: applied through GPUI font features (`tnum`) where the pinned GPUI exposes them on text; otherwise proportional digits | risk 6 |
| A9 | The focus ring is gpui-component's, in `focus_ring`; its thickness and offset may differ from 2 px / 2 px | gap 9 |
| A10 | The selection's text colour is not set (only its fill), so the fill is `accent` at alpha 0.3 to keep `text` readable (2026-11-22) | gap 9 |
| A11 | The drop border is dashed if the pinned GPUI has a dashed border style; otherwise a solid 2 px accent line | risk 6 |
| A12 | gpui-component's dialog close icon ("Close") is hidden where the API allows it; otherwise it stays, labelled | B8 |
| A13 | Motion: no fade-in on the drop overlay and no progress-fill transition (they were 120 ms) | — |
| A14 | Height-based rules for 200% text (the Tar Packager's static build bar under 30rem, the Database Manager's whole-view scroll under 34rem) are dropped: at 100% the 560 px minimum window never reaches them | D15, D20 |
| A15 | Visually hidden live-region announcements are not rendered | D15 |

## 5. Keyboard maps

### 5.1 Shell

| Keys | Action | When |
|---|---|---|
| Up, Down, Home, End | Move between tools | focus in the sidebar |
| Enter, Space | Show the focused tool | a nav item focused |
| Ctrl+Alt+Shift+D | Developer mode on/off (never listed in help) | always |

### 5.2 Pipeline and Schedule Creator

Tab order as §4.2/§4.3; Enter commits a typed path; Escape reverts unsaved
typing in a path field; in a dialog Escape cancels and Tab cycles. No other
shortcuts, as today.

### 5.3 Tar Packager

Bound in the view's key context, gated by `TarpackModel::shortcut(s, cx)`,
run with `run_shortcut`; held keys do not repeat an action:

| Keys | Action | When |
|---|---|---|
| Ctrl+O | Open manifest… | always |
| F5 or Ctrl+R | Reload manifest | a manifest is open |
| Ctrl+E | Edit in editor | developer mode, a manifest is open |
| Ctrl+Enter | Create archive | the archive can be built |
| F8 | Show errors | the manifest has errors |
| Up, Down | Move between files | focus in the table |
| Enter | Browse… for the focused file | a row focused |
| Delete | Clear the focused file | the row has a file |
| Esc | Dismiss the drop result, else the build result | a result shows; not while the shortcuts popover is open |

### 5.4 Database Manager

`dbmanager-ui.md` §3.11, with §12's GPUI notes.

## 6. Screenshot scenarios

Each scenario is `apps/filemanager/src/shoot/scenarios/<tool>.rs`, named as
the PNG stem, with a fixture state from the tool's `fixtures.rs` and the
`FakePlatform`. Selectors are the `.tag(..)` names of §3 and the task plans.

| Image | Size | Fixture | Steps |
|---|---|---|---|
| `pipeline-empty` | 1000×700 | `empty()` | WaitIdle |
| `pipeline-chosen` | 1000×700 | `chosen()` (12 subfolders) | WaitIdle |
| `pipeline-bad` | 1000×700 | `bad()` | WaitIdle |
| `pipeline-runall` | 1000×700 | `chosen()` | Click `pipeline-run-all`, WaitIdle |
| `pipeline-running` | 1000×700 | `chosen()`, Verify takes 30 s | Click `pipeline-run-next`, Wait 600 |
| `pipeline-ran` | 1000×700 | `chosen()`, Verify answers Not implemented | Click `pipeline-run-next`, WaitIdle |
| `schedule-empty` | 1000×700 | `empty()` | SelectTool, WaitIdle |
| `schedule-ready` | 1000×700 | `ready()` | SelectTool, WaitIdle |
| `schedule-bad` | 1000×700 | `bad()` | SelectTool, WaitIdle |
| `schedule-confirm` | 1000×700 | `ready()` | SelectTool, Click `schedule-apply`, WaitIdle |
| `schedule-notimpl` | 1000×700 | `ready()`, apply → ParseNotImplemented | …confirm, Click `dialog-confirm`, WaitIdle |
| `tarpack-empty` | 1366×800 | empty | SelectTool, WaitIdle |
| `tarpack-loaded` | 1366×800 | loaded | SelectTool, WaitIdle |
| `tarpack-recent` | 1366×800 | loaded | Click `tarpack-recent`, WaitIdle |
| `tarpack-shortcuts` | 1366×800 | loaded | Click `tarpack-shortcuts`, WaitIdle |
| `tarpack-drag` | 1366×800 | loaded | DragEnter `C:\Builds\gateway` |
| `tarpack-dropped` | 1366×800 | loaded | Drop the four gateway paths, WaitIdle |
| `tarpack-partial` | 1366×800 | partial | WaitIdle |
| `tarpack-ready` | 1366×800 | ready | WaitIdle |
| `tarpack-buildbar` | 1366×800, **clip** `tarpack-build-bar` | ready | WaitIdle |
| `tarpack-building` | 1366×800 | ready, slow build | Click `tarpack-build`, Wait until progress shows |
| `tarpack-result` | 1366×800 | ready | Click `tarpack-build`, WaitIdle |
| `tarpack-result-partial` | **clip** `tarpack-build-result` | partial | Click `tarpack-build`, WaitIdle |
| `tarpack-replace` | 1366×800 | ready, output exists | Click `tarpack-build`, WaitIdle |
| `tarpack-errors` | 1366×800 | errors | WaitIdle |
| `tarpack-withheld` | 1366×800 | withheld | WaitIdle |
| `tarpack-missing` | 1366×800 | missing | WaitIdle |
| `tarpack-changed` | 1366×800 | ready | Watcher(Tarpack), WaitIdle |
| `tarpack-dev` | **clip** `tarpack-header` | ready | Keys `ctrl-alt-shift-d`, WaitIdle |
| `tarpack-dev-result` | **clip** `tarpack-build-result` | ready | Keys `ctrl-alt-shift-d`, Click `tarpack-build`, WaitIdle |

Four of today's images are crops of one region (`tarpack-buildbar` 1159×54,
`tarpack-dev` 1111×50, `tarpack-dev-result` 1111×333,
`tarpack-result-partial` 1111×359). The harness of M4 has no crop:
**request R1**. The Database Manager's 25 scenarios are in `dbmanager-ui.md`
§9 and its U14.

## 7. UI task index

| # | Task plan | Goal | Depends on |
|---|---|---|---|
| U1 | [`U1-theme.md`](../tasks/gpui-migration/U1-theme.md) | `theme/`: tokens, gpui-component mapping, live light/dark, tests | M3a |
| U2 | [`U2-widgets-controls.md`](../tasks/gpui-migration/U2-widgets-controls.md) | Shared controls: text styles, icon, button, input, checkbox, select, tag, banner, disclosure, status, step number, skeleton, middle path, kbd | U1, M3a |
| U3 | [`U3-widgets-overlays.md`](../tasks/gpui-migration/U3-widgets-overlays.md) | Shared overlays: confirm dialog (extended), menu button, shortcuts help | U2 |
| U4 | [`U4-shell.md`](../tasks/gpui-migration/U4-shell.md) | Chrome: sidebar and current marker, tool area, notice, drop overlay, close prompt | U3, M3b |
| U5 | [`U5-pipeline-views.md`](../tasks/gpui-migration/U5-pipeline-views.md) | Pipeline views | U4, M5 |
| U5b | [`U5b-pipeline-followups.md`](../tasks/gpui-migration/U5b-pipeline-followups.md) | U5 review follow-ups: tags on banner actions, optional accessible label, hidden-view focus, cleaned folder text, long names | U5 |
| U6 | [`U6-pipeline-scenarios.md`](../tasks/gpui-migration/U6-pipeline-scenarios.md) | Six Pipeline scenarios; `shell-sample` removed | U5, M4 |
| U7 | [`U7-schedule-views.md`](../tasks/gpui-migration/U7-schedule-views.md) | Schedule Creator views | U4, M6 |
| U8 | [`U8-schedule-scenarios.md`](../tasks/gpui-migration/U8-schedule-scenarios.md) | Five Schedule scenarios | U7, U6, M4 |
| U9 | [`U9-tarpack-frame-header.md`](../tasks/gpui-migration/U9-tarpack-frame-header.md) | Tar Packager frame, states, header, Recent, banners, empty state, errors region, drop result | U4, M7a |
| U9b | [`U9b-tarpack-header-followups.md`](../tasks/gpui-migration/U9b-tarpack-header-followups.md) | U9 review follow-ups: trailing chevron flip, Edit in editor in the errors region, open ledger rows | U9, after U10 |
| U10 | [`U10-tarpack-entry-table.md`](../tasks/gpui-migration/U10-tarpack-entry-table.md) | The entry table (scrolls inside itself; the body does not scroll: decided 2026-11-23) | U9, M7a |
| U11 | [`U11-tarpack-build.md`](../tasks/gpui-migration/U11-tarpack-build.md) | Build bar, progress, overwrite, result, report, developer extras, shortcuts and help | U10, M7b |
| U12 | [`U12-tarpack-scenarios.md`](../tasks/gpui-migration/U12-tarpack-scenarios.md) | 19 Tar Packager scenarios | U11, U8, M4, R1 |

Order with the backend (gpui-migration §9.2): U1, U2, U3 after M3a; U4
after M3b; U5 after M5, U6 after M4; U7 after M6; U9–U11 after M7a/M7b.
M8 waits for U1–U12. The Database Manager's tasks (dbmanager U2–U12, U14)
need U3 and U4 and their M12 parts; M13 waits for them.

Every U task fills its rows of `docs/gpui-test-ledger.md` (created in M3a):
`ported` to a named Rust test, or `dropped: <reason>`.

## 8. Requests to the planner

- **R1 Crop in the harness (M4, M8).** Add `clip: Option<&'static str>` to
  `shoot::Scenario`: when set, the PNG is cropped to that tag's (`.tag(..)`; originally "debug selector's")
  bounds (plus nothing). Four manual images are crops (§6). M8's acceptance
  "every PNG is the harness output at the scenario's size" then reads "…at
  the scenario's size or clip".
- **R2 Icon files are visual (M3a ownership).** Let the ui-implementer add
  SVGs to `apps/filemanager/assets/icons/` and their names to the `icons!`
  list in `src/assets.rs` (one line per icon). The Database Manager needs ten
  more (`arrow-left`, `arrow-right`, `lock`, `plus`, `trash`, `search`,
  `chevron-left`, `chevron-right`, `undo`, `check`; dbmanager U2).
- **R3 Dark screenshots (M4).** The harness's `--dark` calls
  `fm_app::theme::set_override(Some(ThemeMode::Dark), cx)` before the first
  frame (U1 provides it), rather than forcing the window appearance.
- **R4 Database Manager editor and picker API (M12b, M12c).** The views
  need: `pub enum EditorInput { Text(String), Bool(Option<bool>), Choice(Option<String>) }`
  with `edit(field, EditorInput, cx)` and `edit_link_field(relation, other,
  field, EditorInput, cx)`; a reference picker held in the model
  (`open_picker(field)`, `picker() -> Option<&PickerState>` with search
  text, items, total, loading, error, `picker_search(text)`, `picker_more()`,
  `pick(key)`, `clear_reference(field)`, `close_picker()`), using its own
  list query (it must not disturb the table's list state); and
  `request_delete_of(table, key, label, cx)` for the Delete key on a list
  item that is not the open object.
- **R5 Consumable focus hints (M5, M6, M7a, M12a).** `focus_hint()` returns
  the last hint for ever, so a repeated hint (Run next twice) cannot be told
  from an old one. Replace it with `take_focus_hint(&mut self) ->
  Option<FocusHint>` (cleared on read). The Database Manager's model needs
  hints for: `SheetHeading`, `ListHeading`, `ErrorSummary`, `ErrorBanner`,
  `PickerTrigger(field)`, `DeleteNotice`.
- **R6 Scenario fixtures (M12b–M12d).** For the Database Manager's
  scenarios: a config edit that makes book `pages` read-only (for
  `dbmanager-config-reloaded`); a config where `publisher` is editable with
  `allow_delete = true` and is still referenced (`dbmanager-delete-blocked`);
  a config with one error and one warning (`dbmanager-diagnostics`); a
  config with a syntax error (`dbmanager-readonly-config`).
- **R7 A close request for scenarios (M3b).** `Shell::request_close(window,
  cx)`: the same path as the window's should-close hook, so the
  `dbmanager-close-prompt` scenario (and view tests) can raise the prompt.

## 9. Decisions made here (the human may veto)

- **V1** The danger button is the outline variant `DESIGN.md` planned
  (`danger` label and border, no fill), not the spike's filled one.
- **V2** Announcements (visually hidden today) are not rendered. *(Planner note, 2026-11-22: so the Tar Packager announcer race fix, `tasks/tarpack/U8-announce-race-fix.md`, applies to the Tauri frontend only and ends at the cutover; the GPUI port has no announcer to carry it.)*
- **V3** Developer mode has no visible indicator, as today.
- **V4** The path-loss notice is a warn banner at the top of the tool area,
  above the current tool's view, until dismissed.
- **V5** Tar Packager rows are a fixed two lines high; the full Windows
  path of a row is its cell's tooltip (A3).
- **V6** Tar Packager layout: the table takes its content height and
  shrinks to scroll inside itself when the window is short; header,
  banners and errors sit above it, the result and the build bar below it, so
  the bar stays visible (today's sticky bar, without sticky positioning).
- **V7** The Tar Packager's Recent menu and shortcuts help use the shared
  `MenuButton` and `ShortcutsHelp` (the React tool-local copies are not
  ported as such).
- **V8** Database Manager tasks stay in `docs/plans/tasks/dbmanager/` under
  their old numbers (rewritten in place); U1 and U13 are superseded (merged
  into gpui-migration U2/U3/U4 and dbmanager U6); a new U14 holds its
  scenarios.

## 10. `DESIGN.md` and `PRODUCT.md` for M10

Applied by the ui-designer when M9 lands (with M10), not before: until then
they describe the shipping Tauri app.

`DESIGN.md`:

- Header: "Generated from `apps/filemanager/src/theme/tokens.rs` (normative)
  and `src/widgets/`." Every "Landed (U*n*)" / "Planned" tag is replaced by
  the GPUI facts.
- Elevation: the overlay shadow is gpui-kit's fixed one (D12); the scrim
  keeps `--scrim`.
- Typography: Segoe UI and Cascadia Mono (else Consolas), all system fonts; sizes as
  tokens; the rem Rule becomes "sizes come from the four type tokens; no
  other size".
- Layout: the Tar Packager table scrolls sideways under 1024 px and never
  reflows; rows are two lines high.
- Components: the widget names of §3.1 replace the React names; the danger
  button is landed; MenuButton and ShortcutsHelp are shared and used by the
  Tar Packager.
- Do's and Don'ts: "take every colour from `theme/tokens.rs`"; drop
  `::selection`/`scrollbar-color`/`prefers-reduced-motion` wording; keep the
  rest.

`PRODUCT.md`:

- Platform: "desktop (native, GPUI)"; "Windows 11 only".
- Capabilities: no webview or CSP; "all filesystem work happens in Rust; the
  views render each tool's model"; paths: display strings only for display;
  dialogs are the Windows file picker with filters; a dropped name that is
  not valid Unicode is reported.
- Accessibility & Inclusion: "Every interactive element is keyboard
  reachable and labelled (a visible label, or a tooltip for an icon-only
  control). Nothing is conveyed by colour or icon alone. Screen-reader
  support, axe-style audits, high contrast and text scaling are not
  requirements." (D15)

## 11. Open questions

None that block. The vetoable decisions are in §9.
