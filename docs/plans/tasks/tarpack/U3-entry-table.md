# U3 — Entry table with per-row status

Status: done (landed 2026-09-30, reviewer-approved; amended 2026-09-30 after the review of the first
U3 implementation: path reveal on focus, row-action names, scroll model,
column widths and reflow, no container; amended again 2026-09-30 after the
second review's rendered evidence: the default window now uses the stacked
layout, the table layout starts at 64rem, Actions is 11rem, and the sticky
offsets come from a view padding variable; third amendment: `MiddlePath`'s
shipped tail and whole-character line-width rules)
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
  reserves a slot for the table. Its root element is
  `<section className="tool-view tarpack">`: the `.tarpack` element **is** the
  view's scroll container (`.tool-view` in `src/styles/app.css` sets
  `overflow: auto`, padding `--space-4 --space-5`). Chromium sticks
  elements to the scroll container's **content box**, so that 16 px top
  padding pushes a `top: 0` sticky element 16 px down (see "Scroll model").
- `session.manifest.entries` holds the **passed** entries only, in manifest
  order. It is a list of
  `{ id, source, targetPath, mode, modeText, owner, normalizeEol, assigned, status }`.
  Check `src/lib/generated/` for the exact types:
  - `status` is `"ready"`, `"missing"`, or `"unassigned"`;
  - `targetPath` is the absolute name stored in the archive, always starting
    with `/`, for example `/opt/gateway/bin/gateway`. It is **unique among
    listed entries**: two entries with the same target both fail, so neither
    is listed;
  - `source` is the expected file name. It is **not** unique: two entries may
    expect the same name for different targets;
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
  `--radius`, `--font-size-sm` and `--font-size-md`, and the `--space-*`
  scale (4, 8, 12, 16, 24, 32 px).
- U2 built the shared vocabulary; use it and add no parallel version:
  - `src/app/Button.tsx`: `variant: "primary" | "secondary" | "quiet"`,
    `icon?`, plus native `<button>` props (`title`, `disabled`, …);
  - `src/app/icons.tsx`: `Icon` with `name: IconName`, including
    `check-circle`, `alert-triangle`, and `circle`;
  - the `.num` (tabular numerals), `.mono` (`--font-mono`), and
    `.visually-hidden` utility classes in `src/styles/base.css`.
- **Stylesheets.** Tool styles go in `src/styles/tarpack.css`, and every
  selector there is scoped under the view root class `.tarpack` (for example
  `.tarpack .entry-table`). Shared component styles go in
  `src/styles/controls.css`. Nothing in this task has a shadow.
