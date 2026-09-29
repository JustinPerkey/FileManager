# U3 — Entry table with per-row status

Status: awaiting approval
Project: tarpack   Depends on: U2 (landed), M6 (landed)

## Goal

Show every manifest entry with its expected file, the Windows file assigned to
it, its Linux target, its permissions, its owner, and whether it is ready. This
table is the heart of the view.

## Context

**Why it matters.** The failure this tool prevents is finding out on the Linux
machine that a file was missing or had the wrong permissions. The table must
make "is everything here and correct?" answerable at a glance. It shows the
exact Linux path and mode that will be written.

**What exists.**

- `src/tools/tarpack/TarpackView.tsx` (U2) holds the `TarpackSession` state and
  reserves a slot for the table.
- `session.manifest.entries` is a list of
  `{ id, source, targetPath, mode, modeText, owner, assigned, status }`. Check
  `src/lib/generated/` for the exact types:
  - `status` is `"ready"`, `"missing"`, or `"unassigned"`;
  - `modeText` is like `rwxr-xr-x`;
  - `mode` is the octal string, like `0755`;
  - `owner` is like `root:root`;
  - `assigned` is the Windows path, or `null`.
- `session.readyCount` and `session.totalCount` are also provided.
- The tokens you use are in `src/styles/tokens.css`: `--ok`, `--danger`,
  `--text-muted`, `--font-mono`, `--surface`, `--surface-sunken`, `--border`,
  and the `--space-*` scale.

### Components

| Component | Path | Props | States |
| --- | --- | --- | --- |
| `EntryTable` | `src/tools/tarpack/EntryTable.tsx` | `entries, onBrowse(id), onClear(id)` | empty / populated |
| `EntryStatus` | `src/tools/tarpack/EntryStatus.tsx` | `status` | Ready / Missing / Not assigned |

**Columns, in order:**

1. **Status**: an icon plus a word. Ready uses `--ok` with a check icon.
   Missing uses `--danger` with a warning icon. Not assigned uses
   `--text-muted` with an empty-circle icon. The word is always visible; the
   icon is `aria-hidden`.
2. **File**: the expected `source` name.
3. **Windows location**: `assigned` in `--font-mono`, middle-truncated (keep
   the drive and the file name visible), with the full path in `title` and in
   visually hidden text. Show "—" when unassigned. When the status is Missing,
   add the text "(not found)".
4. **Linux target**: `targetPath` in `--font-mono`.
5. **Mode**: `modeText` in `--font-mono`, followed by the octal value in
   `--text-muted`.
6. **Owner**: `owner`.
7. **Actions**: **Browse…** and **Clear**. Clear is disabled when unassigned.
   In this task the buttons call the `onBrowse` and `onClear` props, which U4
   wires to `lib`.

Above the table, a summary line reads "5 of 6 files ready", or "All 6 files
ready" when complete.

When the manifest has no `[[file]]` entries, show "This manifest lists no
files." instead of an empty table.

**Rules that bind this task.**

- Tokens only.
- Everything is keyboard reachable and labelled.
- Status is never color-only.
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
- Middle truncation keeps the file name visible for a 200-character path.
- The summary line is correct for mixed, all-ready, and empty cases.
- There is no horizontal page scroll at 800×560, and the layout holds at 200%
  text size.

## Tests proving completion

`npm run test`:

- `EntryTable.test.tsx`: a mixed-status fixture, the summary line, the
  empty-manifest message, the Clear-disabled state, and that each button calls
  its prop with the right id.
- `EntryStatus.test.tsx`: the label per status.
- `truncateMiddle.test.ts`: if you add a helper under `src/tools/tarpack/`.
- An axe check on a populated table.

## States covered

Empty manifest, partial (mixed statuses), and all ready.

## Out of scope

- The Browse dialog, Clear, and drop wiring (U4).
- The build bar (U5).
- Row keyboard shortcuts (U6).
