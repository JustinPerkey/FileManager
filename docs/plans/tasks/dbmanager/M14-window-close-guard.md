# M14 — Window-close guard (capability change: reviewed decision)

Status: **dropped (2026-11-21):** the GPUI shell's close guard replaces it (`docs/plans/tasks/gpui-migration/M3-app-b-plumbing.md`, `M12-dbmanager-b-edits.md`). Do not run this plan. Previous status: awaiting approval. New on 2026-11-16, from the human's answer that
closing the window with unsaved edits must prompt. **This milestone changes
`capabilities/default.json`, so the reviewer must explicitly approve the
capability change.** The CSP does not change.
Project: dbmanager   Depends on: none in the backend (can run any time).
It must land before the U task that wires the unsaved-edits prompt.

## Goal

Let the frontend intercept a window close. It keeps the window open and asks
when there are unsaved edits, and closes it once the user confirms. This
needs the narrowest new webview capability, a thin wrapper in `lib/tauri.ts`,
and tests.

## Context

FileManager is a Tauri v2 app. Today closing the main window closes the app
immediately. The Database Manager keeps unsaved object and link edits in the
UI, and the human wants a prompt when the window is closed with unsaved
edits.

**Where to intercept: the frontend.** Only the UI knows whether there are
unsaved edits, so the decision belongs there. The backend alternative would
be a Rust `on_window_event` handler that prevents the close and emits an
event. It was rejected for three reasons:

- It needs a new `Builder` hook outside the tool registry pattern (only
  `tools::register` may touch the builder).
- It needs a round trip in which the UI reports its dirty state.
- It is app-wide behaviour that does not belong to one tool's backend
  module.

**How Tauri v2 does it.** In `@tauri-apps/api/window`,
`getCurrentWindow().onCloseRequested(handler)` listens for the window's
close-requested event.

1. While a listener is registered, the close waits for the frontend.
2. The handler may call `event.preventDefault()` to keep the window open.
3. If it does not, the API then closes the window by calling
   `window.destroy()`.
4. Closing later, after the user confirms, also uses `destroy()`.

So the webview needs the permission for `destroy`. Listening to events is
already granted (`core:event:allow-listen`).

**Verify this against the `@tauri-apps/api` and `tauri` versions in the lock
files** (`package-lock.json`, `Cargo.lock`) before changing anything:

- Run `npm run tauri:dev` on Windows, or on Linux with the webkit packages,
  with the guard registered.
- Check that the close is intercepted, and that both "keep open" and
  "close" work.
- Check which permission the denial message names if one is missing.

Grant **only** the narrowest permission that makes this work. It is expected
to be `core:window:allow-destroy`; if the installed version needs a
different one, use that one and record why in the hand-off.

The current capability file, `apps/desktop/src-tauri/capabilities/default.json`,
grants exactly:

- `core:event:allow-listen`
- `core:event:allow-unlisten`
- `dialog:allow-open`
- `dialog:allow-save`

The test `capabilities_are_minimal` in `apps/desktop/src-tauri/src/tools/mod.rs`
pins that list. Its `"description"` says what is granted and why.

`apps/desktop/src/lib/tauri.ts` holds the thin wrappers over
`@tauri-apps/api`: `call`, `on`, the dialogs and `onDragDrop`.
`apps/desktop/src/lib/tauri.test.ts` tests them with `vi.mock`.

Rules that bind this task:

- `CLAUDE.md` makes the webview's security settings a reviewed decision:
  > "The webview CSP in `tauri.conf.json` is restrictive. Changing it is a
  > reviewed decision, not a fix-up."

  Treat the capability file the same way. Change **only** the one permission,
  update the test that pins the list, and update the description. **Hand the
  diff to the reviewer flagged "capability change: please approve
  explicitly".**
