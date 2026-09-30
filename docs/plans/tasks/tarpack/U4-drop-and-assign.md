# U4 — Drag-and-drop and per-row assignment

Status: awaiting approval
Project: tarpack   Depends on: U3 (landed), M6 (landed; this task reads its
partial-results fields `entriesWithheld`, `failedEntries`, and `errorCount`),
and M5's review follow-up (landed before M6; it generates `UnmatchedReason`).
May run alongside U5.

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
  present; that comes back as unmatched with the reason `alreadyAssigned`.
- Every unmatched item carries a typed **reason** (below). You map each
  reason to copy; you never infer a reason yourself.

**Working while the manifest has errors (decided by the human).** A manifest
with errors still opens. Entries with no error of their own **pass** and are
in `session.manifest.entries`; entries with errors **fail**, are left out of
the table, and are listed by U2's error report. Errors do not block the
build: the archive is built from the passed entries. So the user keeps
preparing the build while fixing the manifest: dropping files, Browse…, and
Clear all work on the passed entries, whatever `errorCount` is. The backend
enforces the rest:

- drop matching never considers failed entries, so a file that belongs to a
  failed entry comes back **unmatched**;
- `assign` or `clear` with a failed entry's id rejects with `UnknownEntry`
  (it cannot happen from the table, which lists only passed entries);
- the remembered file of a failed entry is kept, and comes back on its own
  once the entry is fixed and the manifest reloaded.

When `entries` is empty (a manifest-level error **withholds** every entry,
every entry failed, or the manifest lists none), there is nothing a drop could
match.

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
  - `matched: [id, path][]`;
  - `unmatched: Unmatched[]`, where
    `Unmatched = { path: string, reason: UnmatchedReason }` and
    `UnmatchedReason = "alreadyAssigned" | "noEntry" | "notFound" | "linkNotFollowed" | "folderNoMatch" | "unreadable" | "notUnicode"`
    (both generated; import them, never redeclare them);
  - `ambiguous: { id, candidates: path[] }[]`.

  The reasons mean:

  | Reason | What happened |
  | --- | --- |
  | `alreadyAssigned` | The file's name fits only entries that already have a present file; nothing was replaced. |
  | `noEntry` | A file dropped directly whose name fits no entry in the table. |
  | `notFound` | A dropped path that no longer exists. |
  | `linkNotFollowed` | A dropped path that is a link or junction; links are never followed. |
  | `folderNoMatch` | A dropped folder in which no file's name fits any entry. |
  | `unreadable` | A dropped path, or a folder or file inside a dropped folder, that could not be read. Other matches from the same drop still applied. |
  | `notUnicode` | A file that would have been assigned, but its path has characters the app cannot store or send. It was not assigned. |

  The paths in `unmatched` and `ambiguous` are display strings: show them,
  never pass them back to `lib`. A path that was not valid Unicode arrives
  with U+FFFD in it; render it as U3 does such names.
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
  `--accent`, `--surface`, `--text`, `--text-muted`, `--radius`,
  `--font-size-md` and `--font-size-lg`, and the `--space-*` scale.
- U2 built the shared vocabulary; use it and add no parallel version:
  - `src/app/Button.tsx`: `variant: "primary" | "secondary" | "quiet"`,
    `icon?`;
  - `src/app/icons.tsx`: `Icon` with `name: IconName`, including `folder`,
    `check-circle`, `alert-triangle`, `info`, and `x`;
  - the `.num` utility class for counts.
