# U3 — Entry table with per-row status

Status: awaiting approval
Project: tarpack   Depends on: U2 (landed), M6 (landed; this task reads its
partial-results fields `entriesWithheld` and `failedEntries`)

## Goal

Show every manifest entry with its expected file, the Windows file assigned to
it, its Linux target, its permissions, its owner, whether its line endings are
converted, and whether it is ready, and say plainly when files are left out
because of manifest errors. This table is the heart of the view.

## Context

**Why it matters.** The failure this tool prevents is finding out on the Linux
machine that a file was missing or had the wrong permissions. The table must
make "is everything here and correct?" answerable at a glance. It shows the
exact Linux path and mode that will be written, and which files will have
their Windows line endings converted.

**Line-ending conversion (decided).** By default every file is copied byte for
byte. A manifest entry may opt in with `normalize_eol = true`; the archive
writer then replaces every CRLF pair with LF for that file (lone CR bytes are
kept). This changes file contents, so it must be visible on the row, not only
in the manifest. Typical use: a shell script edited on Windows.

**A manifest with errors still opens (decided by the human).** Entries whose
`[[file]]` table has no error **pass** and are listed in this table as usual.
Entries with errors **fail**: they are not in `entries`, they will be left out
of the archive, and U2 lists them, with every error, in an error report above
the table. Some manifest-level errors (TOML syntax, `version`, `[defaults]`,
…) **withhold** every entry, and `entries` is then empty. Errors do not block
the build: the archive is built from the passed entries, and the build's
result lists what was left out. This table therefore has to be honest about
what it is *not* showing: its summary never says "All … ready" while files
are left out, and an empty table says why it is empty. Failed entries are
never rows.

**What exists.**

- `src/tools/tarpack/TarpackView.tsx` (U2) holds the `TarpackSession` state,
  renders the error region (notice and report) above the table slot, and
  reserves a slot for the table.
- `session.manifest.entries` holds the **passed** entries only, in manifest
  order. It is a list of
  `{ id, source, targetPath, mode, modeText, owner, normalizeEol, assigned, status }`.
  Check `src/lib/generated/` for the exact types:
  - `status` is `"ready"`, `"missing"`, or `"unassigned"`;
  - `targetPath` is the absolute name stored in the archive, always starting
    with `/`, for example `/opt/gateway/bin/gateway`;
  - `normalizeEol` is a boolean, `true` when CRLF → LF conversion is on;
  - `modeText` is like `rwxr-xr-x`;
  - `mode` is the octal string, like `0755`;
  - `owner` is like `root:root`;
  - `assigned` is the Windows path, or `null`.
- `session.readyCount` and `session.totalCount` are also provided; both count
  passed entries only.
- `session.manifest.failedEntries` is a list with one element per failed
  entry; this task uses only its length. `session.manifest.entriesWithheld`
  is `true` when a manifest-level error hides every entry.
- The tokens you use are in `src/styles/tokens.css`: `--ok`, `--danger`,
  `--text-muted`, `--font-mono`, `--surface`, `--surface-sunken`, `--border`,
  `--font-size-sm` and `--font-size-md`, and the `--space-*` scale.
- U2 built the shared vocabulary; use it and add no parallel version:
  - `src/app/Button.tsx`: `variant: "primary" | "secondary" | "quiet"`,
    `icon?`;
  - `src/app/icons.tsx`: `Icon` with `name: IconName`, including
    `check-circle`, `alert-triangle`, and `circle`;
  - the `.num` (tabular numerals) and `.mono` (`--font-mono`) utility
    classes in `src/styles/base.css`.
- **Stylesheets.** Tool styles go in `src/styles/tarpack.css`, and every
  selector there is scoped under the view root class `.tarpack` (for example
  `.tarpack .entry-table`). Shared component styles go in
  `src/styles/controls.css`. Nothing in this task has a shadow.
