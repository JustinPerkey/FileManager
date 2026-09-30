# U4 — Drag-and-drop and per-row assignment

Status: awaiting approval (amended 2026-09-30: row-action names fixed by U3;
scroll model; after the first implementation run: the ambiguous line names
entries, Browse's start folder, the per-line name cap, the empty-outcome
line, and the `notUnicode` copy; after review: focus after Dismiss, the
"Full paths" disclosure, one drop at a time with a "Matching…" line, the
drop state snapshot, and the result's error and test details)
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
  calls `onBrowse(id)` and `onClear(id)`. `TarpackView`'s root,
  `<section className="tool-view tarpack">`, is the view's single scroll
  container: the table has no scroll box of its own, and `DropResult` sits
  in the page flow above the table (no `overflow`, no `max-height`, no
  sticky position).
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
  - `ambiguous: Ambiguity[]`, where `Ambiguity = { id, candidates: path[] }`
    (generated). `id` is always a **passed entry's id**; `candidates` are
    the dropped files, all with that entry's file name, that were not
    assigned because the match was not unique. It covers both directions:
    an entry that several files could fill (one record, several
    candidates), and a file that several entries could take (one record per
    entry, each listing that file; its `candidates` has length 1, because a
    single file that fits a single entry is always matched). An entry can
    be both at once. Either way the entry was left as it was, and the user
    resolves it with Browse… on that entry's row.

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
  copy or fallback. The same module exports
  `toTarpackError(e: unknown): TarpackError`: pass every caught rejection
  through it before showing it (a rejection may not be a `TarpackError`;
  unknown shapes become `Io`). Write no guard or kind list of your own.
  `src/app/Banner.tsx` (U2) shows an error banner:
  `tone, message, action?, onDismiss?`, with the Details disclosure as
  children (styled by `.banner__details` in `controls.css`). The kinds these commands can raise, and the copy
  `errorMessage` returns for them:

  | Kind | Raised by | Message |
  | --- | --- | --- |
  | `NoManifest` | any | "Open a manifest first." |
  | `UnknownEntry` | `assign`, `clear` | "That file is no longer in the manifest. Reload and try again." |
  | `NotAFile` | `assign` | "{source}: the chosen path is not a file." |
  | `Io` | any | "A file could not be read or written." |

  `{source}` is the entry's `source` for `entryId`, or "A file".
- The tokens you use are in `src/styles/tokens.css`: `--drop-overlay`,
  `--accent`, `--surface`, `--surface-sunken`, `--border`, `--text`,
  `--text-muted`, `--ok`, `--warn`, `--radius`, `--font-size-sm`,
  `--font-size-md` and `--font-size-lg`, and the `--space-*` scale.
- U2 built the shared vocabulary; use it and add no parallel version:
  - `src/app/Button.tsx`: `variant: "primary" | "secondary" | "quiet"`,
    `icon?`;
  - `src/app/icons.tsx`: `Icon` with `name: IconName`, including `folder`,
    `check-circle`, `alert-triangle`, `info`, and `x`;
  - the `.num` and `.mono` utility classes in `src/styles/base.css`.
- `TarpackView` (U2) owns `announce(text)`, which sets the text of its
  always-mounted, visually hidden polite announcer (it clears, then sets on
  the next frame, so a repeated text is announced again). Apart from the
  error banner (`role="alert"`), it is the view's only live region. Call it inside `TarpackView`; the
  optional `actionsRef` prop is a test-only seam, not for product code.
- **Stylesheets.** Tool styles go in `src/styles/tarpack.css`, and every
  selector there is scoped under the view root class `.tarpack` (for example
  `.tarpack .drop-result`). Shared component styles (`DropZone`) go in
  `src/styles/controls.css`.
- **Design context.** The root `DESIGN.md` records the visual system ("The
  Packing List"): flat, tonal, with no shadows at rest. Only the drop
  overlay, the confirmation dialog, and menus and popovers float; nothing in
  this task has a shadow. This is an
  Operate surface extending that world; the overlay is a state, not a
  moment of delight.

### Components

| Component | Path | Props | States |
| --- | --- | --- | --- |
| `DropZone` | `src/app/DropZone.tsx` (shared) | `enabled, disabledReason, label, onDrop(paths)` | idle / hover / disabled |
| `DropResult` | `src/tools/tarpack/DropResult.tsx` | `outcome, entries, hasFailedEntries, onDismiss` | matched / unmatched, one line per reason / unmatched with failed entries / ambiguous (several files for an entry; a file for several entries) / nothing matched; "Full paths" collapsed / expanded |
| `DropPending` | `src/tools/tarpack/DropResult.tsx` (second export) | none | shown while a drop is being matched (after 150 ms) |

**Drop state.** `TarpackView` keeps one drop state:
`null | { kind: "pending" } | { kind: "result", outcome, entries, hasFailedEntries }`.
When `assignDropped` resolves, it stores, all from that one result,
`outcome`, `entries = session.manifest.entries`, and
`hasFailedEntries = session.manifest.failedEntries.length > 0` (so every
ambiguous `id` is in `entries`, and a later reload does not change a result
already on screen). It passes the stored values to `DropResult` and to
`dropResultText`; it never recomputes them from the live session.
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
  - building (wired in U5; accept the flag now): "A build is running";
  - a drop still being matched: "Still matching the last drop".
- **After a drop**, `DropResult` appears above the table. It is **not** a
  live region (a region that mounts with its text is not reliably
  announced). `DropResult.tsx` also exports
  `dropResultText(outcome, entries, hasFailedEntries): string`: the same
  lines as plain text (the same names, the same "and N more"), each ending
  in a full stop, joined by a space. After each drop resolves, `TarpackView`
  calls `announce(dropResultText(...))`. It has one line for matched, one
  line per **unmatched reason** that occurs (grouped by `reason`, in the
  table's order below), and one line for ambiguous, each only when
  non-empty. For example:
  - "3 matched"
  - "1 not in the manifest: notes.txt"
  - "2 already assigned, left unchanged: app.dll, core.dll"
  - "1 ambiguous, not assigned — use Browse… on its row: /opt/gw/bin/app.dll (2 files)"
  - "2 ambiguous, not assigned — use Browse… on their rows: /opt/a/app.dll (app.dll fits more than one entry), /opt/b/app.dll (app.dll fits more than one entry)"

  **When all three buckets are empty** (rare; nothing in the drop could be
  reported), `DropResult` shows one line, "Nothing was matched", with the
  `info` icon; the icon **and the text** are `--text-muted`, like the hint
  line. `dropResultText` returns "Nothing was matched." The hint line and
  the "Full paths" disclosure never appear with it.

  **Names per line are capped at 8.** A line with more than 8 names shows
  the first 8 (in the outcome's order) followed by ", and N more", where N
  is the rest (", and 1 more" for one). The count at the start of the line
  is always the full count. Only the shown names are in the line and in
  the announcement; every name, hidden or not, is listed in the "Full
  paths" disclosure below. The cap applies to every line, ambiguous
  included.

  The unmatched copy, as `{n} {phrase}: {names}`:

  | Reason | Phrase | Icon |
  | --- | --- | --- |
  | `noEntry` | "not in the manifest" ("not matched" with failed entries, below) | `info`, `--text-muted` |
  | `alreadyAssigned` | "already assigned, left unchanged" | `info`, `--text-muted` |
  | `folderNoMatch` | "folder(s) with nothing to match" (singular "folder with nothing to match") | `info`, `--text-muted` |
  | `linkNotFollowed` | "link(s) not followed" (singular "link not followed") | `info`, `--text-muted` |
  | `notFound` | "no longer found" | `alert-triangle`, `--warn` |
  | `unreadable` | "could not be read" | `alert-triangle`, `--warn` |
  | `notUnicode` | "not assigned, unsupported characters in its path — rename it or its folder" (plural "not assigned, unsupported characters in their paths — rename them or their folders") | `alert-triangle`, `--warn` |

  Hold the mapping in one exhaustive `Record<UnmatchedReason, …>` in
  `DropResult.tsx`, so a reason added in Rust fails `npm run typecheck`
  until it has copy. Never render `reason` itself. `/impeccable clarify`
  may tighten the wording; the meaning and the grouping stay.

  For example: "1 not assigned, unsupported characters in its path — rename
  it or its folder: r�sum�.pdf". The unsupported character may be in a
  folder name rather than the file name, which is why the copy names both.

  **The ambiguous line** names **entries**, not dropped files, because the
  entry's row is where the user acts:

  - Lead: "{n} ambiguous, not assigned — use Browse… on its row" (n = 1) or
    "… on their rows" (n > 1), where n is `ambiguous.length`, then ": " and
    the items.
  - One item per `Ambiguity`, in the outcome's order. Look up the entry in
    `entries` by `id` and show its `targetPath` in `.mono` (unique among
    listed entries, and the same text as the row's Browse… name, "Browse…
    for {targetPath}"; `source` may repeat, so it is not used).
  - After the target path, in parentheses:
    - `candidates.length > 1`: "({k} files)", k in `.num`;
    - `candidates.length === 1`: "({file name} fits more than one entry)",
      the file name taken from that candidate.

    Do not try to tell the two directions apart beyond `candidates.length`,
    and do not group records by name; that is matching logic, and it stays
    in Rust. The two phrasings above are always true for their case.
  - Each item's `title` is "Matching files:" followed by each candidate's
    full path, one per line (`\n`).
  - If an `id` is not in `entries` (it should not happen), show the first
    candidate's file name instead of the target path, with the same
    parenthesis and `title`. Never show the raw `id`.

  Other paths show as file (or folder) names, with the full path in
  `title`. Each line starts with an `Icon`: `check-circle` in `--ok` for matched, the
  table's icon for each unmatched reason, and `alert-triangle` in `--warn`
  for ambiguous. Counts use `.num`. It sits on `--surface` with a 1 px
  `--border` (no tinted fill and no side stripe). It is dismissed with a quiet
  `Button` using the `x` icon and the name "Dismiss drop result". A new drop
  removes it when the drop starts (below).

  **Full paths (keyboard and screen-reader access).** `title` is for the
  mouse only; the same information is reachable through a disclosure.
  Whenever the result has at least one unmatched or ambiguous item,
  `DropResult` ends with a native `<details>` whose `<summary>` reads
  "Full paths" (the same pattern as the error banner's "Details"; `summary`
  is not a `Button`, and that is intended). It is collapsed for each new
  result, is not announced, and is left out of `dropResultText`. Inside, in
  the same order as the lines:

  - one group per unmatched line: a `<p>` with that line's lead (the text
    before ": ", for example "2 already assigned, left unchanged"), then a
    `<ul>` with **every** path in the group (no cap), each in `.mono`;
  - for ambiguous: a `<p>` with the ambiguous lead, then a `<ul>` with one
    item per `Ambiguity`: the target path (or the fallback name) in
    `.mono`, followed by a nested `<ul>` of **every** candidate's full path
    in `.mono`.

  Matched files are not listed (the table shows them). Paths wrap
  (`overflow-wrap: anywhere`), at `--font-size-sm`. Style it in
  `tarpack.css` as `.tarpack .drop-result__paths`, with the same values as
  `.banner__details` in `controls.css`: `summary` at least 24 px high,
  padding `--space-1 --space-3`, `--radius`, `--surface-sunken` on hover,
  `--text-muted` for the group leads, and the global focus ring on
  `summary`.

  **Focus after Dismiss.** Dismissing unmounts the focused button. When
  focus is inside the result as it is dismissed or removed (on Dismiss or
  on the "Full paths" summary), move it, computed before the result
  unmounts; when focus is elsewhere (U6 adds Escape to dismiss), leave it
  where it is:

  1. to the first tabbable element after the result in document order,
     inside the view root (normally row 1's "Browse… for {targetPath}"
     button; once U6 adds the roving row, the table's tab stop);
  2. if nothing tabbable follows, to the last tabbable element before the
     result (for example the error report's toggle, or the header's "Edit
     in editor"; the header always has buttons when a manifest is loaded).

  Tabbable means a `button`, `a[href]`, `input`, `select`, `textarea`,
  `summary`, or `[tabindex]` that is not disabled, has no
  `tabindex="-1"`, and is not inside `[hidden]` or `[inert]`; elements
  inside the result itself do not count. Use `preventScroll: false` so the
  target scrolls into view. The target shows the normal focus ring. The
  view root `<section>` gets **no** `tabIndex`, is never focused, and no
  focus outline is suppressed anywhere. The same rule applies when a new
  drop removes a result that holds focus.
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
- **Browse…** on a row opens `openFileDialog`. When the row's entry has a
  current assignment (`SessionEntry.assigned` is not `null`), pass
  `defaultPath` = that assignment's folder; otherwise pass no `defaultPath`
  and let the system dialog choose. There is no "last assignment": after
  Clear, `assigned` is `null` and Browse… has no default. Compute the folder
  with `assignedFolder(assigned): string | undefined`, a new export of
  `src/tools/tarpack/pathParts.ts`: everything before the last `\` or `/`,
  keeping the separator when only a drive prefix remains (`C:\a.txt` gives
  `C:\`; `C:\tools\a.txt` gives `C:\tools`; a UNC share root
  `\\srv\share\a.txt` gives `\\srv\share\`); `undefined` when there is
  no separator or the path contains U+FFFD (a lossy string is never passed
  back to `lib`). Work on code points, as `pathParts` does. The folder may
  no longer exist (a Missing row); pass it anyway, the dialog copes. The
  chosen path goes to `assign(id, path)`. A cancelled dialog does nothing.
- **One drop at a time.** From the moment `onDrop` calls `assignDropped`
  until it settles:
  - the drop state is `{ kind: "pending" }`: the previous result is removed
    at once (moving focus as in "Focus after Dismiss" if it held focus);
  - after 150 ms still pending (the view's skeleton delay), `DropPending`
    shows in the result's place: one line, `info` icon and text in
    `--text-muted`, "Matching dropped files…", no Dismiss and no
    disclosure, not focusable, not a live region;
  - after 1 s still pending, `TarpackView` calls
    `announce("Matching dropped files…")` once;
  - drops are **disabled** with the reason "Still matching the last drop"
    (the overlay shows it on a drag, as for the other reasons), so a second
    result can never overwrite the first;
  - Browse… and Clear stay enabled (the backend applies them, then retries
    the drop's matching against the new session).

  On resolve, the state becomes the result and the announcer gets its
  text; on reject, the state returns to `null` (no result shown, so no old
  result sits next to the error) and the error banner shows.
- **Errors** from `assignDropped`, `assign`, or `clear` show as an error
  `Banner` with `errorMessage(error, entries)`, the backend `message` in a
  collapsed "Details" disclosure, and the session left as it was. If
  Browse… or Clear change the session several times while one drop's
  folder walk runs, the backend gives up after three attempts and rejects
  with `Io`; the banner shows the `Io` copy, "A file could not be read or
  written.", and its Details holds the backend's text ("the session
  changed while the dropped files were being matched; drop them again").
  Accept that as is: do not special-case it or write copy for it.
- **Clear** on a row calls `clear(id)`. It only forgets the selection;
  nothing on disk is deleted.
- **Row-action names are U3's; keep them.** U3 already renders both buttons
  with these accessible names and tooltip. Do not change the labels, the
  names, or the `title`; this task only wires the handlers:

  | Button | Visible label | Accessible name | `title` |
  | --- | --- | --- | --- |
  | Browse | `Browse…` | "Browse… for {targetPath}" | none |
  | Clear | `Clear` | "Clear assigned file for {targetPath}" | "Forget this file (nothing is deleted)" |

  The names use `targetPath` because it is unique among listed entries,
  while `source` may repeat. Each name is the visible label plus a visually
  hidden suffix, so it starts with the visible words. In tests, find the
  buttons by these names.

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
- Edits to `TarpackView.tsx` to wire Browse and Clear through the
  `onBrowse`/`onClear` props `EntryTable.tsx` already exposes (change
  `EntryTable.tsx` only if wiring needs it; never its button names)
- `src/tools/tarpack/pathParts.ts`: add the `assignedFolder` export
  (leave `pathParts` unchanged), with tests in `pathParts.test.ts`
- Styles: `src/styles/controls.css` (`DropZone`), `src/styles/tarpack.css`
  (`DropResult`, `DropPending`, and the "Full paths" disclosure, scoped
  under `.tarpack`)
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
  The announcer text after a drop is `dropResultText` of the outcome, and
  `DropResult` has no live role. The mapping is an exhaustive
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
- The ambiguous line names each ambiguous entry by its `targetPath` from
  `entries`, with "({k} files)" for several candidates and "({file name}
  fits more than one entry)" for one; the candidates' full paths are in the
  item's `title`; an `id` missing from `entries` falls back to the first
  candidate's file name, never the `id`.
- A line with more than 8 names shows 8 and ", and N more", with the full
  count at its start; `dropResultText` says the same.
- An outcome with all three buckets empty shows and announces "Nothing was
  matched" (with the full stop in the announcement), and no hint line.
- Browse on a row with an assignment opens the dialog with `defaultPath` =
  `assignedFolder(assigned)`; on an unassigned row, with no `defaultPath`.
- Browse and Clear call the right functions with the right id, and a cancelled
  dialog makes no call. Both are found by their U3 accessible names ("Browse…
  for {targetPath}", "Clear assigned file for {targetPath}"), which this task
  leaves unchanged, and Clear keeps its `title`.
- A rejected `assign` with `NotAFile` shows "{source}: the chosen path is not
  a file." through `errorMessage`, and the table is unchanged.
- Assign (Browse) and Clear never change the announcer text, and a drop
  announces only its result: none of them repeats the manifest's error-count
  sentence, even when `errorCount > 0`.
- After Dismiss, focus is on the first tabbable element after the result
  (row 1's Browse… button when the table has rows), or on the last
  tabbable element before it when nothing follows; never on the view root,
  which has no `tabIndex`, and no outline is suppressed.
- With unmatched or ambiguous items, a collapsed "Full paths" disclosure
  lists every path of every group (past the 8-name cap), and every
  ambiguous candidate's full path under its entry; it is absent for a
  matched-only or empty outcome.
- While a drop is pending, drops are disabled with "Still matching the last
  drop", no second `assignDropped` call is made, the previous result is
  gone, and after 150 ms "Matching dropped files…" shows.
- A rejected `assignDropped` leaves no result on screen, only the error
  banner.
- A result on screen keeps its `entries` and `hasFailedEntries` from its own
  drop when the session later changes.
- Every new selector in `tarpack.css` starts with `.tarpack`.

## Tests proving completion

`npm run test`, with `onDragDrop` and `openFileDialog` mocked:

- `DropZone.test.tsx`: hover, leave, drop, and disabled.
- `DropResult.test.tsx`: each bucket; each of the seven unmatched reasons
  (copy and icon); two items with one reason on one line; combined buckets
  and reasons; a U+FFFD path renders; dismiss; and the `noEntry` wording and
  hint with and without failed entries; `dropResultText` for a combined
  outcome matches the rendered lines; no live role on the result; the
  ambiguous line for an entry with 2 candidates, for two entries sharing
  one candidate, and for an `id` not in `entries`; 9 names on one line
  (8 shown, ", and 1 more", count 9) and in `dropResultText`; the empty
  outcome, whose text and icon are both `--text-muted` (class check);
  `notUnicode` singular and plural copy; the "Full paths" disclosure:
  collapsed by default, expands with Enter on its summary, lists all 9
  paths of a 9-name line and each ambiguous candidate's full path, absent
  for matched-only and empty outcomes, and not in `dropResultText`.
- `pathParts.test.ts`: `assignedFolder` for `C:\a.txt`, `C:\tools\a.txt`,
  a forward-slash path, a UNC share root, a name with no separator
  (`undefined`), and a path with U+FFFD (`undefined`).
- `TarpackView.assign.test.tsx`: the Browse flow, cancelled Browse, Clear,
  a `NotAFile` rejection, a drop plus Browse on a session with
  `errorCount > 0` and passed entries (enabled, `lib` called), and the
  disabled reasons for a withheld, an every-entry-failed, and a no-files
  session; Browse on an assigned row passes its folder as `defaultPath`,
  and on an unassigned row passes none; **silence on assign**: on a session with `errorCount > 0`, a
  Browse assign and a Clear leave the announcer text unchanged, and a drop
  sets it to `dropResultText` only, with no "has N errors" sentence; a
  non-`TarpackError` rejection from `assign` shows the `Io` copy; the
  Browse flow asserts that the session `assign` returned renders (the row's
  Windows file and status change); **focus after Dismiss**: with rows,
  Dismiss by keyboard (focus Dismiss, press Enter) leaves
  `document.activeElement` equal to the first row's "Browse… for
  {targetPath}" button; with a session whose `entries` became empty after
  the drop (the result still shown), it is the last tabbable element
  before the result; in both, the root `<section>` has no `tabindex`;
  **one drop at a time**: with `assignDropped` held unresolved, a second
  drop makes no call and a drag shows "Still matching the last drop";
  "Matching dropped files…" appears after 150 ms (fake timers) and is
  announced after 1 s; resolving shows the result and re-enables drops; a
  rejected drop clears the previous result and shows the banner; a result
  keeps its `hasFailedEntries` when a later session changes
  `failedEntries`.
- Axe checks with the overlay visible, and a **view-level** axe check (the
  whole `TarpackView`) with the result visible and its "Full paths"
  disclosure expanded.

## States covered

Idle, drag hover, disabled (no manifest, entries withheld or every entry
failed, no files, building), enabled with manifest errors, result (each
bucket and each unmatched reason, with and without failed entries; ambiguous
in both directions; more than 8 names on a line; nothing matched; full
paths collapsed and expanded), drop pending (before and after 150 ms, drops
disabled), focus after Dismiss (rows follow, nothing follows), Browse with
and without a current assignment, dialog cancelled, and command error
(including a rejected drop).

## Out of scope

- The error report and notice (U2).
- The build bar (U5).
- Keyboard shortcuts (U6).
- Any matching logic in TS.
