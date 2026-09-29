# U2 — Manifest header, empty state, invalid state, changed-on-disk banner

Status: awaiting approval
Project: tarpack   Depends on: U1 (landed), M6 (landed)

## Goal

Let the user open, reload, and edit a manifest, and see clearly when none is
loaded, when it is invalid, or when it changed on disk. Along the way, create
the shared button, icon, and type vocabulary that every later task uses.

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

**Design context.** The root `PRODUCT.md` records the product. The root
`DESIGN.md` records the visual system U1 built: "The Packing List". It is a
quiet, dense, restrained palette, with tonal depth, no shadows at rest, system
UI type, and mono for data. This is an **Operate** surface that extends the
established world. Invent no new visual identity and do not rewrite
`DESIGN.md`, except to change its "Planned (U2)" markers to landed once you
build them.

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
| `Banner` | `src/app/Banner.tsx` (shared) | `tone: "info"\|"warn"\|"error", message, action?: { label, onAction }, onDismiss?` | info / warn / error |
| `Button` | `src/app/Button.tsx` (shared) + `src/styles/controls.css` | native `<button>` props, plus `variant: "primary"\|"secondary"\|"quiet"` (default `"secondary"`) and `icon?: IconName` | default / hover / active / focus / disabled / busy |
| `Icon` | `src/app/icons.tsx` (shared) | `name: IconName` | — |

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
| `--font-size-lg` | `1rem` | region headings ("This manifest has N problems") |
| `--font-size-xl` | `1.25rem` | the view heading |

Also add `--shadow-overlay`: `0 8px 24px rgb(0 0 0 / 0.18)` in light, and
`0 8px 24px rgb(0 0 0 / 0.5)` in both dark blocks. U5 and U6 use it for the
dialog and popover; nothing else has a shadow.

Point `body` in `base.css` at `--font-size-md`, and `.tool-view h1` in
`app.css` at `--font-size-xl`, so the values have one home.

**Browser surfaces** (`src/styles/base.css`):

- `::selection` uses `--accent-text` on `--accent`;
- `html` sets `scrollbar-color: var(--border) var(--surface-sunken)`. Do not
  build custom scrollbars;
- a `.num` utility class sets `font-variant-numeric: tabular-nums`. Use it for
  counts ("3 problems", "N warnings").

**`Button`.** It wraps a native `<button type="button">` and forwards every
prop and ref. Every button in the app uses it. Styles go in
`src/styles/controls.css`, imported from `App.tsx`:

- **Shape:** `--radius`, 1 px border, font `inherit` at `--font-size-md`,
  padding `--space-1 --space-3`, a minimum 24×24 px target, and an icon gap of
  `--space-1`.
- **primary:** `--accent` fill and border, `--accent-text` label. There is one
  per region; in this task that is **Open manifest…** in the empty state.
- **secondary:** `--surface` fill, `--border` outline, `--text` label. This is
  the default, used for the header actions.
- **quiet:** transparent with a transparent border; `--surface-sunken` on
  hover. Use it for Dismiss and the Details disclosure toggle.
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
- The container is `role="status"` for info and warn, and `role="alert"` for
  error.
- Banners stack above the table, newest first. The changed-on-disk banner
  has no dismiss; it clears on reload.

### Behaviour

- **Header:**
  - shows the manifest `name` as the view heading;
  - shows the manifest path below it in `--font-mono`, `--text-muted`, and
    middle-truncated with the full path in `title`;
  - has these actions, as secondary `Button`s: **Open…**, **Recent** (a menu
    button with the `chevron-down` icon after its label, never a `▾` glyph,
    listing `recentManifests()`; hidden when empty; it opens on Enter, Space,
    or Down, arrows move through it, and Escape closes it and returns focus),
    **Reload**, and **Edit in editor**.
- **No-manifest state** (`session.manifest === null`): one sentence, "Open a
  manifest to list the files this package needs.", and two buttons:
  - **Open manifest…** (primary): an open dialog filtered to `*.toml`;
  - **Create from example…** (secondary): a save dialog, then
    `createManifestFromExample`.

  This state is the first-run screen, so it teaches: under the buttons, one
  `--text-muted` line reads "A manifest is a TOML file that lists each file,
  its Linux path, and its permissions." Nothing else: no illustration and no
  card.
- **Invalid state** (`manifest.errors.length > 0`):
  - an error-tone region headed "This manifest has N problems" at
    `--font-size-lg`, with the `x-circle` icon and the count in `.num`. It
    uses the Banner border treatment, with no tinted fill;
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
  skeleton: two `--surface-sunken` bars in the header's place, with no
  spinner and no shimmer animation. It must not flash for fast loads; delay it
  by about 150 ms. Give it `aria-busy="true"` on the view and the visually
  hidden text "Loading manifest".
- `session.stateWarning`, when set, shows an info `Banner` (for example that
  saved locations were reset).

**Rules that bind this task.**

- Tokens only, no hex. Font sizes only from `--font-size-*`.
- Every button is a `Button`, and every icon is an `Icon`.
- Everything is keyboard reachable and labelled.
- Status is never conveyed by color alone.
- Plain, short copy.
- Call only `lib/` functions. Never touch the filesystem or invoke Tauri
  directly.

## Files

- `src/tools/tarpack/TarpackView.tsx`, `ManifestHeader.tsx`,
  `ManifestErrors.tsx`, `errorMessages.ts`
- `src/app/Banner.tsx`, `src/app/Button.tsx`, `src/app/icons.tsx`
- `src/styles/tokens.css` (the new tokens), `src/styles/base.css` (the browser
  surfaces and body size), `src/styles/app.css` (the heading size), and
  `src/styles/controls.css` (new, imported from `src/App.tsx`)
- The root `DESIGN.md`: change the "Planned (U2)" markers to "Landed (U2)" for
  what you built. Change nothing else in it.
- Tests next to each

## Skill

`/impeccable layout`, then `/impeccable clarify` for all copy.

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
- A `Banner` of each tone has a 1 px tone border, no tinted fill, the tone
  word in its accessible text, and the right role.
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
- `ManifestHeader.test.tsx`: each action calls the right `lib` function;
  Recent is hidden when empty; and the Recent menu opens, moves, and closes
  with Escape, returning focus by keyboard.
- `Button.test.tsx`: variants, `disabled`, `busy`, `ref` forwarding, and an
  axe check.
- `Banner.test.tsx`: each tone's role and hidden tone word; dismiss present
  only with `onDismiss`.
- `tokens.test.ts` (extend U1's): the four size tokens and `--shadow-overlay`
  exist.
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
