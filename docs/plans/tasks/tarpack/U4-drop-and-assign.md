# U4 — Drag-and-drop and per-row assignment

Status: awaiting approval
Project: tarpack   Depends on: U3 (landed), M6 (landed). May run alongside U5.

## Goal

Let the user assign Windows files by dropping files or folders anywhere on the
window, or by picking a file for one row, and show clearly what a drop did.

## Context

**How matching works.** Rust does all of it; you only show the result.

- A dropped file is matched to manifest entries by file name, ignoring case.
- A dropped folder is searched for the files the manifest still needs.
- Rust never guesses. A name that could fit more than one entry is reported as
  **ambiguous**, and a file the manifest does not list is reported as
  **unmatched**. A drop never replaces a file that is already assigned and
  present; that comes back as unmatched with the reason "already assigned".

**What exists.**

- `TarpackView.tsx` holds the `TarpackSession` state. `EntryTable.tsx` (U3)
  calls `onBrowse(id)` and `onClear(id)`.
- `src/lib/tarpack.ts` (M6; read it for exact signatures) provides:
  - `assignDropped(paths)`, which returns `{ session, outcome }`
  - `assign(id, path)`, which returns a session
  - `clear(id)`, which returns a session
- The outcome's type is `DropOutcome` in `src/lib/generated/`:
  `matched: [id, path][]`, `unmatched: path[]` (with a reason where given), and
  `ambiguous: { id, candidates: path[] }[]`.
- `src/lib/tauri.ts` provides:
  - `onDragDrop(handler)`, which yields
    `{ type: "enter"|"over"|"leave"|"drop", paths }`;
  - `openFileDialog({ defaultPath })`.
- The tokens you use are in `src/styles/tokens.css`: `--drop-overlay`,
  `--accent`, `--surface`, `--text`, `--text-muted`, `--radius`, and the
  `--space-*` scale.

### Components

| Component | Path | Props | States |
| --- | --- | --- | --- |
| `DropZone` | `src/app/DropZone.tsx` (shared) | `enabled, disabledReason, label, onDrop(paths)` | idle / hover / disabled |
| `DropResult` | `src/tools/tarpack/DropResult.tsx` | `outcome, onDismiss` | matched / unmatched / ambiguous |

### Behaviour

- `DropZone` subscribes to `onDragDrop`. While a drag is over the window, it
  shows a full-window overlay: the `--drop-overlay` fill, a 2 px dashed
  `--accent` inset border, and a centered label, "Drop files or folders to
  match them to the manifest". The overlay does not steal focus, and it
  disappears on leave or drop.
- **Disabled** when there is no manifest, or the manifest has errors. The
  overlay then reads "Open a valid manifest first", and drops are ignored.
- **After a drop**, `DropResult` appears above the table. It is a
  `role="status"` region with `aria-live="polite"` and one line per non-empty
  bucket, for example:
  - "3 matched"
  - "1 not in the manifest: notes.txt"
  - "1 ambiguous: app.dll could be 2 files — use Browse"

  Paths show as file names, with the full path in `title`. It can be
  dismissed, and it is replaced by the next drop.
- **Browse…** on a row opens `openFileDialog`, starting in the folder of the
  row's current or last assignment when there is one. The chosen path goes to
  `assign(id, path)`. A cancelled dialog does nothing.
- **Clear** on a row calls `clear(id)`. The accessible name is "Clear assigned
  file for <source>". The copy and tooltip make clear that it only forgets the
  selection: "Forget this file (nothing is deleted)".

**Rules that bind this task.**

- Tokens only.
- Everything is keyboard reachable: Browse and Clear are real buttons, and the
  drop result is reachable and dismissible by keyboard.
- The overlay's meaning is never color-only; it always has its label.
- Motion respects `prefers-reduced-motion`: the overlay fades at most 120 ms,
  and not at all when reduced motion is on.
- Call only `lib/` functions.

## Files

- `src/app/DropZone.tsx`
- `src/tools/tarpack/DropResult.tsx`
- Edits to `TarpackView.tsx` and `EntryTable.tsx` to wire Browse and Clear
- Tests next to each

## Skill

`impeccable:impeccable` with `craft`, then `impeccable:clarify` for the result
and overlay copy.

## Acceptance criteria

- A drag shows the overlay, and a leave or drop hides it.
- A drop calls `assignDropped` with exactly the dropped paths, and renders the
  returned session and outcome.
- Every bucket renders correctly, including the "already assigned" reason. The
  live-region text matches the outcome.
- Disabled drops show the reason and make no `lib` call.
- Browse and Clear call the right functions with the right id, and a cancelled
  dialog makes no call.

## Tests proving completion

`npm run test`, with `onDragDrop` and `openFileDialog` mocked:

- `DropZone.test.tsx`: hover, leave, drop, and disabled.
- `DropResult.test.tsx`: each bucket, combined buckets, and dismiss.
- `TarpackView.assign.test.tsx`: the Browse flow, cancelled Browse, and Clear.
- Axe checks with the overlay and the result visible.

## States covered

Idle, drag hover, disabled, result (each bucket), and dialog cancelled.

## Out of scope

- The build bar (U5).
- Keyboard shortcuts (U6).
- Any matching logic in TS.
