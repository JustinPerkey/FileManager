# U5 — Output, format, build, overwrite confirmation, progress, and result

Status: awaiting approval
Project: tarpack   Depends on: U3 (landed), M6 (landed). May run alongside U4.

## Goal

Let the user choose where the archive goes and in which format, create it
safely with visible two-phase progress, and get a result they can verify and
extract on the target.

## Context

**The user and the target.** A developer rebuilding the same package many
times a day for a low-power armv7 Linux machine running GNU tar. The archive
stores **absolute** paths (`/opt/...`), so on the target it must be extracted
with `tar -P`; without `-P`, tar strips the leading `/` and extracts into the
current directory. The build result therefore shows the exact extraction
command and lets the user copy it.

**Formats (decided).** Four formats, picked before building:

| `ArchiveFormat` | Extension (from the session) | Picker label (your copy) |
| --- | --- | --- |
| `"tar"` | `.tar` | tar — uncompressed |
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

**What exists.**

- `TarpackView.tsx` holds the `TarpackSession` in one piece of React state,
  replaced wholesale by every command result, and has a reserved slot for the
  sticky bottom bar. `src/app/Banner.tsx` exists:
  `tone: "info"|"warn"|"error", message, action?: { label, onAction }`.
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
    - `writing` comes first; `verifying` follows and runs its own pass from 0;
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
  `navigator.clipboard.writeText` directly from the Copy click handler.
- From the session:
  - `session.manifest`: `null` or the loaded manifest; `entries[]` carry
    `{ id, source, ... }` for mapping an `entryId` to a file name;
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
  - `session.canBuild`;
  - `session.buildBlockedReason`: one of `"noManifest"`, `"manifestInvalid"`,
    `"entriesNotReady"`, `"noOutput"`, or `null`;
  - `session.readyCount` and `session.totalCount`.
- `BuildSummary`:
  `{ path, format, entries, files, dirs, bytes, uncompressedBytes, sha256Hex, extractCommand, normalizedEntries: { id, crlfReplaced }[] }`.
  - `bytes` is the size of the output file on disk; `uncompressedBytes` is the
    size of the tar stream before compression (equal to `bytes` for `"tar"`).
  - `extractCommand` is the exact command for the target, for example
    `tar --zstd --no-overwrite-dir -xpPf gateway.tar.zst`. It uses the file
    name only, so it runs in the directory that holds the archive.
  - `normalizedEntries` lists every entry with line-ending conversion on, with
    the number of CRLF pairs replaced (possibly 0).
- `TarpackError`: `{ kind: TarpackErrorKind, message: string, entryId?: string }`.
  `entryId` is omitted (not `null`) when absent. `message` is the backend's
  technical detail, not user copy. `TarpackErrorKind` is a closed
  string-literal union of exactly 17 kinds (table under Behaviour → Errors).
- `src/tools/tarpack/errorMessages.ts` (U2) exports
  `errorMessage(error, entries): string`, backed by an exhaustive
  `Record<TarpackErrorKind, (source: string | null) => string>`. Use it for
  every error you display; do not write copy of your own or add a fallback.
  If a message below differs from the file, the file was written from this
  same table, so report the difference rather than patching around it.
- The tokens you use are in `src/styles/tokens.css`: `--surface`,
  `--surface-sunken`, `--border`, `--accent`, `--accent-text`, `--ok`,
  `--danger`, `--text`, `--text-muted`, `--font-mono`, `--radius`, and the
  `--space-*` scale.

### Components

| Component | Path | Props | States |
| --- | --- | --- | --- |
| `BuildBar` | `src/tools/tarpack/BuildBar.tsx` | `session, building, progress, onChooseOutput, onFormatChange, onBuild` | disabled-with-reason / ready / building |
| `FormatPicker` | `src/tools/tarpack/FormatPicker.tsx` | `formats, value, disabled, onChange(format)` | enabled / disabled |
| `BuildProgress` | `src/tools/tarpack/BuildProgress.tsx` | `progress, entries` | writing / verifying / finishing |
| `BuildResult` | `src/tools/tarpack/BuildResult.tsx` | `result: { ok: BuildSummary } \| { err: TarpackError }, entries, onReveal, onDismiss` | success / error |
| `ConfirmDialog` | `src/app/ConfirmDialog.tsx` (shared) | `open, title, body, confirmLabel, onConfirm, onCancel` | open |