- **Design context.** The root `DESIGN.md` records the visual system ("The
  Packing List"). This table is its signature component: the list of what
  goes in the crate. It is an Operate surface, dense and scannable, and never
  decorated. It uses no cards, no zebra striping in status colors, and no
  metric tiles.

### Components

| Component | Path | Props | States |
| --- | --- | --- | --- |
| `EntryTable` | `src/tools/tarpack/EntryTable.tsx` | `entries, failedCount, entriesWithheld, onBrowse(id), onClear(id)` | populated / populated with files left out / no files / every file failed / withheld |
| `EntryStatus` | `src/tools/tarpack/EntryStatus.tsx` | `status` | Ready / Missing / Not assigned |
| `EolMarker` | `src/tools/tarpack/EolMarker.tsx` | — | rendered only for entries with `normalizeEol` |

**Columns, in order:**

1. **Status**: an `Icon` plus a word. Ready uses `--ok` with `check-circle`.
   Missing uses `--danger` with `alert-triangle`. Not assigned uses
   `--text-muted` with `circle`. The word is always visible, in the same
   color as its icon (each pair passes AA on `--surface`); the icon is
   `aria-hidden`. Reserve the column's width for the longest word ("Not
   assigned") so that status changes cause no layout shift.
2. **File**: the expected `source` name. When `normalizeEol` is true, an
   `EolMarker` follows the name on the same line (wrapping below it if space
   runs out):
   - visible text **CRLF → LF** in `--font-mono`, at the table's body size,
     `--text-muted` on `--surface-sunken`, a 1 px `--border` outline,
     `--radius`, and `--space-1` horizontal padding (this pair is already
     verified for WCAG AA);
   - the arrow is decorative: wrap it so the accessible text is not "CRLF
     right arrow LF". The marker's accessible text is "line endings converted
     to LF" (visually hidden), with the visible glyphs `aria-hidden`;
   - `title` reads "Windows line endings (CRLF) are converted to Linux (LF)
     when the archive is written";
   - it is not interactive and not in the tab order; screen readers reach it
     as part of the File cell. Its meaning never depends on color.
3. **Windows location**: `assigned` in `--font-mono`, middle-truncated (keep
   the drive and the file name visible), with the full path in `title` and in
   visually hidden text. Show "—" when unassigned. When the status is Missing,
   add the text "(not found)".
4. **Linux target**: `targetPath` in `--font-mono`, rendered verbatim
   including its leading `/`. Do not strip, join, or rebuild it.
5. **Mode**: `modeText` in `--font-mono`, followed by the octal value in
   `--text-muted`.
6. **Owner**: `owner`.
7. **Actions**: **Browse…** and **Clear**, as quiet `Button`s. Clear is
   disabled when unassigned. In this task the buttons call the `onBrowse` and
   `onClear` props, which U4 wires to `lib`.

**Typography and rhythm** (`/impeccable typeset`):

- Cells are `--font-size-md`, and column headers are `--font-size-sm` at
  weight 600 in `--text-muted`, in sentence case (not uppercase or tracked).
- Mono is only for columns 3–5 and the marker: data a user might paste.
- The octal mode and the summary counts use `.num`.
- Rows are padded `--space-2 --space-3`, with a 1 px `--border` rule between
  rows and no vertical rules.
- Row hover is `--surface-sunken`. There is no hover on the header row.
- Headers are sticky on `--surface`, with a 1 px bottom rule.

**Names that are not valid UTF-8 or are very long** (`/impeccable harden`):

- Every name you receive is a display string. It may contain U+FFFD (�)
  where the Windows or manifest name was not valid UTF-8. Render it as
  given; never try to repair it.
- Middle truncation works on code points (`Array.from`), never on UTF-16
  units, so it never splits a surrogate pair or a U+FFFD. It keeps the drive
  and the full file name. When the file name alone is too long, it keeps the
  last 24 code points of it.
- The File column wraps long source names (`overflow-wrap: anywhere`), and
  never truncates them.

**Scale.** Manifests typically list 1–50 files and can list about 2,000.
- Render every row, with no virtualization below 2,000. Memoize each row by
  `id` and entry value, so that one assignment re-renders one row, not the
  table.
- Use no measurement in render: truncation is computed from string length
  and CSS, never `getBoundingClientRect` in a loop.

`TarpackView` passes `failedCount = manifest.failedEntries.length` and
`entriesWithheld = manifest.entriesWithheld`.

Above the table, a summary line:

- "5 of 6 files ready", or "All 6 files ready" only when every listed entry
  is ready **and** `failedCount` is 0. With `failedCount > 0`, never say
  "All": "6 of 6 files ready".
- When `failedCount > 0`, append " · 1 file left out (errors)" or
  " · N files left out (errors)", so the count of listed files is never
  mistaken for the whole manifest.
- When at least one entry has `normalizeEol`, append " · 1 file converts
  line endings to LF" (or "N files …"), so the conversion is announced once
  without scanning every row.

The summary line is a sentence at `--font-size-md`, not a big-number tile.
Separators " · " are `aria-hidden`; the clauses read as one sentence.

When `entries` is empty, render no `<table>` and no summary. Show one sentence
at `--font-size-md`, then a `--text-muted` line naming the next step. Choose
the case in this order:

- `entriesWithheld`: "No files are listed. A manifest error hides them until
  it's fixed." / "The errors are listed above. Fix them in your editor, then
  Reload.";
- otherwise `failedCount > 0`: "No files are listed. Every file in this
  manifest has errors." / "The errors are listed above. Fix them in your
  editor, then Reload.";
- otherwise: "This manifest lists no files." / "Add a [[file]] entry to the
  manifest, then Reload." Render `[[file]]` in mono.

The error report itself is U2's and sits above; do not repeat its content or
add a second route to it here.

**Rules that bind this task.**

- Tokens only, with font sizes from `--font-size-*`. Use `Button` and `Icon`;
  no `<button>` and no glyph icons.
- Everything is keyboard reachable and labelled.
- Status is never color-only, and neither is the line-ending marker.
- No size or modified-time columns (decided: keep the table narrow).
- The table stays usable with 200 entries, and scrolls without jank at 2,000:
  sticky column headers, no layout shift when statuses change, and no
  horizontal page scroll at 800 px. The
  table may scroll horizontally inside its own container if it must, but
  prefer truncation.

## Files

- `src/tools/tarpack/EntryTable.tsx`, `EntryStatus.tsx`
- Wire the table into `TarpackView.tsx`
- Table styles in `src/styles/tarpack.css`, scoped under `.tarpack`
- Tests next to each

## Skill

`/impeccable typeset`, then `/impeccable layout`, then `/impeccable harden`
for the long-name, non-UTF-8, and scale cases.

How to run it:

- Start with the skill's `impeccable context`, which loads the root
  `PRODUCT.md` and `DESIGN.md`.
- This is an Operate surface extending an established world, so run no
  concept round and do not rewrite `DESIGN.md`.
- Read the skill's `reference/craft-floor.md` before the first edit.
- If the skill is not installed, install it with `npx impeccable install`, or
  follow the named commands' reference docs from
  `github.com/pbakaus/impeccable` by hand. Say which in your report.

## Acceptance criteria

- It is a semantic `<table>` with a `<caption>` (which may be visually hidden)
  and `<th scope="col">` headers.
- Each status's text label is present in the DOM.
- A row with `normalizeEol: true` shows the visible **CRLF → LF** marker and
  exposes "line endings converted to LF" in its accessible name; a row with
  `normalizeEol: false` shows no marker. The marker is not focusable.
- `targetPath` renders exactly as given, leading `/` included.
- Middle truncation keeps the file name visible for a 200-character path. It
  never splits a surrogate pair, and a path containing U+FFFD renders with
  it intact.
- Status words and icons use `Icon` (`check-circle`, `alert-triangle`,
  `circle`). Row actions are quiet `Button`s. Counts and the octal mode use
  `.num`.
- Changing one entry's status re-renders only that row (a render-count test
  on a 2,000-row fixture).