- **Design context.** The root `DESIGN.md` records the visual system ("The
  Packing List"). This table is its signature component: the list of what
  goes in the crate. It is an Operate surface, dense and scannable, and never
  decorated. It uses no cards, no zebra striping in status colors, and no
  metric tiles.
- **Window sizes.** `tauri.conf.json` opens the window at 1000×700, with a
  minimum of 800×560. The tool nav is `12rem` wide (192 px, capped at 40% of
  the window) plus a 1 px rule, and the view is padded `--space-5` (24 px) on
  each side. So the table's width at 100% text is about **742 px** at the
  default window (759 px less the 17 px scrollbar gutter, see "Scroll
  model"), which is about **46.4rem**, and about **542 px** (33.9rem) at the
  minimum. At 200% text (root font size 32 px) the minimum window leaves
  about 431 px, which is only 13.5rem.
- **Rendered evidence (second review).** Seven columns at the default window
  left each flexible column about 94 px: the Windows location showed only
  `C:…`, file names broke mid-word, "(not found)" wrapped, and a long target
  ran to 10 lines. The stacked layout at 800 px read far better. So the
  default window uses the stacked layout, and the seven-column table is for
  wide windows only.

### Components

| Component | Path | Props | States |
| --- | --- | --- | --- |
| `EntryTable` | `src/tools/tarpack/EntryTable.tsx` | `entries, failedCount, entriesWithheld, onBrowse(id), onClear(id)` | populated / populated with files left out / no files / every file failed / withheld; stacked layout (default) / table layout (≥ 64rem) |
| `EntryStatus` | `src/tools/tarpack/EntryStatus.tsx` | `status` | Ready / Missing / Not assigned |
| `EolMarker` | `src/tools/tarpack/EolMarker.tsx` | — | rendered only for entries with `normalizeEol` |
| `MiddlePath` | `src/tools/tarpack/MiddlePath.tsx` | `path: string` | fits / truncated (by CSS) / revealed (in a keyboard-focused row) |
| `pathParts` | `src/tools/tarpack/pathParts.ts` (module) | `pathParts(path: string): { head: string; tail: string }` | — |

`pathParts.ts` and `MiddlePath.tsx` replace the first implementation's
`truncateMiddle.ts` (a fixed 48-code-point budget), which is deleted with its
test. U5 reuses `MiddlePath` for the build bar's output path.

**Columns, in order:**

1. **Status**: an `Icon` plus a word. Ready uses `--ok` with `check-circle`.
   Missing uses `--danger` with `alert-triangle`. Not assigned uses
   `--text-muted` with `circle`. The word is always visible, in the same
   color as its icon (each pair passes AA on `--surface`); the icon is
   `aria-hidden`. In the table layout the column's fixed width (below) holds
   the longest word
   ("Not assigned") on one line (`white-space: nowrap`), so status changes
   cause no layout shift.
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
3. **Windows location**: `assigned`, rendered by `MiddlePath` (below):
   `--font-mono`, truncated in the middle by CSS so the drive and the file
   name stay visible. The folder part truncates first; the file name (up to
   its 32-code-point cap) is never truncated, and wraps only when the name
   alone is wider than the cell. It is one line whenever the file name fits:
   always in the stacked layout at 100% text, and in the table layout for
   names up to about 19 characters. The full path is available three ways:
   - **hover**: `title` on the `MiddlePath` root;
   - **keyboard focus**: while the row holds keyboard focus (a focused Browse
     or Clear now; the focused row itself once U6 adds roving focus), the
     cell shows the full path, wrapped, in place of the truncated line (see
     "Path reveal on focus");
   - **screen readers**: the full path is always the cell's accessible text.

   Show "—" when unassigned (`--text-muted`). When the status is Missing, the
   text "(not found)" follows on its own line below the path, in `--danger`,
   `white-space: nowrap`.
4. **Linux target**: `targetPath`, rendered verbatim including its leading
   `/`, in an inner `<span className="mono entry-table__target-path">`. Do
   not strip, join, or rebuild it. Insert a `<wbr>` after each `/` inside
   that span so wraps fall at directory boundaries; the **span's**
   `textContent` must equal `targetPath` exactly. `overflow-wrap: anywhere`
   remains the fallback for a segment longer than the column.
5. **Mode**: `modeText` in an inner `.mono` span, then the octal value in
   `--text-muted` with `.num`. They are two `white-space: nowrap` spans
   separated by an ordinary space; both fit on one line in either layout.

**Mono goes on inner spans, never on cells.** In columns 3–5 the data sits in
an inner span that carries `mono` (`MiddlePath`'s root, the target span, the
`modeText` span). The `td` stays in the UI font, so the stacked layout's cell
label ("Windows location", "Linux target", "Mode", "Owner") is never
monospace: mono is for data only.
6. **Owner**: `owner`, with `overflow-wrap: anywhere`.
7. **Actions**: **Browse…** and **Clear**, as quiet `Button`s on one line
   (`white-space: nowrap`, `--space-1` gap). Measured: Browse… is 91 px and
   Clear is 63 px wide at 100% text. Clear is disabled when
   unassigned (still rendered, so nothing moves). In this task the buttons
   call the `onBrowse` and `onClear` props, which U4 wires to `lib`. Their
   accessible names and tooltip are fixed here, and U4 does not change them:

   | Button | Visible label | Accessible name | `title` |
   | --- | --- | --- | --- |
   | Browse | `Browse…` | "Browse… for {targetPath}" | none |
   | Clear | `Clear` | "Clear assigned file for {targetPath}" | "Forget this file (nothing is deleted)" |

   Build each name from the visible label plus a `.visually-hidden` suffix
   (" for {targetPath}", " assigned file for {targetPath}"), not with
   `aria-label`, so the name always starts with the visible words (WCAG
   2.5.3). Use `targetPath`, not `source`: `source` may repeat across rows,
   and two buttons must never share a name. The Clear `title` becomes its
   accessible description; it applies whether or not Clear is disabled.

**Typography and rhythm** (`/impeccable typeset`):

- Cells are `--font-size-md`, and column headers are `--font-size-sm` at
  weight 600 in `--text-muted`, in sentence case (not uppercase or tracked).
  Header labels may wrap onto two lines in narrow columns ("Windows
  location"); do not set `nowrap` on headers.
- Mono is only for the data in columns 3–5 (inner spans, above) and the
  marker: data a user might paste. Cell labels and headers are never mono.
- The octal mode and the summary counts use `.num`.
- Cells are padded `--space-2` on every side (8 px; a dense Operate table),
  with a 1 px `--border` rule between rows and no vertical rules. Cells align
  to the top.
- Row hover is `--surface-sunken`. There is no hover on the header row.

### Surface: a ruled sheet, not a card

`DESIGN.md` forbids cards around regions. The table gets **no container
treatment**: no wrapper border, no radius, no shadow, and no wrapper element
with its own background or scrolling. The table itself (`width: 100%`) is
set on `--surface`, spanning the content column, with:

- a 1 px `--border` rule above the header row (the top of the sheet) and one
  below it;
- a 1 px `--border` rule under every row, including the last (it closes the
  list);
- no side borders and no rounded corners.

Use `border-collapse: separate; border-spacing: 0` so sticky header cells
keep their rules.

### Column widths (table layout, 64rem and wider)

The table layout uses `table-layout: fixed` with a `<colgroup>` of seven
`<col>`s, each with a class (`entry-table__col--status`, `--file`,
`--windows`, `--target`, `--mode`, `--owner`, `--actions`). Widths never
depend on cell content, so assigning a file, clearing it, or a status change
never moves a column. All widths are in `rem`, so they scale with the text
size.

| Column | Width | Why |
| --- | --- | --- |
| Status | `7.25rem` | 16 px icon + gap + "Not assigned" + padding |
| File | auto | an equal share of the rest |
| Windows location | auto | an equal share; truncates by CSS |
| Linux target | auto | an equal share; wraps at `/` |
| Mode | `8.25rem` | `rwxr-xr-x 0755` is 14 mono characters, one line |
| Owner | `5rem` | `root:root`; longer owners wrap |
| Actions | `11rem` | 91 px + 4 px gap + 63 px + 16 px padding = 174 px = 10.875rem |

The fixed columns take **31.5rem**. The table layout starts at a table width
of **64rem** (see "Stacked layout"), so each auto column gets at least
(64 − 31.5) / 3 = **10.83rem** (173 px, about 19 mono characters of content):
a typical target such as `/opt/gateway/bin/gateway` wraps once at a `/`,
and a Windows file name up to about 19 characters stays on one line.

Chosen over the reviewer's alternatives for keeping seven columns at the
default window: widening the Windows location at the expense of File and
Target only moves the damage within 742 px (the target still runs to many
lines), and a narrower Actions column would mean icon-only buttons, which the
house rules forbid.

If the rendered check shows "Not assigned", `rwxr-xr-x 0755`, or the two
buttons wrapping, widen that column by the least amount that fits, raise the
64rem threshold by the same amount (so each auto column keeps at least
10.83rem), and report the final values.

### Stacked layout (narrow widths and large text)

Seven readable columns need about 64rem, and the table has about 46.4rem at
the default window, 33.9rem at 800 px, and 13.5rem at 200% text. So the
table **reflows** below 64rem instead of scrolling sideways, and the stacked
layout is the **default**: every window up to about 1280 px at 100% text,
and every usual window at 200% text. Use a container query, so the switch
follows the table's own width and the text size together:

- `.tarpack .entry-list` (the element wrapping the summary and the table)
  gets `container-type: inline-size` and `container-name: entries`. It has
  no `overflow` and no height limit.
- **Table layout** at `@container entries (width >= 64rem)`: the columns
  above. At 100% text this is a window of about 1282 px or wider (1024 px of
  table + 17 px gutter + 48 px padding + 193 px nav). The default window
  (about 46.4rem) is 17.6rem below the switch, so a few pixels of nav,
  padding, or scrollbar width can never flip the default layout.
- **Stacked layout** below 64rem: the default 1000 px window, the 800 px
  minimum, and 200% text. The element stays a `<table>` with the same cells
  and headers; only the CSS changes:
  - `table` and `tbody` are `display: block`; the `<colgroup>` is ignored;
  - the `thead` is visually hidden with the clip pattern (not
    `display: none`), so `th scope="col"` still names every cell for screen
    readers;
  - each `tr` is `display: flex; flex-wrap: wrap; align-items: baseline;
    gap: var(--space-1) var(--space-3); padding: var(--space-2)` on
    `--surface`, with the 1 px `--border` rule below it and the sheet's top
    rule above the first row; each `td` is `display: block` with no padding
    or border of its own;
  - order and flow: Status (`flex: 0 0 auto`) and File (`flex: 1 1 10rem;
    min-width: 0`) share the first line; Windows location and Linux target
    each take a full line (`flex: 1 0 100%`); Mode and Owner
    (`flex: 0 1 auto`) share the next line, and Actions (`flex: 1 0 auto`,
    buttons pushed to the end) joins that line when it fits and wraps to its
    own line when it does not;
  - the Windows location, Linux target, Mode, and Owner cells each show a
    **cell label**: a `<span className="entry-table__cell-label"
    aria-hidden="true">` rendered in every row with the column's header text,
    `--font-size-sm`, `--text-muted`, `margin-right: var(--space-1)`, in the
    UI font (the cell is not mono; the data span after it is). It is
    `display: none` in the table layout and inline in the stacked layout. It
    is `aria-hidden` because the header association already names the cell.
  - Row hover, the status colors, `MiddlePath`, and the path reveal on focus
    work the same way.

WebView2 is Chromium, which keeps table semantics when table parts are given
another `display`. Do not add `role="table"`/`row`/`cell` (the jsx-a11y lint
rejects redundant roles). If the rendered check shows the table role or the
header names lost in the stacked layout, stop and report it to the
ui-designer rather than working around it.

### Scroll model (decided)

**The view scrolls; the table does not.** `.tarpack` (the `.tool-view`
section) is the one vertical scroll container for the table, the drop
result, the build result, and the build bar. There is no internal scroll box
around the table and no `max-height` on it, at any size.

- **The view's block padding is a variable.** Chromium sticks an element to
  the scroll container's content box, so with `.tool-view`'s 16 px top
  padding a `top: 0` header stops 16 px below the top edge and rows show
  through the gap. `tarpack.css` therefore owns this view's block padding:
  `.tarpack { --view-pad-block: var(--space-4); padding-block:
  var(--view-pad-block); }` (the same value as `app.css`; `tarpack.css` loads
  later, so it wins). Every sticky offset in the view is derived from it:
  `top: calc(var(--view-pad-block) * -1)` for the header, and
  `bottom: calc(var(--view-pad-block) * -1)` for U5's bar. Change the padding
  only through the variable.
- **Sticky header.** In the table layout, the `th` cells are
  `position: sticky; top: calc(var(--view-pad-block) * -1)` on `--surface`,
  so they stick flush to the top edge while the page scrolls through 2,000
  rows. No ancestor of the table between it and `.tarpack` may set
  `overflow` (anything but `visible`), or the header would stick to that
  ancestor instead. In the stacked layout (the default window) the header is
  visually hidden, so nothing is sticky and every cell carries its own
  label.
- **Build bar (U5).** The bar is sticky at
  `bottom: calc(var(--view-pad-block) * -1)` in the same scroll container,
  and sets `scroll-padding-bottom` for itself. Nothing in this task reserves
  space for it.
- **Keep focus visible.** In `tarpack.css`, set
  `.tarpack { scrollbar-gutter: stable; scroll-padding-top: 3.5rem; }`.
  The gutter keeps the table width constant when the page starts to scroll
  (no column jump when a drop or reload adds rows). The padding keeps a
  focused control or row from scrolling in under the sticky header (3.5rem
  covers a two-line header at any text size, since it is in rem).
- **Bounded exceptions.** The error report (U2) and the build report (U5)
  stay capped at `40vh` and scroll inside themselves: they are secondary,
  keyboard-scrollable regions (`tabIndex={0}`), and capping them keeps the
  table reachable. The table is the one region that grows.
- **Why not an internal box.** A second scroll box for the table (for example
  `max-height: 70vh`) nests inside the page scroll next to the 40vh error
  report and the sticky bar. At 800×560 with 200% text that leaves three
  scroll areas and a bar in about 560 px, traps the mouse wheel in the table,
  and hides the table's own scrollbar behind the page's. Page scroll gives
  one scrollbar and one keyboard model (Page Up/Down, Home/End on the view).
- **Check at 800×560, 200% text.** Stacked layout; no sticky header; the bar
  (U5) takes two lines, about 130 px; the rows scroll in the remaining space
  with every control reachable by Tab and every row reachable by scrolling.
  With the error report expanded it takes at most 224 px (40vh) and the page
  scrolls past it to the table.

### Path reveal on focus and `MiddlePath`

`pathParts(path)` splits a display path, working on code points
(`Array.from`), never UTF-16 units:

- It finds the drive or UNC prefix (`C:\`, `C:/`, `\\server\share\`) and the
  last `\` or `/` after it.
- `head` is everything before that last separator (for example
  `C:\Users\me\build\out`); `tail` is the separator and the file name (for
  example `\gateway.exe`). With no separator, `head` is `""` and `tail` is
  the whole path.
- When the file name is longer than **32 code points**, `tail` is the
  separator, then "…", then the name's last 32 code points. That is the only
  code-point cap; everything else is truncated by width, in CSS.
- It never splits a surrogate pair, and it keeps U+FFFD as given.

`MiddlePath` renders, for a non-null path:

```html
<span class="middle-path mono" title="{path}">
  <span class="middle-path__line" aria-hidden="true">
    <span class="middle-path__head">{head}</span>
    <span class="middle-path__tail">{tail}</span>
  </span>
  <span class="middle-path__full">{path}</span>
</span>
```

- `.middle-path__line` is `display: flex; min-width: 0; width:
  calc(round(down, 100% - 1px, 1ch) + 0.5px)`. The width rule snaps the
  line to a whole number of mono characters, so the head's "…" sits flush
  against the tail. A plain `round(down, 100%, 1ch)` is not enough:
  Chromium's 1/64 px layout rounding lands it a hair under a whole number of
  characters, and `text-overflow` then drops one more glyph, leaving a
  one-character gap after "…". Subtracting 1 px before rounding and adding
  0.5 px after keeps the result just above the whole-character width
  (verified in Chromium at 36 widths).
- `.middle-path__head` is `flex: 0 1000 auto; min-width: 4ch; overflow:
  hidden; white-space: nowrap; text-overflow: ellipsis`: it shrinks first,
  down to about `C:\…`, so the drive stays visible.
- `.middle-path__tail` is `flex: 0 0 auto; max-width: calc(100% - 4ch);
  overflow-wrap: anywhere`: it never shrinks, so it keeps the whole file name
  on one line whenever the name fits beside the head's `4ch` minimum, and
  wraps only when the name alone is wider than that. Do not use
  `flex: 0 1 auto`: the tail then shrinks by a sub-pixel and the file name
  breaks mid-word.
- Keep these three rules together and change them only together; U5's output
  path uses the same component and styles.
- `.middle-path__full` is visually hidden by default (the same clip pattern
  as `.visually-hidden`, written on this class so it can be undone) and is
  the accessible text of the cell.
- **Reveal.** In the table, a row that holds keyboard focus shows the full
  path instead of the line:
  `.tarpack .entry-table tr:is(:focus-visible, :has(:focus-visible))`
  hides `.middle-path__line` and shows `.middle-path__full` in flow
  (`position: static`, no clip, `white-space: normal`, `overflow-wrap:
  anywhere`). `:focus-visible` keeps mouse clicks on Browse… from expanding
  the row. The change is instant (no transition). Only the focused row grows;
  no column moves. In the stacked layout the full path gets a whole line, so
  a typical path reveals in one or two lines; in the table layout it wraps
  within its 10.83rem-or-wider column.

The truncation needs no measurement in render and no resize listener.

**Names that are not valid UTF-8 or are very long** (`/impeccable harden`):

- Every name you receive is a display string. It may contain U+FFFD (�)
  where the Windows or manifest name was not valid UTF-8. Render it as
  given; never try to repair it.
- The Windows location keeps the drive and the file name visible at any
  width (above), with the 32-code-point cap on the name; the name wraps
  rather than truncates when it alone is wider than the cell.
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
- Everything is keyboard reachable and labelled; no two controls share an
  accessible name.
- Status is never color-only, and neither is the line-ending marker.
- No size or modified-time columns (decided: keep the table narrow).
- No container treatment and no internal scroll box (above).
- The table stays usable with 200 entries, and scrolls without jank at 2,000:
  sticky column headers in the table layout, no layout shift when statuses
  change or files are assigned, and **no horizontal scroll at any size**,
  neither the page nor the table: the table layout (64rem and wider) fits by
  fixed widths and truncation, and narrower widths, including the default
  window, use the stacked layout.

## Files

- `src/tools/tarpack/EntryTable.tsx`, `EntryStatus.tsx`, `EolMarker.tsx`,
  `MiddlePath.tsx`, `pathParts.ts`
- Delete `src/tools/tarpack/truncateMiddle.ts` and `truncateMiddle.test.ts`
  (replaced by `pathParts.ts`)
- Wire the table into `TarpackView.tsx`
- Table and `MiddlePath` styles in `src/styles/tarpack.css`, scoped under
  `.tarpack` (including `.tarpack { --view-pad-block; padding-block;
  scrollbar-gutter; scroll-padding-top }`)
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
- **Rendered check.** Run the frontend (`npm run dev` in `apps/desktop` with
  `lib/` mocked, or `npm run tauri:dev`) with a fixture of about 30 entries
  that includes long Windows paths, a long target, a `normalizeEol` row, a
  Missing row, and U+FFFD names. Check at 1400×800, 1000×700, and 800×560, in
  light and dark, and at 200% text (set the root font size to 32 px in
  DevTools). Confirm: the table layout at 1400 px, and the stacked layout at
  1000 px, at 800 px, and at 200% text; no horizontal scrollbar anywhere; in
  the table layout the header sticks flush to the top edge with no gap while
  scrolling; assigning a file moves no column; the stacked cell labels are in
  the UI font, not mono; tabbing to Browse… reveals the full Windows path;
  and, in the accessibility tree, the table role and header names survive the
  stacked layout. If no browser is available, say so; U6's audit repeats
  these checks.
- If the skill is not installed, install it with `npx impeccable install`, or
  follow the named commands' reference docs from
  `github.com/pbakaus/impeccable` by hand. Say which in your report.

## Acceptance criteria

- It is a semantic `<table>` with a `<caption>` (which may be visually hidden)
  and `<th scope="col">` headers, and a `<colgroup>` of seven classed `<col>`
  elements. The CSS sets `table-layout: fixed` and the widths above.
- Each status's text label is present in the DOM.
- A row with `normalizeEol: true` shows the visible **CRLF → LF** marker and
  exposes "line endings converted to LF" in its accessible name; a row with
  `normalizeEol: false` shows no marker. The marker is not focusable.
- `targetPath` renders exactly as given, leading `/` included: the inner
  `.entry-table__target-path` span carries `mono`, its `textContent` equals
  `targetPath`, and it has a `<wbr>` after each `/`. The cell label beside it
  (stacked layout) is outside that span and not mono.
- `pathParts` keeps the drive and the file name for a 200-character path;
  caps a file name over 32 code points to "…" plus its last 32; never splits
  a surrogate pair; and keeps U+FFFD intact.
- `MiddlePath` renders the head and tail inside an `aria-hidden` line, and
  the full path once as accessible text and in `title`. The Windows location
  cell's accessible text is the full path (plus "(not found)" when Missing).
- The row-reveal rule
  `.tarpack .entry-table tr:is(:focus-visible, :has(:focus-visible))` shows
  `.middle-path__full` and hides `.middle-path__line`.
- Browse and Clear have the accessible names "Browse… for {targetPath}" and
  "Clear assigned file for {targetPath}", and Clear has the `title` "Forget
  this file (nothing is deleted)". Two entries with the same `source` get
  distinct names.
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
- The table has no wrapper with a border, radius, background, `overflow`, or
  `max-height`; `.tarpack .entry-list` has `container-type: inline-size`;
  the table layout's container query is `width >= 64rem`.
- The sticky header's `top` is `calc(var(--view-pad-block) * -1)`, and
  `.tarpack` defines `--view-pad-block` and uses it for `padding-block`.
- No `td` carries `mono`; the data spans in columns 3–5 do.
- There is no horizontal scroll at 1400×800, 1000×700, or 800×560; the
  default window shows the stacked layout; and the layout holds at 200% text
  size (rendered check, or stated as not run).

## Tests proving completion

`npm run test`:

- `EntryTable.test.tsx`: a mixed-status fixture, the summary line (mixed,
  all ready with and without `failedCount`, and the left-out clause singular
  and plural), the three empty messages (withheld, every file failed, no
  files), the Clear-disabled state, that each button calls its prop with the
  right id, the Browse and Clear accessible names (including two entries
  with the same `source` and different targets, found by name with
  `getByRole("button", { name })`), Clear's `title`, the seven `<col>`s, the
  `.entry-table__target-path` span's exact `textContent` and `mono` class,
  that no `td` has the `mono` class, and that each cell label is
  `aria-hidden` and sits outside the data span.
- `EntryStatus.test.tsx`: the label per status.
- `EolMarker.test.tsx` (or cases in `EntryTable.test.tsx`): marker present
  with its accessible text for a `normalizeEol` row, absent otherwise, and the
  summary-line clause.
- `pathParts.test.ts`: drive and UNC prefixes; the head and tail split; no
  separator; a file name of exactly 32 and of 33 code points; an emoji or
  astral character at the cap is not split; U+FFFD is preserved.
- `MiddlePath.test.tsx`: the line is `aria-hidden`, the full path is the
  accessible text and the `title`, and the rendered head plus tail equal the
  path when the name is under the cap.
- `EntryTable.scale.test.tsx`: a 2,000-row fixture renders, and one status
  change re-renders one row.
- An axe check on a populated table that includes a `normalizeEol` row and a
  Missing row.

jsdom does not evaluate container queries, `:has()`, or sticky positioning;
the rendered check covers those.

## States covered

Empty manifest (with its next-step line), entries withheld, every entry
failed, partial (mixed statuses), all ready, all listed files ready with files
left out, rows with and without line-ending conversion, long and non-UTF-8
names, a keyboard-focused row with a long path, 2,000 rows, and the stacked
(default) and table (64rem and wider) layouts.

## Out of scope

- The error report and error notice (U2).
- The Browse dialog, Clear, and drop wiring (U4).
- The build bar and its sticky position and scroll padding (U5).
- Row keyboard shortcuts and roving focus (U6).
