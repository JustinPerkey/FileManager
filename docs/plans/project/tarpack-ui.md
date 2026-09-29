# UI project plan: app shell and the Tar Packager view

Status: **awaiting approval.** Backend project plan: [`tarpack.md`](tarpack.md).

The `impeccable` skills were not available when this plan was written, so the
`shape` method was followed by hand. Each UI task plan names the skill its
`ui-implementer` should run if it is available.

This plan assumes the frontend stack proposed in `tarpack.md` §2 (React +
TypeScript in `apps/desktop/src/`, tokens in `styles/tokens.css`), and it
depends on the foundation milestone (M1).

## 1. Design brief

**Problem.** Building a deployment tarball by hand on Windows is error-prone.
The files come from scattered build folders, and the Linux permissions are
invisible until something fails on the target machine.

**User.** A developer or tester who rebuilds the same package many times a day.
They know the file set. What they want is to confirm that everything is present
and correct, then build.

**Register.** A quiet, dense, trustworthy utility, like a good build tool. It
shows everything that matters at a glance, and nothing decorative. Confidence
comes from explicit per-row status and from showing the exact Linux path and
mode that will be written.

**Direction.**

- A single-window desktop layout:
  - a narrow **tool sidebar** (the home for future tools);
  - a **manifest header**;
  - the **entry table** as the hero;
  - a sticky **build bar** at the bottom.
- The whole window is a drop target.
- Status is always text plus an icon, never color alone.

## 2. Design tokens touched (new)

All tokens are defined in `apps/desktop/src/styles/tokens.css`. Dark mode is
defined under `prefers-color-scheme: dark`, and also under `[data-theme=dark]`
for a later manual toggle.

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
  `--font-mono` is Cascadia Mono, then Consolas, for paths and modes.

Every text/background pair must reach WCAG AA. UI task 1 verifies this.

## 3. Component inventory

All components live under `apps/desktop/src/`.

| Component | Path | Props | States | Scope |
| --- | --- | --- | --- | --- |
| `AppShell` | `app/AppShell.tsx` | `tools` | — | shared |
| `ToolNav` | `app/ToolNav.tsx` | `tools, activeId, onSelect` | — | shared |
| `DropZone` | `app/DropZone.tsx` | `onDrop(paths)`, `label` | idle / hover / disabled | shared |
| `Banner` | `app/Banner.tsx` | `tone, message, action?` | info / warn / error | shared |
| `ConfirmDialog` | `app/ConfirmDialog.tsx` | `title, body, confirmLabel, onConfirm, onCancel` | open | shared |
| `TarpackView` | `tools/tarpack/TarpackView.tsx` | — | loading / no-manifest / invalid / partial / ready / building / success / error | tool |
| `ManifestHeader` | `tools/tarpack/ManifestHeader.tsx` | `session` | loaded / changed-on-disk | tool |
| `ManifestErrors` | `tools/tarpack/ManifestErrors.tsx` | `diagnostics` | errors / warnings only | tool |
| `EntryTable` | `tools/tarpack/EntryTable.tsx` | `entries, onBrowse, onClear` | empty / populated | tool |
| `EntryStatus` | `tools/tarpack/EntryStatus.tsx` | `status` | Ready / Missing / Not assigned | tool |
| `DropResult` | `tools/tarpack/DropResult.tsx` | `outcome` | matched / unmatched / ambiguous | tool |
| `BuildBar` | `tools/tarpack/BuildBar.tsx` | `session, onChooseOutput, onBuild` | disabled-with-reason / ready / building | tool |
| `BuildResult` | `tools/tarpack/BuildResult.tsx` | `summary` | success / error | tool |

### Entry table columns

1. **Status**: icon and word.
2. **File**: the expected `source` name.
3. **Windows location**: monospace, truncated in the middle, with the full path
   on hover and focus.
4. **Linux target**: monospace; for example `/opt/gateway/bin/gateway`.
5. **Mode**: `rwxr-xr-x` followed by the octal `0755`, muted.
6. **Owner**: `root:root`.
7. **Actions**: *Browse…* and *Clear*.

## 4. UI task index

Each UI task is specified in full in its task plan. The task plan is the only
plan its `ui-implementer` reads.

| # | Task plan | Goal | Depends on |
| --- | --- | --- | --- |
| U1 | [`U1-app-shell.md`](../tasks/tarpack/U1-app-shell.md) | Window layout, tokens, tool navigation | M1 |
| U2 | [`U2-manifest-header.md`](../tasks/tarpack/U2-manifest-header.md) | Manifest header; no-manifest, invalid, changed-on-disk states | U1, M6 |
| U3 | [`U3-entry-table.md`](../tasks/tarpack/U3-entry-table.md) | Entry table with per-row status | U2, M6 |
| U4 | [`U4-drop-and-assign.md`](../tasks/tarpack/U4-drop-and-assign.md) | Drag-and-drop and per-row Browse/Clear | U3, M6 |
| U5 | [`U5-build-bar.md`](../tasks/tarpack/U5-build-bar.md) | Output, build, overwrite confirmation, progress, result | U3, M6 |
| U6 | [`U6-shortcuts-polish.md`](../tasks/tarpack/U6-shortcuts-polish.md) | Keyboard shortcuts, polish, audit | U2–U5 |

U4 and U5 can run in parallel.

## 5. Open questions for the human

- Should the shell have a manual light/dark toggle, or follow the system only?
  The proposal is system only; the `data-theme` hook is ready for a later
  toggle.
- Should the entry table show file size and modified time for each assigned
  file? The proposal is no, to keep the table narrow. Q6 in the backend plan may
  change this.
