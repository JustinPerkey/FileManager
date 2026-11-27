# U10 — Tar Packager: the entry table

Status: approved (2026-11-21). U10 landed as ced769f and is approved on condition of item 1 of "Review follow-up"; the next run does only that section.
Project: gpui-migration   Depends on: U9 (landed), M7a (landed)

## Review follow-up (2026-11-26)

**The next run does only this section.** Item 1 is the condition of
approval.

1. **Column x positions: reserve today's scrollbar gutter.**
   - At 1366×800, the columns sit up to 15 px off the acceptance positions
     (216, 332, 534, 737, 939, 1071, 1151). Today's `.tarpack` has
     `scrollbar-gutter: stable` (`apps/desktop/src/styles/tarpack.css`,
     line 8), which reserves about 15 px on the right. GPUI's scrollbars
     overlay the content instead.
   - The look must match today, so reserve the same gutter on the right of
     the Tar Packager's tool view: a right padding of the scrollbar's width,
     from a `theme::tokens` constant (add `SCROLLBAR_GUTTER` in the theme
     task's style if none exists, valued at today's 15 px). No pixel literal
     in the view.
   - No other tool view had a gutter today (`tarpack.css` is the only
     stylesheet with one), so this applies to the Tar Packager only.
   - Test: `column_x_positions_match_today_with_sidebar`. The full shell at
     1366×800 with the sidebar, the `ready` fixture; each column's left edge
     (by debug selector or tag) is within 2 px of 216, 332, 534, 737, 939,
     1071 and 1151. `column_widths_follow_width` has no sidebar, so it
     cannot catch this.
2. **(P3) Tab skips the table while a build runs.** Add
   `tab_skips_the_table_while_building`. It drives Tab through the view with
   a build running (the `Slow`/building fixture), and asserts that focus
   never lands in the table. Reading the `tab_stop` flag alone is not
   enough.

**Recorded (accepted, as built):**

- A row's tooltip uses the **source** name; its accessible name uses the
  **target** path.
- Clear is **absent**, not disabled, on an unassigned row.
- Rows carry no `.tag`.
- U10 edited outside its file list, and both edits are accepted:
  - `FmButton::accessible_name` (`widgets/button.rs`), which is U5b's item
    (b);
  - `tab_stop(false)` support on the table widget.
- Moving the table copy into `text.rs` is a backend task:
  `M7c-tarpack-table-copy.md`.

## Goal

Render the manifest's entries as today's ruled table on gpui-component's
virtual `DataTable`: status, file, Windows location, Linux target, mode,
owner and row actions, with keyboard row focus, Browse and Clear, scaling to
10,000 rows.

## Context

FileManager is moving from Tauri + React to GPUI + gpui-component. The UI
looks like today: same layout and token colours (±1 per channel, geometry
within about 2 px of `docs/manual/images/tarpack-ready.png` at 1366×800);
text widths may differ. The human accepted two differences for this table:
**it never stacks rows into lines in narrow windows: it scrolls sideways**,
and **rows have one fixed height** (gpui-component's table is a uniform
list). Every interactive element is keyboard reachable and labelled. Views
call the model; they never touch the filesystem.

The table is the product's signature: "Show what will be written". Each
row says what goes in, where it lands on Linux, with which mode and owner,
and whether it is ready.

### Model and data

