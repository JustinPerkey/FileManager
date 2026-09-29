# UI plan: app shell and the Tar Packager view

Status: **awaiting approval.** Backend plan: [`tarpack.md`](tarpack.md).

The `impeccable` skills were not available when this plan was written, so the
`shape` method was followed by hand. The `ui-implementer` should still run each
task's named skill if it is available.

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

## 4. Ordered UI tasks

### UI task 1 — App shell, tokens, and tool navigation

**Goal.** Establish the window layout, theme tokens, and a sidebar that lists
tools from `tools/registry.ts`.
**Depends on.** Backend M1.
**Files.**

- `styles/tokens.css`, `styles/base.css`
- `app/AppShell.tsx`, `app/ToolNav.tsx`
- `App.tsx`
- `tools/registry.ts` (the ui-implementer adds the tarpack entry and its view
  placeholder)

**Skill.** `impeccable:impeccable` (craft), then `impeccable:colorize`.
**Acceptance criteria.**

- Light and dark both render from tokens, with no hex values in components.
- Token contrast pairs pass AA.
- The sidebar is a `nav` landmark with an `aria-current` item and is operable
  by arrow keys.
- The layout holds at the 800×560 minimum without horizontal scroll.

**Tests proving completion.** The vitest render test for `AppShell`, and an
`axe` check with no violations.
**States covered.** The single tool is selected.

### UI task 2 — Manifest header, empty state, and invalid-manifest state

**Goal.** Let the user open, reload, and edit a manifest, and see clearly when
none is loaded or when it is invalid.
**Depends on.** UI task 1 and backend M6.
**Files.** `tools/tarpack/{TarpackView, ManifestHeader, ManifestErrors}.tsx`,
`app/Banner.tsx`
**Skill.** `impeccable:layout`, then `impeccable:clarify`.
**Acceptance criteria.**

- The header shows the manifest `name`, its path in monospace, and these
  actions: *Open…*, *Recent ▾*, *Reload*, *Edit in editor*.
- **No-manifest state:** one short sentence and two buttons, *Open manifest…*
  and *Create from example…*.
- **Invalid state:** lists each error as `line:col` plus the message plus the
  entry id. The errors are focusable as a list, and the build bar is disabled
  with the reason "Manifest has N errors".
- **Warnings** show in a collapsed disclosure.
- **Changed on disk:** a `manifest-changed` event shows a warn `Banner` with a
  *Reload* action. Reloading keeps the assignments by id.

**Tests proving completion.** vitest, with `lib/tarpack` mocked, for each
state.
**States covered.** Loading, no-manifest, invalid, loaded, changed-on-disk.

### UI task 3 — Entry table with per-row status

**Goal.** Show every manifest entry with its source, its target, its
permissions, and whether it is ready.
**Depends on.** UI task 2 and backend M6.
**Files.** `tools/tarpack/{EntryTable, EntryStatus}.tsx`
**Skill.** `impeccable:typeset`, then `impeccable:layout`.
**Acceptance criteria.**

- It is a semantic `table` with column headers.
- The status is an icon plus a word (Ready / Missing / Not assigned). The
  tokens used are `--ok`, `--danger`, and `--text-muted`.
- A long path is truncated in the middle, with the full path available to
  screen readers and on focus.
- A summary line reads "5 of 6 files ready".
- The table stays usable with 200 entries: no layout shift, and a sticky
  header.

**Tests proving completion.**

- A vitest render with a mixed-status fixture.
- A test that confirms each status's text label is present.
- An `axe` check.

**States covered.** Empty manifest (no `[[file]]`), partial, all ready.

### UI task 4 — Drag-and-drop and per-row assignment

**Goal.** Assign Windows files by dropping files or folders anywhere, or by
picking them per row.
**Depends on.** UI task 3 and backend M6.
**Files.**

- `app/DropZone.tsx`
- `tools/tarpack/DropResult.tsx`
- Edits to `EntryTable.tsx` and `TarpackView.tsx`

**Skill.** `impeccable:impeccable` (craft), then `impeccable:clarify`.
**Acceptance criteria.**

- **During a drag:** a full-window overlay using `--drop-overlay` and a dashed
  `--accent` border, labelled "Drop files or folders to match them to the
  manifest".
- **After a drop:** a `DropResult` in a polite live region, for example
  "3 matched · 1 not in manifest: notes.txt · 1 ambiguous: app.dll (use
  Browse)". It can be dismissed.
- **Browse…** opens the native picker, starting in the last-used folder.
- **Clear** removes the assignment only. The copy makes clear that nothing on
  disk is touched.
- Drops are disabled, with the reason shown, when no valid manifest is loaded.

**Tests proving completion.** vitest with a mocked drag-drop event and a mocked
dialog, covering the matched, unmatched, and ambiguous rendering, and the
live-region text.
**States covered.** Idle, drag hover, disabled, result.

### UI task 5 — Output selection, build, confirmation, progress, and result

**Goal.** Choose where the tar goes and create it safely, with visible
progress and a verifiable result.
**Depends on.** UI task 3 and backend M6.
**Files.**

- `tools/tarpack/{BuildBar, BuildResult}.tsx`
- `app/ConfirmDialog.tsx`

**Skill.** `impeccable:layout`, `impeccable:clarify`, then `impeccable:animate`
for progress only, respecting `prefers-reduced-motion`.
**Acceptance criteria.**

- The sticky bar shows the output path with *Choose…* (a native Save dialog
  with the manifest's `output_name` as the suggestion) and a primary
  **Create tar** button.
- While the button is disabled, the reason is shown in visible text, not only a
  tooltip.
- If the output exists, a `ConfirmDialog` appears: "Replace gateway.tar?" The
  focus starts on *Cancel*.
- During the build, a determinate progress bar (`role=progressbar` with value
  text) shows the current entry, and controls are disabled.
- **On success:** `BuildResult` shows the path, entry count, size, and SHA-256
  with *Copy*, plus *Show in folder*.
- **On error:** shows the message and names the entry.
- `ManifestChangedOnDisk` shows the reload banner.

**Tests proving completion.** vitest for each disabled reason, the overwrite
confirmation flow, progress rendering, success, and error.
**States covered.** Disabled, ready, confirming, building, success, error.

### UI task 6 — Keyboard shortcuts and final polish and audit

**Goal.** Make the whole tool fast to drive from the keyboard, then polish and
audit it.
**Depends on.** UI tasks 2–5.
**Files.** `tools/tarpack/useTarpackShortcuts.ts`, plus touch-ups across the
tool.
**Skill.** `impeccable:polish`, then `impeccable:audit`. Fix every P0 and P1
finding.
**Acceptance criteria.**

- Shortcuts:

  | Keys | Action |
  | --- | --- |
  | Ctrl+O | Open manifest |
  | F5 / Ctrl+R | Reload |
  | Ctrl+E | Edit in editor |
  | Ctrl+Enter | Create tar (when enabled) |

- Arrow keys move between rows. On a focused row, Enter means Browse and Delete
  means Clear.
- Shortcuts are listed in the header's help popover.
- Visible focus is on everything.
- The layout is intact at 200% text size.
- No color-only meaning anywhere.

**Tests proving completion.** vitest shortcut tests, and `axe` over all
`TarpackView` states.
**States covered.** All.

## 5. Open questions for the human

- Should the shell have a manual light/dark toggle, or follow the system only?
  The proposal is system only; the `data-theme` hook is ready for a later
  toggle.
- Should the entry table show file size and modified time for each assigned
  file? The proposal is no, to keep the table narrow. Q6 in the backend plan may
  change this.
