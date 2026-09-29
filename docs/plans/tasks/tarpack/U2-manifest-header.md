# U2 — Manifest header, empty state, error report and notice, changed-on-disk banner

Status: awaiting approval
Project: tarpack   Depends on: U1 (landed), M6 (landed)

## Goal

Let the user open, reload, and edit a manifest, and see clearly when none is
loaded, when it changed on disk, and when it has errors: which files failed
and why, with a notice that tells them errors exist.

## Context

**The Tar Packager.** It builds a Linux archive (`.tar`, `.tar.gz`, `.tar.zst`,
or `.tar.xz`; the format is picked in the build bar, not in this task) from
Windows files. A
**manifest** (a TOML file the user edits in their own editor) lists the files,
their Linux target paths, and their permissions. The format is documented in
`docs/tarpack-manifest.md`; read it for the vocabulary, including the section
"When the manifest has errors". The user rebuilds the same package many times
a day, so they want confidence at a glance.

**A manifest with errors still opens (decided by the human).** The human's
requirement: *"Show the ones that passed but collect the ones that failed in
some sort of report. Also notify the user that there were errors."* So:

- entries with no error of their own **pass**. They are listed in the entry
  table (U3) and can be assigned files (U4);
- each `[[file]]` table with at least one error **fails**. It is left out of
  the table and listed in the **error report** (this task) with all of its
  errors;
- errors outside every `[[file]]` table are **manifest-level** errors, and
  are listed in the report on their own. Some of them (TOML syntax, a file
  that is not UTF-8, `version`, `[defaults]`, an unknown top-level key)
  mean no entry can be trusted. The backend then **withholds** every entry,
  and the table is empty;
- the archive cannot be created while any error exists. Building only the
  passed entries would silently drop files the manifest lists;
- whenever the manifest has errors, the user is **notified**, visibly and to
  screen readers.

**What exists.**

- U1 built `src/app/AppShell.tsx`, the tokens in `src/styles/tokens.css`, and
  the placeholder `src/tools/tarpack/TarpackView.tsx`, which you now build
  out.
- M6 provides `src/lib/tarpack.ts`. Read it for exact signatures. You use:
  - `session()`
  - `openManifest(path)`
  - `reloadManifest()`
  - `openInEditor()`
  - `recentManifests()`
  - `createManifestFromExample(path)`
  - `onManifestChanged(handler)`
- `src/lib/tauri.ts` provides `openFileDialog` and `saveFileDialog`.
- Types come from `src/lib/generated/`, which includes `TarpackSession`,
  `Diagnostic`, `EntryFailure`, and `TarpackError`. The parts this task reads:
  - `session.manifest`: `null`, or
    `{ path, name, outputName, hash, entries, entriesWithheld, errors, failedEntries, errorCount, warnings }`:
    - `entries`: the entries that **passed** validation only, in manifest
      order (U3 renders them);
    - `entriesWithheld: boolean`: `true` when a manifest-level error hides
      every entry; `entries` is then `[]`;
    - `errors: Diagnostic[]`: manifest-level errors **only** (outside every
      `[[file]]` table), sorted by line and column;
    - `failedEntries: EntryFailure[]`: one per `[[file]]` table with errors,
      in manifest order;
    - `errorCount: number`: `errors.length` plus every
      `failedEntries[i].errors.length`. **`errorCount > 0` is the single
      "this manifest has errors" flag.** Do not derive it from
      `errors.length`, which misses every entry error;
    - `warnings: Diagnostic[]`, sorted by line and column;
    - `name`: the manifest's `name`, or the manifest file's stem when `name`
      itself is in error. Render it as given.
  - `EntryFailure`:
    `{ index: number, id: string | null, source: string | null, line: number, errors: Diagnostic[] }`.
    `index` is the 1-based position of the `[[file]]` table; `id` is `null`
    when the table has no usable id; `source` is `null` when it has no string
    `source`; `line` is the line of the table's `[[file]]` header; `errors` is
    never empty.
  - `Diagnostic`: `{ severity, line, col, entryId: string | null, message }`.
    The `message` is final user-facing text and already carries its entry
    prefix, for example ``file `gateway`: missing field `dir` `` or
    `` [[file]] #3: id must not be empty ``. Render it verbatim: never strip
    the prefix, reword, or truncate.
  - `session.stateWarning`: `string | null`.
  - `TarpackError`: `{ kind: TarpackErrorKind, message: string, entryId?: string }`.
    `entryId` is omitted (not `null`) when absent. `message` is technical
    detail, not user copy.
  - `TarpackErrorKind`: a closed string-literal union of exactly 17 kinds,
    listed in the error table below.
