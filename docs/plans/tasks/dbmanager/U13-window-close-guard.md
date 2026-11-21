# U13 — Ask before closing the window with unsaved edits

Status: **superseded (2026-11-21):** the UI moves to GPUI; the ui-designer rewrites this task for `apps/filemanager` (its design decisions carry over). Do not run this plan. Previous status: awaiting approval (new 2026-11-16: the human's answer "prompt on
window close")
Project: dbmanager   Depends on: U6 (landed), M14 (landed, including the
reviewer-approved capability change), M10 (landed)

## Goal

When the user closes the FileManager window while the Database Manager holds
unsaved edits, keep the window open and ask. The prompt works even when
another tool is on screen. **Close without saving** then closes the window.

## Context

**The product.** FileManager is a portable Windows desktop app (Tauri v2;
React + TypeScript + Vite in `apps/desktop/src/`). It hosts several tools in
one window. The app shell (`app/AppShell.tsx`) renders the tool navigation
and the active tool's view. Switching tools unmounts the old view.

The **Database Manager** keeps its state in a module-level store
(`tools/dbmanager/store.ts`, U2–U6), so a draft survives tool switches. The
store exposes `dirtyCount()` (changed fields plus staged links) and the
draft's place: the open object's `label`, or a create form for a table
display name.

Today, closing the window loses a dirty draft silently. The human decided
the close must **ask**.

You own `apps/desktop/src/` **except `lib/`**: never edit it. Tools never
import each other. Shared code goes in `app/`, and nothing in `app/` imports
from `tools/`.

### The API used (`apps/desktop/src/lib/tauri.ts`, M14)

```ts
/**
 * Intercepts closing the main window. `shouldKeepOpen` runs on every close
 * request; return true to keep the window open, false to let it close. If it
 * throws, the window closes (never trap the user). Resolves to an unlisten
 * function. Only one guard should be active at a time.
 */
export function guardWindowClose(shouldKeepOpen: () => boolean | Promise<boolean>): Promise<UnlistenFn>;
/** Closes the main window now, without running the guard again. */
export function closeWindow(): Promise<void>;
```

M14 granted the one capability this needs (`core:window:allow-destroy`).
**Do not change capabilities or the CSP.** Outside the Tauri webview (tests,
the screenshot mock without a stub), `guardWindowClose` may reject. Ignore
that rejection, as `app/DropZone.tsx` ignores a missing drag-drop source.

### The design (brief excerpt)

**Visual system** (`DESIGN.md`; no new tokens). The shared `ConfirmDialog`
(U1): a native modal `<dialog>`; focus starts on the cancel button and
returns to the opener; it has `cancelLabel`, and `confirmVariant: "danger"`.
The danger variant is for actions that discard the user's work.

**Who owns the guard.** There is exactly **one** guard per app, so it lives in
the shell, not in a tool:

- **`app/closeGuard.tsx`** (shared):

  ```ts
  export interface UnsavedWork { toolLabel: string; description: string }
  /** Registers a source of unsaved work. `describe` returns null when there is none. Returns an unregister function. */
  export function registerUnsavedWork(id: string, describe: () => UnsavedWork | null): () => void;
  export function CloseGuard(): JSX.Element | null;
  export function resetCloseGuard(): void;   // tests
  ```

  - The registry is module-level, so a tool can register from its store and
    stay registered while its view is unmounted.
  - Registering the same `id` again replaces the earlier entry.
- **`CloseGuard`** is rendered **once**, inside `AppShell` (after `main`).
  - On mount it calls `guardWindowClose(shouldKeepOpen)`, and unlistens on
    unmount.
  - `shouldKeepOpen()` collects every non-null `describe()`:
    - none → returns `false`, and the window closes;
    - any → it opens the prompt with those entries and returns `true`.
  - While the prompt is open, any further close request returns `true` and
    changes nothing.
- **The Database Manager registers** once, at store module load (not on view
  mount), with `registerUnsavedWork("dbmanager", describe)`.
  `resetDbmanagerStore()` keeps the registration, and it reads the live
  state. `describe` returns:
  - `null` when `dirtyCount() === 0`;
  - an existing object: `{ toolLabel: "Database Manager", description: "Your
    changes to “{label}” are not saved." }`;
  - a create form: `{ toolLabel: "Database Manager", description: "Your new
    object in {table display name} is not saved." }`.

**The prompt** (a `ConfirmDialog`):

- title: "Close FileManager with unsaved changes?";
- `body`: one sentence, "If you close now, your unsaved changes are lost."
  (the description target);
- `children`: a list with one item per entry, "{toolLabel}: {description}";
- `cancelLabel`: "Keep editing", focused first;
- `confirmLabel`: "Close without saving", with `confirmVariant="danger"`;
- **Keep editing** closes the prompt. Nothing changes, and focus returns to
  whatever had it before the close request;
- **Close without saving** calls `closeWindow()` and shows it busy
  (`confirmBusy`).
  - If `closeWindow()` rejects, the prompt stays, the confirm is no longer
    busy, and an error line appears in `children`: an `x-circle` icon and
    "The window could not be closed. Your changes are still here.".
- The prompt does **not** offer Save. Saving can fail, ask about conflicts,
  or need the user's attention. To save first, the user chooses Keep editing
  and saves.
- Escape is **Keep editing** (the shared dialog's behaviour).

## Files

Create:

- `apps/desktop/src/app/closeGuard.tsx`, `app/closeGuard.test.tsx`;
- `apps/desktop/src/tools/dbmanager/closeGuard.test.ts`, covering the
  registration's `describe`.

Edit:

- `app/AppShell.tsx` (render `<CloseGuard />` once), `app/AppShell.test.tsx`;
- `tools/dbmanager/store.ts` (register at module load);
- `styles/controls.css`, only if the list needs spacing (use existing
  classes where possible);
- the root `DESIGN.md`: change "Planned (dbmanager U13)" to "Landed
  (dbmanager U13)" in the Confirmation dialog section, correcting any detail
  that differs. Change nothing else there.

Do not touch `lib/`, capabilities, the CSP, or other tools.

## Skill

- Run `/impeccable clarify` on the prompt copy (keep the meaning; report
  proposed changes), then `/impeccable harden`: a close request while the
  prompt is open, `closeWindow` failing, a guard registration failing, a save
  pending. Run both from `apps/desktop` with `--target
  src/app/closeGuard.tsx`.
- Read `reference/craft-floor.md` before the first UI edit.
- Extend the established world: the Schedule and Database Manager
  confirmations are the incumbent pattern.
- Run `/impeccable audit` and the detector on the changed files before done.
- If the skill is missing, install it (`npx impeccable install`) or follow
  its references by hand, and say so.

## Acceptance criteria

**When the close goes through:**

- `AppShell` registers exactly one guard (`guardWindowClose` called once),
  and unregisters it on unmount.
- With no unsaved work, `shouldKeepOpen()` returns `false` and no dialog
  opens.

**When the prompt opens:**

- With a dirty Database Manager draft for "Dune", `shouldKeepOpen()`
  returns `true` and the prompt opens:
  - the title "Close FileManager with unsaved changes?";
  - the item "Database Manager: Your changes to “Dune” are not saved.";
  - focus on Keep editing.
- The same holds while the **Pipeline** tool is showing, with the Database
  Manager view unmounted.
- A create draft reads "Your new object in Books is not saved.".

**The choices:**

- Keep editing (and Escape) closes the prompt, nothing else changes, and
  focus returns to the previously focused element.
- Close without saving calls `closeWindow()` once and shows busy.
- A rejected `closeWindow()` shows the error line, and the prompt stays.
- A second close request while the prompt is open returns `true` and opens
  no second dialog.
- A rejected `guardWindowClose` (no Tauri) is ignored, and the app renders
  normally.

**Boundaries and accessibility:**

- `app/closeGuard.tsx` imports nothing from `tools/`.
- jsx-a11y lint passes. The dialog is labelled by its title and described
  by its one-sentence body.

## Tests proving completion

- `app/closeGuard.test.tsx` (mock `lib/tauri`):
  - `registers one guard and unlistens`;
  - `no unsaved work lets the window close`;
  - `unsaved work keeps it open and prompts`;
  - `keep editing restores focus`;
  - `close without saving calls closeWindow`;
  - `closeWindow failure shows the error and keeps the prompt`;
  - `repeat close request while prompting`;
  - `guard registration failure is ignored`;
  - `re-registering an id replaces it`.
- `app/AppShell.test.tsx`:
  - `renders the close guard once`.
- `tools/dbmanager/closeGuard.test.ts`:
  - `describe is null when clean`;
  - `describes an object draft`;
  - `describes a create draft`;
  - `works while the view is unmounted`.

Run: `npm run typecheck && npm run lint && npm run test`.

## States covered

- **Guard:** idle (closes), prompting, closing (busy), close failed.
- **Registration:** a clean draft, a dirty object, a dirty create form.

## Out of scope

- Saving from the prompt (deliberately not offered).
- Registering other tools: they have no unsaved work today. The registry is
  ready for them.
- Any capability or CSP change (M14).
