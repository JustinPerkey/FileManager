# U5 — Output selection, build, overwrite confirmation, progress, and result

Status: awaiting approval
Project: tarpack   Depends on: U3 (landed), M6 (landed). May run alongside U4.

## Goal

Let the user choose where the tar goes and create it safely, with visible
progress and a result they can verify.

## Context

**Safety rules the UI must honour.**

- The tar is written atomically by Rust. An existing file is replaced only when
  the UI passes `overwrite: true`, and it may do so only after the user
  confirms in a dialog.
- If the manifest changed on disk since it was loaded, Rust refuses to build
  (`ManifestChangedOnDisk`). The UI must then show the same "The manifest
  changed on disk." warn banner with **Reload** that U2 shows for the watcher
  event.

**What exists.**

- `TarpackView.tsx` holds the `TarpackSession` state and has a reserved slot
  for the sticky bottom bar. `src/app/Banner.tsx` (U2) exists.
- `src/lib/tarpack.ts` (M6; read it for exact signatures) provides:
  - `setOutput(path)`, which returns a session
  - `build(overwrite)`, which returns a `BuildSummary` or throws a
    `TarpackError`
  - `onBuildProgress(handler)`, which yields `{ entryId, bytesDone, bytesTotal }`
  - `revealOutput()`
- `src/lib/tauri.ts` provides `saveFileDialog({ defaultPath })`.
- From the session:
  - `session.outputPath`
  - `session.canBuild`
  - `session.buildBlockedReason`: one of `"noManifest"`, `"manifestInvalid"`,
    `"entriesNotReady"`, `"noOutput"`, or `null`
  - `session.readyCount` and `session.totalCount`
  - `session.manifest.outputName`: the suggested file name
- `BuildSummary`: `{ path, entries, files, dirs, bytes, sha256Hex }`
- `TarpackError`: `{ kind, message, entryId? }`, where `kind` includes
  `"OutputExists"`, `"ManifestChangedOnDisk"`, `"SourceMissing"`, and
  `"SourceUnreadable"`
- The tokens you use are in `src/styles/tokens.css`: `--surface`, `--border`,
  `--accent`, `--accent-text`, `--ok`, `--danger`, `--text-muted`,
  `--font-mono`, and the `--space-*` scale.

### Components

| Component | Path | Props | States |
| --- | --- | --- | --- |
| `BuildBar` | `src/tools/tarpack/BuildBar.tsx` | `session, building, progress, onChooseOutput, onBuild` | disabled-with-reason / ready / building |
| `BuildResult` | `src/tools/tarpack/BuildResult.tsx` | `result: { ok: BuildSummary } \| { err: TarpackError }, onReveal, onDismiss` | success / error |
| `ConfirmDialog` | `src/app/ConfirmDialog.tsx` (shared) | `open, title, body, confirmLabel, onConfirm, onCancel` | open |

### Behaviour

- **`BuildBar`**, sticky at the bottom of the view:
  - on the left, "Output:", the path in `--font-mono` (middle-truncated) or
    "not chosen", and **Choose…**, which opens `saveFileDialog` with
    `defaultPath` set to the current output or `outputName`;
  - on the right, the primary **Create tar** button.
- **Disabled reason**, shown as visible text next to the button (not only a
  tooltip) and linked with `aria-describedby`:
  - `noManifest`: "Open a manifest first"
  - `manifestInvalid`: "Fix the manifest problems first"
  - `entriesNotReady`: "{totalCount − readyCount} files still need a location"
  - `noOutput`: "Choose where to save the tar"
- **Create tar**:
  1. Call `build(false)`.
  2. On an `OutputExists` error, open `ConfirmDialog` with the title
     "Replace {file name}?", the body "A file with this name already exists in
     {folder}. Replacing it can't be undone.", the confirm label "Replace",
     and initial focus on **Cancel**.
  3. On confirm, call `build(true)`.
- **Building.**
  - A determinate progress bar (`role="progressbar"`, `aria-valuenow`, and
    `aria-valuetext` like "Adding gateway.conf, 40%") computed from the
    progress events.
  - All tarpack controls (bar, table actions, header actions) are disabled.
  - Drops are ignored.
- **Success.** `BuildResult` shows "Created {file name}", then the path,
  "{files} files, {dirs} folders, {size}" (size in KB or MB), and the SHA-256
  in `--font-mono` with **Copy**. It offers **Show in folder**, which calls
  `revealOutput()`. It is dismissible.
- **Errors.**
  - `ManifestChangedOnDisk`: show the warn banner with Reload.
  - Any other error: an error `BuildResult` with the message, naming the entry's
    `source` when `entryId` is present.

**Rules that bind this task.**

- Tokens only.
- Everything is keyboard reachable and labelled.
- `ConfirmDialog` traps focus, closes on Escape (which counts as Cancel), and
  returns focus to **Create tar**.
- Progress animation respects `prefers-reduced-motion`.
- Call only `lib/` functions.

## Files

- `src/tools/tarpack/BuildBar.tsx`, `BuildResult.tsx`
- `src/app/ConfirmDialog.tsx`
- Wiring in `TarpackView.tsx`
- Tests next to each

## Skill

`impeccable:layout` and `impeccable:clarify`, then `impeccable:animate` for the
progress bar only.

## Acceptance criteria

- Each disabled reason shows its exact visible text, and the button is
  `disabled`.
- The overwrite flow is: `build(false)`, then `OutputExists`, then the dialog.
  Cancel makes no further call; Replace calls `build(true)`.
- Progress renders from events and reaches 100%, and controls are disabled
  while building.
- The success view shows every summary field. Copy writes the hash to the
  clipboard.
- `ManifestChangedOnDisk` shows the banner, not a generic error.

## Tests proving completion

`npm run test`, with `lib` mocked:

- `BuildBar.test.tsx`: each disabled reason, and Choose calling
  `saveFileDialog` then `setOutput`.
- `TarpackView.build.test.tsx`:
  - the happy path;
  - `OutputExists`, then Cancel;
  - `OutputExists`, then Replace;
  - `ManifestChangedOnDisk`;
  - `SourceMissing`, naming the file;
  - progress rendering.
- `ConfirmDialog.test.tsx`: focus starts on Cancel, Escape cancels, and focus
  returns afterwards.
- Axe checks on the bar in each state, and on the dialog.

## States covered

Disabled (each reason), ready, confirming, building, success, and error.

## Out of scope

- Drops (U4).
- Keyboard shortcuts (U6).
- Compression options.
