# UI project plan: app shell and the Tar Packager view

Status: **awaiting approval** (U1 landed). Backend project plan:
[`tarpack.md`](tarpack.md). Updated 2026-09-29 for the human's answers to the
backend open questions (`tarpack.md` §6) and to this plan's own §5.

**Re-planned with `impeccable` 4.4 (2026-09-29).** The first draft followed the
`shape` method by hand. This revision:

- runs `init`, which wrote the root [`PRODUCT.md`](../../../PRODUCT.md), and
  `document` (scan mode), which wrote the root
  [`DESIGN.md`](../../../DESIGN.md) from U1's tokens;
- rewrites §1 in `shape`'s brief format;
- critiques this plan, and audits the landed U1 code, against the craft floor
  and Operate-mode guidance (§6), then folds the findings into §2, §3, and the
  U2–U6 task plans.

The skill's launcher (`impeccable context`) could not run in this session, so
the flow was followed from its reference docs. That also means the
deterministic detector did not run; U6 runs it.

**How the skill is named.** `impeccable` is one skill with sub-commands.
Task plans name them as `/impeccable <command>`, for example
`/impeccable layout`. The old names (`impeccable:layout`, `craft`) are retired.
`craft` is now an alias for ordinary new work, and `teach` for `init`. When the
skill is not installed, the implementer installs it with
`npx impeccable install`, or follows the command's reference doc from the
skill's repository by hand, and says so in its report.

**Stack (decided).** Tauri v2 shell, React + TypeScript + Vite frontend in
`apps/desktop/src/`, design tokens in `apps/desktop/src/styles/tokens.css`. The
implementer owns `apps/desktop/src/lib/` (typed client and generated types);
the ui-implementer owns the rest of `apps/desktop/src/`. Every UI task depends
on the foundation milestone (M1); U2–U5 also depend on the Tauri contract (M6).

## 1. Design brief

Written in `/impeccable shape` format. Product truth lives in `PRODUCT.md`, and
the visual system in `DESIGN.md`; this brief keeps only what is specific to the
Tar Packager surface.

1. **Job and audience.** A developer or tester rebuilds the same package many
   times a day for a low-power armv7 Linux target. They know the file set. The
   files come from scattered build folders, and on Windows the Linux
   permissions stay invisible until something fails on the target. **Visitor
   mode: Operate.**
2. **Outcome and proof.** The task: confirm everything is present and correct,
   pick the format, build, and copy the extraction command. Success is a
   verified archive and a copied command, reached in a few keystrokes. The
   proof on screen is the exact Linux path, mode, owner, and line-ending
   treatment of every file, with an explicit per-row status and a summary
   line.
3. **Selected direction.** Extend the established world: U1's tokens, as
   recorded in `DESIGN.md` ("The Packing List"). This is a whole surface
   inside an established world. The structure is a single window with:
   - a narrow **tool sidebar** (the home for future tools);
   - a **manifest header**;
   - the **entry table** as the hero and focal point;
   - a sticky **build bar** at the bottom, holding output, format, and the
     build action.

   The whole window is a drop target. The structure is settled by the product
   and the approved plans, so no concept round is run.
4. **Scope and boundaries.** Production-ready Tar Packager view, with the
   states below. The following stay as they are: U1's frame, the tokens'
   values, the backend contract, and every rule in `CLAUDE.md`. Anti-goals:
   - decoration;
   - cards around regions;
   - metric tiles ("5 of 6" is a sentence, not a hero number);
   - a modal for anything but the overwrite confirmation;
   - any extension, file-name, or matching logic in TS.
5. **States and ranges.**
   - Manifests list 1–50 files typically, and up to about 2,000.
   - Windows paths run to 260+ characters. Stored Linux paths may exceed
     100 bytes (the backend warns). Display strings may contain U+FFFD for
     names that are not valid UTF-8.
   - View states: loading, no manifest, invalid, partial, ready, building
     (writing, verifying, finishing), success, and error.
   - Transient states: changed on disk, drop result, and command errors.
6. **Interaction and layout.**
   - Keyboard first, with the shortcuts in U6.
   - Status is always an icon plus a word, never color alone.
   - One primary action per region, and **Create archive** is the only
     accent button in the view.
   - Feedback is inline, in live regions, not in toasts.
   - Motion only conveys state: the drop overlay fade and the progress fill.
   - The build bar wraps onto two lines at narrow widths and at 200% text.
7. **Constraints and open decisions.**
   - WebView2 under a restrictive CSP: no remote assets, so icons are inline
     SVG.
   - WCAG 2.2 AA, 200% text, and an 800×560 minimum window.
   - The theme follows the system only (decided, §5).
   - Shared vocabulary that builders must not invent per task: the `Button`,
     `Icon`, type-size tokens, and banner treatment (U2, §2–§3).
   - None open.

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

