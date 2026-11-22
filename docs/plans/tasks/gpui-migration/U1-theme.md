# U1 — Theme: tokens, gpui-component mapping, live light and dark

Status: approved (2026-11-21). **Review follow-up (2026-11-22):** the next run does only the section "Review follow-up" below; it must land before U2.
Project: gpui-migration   Depends on: M3a (landed)

## Goal

Give the GPUI app (`apps/filemanager`, package `filemanager-gpui`, lib
`fm_app`) today's visual system: every colour, spacing, radius and type
token of `apps/desktop/src/styles/tokens.css` in `src/theme/tokens.rs`,
mapped onto gpui-component's theme, following the system's light or dark
appearance live, with tests that keep the tokens, `DESIGN.md` and the
contrast pairs honest.

## Review follow-up (2026-11-22)

The U1 review approved it pending CI, but found the items below. **This
run does only this section.** It must land before U2 adds the first text
input. Read `apps/filemanager/src/theme/` first.

1. **Selected text must be readable (P1).**
   - Set the `selection` slot to `accent` at **alpha 0.3**, not full
     opacity. Selected text cannot be recoloured in GPUI, so it stays
     `text`. On full `accent` that is 2.61:1 (light) and 2.05:1 (dark); at
     0.3 over `surface` it is 10.55:1 and 7.61:1.
   - gpui-component applies the same 0.3 cap itself, but only when it loads
     a theme config (`schema.rs`, around lines 1043–1073). `apply` writes
     the slots directly, so the cap never applies.
   - `list_active` and `table_active` stay `surface_sunken`, as the slot
     table says (no cap needed).
2. **Table-driven slot test.** `apply_sets_every_slot`: one row per slot
   `apply` sets, giving the slot, the expected token (or derived value), and
   both modes. It replaces the sampled `apply_sets_slots_from_tokens`.
