# U2 — Manifest header, empty state, invalid state, changed-on-disk banner

Status: awaiting approval
Project: tarpack   Depends on: U1 (landed), M6 (landed)

## Goal

Let the user open, reload, and edit a manifest, and see clearly when none is
loaded, when it is invalid, or when it changed on disk.

## Context

**The Tar Packager.** It builds a Linux archive (`.tar`, `.tar.gz`, `.tar.zst`,
or `.tar.xz`; the format is picked in the build bar, not in this task) from
Windows files. A
**manifest** (a TOML file the user edits in their own editor) lists the files,
their Linux target paths, and their permissions. The format is documented in
`docs/tarpack-manifest.md`; read it for the vocabulary. The user rebuilds the
same package many times a day, so they want confidence at a glance.

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
  `Diagnostic`, and `TarpackError`. The parts this task reads:
  - `session.manifest`: `null`, or `{ path, name, outputName, hash, entries,
    errors: Diagnostic[], warnings: Diagnostic[] }`. When the manifest fails
    validation, `manifest` is still present with its `path`, its `errors`, and
    whatever entries were readable.
  - `Diagnostic`: `{ severity, line, col, entryId: string | null, message }`.
    The `message` is final user-facing text; render it verbatim.
  - `session.stateWarning`: `string | null`.
  - `TarpackError`: `{ kind: TarpackErrorKind, message: string, entryId?: string }`.
    `entryId` is omitted (not `null`) when absent. `message` is technical
    detail, not user copy.
  - `TarpackErrorKind`: a closed string-literal union of exactly 17 kinds,
    listed in the error table below.

**The single source of truth.** Every command returns a full `TarpackSession`,
and the view renders it. Keep it in one piece of React state in
`TarpackView`, replaced wholesale on each command result. Never derive a second
copy.

**The view's full structure.** Later tasks fill in the rest:

- the header (this task);
- an errors or warnings region (this task);
- the entry table (U3);
- a sticky build bar at the bottom (U5).

Reserve those regions, and leave the table and bar as empty slots.

### Components

| Component | Path | Props | States |
| --- | --- | --- | --- |
| `TarpackView` | `src/tools/tarpack/TarpackView.tsx` | — | loading / no-manifest / invalid / loaded / changed-on-disk |
| `ManifestHeader` | `src/tools/tarpack/ManifestHeader.tsx` | `session, onOpen, onOpenRecent, onReload, onEdit` | loaded |
| `ManifestErrors` | `src/tools/tarpack/ManifestErrors.tsx` | `errors, warnings` | errors / warnings only |
| `errorMessages` | `src/tools/tarpack/errorMessages.ts` (module) | exports `errorMessage(error: TarpackError, entries): string` | one entry per kind |
| `Banner` | `src/app/Banner.tsx` (shared) | `tone: "info"\|"warn"\|"error", message, action?: { label, onAction }` | — |

### Behaviour

- **Header:**
  - shows the manifest `name` as the view heading;
  - shows the manifest path below it in `--font-mono`, `--text-muted`, and
    middle-truncated with the full path in `title`;
  - has these actions: **Open…**, **Recent ▾** (a menu from
    `recentManifests()`; hidden when empty), **Reload**, and **Edit in
    editor**.
- **No-manifest state** (`session.manifest === null`): one sentence, "Open a
  manifest to list the files this package needs.", and two buttons:
  - **Open manifest…**: an open dialog filtered to `*.toml`;
  - **Create from example…**: a save dialog, then `createManifestFromExample`.
- **Invalid state** (`manifest.errors.length > 0`):
  - an error-tone region headed "This manifest has N problems";
  - a list of `line:col — message` items, with the entry id in `--font-mono`
    when present;
  - the list is focusable and each item is readable by a screen reader;
  - Reload and Edit remain available.
- **Warnings**, when there are no errors, go in a collapsed `<details>` headed
  "N warnings", using the same `line:col — message` item format. The backend
  emits three kinds; show each `message` exactly as given, never rewording or
  truncating it:
  - two entries share a `source` name (a drop cannot tell them apart);
  - a stored path is **100 bytes or longer**: the message names the entry, says
    "100 bytes or longer", and gives the byte count of the stored path
    including its leading `/` (for example 112 bytes). The item must wrap, not
    truncate, so the byte count stays visible;
  - the manifest lists no files.
- **Changed on disk.** An `onManifestChanged` event shows a warn `Banner`:
  "The manifest changed on disk.", with the action **Reload**. It clears after
  a reload.
- **Command errors** (`TarpackError`) show as an error `Banner` whose message
  is `errorMessage(error, entries)`, with the backend `message` under it in a
  collapsed "Details" disclosure. In this task's flows:
  - **Open…** or a Recent item, and **Reload**, can fail with
    `ManifestUnreadable` (file missing or not permitted). Show its message;
    for a Recent item, keep the current session unchanged.
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
placeholder then reads "A file".

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

- **Loading:** while the first `session()` call is pending, show a quiet
  skeleton. It must not flash for fast loads; delay it by about 150 ms.
- `session.stateWarning`, when set, shows an info `Banner` (for example that
  saved locations were reset).

**Rules that bind this task.**

- Tokens only, no hex.
- Everything is keyboard reachable and labelled.
- Status is never conveyed by color alone.
- Plain, short copy.
- Call only `lib/` functions. Never touch the filesystem or invoke Tauri
  directly.

## Files

- `src/tools/tarpack/TarpackView.tsx`, `ManifestHeader.tsx`,
  `ManifestErrors.tsx`
- `src/app/Banner.tsx`
- Tests next to each

## Skill

`impeccable:layout`, then `impeccable:clarify` for all copy.

## Acceptance criteria

- Every state above renders from a `TarpackSession` fixture.
- **Open…** calls `openFileDialog` with a `.toml` filter, then `openManifest`.
  A cancelled dialog does nothing.
- The banner appears on a `manifest-changed` event, and Reload clears it.
- The errors list announces its count, and each item is reachable.
- `errorMessages.ts` is a `Record` over `TarpackErrorKind` with no fallback
  branch; removing any key fails `npm run typecheck`.
- `ManifestUnreadable` on Open or Reload, and `PathExists` on Create from
  example, show their messages in an error banner with a Details disclosure
  that is keyboard operable; the prior state stays usable.
- Warning messages render verbatim; a long-name warning's byte count is
  visible at 800 px and at 200% text size.
- The header wraps cleanly at 800 px wide and at 200% text size.

## Tests proving completion

`npm run test`, with `src/lib/tarpack` and `src/lib/tauri` mocked:

- `TarpackView.states.test.tsx`: loading, no-manifest, invalid, loaded, and
  changed-on-disk; `ManifestUnreadable` from Open and Reload; `PathExists`
  from Create from example.
- `ManifestHeader.test.tsx`: each action calls the right `lib` function, and
  Recent is hidden when empty.
- `errorMessages.test.ts`: a non-empty message for each of the 17 kinds
  (listed explicitly in the test), `{source}` filled from `entryId`, and
  "A file" when `entryId` is omitted or unknown. `npm run typecheck` proves
  exhaustiveness.
- `ManifestErrors.test.tsx`: the count heading, the item text, and the
  collapsed warnings, including a long-name warning whose message (with its
  byte count) renders verbatim.
- Axe checks on each state.

## States covered

Loading, no-manifest, invalid, loaded, changed-on-disk, and command error
(`ManifestUnreadable`, `PathExists`, `OpenerFailed`, `Io`).

## Out of scope

- The entry table (U3).
- Drops (U4).
- The build bar (U5).
- Keyboard shortcuts (U6).
