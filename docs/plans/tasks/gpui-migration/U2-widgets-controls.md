# U2 — Shared widgets, part 1: the controls

Status: approved (2026-11-21)
Project: gpui-migration   Depends on: U1 (landed), M3a (landed)

## Goal

Build `apps/filemanager/src/widgets/`, the shared controls every tool view
uses, transcribed from today's React components and CSS onto gpui-component
and styled only from `crate::theme`, each with a view test of its keyboard
behaviour and label.

## Context

FileManager is moving from Tauri + React to GPUI + gpui-component. The
human decided: **looks like today** (same layout and token colours; text
rendering may differ); **every interactive element is keyboard reachable
and labelled** (a visible label, or a tooltip for an icon-only control); no
screen-reader or text-scaling work. Visual system "The Packing List": flat
tonal layers, 1 px `border` rules, 6 px radius, one accent for the primary
action, current place and focus; `ok`/`warn`/`danger` only on status, always
with an icon and a word.

What exists: `crate::theme` (U1: `palette(cx)` with every token as `Hsla`
plus `primary_hover`, `danger_hover`; `tokens::{SPACE_1..6, RADIUS,
FONT_SIZE_SM/MD/LG/XL, REM, LINE_HEIGHT}`; `INTRO_MAX_WIDTH` 546 px;
`DISABLED_OPACITY` 0.55, `QUIET_DISABLED_OPACITY` 0.45,
`INPUT_DISABLED_OPACITY` 0.6; gpui-component slots mapped), `crate::fonts::mono_family(cx)` (Cascadia Mono, else Consolas, from the system),
`crate::assets::{icon_path, ICON_NAMES}` (icons `check-circle`,
`alert-triangle`, `circle`, `x-circle`, `info`, `chevron-down`, `folder`,
`copy`, `x`, `keyboard`; 16 px grid, 1.5 px stroke, `currentColor`).
The spike shows the gpui-component calls that work
(`git show origin/spike/gpui-migration:spike/gpui/src/widgets.rs` and
`…/pipeline_screen.rs`): `Button::new(id).label(..).primary()/.danger()/.disabled()/.loading()`,
`Input::new(&state)` (it is `Styled`: `.bg`, `.border_color`, `.font_family`;
`Input::h()` is the multi-line height, not `Styled::h`), `InputState::new(window, cx)`,
`.placeholder`, `set_value`, `set_readonly`, `Checkbox::new(id).checked(b).indeterminate(b).on_change(..)`,
`Icon::empty().path(icon_path(name)).with_size(..)`, `h_flex`/`v_flex`, `ActiveTheme`.
Confirm other names (Select, Tooltip) in the pinned gpui-component 0.10.0
source.

Facts that bind: a mouse press does not focus a gpui-component `Button`;
Enter/Space activate a Button or Checkbox only on key-down **plus** key-up,
so tests send both events (`KeyDownEvent` then `KeyUpEvent`, or
`simulate_keystrokes` if it sends both); debug selectors exist only in
debug builds (tests are debug builds).

### The widgets (today's look, from `controls.css`, `base.css`)

All text defaults: body size 14 px (`FONT_SIZE_MD × REM`), line height 1.4,
colour `text`.

- **`text.rs`**: `h1(text)` (xl = 20 px, weight 700), `h2(text)` (lg 16 px,
  600), `body`, `muted` (text_muted), `small` (sm 13 px), `small_muted`,
  `mono` (`mono_family(cx)`, 14 px), `mono_small_muted`, `intro(text)` (muted,
  max width `INTRO_MAX_WIDTH`), `tabular(el)` (applies the `tnum` font
  feature if the pinned GPUI exposes font features on text; else a no-op
  with a doc comment saying so).
- **`icon.rs`**: `enum IconName` (the ten names; `as_str`), `icon(name)`
  sized 1em of the surrounding text (14 px at body size) in the current
  text colour.
