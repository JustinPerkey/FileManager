# U3 — Shared widgets, part 2: confirmation dialog, menu button, shortcuts help

Status: approved (2026-11-21). **Review follow-up (2026-11-23):** the next run does only the section "Review follow-up" below.
Project: gpui-migration   Depends on: U2 (landed)

## Review follow-up (2026-11-23)

From the review of the first U3 run (commit 8945367). Do only this; the rest
of U3 has landed. Edit `apps/filemanager/src/widgets/{shortcuts_help.rs,menu_button.rs,tests.rs}`.

### Shortcuts help (`shortcuts_help.rs`): match today

- **No row rules.** Remove the 1 px `border` line between rows. Today's
  table has none, and the column alignment already separates the rows.
- **Row spacing as today:** 4 px top and bottom per row (`SPACE_1`), so rows
  are 8 px apart, and 12 px between columns (`SPACE_3`). The old line here
  said "8 px row padding", which meant 8 px between rows.
- **Columns size themselves.** Delete `KEYS_WIDTH` and `WHEN_WIDTH`. Lay the
  table out as one grid with three columns sized by their content: GPUI's
  `.grid().grid_cols_max_content(3)`, which is `repeat(3, minmax(0,
  max-content))`. Each cell is a direct grid child, row by row, so each
  column is as wide as its widest cell, like today's auto table. The keys
  cell does not wrap (`whitespace_nowrap`, and its chords stay on one line).
  The action cell wraps. The "when" cell is sm `text_muted` and wraps only
  if the 576 px cap forces it. Cells align to the first line's baseline, or
  to the top if GPUI has no baseline alignment.
  - Check this in the test host at the 576 px cap with one long action. If
    the grid shrinks the keys column below its content, give the keys cells
    `min_w` = their own content (`flex_none`) rather than a number. If that
    also fails, use a width in `REM` multiples from `theme::tokens` (so it
    scales with text size), never a px literal, and say so in the hand-off.
- **Anchor: right-aligned under the button.** Use
  `Popover::anchor(Anchor::TopRight)`: the popover's top-right corner sits
  at the button's bottom-right, with the default 4 px gap (0.25rem, today's
  `--space-1`). In the Tar Packager header the button is at the content's
  right edge, so this gives today's position. gpui-component clamps the
  popover to the window rather than flipping it; that is accepted.

### Menu button (`menu_button.rs`): match today

- **Anchor: right-aligned under the trigger,** as today's `right: 0`:
  `Popover::anchor(Anchor::TopRight)`, default 4 px gap.

### Selected menu row: keep as built

Keep the 2 px `focus_ring` border inside the row, on a `surface_sunken`
fill, for the one selected row (keyboard or mouse hover). Reasons:

- Today's outline sits 2 px outside the row. In a list with 4 px padding
  and no gap between rows it would overlap the next row or be clipped by
  the list.
- One selected row means keyboard and mouse never highlight two rows at
  once.
- Contrast: the ring against `surface_sunken` is 5.67:1 light and 7.47:1
  dark, and `text` on `surface_sunken` is 14.8:1 and 15.3:1.

The transparent 2 px border on unselected rows stays, so selection never
shifts the layout.

### Tests

- `shortcuts_popover_opens_under_button_and_escape_closes`: also assert
  that the popover's right edge equals the button's right edge (within
  1 px) and that its top is below the button's bottom.
- `menu_opens_with_keys_wraps_and_picks`: also assert that the list's right
  edge equals the trigger's right edge (within 1 px).
- `shortcuts_columns_align`: in the test host, with three rows whose keys
  differ in width, the action cells of every row start at the same x, and
  that x lies past the widest keys cell.
- Acceptance: `rg "KEYS_WIDTH|WHEN_WIDTH|border_b" apps/filemanager/src/widgets/shortcuts_help.rs`
  finds nothing.

## Goal

Add the three shared overlays to `apps/filemanager/src/widgets/`: a modal
confirmation dialog driven by model state (with the extensions the Database
Manager needs), a menu button, and the keyboard-shortcuts popover, each
keyboard operable and tested.

