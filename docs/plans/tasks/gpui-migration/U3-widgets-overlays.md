# U3 — Shared widgets, part 2: confirmation dialog, menu button, shortcuts help

Status: approved (2026-11-21)
Project: gpui-migration   Depends on: U2 (landed)

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
the list under it: `surface`, 1 px `border`, radius 6, padding 4, min
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
shortcuts", opening a non-modal popover anchored under it: `surface`, 1 px
`border`, radius 6, padding 12/16, max width 576 px, scrolling inside
itself; heading "Keyboard shortcuts" (lg 600); a three-column table with
8 px row padding: keys (`alternatives` of `chord`s of `kbd`), action,
when (sm `text_muted`). Escape closes it and returns focus to the button;
`on_open_change` fires on open and close (the Tar Packager's Escape key
must not dismiss a result while it is open). Debug selectors: `id`,
`<id>-popover`.

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
