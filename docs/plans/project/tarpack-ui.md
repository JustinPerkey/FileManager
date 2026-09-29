# UI project plan: app shell and the Tar Packager view

Status: **awaiting approval.** Backend project plan: [`tarpack.md`](tarpack.md).
Updated 2026-09-29 for the human's answers to the backend open questions
(`tarpack.md` §6) and to this plan's own §5, and again for the partial-results
contract (`tarpack.md` §3.2.1, decisions 31–36, §6.3): see "Manifests with
errors" in §1.

The `impeccable` skills were not available when this plan was written, nor for
the partial-results revision, so the `shape` and `critique` methods were
followed by hand. Each UI task plan names the skill its
`ui-implementer` should run if it is available.

**Stack (decided).** Tauri v2 shell, React + TypeScript + Vite frontend in
`apps/desktop/src/`, design tokens in `apps/desktop/src/styles/tokens.css`. The
implementer owns `apps/desktop/src/lib/` (typed client and generated types);
the ui-implementer owns the rest of `apps/desktop/src/`. Every UI task depends
on the foundation milestone (M1); U2–U5 also depend on the Tauri contract (M6).

## 1. Design brief

**Problem.** Building a deployment archive by hand on Windows is error-prone.
The files come from scattered build folders, and the Linux permissions are
invisible until something fails on the target machine.

**User.** A developer or tester who rebuilds the same package many times a day
for a low-power armv7 Linux target. They know the file set. What they want is
to confirm that everything is present and correct, pick the archive format,
build, and copy the command that extracts it on the target.

**Register.** A quiet, dense, trustworthy utility, like a good build tool. It
shows everything that matters at a glance, and nothing decorative. Confidence
comes from explicit per-row status and from showing the exact Linux path, mode,
and line-ending treatment that will be written.

**Direction.**

- A single-window desktop layout:
  - a narrow **tool sidebar** (the home for future tools);
  - a **manifest header**;
  - the **error report**, only when the manifest has errors: a notice strip
    plus a collapsible list of what failed;
  - the **entry table** as the hero;
  - a sticky **build bar** at the bottom, holding output, format, and the
    build action.
- The whole window is a drop target.
- Status is always text plus an icon, never color alone.
- The theme follows the system only (decided, §5).

**Decisions carried from the backend contract.**

- **Four output formats**: `.tar`, `.tar.gz`, `.tar.zst`, `.tar.xz`. The user
  picks one in the build bar. The backend owns every extension and file-name
  rule: the UI renders `session.formats` in the order given, shows
  `session.format` as selected, and re-renders from the session returned by
  `setFormat` and `setOutput`. It never adds or strips an extension itself.
  `setOutput` may switch the format (a typed `.tar.xz` name selects xz) or
  append the extension; the UI announces a switch.
  The Save dialog opens at `outputPath`, else `suggestedOutputName`, with one
  filter built from the current option's `filterExtension` (`tar`, `gz`,
  `zst`, `xz`); the UI derives nothing from `extension`.
- **Copy actions** call `navigator.clipboard.writeText` directly; there is no
  clipboard wrapper or plugin.
- **Format-neutral wording.** The primary button reads **Create archive**, not
  "Create tar".
- **Absolute stored paths.** `targetPath` starts with `/`. The archive must be
  extracted with GNU tar `-P`; the build result shows the exact
  `extractCommand` with a copy action.
- **Line-ending conversion is visible.** Entries with `normalizeEol` carry a
  labelled "CRLF → LF" marker in the table, and the build result lists how
  many CRLF pairs each such entry had replaced.
- **Two-phase progress.** Writing, then verifying, each 0 → 100% over the same
  `bytesTotal`, shown as distinct labelled steps. Each phase ends with one
  event at 100% with no entry. The SHA-256 is computed during verifying, so the
  short "Finishing…" state after it covers only the final flush and rename.
- **No manifest.** `format` is `"tar"`, all four formats are listed, and the
  output path and suggested name are `null`. `setFormat` and `setOutput` would
  fail with `NoManifest`, so the Format picker and **Choose…** stay disabled.
- **Verify failure is safe.** On `VerifyFailed` nothing is saved and an
  existing output file is left byte-for-byte unchanged; the copy says so.

