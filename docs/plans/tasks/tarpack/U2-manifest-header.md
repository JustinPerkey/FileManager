# U2 — Manifest header, empty state, error report and notice, changed-on-disk banner

Status: done (landed 2026-09-30, reviewer-approved)
Amended: 2026-09-30 by the ui-designer, after the reviewer's findings on the
first implementation (test seam, menu elevation, banner announcements and
order, failed restore, error normalisation, stylesheet home, tests).
Project: tarpack   Depends on: U1 (landed), M6 (landed; this task reads its
partial-results fields `entriesWithheld`, `failedEntries`, and `errorCount`)

## Goal

Let the user open, reload, and edit a manifest, and see clearly when none is
loaded, when it changed on disk, and when it has errors: which files failed
and why, with a notice that tells them errors exist. Along the way, create the
shared button, icon, and type vocabulary that every later task uses.

## Context

**The Tar Packager.** It builds a Linux archive (`.tar`, `.tar.gz`, `.tar.zst`,
or `.tar.xz`; the format is picked in the build bar, not in this task) from
Windows files. A
**manifest** (a TOML file the user edits in their own editor) lists the files,
their Linux target paths, and their permissions. The format is documented in
`docs/tarpack-manifest.md`; read it for the vocabulary, including its section
"When the manifest has errors". The user rebuilds the same package many times
a day, so they want confidence at a glance.

**A manifest with errors still opens, and errors do not block the build
(decided by the human).** The human's words: *"Show the ones that passed but
collect the ones that failed in some sort of report. Also notify the user that
there were errors."* and *"A single error does not block builds but is
included as an error in the final report."* So:

- a `[[file]]` table with no error of its own **passes**. It is listed in the
  entry table (U3), can be assigned a file (U4), and is built (U5);
- a `[[file]]` table with at least one error **fails**. It is left out of the
  table and out of the archive, and it is listed in the **error report** (this
  task) with all of its errors;
- errors outside every `[[file]]` table are **manifest-level** errors, listed
  in the report on their own. Some of them (TOML syntax, a file that is not
  UTF-8, `version`, `[defaults]`, an unknown top-level key, `file` not being
  an array) mean no entry can be trusted, so the backend **withholds** every
  entry and the table is empty. Others (`name`, `output_name`) hide nothing;
- whenever the manifest has errors, the user is **notified**, visibly and to
  screen readers;
- **errors never block building.** The build writes the passed entries, and its
  result lists everything it left out (U5). Only "no file passed" stops a
  build. Never write copy saying errors must be fixed before building, except
  where no file passed at all.

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
- `openManifest`, `reloadManifest`, and the first `session()` call succeed for
  any readable file, whatever its errors. Only an unreadable file rejects,
  with `ManifestUnreadable`. A manifest with errors is **not** a command
  error.
- Types come from `src/lib/generated/`, which includes `TarpackSession`,
  `Diagnostic`, `EntryFailure`, and `TarpackError`. The parts this task reads:
  - `session.manifest`: `null`, or
    `{ path, name, outputName, hash, entries, entriesWithheld, errors, failedEntries, errorCount, warnings }`:
    - `entries`: the entries that **passed** validation only, in manifest
      order (U3 renders them);
    - `entriesWithheld: boolean`: `true` when a manifest-level error hides
      every entry; `entries` is then `[]`;
    - `errors: Diagnostic[]`: manifest-level errors **only** (outside every
      `[[file]]` table);
    - `failedEntries: EntryFailure[]`: one per `[[file]]` table with errors,
      in manifest order;
    - `errorCount: number`: `errors.length` plus every
      `failedEntries[i].errors.length`. **`errorCount > 0` is the single
      "this manifest has errors" flag.** Never derive it from
      `errors.length`, which misses every entry error;
    - `warnings: Diagnostic[]`;
    - `name`: the manifest's `name`, or the manifest file's stem when `name`
      itself is in error. Render it as given.
  - `EntryFailure`:
    `{ index: number, id: string | null, source: string | null, line: number, errors: Diagnostic[] }`.
    `index` is the 1-based position of the `[[file]]` table; `id` is `null`
    when the table has no usable id; `source` is `null` when it has no string
    `source`; `line` is the line of the table's `[[file]]` header; `errors` is
    never empty and is sorted by line and column.
  - `Diagnostic`: `{ severity, line, col, entryId: string | null, message }`.
    `line` and `col` are 1-based. The `message` is final user-facing text and
    already carries its entry prefix, for example
    ``file `gateway`: missing field `dir` `` or
    `` [[file]] #3: id must not be empty ``. Render it verbatim: never strip
    the prefix, reword, or truncate it.
  - `session.stateWarning`: `string | null`.
  - `TarpackError`: `{ kind: TarpackErrorKind, message: string, entryId?: string }`.
    `entryId` is omitted (not `null`) when absent. `message` is technical
    detail, not user copy.
  - `TarpackErrorKind`: a closed string-literal union of exactly 17 kinds,
    listed in the error table below. It has `NoEntries`; there is no
    `ManifestInvalid`.