`TarpackModel` (`apps/filemanager/src/tools/tarpack/model.rs`, M7a):
`session() -> Option<&TarpackSession>` (`manifest.entries: Vec<SessionEntry>`;
each entry: `id`, `source` (file name), `assigned` display path or none,
`status` Ready / Missing / Unassigned, `target_path`, mode symbolic and
octal text, `owner`, line-ending conversion flag, a missing-file note),
`browse(entry_id, cx)`, `clear(entry_id, cx)`, `building()`
(the table is inert while building; M7b). Copy from `tools/tarpack/text.rs`:
column headings ("Status", "File", "Windows location", "Linux target",
"Mode", "Owner", "Actions"), status words, "Browse…", "Clear" and its
tooltip ("Forget this file (nothing is deleted). Shortcut: Delete"),
the `CRLF → LF` marker text, the summary line ("All 5 files ready · 1 file
converts line endings to LF" and its variants), the empty-entries text.
Read the exact field names in the session types
(`crates/fm-tarpack/src/session/`).

Proven in the spike (`git show origin/spike/gpui-migration:spike/gpui/src/widgets.rs`,
`Rows`): `TableState::new(delegate, window, cx)` with `.col_resizable(false)
.col_movable(false).sortable(false)`, a `TableDelegate` (`columns_count`,
`rows_count`, `column` → `Column::new(key, name).width(px)`, `render_td`),
`DataTable::new(&state)`, `selected_row()`, `scroll_to_row`; 60 Down presses
select row 59 and scroll it into view; 10,000 rows scroll in a median
17 ms per 100-row jump in an unoptimised headless build.

### Widgets and tokens

`status(StatusTone, IconName, word)`, `MiddlePath`, `text::{mono,
small_muted}`, `button` (quiet, `.shortcut`, `.tooltip`), `tabular`.
Tokens: `surface`, `surface_sunken`, `border`, `text`, `text_muted`, `ok`,
`danger`, `SPACE_1..3`, `RADIUS`.

### Look (today's `tarpack.css`)

- Above the table: the summary line (14 px) from `text.rs`, 8 px above the
  table. With no entries: the empty-entries text (max 546 px) instead of a
  table.
- **A ruled sheet**: `surface`; header row: 13 px, 600, `text_muted`,
  padding 8, 1 px `border` rules above and below; body rows: padding 8, a
  1 px rule under each; hover and focused row `surface_sunken`; no side
  borders, no radius, no wrapper. The header stays put while rows scroll.
- **Columns**: Status 116 px, Mode 132 px, Owner 80 px, Actions 176 px;
  File, Windows location and Linux target share the remaining width
  equally when the table is at least 1024 px wide; below that they keep
  their 1024 px share (≈ 173 px each) and **the table scrolls sideways**.
  Recompute widths when the view resizes (the delegate's `column()` reads
  them from the view; refresh the state).
- **Row height**: fixed, two text lines (2 × 14 px × 1.4) + 16 px padding
  = 56 px (+1 px rule). Set it through the table's row-size API in the
  pinned gpui-component; if only preset sizes exist, use the closest and
  say so in the PR.
- **Cells** (top-aligned):
  Status: `status(Ok, check-circle, "Ready")`, `(Danger, x-circle,
  "Missing")`, `(Muted, circle, "Unassigned")`, no wrapping.
  File: the name, then the `CRLF → LF` marker when the entry converts line
  endings (mono 14 px muted on `surface_sunken`, 1 px `border`, radius 6,
  padding 0/4).
  Windows location: `MiddlePath` (mono) keeping the drive and file name,
  full path as tooltip; "none" muted when unassigned; a missing note in
  `danger` on the second line when missing.
  Linux target: mono, wrapping onto at most two lines; longer → middle
  truncated with the full path as tooltip.
  Mode: symbolic on line 1, octal on line 2 in `text_muted`; mono;
  tabular.
  Owner: text, wrapping within two lines.
  Actions: quiet **Browse…** (tooltip "Shortcut: Enter") and quiet
  **Clear** (only when assigned; its tooltip above), 4 px apart; a row's
  buttons have accessible names that include the file ("Browse… for
  gateway.conf" as the tooltip's first line).

### Keyboard (today's roving focus)

The table is one Tab stop. Up/Down move the focused row (wrapping off;
scrolls into view); the focused row shows `surface_sunken` and the focus
ring inset. **Enter** = Browse… for the focused row; **Delete** = Clear when
it has a file. Tab from the table moves to the next control (the row's
buttons are reachable by mouse and by Enter/Delete; they are not separate
Tab stops, as today). These bindings live in the table's key context and
call `browse`/`clear`; they are inert while `building()`.

Debug selectors: `tarpack-table`, `tarpack-row-<n>` (0-based),
`tarpack-browse-<n>`, `tarpack-clear-<n>`.

**Ledger rows owned here (2026-11-23).** The `pathParts.test.ts` rows
"fileName is what follows the last separator" and "displayFolder shows a
lossy folder as is" are this task's: the entry table renders each row's file
name and folder. Port them as tests of the view's path helpers (or of
`widgets::middle_path` if you use it there), and settle the rows.

**Where the table scrolls (planner decision after the U9 review, 2026-11-23).**
The table **shrinks and scrolls inside itself**, as the approved layout
decision V6 says. Reasons: the table virtualises rows only inside a bounded
viewport, and the build bar must stay visible. U9 built the frame with the
page body scrolling and only the build bar outside it; this task changes
that frame:

- With a manifest loaded, the tool body column does **not** scroll.
  - Header, banners, errors region and drop result take their content
    height. The errors report already scrolls inside itself at 40% of the
    view height.
  - The table takes `flex_1` with `min_h_0` and scrolls its rows inside
    itself, with its header fixed.
  - The build result and the build bar sit below it.
- The table keeps a minimum height of three rows. Only if the window is
  shorter than everything above plus three rows plus the bar does the body
  column scroll, as a fallback; record that in a comment.
- Without a manifest (empty state, skeleton), the body scrolls as U9 built
  it.
- Tests: `table_scrolls_inside_itself_when_short` (a short window: the
  body's scroll offset stays 0, the table's moves on Down) and
  `build_bar_stays_visible_with_many_rows`.

**Hosting rules (after the U3 and U4 reviews, 2026-11-23).**

- **Sizing.** The chrome hosts the tool view in a `flex_1` / `min_h_0`
  column and cannot style it. So this task's root view is `size_full`, adds
  its own padding (16 px vertical, 24 px horizontal: `SPACE_4` / `SPACE_5`
  from `theme::tokens`), and handles its own scrolling (an
  `overflow_y_scroll` region inside the root, not the root's parent).
- **Confirmations.** If this view shows a confirmation, call `sync_confirm`
  only from a model observer (`cx.observe(&model, …)`), never inside
  `render`. Opening or closing a dialog during render re-enters the window.

**Scenario tags (planner rule, 2026-11-22, after M4).** The screenshot
harness resolves `Step::Click` and `Scenario::clip` only by `.tag("<name>")`
names (`apps/filemanager/src/app/probe.rs`); GPUI's debug selectors cannot be
read outside `VisualTestContext`, so they do not work in the harness. Every
element a scenario clicks or clips therefore carries `.tag("<name>")` as the
**outermost (last) call of its builder chain**, using the names this plan
lists (where this plan says "debug selector", read "tag"). If a
`VisualTestContext` view test also clicks the element, add
`.debug_selector(..)` with the same name as well.

## Files

Create: `apps/filemanager/src/tools/tarpack/view/{entry_table.rs,entry_cells.rs}`.
Edit: `apps/filemanager/src/tools/tarpack/view/{mod.rs,tests.rs}` (fill
`table_slot`), `docs/gpui-test-ledger.md`.
Do not touch: model, text, fixtures, other tools, `crates/`, `apps/desktop/`,
`docs/plans/`.

## Skill

None here; U12 critiques the screenshots.

## Acceptance criteria

- At 1366×800 with the `ready` fixture, the column x positions match
  `tarpack-ready.png` within 2 px (Status 216, File ≈ 332, Windows ≈ 534,
  Linux ≈ 737, Mode ≈ 939, Owner ≈ 1071, Actions ≈ 1151).
- At 1000×700 the table scrolls sideways and no row stacks.
- 10,000 rows render and Down moves through them without loading all rows.
- Keyboard: one Tab stop; Up/Down, Enter, Delete as specified.
- Labels/tooltips on every button. No colour literal outside `theme`.
- fmt; clippy pedantic `-D warnings`; `cargo test --workspace`.

## Tests proving completion

In `tools/tarpack/view/tests.rs` (`#[gpui::test]`, fixtures, `FakePlatform`):
`table_renders_status_cells`, `crlf_marker_shown_for_converting_entries`,
`down_moves_focus_and_scrolls` (a 10,000-entry fixture built in the test:
60 Downs select row 59 and it is visible), `enter_browses_focused_row`
(FakePlatform logs a pick), `delete_clears_only_assigned_row`,
`keys_inert_while_building`, `narrow_window_scrolls_sideways` (at 1000 px
the table's content width exceeds its bounds),
`column_widths_follow_width`.

Ledger: `EntryTable.test.tsx`, `EntryTable.keyboard.test.tsx`,
`EntryTable.scale.test.tsx`, `EntryStatus.test.tsx`, and the keyboard and
label rows of `TarpackView.a11y.test.tsx` → ported; reflow, sticky-CSS and
ARIA rows → dropped with the reason (D13 / DOM).

## States covered

No entries, unassigned, ready, missing (with note), converting line
endings, focused row, hover, building (inert), 10,000 rows, narrow.

## Out of scope

Drop routing (Shell/model); build bar and results (U11).