- **Design context.** The root `DESIGN.md` records the visual system ("The
  Packing List"): flat, tonal, with no shadows at rest. The drop overlay and
  the confirmation dialog are the only things allowed to float. This is an
  Operate surface extending that world; the overlay is a state, not a
  moment of delight.

### Components

| Component | Path | Props | States |
| --- | --- | --- | --- |
| `DropZone` | `src/app/DropZone.tsx` (shared) | `enabled, disabledReason, label, onDrop(paths)` | idle / hover / disabled |
| `DropResult` | `src/tools/tarpack/DropResult.tsx` | `outcome, hasFailedEntries, onDismiss` | matched / unmatched, one line per reason / unmatched with failed entries / ambiguous |

`TarpackView` passes `hasFailedEntries = manifest.failedEntries.length > 0`.
`DropZone` is shared and knows nothing about manifests: `TarpackView` computes
`enabled` and `disabledReason` as below.

### Behaviour

- `DropZone` subscribes to `onDragDrop`. While a drag is over the window, it
  shows a full-window overlay: the `--drop-overlay` fill, a 2 px dashed
  `--accent` inset border, and a centered label, "Drop files or folders to
  match them to the manifest". The label sits on a `--surface` plate with
  `--radius` and `--space-3 --space-4` padding, at `--font-size-lg`, with the
  `folder` icon before it. The plate keeps the text at AA contrast over the
  translucent fill. The overlay does not steal focus, and it disappears on
  leave or drop.
- **Enabled** whenever a manifest is loaded, `entries` is non-empty, and no
  build is running, **including when the manifest has errors**. Errors alone
  never disable drops, Browse…, or Clear.
- **Disabled** otherwise. The overlay still appears on a drag, shows the
  reason in place of the drop label (same plate, with the `info` icon), and
  drops are ignored (no `lib` call). The reason, in this order:
  - no manifest: "Open a manifest first";
  - `entries` empty and (`entriesWithheld` or `failedEntries` non-empty):
    "Fix the manifest errors first. No files can be matched yet.";
  - `entries` empty otherwise: "This manifest lists no files";
  - building (wired in U5; accept the flag now): "A build is running".
- **After a drop**, `DropResult` appears above the table. It is a
  `role="status"` region with `aria-live="polite"`. It has one line for
  matched, one line per **unmatched reason** that occurs (grouped by
  `reason`, in the table's order below), and one line for ambiguous, each
  only when non-empty. For example:
  - "3 matched"
  - "1 not in the manifest: notes.txt"
  - "2 already assigned, left unchanged: app.dll, core.dll"
  - "1 ambiguous: app.dll could be 2 files — use Browse"

  The unmatched copy, as `{n} {phrase}: {names}`:

  | Reason | Phrase | Icon |
  | --- | --- | --- |
  | `noEntry` | "not in the manifest" ("not matched" with failed entries, below) | `info`, `--text-muted` |
  | `alreadyAssigned` | "already assigned, left unchanged" | `info`, `--text-muted` |
  | `folderNoMatch` | "folder(s) with nothing to match" (singular "folder with nothing to match") | `info`, `--text-muted` |
  | `linkNotFollowed` | "link(s) not followed" (singular "link not followed") | `info`, `--text-muted` |
  | `notFound` | "no longer found" | `alert-triangle`, `--warn` |
  | `unreadable` | "could not be read" | `alert-triangle`, `--warn` |
  | `notUnicode` | "can't be assigned: the path has unsupported characters. Rename it" | `alert-triangle`, `--warn` |

  Hold the mapping in one exhaustive `Record<UnmatchedReason, …>` in
  `DropResult.tsx`, so a reason added in Rust fails `npm run typecheck`
  until it has copy. Never render `reason` itself. `/impeccable clarify`
  may tighten the wording; the meaning and the grouping stay.

  Paths show as file (or folder) names, with the full path in `title`. Each
  line starts with an `Icon`: `check-circle` in `--ok` for matched, the
  table's icon for each unmatched reason, and `alert-triangle` in `--warn`
  for ambiguous. Counts use `.num`. It sits on `--surface` with a 1 px
  `--border` (no tinted fill and no side stripe). It is dismissed with a quiet
  `Button` using the `x` icon and the name "Dismiss drop result", and it is
  replaced by the next drop.
- **Unmatched files while entries have errors.** When `hasFailedEntries` is
  true, an unmatched file may belong to a failed entry, so "not in the
  manifest" would be wrong. Then:
  - the `noEntry` line reads "1 not matched: notes.txt" (or "N not
    matched: …") instead of "… not in the manifest: …";
  - one more line follows the unmatched lines, in `--text-muted` with the
    `info` icon, when a `noEntry` or `folderNoMatch` line is shown:
    "Files for entries with errors can't be matched until those errors are
    fixed."

  Do not try to work out which unmatched file belongs to which failed entry;
  that is matching logic, and it stays in Rust. The other reasons' lines
  are the same with or without failed entries.
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

- Tokens only, with font sizes from `--font-size-*`. Use `Button` and `Icon`;
  no `<button>` and no glyph icons.
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

`/impeccable` (new work inside the established world: extend an existing
surface), then `/impeccable clarify` for the result and overlay copy.

How to run it:

- Start with the skill's `impeccable context`, which loads the root
  `PRODUCT.md` and `DESIGN.md`.
- This is a local extension of an established surface. Per the skill's
  new-work flow, run no concept round or concept seed, write no direction
  contract, and do not rewrite `DESIGN.md`.
- Read the skill's `reference/craft-floor.md` before the first edit.
- If the skill is not installed, install it with `npx impeccable install`, or
  follow the reference docs from `github.com/pbakaus/impeccable` by hand. Say
  which in your report.

## Acceptance criteria

- A drag shows the overlay, and a leave or drop hides it.
- A drop calls `assignDropped` with exactly the dropped paths, and renders the
  returned session and outcome.
- Every bucket renders correctly, and every `UnmatchedReason` has its own
  line with its copy and icon; items with the same reason share one line.
  The live-region text matches the outcome. The mapping is an exhaustive
  `Record<UnmatchedReason, …>`.
- Disabled drops show the reason for each case (no manifest, entries
  withheld or every entry failed, no files) and make no `lib` call.
- With `errorCount > 0` and non-empty `entries`, drops, Browse…, and Clear
  are enabled and call `lib` exactly as with a clean manifest.
- With failed entries, the `noEntry` line reads "not matched" and the hint
  line appears (also after a `folderNoMatch` line alone); without failed
  entries it reads "not in the manifest" and there is no hint.
- The overlay label is on a `--surface` plate, and each result line has its
  icon, so meaning never depends on color or the dashed border alone.
- Browse and Clear call the right functions with the right id, and a cancelled
  dialog makes no call.
- A rejected `assign` with `NotAFile` shows "{source}: the chosen path is not
  a file." through `errorMessage`, and the table is unchanged.

## Tests proving completion

`npm run test`, with `onDragDrop` and `openFileDialog` mocked:

- `DropZone.test.tsx`: hover, leave, drop, and disabled.
- `DropResult.test.tsx`: each bucket; each of the seven unmatched reasons
  (copy and icon); two items with one reason on one line; combined buckets
  and reasons; a U+FFFD path renders; dismiss; and the `noEntry` wording and
  hint with and without failed entries.
- `TarpackView.assign.test.tsx`: the Browse flow, cancelled Browse, Clear,
  a `NotAFile` rejection, a drop plus Browse on a session with
  `errorCount > 0` and passed entries (enabled, `lib` called), and the
  disabled reasons for a withheld, an every-entry-failed, and a no-files
  session.
- Axe checks with the overlay and the result visible.

## States covered

Idle, drag hover, disabled (no manifest, entries withheld or every entry
failed, no files, building), enabled with manifest errors, result (each
bucket and each unmatched reason, with and without failed entries), dialog
cancelled, and command error.

## Out of scope

- The error report and notice (U2).
- The build bar (U5).
- Keyboard shortcuts (U6).
- Any matching logic in TS.