**Design context.** The root `PRODUCT.md` records the product. The root
`DESIGN.md` records the visual system U1 built: "The Packing List". It is a
quiet, dense, restrained palette, with tonal depth, no shadows at rest, system
UI type, and mono for data. This is an **Operate** surface that extends the
established world. Invent no new visual identity and do not rewrite
`DESIGN.md`, except to change its "Planned (U2)" markers to landed once you
build them. The ui-designer has already updated its intro and its Elevation &
Depth section for this task; leave those as they are.

**The single source of truth.** Every command returns a full `TarpackSession`,
and the view renders it. Keep it in one piece of React state in
`TarpackView`, replaced wholesale on each command result. Never derive a second
copy. View-only UI state (whether the error report is expanded, the text of the
announcer) is not a copy of the session and may live in `TarpackView`.

**The view's full structure**, top to bottom. Later tasks fill in the rest:

- the header (this task);
- the banners: command error, changed on disk, state warning (this task);
- the **error region**: notice and report (this task);
- the warnings (this task);
- the drop result (U4);
- the entry table (U3);
- the build result, above the bar (U5);
- a sticky build bar at the bottom (U5).

Reserve those regions, and leave the drop result, table, result, and bar as
empty slots.

### Components

| Component | Path | Props | States |
| --- | --- | --- | --- |
| `TarpackView` | `src/tools/tarpack/TarpackView.tsx` | `actionsRef?: Ref<TarpackActions>` (test-only seam, below) | loading / no-manifest / no-manifest after a failed restore / loaded / errors (entries shown) / errors (entries withheld) / changed-on-disk |
| `ManifestHeader` | `src/tools/tarpack/ManifestHeader.tsx` | `session, onOpen, onOpenRecent, onReload, onEdit` | loaded |
| `ManifestErrors` | `src/tools/tarpack/ManifestErrors.tsx` | `manifest, expanded, onExpandedChange(expanded), onEdit, reportRef` | hidden / warnings only / errors expanded / errors collapsed / errors with entries withheld |
| `FailureList` | `src/tools/tarpack/FailureList.tsx` | `failures: EntryFailure[], headingLevel: 3 \| 4` | — (renders nothing for `[]`) |
| `DiagnosticList` | `src/tools/tarpack/DiagnosticList.tsx` | `diagnostics: Diagnostic[], label: string` | — (renders nothing for `[]`) |
| `errorMessages` | `src/tools/tarpack/errorMessages.ts` (module) | exports `errorMessage(error: TarpackError, entries): string` and `toTarpackError(e: unknown): TarpackError` | one entry per kind |
| `Banner` | `src/app/Banner.tsx` (shared) | `tone: "info"\|"warn"\|"error", message, action?: { label, onAction }, onDismiss?, children?` (children: the Details disclosure) | info / warn / error |
| `Button` | `src/app/Button.tsx` (shared) + `src/styles/controls.css` | native `<button>` props, plus `variant: "primary"\|"secondary"\|"quiet"` (default `"secondary"`) and `icon?: IconName` | default / hover / active / focus / disabled / busy |
| `Icon` | `src/app/icons.tsx` (shared) | `name: IconName` | — |

`FailureList` and `DiagnosticList` are tool-local, not shared app components.
U5 reuses both, unchanged, in the build's final report, so the errors look the
same before and after a build. `reportRef` is a ref to the report body, so
`TarpackView` can move focus to it.

`TarpackView` also owns two functions that later tasks call:

- **`showErrors()`**: expands the report body, then moves focus to it
  (`reportRef`) after it renders, and scrolls it into view (instantly under
  `prefers-reduced-motion`). It does nothing when `errorCount` is 0. The
  notice's own toggle does not use it (see below); U5's build bar and U6's
  shortcut do.
- **`announce(text)`**: sets the text of the view's announcer (below). U5 uses
  it to announce the build result.

**Test-only seam.** Export
`interface TarpackActions { showErrors(): void; announce(text: string): void }`
from `TarpackView.tsx`, and give `TarpackView` one optional prop,
`actionsRef?: Ref<TarpackActions>`, filled with `useImperativeHandle`. It
exists only so this task's tests can call the two functions before any caller
exists. The registry and `AppShell` never pass it, and product code never
reads it: U5 and U6 call `showErrors()` and `announce()` inside `TarpackView`
(passed down as props, or from a hook the view owns). Do not add other
imperative methods to it.

### Shared vocabulary (you create it)

