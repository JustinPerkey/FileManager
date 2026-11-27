# U4b — Tool shortcuts work wherever focus is in the window

Status: approved (2026-11-27; a planner decision after the dbmanager U2 review)
Project: gpui-migration   Depends on: U4 and U9 (landed). Lands before U11 and dbmanager U12.

## Goal

Make each tool's shortcuts (Ctrl+O, F5, F8, Ctrl+E, Ctrl+Enter, Escape for
results; the Database Manager's later ones) fire wherever focus is in the
window, as today's window-level listeners did. Keep the sidebar's arrow-key
navigation, and keep text fields' and tables' own keys.

## Context

In GPUI a key binding fires only when its key context is on the focused
element's path. Today each tool view puts its key context and `on_action`
handlers on its own root, so they fire only while focus is inside the tool
view. After a sidebar click, focus stays on the sidebar item: U5 made that so
Up and Down work. So Ctrl+O, F5 and F8 then do nothing. The React app
listened on `window`, so they always worked.

The chrome is `apps/filemanager/src/app/view/` (ui-implementer's). It holds
the sidebar and hosts the current tool's view, which the Shell gives it as
an `AnyView` through `Shell::tool_view(id)`. The Shell is
`apps/filemanager/src/app/shell.rs` and carries the key context `"Shell"`.
The tool views are in `apps/filemanager/src/tools/<tool>/view/`.

**Decision (planner, 2026-11-27):** the chrome puts the **current tool's**
key context, and that tool's shortcut handlers, on the **chrome root**,
which is an ancestor of both the sidebar and the tool view. The
alternative, moving focus into the tool on `activated`, would break the
sidebar's arrow-key navigation.

**Conflict check.** In GPUI, when several bindings match a keystroke, the
one in the **deepest** key context on the focused path wins. So:

- **Text inputs** keep their own keys: characters, Delete, Backspace,
  Home/End and gpui-component's input bindings. Shortcuts with Ctrl or a
  function key do not clash with typing. Escape in an input is the input's
  own key, if gpui-component binds it.
- **Tables and lists** keep Up, Down, Enter and Delete in their own
  context.
- **Dialogs** have their own contexts (`Root`'s dialog layer is outside the
  chrome root). A tool shortcut must not fire behind a modal: the handlers
  check `window.has_active_dialog(cx)` and do nothing while one is open.
- **The developer-mode chord** stays the app-level keystroke interceptor
  (unchanged).

If the deepest-context rule does not hold in the pinned GPUI for some key,
stop and report it with the key.

### What to build

- **Each tool view module exposes its shortcuts**, for example in
  `tools/<tool>/view/shortcuts.rs`: the key context name and a function
  that attaches the tool's `on_action` handlers to an element, forwarding
  to the tool view or model. Today the Tar Packager has shortcuts (U9) and
  the Pipeline and Schedule Creator have none; the Database Manager adds
  its own in dbmanager U12, the same way.
- **The chrome root** sets `.key_context(<current tool's context>)` and
  attaches the current tool's handlers. It switches both when the current
  tool changes.
- **Remove the duplicates:** move the tool's shortcut handlers from its own
  root to this route, so no action fires twice. Keep row-level keys where
  they are.
- **Tests:**
  - `tool_shortcut_fires_after_sidebar_click`: click a sidebar item, then
    Ctrl+O on the Tar Packager opens its dialog (via `FakePlatform`), and
    focus stays on the sidebar item;
  - `typing_in_a_field_is_not_a_shortcut`: in a focused text input, typed
    characters and Delete edit the text, and no tool action fires;
  - `shortcut_does_nothing_behind_a_dialog`;
  - `sidebar_arrows_still_move`.

### Rules that bind this task

- Views call only their model. Every interactive element stays keyboard
  reachable and labelled. Tags go last in a builder chain.
- No change to models, backends, `src/app/shell.rs` or `src/tools/mod.rs`.
  If a Shell hook is needed, stop and report it.
- fmt (linux job), clippy pedantic `-D warnings`, `cargo test --workspace`.

## Files

Edit: `apps/filemanager/src/app/view/` (the chrome root), and
`apps/filemanager/src/tools/tarpack/view/` (move its shortcut handlers to
the new route). Add a `shortcuts.rs` per tool view where needed.

## Skill

None. Do not run `impeccable`.

## Acceptance criteria

- The four tests pass, and the existing shortcut tests pass unchanged or
  moved, with no action firing twice.

## Tests proving completion

The four named tests and the existing Tar Packager shortcut tests.

## States covered

Focus in the sidebar, in the tool view, in a text field, and behind a
dialog.

## Out of scope

- New shortcuts.
- The Database Manager's own shortcuts (dbmanager U12).
