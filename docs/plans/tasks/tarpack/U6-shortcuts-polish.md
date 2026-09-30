# U6 — Keyboard shortcuts, final polish, and audit

Status: done (landed 2026-09-30 at ee3da63, with the review fixes at
01d6a0c and the audit fixes at 0fe3fe1; reviewer-approved after re-review;
not yet run in real Tauri or on Windows, which moves to M7's Windows check,
including the WebView2 middle-truncation check, step 10 of its checklist;
amended 2026-09-30 at close-out: the detector command runs from
`apps/desktop`, the tall-row scroll rule as landed, and the `stopId` rule
reworded for pointer focus during a build)
(amended 2026-09-30: path reveal on the focused row,
row-action names and hints, layout and scroll model from U3; stacked layout
at the default window, 64rem switch, sticky offsets; WebView2 check of
middle truncation; focus after dismissing the drop result, from U4;
amended 2026-09-30: U5's waived audit and the short-window bar fallback;
amended 2026-09-30 after review of ee3da63: fixes R1–R8, the implementer's
gap decisions confirmed, the audit deliverables, the `--scrim` token, and
the WebView2 truncation check moved to M7's Windows check, run by the user)
Project: tarpack   Depends on: U2, U3, U4, U5 (all landed)

## Goal

Make the whole Tar Packager fast to drive from the keyboard, then polish and
audit it until it has no P0 or P1 findings.

## Context

**The user.** A developer who rebuilds the same package many times a day. Their
common loop is: reload the edited manifest, check the table, create the
archive, copy the extraction command.
That loop must take a few keystrokes, not a mouse trip.

**What exists.** Under `src/tools/tarpack/`:

- `TarpackView.tsx`, `ManifestHeader.tsx` (Open…, Recent, Reload, Edit in
  editor), and `ManifestErrors.tsx`. A manifest with errors still opens:
  entries that passed are in the table, and `ManifestErrors` is the **error
  region** for the rest. It has a notice that is always visible while
  `session.manifest.errorCount > 0` ("3 errors in this manifest", a
  consequence line, a **Show errors** / **Hide errors** toggle, and Edit in
  editor), and a collapsible, focusable report body
  (`id="manifest-error-report"`) listing the whole-manifest errors and each
  failed entry with all of its errors, through `FailureList.tsx` and
  `DiagnosticList.tsx`. A polite live-region announcer in `TarpackView`
  announces the error count after each open, reload, and session restore.
  `TarpackView` has a `showErrors()` handler that expands the report and
  moves focus to it (it does nothing when `errorCount` is 0). Call it from
  inside the view (the shortcut hook is owned by `TarpackView`); the optional
  `actionsRef` prop is a test-only seam, not for product code. Info and warn
  banners are not live regions; the view announces them through the
  announcer;
- `EntryTable.tsx`, with per-row Browse… and Clear, `EntryStatus.tsx`,
  `EolMarker.tsx` (the non-interactive "CRLF → LF" marker on rows whose line
  endings are converted), and `MiddlePath.tsx` (the Windows location: one
  mono line truncated in the middle by CSS, the full path in `title` and as
  accessible text). Details you build on:
  - Row actions have the accessible names "Browse… for {targetPath}" and
    "Clear assigned file for {targetPath}" (visible label plus a visually
    hidden suffix), and Clear has the `title` "Forget this file (nothing is
    deleted)". Keep the names.
  - **Path reveal on focus.** `tarpack.css` has a rule
    `.tarpack .entry-table tr:is(:focus-visible, :has(:focus-visible))` that
    swaps the truncated Windows location for the full path, wrapped. It
    already covers a focused row, so your roving focus gets the reveal with
    no new code: the focused row is how a sighted keyboard user reads a
    truncated path. Keep the rule, and do not animate it.
  - **Layout.** Below a table width of 64rem (the default 1000 px window,
    the 800 px minimum, and 200% text) a container query stacks each row
    into lines with visible, `aria-hidden` cell labels in the UI font and a
    visually hidden header row; this is the layout most users see. At 64rem
    or more (windows about 1282 px and wider at 100% text) it is a
    seven-column `table-layout: fixed` table with `<colgroup>` widths and a
    sticky header. Roving focus must work in both layouts: rows are `tr`
    elements in both.
  - **Scroll model.** `.tarpack` (the view root) is the one scroll container:
    no scroll box around the table; the sticky bar at the bottom; only the
    error report and the build report scroll inside themselves (`40vh`).
    `.tarpack` sets `scrollbar-gutter: stable`, `scroll-padding-top: 3.5rem`
    (the sticky header), and a `scroll-padding-bottom` for the bar (U5,
    14.5rem: the bar is 14 rem tall at 800×560 with 200% text). At
    `@media (max-height: 30rem)` the bar is `position: static` and
    `scroll-padding-bottom` is 0 (the human's decision, 2026-09-30; keep
    it, height-only threshold accepted).
    Its block padding is `--view-pad-block`, and the sticky header and bar
    use `top`/`bottom: calc(var(--view-pad-block) * -1)` because Chromium
    sticks to the content box; if you change the view's padding, change the
    variable, never a sticky offset.
