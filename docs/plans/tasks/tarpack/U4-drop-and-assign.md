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

**Working while the manifest has errors (decided by the human).** A manifest
with errors still opens. Entries with no error of their own **pass** and are
in `session.manifest.entries`; entries with errors **fail**, are left out of
the table, and are listed by U2's error report. The user may keep preparing
the build while they fix the manifest: dropping files, Browse…, and Clear
all work on the passed entries. The backend enforces the rest:

- drop matching only considers passed entries. A dropped file meant for a
  failed entry comes back **unmatched**, because that entry is not in the
  manifest the backend can trust yet;
- `assign` or `clear` with a failed entry's id rejects with `UnknownEntry`
  (it cannot happen from the table, which lists only passed entries);
- the remembered file of a failed entry is kept, and comes back on its own
  once the entry is fixed and the manifest reloaded;
- the archive still cannot be created while any error exists; that is the
  build bar's job (U5), not this task's.

When a manifest-level error **withholds** every entry
(`session.manifest.entriesWithheld` is `true`), `entries` is empty and there
is nothing a drop could match.

**What exists.**

- `TarpackView.tsx` holds the `TarpackSession` state. `EntryTable.tsx` (U3)
  calls `onBrowse(id)` and `onClear(id)`.
- From the session: `session.manifest` is `null` or has `entries` (passed
  entries only), `entriesWithheld: boolean`, `failedEntries` (use only its
  length), and `errorCount: number` (above 0 whenever the manifest has
  errors).
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
- Commands reject with `TarpackError`:
  `{ kind: TarpackErrorKind, message: string, entryId?: string }`, where
  `entryId` is omitted when absent. `src/tools/tarpack/errorMessages.ts` (U2)
  exports `errorMessage(error, entries)`, an exhaustive
  `Record<TarpackErrorKind, …>` of user copy; use it, and do not write your own
  copy or fallback. `src/app/Banner.tsx` (U2) shows an error banner:
  `tone, message, action?`. The kinds these commands can raise, and the copy
  `errorMessage` returns for them:

  | Kind | Raised by | Message |
  | --- | --- | --- |
  | `NoManifest` | any | "Open a manifest first." |
  | `UnknownEntry` | `assign`, `clear` | "That file is no longer in the manifest. Reload and try again." |
  | `NotAFile` | `assign` | "{source}: the chosen path is not a file." |
  | `Io` | any | "A file could not be read or written." |

  `{source}` is the entry's `source` for `entryId`, or "A file".
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
- **Enabled** whenever a manifest is loaded, `entries` is non-empty, and no
  build is running, **including when the manifest has errors**. Errors alone
  never disable drops, Browse…, or Clear.
- **Disabled** otherwise. The overlay still appears on a drag, shows the
  reason instead of the drop label, and drops are ignored (no `lib` call):
  - no manifest: "Open a manifest first";
  - `entries` empty and `errorCount > 0` (withheld, or every entry failed):
    "Fix the manifest errors first. No files can be matched yet.";
  - `entries` empty and `errorCount` 0: "This manifest lists no files";
  - building (wired in U5; accept the flag now): "A build is running".
- **After a drop**, `DropResult` appears above the table. It is a
  `role="status"` region with `aria-live="polite"` and one line per non-empty
  bucket, for example:
  - "3 matched"
  - "1 not in the manifest: notes.txt"
  - "1 ambiguous: app.dll could be 2 files — use Browse"

  Paths show as file names, with the full path in `title`. It can be
  dismissed, and it is replaced by the next drop.
- **Unmatched files while the manifest has errors.** When `errorCount > 0`
  and `failedEntries` is non-empty, an unmatched file may belong to a failed
  entry, so "not in the manifest" would be wrong. Then:
  - the unmatched line reads "1 not matched: notes.txt" (or "N not
    matched: …") instead of "… not in the manifest: …";
  - one more line follows it: "Files for entries with errors can't be
    matched until those errors are fixed."

  Do not try to work out which unmatched file belongs to which failed entry;
  that is matching logic, and it stays in Rust. The "already assigned"
  reason, when present, is shown as before.
- **Browse…** on a row opens `openFileDialog`, starting in the folder of the
  row's current or last assignment when there is one. The chosen path goes to
  `assign(id, path)`. A cancelled dialog does nothing.
- **Errors** from `assignDropped`, `assign`, or `clear` show as an error
  `Banner` with `errorMessage(error, entries)`, the backend `message` in a
  collapsed "Details" disclosure, and the session left as it was.
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
- Disabled drops show the reason and make no `lib` call, for each reason
  above.
- With `errorCount > 0` and non-empty `entries`, drops, Browse…, and Clear
  are enabled and call `lib` exactly as with a clean manifest.
- With `errorCount > 0` and failed entries, the unmatched line reads
  "not matched" and the hint line appears; with `errorCount` 0 it reads
  "not in the manifest" and there is no hint.
- Browse and Clear call the right functions with the right id, and a cancelled
  dialog makes no call.
- A rejected `assign` with `NotAFile` shows "{source}: the chosen path is not
  a file." through `errorMessage`, and the table is unchanged.

## Tests proving completion

`npm run test`, with `onDragDrop` and `openFileDialog` mocked:

- `DropZone.test.tsx`: hover, leave, drop, and disabled (each reason).
- `DropResult.test.tsx`: each bucket, combined buckets, dismiss, and the
  unmatched wording and hint with and without manifest errors.
- `TarpackView.assign.test.tsx`: the Browse flow, cancelled Browse, Clear,
  a `NotAFile` rejection, and a drop plus Browse on a session with
  `errorCount > 0` and passed entries (enabled, `lib` called).
- Axe checks with the overlay and the result visible.

## States covered

Idle, drag hover, disabled (no manifest, entries withheld or all failed, no
files, building), enabled with manifest errors, result (each bucket, with and
without manifest errors), dialog cancelled, and command error.

## Out of scope

- The error report and notice (U2).
- The build bar (U5).
- Keyboard shortcuts (U6).
- Any matching logic in TS.