An `impeccable` critique of the plan found that without this, U2–U5 would each
style their own buttons, reach for Unicode glyphs as icons, and pick one-off
font sizes. Build it first, use it in this task, and later tasks import it.

**Type-size tokens.** Add these to `src/styles/tokens.css`, on `:root` only;
they do not change with the theme:

| Token | Value | Use |
| --- | --- | --- |
| `--font-size-sm` | `0.8125rem` | secondary lines, hints, header labels |
| `--font-size-md` | `0.875rem` | body, controls, mono data |
| `--font-size-lg` | `1rem` | region headings ("3 errors in this manifest") |
| `--font-size-xl` | `1.25rem` | the view heading |

Also add `--shadow-overlay`: `0 8px 24px rgb(0 0 0 / 0.18)` in light, and
`0 8px 24px rgb(0 0 0 / 0.5)` in both dark blocks. It is for surfaces that
float over content, and only three use it: the **Recent** menu (this task),
U5's confirmation dialog, and U6's shortcuts popover. Nothing else has a
shadow.

Point `body` in `base.css` at `--font-size-md`, and `.tool-view h1` in
`app.css` at `--font-size-xl`, so the values have one home.

**Browser surfaces** (`src/styles/base.css`):

- `::selection` uses `--accent-text` on `--accent`;
- `html` sets `scrollbar-color: var(--border) var(--surface-sunken)`. Do not
  build custom scrollbars;
- a `.num` utility class sets `font-variant-numeric: tabular-nums`. Use it for
  counts ("3 errors", "N warnings");
- a `.mono` utility class, beside `.num`, sets
  `font-family: var(--font-mono)` only; the size inherits from its context.
  Use it for ids, sources, paths, and `line:col`;
- a `.visually-hidden` utility (the standard clip pattern), if U1 did not
  already add one. The report and the announcer use it.

**`Button`.** It wraps a native `<button type="button">` and forwards every
prop and ref. Every button in the app uses it. Styles go in
`src/styles/controls.css`, imported from `App.tsx`:

- **Shape:** `--radius`, 1 px border, font `inherit` at `--font-size-md`,
  padding `--space-1 --space-3`, a minimum 24×24 px target, and an icon gap of
  `--space-1`.
- **primary:** `--accent` fill and border, `--accent-text` label. There is one
  per region; in this task that is **Open manifest…** in the empty state.
- **secondary:** `--surface` fill, `--border` outline, `--text` label. This is
  the default, used for the header actions and the error notice's actions.
- **quiet:** transparent with a transparent border; `--surface-sunken` on
  hover. Use it for Dismiss.
- **Details disclosure:** not a `Button`. It is a native
  `<details>`/`<summary>` (the browser supplies the toggle semantics and
  Enter/Space), with the `<summary>` styled like a quiet control: a minimum
  24×24 px target, the quiet-button padding (`--space-1 --space-3`),
  `--radius`, `cursor: pointer`, a `--surface-sunken` fill on hover, and the
  global `:focus-visible` ring. The rule is `.banner__details summary` in
  `controls.css`.
- **hover:** secondary and quiet use `--surface-sunken`. Primary uses
  `color-mix(in srgb, var(--accent) 88%, var(--text))`. The mix references
  only tokens, so it follows the theme.
- **active:** the same fill as hover, plus `translate: 0 1px`. No transition
  on layout properties.
- **focus:** the global `:focus-visible` ring from `base.css`. Never remove
  it.
- **disabled:** `--text-muted` on `--surface-sunken`, with `--border` and
  `cursor: default`. Use the real `disabled` attribute.
- **busy:** `aria-busy="true"`, the label unchanged, and the button disabled.

**`Icon`.** Inline SVG authored in `src/app/icons.tsx`. There is no icon
library, and the CSP forbids remote assets.

- One style: a 16×16 viewBox, `fill="none"`, `stroke="currentColor"`,
  `stroke-width="1.5"`, round caps and joins, sized `1em`, and
  `aria-hidden="true"` with `focusable="false"`.
- Export `type IconName` as a union of `check-circle`, `alert-triangle`,
  `circle`, `x-circle`, `info`, `chevron-down`, `folder`, `copy`, `x`, and
  `keyboard`. U3–U6 use these; add names here only.
- The meaning always sits in adjacent text. Never use Unicode glyphs (`▾`,
  `✓`, `⚠`, `×`) or emoji as icons.

**`Banner` look.**

- `--surface` fill with a 1 px border in the tone color (`--accent` for info,
  `--warn`, or `--danger`), `--radius`, and padding `--space-2 --space-3`.
- A tone icon (`info`, `alert-triangle`, or `x-circle`) in the tone color,
  then the message in `--text`, then the action as a secondary `Button`, then
  a quiet dismiss `Button` with the `x` icon and the accessible name "Dismiss",
  when `onDismiss` is given.
