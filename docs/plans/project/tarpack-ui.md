# UI project plan: app shell and the Tar Packager view

Status: **awaiting approval.** Backend project plan: [`tarpack.md`](tarpack.md).
Updated 2026-09-29 for the human's answers to the backend open questions
(`tarpack.md` §6) and to this plan's own §5.

The `impeccable` skills were not available when this plan was written, so the
`shape` method was followed by hand. Each UI task plan names the skill its
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
- **Format-neutral wording.** The primary button reads **Create archive**, not
  "Create tar".
- **Absolute stored paths.** `targetPath` starts with `/`. The archive must be
  extracted with GNU tar `-P`; the build result shows the exact
  `extractCommand` with a copy action.
- **Line-ending conversion is visible.** Entries with `normalizeEol` carry a
  labelled "CRLF → LF" marker in the table, and the build result lists how
  many CRLF pairs each such entry had replaced.
- **Two-phase progress.** Writing, then verifying, each 0 → 100%, shown as
  distinct labelled steps.

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

The format picker, EOL marker, and extraction command need no new tokens: the
marker uses `--text-muted` on `--surface-sunken` with `--border`, a pair U1
already verifies for AA. Every text/background pair must reach WCAG AA. UI task
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
| `TarpackView` | `tools/tarpack/TarpackView.tsx` | — | loading / no-manifest / invalid / partial / ready / building (writing, verifying) / success / error | tool |
| `ManifestHeader` | `tools/tarpack/ManifestHeader.tsx` | `session, onOpen, onOpenRecent, onReload, onEdit` | loaded / changed-on-disk | tool |
| `ManifestErrors` | `tools/tarpack/ManifestErrors.tsx` | `errors, warnings` | errors / warnings only | tool |
| `EntryTable` | `tools/tarpack/EntryTable.tsx` | `entries, onBrowse, onClear` | empty / populated | tool |
| `EntryStatus` | `tools/tarpack/EntryStatus.tsx` | `status` | Ready / Missing / Not assigned | tool |
| `EolMarker` | `tools/tarpack/EolMarker.tsx` | — | shown only when `normalizeEol` | tool |
| `DropResult` | `tools/tarpack/DropResult.tsx` | `outcome, onDismiss` | matched / unmatched / ambiguous | tool |
| `BuildBar` | `tools/tarpack/BuildBar.tsx` | `session, building, progress, onChooseOutput, onFormatChange, onBuild` | disabled-with-reason / ready / building | tool |
| `FormatPicker` | `tools/tarpack/FormatPicker.tsx` | `formats, value, disabled, onChange` | enabled / disabled | tool |
| `BuildProgress` | `tools/tarpack/BuildProgress.tsx` | `progress, entries` | writing / verifying / finishing | tool |
| `BuildResult` | `tools/tarpack/BuildResult.tsx` | `result, entries, onReveal, onDismiss` | success / error | tool |

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
`<select>`) · disabled reason · **Create archive**. While building, the
progress region replaces the reason text.

### Error copy

User-facing messages per `TarpackError.kind`, with the backend `message` shown
as secondary detail:

| Kind | Message |
| --- | --- |
| `OutputExists` | (never shown as an error; opens the replace confirmation) |
| `ManifestChangedOnDisk` | the warn banner "The manifest changed on disk." with **Reload** |
| `SourceMissing` | "{source} is no longer at its assigned location." |
| `SourceUnreadable` | "{source} could not be read." |
| `SourceChanged` | "{source} changed while the archive was being written. Nothing was saved. Try again." |
| `VerifyFailed` | "The archive failed its check after writing, so it was not saved. Any existing file was left unchanged. Try again." |
| any other | "The archive could not be created." |

## 4. UI task index

Each UI task is specified in full in its task plan. The task plan is the only
plan its `ui-implementer` reads.

| # | Task plan | Goal | Depends on |
| --- | --- | --- | --- |
| U1 | [`U1-app-shell.md`](../tasks/tarpack/U1-app-shell.md) | Window layout, tokens, tool navigation | M1 |
| U2 | [`U2-manifest-header.md`](../tasks/tarpack/U2-manifest-header.md) | Manifest header; no-manifest, invalid, changed-on-disk states | U1, M6 |
| U3 | [`U3-entry-table.md`](../tasks/tarpack/U3-entry-table.md) | Entry table with per-row status and the CRLF → LF marker | U2, M6 |
| U4 | [`U4-drop-and-assign.md`](../tasks/tarpack/U4-drop-and-assign.md) | Drag-and-drop and per-row Browse/Clear | U3, M6 |
| U5 | [`U5-build-bar.md`](../tasks/tarpack/U5-build-bar.md) | Output, format picker, build, overwrite confirmation, two-phase progress, result with extraction command | U3, M6 |
| U6 | [`U6-shortcuts-polish.md`](../tasks/tarpack/U6-shortcuts-polish.md) | Keyboard shortcuts, polish, audit | U2–U5 |

U4 and U5 can run in parallel. M7 (packaging) depends on U6.

## 5. Open questions for the human

None open. Resolved 2026-09-29 (the human accepted the proposed defaults):

- **Theme:** follow the system only. No manual light/dark toggle is built; the
  `data-theme` hook in `tokens.css` stays for a possible later toggle.
- **Size and modified-time columns:** not shown. The entry table stays narrow.
