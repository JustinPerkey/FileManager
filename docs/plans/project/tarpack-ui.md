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

**Partial results and building with errors (2026-09-29).** This revision
folds in the backend contract that main's re-plan predates: `tarpack.md`
§3.2.1, §5, decisions 31–41 (34 superseded), and §6.3, and M4/M6. The human
decided: *"Show the ones that passed but collect the ones that failed in some
sort of report. Also notify the user that there were errors."* and *"A single
error does not block builds but is included as an error in the final
report."* It ran `/impeccable critique` on this plan and U2–U6, `shape` for
the two new surfaces (the error region and the build's final report), and
`/impeccable audit` on the result (§7). The launcher ran this time; the
detector ran on the landed code and found nothing (only U1 has landed).

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
   - View states: loading, no manifest, errors with entries shown, errors
     with entries withheld, partial, ready, ready with files left out,
     building (writing, verifying, finishing), success, success with files
     left out, and error.
   - A manifest may have 0–200+ failed entries, each with 1–10 errors.
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

### 1.1 Brief: errors, the report, and building anyway

Written in `/impeccable shape` format for the two new surfaces. The discovery
answers are the human's two requirements above; no interview round could run
in this session, so the assumptions are marked.

1. **Job and audience.** The same developer, mid-loop, has just saved a
   manifest with a typo in one `[[file]]` table. They want a test build now,
   and to fix the typo next. Operate mode.
2. **Outcome and proof.** They build without being stopped, and they know,
   before and after, exactly which files the archive does not hold and why.
   Proof: one number, "N files left out", in four places (the notice, the
   table summary, the build bar, the result heading), and one report that
   names every failed entry with its `[[file]]` line and every error with
   `line:col`.
3. **Selected direction.** Extend "The Packing List": a packing list with a
   "not packed" section. The error region uses the Banner border treatment
   (1 px `--danger`, `x-circle`, no tint, no stripe). One pair of list
   components (`FailureList`, `DiagnosticList`) renders errors before and
   after the build, so the user learns one shape. The result heading turns
   from `check-circle`/`--ok` to `alert-triangle`/`--warn` and says "with N
   files left out"; the archive was made, so it is a warning, not an error.
4. **Scope and boundaries.** U2 (error region, announcer), U3 (summary and
   empty states), U4 (drops while errors exist), U5 (bar note, final report),
   U6 (F8, audit states). Anti-goals: a confirmation dialog for building with
   errors; any copy saying errors block the build (only "no files can be
   built" does); failed entries as table rows; a report file or "log"
   wording; toasts.
5. **States and ranges.** Errors with entries shown; entries withheld;
   every entry failed; manifest-level errors only (a bad `name`); warnings
   with and without errors; 200 failed entries (the report scrolls within
   `40vh`); a reload that clears every error.
6. **Interaction and layout.** The error region sits between the banners
   and the table, expanded after every open, reload, and restore that has
   errors, collapsible, and never steals focus. A persistent polite
   announcer tells screen-reader users the count. **Show errors** (in the
   notice, the bar, and F8) is the one route to the report. The bar's note
   is linked to Create archive with `aria-describedby`. The result is
   announced with its left-out count.
7. **Constraints and open decisions.** No new tokens and no new icons. The
   final report exists only in the UI (decision 41); **Copy report** puts
   it on the clipboard for the user's own notes. *Assumed:* the notice's
   report starts expanded (the human asked to be told, so show it); F8 is
   the shortcut (fits the F5 scheme and Windows editors' next-error key).
   Nothing open.

**Decisions carried from the backend contract.**

- **Partial results.** `session.manifest.entries` holds only passed entries;
  failed ones are in `failedEntries` (`EntryFailure`:
  `{ index, id|null, source|null, line, errors }`) and never table rows.
  `entriesWithheld` means a manifest-level error hides every entry.
  `manifest.errors` holds manifest-level errors only. `errorCount > 0` is
  the single "there were errors" flag; `readyCount`/`totalCount` count
  passed entries.
- **Errors never block building.** `canBuild` ignores `errorCount`.
  `buildBlockedReason` is `noManifest`, `noEntries`, `entriesNotReady`, or
  `noOutput`; there is no `manifestInvalid`. `noEntries` (no entry passed)
  is the only error-related block. No confirmation for building with
  errors.
- **The result is the final report.** `BuildSummary` adds `builtIds`,
  `leftOut`, `manifestErrors`, `warnings`, and `errorCount`. The UI renders
  the result from the summary, never the session. Nothing is written to a
  file.

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
- **Two-phase progress.** Writing, then verifying, each starting over and
  running up to 100% over the same `bytesTotal` (a phase's first event may
  already be a little past 0), shown as distinct labelled steps. Each phase ends with one
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
| `--font-size-lg` | `1rem` | region headings ("3 errors in this manifest", "Created …") |
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
| `TarpackView` | `tools/tarpack/TarpackView.tsx` | — (owns `showErrors()`, `announce(text)`, and the always-mounted polite announcer) | loading / no-manifest / errors (entries shown) / errors (entries withheld) / partial / ready / building (writing, verifying) / success / error | tool |
| `ManifestHeader` | `tools/tarpack/ManifestHeader.tsx` | `session, onOpen, onOpenRecent, onReload, onEdit` | loaded / changed-on-disk | tool |
| `ManifestErrors` | `tools/tarpack/ManifestErrors.tsx` | `manifest, expanded, onExpandedChange, onEdit, reportRef` | hidden / warnings only / errors expanded / errors collapsed / errors with entries withheld | tool (U2) |
| `FailureList` | `tools/tarpack/FailureList.tsx` | `failures: EntryFailure[], headingLevel: 3 \| 4` | — (nothing for `[]`) | tool (U2; reused by U5) |
| `DiagnosticList` | `tools/tarpack/DiagnosticList.tsx` | `diagnostics: Diagnostic[], label` | — (nothing for `[]`) | tool (U2; reused by U5) |
| `EntryTable` | `tools/tarpack/EntryTable.tsx` | `entries, failedCount, entriesWithheld, onBrowse, onClear` | populated / populated with files left out / no files / every file failed / withheld | tool |
| `EntryStatus` | `tools/tarpack/EntryStatus.tsx` | `status` | Ready / Missing / Not assigned | tool |
| `EolMarker` | `tools/tarpack/EolMarker.tsx` | — | shown only when `normalizeEol` | tool |
| `DropResult` | `tools/tarpack/DropResult.tsx` | `outcome, hasFailedEntries, onDismiss` | matched / unmatched, one line per `UnmatchedReason` / unmatched with failed entries / ambiguous | tool |
| `BuildBar` | `tools/tarpack/BuildBar.tsx` | `session, building, progress, onChooseOutput, onFormatChange, onBuild, onShowErrors` | disabled-with-reason / no entries (three cases) / ready / ready with files left out / ready with manifest errors only / building | tool |
| `FormatPicker` | `tools/tarpack/FormatPicker.tsx` | `formats, value, disabled, onChange` | enabled / disabled | tool |
| `BuildProgress` | `tools/tarpack/BuildProgress.tsx` | `progress, entries` | writing / verifying / finishing | tool |
| `BuildResult` | `tools/tarpack/BuildResult.tsx` | `result, entries, onReveal, onDismiss` | success / success with files left out / success with manifest errors only / error | tool |
| `BuildReport` | `tools/tarpack/BuildReport.tsx` | `leftOut, manifestErrors, warnings` | errors / warnings only / nothing | tool (U5) |
| `reportText` | `tools/tarpack/reportText.ts` | `reportText(summary): string` (module, for Copy report) | — | tool (U5) |
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

### Error region (U2)

Between the banners and the table, only while `errorCount > 0`: a notice
("{n} errors in this manifest", one consequence line, **Show/Hide errors**,
**Edit in editor**) over a report body (`id="manifest-error-report"`,
`40vh` max, one tab stop) with a "Whole manifest" group and a "Files with
errors" `FailureList`. Each failed entry is named by `id` (mono) or "Entry
#index", with "source {source} · [[file]] on line {line}", then every error
as `line:col` plus the verbatim message ("Line N, column M:" for screen
readers). Warnings follow in a collapsed `<details>`, shown even when there
are errors. Consequence lines:

- withheld: "No files can be listed or built until the manifest errors are
  fixed.";
- failed entries: "{n} files are left out of the list and the archive until
  they're fixed.";
- manifest-level only: "Every file is listed and can still be built."

### Left-out vocabulary

One phrase, "left out", everywhere a count of failed entries appears:

| Where | Text |
| --- | --- |
| Notice (U2) | "{n} files are left out of the list and the archive until they're fixed." |
| Announcer (U2) | "{name} has {e} errors. {n} files will be left out of the archive." |
| Table summary (U3) | "5 of 5 files ready · 2 files left out (errors)" (never "All …") |
| Build bar note (U5) | "{n} files will be left out; errors will be listed after the build" |
| Result heading (U5) | "Created {file} with {n} files left out" |
| Result announcement (U5) | "Created {file}. {n} files were left out because of errors." |

Every count has a singular form ("1 file is", "1 file will be", "with 1 file
left out").

### Build bar layout

Left to right, wrapping onto two lines at narrow widths or 200% text:
**Output:** path (or "not chosen") and **Choose…** · **Format** picker (native
`<select>`) · status text (the disabled reason and/or the left-out note) ·
**Show errors** (when `errorCount > 0`) · **Create archive**. The settings and
the action wrap as two groups. While building, the progress region replaces
the status text.

- Reasons: `noManifest` "Open a manifest first"; `noEntries` "No files can be
  built until the manifest errors are fixed" (withheld) / "Every file in the
  manifest has errors" (all failed) / "This manifest lists no files" (none);
  `entriesNotReady` "{n} files still need a location"; `noOutput` "Choose
  where to save the archive".
- Left-out note (when `errorCount > 0` and entries exist; `aria-describedby`
  on Create archive): "{n} files will be left out; errors will be listed
  after the build", or "Builds with {n} manifest errors".
- Create archive is enabled whenever `canBuild`, with or without errors. No
  confirmation for errors.

### Build result: the final report (U5)

With `summary.errorCount` 0 the result is the success-only layout (plus
collapsed warnings, if any). Otherwise the heading reads "Created {file} with
{n} files left out" (or "with {n} manifest errors") with `alert-triangle` in
`--warn`, a line "The archive holds {built} of the manifest's {total} files",
then the usual details, then a `BuildReport` region ("Left out of the
archive", "Manifest errors", "Warnings", from `leftOut`, `manifestErrors`,
`warnings`), **Show in folder**, and **Copy report**. The report is not
saved anywhere, and no copy implies a log.

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
manifest changed on disk." with **Reload**. `ManifestChangedOnDisk` also
covers a manifest that can no longer be read at build time; the same copy
fits, and **Reload** then reports `ManifestUnreadable`.

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

## 4. UI task index

Each UI task is specified in full in its task plan. The task plan is the only
plan its `ui-implementer` reads.

| # | Task plan | Goal | Depends on | `/impeccable` commands |
| --- | --- | --- | --- | --- |
| U1 | [`U1-app-shell.md`](../tasks/tarpack/U1-app-shell.md) | Window layout, tokens, tool navigation | M1 | landed (ran under the old names) |
| U2 | [`U2-manifest-header.md`](../tasks/tarpack/U2-manifest-header.md) | Manifest header; no-manifest and changed-on-disk states; error region (notice, report, announcer), warnings; `FailureList`, `DiagnosticList`; shared `Button`, `Icon`, type tokens | U1, M6 | `layout`, `clarify`, `audit` |
| U3 | [`U3-entry-table.md`](../tasks/tarpack/U3-entry-table.md) | Entry table (passed entries only) with per-row status, the CRLF → LF marker, the left-out summary clause, and the withheld / every-file-failed / no-files sentences | U2, M6 | `typeset`, `layout`, `harden` |
| U4 | [`U4-drop-and-assign.md`](../tasks/tarpack/U4-drop-and-assign.md) | Drag-and-drop and per-row Browse/Clear, enabled while errors exist; disabled with a reason when no entry passed | U3, M6 | new work in the established world, `clarify` |
| U5 | [`U5-build-bar.md`](../tasks/tarpack/U5-build-bar.md) | Output, format picker, build (enabled with errors, with the left-out note), overwrite confirmation, two-phase progress, result with extraction command and the final report | U2, U3, M6 | `layout`, `clarify`, `animate`, `audit` |
| U6 | [`U6-shortcuts-polish.md`](../tasks/tarpack/U6-shortcuts-polish.md) | Keyboard shortcuts (including F8 Show errors), polish, audit | U2–U5 | `polish`, `audit` (with the detector) |

U4 and U5 can run in parallel. Every UI task that reads the partial-results
or build-report fields (U2–U5) runs after M6, which also depends on M3 and
M4 for those types. M7 (packaging) depends on U6.

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
- **Errors and building** (the human, 2026-09-29): show the passed entries,
  report the failed ones, notify the user; a single error does not block
  the build and is included in the final report. Recorded in §1.1; the
  superseded "build blocked while any error exists" wording is gone from
  every UI plan.
- **Designer's choices for the above**, which stand unless the human
  overrides them: no confirmation for building with errors (backend
  decision 40); the report starts expanded after each open or reload; F8
  shows the errors; **Copy report** copies the final report as plain text,
  and nothing is saved to a file (decision 41).

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

## 7. impeccable critique and audit: partial results (2026-09-29)

Run on this plan and U2–U6 after main's re-plan was merged with the
partial-results backend work. The `impeccable context` launcher ran. The
critique ran single-context (no sub-agent tool in that session). The detector
ran over `apps/desktop/src` and found nothing: only U1 has landed, and the
plans are markdown. No browser was available, so U6's rendered audit is still
the first visual check.

**Critique before the changes: 24/40 (Acceptable).**

| # | Sev | Finding | Where it is fixed |
| --- | --- | --- | --- |
| 1 | P0 | U5 kept the superseded contract: a `manifestInvalid` reason, "Fix N manifest errors first", and "cannot be created while any error exists"; U2 and U5 listed `ManifestInvalid`, which `TarpackErrorKind` no longer has, so typecheck would fail. | U5 (`noEntries`, left-out note, enabled build), U2 and U5 (`NoEntries` copy), §3 |
| 2 | P1 | No error report and no notification. U2 keyed the error state on `manifest.errors.length` (manifest-level only) and hid warnings when errors existed. | U2 error region, `FailureList`, `DiagnosticList`, announcer |
| 3 | P1 | Silent drop in the table: "All N files ready" counted passed entries only, and one empty sentence covered withheld, all-failed, and no files. | U3 summary clause and three sentences |
| 4 | P1 | The build result had no report and showed a success check when files were left out. | U5 heading, `BuildReport`, announcement, Copy report |
| 5 | P2 | U4 disabled drops on any error, and "not in the manifest" was false for a failed entry's file. | U4 enabled rule, "not matched" and hint |
| 6 | P3 | "problems" and "errors" mixed; U6 lacked the error states in its axe list and had no route to the errors. | U2 copy rule, U6 (F8, states) |

**Audit of the revised plans (static): 18/20 (Excellent).**

| Dimension | Score | Key finding |
| --- | --- | --- |
| Accessibility | 4 | Fixed in the plans: the error block is no longer a second landmark with the report's name; the collapsed report stays mounted (`hidden`) for `aria-controls`; heading levels are pinned (h1 view, h2 notice/result, h3 groups, h4 entries) |
| Performance | 4 | 200 failures × 10 errors is about 2,000 list items, rendered once per session change, with no measurement |
| Responsive | 3 | Both reports are capped at `40vh`, but together with the table and bar at 800×560 and 200% text they need the view to scroll; U6 now checks that worst case |
| Theming | 4 | No new tokens, colors, or icons |
| Implementation integrity | 3 | One list pair serves both reports; no matching or path logic added in TS. P3s below |

Open P3s, left for the builders to report rather than fixed here:

- `NoEntries` copy ("… Fix the manifest errors first.") is wrong for a
  manifest that lists no files and has no errors. It is defensive, since the
  bar blocks that build first, and the wording was given by the planner.
- The notice's **Show errors** toggle expands without moving focus, and the
  bar's **Show errors** (and F8) expands and focuses. The labels are the same
  because the outcome, the report open on screen, is the same; the notice
  follows the standard disclosure pattern.
- "Created {file name}" takes the file name from `summary.path` in TS, as
  main's plan already did. A backend display field would remove that path
  handling; that would be a contract change and is not requested here.

## Contract change from M5's review (planner, 2026-09-30)

`DropOutcome.unmatched` is now `Unmatched[]` with a typed
`reason: UnmatchedReason` (`alreadyAssigned`, `noEntry`, `notFound`,
`linkNotFollowed`, `folderNoMatch`, `unreadable`, `notUnicode`) instead of
free English text; the paths in `unmatched` and `ambiguous` are display
strings. The planner updated U4 to match: one result line per reason, an
exhaustive `Record<UnmatchedReason, …>` of copy and icons, and the
failed-entries hint also after a `folderNoMatch` line. The copy in U4 is the
planner's first draft; the ui-designer may revise the wording in U4 before
it runs, keeping one line per reason and the exhaustive mapping.