- `openManifest`, `reloadManifest`, and the first `session()` call succeed for
  any readable file, whatever its errors. Only an unreadable file rejects,
  with `ManifestUnreadable`.
- The tokens you use are in `src/styles/tokens.css`: `--surface`,
  `--surface-sunken`, `--border`, `--text`, `--text-muted`, `--danger`,
  `--warn`, `--accent`, `--focus-ring`, `--font-mono`, `--radius`, and the
  `--space-*` scale. No new tokens.

**The single source of truth.** Every command returns a full `TarpackSession`,
and the view renders it. Keep it in one piece of React state in
`TarpackView`, replaced wholesale on each command result. Never derive a second
copy. View-only UI state (whether the error report is expanded, the text of
the announcer) is not a copy of the session and may live in `TarpackView`.

**The view's full structure**, top to bottom. Later tasks fill in the rest:

- the header (this task);
- the banners: command error, changed on disk, state warning (this task);
- the **error report** with its notice strip, and the warnings (this task);
- the drop result (U4);
- the entry table (U3);
- a sticky build bar at the bottom (U5).

Reserve those regions, and leave the drop result, table, and bar as empty
slots.

### Components

| Component | Path | Props | States |
| --- | --- | --- | --- |
| `TarpackView` | `src/tools/tarpack/TarpackView.tsx` | — | loading / no-manifest / loaded / errors (entries shown) / errors (entries withheld) / changed-on-disk |
| `ManifestHeader` | `src/tools/tarpack/ManifestHeader.tsx` | `session, onOpen, onOpenRecent, onReload, onEdit` | loaded |
| `ManifestErrors` | `src/tools/tarpack/ManifestErrors.tsx` | `errors, failedEntries, entriesWithheld, errorCount, warnings, expanded, onExpandedChange(expanded), onEdit, reportRef` | hidden / warnings only / errors expanded / errors collapsed / errors with entries withheld |
| `errorMessages` | `src/tools/tarpack/errorMessages.ts` (module) | exports `errorMessage(error: TarpackError, entries): string` | one entry per kind |
| `Banner` | `src/app/Banner.tsx` (shared) | `tone: "info"\|"warn"\|"error", message, action?: { label, onAction }` | — |

`reportRef` is a ref to the report body, so `TarpackView` can move focus to
it. `TarpackView` also defines one handler, `showErrors()`, described below;
the build bar (U5) and the shortcuts (U6) call it later.

### Behaviour

- **Header:**
  - shows the manifest `name` as the view heading;
  - shows the manifest path below it in `--font-mono`, `--text-muted`, and
    middle-truncated with the full path in `title`;
  - has these actions: **Open…**, **Recent ▾** (a menu from
    `recentManifests()`; hidden when empty), **Reload**, and **Edit in
    editor**. All stay enabled when the manifest has errors.
- **No-manifest state** (`session.manifest === null`): one sentence, "Open a
  manifest to list the files this package needs.", and two buttons:
  - **Open manifest…**: an open dialog filtered to `*.toml`;
  - **Create from example…**: a save dialog, then `createManifestFromExample`.

#### The error report (`ManifestErrors`), when `manifest.errorCount > 0`

One region directly below the banners and above the table. It has two parts:
a **notice strip** that is always visible while errors exist, and a
**report body** the user can collapse.