The format picker, EOL marker, and extraction command need no new tokens: the
marker uses `--text-muted` on `--surface-sunken` with `--border`, a pair U1
already verifies for AA. Every text/background pair must reach WCAG AA. UI task
1 verifies this.

**Added by U2 (impeccable critique, §6).** These tokens are theme-independent.
They fix the type scale, so no component invents a size (Operate mode uses a
fixed rem scale with a ratio of about 1.125–1.25):

| Token | Value | Use |
| --- | --- | --- |
| `--font-size-sm` | `0.8125rem` | secondary lines, hints, table header labels |
| `--font-size-md` | `0.875rem` | body, controls, table cells, mono data (current `body` size) |
| `--font-size-lg` | `1rem` | region headings ("This manifest has N problems", "Created …") |
| `--font-size-xl` | `1.25rem` | the view heading (current `.tool-view h1`) |
| `--shadow-overlay` | `0 8px 24px rgb(0 0 0 / 0.18)` light, `0 8px 24px rgb(0 0 0 / 0.5)` dark | the confirmation dialog and the shortcuts popover only |

U2 also themes the browser surfaces in `base.css`, with no new color values:

- `::selection` uses `--accent` behind `--accent-text`;
- `scrollbar-color` uses `--border` on `--surface-sunken`;
- a `.num` utility sets `font-variant-numeric: tabular-nums`.

## 3. Component inventory

All components live under `apps/desktop/src/`.

| Component | Path | Props | States | Scope |
| --- | --- | --- | --- | --- |
| `AppShell` | `app/AppShell.tsx` | `tools` | — | shared |
| `ToolNav` | `app/ToolNav.tsx` | `tools, activeId, onSelect` | — | shared |
| `DropZone` | `app/DropZone.tsx` | `enabled, disabledReason, label, onDrop(paths)` | idle / hover / disabled | shared |
| `Button` | `app/Button.tsx` (+ `styles/controls.css`) | native `<button>` props plus `variant: "primary" \| "secondary" \| "quiet"`, `icon?` | default / hover / active / focus / disabled / busy | shared (U2) |
| `Icon` | `app/icons.tsx` | `name`, `label?` | — | shared (U2) |
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
| `errorMessages` | `tools/tarpack/errorMessages.ts` | `errorMessage(error, entries)` (module, not a component) | one entry per `TarpackErrorKind` | tool |

### Shared vocabulary (U2, from the impeccable critique)

- **Buttons.** Every button in the app is a `Button`. It has one shape
  (`--radius`, 1 px border, `--font-size-md`, at least 24×24 px, padding
  `--space-1 --space-3`) and three variants:
  - **primary**: `--accent` fill, `--accent-text` label. There is one per
    region: **Create archive**, **Replace** in the dialog, and **Open
    manifest…** in the empty state.
  - **secondary**: `--surface` fill, `--border` outline, `--text` label. This
    is the default.
  - **quiet**: transparent until hover. Use it for row actions (Browse…,
    Clear), Copy, and Dismiss.

  Hover is `--surface-sunken`; for primary it is
  `color-mix(in srgb, var(--accent) 88%, var(--text))`, which references only
  tokens and so follows the theme. Focus is the global ring. Disabled is `--text-muted`
  on `--surface-sunken` with a default cursor. Busy is `aria-busy` with the
  label unchanged.
- **Icons.** One authored inline-SVG set in `app/icons.tsx`: a 16 px grid,
  1.5 px stroke, round caps and joins, and `currentColor`. The initial set is
  `check-circle`, `alert-triangle`, `circle`, `x-circle`, `info`,
  `chevron-down`, `folder`, `copy`, `x`, and `keyboard`. Icons are
  `aria-hidden`; the word next to them carries the meaning. No Unicode
  glyphs (`▾`, `✓`, `⚠`) and no emoji.
- **Banners.** `--surface` fill with a 1 px border in the tone color, the tone
  icon in the tone color, and the message in `--text`. The tone is also named
  in visually hidden text ("Warning:", "Error:"). There is no tinted fill and
  no thick left stripe. Banners stack above the table, newest first, each
  dismissible except changed-on-disk, which clears on reload.

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

## 4. UI task index

Each UI task is specified in full in its task plan. The task plan is the only
plan its `ui-implementer` reads.