### Behaviour

- **`BuildBar`**, sticky at the bottom of the view. Left to right, wrapping to
  two lines when narrow or at 200% text:
  - "Output:", the path in `--font-mono` (middle-truncated, full path in
    `title`) or "not chosen", and **Choose…**;
  - the **Format** picker;
  - the disabled reason, when there is one;
  - the primary **Create archive** button. The wording is format-neutral;
    never "Create tar".
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
- **Format switched by the file name.** If the session returned by
  `setOutput` has a different `format` from the one before the call, announce
  it in a polite live region in the bar: "Format changed to xz to match the
  file name." The picker already shows the new value because it renders the
  session.
- **Disabled reason**, shown as visible text next to the button (not only a
  tooltip) and linked with `aria-describedby`:
  - `noManifest`: "Open a manifest first"
  - `manifestInvalid`: "Fix the manifest problems first"
  - `entriesNotReady`: "{totalCount − readyCount} files still need a location"
  - `noOutput`: "Choose where to save the archive"
- **Create archive**:
  1. Call `build(false)`.
  2. On an `OutputExists` error, open `ConfirmDialog` with the title
     "Replace {file name}?", the body "A file with this name already exists in
     {folder}. Replacing it can't be undone.", the confirm label "Replace",
     and initial focus on **Cancel**.
  3. On confirm, call `build(true)`.