- **Notice strip** (this is the visible notification):
  - `--surface` background, 1 px `--border` outline, `--radius`, and a 3 px
    `--danger` bar on its inline-start edge. An error icon in `--danger`,
    `aria-hidden`. The meaning is carried by the text, never by the color or
    icon alone.
  - Heading (one level below the view heading), in `--text`:
    "1 error in this manifest" / "{errorCount} errors in this manifest".
  - One line of consequence, in `--text`:
    - failed entries and no withholding: "1 file is left out of the list
      until it's fixed." / "{failedEntries.length} files are left out of the
      list until they're fixed.";
    - `entriesWithheld`: "No files can be listed until the errors under
      Whole manifest are fixed.";
    - only manifest-level errors that do not withhold (for example a bad
      `name`): no sentence here.
    
    Then, always: "The archive can't be created until every error is fixed."
  - Actions: a toggle button, **Hide errors** / **Show errors**, with
    `aria-expanded` and `aria-controls` pointing at the report body; and
    **Edit in editor** (`onEdit`, the same handler as the header's).
- **Report body** (`id="manifest-error-report"`):
  - A `role="region"` with `aria-labelledby` pointing at the strip's
    heading, and `tabIndex={0}`, because it may scroll. Max height `40vh`,
    then it scrolls inside itself; the page does not grow without bound for
    a manifest with many failures.
  - **Whole manifest** group, first, only when `errors` is non-empty: a
    sub-heading "Whole manifest" and a list of its errors.
  - **One group per `failedEntries` element**, in the order given, as items
    of a list labelled "Files with errors". Each group has:
    - a sub-heading naming the entry:
      - `id` present: the `id` in `--font-mono`;
      - `id` null: "Entry #{index}";
    - after it, in `--text-muted`: "source {source}" (source in
      `--font-mono`) when `source` is non-null, then "line {line}" (the
      `[[file]]` header line). Separate them with " · ", and mark the
      separators `aria-hidden`. With `id` null and `source` null, only
      "Entry #{index} · line {line}";
    - a list of **every** error in `errors`, none hidden behind a "more"
      link.
  - **Each error item:** `{line}:{col}` in `--font-mono` `--text-muted`,
    then the `message` verbatim in `--text`. The visible `12:5` is
    `aria-hidden`, and visually hidden text reads "Line 12, column 5:" before
    the message, so a screen reader does not say "12 colon 5". Messages and
    long ids or source names wrap (`overflow-wrap: anywhere`), never truncate.
  - Items are not tab stops. The body is one tab stop; screen reader users
    move through it by heading and list.
- **Expanded by default.** Every session that results from open, reload, or
  the first `session()` restore and has `errorCount > 0` sets the report
  body expanded, even if the user collapsed it before. Results of other
  commands (assign, clear, drop, format, output) leave the expanded state as
  the user set it.
- **`showErrors()`** in `TarpackView`: expands the report body, then moves
  focus to it (`reportRef`) after it renders, and scrolls it into view
  (instantly when `prefers-reduced-motion` is set). It does nothing when
  `errorCount` is 0. The notice strip's **Show errors** uses it; U5's build
  bar and U6's shortcut will too.
- **No focus stealing.** Opening or reloading a manifest with errors never
  moves focus on its own; focus stays on the control the user used.

#### The announcement (screen-reader notification)

- An always-mounted, visually hidden `role="status"` (`aria-live="polite"`)
  announcer in `TarpackView`. It is in the DOM from the first render,
  including the loading state, so the first message is announced. Do not
  make the strip itself the live region: it mounts and unmounts.
- After every open, reload, and the first `session()` restore:
  - `errorCount > 0`: "{name} has 1 error." / "{name} has {errorCount}
    errors.", followed by "1 file left out." / "{n} files left out." when
    there are failed entries, or "No files can be listed." when entries are
    withheld. Announce it after **every** such result, even when the text is
    the same as the last one, because the user asked for the reload: clear
    the announcer, then set the text on the next frame.
  - A reload that takes `errorCount` from above 0 to 0: "{name} reloaded. No
    errors." The notice strip and report disappear at the same time.
  - Otherwise, announce nothing.
- Do not announce on assign, clear, drop, or format changes; the error count
  does not change there.

#### Warnings

Whenever `warnings` is non-empty, **including when there are errors**, show
a collapsed `<details>` headed "1 warning" / "N warnings" below the error
report, using the same item format (`line:col`, then the message). Warnings
never block the build. The backend emits three kinds; show each `message`
exactly as given, never rewording or truncating it:

- two entries share a `source` name (a drop cannot tell them apart);
- a stored path is **100 bytes or longer**: the message names the entry, says
  "100 bytes or longer", and gives the byte count of the stored path
  including its leading `/` (for example 112 bytes). The item must wrap, not
  truncate, so the byte count stays visible;
- the manifest lists no files.

#### Other states

- **Changed on disk.** An `onManifestChanged` event shows a warn `Banner`:
  "The manifest changed on disk.", with the action **Reload**. It clears after
  a reload. This is the usual way the user gets from fixing an error in their
  editor to a clean manifest.
- **Command errors** (`TarpackError`) show as an error `Banner` whose message
  is `errorMessage(error, entries)`, with the backend `message` under it in a
  collapsed "Details" disclosure. In this task's flows:
  - **Open…** or a Recent item, and **Reload**, can fail with
    `ManifestUnreadable` (file missing or not permitted). Show its message;
    for a Recent item, keep the current session unchanged. A manifest with
    errors is **not** a command error: it opens and shows the report.
  - **Create from example…** fails with `PathExists` when the chosen path
    already exists (the native Save dialog may have asked "Replace?", but the
    backend never overwrites). Show its message and leave the no-manifest
    state in place so the user can try another name.
  - **Edit in editor** can fail with `OpenerFailed`.
  - Any command can fail with `Io`.

### Error copy (you create `errorMessages.ts`)

Create `src/tools/tarpack/errorMessages.ts` with all 17 kinds now; later tasks
(assignment, build) only import it. Type the table as
`Record<TarpackErrorKind, (source: string | null) => string>`, importing
`TarpackErrorKind` from `src/lib/generated/`, so the compiler rejects a missing
or extra kind; do not add a default or "any other" branch. `errorMessage`
looks up `entryId` in `session.manifest.entries` and passes that entry's
`source`, or `null` when `entryId` is omitted or unknown; a `{source}`
placeholder then reads "A file". (The ids of failed entries are not in
`entries`, so they read "A file" too.)

| Kind | Raised by | Message |
| --- | --- | --- |
| `NoManifest` | any command needing a manifest | "Open a manifest first." |
| `ManifestUnreadable` | open, reload | "The manifest could not be read. Check that the file still exists and that you can open it." |
| `ManifestInvalid` | build | "Fix the manifest errors first." |
| `ManifestChangedOnDisk` | build | "The manifest changed on disk. Reload it, then build again." |
| `UnknownEntry` | assign, clear | "That file is no longer in the manifest. Reload and try again." |
| `NotAFile` | assign | "{source}: the chosen path is not a file." |
| `NoOutput` | build | "Choose where to save the archive." |
| `EntriesNotReady` | build | "{source} still needs a location." |
| `OutputExists` | build | "A file with this name already exists." |
| `PathExists` | create from example | "A file already exists there. Choose a new name; the example never replaces a file." |
| `SourceMissing` | build | "{source} is no longer at its assigned location." |
| `SourceUnreadable` | build | "{source} could not be read." |
| `SourceChanged` | build | "{source} changed while the archive was being written. Nothing was saved. Try again." |
| `VerifyFailed` | build | "The archive failed its check after writing, so it was not saved. Any existing file was left unchanged. Try again." |
| `BuildInProgress` | build | "A build is already running." |
| `OpenerFailed` | edit in editor, show in folder | "Windows could not open it." |
| `Io` | any | "A file could not be read or written." |

- **Loading:** while the first `session()` call is pending, show a quiet
  skeleton. It must not flash for fast loads; delay it by about 150 ms.
- `session.stateWarning`, when set, shows an info `Banner` (for example that
  saved locations were reset).

**Rules that bind this task.**

- Tokens only, no hex.
- Everything is keyboard reachable and labelled.
- Status is never conveyed by color alone.
- Plain, short copy. Say "error" for errors and "warning" for warnings;
  do not use "problem".
- Call only `lib/` functions. Never touch the filesystem or invoke Tauri
  directly.
- Never rewrite, split, or strip a backend `message`.

## Files

- `src/tools/tarpack/TarpackView.tsx`, `ManifestHeader.tsx`,
  `ManifestErrors.tsx`, `errorMessages.ts`
- `src/app/Banner.tsx`
- Tests next to each

## Skill

`impeccable:layout`, then `impeccable:clarify` for all copy. Finish with
`impeccable:audit` on the error report, notice strip, and announcer. If the
`impeccable` skills are not available, follow their method by hand and say so
in your report.

## Acceptance criteria

- Every state above renders from a `TarpackSession` fixture.
- **Open…** calls `openFileDialog` with a `.toml` filter, then `openManifest`.
  A cancelled dialog does nothing.
- The banner appears on a `manifest-changed` event, and Reload clears it.
- **Error state is keyed on `errorCount`.** A fixture with `errors: []` and
  one failed entry shows the notice strip and report; a fixture with
  `errorCount: 0` shows neither.
- The notice strip shows the exact count ("1 error" / "N errors") and the
  consequence line for each case: failed entries, entries withheld, and
  manifest-level errors only.
- The report lists the Whole manifest group (when `errors` is non-empty),
  then one group per failed entry in the given order. A group is named by
  `id`, or "Entry #{index}" when `id` is `null`, with "source {source}" when
  present and "line {line}"; every one of its errors is listed with
  `line:col` and the verbatim message.
- Each error item's accessible text starts "Line {line}, column {col}:";
  the visible `line:col` is `aria-hidden`.
- The toggle has `aria-expanded` and `aria-controls`, works with Enter and
  Space, and the body is expanded after each open, reload, or restore with
  errors, even if it was collapsed before.
- **Show errors** expands the body and moves focus to it. Opening or
  reloading never moves focus by itself.
- The announcer is in the DOM before the first session arrives, announces the
  error count after open, reload, and restore, repeats the announcement on a
  reload with the same count, announces "No errors." when a reload clears
  them, and says nothing on assign, clear, or drop.
- Warnings render when errors are also present, collapsed.
- A report with 200 failed entries stays within `40vh`, scrolls inside its
  region, and the region is keyboard scrollable.
- `errorMessages.ts` is a `Record` over `TarpackErrorKind` with no fallback
  branch; removing any key fails `npm run typecheck`.
- `ManifestUnreadable` on Open or Reload, and `PathExists` on Create from
  example, show their messages in an error banner with a Details disclosure
  that is keyboard operable; the prior state stays usable.
- Warning and error messages render verbatim; a long-name warning's byte
  count is visible at 800 px and at 200% text size.
- The header, notice strip, and report wrap cleanly at 800 px wide and at
  200% text size, with no horizontal page scroll.

## Tests proving completion

`npm run test`, with `src/lib/tarpack` and `src/lib/tauri` mocked:

- `TarpackView.states.test.tsx`: loading, no-manifest, loaded, errors with
  entries shown, errors with entries withheld, and changed-on-disk;
  `ManifestUnreadable` from Open and Reload; `PathExists` from Create from
  example.
- `TarpackView.errors.test.tsx`:
  - the announcer text after open, reload, and restore with errors; the
    repeat on a same-count reload; "No errors." on a reload to 0; silence on
    assign;
  - the report re-expanding after a reload when the user had collapsed it;
  - **Show errors** moving focus to the report body; no focus change on open.
- `ManifestHeader.test.tsx`: each action calls the right `lib` function, and
  Recent is hidden when empty.
- `errorMessages.test.ts`: a non-empty message for each of the 17 kinds
  (listed explicitly in the test), `{source}` filled from `entryId`, and
  "A file" when `entryId` is omitted or unknown. `npm run typecheck` proves
  exhaustiveness.
- `ManifestErrors.test.tsx`: the count heading (singular and plural); each
  consequence line; the Whole manifest group; a failed entry with an `id`,
  one with `id: null` and a `source`, and one with both `null`; every error
  of a multi-error entry listed; the "Line N, column M:" accessible text;
  the toggle; warnings alongside errors; and a long-name warning whose
  message (with its byte count) renders verbatim.
- Axe checks on each state, including the report expanded and collapsed.

## States covered

Loading, no-manifest, loaded, errors with entries shown (expanded and
collapsed), errors with entries withheld, manifest-level errors only,
warnings only, errors plus warnings, changed-on-disk, and command error
(`ManifestUnreadable`, `PathExists`, `OpenerFailed`, `Io`).

## Out of scope

- The entry table, including its empty and withheld messages (U3).
- Drops (U4).
- The build bar and its link to the report (U5).
- Keyboard shortcuts (U6).