- The summary line is correct for mixed and all-ready cases, and adds the
  line-ending clause only when some entry has `normalizeEol`.
- With `failedCount > 0` the summary never starts with "All", and includes
  "1 file left out (errors)" / "N files left out (errors)".
- With `entries` empty, the withheld, every-file-failed, and no-files
  sentences each appear in their case, with their next-step line, and no
  `<table>` is rendered. No failed entry ever appears as a row.
- There is no horizontal page scroll at 800×560, and the layout holds at 200%
  text size.

## Tests proving completion

`npm run test`:

- `EntryTable.test.tsx`: a mixed-status fixture, the summary line (mixed,
  all ready with and without `failedCount`, and the left-out clause singular
  and plural), the three empty messages (withheld, every file failed, no
  files), the Clear-disabled state, and that each button calls its prop with
  the right id.
- `EntryStatus.test.tsx`: the label per status.
- `EolMarker.test.tsx` (or cases in `EntryTable.test.tsx`): marker present
  with its accessible text for a `normalizeEol` row, absent otherwise, and the
  summary-line clause.
- `truncateMiddle.test.ts` (the helper lives in `src/tools/tarpack/`): the
  drive and file name are kept; an emoji or astral character at the cut
  point is not split; U+FFFD is preserved; and a very long file name keeps
  its last 24 code points.
- `EntryTable.scale.test.tsx`: a 2,000-row fixture renders, and one status
  change re-renders one row.
- An axe check on a populated table that includes a `normalizeEol` row.

## States covered

Empty manifest (with its next-step line), entries withheld, every entry
failed, partial (mixed statuses), all ready, all listed files ready with files
left out, rows with and without line-ending conversion, long and non-UTF-8
names, and 2,000 rows.

## Out of scope

- The error report and error notice (U2).
- The Browse dialog, Clear, and drop wiring (U4).
- The build bar (U5).
- Row keyboard shortcuts (U6).
