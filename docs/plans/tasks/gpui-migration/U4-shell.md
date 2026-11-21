# U4 — Shell views: sidebar, tool area, notice, drop overlay, close prompt

Status: awaiting approval
Project: gpui-migration   Depends on: U3 (landed), M3b (landed)

## Goal

Replace the placeholder chrome of the GPUI app with today's shell: the
12rem tool sidebar with the current-tool marker, the tool area, the
path-loss notice, the full-window drop overlay and the "close with unsaved
changes" prompt, all keyboard operable.

## Context

FileManager is moving from Tauri + React to GPUI + gpui-component; the UI
looks like today (same layout, token colours; text may differ), and every
interactive element is keyboard reachable and labelled.

What exists:

- `apps/filemanager/src/app/shell.rs` (`Shell`, the window root, owned by
  the backend implementer). API: `current() -> ToolId`,
  `select(id, window, cx)`, `tools() -> &[ToolSlot]` (registry order; each
  `ToolSlot { id, label, view, hooks }`), `tool_view(id) -> AnyView`,
  `drag_over() -> Option<DropTarget>` (Some while a file drag is over the
  window; `DropTarget::{None, Enabled { label }, Disabled { reason }}`),
  `notice() -> Option<ShellNotice>` (`ShellNotice::DroppedNamesUnreadable`),
  `dismiss_notice(cx)`, `close_request() -> Option<&CloseRequest>`
  (`CloseRequest { items: Vec<(ToolId, SharedString)> }`),
  `confirm_close(window, cx)`, `cancel_close(cx)`. The Shell notifies on
  every change; observe it.
- `apps/filemanager/src/app/text.rs`: the notice copy ("Some dropped items
  could not be read: their names are not valid Unicode. Rename them and
  drop them again.") and the developer-mode strings (not shown: developer
  mode has no visible indicator, as today).
- `apps/filemanager/src/app/view/mod.rs`: `pub fn chrome(shell, window, cx)
  -> AnyView`, currently a placeholder (`placeholder.rs`) that U1 made call
  `crate::theme::init(window, cx)` first. **This task replaces the body**;
  keep the `theme::init` call first.
- `crate::theme` (U1: `palette(cx)`, tokens), `crate::widgets` (U2/U3:
  `button`, `icon`, `Banner`, `Tone`, `sync_confirm`, `ConfirmSpec`,
  `ConfirmButton`, `ConfirmVariant::Danger`, text helpers).

### Look (from today's `app.css`, `base.css`, `controls.css`)

- Window ground `bg`; frame = sidebar + tool area side by side, nothing
  scrolls at window level.
- **Sidebar** (`sidebar.rs`): width 192 px (12rem), at most 40% of the
  window; `surface_sunken`; 1 px `border` on its right; padding 12 px
  top/bottom, 8 px left/right; scrolls vertically if needed. One nav item
  per `Shell::tools()`, stacked with 4 px gaps: a gpui-component `Button`
  styled full width, left-aligned, padding 8/12, radius 6, 1 px transparent
  border, label in `text` at 14 px, wrapping. **Hover:** `surface` fill.
  **Current:** `surface` fill, 1 px `border`, weight 600, and a 6 px
  `accent` dot 8 px before the label (so the state is not weight and fill
  alone). No side stripe. Debug selector `tool-nav-<id>` (`ToolId::id()`).
- **Tool area**: fills the rest; shows `shell.tool_view(shell.current())`.
  Each tool view pads itself (16/24) and owns its scrolling. If the notice
  is set, a warn `Banner` with the notice copy and a dismiss button
  (`Shell::dismiss_notice`) sits at the top of the tool area, 16/24 px
  inset, above the view (debug selector `shell-notice`).
- **Drop overlay** (`drop_overlay.rs`), when `drag_over()` is `Some`: over
  the whole window (sidebar included), fill `drop_overlay`, a 2 px
  **dashed** `accent` border inset 8 px with radius 6 (if the pinned GPUI
  has no dashed border style, a solid 2 px line, and say so in the PR); in
  the centre a plate: `surface`, radius 6, padding 12/16, max width 640 px,
  16 px text in `text`, icon `folder` + label for `Enabled`, `info` + reason
  for `Disabled`. Not focusable; takes no pointer events. No fade (today's
  120 ms fade is dropped). Debug selector `drop-overlay`.