- The tone is also given in visually hidden text before the message:
  "Information:", "Warning:", or "Error:".
- There is no tinted fill and no colored side border thicker than 1 px.
- The container is `role="alert"` for error. Info and warn banners are **not**
  live regions (no `role="status"`, no `aria-live`): a region that mounts
  with its text already in it is not reliably announced. The view that shows
  an info or warn banner announces its text through its own always-mounted
  announcer instead (see "The announcement" below).
- The **Details** disclosure that command-error banners carry is passed as
  `Banner` children; its style (`.banner__details`) lives in `controls.css`
  with the rest of `Banner`, because U4 and U5 reuse it.
- Banners sit in one container directly below the header, in a **fixed
  order**, at most one of each: command error, changed on disk, state
  warning. A fixed order is predictable; nothing is ordered by time. A new
  command error replaces the previous one. The changed-on-disk banner has no
  dismiss; it clears on reload.

### Behaviour

- **Header:**
  - shows the manifest `name` as the view heading;
  - shows the manifest path below it in `--font-mono`, `--text-muted`, and
    middle-truncated with the full path in `title`;
  - has these actions, as secondary `Button`s: **Open…**, **Recent** (a menu
    button with the `chevron-down` icon after its label, never a `▾` glyph,
    listing `recentManifests()`; hidden when empty; it opens on Enter, Space,
    or Down, arrows move through it, and Escape closes it and returns focus),
    **Reload**, and **Edit in editor**. All stay enabled when the manifest
    has errors.
  - The **Recent** menu floats over the content below it: `--surface`, a
    1 px `--border`, `--radius`, `--space-1` padding, and `--shadow-overlay`.
    Items are paths in `.mono` at `--font-size-sm` and wrap
    (`overflow-wrap: anywhere`) rather than truncate.
- **No-manifest state** (`session.manifest === null`, or the first
  `session()` call rejected; see "Other states"). Top to bottom: the view
  heading `h1` "Tar Packager", the banners container (above), then one
  sentence, "Open a manifest to list the files this package needs.", and two
  buttons:
  - **Open manifest…** (primary): an open dialog filtered to `*.toml`;
  - **Create from example…** (secondary): a save dialog, then
    `createManifestFromExample`.

  This state is the first-run screen, so it teaches: under the buttons, one
  `--text-muted` line reads "A manifest is a TOML file that lists each file,
  its Linux path, and its permissions." Nothing else: no illustration and no
  card.

#### The error region (`ManifestErrors`), when `manifest.errorCount > 0`

One block directly below the banners and above the table: a plain `<div>`,
not a landmark (the report body is the landmark, so no two landmarks share a
name). It has two parts: a **notice** that is always visible
while errors exist, and a **report body** the user can collapse. The whole
region uses the Banner border treatment: `--surface` fill, a 1 px `--danger`
border, `--radius`, and padding `--space-2 --space-3`. There is no tinted
fill and no side stripe thicker than 1 px.

