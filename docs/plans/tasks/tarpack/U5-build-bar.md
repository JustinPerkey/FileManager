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
yourself. The only string you derive is the Save-dialog filter suffix (see
Behaviour), because the dialog plugin needs it without the leading dot.

**Safety rules the UI must honour.**

- The archive is written atomically by Rust and verified before it replaces
  anything. An existing file is replaced only when the UI passes
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
  - `build(overwrite)`, which returns a `BuildSummary` or throws a
    `TarpackError`;
  - `onBuildProgress(handler)`, which yields
    `{ phase: "writing" | "verifying", entryId: string | null, bytesDone, bytesTotal }`.
    Bytes are uncompressed tar-stream bytes. The `verifying` phase follows
    `writing` and runs its own 0-to-`bytesTotal` pass;
  - `revealOutput()`.
- `src/lib/tauri.ts` provides `saveFileDialog({ defaultPath, filters? })`,
  where `filters` is `{ name: string, extensions: string[] }[]`. Extensions
  are given without the leading dot, and Windows matches only the last suffix
  (for example `zst`), so the backend's `setOutput` normalisation is what
  guarantees the full extension.
- From the session:
  - `session.manifest`: `null` or the loaded manifest; `entries[]` carry
    `{ id, source, ... }` for mapping an `entryId` to a file name;
  - `session.outputPath`: `string | null`, always ending with the current
    format's extension;
  - `session.format`: `ArchiveFormat`;
  - `session.formats`: `{ format: ArchiveFormat, extension: string }[]`, in
    the fixed order tar, tarGz, tarZst, tarXz;
  - `session.suggestedOutputName`: `string | null`, the file name for the
    Save dialog with the current extension; `null` when no manifest is loaded;
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
- `TarpackError`: `{ kind, message, entryId? }`, where `kind` includes
  `"OutputExists"`, `"ManifestChangedOnDisk"`, `"SourceMissing"`,
  `"SourceUnreadable"`, `"SourceChanged"`, and `"VerifyFailed"`. `message` is
  the backend's technical detail.
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
  - Disabled when no manifest is loaded and while building.
- **Choose…** opens `saveFileDialog` with:
  - `defaultPath`: `session.outputPath`, else `session.suggestedOutputName`;
  - `filters`: one filter for the current format,
    `{ name: "{label} archive ({extension})", extensions: [<the text after the
    last "." of the current extension>] }`, for example
    `{ name: "zstd archive (.tar.zst)", extensions: ["zst"] }`.

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
    `source`; with a `null` `entryId`, "Writing, 40%".
  - Announce the phase change once in a polite live region ("Verifying the
    archive"); do not announce every percentage.
  - After verifying reaches 100% and until `build` resolves, show
    "Finishing…" with the bar full.
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
- **Errors.**
  - `ManifestChangedOnDisk`: show the warn banner with Reload, not a
    `BuildResult`.
  - Every other error: an error `BuildResult` with a user-facing message, and
    the backend `message` below it in a collapsed "Details" disclosure.
    `{source}` is the entry's `source` for `entryId`:

    | Kind | Message |
    | --- | --- |
    | `SourceMissing` | "{source} is no longer at its assigned location." |
    | `SourceUnreadable` | "{source} could not be read." |
    | `SourceChanged` | "{source} changed while the archive was being written. Nothing was saved. Try again." |
    | `VerifyFailed` | "The archive failed its check after writing, so it was not saved. Any existing file was left unchanged. Try again." |
    | any other | "The archive could not be created." |

    When `entryId` is absent for a kind that names a source, use "A file"
    instead of `{source}`.

**Rules that bind this task.**

- Tokens only.
- Everything is keyboard reachable and labelled: Choose…, the Format picker,
  Create archive, both Copy buttons, Show in folder, the Details disclosure,
  and Dismiss are real, focusable controls with visible focus.
- `ConfirmDialog` traps focus, closes on Escape (which counts as Cancel), and
  returns focus to **Create archive**.
- Progress animation respects `prefers-reduced-motion`.
- Call only `lib/` functions. No extension, file-name, or command logic in TS
  beyond the filter suffix above.

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
  `suggestedOutputName` and a single filter for the current format, then
  `setOutput`. A cancelled dialog makes no call.
- When `setOutput` returns a different format, the picker shows it and the
  live region announces the switch.
- The overwrite flow is: `build(false)`, then `OutputExists`, then the dialog.
  Cancel makes no further call; Replace calls `build(true)`.
- Progress shows "Step 1 of 2 · Writing" then "Step 2 of 2 · Verifying", each
  running 0 → 100% from events, with `aria-valuetext` naming the phase; then
  "Finishing…". Controls, including the picker, are disabled while building.
- The success view shows every summary field: file name, path, format, both
  sizes, SHA-256, the extraction command verbatim, and each normalised entry.
  Each Copy writes its exact text to the clipboard and announces "Copied".
- `ManifestChangedOnDisk` shows the banner, not a generic error.
  `SourceChanged` and `VerifyFailed` show their messages from the table above.

## Tests proving completion

`npm run test`, with `lib` mocked:

- `BuildBar.test.tsx`: each disabled reason; the button text; Choose calling
  `saveFileDialog` with the right `defaultPath` and filter, then `setOutput`;
  cancelled Choose.
- `FormatPicker.test.tsx`: options and order from `formats`, the selected
  value, `onChange` on keyboard selection, and the disabled state.
- `TarpackView.build.test.tsx`:
  - the happy path;
  - `setFormat` re-rendering the output path from the returned session;
  - `setOutput` returning a switched format, and the announcement;
  - `OutputExists`, then Cancel;
  - `OutputExists`, then Replace;
  - `ManifestChangedOnDisk`;
  - `SourceMissing`, naming the file;
  - `SourceChanged` and `VerifyFailed`, with their messages and details;
  - progress rendering across both phases, including the reset at the phase
    change and "Finishing…".
- `BuildResult.test.tsx`: all summary fields, one size for `"tar"`, the
  extraction command verbatim, both Copy buttons, and the normalised-entries
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