- `DropResult.tsx` (U4): dismissing it moves focus only when focus was
  inside the result, to the first tabbable element after it (else the last
  one before it); the view root is never focusable. Escape dismisses
  through the same `onDismiss`, so keep that rule. With roving focus, the
  table's tab-stop row (`tabindex="0"`) is what "first tabbable after the
  result" finds; check that in `EntryTable.keyboard.test.tsx`. The result
  also has a native "Full paths" `<details>`; its `summary` is in the tab
  order;
- `BuildBar.tsx` (output path with Choose…, the **Format** picker — a labelled
  native `<select>` over the four formats tar, gzip, zstd, xz — the status
  text, **Show errors** when the manifest has errors, and the **Create
  archive** button), `FormatPicker.tsx`, and `BuildProgress.tsx` (two
  labelled steps, "Writing" then "Verifying", each 0 → 100%). **Errors do
  not block the build** (decided by the human): Create archive follows
  `session.canBuild`, which is true with errors as long as at least one
  entry passed, every passed entry is ready, and an output is chosen. The bar
  then shows a note such as "2 files will be left out; errors will be listed
  after the build";
- `BuildResult.tsx`: on success, the SHA-256 and the target extraction command
  (for example `tar --zstd --no-overwrite-dir -xpPf gateway.tar.zst`), each
  with a **Copy** button, the list of entries whose line endings were
  converted, and Show in folder; when the build left files out, a heading
  such as "Created gateway.tar.zst with 2 files left out" and the final
  report (`BuildReport.tsx`: Left out of the archive, Manifest errors,
  Warnings) with **Copy report**; on error, a message and a Details
  disclosure.
- `errorMessages.ts`: the single, exhaustive
  `Record<TarpackErrorKind, …>` of user-facing error copy (17 kinds). When
  re-reading copy, edit it there; no component holds its own error strings.

Under `src/app/`:

- `AppShell.tsx`, `ToolNav.tsx`, `Banner.tsx`, `DropZone.tsx`, and
  `ConfirmDialog.tsx`;
- the shared vocabulary from U2: `Button.tsx` (`primary`, `secondary`,
  `quiet`), `icons.tsx` (`Icon`, `IconName`, including `keyboard`), and
  `src/styles/controls.css`. The type-size tokens `--font-size-sm/md/lg/xl`,
  `--shadow-overlay`, and the `.num` and `.mono` utilities are in
  `src/styles/`. Tool styles are in `src/styles/tarpack.css`, every selector
  scoped under the view root class `.tarpack`; shared component styles are
  in `controls.css`. `--shadow-overlay` is used by the Recent menu (U2), the
  confirmation dialog (U5), and your shortcuts popover, and nothing else.

**Design context.** The root `PRODUCT.md` and `DESIGN.md` record the product
and its visual system ("The Packing List"). `DESIGN.md` lists one known drift
that this task fixes: the current tool-nav item's 3 px `--accent` left border.

All actions already exist as handlers in `TarpackView`. This task adds keyboard
access to them; it adds no new behaviour.

### Shortcuts

| Keys | Action | Active when |
| --- | --- | --- |
| Ctrl+O | Open manifest… | always |
| F5 or Ctrl+R | Reload manifest | a manifest is loaded |
| Ctrl+E | Edit in editor | a manifest is loaded |
| Ctrl+Enter | Create archive | `session.canBuild` (true even when the manifest has errors) |
| F8 | Show errors: expand the error report and move focus to it (`showErrors()`) | a manifest is loaded and `errorCount > 0` |
| Up / Down | Move between table rows | focus is in the table |
| Enter | Browse… for the focused row | a row is focused |
| Delete | Clear the focused row | the row is assigned |
| Escape | Dismiss the drop result or build result | one is visible (Escape never collapses the error report) |

- Shortcuts are ignored while `ConfirmDialog` is open or a build is running.
  That includes the row keys (Up, Down, Enter, Delete): during a build every
  row leaves the tab order and ignores its keys (R1).