- The CSP in `tauri.conf.json` does not change.
- The implementer owns `apps/desktop/src/lib/` and the Rust/config side. Do
  not register the guard in any view, `App.tsx` or the app shell: wiring it
  to unsaved edits is the ui-implementer's U task.
- fmt and clippy pedantic pass; typecheck, lint and tests pass.

### Wrapper API (`apps/desktop/src/lib/tauri.ts`)

```ts
/**
 * Intercepts closing the main window. `shouldKeepOpen` runs on every close
 * request; return true to keep the window open (for example to ask about
 * unsaved edits), false to let it close. If it throws, the window closes
 * (never trap the user). Resolves to an unlisten function.
 */
export function guardWindowClose(
  shouldKeepOpen: () => boolean | Promise<boolean>,
): Promise<UnlistenFn>;

/** Closes the main window now, without running the guard again (after the user confirmed). */
export function closeWindow(): Promise<void>;
```

- Implement `guardWindowClose` with `getCurrentWindow().onCloseRequested`.
  Call `event.preventDefault()` only when `shouldKeepOpen` resolves to
  `true`.
- Implement `closeWindow` with `getCurrentWindow().destroy()`. `destroy`
  does not emit another close request, so the guard does not run again.
- Only one guard should be active at a time. Document that the app shell or
  the tool view owns it; that wiring is the ui-implementer's design.

## Files

Edit:

- `apps/desktop/src-tauri/capabilities/default.json`: add the one
  permission, and extend `"description"` with "…and destroying the window
  after a close request the frontend intercepted (unsaved-edits prompt)";
- `apps/desktop/src-tauri/src/tools/mod.rs`: the pinned list in
  `capabilities_are_minimal`;
- `apps/desktop/src/lib/tauri.ts` and `apps/desktop/src/lib/tauri.test.ts`;
- `CLAUDE.md`: one line under Invariants or the layout entry for
  `capabilities/default.json`, saying that the window-destroy permission
  exists for the unsaved-edits close prompt.

Do not touch `tauri.conf.json` (the CSP), views, `App.tsx`, other tools, or
`docs/plans/`.

## Acceptance criteria

- `capabilities/default.json` grants exactly the four previous permissions
  plus the one window permission. `capabilities_are_minimal` pins the five.
- The CSP and `devCsp` in `tauri.conf.json` are byte-identical to before.
- `tauri.test.ts`, with `@tauri-apps/api/window` mocked, checks
  `guardWindowClose`:
  - it registers through `onCloseRequested`;
  - it calls `preventDefault` when the callback returns `true` or resolves to
    `true`;
  - it does not call `preventDefault` when the callback returns `false` or
    throws;
  - the returned unlisten function is the one from `onCloseRequested`.
- `tauri.test.ts` checks that `closeWindow` calls `destroy`.
- The manual check above is done and recorded in the hand-off: the Tauri
  versions, the permission needed, and that keep-open and close both work.
  Do it on Windows if possible; otherwise on Linux with the webkit packages,
  and say which.
- The reviewer has explicitly approved the capability change.
- `cargo test --workspace`, fmt, clippy, and `npm run typecheck && npm run
  lint && npm run test` pass.

## Tests proving completion

`capabilities_are_minimal` (updated).

`tauri.test.ts`:

- `guardWindowClose keeps the window open when asked`
- `guardWindowClose lets it close otherwise`
- `guardWindowClose closes when the callback throws`
- `closeWindow destroys`

Run `cargo test -p filemanager --lib`, then
`npm run typecheck && npm run lint && npm run test`.

## Out of scope

- The prompt itself, the dirty-state tracking, and where the guard is
  registered. These belong to the ui-implementer's U task.
- Any backend `on_window_event` hook.
- The CSP.

## Risks

- **Tauri API drift.** The exact permission and the behaviour of
  `onCloseRequested` must match the installed versions; the manual check
  guards this.
- **A guard that never resolves would trap the user.** The wrapper closes on
  a thrown error. A UI that awaits a dialog must always resolve it.
