# U3 — Entry table with per-row status

Status: awaiting approval
Project: tarpack   Depends on: U2 (landed), M6 (landed)

## Goal

Show every manifest entry that passed validation with its expected file, the
Windows file assigned to it, its Linux target, its permissions, its owner,
whether its line endings are converted, and whether it is ready, and say
plainly when files are left out because of errors. This table is the heart of
the view.

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
Entries with errors **fail**: they are not in `entries`, and U2 lists them,
with every error, in an error report above the table, under a notice strip
("3 errors in this manifest"). Some manifest-level errors (TOML syntax,
`version`, `[defaults]`, …) **withhold** every entry, and `entries` is then
empty. The archive cannot be created while any error exists. This table
therefore has to be honest about what it is *not* showing: its summary never
says "All files ready" while files are left out, and an empty table says why
it is empty.

**What exists.**

- `src/tools/tarpack/TarpackView.tsx` (U2) holds the `TarpackSession` state,
  renders the error report above the table slot, and reserves a slot for the
  table.
- `session.manifest.entries` holds the **passed** entries only, in manifest
  order. It is a list of
  `{ id, source, targetPath, mode, modeText, owner, uid, gid, normalizeEol, assigned, status }`
  (the backend's `EntryView` plus `assigned` and `status`).
  Check `src/lib/generated/` for the exact types:
  - `status` is `"ready"`, `"missing"`, or `"unassigned"`;
  - `targetPath` is the absolute name stored in the archive, always starting
    with `/`, for example `/opt/gateway/bin/gateway`;
  - `normalizeEol` is a boolean, `true` when CRLF → LF conversion is on;
  - `modeText` is like `rwxr-xr-x`;
  - `mode` is the octal string, like `0755`;
  - `owner` is like `root:root` (user name, colon, group name);
  - `uid` and `gid` are the numeric ids written to the archive, as numbers.
    This task's columns do not require them; the Owner column shows `owner`;
  - `assigned` is the Windows path, or `null`.
- `session.readyCount` and `session.totalCount` count the passed entries
  only.
- `session.manifest.failedEntries` is a list with one element per failed
  entry; this task uses only its length. `session.manifest.entriesWithheld`
  is `true` when a manifest-level error hides every entry.
  `session.manifest.errorCount` is above 0 whenever the manifest has errors.
- The tokens you use are in `src/styles/tokens.css`: `--ok`, `--danger`,
  `--text-muted`, `--font-mono`, `--surface`, `--surface-sunken`, `--border`,
  and the `--space-*` scale.

### Components

| Component | Path | Props | States |
| --- | --- | --- | --- |
| `EntryTable` | `src/tools/tarpack/EntryTable.tsx` | `entries, failedCount, entriesWithheld, onBrowse(id), onClear(id)` | populated / populated with files left out / no files / all files failed / withheld |
| `EntryStatus` | `src/tools/tarpack/EntryStatus.tsx` | `status` | Ready / Missing / Not assigned |
| `EolMarker` | `src/tools/tarpack/EolMarker.tsx` | — | rendered only for entries with `normalizeEol` |

**Columns, in order:**

1. **Status**: an icon plus a word. Ready uses `--ok` with a check icon.
   Missing uses `--danger` with a warning icon. Not assigned uses
   `--text-muted` with an empty-circle icon. The word is always visible; the
   icon is `aria-hidden`.
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
7. **Actions**: **Browse…** and **Clear**. Clear is disabled when unassigned.
   In this task the buttons call the `onBrowse` and `onClear` props, which U4
   wires to `lib`.

`TarpackView` passes `failedCount = manifest.failedEntries.length` and
`entriesWithheld = manifest.entriesWithheld`.

Above the table, a summary line:

- "5 of 6 files ready", or "All 6 files ready" when every listed entry is
  ready **and** `failedCount` is 0. With `failedCount > 0`, never say "All":
  "6 of 6 files ready".
- When `failedCount > 0`, append " · 1 file left out (errors)" or
  " · N files left out (errors)", so the count of listed files is never
  mistaken for the whole manifest.
- When at least one entry has `normalizeEol`, append " · 1 file converts
  line endings to LF" (or "N files …"), so the conversion is announced once
  without scanning every row.

The clauses keep that order. The summary is plain text, not a live region.

**When `entries` is empty**, show one sentence in `--text-muted` in place of
the table (no empty `<table>`, no summary line):

- `entriesWithheld`: "No files are listed. An error in the whole manifest
  hides them until it's fixed.";
- otherwise, `failedCount > 0`: "No files are listed. Every file in this
  manifest has errors.";
- otherwise: "This manifest lists no files."

The error report itself is U2's and sits above; do not repeat its content
here.

**Rules that bind this task.**

- Tokens only.
- Everything is keyboard reachable and labelled.
- Status is never color-only, and neither is the line-ending marker.
- No size or modified-time columns (decided: keep the table narrow).
- The table stays usable with 200 entries: sticky column headers, no layout
  shift when statuses change, and no horizontal page scroll at 800 px. The
  table may scroll horizontally inside its own container if it must, but
  prefer truncation.

## Files

- `src/tools/tarpack/EntryTable.tsx`, `EntryStatus.tsx`
- Wire the table into `TarpackView.tsx`
- Tests next to each

## Skill

`impeccable:typeset`, then `impeccable:layout`.

## Acceptance criteria

- It is a semantic `<table>` with a `<caption>` (which may be visually hidden)
  and `<th scope="col">` headers.
- Each status's text label is present in the DOM.
- A row with `normalizeEol: true` shows the visible **CRLF → LF** marker and
  exposes "line endings converted to LF" in its accessible name; a row with
  `normalizeEol: false` shows no marker. The marker is not focusable.
- `targetPath` renders exactly as given, leading `/` included.
- Middle truncation keeps the file name visible for a 200-character path.
- The summary line is correct for mixed, all-ready, and empty cases, and adds
  the line-ending clause only when some entry has `normalizeEol`.
- With `failedCount > 0` the summary never starts with "All", and includes
  "N files left out (errors)", singular and plural.
- With `entries` empty, the withheld, all-failed, and no-files sentences each
  appear in their case, and no `<table>` is rendered.
- There is no horizontal page scroll at 800×560, and the layout holds at 200%
  text size.

## Tests proving completion

`npm run test`:

- `EntryTable.test.tsx`: a mixed-status fixture, the summary line (including
  all-ready with and without `failedCount`, and the left-out clause), the
  three empty messages (withheld, all failed, no files), the Clear-disabled
  state, and that each button calls its prop with the right id.
- `EntryStatus.test.tsx`: the label per status.
- `EolMarker.test.tsx` (or cases in `EntryTable.test.tsx`): marker present
  with its accessible text for a `normalizeEol` row, absent otherwise, and the
  summary-line clause.
- `truncateMiddle.test.ts`: if you add a helper under `src/tools/tarpack/`.
- An axe check on a populated table that includes a `normalizeEol` row.

## States covered

Empty manifest, entries withheld, every entry failed, partial (mixed
statuses), all ready, all listed files ready with files left out, and rows
with and without line-ending conversion.

## Out of scope

- The error report and error notice (U2).
- The Browse dialog, Clear, and drop wiring (U4).
- The build bar (U5).
- Row keyboard shortcuts (U6).