- **Close prompt** (`close_prompt.rs`): `sync_confirm` keyed
  `"close-prompt"` while `close_request()` is `Some`: title "Close
  FileManager with unsaved changes?", body one line per item (the item's
  text, e.g. "Database Manager: your changes to “Dune” are not saved."),
  cancel label "Keep editing" (focused), confirm "Close without saving"
  (danger). Confirm → `Shell::confirm_close`; Keep editing or Escape →
  `Shell::cancel_close`. This copy is the shell view's own (put the two
  strings and the title as constants in `close_prompt.rs`).

### Keyboard

Tab reaches the current nav item, then the tool view. In the sidebar,
Up/Down move focus to the previous/next nav item and wrap, Home/End jump
(key context `ToolNav`, actions bound with `KeyBinding`); Enter/Space select
the focused tool (`Shell::select`). Ctrl+Alt+Shift+D is the Shell's own
binding (developer mode); do not rebind it.

## Files

Create: `apps/filemanager/src/app/view/{sidebar.rs,drop_overlay.rs,notice.rs,close_prompt.rs,tests.rs}`.
Edit: `apps/filemanager/src/app/view/mod.rs` (replace the placeholder body),
delete `apps/filemanager/src/app/view/placeholder.rs` once nothing uses it,
`docs/gpui-test-ledger.md`.
Do not touch: `src/app/*.rs` outside `view/`, models, `platform/`,
`jobs.rs`, `shoot/`, `crates/`, `apps/desktop/`, `docs/plans/`.

## Skill

`impeccable critique` is not run here (no scenario yet); the Pipeline
scenario task critiques the shell with its first screenshots.

## Acceptance criteria

- The sidebar matches the geometry above (192 px wide at 1000×700; first
  item at y 12 px) and the current marker is fill + border + weight + dot.
- Keyboard: every nav item reachable and operable; arrows wrap; the close
  prompt starts on "Keep editing"; Escape keeps editing.
- The overlay shows the label or the reason, and disappears on drop or
  leave.
- The notice shows and dismisses.
- No colour literal outside `theme`.
- fmt; clippy pedantic `-D warnings`; `cargo test --workspace` on both jobs.

## Tests proving completion

`apps/filemanager/src/app/view/tests.rs` (`#[gpui::test]`,
`VisualTestContext`, a test `Shell` with counting placeholder tools):

- `nav_click_selects_tool`, `nav_keys_move_wrap_and_select` (Down, Up,
  Home, End; Enter key-down + key-up selects), `current_item_is_marked`
  (the current item renders the dot element; the others do not).
- `drop_overlay_shows_label_or_reason` (drive `FileDropEvent::Entered` /
  `Exited` through `simulate_event`; `drop-overlay` bounds exist, then not).
- `notice_shows_and_dismisses`.
- `close_prompt_opens_on_request_and_keep_editing_cancels`,
  `close_prompt_confirm_closes` (raise the request through
  `simulate_close()` with a tool whose `unsaved_work` is non-empty).

Ledger rows: `app/AppShell.test.tsx` (renders entries, selecting renders
its view → ported; empty-state and axe → dropped: no empty registry / axe),
`app/ToolNav.test.tsx` (aria-current → ported as the marker test; arrows,
Enter/Space → ported; "no thick side border" → dropped: CSS),
`App.test.tsx` (Pipeline first → ported in `nav_click_selects_tool`'s setup
or dropped if M3a's registry test covers it), the `DropZone.test.tsx` rows
M3b marked U (overlay look) → ported to `drop_overlay_shows_label_or_reason`.

## States covered

Sidebar: current, hover, focus. Overlay: hidden, enabled, disabled. Notice:
none, shown. Close prompt: hidden, shown.

## Out of scope

Tool views (later U tasks); screenshots (the Pipeline scenario task removes
the M4 sample `shell-sample`).