- No reload key may reload the webview, in any state: call `preventDefault`
  on F5 with any modifier (F5, Shift+F5, Ctrl+F5) and on Ctrl+R and
  Ctrl+Shift+R. Only bare F5 and Ctrl+R (no Shift) run Reload manifest; the
  other variants do nothing else (R2).
- Ctrl+Enter follows `session.canBuild` only. Never add an `errorCount`
  condition to it, and never open a dialog because errors exist: building
  with errors is allowed, and the result reports what was left out.
- F8 is the "go to errors" key (it is the next-error key in common Windows
  editors, and sits beside F5 in this scheme). It calls the same
  `showErrors()` as the bar's **Show errors**.
- The table uses a roving `tabindex`: one row is in the tab order, and the
  arrow keys move between rows. The tab-stop row's own Browse… and Clear
  stay in the tab order after it (`tabIndex={0}`, Clear only when enabled);
  every other row's Browse… and Clear are `tabIndex={-1}`, so the table is
  one stop plus the focused row's actions (confirmed 2026-09-30).
- The Format picker gets no shortcut of its own: it is reached with Tab and
  operated with the native `<select>` keys (arrows, type-ahead, Alt+Down).
  Verify that order: Choose…, Format, Show errors (only when the manifest has
  errors), Create archive.
- A **Keyboard shortcuts** help button in the header (a quiet `Button` with
  the `keyboard` icon and a visible label) opens a popover listing the table
  above. Use the native `popover` attribute, anchored under the button, with
  `--surface`, a 1 px `--border`, `--radius`, and `--shadow-overlay`. It is
  not a modal. Render keys in `<kbd>` at `--font-size-sm` on
  `--surface-sunken` with a 1 px `--border`. Escape closes it and returns
  focus to the button. While the popover is open, Escape belongs to it
  even when focus is elsewhere: the view's Escape shortcut neither
  dismisses a result nor calls `preventDefault`, so the browser's own
  light dismiss closes the popover; the next Escape dismisses the result
  (R6).
  - **Placement (confirmed 2026-09-30).** With a manifest, the help button is
    the last control in `ManifestHeader`'s actions. In the no-manifest state
    it sits beside the `h1` in `.tarpack__titlebar`. In the loading state it
    is not rendered.
  - **Position (confirmed 2026-09-30).** In windows at least 40rem tall, and
    where CSS anchor positioning is supported (Chromium 125+, which current
    Evergreen WebView2 is), it is anchored under the button with
    `position-try-fallbacks: flip-block`. Otherwise, and in any window under
    40rem tall, it is pinned to the top right of the window
    (`top: var(--space-4); right: var(--space-5)`). It scrolls inside
    itself. Accepted limitation (P3): its `max-height` is measured from the
    viewport, not from its anchored top, so in a tall window with the view
    scrolled it can run past the bottom edge; the list is nine rows, so this
    is left as is. Visible shortcut hints (`title` and `aria-keyshortcuts`) go
  on the corresponding buttons, including `aria-keyshortcuts="F8"` on the
  build bar's **Show errors**.

### Polish targets

- A visible focus state on every interactive element, in `--focus-ring`, from
  `src/styles/base.css`.
- **The focused table row** (roving focus): the row's `:focus-visible` gets the
  global ring, inset (`outline-offset: -2px`), plus the `--surface-sunken`
  hover fill, and U3's rule shows its full Windows path. Do not add a colored
  side stripe. Moving focus with Up/Down scrolls the row into view
  (`scrollIntoView({ block: "nearest" })`, instant under reduced motion; as
  landed, a row taller than half the window, `offsetHeight >
  innerHeight / 2`, which happens at 200% text with its path revealed,
  uses `block: "start"` instead, so its top aligns under the sticky header
  and its name stays readable: `EntryTable.tsx` lines 238-241), and
  the focused row is never hidden under the sticky header or the bar (WCAG
  2.4.11); check it at the top and bottom of a 200-row table in both
  layouts.
- **Row-action shortcut hints.** Add `aria-keyshortcuts="Enter"` to Browse…
  and `aria-keyshortcuts="Delete"` to Clear. Append the hint to Clear's
  existing `title` rather than replacing it: "Forget this file (nothing is
  deleted). Shortcut: Delete". Browse… gets the `title` "Shortcut: Enter".
  Do not change either accessible name.
- **Tool nav (U1 drift).** In `src/styles/app.css`, remove the 3 px
  `border-left` from `.tool-nav__item` and its `[aria-current="page"]` rule.
  The current item keeps its `--surface` fill, 1 px `--border` outline, and
  weight 600, plus a 6 px `--accent` dot before the label (a pseudo-element,
  `aria-hidden` by nature), so the state is not carried by weight alone. In
  `DESIGN.md`, remove the "Known drift" note under Navigation and describe the
  dot.