**Manifests with errors (partial results).** The human's requirement: *"Show
the ones that passed but collect the ones that failed in some sort of report.
Also notify the user that there were errors."* A readable manifest always
opens. `manifest.entries` holds only the passed entries,
`manifest.failedEntries` one `EntryFailure { index, id|null, source|null,
line, errors }` per failed `[[file]]` table, `manifest.errors` only the
manifest-level errors, `manifest.entriesWithheld` says a manifest-level error
hid every entry, and `manifest.errorCount > 0` is the single "has errors"
flag. Build is blocked (`manifestInvalid`) while it is above 0; assign, clear,
drop, format, and output keep working on passed entries.

Critique of the previous design against this contract (by hand): the invalid
state was all-or-nothing and keyed on `errors.length`, which misses every
entry error; warnings were hidden whenever errors existed; the summary line
could say "All 4 files ready" while 2 files had failed; "This manifest lists no
files." was wrong for withheld entries; drops were disabled; the build bar's
reason gave no count and no route to the errors. The design below fixes each.

- **Where the report lives: its own region**, between the banners and the
  entry table, owned by `ManifestErrors` (U2). Not in the header, which stays
  a stable one-line identity bar, and not inside the table, whose rows mean
  "will be in the archive". Failed entries never appear as table rows.
