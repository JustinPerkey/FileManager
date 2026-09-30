# U5 — Output, format, build, overwrite confirmation, progress, and result

Status: awaiting approval (amended 2026-09-30: scroll model and `MiddlePath`
from U3; sticky offsets from `--view-pad-block`; `MiddlePath` slot width;
amended 2026-09-30 after review: failed-build announcement, short format
labels, progress text rules, accepted decisions, overwrite-dialog folder copy,
rendered-check method, and the required fixes listed under "Fixes required
after review"; amended 2026-09-30 after re-review: short-window fallback
for the bar (human's decision), measured `scroll-padding-bottom`,
`/impeccable audit` waived to U6, test fixes F1, F3, F6)
Project: tarpack   Depends on: U2 (landed; `FailureList`, `DiagnosticList`,
`showErrors()`, `announce()`), U3 (landed), M6 (landed; this task reads its
partial-results and build-report fields). May run alongside U4.

## Goal

Let the user choose where the archive goes and in which format, create it
safely with visible two-phase progress, and get a result they can verify and
extract on the target. When the manifest has errors, build anyway from the
entries that passed, say beforehand what will be left out, and make the
result the final report of everything that was.

## Context

**The user and the target.** A developer rebuilding the same package many
times a day for a low-power armv7 Linux machine running GNU tar. The archive
stores **absolute** paths (`/opt/...`), so on the target it must be extracted
with `tar -P`; without `-P`, tar strips the leading `/` and extracts into the
current directory. The build result therefore shows the exact extraction
command and lets the user copy it.

**Formats (decided).** Four formats, picked before building. The picker
labels are the short names below (accepted at review); the option text adds
the extension, so the uncompressed format reads "tar (.tar)":

| `ArchiveFormat` | Extension (from the session) | Picker label (your copy) |
| --- | --- | --- |
| `"tar"` | `.tar` | tar |
| `"tarGz"` | `.tar.gz` | gzip |
| `"tarZst"` | `.tar.zst` | zstd |
| `"tarXz"` | `.tar.xz` | xz |

**The backend owns all format and extension logic.** You render
`session.formats` in the order given, show `session.format` as selected, and
render whatever session `setFormat` or `setOutput` returns. Never append,
strip, or compare extensions in TS, and never build a file name or command
yourself. Even the Save-dialog filter suffix comes from the session
(`filterExtension`); derive nothing from `extension`.

**Safety rules the UI must honour.**

- The archive is written atomically by Rust and verified before it replaces
  anything. If verification fails (`VerifyFailed`), nothing is saved and an
  existing output file is left byte-for-byte unchanged (guaranteed and tested
  by the backend). An existing file is replaced only when the UI passes
  `overwrite: true`, and it may do so only after the user confirms in a
  dialog.
- If the manifest changed on disk since it was loaded, Rust refuses to build
  (`ManifestChangedOnDisk`). The UI must then show the same "The manifest
  changed on disk." warn banner with **Reload** that the header shows for the
  file-watcher event.

**A manifest with errors does not block the build (decided by the human).**
The human's words: *"A single error does not block builds but is included as
an error in the final report."* and *"Show the ones that passed but collect
the ones that failed in some sort of report. Also notify the user that there
were errors."* So:

- A manifest with errors still opens. Entries that passed validation are in
  the table and can be assigned; entries with errors (**failed** entries) are
  left out of the table and listed in U2's error report, under a notice ("3
  errors in this manifest").
- **Create archive is enabled whenever `session.canBuild` is true, whatever
  `errorCount` is.** The archive holds the passed entries only.
- Nothing is dropped silently: **before** the build, the bar says how many
  files will be left out and that the errors will be listed after the build;
  **after** it, the result is the final report, listing every left-out entry
  with all its errors, every manifest-level error, and the warnings.
- There is **no extra confirmation** for building with errors. A dialog on
  every test build of a manifest being fixed would be dismissed by habit. The
  overwrite confirmation stays the only build dialog.
- The only error-related block is "nothing to build": no entry passed
  (every entry withheld by a manifest-level error, every entry failed, or the
  manifest lists none). The bar then says why.
- The report exists only in the UI. Nothing about the errors is written into
  the archive, and no report or log file is saved beside it. The copy must
  not suggest otherwise.
- The user may choose the output and the format while errors exist; both are
  remembered for the manifest.

**What exists.**

- `TarpackView.tsx` holds the `TarpackSession` in one piece of React state,
  replaced wholesale by every command result, and has a reserved slot for the
  sticky bottom bar. `src/app/Banner.tsx` exists:
  `tone: "info"|"warn"|"error", message, action?: { label, onAction }, onDismiss?`,
  with a Details disclosure passed as children (styled by
  `.banner__details` in `controls.css`). Only the error tone is a live
  region (`role="alert"`).
  `TarpackView` (U2) defines `showErrors()`: it expands the error report,
  moves focus to it, and scrolls it into view. It does nothing when
  `errorCount` is 0. The report body has `id="manifest-error-report"`.
  `TarpackView` also defines `announce(text)`, which sets the text of its
  always-mounted, visually hidden polite live region. Call both inside
  `TarpackView` (pass `showErrors` down as `onShowErrors`). `TarpackView`'s
  optional `actionsRef` prop is a test-only seam; product code never uses
  it.
- U2 built two tool-local list components; reuse them unchanged in the final
  report so errors look the same before and after a build:
  - `src/tools/tarpack/FailureList.tsx`:
    `failures: EntryFailure[], headingLevel: 3 | 4`. One list item per
    failure: a heading naming the entry (`id` in mono, or "Entry #{index}"
    when `id` is `null`), a muted line "source {source} · [[file]] on line
    {line}" (the source part only when `source` is non-null), and a
    `DiagnosticList` of every error;
  - `src/tools/tarpack/DiagnosticList.tsx`:
    `diagnostics: Diagnostic[], label: string`. A labelled `<ul>`; each item
    is `line:col` (mono, muted, `aria-hidden`, with visually hidden "Line N,
    column M:") and the message verbatim, wrapping.

  Both render nothing for an empty list.
- `src/lib/tarpack.ts` (M6; read it for exact signatures) provides:
  - `setOutput(path)`, which returns a session. The backend keeps the path if
    it already ends with the current format's extension, **switches
    `format`** if it ends with a different known archive suffix (`.tar`,
    `.tar.gz`, `.tgz`, `.tar.zst`, `.tar.xz`), and otherwise appends the
    current extension;
  - `setFormat(format)`, which returns a session. It rewrites the extension of
    `outputPath` when one is set, and remembers the format for this manifest;
  - with no manifest loaded, both `setOutput` and `setFormat` reject with
    `NoManifest` and change nothing, because both are remembered per manifest.
    The UI keeps Choose… and the Format picker disabled in that state, so
    this never happens in normal use;
  - `build(overwrite)`, which returns a `BuildSummary` or throws a
    `TarpackError`;
  - `onBuildProgress(handler)`, which yields
    `{ phase: "writing" | "verifying", entryId: string | null, bytesDone: number, bytesTotal: number }`:
    - bytes are uncompressed tar-stream bytes, and both phases share the same
      `bytesTotal`;
    - `writing` comes first; `verifying` follows, and each phase starts
      over. A phase's first event is not necessarily at 0: the backend
      reports after the first 512-byte record, so it may already be past 0,
      and it is always below `bytesTotal`;
    - `bytesDone` never decreases within a phase;
    - `entryId` is the file entry being processed, in both phases, and `null`
      for directory records, long-name records, and the end-of-archive blocks;
    - each phase ends with exactly one event where `bytesDone == bytesTotal`
      and `entryId` is `null`;
    - no event follows the final verifying event, and there is no separate
      finalizing phase. The SHA-256 is computed during verifying, so only an
      fsync and a rename remain before `build` settles;
    - on failure, events simply stop, and `build` rejects;
  - `revealOutput()`.
- `src/lib/tauri.ts` provides `saveFileDialog({ defaultPath, filters? })`,
  where `filters` is `{ name: string, extensions: string[] }[]`. Extensions
  are given without the leading dot, and Windows matches only the last suffix
  (for example `zst`), so the backend's `setOutput` normalisation is what
  guarantees the full extension.
- Clipboard: there is no `lib/` wrapper and no plugin. Call
  `navigator.clipboard.writeText` directly from the Copy click handler. It
  can reject (focus lost, permission); see Copy under Success.
- `src/tools/tarpack/pathParts.ts` (U3, U4) holds the display-path helpers:
  `pathParts(path)`, and `assignedFolder(path): string | undefined`, which
  returns `undefined` for a lossy path (one containing U+FFFD) because its
  result is passed back to `lib`. This task adds two more there (see Files):
  `fileName(path)` (moved from `reportText.ts`) and `displayFolder(path)`.
- From the session:
  - `session.manifest`: `null` or the loaded manifest, with:
    - `entries[]`: passed entries only, carrying `{ id, source, ... }` for
      mapping an `entryId` to a file name;
    - `entriesWithheld: boolean`: a manifest-level error hides every entry,
      and `entries` is `[]`;
    - `errors: Diagnostic[]`: manifest-level errors only;
    - `failedEntries: EntryFailure[]`: one per failed `[[file]]` table, where
      `EntryFailure` is
      `{ index: number, id: string | null, source: string | null, line: number, errors: Diagnostic[] }`
      and `Diagnostic` is `{ severity, line, col, entryId, message }`;
    - `errorCount: number`: `errors.length` plus every failed entry's errors;
      above 0 whenever the manifest has errors. It never disables the build;
  - `session.outputPath`: `string | null`, always ending with the current
    format's extension;
  - `session.format`: `ArchiveFormat`;
  - `session.formats`:
    `{ format: ArchiveFormat, extension: string, filterExtension: string }[]`,
    in the fixed order tar, tarGz, tarZst, tarXz, for example
    `{ format: "tarZst", extension: ".tar.zst", filterExtension: "zst" }`.
    `filterExtension` is `"tar"`, `"gz"`, `"zst"`, or `"xz"`;
  - `session.suggestedOutputName`: `string | null`, the file name for the
    Save dialog with the current extension;
  - **with no manifest loaded**: `format` is `"tar"`, `formats` still lists all
    four, and `outputPath` and `suggestedOutputName` are `null`;
  - `session.canBuild`: true exactly when a manifest is loaded, at least one
    entry passed, every passed entry is `ready`, and `outputPath` is set,
    **whatever `errorCount` is**;
  - `session.buildBlockedReason`: one of `"noManifest"`, `"noEntries"`,
    `"entriesNotReady"`, `"noOutput"`, or `null`, the first failing
    condition in that order. `"noEntries"` means `manifest.entries` is empty
    (withheld, every entry failed, or the manifest lists none). There is no
    reason for "the manifest has errors". Render the reason the session
    gives; never compute your own;
  - `session.readyCount` and `session.totalCount` (passed entries only).
- `BuildSummary`:
  `{ path, format, entries, files, dirs, bytes, uncompressedBytes, sha256Hex, extractCommand, normalizedEntries: { id, crlfReplaced }[], builtIds, leftOut, manifestErrors, warnings, errorCount }`.
  - `bytes` is the size of the output file on disk; `uncompressedBytes` is the
    size of the tar stream before compression (equal to `bytes` for `"tar"`).
  - `extractCommand` is the exact command for the target, for example
    `tar --zstd --no-overwrite-dir -xpPf gateway.tar.zst`. It uses the file
    name only, so it runs in the directory that holds the archive.
  - `normalizedEntries` lists every entry with line-ending conversion on, with
    the number of CRLF pairs replaced (possibly 0).
  - The final report, straight from the backend:
    - `builtIds: string[]`: the ids of the file entries in the archive, in
      archive order;
    - `leftOut: EntryFailure[]`: every failed entry, none of which is in the
      archive, each with all its errors;
    - `manifestErrors: Diagnostic[]`: the manifest-level errors (in a build
      that ran, only ones that hide no entry, such as a bad `name`);
    - `warnings: Diagnostic[]`;
    - `errorCount: number`: `manifestErrors.length` plus every `leftOut`
      entry's errors. 0 means the archive holds every file the manifest
      lists.

    Render the result from the summary only, never from the session: the user
    may reload the manifest while the result is still on screen.
- `TarpackError`: `{ kind: TarpackErrorKind, message: string, entryId?: string }`.
  `entryId` is omitted (not `null`) when absent. `message` is the backend's
  technical detail, not user copy. `TarpackErrorKind` is a closed
  string-literal union of exactly 17 kinds (table under Behaviour → Errors).
- `src/tools/tarpack/errorMessages.ts` (U2) exports
  `errorMessage(error, entries): string`, backed by an exhaustive
  `Record<TarpackErrorKind, (source: string | null) => string>`. Use it for
  every error you display; do not write copy of your own or add a fallback.
  The same module exports `toTarpackError(e: unknown): TarpackError`; pass
  every caught rejection through it (unknown shapes become `Io`), and write
  no guard or kind list of your own.
  If a message below differs from the file, the file was written from this
  same table, so report the difference rather than patching around it.
- The tokens you use are in `src/styles/tokens.css`: `--surface`,
  `--surface-sunken`, `--border`, `--accent`, `--accent-text`, `--ok`,
  `--danger`, `--text`, `--text-muted`, `--font-mono`, `--radius`,
  `--font-size-sm/md/lg`, `--shadow-overlay`, and the `--space-*` scale.
- U2 built the shared vocabulary; use it and add no parallel version:
  - `src/app/Button.tsx`: `variant: "primary" | "secondary" | "quiet"`,
    `icon?`, and `aria-busy` support;
  - `src/app/icons.tsx`: `Icon` with `name: IconName`, including `copy`,
    `folder`, `check-circle`, `x-circle`, and `x`;
  - the `.num` (tabular numerals) and `.mono` (`--font-mono`) utility
    classes in `src/styles/base.css`.
- **Stylesheets.** Tool styles go in `src/styles/tarpack.css`, and every
  selector there is scoped under the view root class `.tarpack`. Shared
  component styles (`ConfirmDialog`) go in `src/styles/controls.css`.
  `--shadow-overlay` is for floating surfaces only; in this task, only the
  dialog.
- **Design context.** The root `DESIGN.md` records the visual system ("The
  Packing List"). This is an Operate surface: the bar is the end of the
  daily loop, so it stays quiet until the result. **Create archive** is the
  only accent-filled button in the whole view, and **Replace** in the dialog
  is the only other primary. Results are inline panels, not toasts, and not
  metric tiles.

### Components

| Component | Path | Props | States |
| --- | --- | --- | --- |
| `BuildBar` | `src/tools/tarpack/BuildBar.tsx` | `session, building, progress, progressUnavailable, onChooseOutput, onFormatChange, onBuild, onShowErrors, onError(e: unknown)`, plus `formatNotice?` | disabled-with-reason / no entries (three cases) / ready / ready with files left out / ready with manifest errors only / building |
| `FormatPicker` | `src/tools/tarpack/FormatPicker.tsx` | `formats, value, disabled, onChange(format)` | enabled / disabled |
| `BuildProgress` | `src/tools/tarpack/BuildProgress.tsx` | `progress, entries` | writing / verifying / finishing |
| `BuildResult` | `src/tools/tarpack/BuildResult.tsx` | `result: { ok: BuildSummary } \| { err: TarpackError }, entries, onReveal, onDismiss` | success / success with files left out / success with manifest errors only / error |
| `BuildReport` | `src/tools/tarpack/BuildReport.tsx` | `leftOut, manifestErrors, warnings` | errors / warnings only / nothing (renders nothing) |
| `reportText` | `src/tools/tarpack/reportText.ts` (module) | `reportText(summary): string` | — |
| `ConfirmDialog` | `src/app/ConfirmDialog.tsx` (shared) | `open, title, body, confirmLabel, onConfirm, onCancel` | open (title as `aria-labelledby`, body as `aria-describedby`, both `useId()` ids) |

### Behaviour

- **`BuildBar`**, sticky at the bottom of the view. Left to right, wrapping to
  two lines when narrow or at 200% text:
  - "Output:", the path rendered by U3's `MiddlePath`
    (`src/tools/tarpack/MiddlePath.tsx`, prop `path: string`: one mono line
    that CSS truncates in the middle, keeping the drive and the file name,
    with the full path in `title` and as its accessible text; its styles are
    already in `tarpack.css`), or "not chosen", and **Choose…**. Give the
    path a `min-width: 0` flex slot so it truncates instead of pushing
    Choose… to a new line; do not write a second truncation helper;
    do not restyle `.middle-path__*` (its line snaps to whole characters with
    `width: calc(round(down, 100% - 1px, 1ch) + 0.5px)` and its tail is
    `flex: 0 0 auto; max-width: calc(100% - 4ch)`, so "…" sits flush
    against the file name). Because the line's width is a percentage of its
    container, the slot must have a definite width: make it a flex item with
    `flex: 1 1 auto; min-width: 0`, not a shrink-to-fit inline box;
  - the **Format** picker;
  - the status text: the disabled reason, and the left-out note (below),
    when there is one;
  - **Show errors**, when `errorCount > 0`;
  - the primary **Create archive** button. The wording is format-neutral;
    never "Create tar".

  The bar has two groups that wrap as units: the settings (Output, Choose…,
  Format) and the action (status text, Show errors, Create archive). At
  800 px or 200% text the action group takes the second line, right-aligned,
  and the status text wraps inside it rather than pushing the button off.

  The bar is `--surface`, with a 1 px `--border` top rule and padding
  `--space-3 --space-5`. It has no shadow; stickiness alone separates it.
- **Scroll model (decided in U3; build on it).** The view root,
  `<section className="tool-view tarpack">`, is the one vertical scroll
  container. The table has no scroll box of its own and no `max-height`; in
  its wide (table) layout its header cells are sticky in the same scroll
  container (offset below), and `.tarpack` already sets `scrollbar-gutter: stable` and
  `scroll-padding-top: 3.5rem`. Chromium sticks elements to the scroll
  container's **content box**, so the view's padding offsets every sticky
  edge. U3 made that padding one variable in `tarpack.css`:
  `.tarpack { --view-pad-block: var(--space-4); padding-block:
  var(--view-pad-block); }`, and the table header sticks at
  `top: calc(var(--view-pad-block) * -1)` (on wide windows only; at the
  default window the table is stacked and its header is hidden). So:
  - the bar is the last child of `.tarpack`, `position: sticky;
    bottom: calc(var(--view-pad-block) * -1)`, with
    `margin-bottom: calc(var(--view-pad-block) * -1)` so it also sits flush
    at the end of the scroll, and `margin-inline: calc(var(--space-5) * -1)`
    to span the view's full width. A plain `bottom: 0` would float 16 px
    above the bottom edge with rows showing beneath it. Never hard-code the
    16 px; use the variable;
  - set `.tarpack { scroll-padding-bottom: … }` in rem, at least the bar's
    two-line height at 200% text, so a focused table row or control never
    scrolls in under the bar. **How to measure** (in the rendered check
    below): at a 800×560 viewport with the root font size set to 32 px,
    with a manifest loaded and errors present so the bar shows its note and
    Show errors and takes two lines, read
    `document.querySelector('.tarpack .build-bar').getBoundingClientRect().height`
    (use the bar's actual root class), divide by 32, round **up** to the
    next 0.25 rem, and add 0.5 rem of breathing room. Then verify: from the
    top of the view, Tab through every table control and check each focused
    element's `getBoundingClientRect().bottom` is at or above the bar's
    `top`. Record the measured height and the chosen value in your report.
    **Measured and decided (2026-09-30).** A root font of 32 px does not
    trigger media queries, so the 200%-text check is done at a **400×280**
    viewport at the default root size (the same layout in CSS pixels as
    800×560 at 200% zoom). There the sidebar leaves the view about 480 px
    of device width, the bar wraps to its full height of **448 px =
    14 rem**, so `.tarpack { scroll-padding-bottom: 14.5rem; }`;
  - **Short-window fallback (the human's decision, 2026-09-30).** At
    `@media (max-height: 30rem)` the bar is `position: static` (it sits at
    the end of the scroll, still the last child of `.tarpack`) and
    `.tarpack { scroll-padding-bottom: 0; }`, because a sticky bar of
    14 rem would cover most of a view that short. Accepted limitation: the
    threshold is height-only while the bar's height depends on width, so,
    for example, a window of about 480×520 CSS pixels at 200% zoom stays
    sticky with the bar covering about 40% of the view. Keep this rule as
    is; do not add a width condition;
  - `BuildResult` sits in the page flow above the bar, with no `overflow`,
    no `max-height`, and no sticky position; only `BuildReport` inside it
    scrolls, capped at `40vh`, like U2's error report;
  - add no other scroll container. At the default window, at 800×560, and at
    200% text the table is in its stacked layout with no sticky header; at
    800×560 with 200% text the bar wraps to its full height (14 rem, more
    than two lines) and, under the short-window fallback, is static;
    the view scrolls past the error report, the table, and the result to the
    bar, and every control stays reachable by Tab.
  **Choose…** is a secondary `Button` with the `folder` icon. The disabled
  reason is `--font-size-sm` in `--text-muted`.
- **`FormatPicker`** is a native `<select>` with a visible `<label>` "Format"
  (not a placeholder), so it is keyboard operable with the platform's
  behaviour and announced as "Format, combo box, zstd (.tar.zst)".
  - One `<option>` per entry of `formats`, in the given order, reading
    "{label} ({extension})", for example "zstd (.tar.zst)", with the label from
    the table above and the extension from the session.
  - Its value is `session.format`. On change, call `setFormat(format)` and
    render the returned session; the output path updates with it.
  - Disabled when no manifest is loaded (it still shows "tar (.tar)" from the
    session) and while building.
- **Choose…** opens `saveFileDialog` with:
  - `defaultPath`: `session.outputPath`, else `session.suggestedOutputName`;
  - `filters`: one filter for the current format, taken from the matching
    entry of `session.formats`:
    `{ name: "{label} archive ({extension})", extensions: [filterExtension] }`,
    for example `{ name: "zstd archive (.tar.zst)", extensions: ["zst"] }`.

  A chosen path goes to `setOutput(path)`; a cancelled dialog does nothing.
  Choose… is disabled when no manifest is loaded and while building.
  **`BuildBar` calls `saveFileDialog` itself (accepted at review)**, but it
  never swallows or displays an error: a rejection from `saveFileDialog` goes
  to its `onError(e)` prop, which `TarpackView` wires to the same handler
  `onOpen`, `onCreate`, and `onBrowse` use (`toTarpackError`, then the
  view's error `Banner` with Details). `setOutput` and `setFormat` errors go
  the same way.
- **Manifest errors disable nothing.** Choose… and the Format picker are
  disabled only with no manifest and while building; Create archive follows
  `canBuild`.
- **Format switched by the file name.** If the session returned by
  `setOutput` has a different `format` from the one before the call, announce
  it in a polite live region in the bar: "Format changed to xz to match the
  file name." The picker already shows the new value because it renders the
  session.
- **Disabled reason**, shown as visible text next to the button (not only a
  tooltip) and linked with `aria-describedby`:
  - `noManifest`: "Open a manifest first"
  - `noEntries`, one sentence chosen from `session.manifest` in this order:
    - `entriesWithheld`: "No files can be built until the manifest errors
      are fixed";
    - `failedEntries` non-empty: "Every file in the manifest has errors";
    - otherwise: "This manifest lists no files";
  - `entriesNotReady`: "1 file still needs a location" / "{totalCount −
    readyCount} files still need a location"
  - `noOutput`: "Choose where to save the archive"
- **The left-out note**, shown when `errorCount > 0` and `entries` is
  non-empty (so the reason is `null`, `entriesNotReady`, or `noOutput`). It is
  visible text, not a tooltip, with the `alert-triangle` icon in `--warn`
  before it and the words in `--text` at `--font-size-sm`, and it is linked
  to Create archive with `aria-describedby` (together with the reason, when
  there is one):
  - `failedEntries` non-empty: "1 file will be left out; errors will be
    listed after the build" / "{failedEntries.length} files will be left out;
    errors will be listed after the build";
  - only manifest-level errors: "Builds with 1 manifest error" / "Builds with
    {errors.length} manifest errors".

  It never says the errors block the build or must be fixed first.
- **Show errors**, whenever `errorCount > 0` (with any reason, or none): a
  secondary (not primary) button after the status text and before Create
  archive, calling `onShowErrors`, which `TarpackView` wires to
  `showErrors()`. It has `aria-controls="manifest-error-report"`. Tab order:
  Choose…, Format, Show errors, Create archive (skipped when disabled).
- **Create archive** is enabled exactly when `session.canBuild` is true and
  no build is running, including when `errorCount > 0`. Building with errors
  opens no dialog of its own.
  1. Call `build(false)`.
  2. On an `OutputExists` error, open `ConfirmDialog` with the title
     "Replace {file name}?", the body "A file with this name already exists in
     {folder}. Replacing it can't be undone.", the confirm label "Replace",
     and initial focus on **Cancel**. `{file name}` is
     `fileName(session.outputPath)` and `{folder}` is
     `displayFolder(session.outputPath)`, both from `pathParts.ts`.
     `{folder}` is for display only, so it **shows a lossy path as is**,
     U+FFFD included; it never falls back to "that folder" or other
     invented copy. Do not use `assignedFolder` here (it returns `undefined`
     for lossy paths because its result goes back to `lib`). Only if
     `displayFolder` finds no separator (not expected: `outputPath` is
     absolute) is the body "A file with this name already exists.
     Replacing it can't be undone."
  3. On confirm, call `build(true)`.
- **Building: `BuildProgress`**, in place of the disabled reason.
  - A visible step label: "Step 1 of 2 · Writing" then "Step 2 of 2 ·
    Verifying". The phase is always in text, never shown by color alone.
  - The bar is 6 px tall with a `--surface-sunken` track and an `--accent`
    fill, and `--radius` on both. The fill moves with `transform: scaleX()`
    from the left, with a 120 ms linear transition between events. It does
    not animate `width`. With `prefers-reduced-motion`, there is no
    transition: the fill jumps to each value, which still shows the state.
    The percentage beside it uses `.num`, so the digits do not jitter.
  - One determinate progress bar (`role="progressbar"`, `aria-valuemin="0"`,
    `aria-valuemax="100"`, `aria-valuenow`, and an accessible name "Building
    archive"). It fills from `bytesDone / bytesTotal` up to 100% in each
    phase, and starts over when the phase changes to `verifying`, showing
    that phase's first event (which may already be a little past 0).
  - `aria-valuetext` describes the **current event only**: "Writing
    gateway.conf, 40%" or "Verifying gateway.conf, 40%", mapping `entryId` to
    the entry's `source`. For an event whose `entryId` is `null`
    (directories, long-name records, the phase's final event) it is exactly
    "{Phase}, {n}%", for example "Writing, 40%" or "Verifying, 100%", with
    **no file name**; it never reuses a remembered name.
  - The **visible** label alone keeps the last named file, so it does not
    flicker to no name on directory records. That remembered file belongs to
    its phase: it is **cleared when the phase changes** (writing →
    verifying), so the verifying label never shows a file from the writing
    phase. Until the first named verifying event, the visible label shows
    the phase with no file.
  - Announce the phase change once in a polite live region ("Verifying the
    archive"); do not announce every percentage.
  - When the final verifying event arrives (`phase: "verifying"`,
    `bytesDone == bytesTotal`), show "Finishing…" with the bar full until
    `build` settles. It covers only the flush and rename, so it is brief; do
    not wait for any further event.
  - If `build` rejects, stop showing progress wherever it stopped and show the
    error; no further events arrive.
  - **Subscription failure.** Subscribe with `onBuildProgress` before calling
    `build`. If the subscription itself rejects, still run the build, but do
    not show a progressbar stuck at 0%: `TarpackView` sets
    `progressUnavailable`, and `BuildBar` shows, in place of the bar, the
    text "Creating the archive… (progress is unavailable)" in
    `--text-muted` at `--font-size-sm`, with Create archive `aria-busy`.
    The build's own result or error then replaces it as usual. Unsubscribe
    whenever the build settles, and ignore a late subscription that
    resolves after the build settled (unlisten it at once).
  - All tarpack controls (bar, format picker, table actions, header actions)
    are disabled, and drops are ignored. **Mechanism (accepted at review):**
    the header and the table sit inside one
    `<fieldset className="tarpack__controls" disabled={building}>` with
    `display: contents` (and no border, padding, or legend), so a single
    attribute disables every native control in them without changing layout.
  - **While building, the bar's `aria-describedby` points only at rendered
    elements.** The reason and note are not rendered while the progress
    replaces them, so drop their ids from Create archive's
    `aria-describedby` (omit the attribute when nothing is left); never
    reference an id that is not in the DOM.
  - **Focus after the build ends.** When a build settles (success, error,
    cancelled confirmation) or its result is dismissed, move focus to
    **Create archive** only if focus was lost, that is when
    `document.activeElement` is `document.body` or `null` (the focused
    control was disabled or unmounted). If focus is anywhere else, leave it
    there. The one exception is `ConfirmDialog`, which always returns focus
    to Create archive (below).
- **Success: `BuildResult`**, an inline panel above the bar: `--surface`
  with a 1 px `--border` and `--radius`, with no shadow and no tinted fill. Its
  heading is an `h2` at `--font-size-lg` (the report's sections are `h3`, its
  entries `h4`). It is dismissible with a quiet `Button` (the
  `x` icon, "Dismiss result"). The heading and icon depend on
  `summary.errorCount`; the words always carry the meaning, never the color
  alone:
  - `errorCount` 0: `check-circle` in `--ok`, "Created {file name}". This is
    the success-only layout: no report sections, except a collapsed Warnings
    disclosure when `warnings` is non-empty;
  - `errorCount > 0` and `leftOut` non-empty: `alert-triangle` in `--warn`,
    "Created {file name} with 1 file left out" / "Created {file name} with
    {leftOut.length} files left out". Directly under the heading, in
    `--text`: "The archive holds {builtIds.length} of the manifest's
    {builtIds.length + leftOut.length} files. The rest have errors, listed
    below.";
  - `errorCount > 0` and `leftOut` empty (manifest-level errors only):
    `alert-triangle` in `--warn`, "Created {file name} with 1 manifest error"
    / "Created {file name} with {manifestErrors.length} manifest errors".

  An error result (a failed build) uses `x-circle` in `--danger`. A success
  panel then contains:
  - the full path in `--font-mono`;
  - the format and sizes: "zstd · {files} files, {dirs} folders ·
    {bytes} ({uncompressedBytes} uncompressed)", sizes in KB or MB. For
    `"tar"`, show only one size;
  - the SHA-256 in `--font-mono`, wrapping (`overflow-wrap: anywhere`),
    never truncated, with **Copy** (a quiet `Button` with the `copy` icon),
    accessible name "Copy SHA-256";
  - **Extract on the target**: `extractCommand` in a `--font-mono` block on
    `--surface-sunken`, wrapping (never truncated or ellipsised), with **Copy**,
    accessible name "Copy extraction command", and the hint "Run this in the
    folder that holds the file. -P keeps the absolute paths." Render the
    command exactly as given;
  - **Line endings converted**, only when `normalizedEntries` is non-empty: a
    list of "{source}: {crlfReplaced} CRLF replaced", or "{source}: no CRLF
    found" for 0, with `source` looked up from the entry id;
  - **the final report** (`BuildReport`), when `errorCount > 0` or `warnings`
    is non-empty, described below;
  - **Show in folder**, which calls `revealOutput()`, and, when `errorCount >
    0`, **Copy report** (a quiet `Button` with the `copy` icon and that
    visible label).

  Copy uses `navigator.clipboard.writeText` and confirms with "Copied" in a
  polite live region next to the button for about 2 s. If `writeText`
  rejects, the same live region says "Couldn't copy" for about 2 s instead;
  never "Copied" for a failed write, and never an unhandled rejection. This
  applies to both Copy buttons and to Copy report.
- **`BuildReport`**, the part of the result that lists what the build left
  out. A `role="region"` labelled "Build report" (visually hidden label),
  `tabIndex={0}`, max height `40vh`, scrolling inside itself, set off from the
  details above by a 1 px `--border` rule. Its sections, each with an `h3`
  at `--font-size-md` weight 600 and its count in `.num`:
  - **Left out of the archive ({leftOut.length})**, when `leftOut` is
    non-empty: a `FailureList` of `leftOut` with `headingLevel={4}`;
  - **Manifest errors ({manifestErrors.length})**, when non-empty: a
    `DiagnosticList` of `manifestErrors` (label "Manifest errors");
  - **Warnings ({warnings.length})**, when non-empty: a collapsed `<details>`
    whose summary is that heading, holding a `DiagnosticList` of `warnings`
    (label "Warnings").

  Items are not tab stops; the region is one. Every message renders
  verbatim and wraps.
- **`reportText(summary)`** builds the plain text for **Copy
  report**: the heading line; then "Left out of the archive:" with one line
  per entry (its name as in `FailureList`, plus "(source {source}, line
  {line})"), each followed by its errors indented as "{line}:{col} {message}";
  then "Manifest errors:" and "Warnings:" in the same item format. Empty
  sections are omitted. It formats only what the summary holds (the file
  name in the heading line is the one the heading shows) and adds nothing
  else.
- **Announce the result** with `announce()`, once, when `build` resolves:
  - `errorCount` 0: "Created {file name}.";
  - with `leftOut`: "Created {file name}. 1 file was left out because of
    errors." / "… {n} files were left out because of errors.";
  - manifest-level errors only: "Created {file name} with 1 manifest error."
    / "… with {n} manifest errors."
  - **a failed build** (P0, added after review): when `build` rejects with
    any kind other than `OutputExists` (which opens the dialog) and
    `ManifestChangedOnDisk` (whose warn banner is already announced), call
    `announce()` once with `errorMessage(error, entries)`, the same string
    the error `BuildResult` shows. The copy source is `errorMessage`; add no
    wording of your own. The error `BuildResult` is not itself a live region
    (no `role="alert"`), so the message is announced exactly once.
- **No "log" or "saved report" wording.** The report lives in this panel
  only; it is gone when dismissed or replaced by the next build. Do not
  write "log", "saved", "report file", or anything that suggests a file was
  written beside the archive.
- **`ConfirmDialog` look.** A native `<dialog>` opened with `showModal()`, so
  the top layer handles stacking and inertness. It is `--surface` with a
  1 px `--border`, `--radius`, `--shadow-overlay`, and a
  `::backdrop` of `rgb(0 0 0 / 0.4)`. Its width is
  `min(28rem, calc(100% - 2 * var(--space-5)))`, and its padding
  `--space-5`. The title is at
  `--font-size-lg`. Buttons are right-aligned: **Cancel** (secondary), then
  **Replace** (primary). The `<dialog>` has `aria-labelledby` pointing at the
  title and `aria-describedby` pointing at the body; both ids come from
  `useId()` (never hard-coded, so two dialogs cannot collide).
- **Errors.** Every error comes from `build`, `setOutput`, `setFormat`, or
  `revealOutput`:
  - `OutputExists` from `build(false)`: open the replace confirmation above.
    Never shown as an error.
  - `ManifestChangedOnDisk`: show the warn banner "The manifest changed on
    disk." with **Reload**, not a `BuildResult`. The backend raises this kind
    both when the file's hash differs and when the manifest can no longer be
    read at build time (deleted, locked); the copy fits both, and if the
    file is gone, **Reload** then reports `ManifestUnreadable` with its own
    copy.
  - Every other kind from `build`: an error `BuildResult` whose message is
    `errorMessage(error, entries)`, with the backend `message` below it in a
    collapsed "Details" disclosure, and that same `errorMessage` announced
    once through `announce()` (see Announce the result).
  - Errors from `setOutput`, `setFormat`, or `revealOutput` (for example
    `OpenerFailed` from Show in folder): an error `Banner` with the same
    message and Details.

  The copy `errorMessage` returns, for all 17 kinds. `{source}` is the
  `source` of the entry named by `entryId`, or "A file" when `entryId` is
  omitted or unknown:

    | Kind | Raised by | Message |
    | --- | --- | --- |
    | `NoManifest` | any command needing a manifest | "Open a manifest first." |
    | `ManifestUnreadable` | open, reload | "The manifest could not be read. Check that the file still exists and that you can open it." |
    | `NoEntries` | build | "There are no files that can be built. Fix the manifest errors first." |
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

  The kinds that matter most for this task are `SourceMissing`,
  `SourceUnreadable`, `SourceChanged`, and `VerifyFailed`. `NoManifest`,
  `NoEntries`, `NoOutput`, `EntriesNotReady`, and `BuildInProgress` are
  defensive: the disabled states normally prevent them.

**Rules that bind this task.**

- Tokens only, with font sizes from `--font-size-*`. Use `Button` and `Icon`;
  no `<button>` and no glyph icons. Sizes, counts, and percentages use `.num`.
- Everything is keyboard reachable and labelled: Choose…, the Format picker,
  Show errors, Create archive, both Copy buttons, Copy report, Show in folder,
  the Details disclosure, the Warnings disclosure, the report region, and
  Dismiss are real, focusable controls with visible focus.
- `ConfirmDialog` traps focus, closes on Escape (which counts as Cancel), and
  returns focus to **Create archive**. Elsewhere, focus moves to Create
  archive after a build or Dismiss only when it was lost (see Building).
- **Create archive** is the only `primary` button in the view, outside the
  dialog.
- No confirmation dialog for building with errors, and no copy that says
  errors block or prevent the build (except the `noEntries` reasons and the
  defensive `NoEntries` message, when no file passed).
- Progress animation respects `prefers-reduced-motion`.
- Call only `lib/` functions (plus `navigator.clipboard.writeText`). No
  extension, file-name, filter, or command logic in TS.

## Files

- `src/tools/tarpack/BuildBar.tsx`, `FormatPicker.tsx`, `BuildProgress.tsx`,
  `BuildResult.tsx`, `BuildReport.tsx`, `reportText.ts`
- `src/tools/tarpack/pathParts.ts` (edit): add `fileName(path): string` (the
  last path segment, code-point safe, moved here from `reportText.ts`; update
  every import) and `displayFolder(path): string | undefined` (the same
  folder split as `assignedFolder` but without the lossy check, for display
  only; `assignedFolder` may delegate to it), with tests in
  `pathParts.test.ts`
- `DESIGN.md` (root), Elevation & Depth (~line 185): change "the confirmation
  dialog (planned, U5)" to "the confirmation dialog (landed, U5,
  `ConfirmDialog`)". This is a status update only, not a rewrite.
- `src/app/ConfirmDialog.tsx`
- Wiring in `TarpackView.tsx`
- Styles: `src/styles/controls.css` (`ConfirmDialog`), `src/styles/tarpack.css`
  (the bar, progress, result, and report, scoped under `.tarpack`)
- Tests next to each

## Skill

`/impeccable layout` and `/impeccable clarify` (including every left-out,
report, and announcement string), then `/impeccable animate` for the progress
bar only. **`/impeccable audit` is waived for U5 (decided 2026-09-30,
after re-review)**: the axe checks in the tests and the rendered check below
stand in for it, and U6 runs the audit, with the detector, over U5's bar,
dialog, result, and report.

How to run it:

- Start with the skill's `impeccable context`, which loads the root
  `PRODUCT.md` and `DESIGN.md`.
- This is an Operate surface extending an established world, so run no
  concept round and do not rewrite `DESIGN.md`.
- Read the skill's `reference/craft-floor.md` before the first edit.
- Motion here conveys state only: the progress fill.
- **Rendered check (required, not optional).** Chromium is pre-installed at
  `/opt/pw-browsers/chromium` and Playwright is configured globally
  (`PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers`, the `playwright` package is
  installed globally). **Do not run `playwright install`.** Run the Vite dev
  server (`npm run dev` in `apps/desktop`) and drive it with a Playwright
  script kept in your scratch directory (not committed). Mock the Tauri API
  with `page.addInitScript` before navigation: define
  `window.__TAURI_INTERNALS__` with an `invoke(cmd, args)` that returns
  fixture sessions and summaries per `tarpack_*` command (the shapes in
  `src/tools/tarpack/fixtures.ts`), and a `transformCallback` so
  `listen`/`onBuildProgress` can be fed scripted events. Check at 1400×800,
  1000×700, and 800×560, in light and dark (`colorScheme`), and at 200% text
  (root font size 32 px, via `page.addStyleTag({ content: 'html{font-size:32px}' })`,
  for text reflow; **for anything involving media queries, use a 400×280
  viewport instead**, since the root-font hack does not trigger them):
  the bar flush at the bottom edge while scrolling and at the end; at
  400×280 the bar static (short-window fallback) and `scroll-padding-bottom`
  0; no horizontal scroll
  (`scrollWidth <= clientWidth` on `.tarpack` and `document.documentElement`);
  every control reachable by Tab; the `scroll-padding-bottom` measurement
  above; the result and dialog in both themes. Take screenshots of each and
  look at them. (`/impeccable audit` is waived to U6; see above.)
  Known and deferred to U6: U2's manifest header overflows by about 3 px at
  400×280; do not fix it in U5.
- If the skill is not installed, install it with `npx impeccable install`, or
  follow the named commands' reference docs from
  `github.com/pbakaus/impeccable` by hand. Say which in your report.

## Acceptance criteria

- The bar is `position: sticky; bottom: calc(var(--view-pad-block) * -1)`
  inside `.tarpack`, flush with the bottom edge both while scrolling and at
  the end of the scroll, `.tarpack` sets a rem `scroll-padding-bottom`
  (14.5rem; 0 and a static bar at `max-height: 30rem`), and neither the bar, the result, nor
  the table adds a scroll container (only `BuildReport`, capped at `40vh`).
  At 800×560 with 200% text the view scrolls to every control, and nothing
  scrolls horizontally.
- The output path uses `MiddlePath`: the drive and file name stay visible,
  and the full path is in `title` and is the accessible text.
- Each disabled reason shows its exact visible text, and the button is
  `disabled`. The button reads "Create archive" in every state.
- With `buildBlockedReason: "noEntries"`, the bar shows the withheld, the
  every-file-failed, or the no-files sentence for its fixture, and **Show
  errors** only when `errorCount > 0`.
- With `canBuild: true` and `errorCount > 0`, **Create archive is enabled**,
  the left-out note shows its count (singular and plural), or "Builds with N
  manifest errors" when only manifest-level errors exist, and Create
  archive's `aria-describedby` includes the note. **Show errors** sits before
  Create archive in tab order, calls `onShowErrors`, and in `TarpackView`
  focus lands on the error report. Clicking Create archive calls
  `build(false)` with no dialog in between.
- No bar string says errors must be fixed before building, except the
  `noEntries` withheld sentence; there is no `manifestInvalid` branch
  anywhere.
- The Format picker has a visible label, lists `session.formats` in order with
  their extensions, shows `session.format`, and is operable with the keyboard
  alone. Changing it calls `setFormat` with the chosen value, and the output
  path shown afterwards is the one in the returned session.
- Choose… calls `saveFileDialog` with `defaultPath` from `outputPath` or
  `suggestedOutputName` and a single filter whose `extensions` is exactly
  `[filterExtension]` of the current format, then `setOutput`. A cancelled
  dialog makes no call. No TS code splits or slices `extension`.
- With no manifest, the picker shows "tar (.tar)" and is `disabled`, Choose…
  is `disabled`, and neither `setFormat` nor `setOutput` is ever called.
- When `setOutput` returns a different format, the picker shows it and the
  live region announces the switch.
- The overwrite flow is: `build(false)`, then `OutputExists`, then the dialog.
  Cancel makes no further call; Replace calls `build(true)`.
- Progress shows "Step 1 of 2 · Writing" then "Step 2 of 2 · Verifying", each
  starting over and running up to 100% from events, with `aria-valuetext` naming the phase and
  file (no file for `null` `entryId`); "Finishing…" appears on the final
  verifying event without waiting for another event; a rejected build stops
  the progress and shows the error. Controls, including the picker, are disabled while building.
- The progress fill animates `transform`, not `width`, and has no transition
  under `prefers-reduced-motion`. The percentage uses tabular numerals.
- The dialog uses `--shadow-overlay`; no other element in this task has a
  shadow.
- The success view shows every summary field: file name, path, format, both
  sizes, SHA-256, the extraction command verbatim, and each normalised entry.
  Each Copy writes its exact text to the clipboard and announces "Copied".
- With `summary.errorCount > 0`, the heading reads "Created {file} with N
  files left out" (or "with N manifest errors"), the icon is
  `alert-triangle`, and the words carry the tone. The report lists every
  `leftOut` entry, named by `id` or "Entry #index", with its source, its
  `[[file]]` line, and every error with `line:col`; then the manifest errors;
  then the warnings, collapsed. With `errorCount` 0 there is no report
  section except collapsed warnings when present.
- The result is announced once, including the left-out count.
- Copy report writes `reportText(summary)` to the clipboard. No
  string in the result mentions a log or a saved report.
- A failed build (any kind except `OutputExists` and
  `ManifestChangedOnDisk`) announces `errorMessage(error, entries)` once
  through the view's announcer.
- `aria-valuetext` for a `null`-`entryId` event is exactly "{Phase}, {n}%";
  the visible label resets its remembered file at the phase change.
- A failed `onBuildProgress` subscription shows "Creating the archive…
  (progress is unavailable)", not a 0% progressbar.
- A rejected clipboard write announces "Couldn't copy".
- During a real build (via `TarpackView`), Reload, Edit in editor, Browse…,
  and Clear are `disabled`; after it ends, focus goes to Create archive only
  if it was on `body`.
- The confirm dialog has `aria-labelledby` and `aria-describedby` from
  `useId()`, and its body names the folder of a lossy output path as is.
- While building, every id in Create archive's `aria-describedby` exists in
  the DOM.
- `ManifestChangedOnDisk` shows the banner, not a generic error.
  `SourceChanged` and `VerifyFailed` show their messages from the table above,
  via `errorMessage`; no component contains its own error copy or an
  "any other" branch.

## Tests proving completion

`npm run test`, with `lib` mocked. Tests select the view's announcer by a
stable hook, never by position (`getAllByRole("status")[0]` breaks as soon as
another live region renders first): give the announcer
`data-testid="tarpack-announcer"` and query it with `getByTestId`.

- `BuildBar.test.tsx`: each disabled reason, including the three `noEntries`
  sentences; a `canBuild: true` session with failed entries (button
  enabled, the left-out note singular and plural, `aria-describedby`, **Show
  errors** calling `onShowErrors` and preceding Create archive); a
  `canBuild: true` session with manifest-level errors only ("Builds with N
  manifest errors"); an `entriesNotReady` session with errors (reason and
  note both shown); the button text; Choose calling
  `saveFileDialog` with the right `defaultPath` and
  `extensions: [filterExtension]`, then `setOutput`; cancelled Choose; the
  no-manifest session (picker and Choose disabled, no `lib` call).
- `FormatPicker.test.tsx`: options and order from `formats`, the selected
  value, `onChange` on keyboard selection, and the disabled state.
- `TarpackView.build.test.tsx`:
  - the happy path;
  - `setFormat` re-rendering the output path from the returned session;
  - `setOutput` returning a switched format, and the announcement;
  - `OutputExists`, then Cancel;
  - `OutputExists`, then Replace;
  - `ManifestChangedOnDisk`;
  - `SourceMissing`, naming the file, and with `entryId` omitted ("A file");
  - `SourceChanged` and `VerifyFailed`, with their messages and details;
  - progress rendering from a scripted event sequence that follows the
    contract (file ids, `null`-id directory events, a final `null`-id 100%
    event per phase): the reset at the phase change, `aria-valuetext` with
    no file name on `null`-id events, the visible label not showing a
    writing-phase file during verifying, "Finishing…" on the final
    verifying event, and a rejection mid-verify stopping the progress;
  - `OpenerFailed` from Show in folder, as a banner;
  - **Show errors** in the bar moving focus to the error report;
  - a failed build (`SourceMissing`, `VerifyFailed`) announcing its
    `errorMessage` once in the view's announcer; `OutputExists` and
    `ManifestChangedOnDisk` announcing no error message;
  - `onBuildProgress` rejecting: the build still runs, the "progress is
    unavailable" text shows, and no progressbar renders;
  - during a real build (a pending `build` promise), Reload, Edit in editor,
    Browse…, and Clear are `disabled`, and re-enabled when it settles;
  - a `saveFileDialog` rejection from Choose… showing the error banner;
  - the lossy output path in the replace dialog body;
  - a build with `errorCount > 0`: no dialog, `build(false)` called once, the
    result with the left-out heading and report, and the announcement text.
- `BuildResult.test.tsx`: all summary fields, one size for `"tar"`, the
  extraction command verbatim, both Copy buttons (with
  `navigator.clipboard.writeText` stubbed), and the normalised-entries
  list (including a 0 count and its absence when empty); the three headings
  (no errors, files left out singular and plural, manifest errors only) with
  their icons; the "holds N of M files" line; the success-only layout at
  `errorCount` 0, with and without warnings.
- `BuildReport.test.tsx`: each section present only when non-empty, a
  `leftOut` entry with `id: null`, every error listed, warnings collapsed,
  and the region bounded and focusable.
- `reportText.test.ts`: the text for a summary with left-out entries,
  manifest errors, and warnings; empty sections omitted.
- `ConfirmDialog.test.tsx`: focus starts on Cancel, Escape cancels, focus
  returns afterwards, and `aria-labelledby` / `aria-describedby` resolve to
  the title and body.
- `BuildResult.test.tsx` also covers a rejected `writeText` ("Couldn't
  copy").
- `pathParts.test.ts`: `fileName` and `displayFolder`, including a lossy
  path.
- Axe checks on the bar in each state (including both progress phases and
  ready with files left out), on the dialog, and on the success (with and
  without a report) and error results.

## States covered

No manifest (picker and Choose disabled), disabled (each reason, including
the three `noEntries` cases), ready, ready with files left out (note, Show
errors, button enabled), ready with manifest errors only, format
changed, format switched by file name, confirming, building (writing,
verifying, finishing), success (with and without normalised entries), success
with files left out, success with manifest errors only, success with warnings
only, and error (each kind above).

## Fixes required after review (2026-09-30)

The first implementation (commits 716297b, 2357f69) was reviewed. Everything
above is the target; this list is what the review found missing, so fix each
one:

1. **P0.** Announce a failed build: `errorMessage(error, entries)`, once,
   through `announce()` (not for `OutputExists` or `ManifestChangedOnDisk`).
2. Picker labels are the short names tar, gzip, zstd, xz (option text
   "tar (.tar)" etc.); keep them.
3. `aria-valuetext` for a `null`-`entryId` event is "{Phase}, {n}%" with no
   file; the visible label's remembered file resets at the phase change.
4. Keep the accepted decisions, and add the missing test: `BuildBar` routes
   `saveFileDialog`, `setOutput`, and `setFormat` errors to the view's
   banner through `onError`; the `<fieldset disabled>` with
   `display: contents`, plus a `TarpackView` test that Reload, Edit in
   editor, Browse…, and Clear are disabled during a real build; focus to
   Create archive after a build or Dismiss only when focus was lost;
   `ConfirmDialog` `aria-describedby` and `useId()` ids; no dangling
   `aria-describedby` ids while building.
5. Replace the "that folder" fallback: the dialog body uses
   `displayFolder(outputPath)`, which shows a lossy folder as is.
6. Measure `scroll-padding-bottom` as described under Scroll model, and do
   the rendered check in Chromium (Skill section).
7. A clipboard failure announces "Couldn't copy".
8. Handle a failed `onBuildProgress` subscription ("progress is
   unavailable"; no progressbar stuck at 0%).
9. Move `fileName` from `reportText.ts` to `pathParts.ts`.
10. Tests select the view's announcer by `data-testid="tarpack-announcer"`,
    not `getAllByRole("status")[0]`.
11. Update `DESIGN.md` (~line 185): the confirmation dialog is landed
    (`ConfirmDialog`, U5), no longer planned.

### Fixes required after re-review (2026-09-30)

The fixes above landed in 49ed82a and the product code was accepted. The
re-review left test fixes only:

- **F1.** In `TarpackView.build.test.tsx` (~lines 382-395), the "focus
  elsewhere stays there" part proves nothing: it focuses the Format picker,
  which is disabled during the build, and asserts only
  `document.activeElement !== document.body`. Rewrite it: mount a session
  with manifest errors, so U2's error report region (`tabIndex={0}`,
  outside the `<fieldset>`, so not disabled) is rendered; start a build
  with a pending `build` promise; focus that report region; settle the
  build; assert the report region has focus and Create archive does not.
  Delete the comment about simulating a user who moved on.
- **F3.** The disabled-controls test (~lines 320-338) uses an unassigned
  entry, so Clear is disabled anyway, and never proves Edit in editor
  exists. Use an entry with an assigned file. For each of Reload, Edit in
  editor, Browse…, and Clear, assert the button exists (`getByRole`, not
  `queryAllByRole` over a possibly empty list), is `disabled` while the
  build is pending, and is enabled after it settles.
- **F6.** Give the test in `EntryTable.scale.test.tsx` (~line 16, "2,000
  rows render…") an explicit timeout of 15000 ms (third argument to
  `test`); it flakes under full-suite load. Optional: in the "announces no
  error message" test, replace the 50 ms sleep and regex with an assertion
  that the announcer (`getByTestId("tarpack-announcer")`) never contains the
  exact `errorMessage(...)` string for that error.

These are tests only; change no product code for them.

## Out of scope

- Drops (U4).
- Keyboard shortcuts (U6).
- Compression levels or any format option beyond the four formats.