| # | Task plan | Goal | Depends on | `/impeccable` commands |
| --- | --- | --- | --- | --- |
| U1 | [`U1-app-shell.md`](../tasks/tarpack/U1-app-shell.md) | Window layout, tokens, tool navigation | M1 | landed (ran under the old names) |
| U2 | [`U2-manifest-header.md`](../tasks/tarpack/U2-manifest-header.md) | Manifest header; no-manifest, invalid, changed-on-disk states; shared `Button`, `Icon`, type tokens | U1, M6 | `layout`, `clarify` |
| U3 | [`U3-entry-table.md`](../tasks/tarpack/U3-entry-table.md) | Entry table with per-row status and the CRLF → LF marker | U2, M6 | `typeset`, `layout`, `harden` |
| U4 | [`U4-drop-and-assign.md`](../tasks/tarpack/U4-drop-and-assign.md) | Drag-and-drop and per-row Browse/Clear | U3, M6 | new work in the established world, `clarify` |
| U5 | [`U5-build-bar.md`](../tasks/tarpack/U5-build-bar.md) | Output, format picker, build, overwrite confirmation, two-phase progress, result with extraction command | U3, M6 | `layout`, `clarify`, `animate`, `audit` |
| U6 | [`U6-shortcuts-polish.md`](../tasks/tarpack/U6-shortcuts-polish.md) | Keyboard shortcuts, polish, audit | U2–U5 | `polish`, `audit` (with the detector) |

U4 and U5 can run in parallel. M7 (packaging) depends on U6.

## 5. Open questions for the human

None open. Resolved 2026-09-29 (the human accepted the proposed defaults):

- **Theme:** follow the system only. No manual light/dark toggle is built; the
  `data-theme` hook in `tokens.css` stays for a possible later toggle.
- **Size and modified-time columns:** not shown. The entry table stays narrow.
- **impeccable context files:** add `PRODUCT.md` and `DESIGN.md` at the repo
  root (2026-09-29).
- **Skill naming:** plans and both agent runtimes use `/impeccable <command>`.
  The skill is not vendored into the repository (2026-09-29).
- **Critique findings:** folded into §2, §3, and the U2–U6 task plans, not left
  as open items (2026-09-29).

## 6. impeccable critique and audit (2026-09-29)

`/impeccable critique` of this plan and `/impeccable audit` of the landed U1
code were run by reading, against the craft floor and Operate-mode guidance.
The detector and screenshots were not available, so U6 re-runs `audit` with
both. Severity follows the audit scale (P0 blocking … P3 polish).

| # | Sev | Finding | Where it is fixed |
| --- | --- | --- | --- |
| 1 | P1 | No shared button vocabulary. U2–U5 each add buttons (primary, secondary, row, copy, dismiss) and would each style their own, so the same action looks different in two places. | `Button` in U2; U3–U5 use it |
| 2 | P1 | Status icons, "Recent ▾", and dismiss have no icon system, and the plan invites Unicode glyphs. | `Icon` set in U2; U2–U5 use it |
| 3 | P1 | No type scale beyond `body` and `h1`, so each task would pick one-off sizes. | `--font-size-*` tokens in U2 |
| 4 | P2 | `Banner` has no visual spec. The default reach, a tinted fill with a thick colored left stripe, is a craft-floor refusal. | Banner spec in U2 |
| 5 | P2 | U1's current nav item has a 3 px `--accent` left border, the same side-stripe refusal. Fill, outline, and weight already carry the state. | U6 |
| 6 | P2 | Browser surfaces are unthemed: text selection, scrollbars, and proportional numerals in counts, sizes, octal modes, and percentages, which jitter while progress updates. | `base.css` in U2; `.num` in U3 and U5 |
| 7 | P2 | The list-scale behavior is stated only for 200 rows. Non-UTF-8 display names (U+FFFD) and code-point-safe middle truncation are unstated. | U3 (`harden`) |
| 8 | P2 | Empty and error states name the problem, but not always the next step ("This manifest lists no files."). | U2, U3 copy |
| 9 | P3 | The dialog and popover have no elevation rule. | `--shadow-overlay`, U5 and U6 |
| 10 | P3 | Row hover and roving-focus row styles are unspecified. | U3 (hover), U6 (focused row) |

**U1 audit (static).**

| Dimension | Score | Key finding |
| --- | --- | --- |
| Accessibility | 4 | `nav` landmark, roving tabindex, `aria-current`, AA contrast asserted by a test |
| Performance | 4 | trivial surface |
| Responsive | 3 | holds at 800×560; 200% text not yet exercised by a test |
| Theming | 4 | all colors are tokens; dark mode is defined twice as specified |
| Implementation integrity | 3 | finding 5; otherwise coherent |
| **Total** | **18/20** | Excellent (minor polish) |

**What works and is kept:** the restrained palette, tonal depth with no
shadows, system UI type plus a data mono, one radius, and token-only color.