- **`button.rs`**: `button(id, label) -> FmButton`, a builder over
  gpui-component `Button`: min 24×24, padding 4/12 px, 1 px border, radius
  6, body size, height 30 px. Variants: **secondary** (default: `surface`
  fill, `border` outline, `text`; hover/active `surface_sunken`),
  **primary** (`accent` fill and border, `accent_text`; hover/active
  `primary_hover`), **quiet** (transparent, no border until hover:
  `surface_sunken`), **danger** (`danger` label and 1 px `danger` border on
  `surface`, **no fill**; hover/active `surface_sunken`; never primary).
  `.icon(IconName)` before the label with a 4 px gap; `.icon_only(IconName,
  tooltip)` (no visible label: the tooltip is mandatory, the type requires
  it); `.disabled(b)` (`text_muted` on `surface_sunken`, `border`, opacity
  0.55; a disabled quiet button stays transparent at 0.45); `.busy(b)`
  (inert, full opacity, label unchanged, gpui-component's loading
  indicator); `.shortcut("Ctrl+O")` (tooltip "Shortcut: Ctrl+O", appended
  to any tooltip); `.tooltip(text)`; `.on_click(..)`; `.debug_selector(..)`.
- **`input.rs`**: `text_input(&Entity<InputState>) -> FmInput`: `surface`
  fill (set on the element: the slot is not it), 1 px `border`, radius 6,
  min height 32 px, padding 4/8, placeholder `text_muted`. `.mono()`,
  `.invalid(b)` (`danger` border), `.read_only(b)` (`surface_sunken`, no
  border, full `text`, still focusable and selectable),
  `.disabled(b)` (opacity 0.6), `.multiline(rows)`.
- **`checkbox.rs`**: `enum CheckState { Unchecked, Checked, NotSet }` with
  `next()` (Unchecked → Checked → NotSet → Unchecked: the nullable boolean
  cycle) and `checkbox(id, label, state)`: gpui-component `Checkbox` (1rem,
  checked colour `accent`), `.indeterminate` for `NotSet`, the visible
  label after it, and the word "Not set" beside it in `text_muted` when
  `NotSet`; `.disabled(b)`, `.on_change(..)`.
- **`select.rs`**: `select(id, label, options: Vec<(SharedString, value)>, selected)`
  over gpui-component's select control: looks like a secondary button with
  a chevron-down (padding 4/8, min height 24 px, `surface`, `border`);
  disabled: `text_muted` on `surface_sunken`; the list uses the popover
  colours. Keyboard: Enter/Space/Down opens, Up/Down move, Enter picks,
  Escape closes.
- **`tag.rs`**: `tag(label, Option<IconName>)`: sm, `text_muted`, 1 px
  `border`, radius 6, padding 0/4.
- **`banner.rs`**: `enum Tone { Info, Warn, Error }`;
  `Banner::new(tone, message)`: `surface` fill, 1 px border in the tone
  colour (accent, warn, danger), radius 6, padding 8/12, gap 8/12, the tone
  icon (`info`, `alert-triangle`, `x-circle`) in the tone colour, the
  message in `text` wrapping; `.action(FmButton)` (right-aligned, 4 px
  apart); `.dismiss(handler)` (quiet icon-only `x`, tooltip "Dismiss");
  `.details(text)`: a `Disclosure` "Details" under the message, sm muted,
  the text in mono. No tinted fill, no thick stripe.
- **`disclosure.rs`**: `Disclosure::new(id, summary).open(b).on_toggle(..)`:
  a quiet button with a chevron (rotated when open) and the summary; content
  below when open; Enter/Space toggle.
- **`status.rs`**: `enum StatusTone { Ok, Warn, Danger, Muted }`;
  `status(tone, IconName, word)`: icon and word in the tone colour, 4 px
  apart, no wrapping.
- **`step_number.rs`**: `step_number(n)`: a 24 px circle, `surface_sunken`,
  1 px `border`, sm text, centred.
- **`skeleton.rs`**: `skeleton_bar(fraction, wide)`: `surface_sunken`,
  radius 6, height 16 px (wide: 24 px), width `fraction` of the parent.