- **Building: `BuildProgress`**, in place of the disabled reason.
  - A visible step label: "Step 1 of 2 · Writing" then "Step 2 of 2 ·
    Verifying". The phase is always in text, never shown by color alone.
  - One determinate progress bar (`role="progressbar"`, `aria-valuemin="0"`,
    `aria-valuemax="100"`, `aria-valuenow`, and an accessible name "Building
    archive"). It fills 0 → 100% from `bytesDone / bytesTotal` in each phase,
    and resets to 0 when the phase changes to `verifying`.
  - `aria-valuetext` names the phase and the entry: "Writing gateway.conf,
    40%" or "Verifying gateway.conf, 40%", mapping `entryId` to the entry's
    `source`; with a `null` `entryId` (directories, long-name records, the
    phase's final event), "Writing, 40%". Keep the visible label on the last
    named file rather than flickering to no name on directory records.
  - Announce the phase change once in a polite live region ("Verifying the
    archive"); do not announce every percentage.
  - When the final verifying event arrives (`phase: "verifying"`,
    `bytesDone == bytesTotal`), show "Finishing…" with the bar full until
    `build` settles. It covers only the flush and rename, so it is brief; do
    not wait for any further event.
  - If `build` rejects, stop showing progress wherever it stopped and show the
    error; no further events arrive.
  - All tarpack controls (bar, format picker, table actions, header actions)
    are disabled, and drops are ignored.
- **Success: `BuildResult`**, dismissible, with:
  - "Created {file name}", then the full path in `--font-mono`;
  - the format and sizes: "zstd · {files} files, {dirs} folders ·
    {bytes} ({uncompressedBytes} uncompressed)", sizes in KB or MB. For
    `"tar"`, show only one size;
  - the SHA-256 in `--font-mono` with **Copy**, accessible name "Copy
    SHA-256";
  - **Extract on the target**: `extractCommand` in a `--font-mono` block on
    `--surface-sunken`, wrapping (never truncated or ellipsised), with **Copy**,
    accessible name "Copy extraction command", and the hint "Run this in the
    folder that holds the file. -P keeps the absolute paths." Render the
    command exactly as given;
  - **Line endings converted**, only when `normalizedEntries` is non-empty: a
    list of "{source}: {crlfReplaced} CRLF replaced", or "{source}: no CRLF
    found" for 0, with `source` looked up from the entry id;
  - **Show in folder**, which calls `revealOutput()`.

  Copy uses `navigator.clipboard.writeText` and confirms with "Copied" in a
  polite live region next to the button for about 2 s.
- **Errors.** Every error comes from `build`, `setOutput`, `setFormat`, or
  `revealOutput`:
  - `OutputExists` from `build(false)`: open the replace confirmation above.
    Never shown as an error.
  - `ManifestChangedOnDisk`: show the warn banner "The manifest changed on
    disk." with **Reload**, not a `BuildResult`.
  - Every other kind from `build`: an error `BuildResult` whose message is
    `errorMessage(error, entries)`, with the backend `message` below it in a
    collapsed "Details" disclosure.
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
    | `ManifestInvalid` | build | "Fix the manifest problems first." |
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
  `ManifestInvalid`, `NoOutput`, `EntriesNotReady`, and `BuildInProgress` are
  defensive: the disabled states normally prevent them.

**Rules that bind this task.**

- Tokens only.
- Everything is keyboard reachable and labelled: Choose…, the Format picker,
  Create archive, both Copy buttons, Show in folder, the Details disclosure,
  and Dismiss are real, focusable controls with visible focus.
- `ConfirmDialog` traps focus, closes on Escape (which counts as Cancel), and
  returns focus to **Create archive**.
- Progress animation respects `prefers-reduced-motion`.
- Call only `lib/` functions (plus `navigator.clipboard.writeText`). No
  extension, file-name, filter, or command logic in TS.

## Files

- `src/tools/tarpack/BuildBar.tsx`, `FormatPicker.tsx`, `BuildProgress.tsx`,
  `BuildResult.tsx`
- `src/app/ConfirmDialog.tsx`
- Wiring in `TarpackView.tsx`
- Tests next to each

## Skill

`impeccable:layout` and `impeccable:clarify`, then `impeccable:animate` for the
progress bar only. Finish with `impeccable:audit` on the bar, dialog, and
result.

## Acceptance criteria

- Each disabled reason shows its exact visible text, and the button is
  `disabled`. The button reads "Create archive" in every state.
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
  running 0 → 100% from events, with `aria-valuetext` naming the phase and
  file (no file for `null` `entryId`); "Finishing…" appears on the final
  verifying event without waiting for another event; a rejected build stops
  the progress and shows the error. Controls, including the picker, are disabled while building.
- The success view shows every summary field: file name, path, format, both
  sizes, SHA-256, the extraction command verbatim, and each normalised entry.
  Each Copy writes its exact text to the clipboard and announces "Copied".
- `ManifestChangedOnDisk` shows the banner, not a generic error.
  `SourceChanged` and `VerifyFailed` show their messages from the table above,
  via `errorMessage`; no component contains its own error copy or an
  "any other" branch.

## Tests proving completion

`npm run test`, with `lib` mocked:

- `BuildBar.test.tsx`: each disabled reason; the button text; Choose calling
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
    event per phase): the reset at the phase change, "Finishing…" on the final
    verifying event, and a rejection mid-verify stopping the progress;
  - `OpenerFailed` from Show in folder, as a banner.
- `BuildResult.test.tsx`: all summary fields, one size for `"tar"`, the
  extraction command verbatim, both Copy buttons (with
  `navigator.clipboard.writeText` stubbed), and the normalised-entries
  list (including a 0 count and its absence when empty).
- `ConfirmDialog.test.tsx`: focus starts on Cancel, Escape cancels, and focus
  returns afterwards.
- Axe checks on the bar in each state (including both progress phases), on the
  dialog, and on the success and error results.

## States covered

No manifest (picker and Choose disabled), disabled (each reason), ready, format
changed, format switched by file name, confirming, building (writing,
verifying, finishing), success (with and without normalised entries), and
error (each kind above).

## Out of scope

- Drops (U4).
- Keyboard shortcuts (U6).
- Compression levels or any format option beyond the four formats.