- **Notice** (the visible notification):
  - the `x-circle` icon in `--danger`, `aria-hidden`;
  - a heading, one level below the view heading, at `--font-size-lg` in
    `--text`, with the count in `.num`: "1 error in this manifest" /
    "{errorCount} errors in this manifest";
  - one consequence line in `--text` at `--font-size-md`, chosen in this
    order:
    - `entriesWithheld`: "No files can be listed or built until the manifest
      errors are fixed.";
    - `failedEntries` non-empty: "1 file is left out of the list and the
      archive until it's fixed." / "{failedEntries.length} files are left out
      of the list and the archive until they're fixed.";
    - otherwise (only manifest-level errors that hide nothing, such as a bad
      `name`): "Every file is listed and can still be built.";
  - two secondary `Button`s:
    - a toggle, **Hide errors** / **Show errors**, with the `chevron-down`
      icon after the label (rotated 180° when expanded, with no transition),
      `aria-expanded`, and `aria-controls="manifest-error-report"`. It only
      expands or collapses; it does not move focus;
    - **Edit in editor** (`onEdit`, the same handler as the header's).
  - The notice's meaning is carried by its words. Never write that errors
    stop or block the build: they don't.
- **Report body** (`id="manifest-error-report"`), below the notice after a
  1 px `--border` rule:
  - a `role="region"` with `aria-labelledby` pointing at the notice heading,
    and `tabIndex={0}`, because it may scroll. When collapsed it stays
    mounted with the `hidden` attribute, so the toggle's `aria-controls`
    always points at an element. Max height `40vh`, then it
    scrolls inside itself, so a manifest with many failures never pushes the
    table off the screen;
  - **Whole manifest**, first, only when `errors` is non-empty: an `h3`
    "Whole manifest" and a `DiagnosticList` of `errors` (label "Whole
    manifest errors");
  - **Files with errors**, only when `failedEntries` is non-empty: an `h3`
    "Files with errors" and a `FailureList` of `failedEntries` with
    `headingLevel={4}`.

  Heading levels: the view heading is `h1`, the notice heading `h2`, the
  groups `h3`, and each failed entry `h4`. Sub-headings are at
  `--font-size-md`, weight 600.
  - Items are not tab stops. The body is one tab stop; screen-reader users
    move through it by heading and list.
- **Expanded by default.** Every session that results from open, reload, or
  the first `session()` restore, and has `errorCount > 0`, sets the report
  body expanded, even if the user collapsed it before. Results of other
  commands (assign, clear, drop, format, output, build) leave it as the user
  set it.
- **No focus stealing.** Opening or reloading a manifest with errors never
  moves focus on its own; focus stays on the control the user used.
- When a reload brings `errorCount` to 0, the region disappears.

**`FailureList`** renders a `<ul>` with one `<li>` per failure, in the order
given:

- a heading at `headingLevel` naming the entry: the `id` in `--font-mono`
  when present, else "Entry #{index}" (index in `.num`);
- after it, one `--text-muted` line at `--font-size-sm`: "source {source}"
  (source in `--font-mono`) when `source` is non-null, then "[[file]] on line
  {line}" (`[[file]]` in mono), joined by " · " with the separator
  `aria-hidden`. With `source` null, only the line part;
- a `DiagnosticList` of **every** error in `errors`, none hidden behind a
  "more" link.

**`DiagnosticList`** renders a `<ul aria-label={label}>`. Each item is
`{line}:{col}` in `--font-mono` `--text-muted` (`.num`), then the `message`
verbatim in `--text`. The visible `12:5` is `aria-hidden`, and visually hidden
text reads "Line 12, column 5:" before the message, so a screen reader does
not say "12 colon 5". Messages, long ids, and source names wrap
(`overflow-wrap: anywhere`); they are never truncated.

#### The announcement (screen-reader notification)

The announcer and the error banner (`role="alert"`) are the view's only live
regions. The error region, the info and warn banners, and (in U4) the drop
result are not live regions; their text reaches screen readers through
`announce()`.

- An always-mounted, visually hidden `role="status"` (`aria-live="polite"`)
  announcer in `TarpackView`. It is in the DOM from the first render,
  including the loading state, so the first message is announced. Do not make
  the error region the live region: it mounts and unmounts.
- After every open, reload, and the first `session()` restore:
  - `errorCount > 0`: "{name} has 1 error." / "{name} has {errorCount}
    errors.", followed by one of:
    - `entriesWithheld`: "No files can be built until the manifest errors are
      fixed.";
    - `failedEntries` non-empty: "1 file will be left out of the archive." /
      "{n} files will be left out of the archive.";
    - otherwise nothing more.

    Announce after **every** such result, even when the text matches the last
    one, because the user asked for the reload: clear the announcer, then set
    the text on the next frame.
  - A reload that takes `errorCount` from above 0 to 0: "{name} reloaded. No
    errors."
  - Otherwise, announce nothing for the manifest itself.
- **Changed on disk:** when the banner goes from hidden to shown, announce
  "The manifest changed on disk." Further events while it is already shown
  announce nothing.
- **State warning:** when a session result carries a `stateWarning` that is
  not already on screen, announce its text. When the same result also has an
  error sentence (for example the first restore), make **one** `announce()`
  call with the state warning first, then the error sentence, separated by a
  space, in the banners' visual order. Two calls in a row would lose the
  first, because `announce()` clears and sets on the next frame.
- A command error banner is `role="alert"` and announces itself; do not also
  pass it to `announce()`.
- Do not announce the error count on assign, clear, drop, or format changes;
  the error count does not change there. (U2 has none of these flows; U4
  tests that assign and clear stay silent.)

#### Warnings

Whenever `warnings` is non-empty, **including when there are errors**, show a
collapsed `<details>` below the error region, headed "1 warning" / "N
warnings" with the `alert-triangle` icon in `--warn`, and a `DiagnosticList`
of `warnings` (label "Warnings"). Warnings never fail an entry or affect the
build. The backend emits three kinds; show each `message` exactly as given:

- two entries share a `source` name (a drop cannot tell them apart);
- a stored path is **100 bytes or longer**: the message names the entry, says
  "100 bytes or longer", and gives the byte count of the stored path including
  its leading `/` (for example 112 bytes). The item must wrap, not truncate,
  so the byte count stays visible;
- the manifest lists no files.

#### Other states

- **Changed on disk.** An `onManifestChanged` event shows a warn `Banner`:
  "The manifest changed on disk.", with the action **Reload**. It clears after
  a reload. This is the usual way the user gets from fixing an error in their
  editor to a clean manifest.
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
  - **The first `session()` call rejects** (for example `Io`): there is no
    session to show, so render the **no-manifest state** with the error
    banner in the banners container, below the `h1` and above the sentence
    and buttons. Clear `aria-busy`. **Open manifest…** and **Create from
    example…** work as usual; a successful one replaces the state and clears
    the banner. There is no Recent menu here (it lives in the header). Never
    leave the error banner alone on an otherwise empty view.
- **Normalising rejections.** `src/lib/tarpack.ts` passes rejections through
  unchanged, so a rejection may not be a `TarpackError` (a Tauri IPC failure,
  a thrown `Error`). Every `catch` in the view passes the value through
  `toTarpackError` (below) before storing it.

### Error copy (you create `errorMessages.ts`)

Create `src/tools/tarpack/errorMessages.ts` with all 17 kinds now; later tasks
(assignment, build) only import it. Type the table as
`Record<TarpackErrorKind, (source: string | null) => string>`, importing
`TarpackErrorKind` from `src/lib/generated/`, so the compiler rejects a missing
or extra kind; do not add a default or "any other" branch. `errorMessage`
looks up `entryId` in `session.manifest.entries` and passes that entry's
`source`, or `null` when `entryId` is omitted or unknown; a `{source}`
placeholder then reads "A file". (The ids of failed entries are not in
`entries`, so they read "A file" too.)

The same module exports `toTarpackError(e: unknown): TarpackError`. It
returns `e` unchanged when `e` is an object whose `kind` is an own key of the
`messages` Record (`Object.hasOwn(messages, kind)`), whose `message` is a
string, and whose `entryId` is a string or absent. Anything else becomes
`{ kind: "Io", message: e instanceof Error ? e.message : String(e) }`. Key the
check off the Record: do not write a second list of the 17 kinds anywhere.
This guard is temporary. Normalising rejections belongs in `lib/` (the
backend implementer's side); when `lib/tarpack.ts` guarantees `TarpackError`,
the guard is removed. Until then, U4 and U5 import this function and write no
guard of their own.

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

`NoEntries` is defensive: the build bar (U5) disables the build when no file
passed, so it should not be seen in normal use.

`ManifestChangedOnDisk` is raised both when the manifest's hash differs from
the loaded one and when the manifest can no longer be read at build time
(deleted, locked). The copy above fits both; if the file is gone, **Reload**
then reports `ManifestUnreadable` with its own copy.

- **Loading:** while the first `session()` call is pending, show a quiet
  skeleton: two `--surface-sunken` bars in the header's place, with no
  spinner and no shimmer animation. It must not flash for fast loads; delay it
  by about 150 ms. Give it `aria-busy="true"` on the view and the visually
  hidden text "Loading manifest".
- `session.stateWarning`, when set, shows an info `Banner` with its text (for
  example that saved locations were reset, or that the last manifest could
  not be reopened at startup; several warnings arrive joined in one string).

### Stylesheets

- Styles for this tool's components live in `src/styles/tarpack.css`,
  imported from `src/App.tsx` after `controls.css`. Every selector in it is
  scoped under the view root class `.tarpack` (for example
  `.tarpack .menu`, `.tarpack .empty`, `.tarpack .skeleton`,
  `.tarpack .banners`), so its generic class names cannot reach another
  tool. The view's root `<section>` carries `tool-view tarpack` in **every**
  state, including loading, no-manifest, and a failed restore.
- Shared components (`Button`, `Banner`, including `.banner__details`) are
  styled in `src/styles/controls.css`.
- Utilities (`.num`, `.mono`, `.visually-hidden`) live in
  `src/styles/base.css`.
- Later tasks add their tool styles to `tarpack.css` under the same scope,
  and shared-component styles to `controls.css`.

**Rules that bind this task.**

- Tokens only, no hex. Font sizes only from `--font-size-*`.
- Every button is a `Button`, and every icon is an `Icon`.
- Everything is keyboard reachable and labelled.
- Status is never conveyed by color alone.
- Plain, short copy. Say "error" for errors and "warning" for warnings; do
  not use "problem".
- Never write that errors block or prevent building.
- Call only `lib/` functions. Never touch the filesystem or invoke Tauri
  directly.
- Never rewrite, split, or strip a backend `message`.

## Files

- `src/tools/tarpack/TarpackView.tsx`, `ManifestHeader.tsx`,
  `ManifestErrors.tsx`, `FailureList.tsx`, `DiagnosticList.tsx`,
  `errorMessages.ts`
- `src/app/Banner.tsx`, `src/app/Button.tsx`, `src/app/icons.tsx`
- `src/styles/tokens.css` (the new tokens), `src/styles/base.css` (the browser
  surfaces, `.num`, `.mono`, `.visually-hidden`, and body size),
  `src/styles/app.css` (the heading size), `src/styles/controls.css` (new,
  imported from `src/App.tsx`: `Button`, `Banner`, `.banner__details`), and
  `src/styles/tarpack.css` (new, imported from `src/App.tsx` after
  `controls.css`; every selector scoped under `.tarpack`)
- The root `DESIGN.md`: change the "Planned (U2)" markers to "Landed (U2)" for
  what you built. Change nothing else in it.
- Tests next to each

## Skill

`/impeccable layout`, then `/impeccable clarify` for all copy. Finish with
`/impeccable audit` on the error region, the warnings, and the announcer, and
fix every P0 and P1 finding.

How to run it:

- Start with the skill's `impeccable context`, which loads the root
  `PRODUCT.md` and `DESIGN.md`.
- This is an Operate surface extending an established world, so run no
  concept round and write no direction contract.
- Read the skill's `reference/craft-floor.md` before the first edit.
- If the skill is not installed, install it with `npx impeccable install`, or
  follow the named commands' reference docs from
  `github.com/pbakaus/impeccable` by hand. Say which in your report.

## Acceptance criteria

- `--font-size-sm/md/lg/xl` and `--shadow-overlay` exist in `tokens.css`. No
  `font-size` under `src/` outside `tokens.css` uses a literal value.
- No `<button>` element is written outside `Button.tsx`, except U1's
  `ToolNav.tsx`, whose nav items keep their own style. No `▾`, `✓`, `⚠`, or
  `×` character appears in any `.tsx` file.
- `Button` renders each variant, forwards `ref` and `disabled`, is at least
  24×24 px, and keeps the focus ring. Busy sets `aria-busy` and disables it.
- A `Banner` of each tone has a 1 px tone border, no tinted fill, and the
  tone word in its accessible text. Error is `role="alert"`; info and warn
  have no live role.
- Banners render in the fixed order command error, changed on disk, state
  warning.
- The changed-on-disk banner appearing, and a newly shown state warning, are
  announced through the announcer (one call when the state warning and an
  error sentence arrive together).
- A rejected first `session()` call shows the no-manifest state (heading,
  sentence, **Open manifest…**, **Create from example…**) with the error
  banner below the heading; `aria-busy` is cleared, and Open from there
  loads a manifest and clears the banner.
- A rejection that is not a `TarpackError` shows the `Io` copy, with the
  thrown text under Details.
- The Recent menu uses `--shadow-overlay`; nothing else in this task has a
  `box-shadow`.
- Every selector in `tarpack.css` starts with `.tarpack`; `.mono` and `.num`
  are in `base.css`; `.banner__details` is in `controls.css`.
- `actionsRef` is optional and is passed only by tests.
- Every state above renders from a `TarpackSession` fixture.
- **Open…** calls `openFileDialog` with a `.toml` filter, then `openManifest`.
  A cancelled dialog does nothing.
- The banner appears on a `manifest-changed` event, and Reload clears it.
- **The error state is keyed on `errorCount`.** A fixture with `errors: []`
  and one failed entry shows the notice and report; a fixture with
  `errorCount: 0` shows neither.
- The notice shows the exact count ("1 error" / "N errors") and the
  consequence line for each case: entries withheld, failed entries, and
  manifest-level errors only. No string in the region says errors block or
  prevent building.
- The report lists the Whole manifest group (when `errors` is non-empty),
  then one group per failed entry in the given order. A group is named by
  `id`, or "Entry #{index}" when `id` is `null`, with "source {source}" when
  present and the `[[file]]` line; every one of its errors is listed with
  `line:col` and the verbatim message.
- Each error item's accessible text starts "Line {line}, column {col}:"; the
  visible `line:col` is `aria-hidden`.
- The toggle has `aria-expanded` and `aria-controls`, works with Enter and
  Space, and the body is expanded after each open, reload, or restore with
  errors, even if it was collapsed before.
- `showErrors()` expands the body and moves focus to it. Opening or reloading
  never moves focus by itself.
- The announcer is in the DOM before the first session arrives, announces the
  error count and consequence after open, reload, and restore, repeats on a
  reload with the same count, announces "No errors." when a reload clears
  them, and says nothing on assign, clear, or drop.
- Warnings render when errors are also present, collapsed.
- A report with 200 failed entries stays within `40vh`, scrolls inside its
  region, and the region is keyboard scrollable.
- `errorMessages.ts` is a `Record` over `TarpackErrorKind` with no fallback
  branch; removing any key fails `npm run typecheck`, and it has `NoEntries`
  and no `ManifestInvalid`.
- `ManifestUnreadable` on Open or Reload, and `PathExists` on Create from
  example, show their messages in an error banner with a Details disclosure
  that is keyboard operable; the prior state stays usable.
- Warning and error messages render verbatim; a long-name warning's byte count
  is visible at 800 px and at 200% text size.
- The header, error region, and report wrap cleanly at 800 px wide and at
  200% text size, with no horizontal page scroll.

## Tests proving completion

`npm run test`, with `src/lib/tarpack` and `src/lib/tauri` mocked:

- `TarpackView.states.test.tsx`: loading, no-manifest, loaded, errors with
  entries shown, errors with entries withheld, and changed-on-disk;
  `ManifestUnreadable` from Open and Reload; `PathExists` from Create from
  example; a rejected first `session()` (`Io`) showing the no-manifest state
  with the error banner and working Open / Create buttons; a non-`TarpackError`
  rejection showing the `Io` copy; the banner order with all three banners
  shown; the root `<section>` having the `tarpack` class in the loading,
  no-manifest, and loaded states.
- `TarpackView.errors.test.tsx` (calls `showErrors()` and `announce()`
  through `actionsRef`):
  - the announcer text after open, reload, and restore with errors (withheld,
    failed entries, manifest-level only); the repeat on a same-count reload;
    "No errors." on a reload to 0;
  - the announcer text for the changed-on-disk banner appearing (once, not
    again on a second event), and for a restore that has both a
    `stateWarning` and errors (one combined text);
  - `announce(text)` setting the announcer text;
  - the report re-expanding after a reload when the user had collapsed it;
  - `showErrors()` expanding the body and moving focus to it; doing nothing
    when `errorCount` is 0;
  - **no focus change on open or reload:** focus **Open…** (or **Reload**),
    trigger it, let a session with errors resolve, and assert
    `document.activeElement` is still that button and the report body is not
    focused;
  - **200 failed entries:** a fixture with 200 `failedEntries` renders all
    200 entry headings and their items; the report body is the only element
    in the region with `tabIndex=0` (no descendant is a tab stop), and it
    carries the scrolling class (`error-region__report`, whose rule in
    `tarpack.css` sets the `40vh` max height and `overflow-y: auto`).
- `ManifestHeader.test.tsx`: each action calls the right `lib` function;
  Recent is hidden when empty; and the Recent menu opens, moves, and closes
  with Escape, returning focus by keyboard.
- `Button.test.tsx`: variants, `disabled`, `busy`, `ref` forwarding, and an
  axe check.
- `Banner.test.tsx`: `role="alert"` for error and no live role for info and
  warn; each tone's hidden tone word; dismiss present only with `onDismiss`;
  children (the Details disclosure) rendered.
- `tokens.test.ts` (extend U1's): the four size tokens and `--shadow-overlay`
  exist.
- `errorMessages.test.ts`: a non-empty message for each of the 17 kinds
  (listed explicitly in the test, including `NoEntries`), `{source}` filled
  from `entryId`, and "A file" when `entryId` is omitted or unknown.
  `npm run typecheck` proves exhaustiveness. `toTarpackError`: a valid
  `TarpackError` passes through unchanged; an unknown `kind`, a missing
  `message`, a string, an `Error`, and `undefined` each become `Io` with the
  thrown text as `message`.
- `tokens.test.ts` also reads `tarpack.css` (`?raw`) and asserts every
  selector starts with `.tarpack`, and that `box-shadow` appears only on the
  Recent menu rule.
- `ManifestErrors.test.tsx`: the count heading (singular and plural); each
  consequence line; the Whole manifest group; the toggle; warnings alongside
  errors; and a long-name warning whose message (with its byte count) renders
  verbatim.
- `FailureList.test.tsx` and `DiagnosticList.test.tsx`: a failure with an
  `id`, one with `id: null` and a `source`, and one with both `null`; every
  error of a multi-error entry listed; the "Line N, column M:" accessible
  text; empty input renders nothing.
- Axe checks on each state, including the report expanded and collapsed,
  and explicitly on: loading (after the 150 ms delay, skeleton shown);
  changed-on-disk; a command error banner with Details closed and with
  Details open; and the no-manifest state after a failed restore.

## States covered

Loading, no-manifest, no-manifest after a failed restore, loaded, errors
with entries shown (expanded and collapsed), errors with entries withheld,
manifest-level errors only, warnings only, errors plus warnings,
changed-on-disk, state warning, and command error (`ManifestUnreadable`,
`PathExists`, `OpenerFailed`, `Io`, and a non-`TarpackError` rejection).

## Out of scope

- The entry table, including its empty and withheld messages (U3).
- Drops, assign, and clear (U4), including the test that they do not
  announce the error count.
- The build bar, its route to the report, and the build's final report (U5).
- Keyboard shortcuts, including the one that shows the errors (U6).