3. **Contrast for derived pairs.** Extend `contrast_pairs_meet_4_5`, both
   themes, with `text::contrast` on composited colours (use
   `over_composites_like_a_browser`'s compositing):
   - `accent_text` on the `ok`, `warn` and `danger` fills;
   - `accent_text` on `primary_hover` and on `danger_hover`;
   - `text` on `selection` composited over `surface`, and over `bg`.

   If a pair fails 4.5:1, stop and report it with the measured ratio. Do
   not change a token; that is the ui-designer's decision.
4. **The `tokens.css` dark media block.** `tokens_match_tokens_css` also
   compares the `@media (prefers-color-scheme: dark)` block with
   `[data-theme="dark"]`, value by value. Or, if that block is only a hook
   with no values, reword the ledger row "dark hooks define identical
   values" to say what is actually checked.
5. **Drop the redundant `window.refresh()`** in the appearance observer.
   Applying the theme already notifies.
6. **For U4:** `theme::init` must be called once per window. Add a doc
   comment on `theme::init` saying so. Make a second call a no-op, so a
   chrome that calls it twice does not register two observers. Test:
   `init_twice_registers_one_observer`.

Follow-up acceptance: the named tests pass; fmt (linux job), clippy
pedantic and `cargo test --workspace` pass.

## Context

FileManager (a portable Windows 11 exe) is moving from Tauri + React to GPUI
with gpui-component. The human decided the UI **looks like today**: same
layout and token colours, sampled colours within ±1 per channel; text
rendering may differ. Light and dark follow the system; there is no toggle.

A throwaway spike proved this theme on branch `spike/gpui-migration`. Port
its files by name: `git show origin/spike/gpui-migration:spike/gpui/src/tokens.rs`
and `…/src/theme.rs` (its colour check passed 32 of 32 samples). What
M3a built: `apps/filemanager/src/lib.rs` (`run`: `gpui_component::init`,
mono font, window, `Root`), `src/fonts.rs` (`UI_FAMILY = "Segoe UI"`,
`MONO_FAMILIES = ["Cascadia Mono", "Consolas"]`, `mono_family(cx)`: the
first of those installed on the machine; no font is embedded), `src/app/view/mod.rs` (`pub fn chrome(shell:
Entity<Shell>, window: &mut Window, cx: &mut App) -> AnyView`, the chrome
slot, a placeholder you may call into but whose body U4 replaces).

### Tokens (port exactly; names = CSS names in `snake_case`)

| CSS | Light | Dark |
|---|---|---|
| `--bg` | `#f7f7f8` | `#16171a` |
| `--surface` | `#ffffff` | `#1e2024` |
| `--surface-sunken` | `#eef0f2` | `#121316` |
| `--border` | `#d9dce1` | `#33363d` |
| `--text` | `#1b1d21` | `#e8e9ec` |
| `--text-muted` | `#5b616b` | `#a3a8b1` |
| `--accent` | `#2457c5` | `#7aa2ff` |
| `--accent-text` | `#ffffff` | `#0d1530` |
| `--ok` | `#1d7a46` | `#5fd08f` |
| `--warn` | `#8a5a00` | `#f2c14e` |
| `--danger` | `#b3261e` | `#ff8a80` |
| `--focus-ring` | `#2457c5` | `#7aa2ff` |
| `--drop-overlay` | `rgb(36 87 197 / 0.08)` | `rgb(122 162 255 / 0.12)` |
| `--shadow-overlay` | alpha 0.18 (`0 8px 24px`) | alpha 0.5 |
| `--scrim` | `rgb(0 0 0 / 0.4)` | `rgb(0 0 0 / 0.6)` |

Scales: `SPACE_1..6` = 4, 8, 12, 16, 24, 32 px; `RADIUS` 6 px;
`FONT_SIZE_SM/MD/LG/XL` = 0.8125, 0.875, 1, 1.25 rem; `REM` 16 px;
`LINE_HEIGHT` 1.4; `FONT_UI = "Segoe UI"`; `FONT_MONO` =
`crate::fonts::mono_family(cx)` (system fonts: Cascadia Mono, else
Consolas; do not add another picker). Keep `shadow_overlay_alpha` as a field, documented as **not
applied**: gpui-component's dialog and popover shadow is fixed and the human
accepted it.

### Mapping onto gpui-component (`theme::apply`)

`Theme::change(mode, None, cx)` **then** `Theme::update(cx, |th| …)` (a mode
change reloads colours). `th.font_size` **stays 16 px**: gpui-component's
`Root` makes it the window rem, and 14 px would shrink every control. Set
`font_family` = Segoe UI, `mono_font_family` = `crate::fonts::mono_family(cx)`,
`mono_font_size` 14 px, `radius`/`radius_lg` 6 px, `shadow = false`. Slots:

| Slots | Token |
|---|---|
| `background` | bg |
| `foreground`, `caret`, `popover_foreground`, `accent_foreground`, `button_foreground`, `secondary_foreground`, `sidebar_foreground`, `sidebar_accent_foreground` | text |
| `border`, `input`, `sidebar_border`, `table_row_border`, `window_border`, `title_bar_border`, `scrollbar_thumb` | border |
| `muted`, `accent`, `sidebar`, `button_hover`, `button_active`, `secondary_hover`, `secondary_active`, `list_hover`, `list_active`, `table_hover`, `table_active`, `scrollbar`, `title_bar` | surface_sunken |
| `muted_foreground`, `table_head_foreground`, `scrollbar_thumb_hover` | text_muted |
| `popover`, `button`, `secondary`, `sidebar_accent`, `colors.list`, `list_even`, `table`, `table_even`, `table_head` | surface |
| `list_head` | bg |
| `primary`, `button_primary`, `sidebar_primary`, `link`, `info`, `ring`, `progress_bar`, `drag_border`, `list_active_border`, `table_active_border` | accent (`ring` from focus_ring) |
| `selection` | accent at **alpha 0.3** (selected text stays `text`; see the Review follow-up) |
| `primary_hover/active`, `button_primary_hover/active`, `link_hover/active` | `accent.over(0.88, text)` |
| `primary_foreground`, `button_primary_foreground`, `sidebar_primary_foreground`, `info_foreground`, `success_foreground`, `warning_foreground`, `danger_foreground`, `button_danger_foreground` | accent_text |
| `danger`, `button_danger` / their hover and active | danger / `danger.over(0.88, text)` |
| `success`, `warning` | ok, warn |
| `overlay` | black at `scrim_alpha` |
| `drop_target` | drop_overlay |

Any other slot stays at gpui-component's value. Also expose the derived
values the widgets use, computed here so no hex exists elsewhere:
`Palette` (the current `Colors` as `Hsla`, plus `primary_hover`,
`danger_hover`), `INTRO_MAX_WIDTH = 546.0` px (CSS `65ch` at 14 px),
`DISABLED_OPACITY = 0.55`, `QUIET_DISABLED_OPACITY = 0.45`,
`INPUT_DISABLED_OPACITY = 0.6`. A widget reads the palette with
`theme::palette(cx) -> &Palette` (a global updated by `apply`).

### Live appearance and the screenshot override

- `pub fn init(window: &mut Window, cx: &mut App)`: apply for
  `window.appearance()` (`Light`/`VibrantLight` → light,
  `Dark`/`VibrantDark` → dark) and follow
  `window.observe_window_appearance(..)` live, refreshing the window.
  Call it once from `src/app/view/mod.rs::chrome` (first line; leave the
  rest of the placeholder alone).
- `pub fn set_override(mode: Option<ThemeMode>, cx: &mut App)`: a global
  that wins over the system appearance when `Some` (the screenshot harness
  uses it for `--dark`); `None` returns to following the system.

## Files

Create: `apps/filemanager/src/theme/{mod.rs,tokens.rs,tests.rs}`.
Edit: `apps/filemanager/src/lib.rs` (only `pub mod theme;`),
`apps/filemanager/src/app/view/mod.rs` (only the `theme::init` call),
`docs/gpui-test-ledger.md` (rows of `styles/tokens.test.ts`).
Do not touch anything else (models, `platform/`, `jobs.rs`, `crates/`,
`apps/desktop/`, `docs/plans/`).

## Skill

None to run. (`impeccable critique` happens on screenshots in the scenario
tasks.)

## Acceptance criteria

- Every `tokens.css` token, light and dark, exists in `tokens.rs` with the
  value above; nothing in `src/` outside `theme/` contains a colour literal
  (grep `Rgb(`, `rgb(`, `hsla(`, `0x` colour constants).
- `theme::apply` sets every slot in the table; `font_size` is 16 px.
- The app opens in light or dark per the system and switches live when the
  system setting changes (the human checks this on Windows 11; record it
  as a checklist line in the PR).
- `set_override(Some(Dark))` turns the theme dark regardless of the system;
  `set_override(None)` returns to the system.
- No interactive element is added.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`
  (pedantic; scoped `#[expect(.., reason)]` only) and `cargo test --workspace`
  pass on both CI jobs. GPUI's `test-support` stays a dev-dependency feature.

## Tests proving completion

`cargo test -p filemanager-gpui theme` (in `src/theme/tests.rs`):

- `tokens_match_tokens_css`: parse `apps/desktop/src/styles/tokens.css`
  (path from `CARGO_MANIFEST_DIR`, read at run time) and compare every
  colour (light block `:root {`, dark block `:root[data-theme="dark"]`),
  the drop overlay, shadow and scrim alphas, spacing, radius, type sizes,
  `--font-ui` (`"Segoe UI", system-ui, sans-serif`) and that `--font-mono`
  starts with `"Cascadia Mono"`. If the file does not exist, assert that
  `apps/desktop/` does not exist either (the Tauri app was removed) and
  return; fail otherwise.
- `design_md_matches_tokens`: parse the YAML front matter of the root
  `DESIGN.md` (`colors:` light names and `-dark` names, `rounded.md`,
  `spacing`) and assert each equals `tokens.rs` (plain string parsing; no
  YAML crate).
- `contrast_pairs_meet_4_5` (WCAG relative-luminance formula, both themes):
  text and text_muted on bg, surface, surface_sunken; accent_text on accent;
  ok, warn, danger on surface; danger on surface_sunken.
- `over_composites_like_a_browser` (from the spike).
- `apply_sets_slots_from_tokens` (`#[gpui::test]`: background, sidebar,
  primary, overlay, table_row_border, font_size in both modes).
- `override_wins_over_appearance`, `appearance_change_reapplies`
  (`#[gpui::test]` with a `VisualTestContext` window).

Settle the ledger rows of `styles/tokens.test.ts`: contrast and token rows
→ `ported` to the tests above; the `.tarpack`-scoped selector and shadow
rows → `dropped: CSS structure, no CSS in the GPUI app`.

## States covered

Light, dark, live switch, override. (No loading/empty/error states: no
view.)

## Out of scope

Widgets (U2, U3), the shell's look (U4), `DESIGN.md`/`PRODUCT.md` edits
(the ui-designer, at cutover).