- **The notification is the report's notice strip, plus a live-region
  announcement.** No toast: it would vanish, and this state persists until the
  user fixes the file.
  - The strip is always visible while `errorCount > 0`: `--surface`, 1 px
    `--border`, a 3 px `--danger` inline-start edge and an `aria-hidden` error
    icon, heading "N errors in this manifest", one consequence line ("2 files
    are left out of the list until they're fixed." / "No files can be listed
    until the errors under Whole manifest are fixed."), then "The archive
    can't be created until every error is fixed." Actions: **Show errors** /
    **Hide errors** (`aria-expanded`, `aria-controls`) and **Edit in editor**.
  - An always-mounted, visually hidden `role="status"` announcer in
    `TarpackView` announces "{name} has N errors. K files left out." after
    every open, reload, and first-session restore, repeated even when the text
    is unchanged, and "{name} reloaded. No errors." when a reload clears them.
    Nothing is announced on assign, clear, drop, or format changes.
  - Focus is never moved automatically. `showErrors()` in `TarpackView`
    expands the report and moves focus to it; it backs the strip's **Show
    errors**, the build bar's **Show errors**, and F8.
- **Report body.** A focusable `role="region"` (`id="manifest-error-report"`),
  capped at `40vh` with internal scroll so a manifest with many failures never
  buries the table. A "Whole manifest" group first (manifest-level errors),
  then one group per failed entry in manifest order, headed by the `id` in
  mono, or "Entry #{index}", then muted "source {source}" (when present) and
  "line {line}". Every error is listed: visible `line:col` in mono muted
  (`aria-hidden`), visually hidden "Line L, column C:", then the message
  verbatim with its prefix. Items are not tab stops; screen-reader users move
  by heading and list. The body re-expands after every open, reload, or
  restore that has errors.
- **Warnings** show in their collapsed `<details>` whether or not there are
  errors.
- **Table (U3).** Summary line never says "All" while `failedEntries` is
  non-empty, and appends " · N files left out (errors)". An empty `entries`
  shows one sentence instead of the table: withheld, all failed, or no files.
- **Drops and assignment (U4).** Enabled while errors exist whenever there
  are passed entries. Disabled with a stated reason when `entries` is empty.
  With failed entries, unmatched files read "N not matched" plus the hint
  "Files for entries with errors can't be matched until those errors are
  fixed." (no TS matching against failed sources).
- **Build bar (U5).** `manifestInvalid` reads "Fix N manifest errors first"
  followed by a secondary **Show errors** button; Choose… and Format stay
  enabled. It shows whenever the session says so, including when every listed
  entry is ready.
- **Shortcut (U6).** F8, the usual "next error" key on Windows developer
  tools, runs `showErrors()` when `errorCount > 0`.
- **Wording.** "error" and "warning", never "problem".

**Platform constraints from M1** (factual notes added by the planner after
the M1 review; they change no design decision):

- **CSP.** The webview Content Security Policy allows inline `style`
  attributes and `<style>`. It allows no inline scripts and nothing remote:
  no CDN fonts, images, or scripts. Fonts and assets ship with the frontend,
  and images may also be `data:` URIs.
- **Accessibility lint.** `npm run lint` already includes
  `eslint-plugin-jsx-a11y` (recommended rules), and fails on any finding. UI
  tasks do not add it, and do not disable its rules.
- **Test cleanup.** `src/test-setup.ts` already registers Testing Library's
  `afterEach(cleanup)`.

## 2. Design tokens touched (new)

All tokens are defined in `apps/desktop/src/styles/tokens.css`. Dark mode is
defined under `prefers-color-scheme: dark`, and also under `[data-theme=dark]`.
The `data-theme` hook stays in place but no toggle is built (§5).

| Token | Light | Dark |
| --- | --- | --- |
| `--bg` | `#f7f7f8` | `#16171a` |
| `--surface` | `#ffffff` | `#1e2024` |
| `--surface-sunken` | `#eef0f2` | `#121316` |
| `--border` | `#d9dce1` | `#33363d` |
| `--text` | `#1b1d21` | `#e8e9ec` |
| `--text-muted` | `#5b616b` | `#a3a8b1` |
| `--accent` | `#2457c5` | `#7aa2ff` |
| `--accent-text` | `#ffffff` | `#0d1530` |
| `--ok` | `#1d7a46` | `#5fd08f` |
| `--warn` | `#8a5a00` | `#f2c14e` |
| `--danger` | `#b3261e` | `#ff8a80` |
| `--focus-ring` | `#2457c5` | `#7aa2ff` |
| `--drop-overlay` | `rgb(36 87 197 / 0.08)` | `rgb(122 162 255 / 0.12)` |

Other tokens:

- **Spacing:** `--space-1…6` = 4, 8, 12, 16, 24, 32 px.
- **Radius:** `--radius` = 6 px.
- **Fonts:** `--font-ui` is the system UI stack (Segoe UI on Windows).
  `--font-mono` is Cascadia Mono, then Consolas, for paths, modes, hashes, and
  the extraction command.

The format picker, EOL marker, extraction command, and error report need no
new tokens: the marker uses `--text-muted` on `--surface-sunken` with
`--border`, a pair U1 already verifies for AA; the error notice strip uses
`--text` and `--text-muted` on `--surface`, with `--danger` only for its edge
and icon (non-text, at least 3:1 against `--surface` in both themes: about
6.5:1 light, 7:1 dark). Every text/background pair must reach WCAG AA. UI task
1 verifies this.

## 3. Component inventory

All components live under `apps/desktop/src/`.

| Component | Path | Props | States | Scope |
| --- | --- | --- | --- | --- |
| `AppShell` | `app/AppShell.tsx` | `tools` | — | shared |
| `ToolNav` | `app/ToolNav.tsx` | `tools, activeId, onSelect` | — | shared |
| `DropZone` | `app/DropZone.tsx` | `enabled, disabledReason, label, onDrop(paths)` | idle / hover / disabled | shared |
| `Banner` | `app/Banner.tsx` | `tone, message, action?` | info / warn / error | shared |
| `ConfirmDialog` | `app/ConfirmDialog.tsx` | `open, title, body, confirmLabel, onConfirm, onCancel` | open | shared |
| `TarpackView` | `tools/tarpack/TarpackView.tsx` | — | loading / no-manifest / errors (entries shown) / errors (entries withheld) / partial / ready / building (writing, verifying) / success / error | tool |
| `ManifestHeader` | `tools/tarpack/ManifestHeader.tsx` | `session, onOpen, onOpenRecent, onReload, onEdit` | loaded / changed-on-disk | tool |
| `ManifestErrors` | `tools/tarpack/ManifestErrors.tsx` | `errors, failedEntries, entriesWithheld, errorCount, warnings, expanded, onExpandedChange, onEdit, reportRef` | hidden / warnings only / errors expanded / errors collapsed / entries withheld | tool |
| `EntryTable` | `tools/tarpack/EntryTable.tsx` | `entries, failedCount, entriesWithheld, onBrowse, onClear` | populated / populated with files left out / no files / all failed / withheld | tool |
| `EntryStatus` | `tools/tarpack/EntryStatus.tsx` | `status` | Ready / Missing / Not assigned | tool |
| `EolMarker` | `tools/tarpack/EolMarker.tsx` | — | shown only when `normalizeEol` | tool |
| `DropResult` | `tools/tarpack/DropResult.tsx` | `outcome, onDismiss` | matched / unmatched / ambiguous | tool |
| `BuildBar` | `tools/tarpack/BuildBar.tsx` | `session, building, progress, onChooseOutput, onFormatChange, onBuild, onShowErrors` | disabled-with-reason / manifest errors / ready / building | tool |
| `FormatPicker` | `tools/tarpack/FormatPicker.tsx` | `formats, value, disabled, onChange` | enabled / disabled | tool |
| `BuildProgress` | `tools/tarpack/BuildProgress.tsx` | `progress, entries` | writing / verifying / finishing | tool |
| `BuildResult` | `tools/tarpack/BuildResult.tsx` | `result, entries, onReveal, onDismiss` | success / error | tool |
| `errorMessages` | `tools/tarpack/errorMessages.ts` | `errorMessage(error, entries)` (module, not a component) | one entry per `TarpackErrorKind` | tool |

### Entry table columns

1. **Status**: icon and word.
2. **File**: the expected `source` name, followed by the **CRLF → LF** marker
   when `normalizeEol` is true.
3. **Windows location**: monospace, truncated in the middle, with the full path
   on hover and focus.
4. **Linux target**: monospace, the absolute stored name, for example
   `/opt/gateway/bin/gateway`.
5. **Mode**: `rwxr-xr-x` followed by the octal `0755`, muted.
6. **Owner**: `root:root`.
7. **Actions**: *Browse…* and *Clear*.

No size or modified-time columns (decided, §5).

### Build bar layout

Left to right, wrapping onto two lines at narrow widths or 200% text:
**Output:** path (or "not chosen") and **Choose…** · **Format** picker (native
`<select>`) · disabled reason (for `manifestInvalid`: "Fix N manifest errors
first" and **Show errors**) · **Create archive**. While building, the
progress region replaces the reason text.

### Error copy

`TarpackError` is `{ kind: TarpackErrorKind, message: string, entryId?: string }`.
`TarpackErrorKind` is a closed string-literal union of 17 kinds. The UI maps
every kind to user-facing copy in one module,
`tools/tarpack/errorMessages.ts`, typed as
`Record<TarpackErrorKind, (source: string | null) => string>`, so a kind added
or removed in Rust fails `npm run typecheck` until the copy changes too. There
is no "any other" fallback. `{source}` is the `source` of the entry named by
`entryId`; when `entryId` is omitted or matches no entry, it reads "A file".
The backend `message` is shown under the copy in a collapsed "Details"
disclosure.

U2 creates the module with all 17 entries; U4 and U5 only consume it. Two kinds
are intercepted before the map in the build flow (U5): `OutputExists` opens the
replace confirmation, and `ManifestChangedOnDisk` shows the warn banner "The
manifest changed on disk." with **Reload**.

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

## 4. UI task index

Each UI task is specified in full in its task plan. The task plan is the only
plan its `ui-implementer` reads.

| # | Task plan | Goal | Depends on |
| --- | --- | --- | --- |
| U1 | [`U1-app-shell.md`](../tasks/tarpack/U1-app-shell.md) | Window layout, tokens, tool navigation | M1 |
| U2 | [`U2-manifest-header.md`](../tasks/tarpack/U2-manifest-header.md) | Manifest header; no-manifest and changed-on-disk states; error report, notice strip, and announcement | U1, M6 |
| U3 | [`U3-entry-table.md`](../tasks/tarpack/U3-entry-table.md) | Entry table of passed entries with per-row status, the CRLF → LF marker, and honest summary and empty states | U2, M6 |
| U4 | [`U4-drop-and-assign.md`](../tasks/tarpack/U4-drop-and-assign.md) | Drag-and-drop and per-row Browse/Clear | U3, M6 |
| U5 | [`U5-build-bar.md`](../tasks/tarpack/U5-build-bar.md) | Output, format picker, build, overwrite confirmation, two-phase progress, result with extraction command | U3, M6 |
| U6 | [`U6-shortcuts-polish.md`](../tasks/tarpack/U6-shortcuts-polish.md) | Keyboard shortcuts, polish, audit | U2–U5 |

U4 and U5 can run in parallel. M7 (packaging) depends on U6.

## 5. Open questions for the human

None open. Resolved 2026-09-29 (the human accepted the proposed defaults):

- **Theme:** follow the system only. No manual light/dark toggle is built; the
  `data-theme` hook in `tokens.css` stays for a possible later toggle.
- **Size and modified-time columns:** not shown. The entry table stays narrow.