- **Vocabulary sweep.** No `<button>` outside `Button.tsx` and `ToolNav.tsx`;
  no glyph icons; no literal `font-size`; no `box-shadow` except
  `--shadow-overlay` on the Recent menu, the dialog, and the shortcuts
  popover; every selector in `tarpack.css` starts with `.tarpack`; no colored border thicker than 1 px except the drop
  overlay's dashed border and the focus ring; no literal color (`#…`,
  `rgb(`, `hsl(`) in any stylesheet except `tokens.css` (R4 moves the last
  one, the dialog backdrop, to `--scrim`).
- **What "200% text" means here (confirmed 2026-09-30).** The check is a
  **400×280** viewport at the default root font size: the same CSS layout
  as the 800×560 minimum window at 200% zoom, and it triggers media and
  container queries as real zoom does. A 32 px root font at 800×560 is not
  used: it does not trigger media queries and does not scale px values the
  way zoom does.
- The layout is intact at 200% text size and at the 800×560 minimum,
  including the worst case: the error report expanded, a build result with
  its report, and the sticky bar all at once. The view scrolls (one scroll
  container, per the scroll model above), the table and every control stay
  reachable, and nothing is clipped or overlaps the bar. At 1000×700, at
  800×560, and at 200% text the table is stacked; at 1400×800 it is the
  seven-column layout, and its header sticks flush to the top edge; the bar
  sits flush to the bottom edge; there is no horizontal scroll; assigning a
  file moves no column. In the
  accessibility tree, the stacked table still has the table role and each
  cell's column header name.
- No color-only meaning anywhere. Re-check the status icons, the CRLF → LF
  marker, the progress step label, banners, overlay, the error region, the
  bar's left-out note, and the result heading with files left out.
- Long unbroken strings wrap or truncate as designed at 800 px and 200% text:
  Windows paths (middle-truncated), long-name warning messages, error
  messages in both reports, and the extraction command (wrapped, never
  truncated). Both reports stay within `40vh` and scroll inside themselves.
- **Middle truncation in WebView2 (moved out of this run, 2026-09-30).**
  This check needs Windows, and the implementer's container is Linux. It is
  not an acceptance criterion of this task: the user runs it on Windows as
  part of M7's Windows end-to-end check, before M7 is signed off. Your
  report says "WebView2 truncation check: not run (Linux); tracked for M7".
  The check, for the record: in the running app (`npm run
  tauri:dev` on Windows, so the real Cascadia Mono or Consolas is used),
  every truncated Windows path, in the table and in the build bar's output
  path, shows "…" directly against the file name's leading `\`, with no gap,
  and never splits the file name mid-word. Check several window widths,
  resizing a pixel or two at a time, in both table layouts and at 200% text.
  `MiddlePath`'s line snaps to whole characters with
  `width: calc(round(down, 100% - 1px, 1ch) + 0.5px)`, and its tail is
  `flex: 0 0 auto; max-width: calc(100% - 4ch)`. These were verified only in
  Chromium on Linux with a fallback mono; U3's sandbox could not check
  WebView2's fonts. If a gap or a mid-word break appears, report it with a
  screenshot and the window width rather than changing the rules
  piecemeal. A failure found there becomes a follow-up UI task; it does not
  reopen U6.
- Consistent spacing on the `--space-*` scale, with no one-off pixel values.
- All copy is plain and short. Re-read every string. Keep the no-manifest build bar
  honest: the Format picker (showing "tar (.tar)") and Choose… stay disabled.
  Keep the error copy honest: say "error", never "problem"; nothing says
  errors block or prevent building, except the "no files can be built"
  cases; the summary never says "All … ready" while files are left out; and
  nothing suggests the build report is saved or logged.

**Rules that bind this task.**

- Tokens only.
- Do not change behaviour or the `lib/` API. If polish reveals a missing
  capability, report it as a blocker for the backend implementer.

## Files

- `src/tools/tarpack/useTarpackShortcuts.ts`
- `src/tools/tarpack/ShortcutsHelp.tsx`
- Touch-ups across `src/tools/tarpack/`, `src/app/`, and `src/styles/`
  (including the nav fix in `app.css`); the shortcuts popover's styles in
  `src/styles/tarpack.css`, scoped under `.tarpack`
- The root `DESIGN.md`: the Navigation entry, plus the entries R3 and R4
  name (shortcuts popover, `<kbd>`, focused row, `--scrim`), plus any drift
  the audit proves