- **`middle_path.rs`**: `pub fn middle_truncate(text: &str, max_chars: usize) -> String`
  (keeps the start, `…`, and the whole file name when the head and the
  ellipsis still fit; else an even split; returns the text unchanged when it
  fits or `max_chars < 5`; counts `char`s) and `MiddlePath::new(display)`:
  an element that lays out in mono at the surrounding size, computes
  `max_chars` from its width and the mono advance
  (`window.text_system()`), and shows the truncated text with the full
  display as its tooltip. Paths arrive as display strings; never parse them
  into `PathBuf`.
- **`kbd.rs`**: `kbd(key)`: mono sm on `surface_sunken`, 1 px `border`,
  radius 6, padding 0/4; `chord(&["Ctrl", "O"])` joins with "+";
  `alternatives(&[chords])` joins with "or" in `text_muted`.
- **`mod.rs`**: re-exports; a module doc comment listing the widgets and the
  rule: no colour literal, sizes from tokens only.
- **Gallery for tests** (`src/widgets/tests.rs`): a test-only view
  rendering one of each.

## Files

Create: `apps/filemanager/src/widgets/{mod.rs,text.rs,icon.rs,button.rs,input.rs,checkbox.rs,select.rs,tag.rs,banner.rs,disclosure.rs,status.rs,step_number.rs,skeleton.rs,middle_path.rs,kbd.rs,tests.rs}`.
Edit: `apps/filemanager/src/lib.rs` (only `pub mod widgets;`),
`docs/gpui-test-ledger.md`.
Do not touch: models, `app/` (except nothing), `platform/`, `crates/`,
`apps/desktop/`, `docs/plans/`.

## Skill

None to run.

## Acceptance criteria

- Each widget above exists with the API named, a doc comment, and only
  `theme` colours and token sizes (no literal colour or size outside
  `theme::tokens`, except the fixed values written above: 24, 30, 32 px).
- Every interactive widget is reachable with Tab and has a visible label or
  a tooltip; `icon_only` cannot be built without a tooltip.
- `middle_truncate` keeps the drive and the file name in today's cases.
- fmt; clippy pedantic `-D warnings` (scoped `#[expect]` only);
  `cargo test --workspace` on both CI jobs.

## Tests proving completion

`cargo test -p filemanager-gpui widgets`:

- `button_click_and_keyboard_activate` (click; Tab then Enter key-down +
  key-up; Space likewise), `disabled_button_ignores_activation`,
  `busy_button_is_inert_and_keeps_label`, `icon_only_button_has_tooltip`.
- `checkbox_cycles_not_set` (clicks: Unchecked → Checked → NotSet →
  Unchecked; Space likewise), `not_set_shows_the_words`.
- `input_typing_and_read_only_selects` (typing reaches the state; a
  read-only input ignores typing and Ctrl+A selects).
- `banner_dismiss_and_action`, `banner_details_toggle`.
- `disclosure_toggles_by_keyboard`.
- `select_opens_and_picks_by_keyboard`.
- `middle_truncate_cases` (plain tests: today's `pathParts.test.ts` and
  `MiddlePath.test.tsx` cases: drive and name kept, short text unchanged,
  very long name, no separator, U+FFFD kept).
- `middle_path_tooltip_is_full_path`.

Ledger rows settled here: `app/Button.test.tsx` (variants, disabled, busy →
ported; ref forwarding and axe → dropped: DOM), `app/Banner.test.tsx`
(action and dismiss, children → ported; tone word in accessible name →
dropped: screen reader), `tools/tarpack/MiddlePath.test.tsx` and
`tools/tarpack/pathParts.test.ts` (→ `middle_truncate_cases` or dropped:
CSS-only behaviour, with the reason per row).

## States covered

Each widget's states: default, hover, active, focus, disabled, busy,
read-only, invalid, not set, open/closed. No data loading here.

## Out of scope

Dialogs, menus, popovers (U3); any tool view.