## Context

FileManager is moving from Tauri + React to GPUI + gpui-component; the UI
looks like today, every interactive element is keyboard reachable and
labelled. A modal is used **only** for confirmations (overwrite, apply, a
step that changes files, delete, discarding unsaved changes, closing with
unsaved changes); menus and help are non-modal.

What exists: `crate::theme` (`palette(cx)`, tokens, `theme.overlay` = the
scrim: black at 0.4 light / 0.6 dark), `crate::widgets` (U2: `button`
with primary/quiet/danger/icon/busy/shortcut, `icon`, `IconName`, `text`
helpers, `kbd`, `chord`, `alternatives`). The app's root view is wrapped in
`gpui_component::Root` (M3a), which hosts dialogs and popovers.

Proven in the spike (`git show origin/spike/gpui-migration:spike/gpui/src/widgets.rs`,
`open_confirm`, `recent_menu`, `overlays`): `window.open_dialog(cx, |dialog, window, cx| ..)`
(the builder runs on **every render**, so it can read live state),
`Dialog::{title, w, keyboard(bool), overlay_closable(bool), content, footer}`,
`DialogFooter`, `window.close_dialog(cx)`, `window.has_active_dialog(cx)`;
focus starts on Cancel only if focused explicitly after opening
(`window.defer(cx, move |w, cx| cancel.focus(w, cx))`); focus returns to the
opener when the opener had focus. `Button::dropdown_menu(|menu, w, cx|
menu.item(PopupMenuItem::element(..).on_click(..)))`: Up/Down wrap, Enter
picks, Escape closes, focus returns to the trigger. `Popover::new(id).trigger(button).content(..)`:
anchored under the trigger, Escape closes. The dialog, menu and popover
shadows are fixed by gpui-component (accepted); our `--shadow-overlay`
token is not applied. A mouse press does not focus a button, so focus return
after a mouse-opened dialog is not guaranteed (accepted).

### What to build

**`dialog.rs`**

```rust
pub enum ConfirmVariant { Primary, Danger }
pub struct ConfirmButton { pub label: SharedString, pub variant: ConfirmVariant, pub icon: Option<IconName> }
pub struct ConfirmSpec {
    pub title: SharedString,
    pub body: SharedString,                       // the short description
    pub extra: Option<Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>>, // scrolling region below the body
    pub cancel_label: SharedString,               // "Cancel" by default; "Keep editing", "Close"
    pub confirm: Option<ConfirmButton>,           // None: single-button form (only the cancel button)
    pub busy: bool,                               // confirm busy; Cancel and Escape inert
    pub wide: bool,                               // 640 px instead of 448 px
}
pub fn sync_confirm(window: &mut Window, cx: &mut App, key: &'static str,
                    spec: Option<ConfirmSpec>, on_confirm: impl Fn(&mut Window, &mut App) + 'static,
                    on_cancel: impl Fn(&mut Window, &mut App) + 'static);
```

`sync_confirm` is called by a view on every model change: `Some` opens the
dialog if none with this `key` is open (focus moves to Cancel), updates it
live if open, `None` closes it. Look: `surface`, 1 px `border`, radius 6,
padding 24, width 448 px (wide 640 px, never wider than the window minus
48 px); title lg (16 px) 600, 8 px above the body; body in `text`; `extra`
in a region that scrolls (max 50% of the window height) 16 px below the
body; footer right-aligned, 8 px apart: Cancel (secondary), confirm (primary
or danger, with its icon; busy shows the busy button). Scrim
`theme.overlay`; clicking the scrim does nothing (`overlay_closable(false)`).
Escape = cancel unless busy (`keyboard(!busy)`); while busy Cancel is
disabled. Hide gpui-component's own close icon if the API allows it; if it
cannot be hidden, it stays (labelled "Close") and the PR says so. Debug
selectors `dialog-cancel`, `dialog-confirm`, `dialog-title`.