- `src/styles/tokens.css` and `src/styles/tokens.test.ts`: the `--scrim`
  token (R4)
- Tests

## Skill

`/impeccable polish`, then `/impeccable audit`. Fix every P0 and P1 finding,
and list any P2 findings you deliberately defer in your report.

How to run it:

- Start with the skill's `impeccable context`, which loads the root
  `PRODUCT.md` and `DESIGN.md`.
- This is an Operate surface extending an established world, so run no
  concept round.
- Read the skill's `reference/craft-floor.md` before the first edit.
- **The audit covers U5's surfaces too.** U5's `/impeccable audit` was
  waived to this task (2026-09-30): include the build bar (every state,
  including building and the short-window static bar), the replace
  confirmation dialog, the build result, and the build report.
- **Known finding to fix (from U5's rendered check).** U2's manifest header
  overflows horizontally by about 3 px at a 400×280 viewport (200% text at
  the minimum window). Fix it in `tarpack.css`, scoped under `.tarpack`.
- **200% text and media queries.** Setting the root font size to 32 px
  does not trigger media queries; check anything that depends on one (the
  bar's short-window fallback) at a 400×280 viewport instead. Chromium is
  pre-installed at `/opt/pw-browsers/chromium` with Playwright installed
  globally; do not run `playwright install`.
- The audit includes the deterministic detector. Run
  `impeccable detect --json src/` from `apps/desktop` and verify each
  finding in context.
- Capture screenshots of the running frontend (`npm run dev` in
  `apps/desktop`, with `lib/` mocked or in the Tauri dev shell) at 1280×800
  and at 800×560, in light and dark, and at 200% text (400×280, see Polish
  targets). Use them as the audit's evidence. This session's plan audit could run neither the detector
  nor screenshots, so this is the first rendered check. The exact set, and
  what the report must contain, is in "The audit deliverables" under the
  review fixes below.
- If the skill is not installed, install it with `npx impeccable install`. If
  its launcher cannot run, follow `reference/polish.md` and
  `reference/audit.md` from `github.com/pbakaus/impeccable` by hand, and say
  that the detector did not run.
- `DESIGN.md` is updated only for the nav fix above, for R3 and R4, and for
  any drift the audit proves.

## Acceptance criteria

- Every shortcut in the table works in its "active when" condition, and does
  nothing outside it.
- No reload key reloads the webview (F5, Shift+F5, Ctrl+F5, Ctrl+R,
  Ctrl+Shift+R), in any state, including during a build; only F5 and Ctrl+R
  run Reload manifest.
- Roving focus in the table works with Up and Down, and Enter and Delete act
  on the focused row, in the table and stacked layouts. The focused row shows
  its full Windows path and is never obscured by the sticky header or bar.
- During a build no row is in the tab order and no row key (Up, Down,
  Enter, Delete) does anything; after the build the tab stop is back on the
  row that had it, unless the user clicked another row during the build.
- Clearing a row from its Clear button (click, Enter, Space, or Delete)
  leaves focus on that row, never on `<body>`.
- The shortcuts help is reachable by keyboard and lists every shortcut,
  including F8. With it open and focus elsewhere, Escape does not dismiss a
  drop or build result.
- F8 moves focus to the error report when `errorCount > 0`, and does nothing
  otherwise. Ctrl+Enter builds a `canBuild: true` session that has errors,
  with no dialog.
- The `/impeccable audit` report has no open P0 or P1 findings, and your
  report contains everything listed in "The audit deliverables" below: the
  numeric health score table, the detector output with a verdict per
  finding, and the screenshot list. A written description of a rendered
  check does not replace any of the three.
- `controls.css` has no literal color; the dialog backdrop uses `--scrim`,
  defined in all three theme blocks of `tokens.css`. `DESIGN.md` describes
  the shortcuts popover as landed, the `<kbd>` style, and `--scrim`.
- `.tool-nav__item` has no border thicker than 1 px, and the current item
  shows the accent dot. `DESIGN.md` no longer lists the drift.
- The vocabulary sweep above finds nothing.
- Axe passes with no violations on every `TarpackView` state: no manifest,
  errors with entries shown (report expanded and collapsed), errors with
  entries withheld, partial, ready, ready with files left out, building
  (writing and verifying), success (with the extraction command and
  normalised entries), success with files left out (with the report), and
  error.
- Tab order through the build bar is Choose…, Format, Show errors (when the
  manifest has errors), Create archive, and the Format picker is fully usable
  without a mouse.

## Tests proving completion

`npm run test`:

- `useTarpackShortcuts.test.tsx`: each shortcut, its active and inactive
  conditions (F8 with and without errors; Ctrl+Enter on a `canBuild: true`
  session with errors), and suppression during the dialog and the build;
  every reload variant prevented in every state (R2); Escape while the
  shortcuts popover is open (R6).
- `EntryTable.keyboard.test.tsx`: roving focus, Enter, and Delete; Escape
  with focus on the drop result's Dismiss moves focus to the tab-stop row,
  and Escape with focus elsewhere leaves focus where it was; the `disabled`
  prop (R1); focus stays on the row after Clear (R5).
- `TarpackView.build.test.tsx` (extend U5's): Enter and Delete on the
  tab-stop row during a real pending build call neither the file dialog nor
  `clear` (R1).
- `ShortcutsHelp.test.tsx`: opens from the button by keyboard, lists every
  shortcut, and closes on Escape, returning focus; reports its open state
  through `onOpenChange`, including `false` on unmount (R6).
- `tokens.test.ts`: `--scrim` in each theme block (R4).
- `ToolNav.test.tsx` (extend U1's): the current item still has
  `aria-current="page"` after the style change.
- `TarpackView.a11y.test.tsx`: axe over every state.

## States covered

All states of the view.

## Fixes required after review (2026-09-30)

The first run landed at ee3da63 and the reviewer returned "changes
required". Apply R1–R8 on top of ee3da63, in this order, then re-run the
audit as "The audit deliverables" says. Line numbers are against ee3da63.
Everything else in ee3da63 stands, including these choices, which are now
part of this plan (see Shortcuts and Polish targets above): non-tab-stop
rows' Browse… and Clear are `tabIndex={-1}`; the help button sits beside
the `h1` when no manifest is open; "200% text" is a 400×280 viewport; the
popover is top-pinned under 40rem of window height and where anchor
positioning is missing.

### R1 (P1). Row keys act during a build

`TarpackView.tsx` (~line 402) disables the header and table with
`<fieldset disabled={building}>`, but that disables only form controls. A
`<tr tabIndex={0}>` stays focusable, and `EntryTable`'s `tbody` `onKeyDown`
(`EntryTable.tsx` ~lines 207-232) still runs: Enter on the tab-stop row
mid-build opened the file dialog, and Delete would call `onClear`.

- Add a prop to `EntryTable`: `disabled?: boolean` (default `false`).
  `TarpackView` passes `disabled={building}`.
- While `disabled`:
  - every row renders `tabIndex={-1}`, and so do its Browse… and Clear
    (they are also disabled by the fieldset). Do this by passing
    `tabStop={!disabled && e.id === tabId}` to `EntryRow`, so the memo
    comparator (`EntryTable.tsx` ~line 169) sees the change;
  - `onKeyDown` returns at once for every key, before the arrow handling:
    no Up/Down, Enter, or Delete;
  - do not use `inert` and do not blur: a row that holds focus when the
    build starts keeps it (a `tabIndex={-1}` element can still hold focus),
    so U5's "return focus to Create archive only when it was lost" rule
    leaves it there after the build.
- No row key changes `stopId` while disabled, so after a build driven from
  the keyboard the tab stop is back on the same row. (Reworded at
  close-out: `onFocus` is not gated on `disabled`, so clicking a row during
  a build focuses it and moves the tab stop to it. That is accepted; the
  pointer put focus there, and after the build Tab returns to that row.)
- Tests:
  - `EntryTable.keyboard.test.tsx`: render with `disabled`; assert every
    `tbody tr` and every row button has `tabindex="-1"`; focus a row
    programmatically and press Enter, Delete, and ArrowDown: `onBrowse` and
    `onClear` are not called and focus does not move. Rerender with
    `disabled={false}`: the row that was the tab stop before is
    `tabindex="0"` again.
  - `TarpackView.build.test.tsx`: with an assigned entry, focus the
    tab-stop row, start a build with a pending `build` promise, press Enter
    and then Delete on the row: the mocked `openFileDialog` and `clear` are
    not called. Settle the build; Enter on the row then calls
    `openFileDialog`.

### R2 (P2). Every reload key must be blocked

`useTarpackShortcuts.ts` (~line 39) prevents only bare F5 and Ctrl+R.
Shift+F5, Ctrl+F5, and Ctrl+Shift+R still reload WebView2 and lose the
session. Change the first branch to:

- if `e.key === "F5"` with any modifiers, or `e.ctrlKey && key === "r"`
  (with or without Shift; `key` is already lower-cased), call
  `e.preventDefault()`;
- then, only when the chord is bare F5 or Ctrl+R without Shift (no Alt, no
  Meta), and `active && hasManifest && !e.repeat`, call `onReload()`;
- return in every case.

The shortcuts help lists only F5 and Ctrl+R; do not add the blocked
variants to it. Test in `useTarpackShortcuts.test.tsx`: for each of
Shift+F5, Ctrl+F5, Ctrl+Shift+R, with a manifest and with `active: true`
and then `active: false`, the event's `defaultPrevented` is `true` and
`onReload` is not called; bare F5 and Ctrl+R still call it once.

### R3 (P2). `DESIGN.md` is stale about the popover and the keys

Edit the root `DESIGN.md`:

- **Elevation & Depth** (~lines 185-189): "the shortcuts popover (planned,
  U6)" becomes "the shortcuts popover (landed, U6, `ShortcutsHelp`)". The
  backdrop sentence is in R4.
- Add a section after **Banners**, headed `### Shortcuts help and keys
  (Landed, U6)`, with:
  - **Help button:** a quiet `Button` with the `keyboard` icon and the
    visible label "Keyboard shortcuts"; the last header action, or beside
    the `h1` when no manifest is open.
  - **Popover:** native `popover="auto"`, `role="dialog"`, not modal;
    `--surface`, 1 px `--border`, `--radius`, `--shadow-overlay`, padding
    `--space-3 --space-4`; anchored under the button in windows at least
    40rem tall where anchor positioning is supported, else pinned to the top
    right; scrolls inside itself. Escape closes it and returns focus to the
    button.
  - **Keys:** `<kbd>` is inline-block, `--font-mono` at `--font-size-sm`,
    on `--surface-sunken` with a 1 px `--border` and `--radius`, padding
    `0 --space-1`. A chord joins keys with an `aria-hidden` "+"; alternatives
    are joined by "or" in `--text-muted`. The "active when" column is
    `--text-muted` at `--font-size-sm`.
  - **Hints on controls:** every button with a shortcut has
    `aria-keyshortcuts` and a `title` "Shortcut: …" (Clear appends it to its
    own `title`).
- **Data table**: change its heading from "(Planned, U3)" to "(Landed, U3;
  focused row U6)" and add one bullet: "**Focused row.** Roving focus: one
  row in the tab order, Up/Down move, Enter browses, Delete clears. The
  focused row gets the global ring inset (`outline-offset: -2px`) and the
  `--surface-sunken` fill, and shows its full Windows path. No side stripe."

Check each statement against `tarpack.css` as it stands after your fixes;
where the code differs, fix the code if the plan above says so, otherwise
describe the code.

### R4 (P2, decided: fix now). The dialog backdrop is a literal color

`controls.css` (~lines 180-182) has
`.confirm-dialog::backdrop { background: rgb(0 0 0 / 0.4); }`. Add a token:

| Token | Light | Dark | Use |
| --- | --- | --- | --- |
| `--scrim` | `rgb(0 0 0 / 0.4)` | `rgb(0 0 0 / 0.6)` | the backdrop behind a modal dialog, only |

- Define it in all three blocks of `src/styles/tokens.css`: `:root`, the
  `@media (prefers-color-scheme: dark)` `:root:not([data-theme="light"])`
  block, and `:root[data-theme="dark"]` (the two dark blocks must stay
  identical; `tokens.test.ts` asserts that).
- `controls.css`: `background: var(--scrim);`.
- `tokens.test.ts`: a test per theme block, like the overlay-shadow one,
  that `--scrim` is `rgb(0 0 0 / 0.4)` in light and `rgb(0 0 0 / 0.6)` in
  both dark blocks.
- `DESIGN.md`, Elevation & Depth: "The dialog also sits over a dimmed
  backdrop." becomes "The dialog also sits over a dimmed backdrop,
  `--scrim` (`rgb(0 0 0 / 0.4)` light, `rgb(0 0 0 / 0.6)` dark), used for
  nothing else."
- Look at the open dialog in both themes in the rendered check: the dialog
  must stand clear of the dimmed view in dark.

This clears the detector's one advisory from the first run.

### R5 (P2). Clear drops focus to `<body>`

Delete (or Enter, Space, or a click) on a focused Clear button clears the
row, Clear becomes disabled, and focus falls to `<body>`
(`EntryTable.tsx` ~lines 226-229, and the Clear `onClick` ~line 160).

- In `EntryTable`, route every Clear through one function: given the row
  element and the entry id, if `document.activeElement` is inside that row
  and is not the row itself, call `row.focus({ preventScroll: true })`
  first, then `onClear(id)`. The Delete key handler and Clear's `onClick`
  both use it (in `EntryRow`, get the row from `event.currentTarget.closest("tr")`).
- Focus on the row is not lost, so the view's focus rules leave it there,
  and U3's reveal shows the full path of the row just cleared (now "—").
- Tests in `EntryTable.keyboard.test.tsx`, with an assigned tab-stop row:
  focus its Clear and press Delete: `onClear` is called once and the row
  has focus; focus its Clear and click it: same; focus the row itself and
  press Delete: `onClear` is called and focus stays on the row.

### R6 (P3). Escape with the popover open and focus elsewhere

`ShortcutsHelp.tsx` (~lines 32-40) handles Escape only when it bubbles from
the button or the popover. With the popover open and focus elsewhere, the
view's Escape shortcut dismisses a result and its `preventDefault` stops
the popover's own light dismiss.

- `ShortcutsHelp` gets a prop `onOpenChange?: (open: boolean) => void`,
  called from `onToggle` with the new state, and called with `false` from
  an unmount cleanup (the header and no-manifest instances swap when a
  manifest opens or fails).
- `TarpackView` keeps `const [shortcutsOpen, setShortcutsOpen] =
  useState(false)`, passes `onOpenChange={setShortcutsOpen}` to whichever
  `ShortcutsHelp` it renders (the header one goes through a new
  `ManifestHeader` prop of the same name), and passes `shortcutsOpen` to
  `useTarpackShortcuts`.
- In the hook, add `shortcutsOpen: boolean` to `TarpackShortcutOptions`.
  In the Escape branch, when `shortcutsOpen` is true, return without
  `preventDefault` and without dismissing. Other shortcuts are unchanged
  (the popover is not modal).
- Tests: in `useTarpackShortcuts.test.tsx`, with `shortcutsOpen: true` and
  a visible build result, Escape dispatched on `document.body` does not call
  `onDismissBuildResult` or `onDismissDropResult`, and `defaultPrevented`
  is `false`; with `shortcutsOpen: false` it dismisses as before. In
  `ShortcutsHelp.test.tsx`, `onOpenChange` receives `true` then `false` on
  toggle, and `false` on unmount while open.

### R7 (P3, accepted). Popover height from the viewport

The anchored popover's `max-height` is measured from the viewport, not
from its anchored top (`tarpack.css` ~lines 120-130). Accepted; change
nothing. It is recorded under Shortcuts.

### R8. The audit deliverables

The first run gave detector output and a written rendered check at five
sizes, but no health score and no screenshots. After R1–R7, re-run
`/impeccable audit` over the whole Tar Packager view (U2–U6's surfaces,
including U5's bar, dialog, result, and report) against the rendered page,
fix every P0 and P1, and put all of the following in your report:

1. **Health score.** The audit's score table: Accessibility, Performance,
   Responsive, Theming, and Implementation integrity, each 0–4, with one
   key finding per row, and the total out of 20 with its band (for example
   "18/20, Excellent").
2. **Findings.** Every finding with its severity (P0–P3), where it is, and
   its status: fixed (in which file), or deferred with the reason. P2s you
   defer are listed by name.
3. **Detector.** The exact command you ran (`impeccable detect --json
   src/`, run from `apps/desktop`), its exit status, and its JSON output, in full
   if it is under about 100 lines, otherwise a count per rule plus every
   finding; for each finding, file:line and your verdict (fixed, or false
   positive and why). If it cannot run, say so and why; that does not
   waive items 1, 2, and 4.
4. **Screenshots.** PNG files taken with the pre-installed Chromium and
   Playwright against the Vite dev server with the Tauri IPC mocked (the
   fixtures in `src/tools/tarpack/fixtures.ts`). Save them outside the
   repository, in your session's scratchpad directory, and do not commit
   them. List each with its absolute path, viewport, theme, state, and one
   line on what you saw in it. You must open and look at each one. The
   minimum set:
   - 1280×800, light and dark: the worst case (error report expanded, a
     success result with files left out and its report, the bar);
   - 1280×800, light: the replace dialog open; the shortcuts popover open;
   - 1400×800, light: the seven-column table with a focused row mid-table
     (full path revealed, header sticky);
   - 800×560, light and dark: the worst case; the bar while building
     (verifying);
   - 400×280 (200% text), light: the worst case scrolled to the bar (the
     short-window static bar); the shortcuts popover open (top-pinned);
     the stacked table with a focused row.

   If a screenshot shows a defect, fix it and replace the screenshot.
5. **Not run.** "WebView2 truncation check: not run (Linux); tracked for
   M7" (see Polish targets).

## Out of scope

- New features or behaviour changes.
- Theme toggle (decided: the theme follows the system only).
- Packaging (M7).