**`menu_button.rs`**

```rust
pub struct MenuItem { pub label: SharedString, pub detail: Option<SharedString>, pub tooltip: Option<SharedString>, pub disabled: bool }
pub struct MenuButton { .. }   // MenuButton::new(id, label).items(Vec<MenuItem>).on_select(Fn(usize, &mut Window, &mut App)).disabled(b).shortcut(..)
```

A secondary button with a trailing `chevron-down` (never a glyph) opening
the list right-aligned under it: `surface`, 1 px `border`, radius 6, padding 4, min
width 256 px, max 512 px, scrolling at 60% of the window height; items are
quiet rows, label sm in `text`, optional `detail` line sm `text_muted`,
wrapping. Keys: Enter, Space or Down opens; Up/Down wrap; Home/End jump;
Enter picks; Escape and Tab close and refocus the trigger. With no items the
button is not rendered. Debug selectors: the trigger `id`, items
`<id>-item-<n>`.

**`shortcuts_help.rs`**

```rust
pub struct ShortcutRow { pub keys: Vec<Vec<&'static str>>, pub action: SharedString, pub when: SharedString }
pub struct ShortcutsHelp { .. } // ShortcutsHelp::new(id, rows).on_open_change(Fn(bool, &mut Window, &mut App))
```

A quiet button with the `keyboard` icon and the visible label "Keyboard
shortcuts", opening a non-modal popover right-aligned under it (see "Review follow-up"): `surface`, 1 px
`border`, radius 6, padding 12/16, max width 576 px, scrolling inside
itself; heading "Keyboard shortcuts" (lg 600); a three-column table, no
rules, self-sizing columns, 4 px row padding: keys (`alternatives` of `chord`s of `kbd`), action,
when (sm `text_muted`). Escape closes it and returns focus to the button;
`on_open_change` fires on open and close (the Tar Packager's Escape key
must not dismiss a result while it is open). Debug selectors: `id`,
`<id>-popover`.

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

Create: `apps/filemanager/src/widgets/{dialog.rs,menu_button.rs,shortcuts_help.rs}`.
Edit: `apps/filemanager/src/widgets/{mod.rs,tests.rs}`, `docs/gpui-test-ledger.md`.
Do not touch: models, `app/`, `platform/`, `crates/`, `apps/desktop/`,
`docs/plans/`.

## Skill

None to run.

## Acceptance criteria

- The three APIs above exist with doc comments; colours and sizes only from
  `theme`.
- Keyboard: dialog focus starts on Cancel, Tab cycles inside it, Escape
  cancels (not while busy); the menu and popover are fully keyboard operable
  and return focus to their trigger.
- Labels: every button has a visible label; the dialog has its title.
- fmt; clippy pedantic `-D warnings`; `cargo test --workspace` on both jobs.

## Tests proving completion

`cargo test -p filemanager-gpui widgets` (test-only host views in
`widgets/tests.rs`):

- `confirm_focus_starts_on_cancel_and_escape_cancels`,
  `confirm_button_confirms`, `busy_confirm_is_inert` (Escape and Cancel do
  nothing, the dialog stays), `single_button_form_has_only_cancel`,
  `sync_none_closes_and_some_updates_live`,
  `keyboard_opened_dialog_returns_focus_to_opener`.
- `menu_opens_with_keys_wraps_and_picks`, `menu_escape_refocuses_trigger`,
  `empty_menu_is_not_rendered`, `disabled_item_is_skipped`.
- `shortcuts_popover_opens_under_button_and_escape_closes`,
  `shortcuts_open_change_fires`.

Ledger rows: `app/ConfirmDialog.test.tsx` (focus, Escape, return, confirm →
ported; aria-labelledby/describedby and axe → dropped: DOM/ARIA).

## States covered

Dialog: open, busy, single-button, wide, with extra content, closed. Menu:
closed, open, empty, item disabled. Popover: closed, open.

## Out of scope

Any tool's dialog copy or shortcut list (the tool tasks); the close prompt
(U4).
