# U6 — Keyboard shortcuts, final polish, and audit

Status: awaiting approval (amended 2026-09-30: path reveal on the focused row,
row-action names and hints, layout and scroll model from U3; stacked layout
at the default window, 64rem switch, sticky offsets; WebView2 check of
middle truncation; focus after dismissing the drop result, from U4)
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
    (the sticky header), and a `scroll-padding-bottom` for the bar (U5).
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
- Ctrl+R and F5 must not reload the webview; call `preventDefault`.
- Ctrl+Enter follows `session.canBuild` only. Never add an `errorCount`
  condition to it, and never open a dialog because errors exist: building
  with errors is allowed, and the result reports what was left out.
- F8 is the "go to errors" key (it is the next-error key in common Windows
  editors, and sits beside F5 in this scheme). It calls the same
  `showErrors()` as the bar's **Show errors**.
- The table uses a roving `tabindex`: one row is in the tab order, and the
  arrow keys move between rows.
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
  focus to the button. Visible shortcut hints (`title` and `aria-keyshortcuts`) go
  on the corresponding buttons, including `aria-keyshortcuts="F8"` on the
  build bar's **Show errors**.

### Polish targets

- A visible focus state on every interactive element, in `--focus-ring`, from
  `src/styles/base.css`.
- **The focused table row** (roving focus): the row's `:focus-visible` gets the
  global ring, inset (`outline-offset: -2px`), plus the `--surface-sunken`
  hover fill, and U3's rule shows its full Windows path. Do not add a colored
  side stripe. Moving focus with Up/Down scrolls the row into view
  (`scrollIntoView({ block: "nearest" })`, instant under reduced motion), and
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
  overlay's dashed border and the focus ring.
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
- **Middle truncation in WebView2.** In the running app (`npm run
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
  piecemeal.
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
- The root `DESIGN.md`: the Navigation entry only, plus any drift the audit
  proves
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
- The audit includes the deterministic detector. Run
  `impeccable detect --json` over `apps/desktop/src/` and verify each finding
  in context.
- Capture screenshots of the running frontend (`npm run dev` in
  `apps/desktop`, with `lib/` mocked or in the Tauri dev shell) at 1280×800
  and at 800×560, in light and dark, and at 200% text. Use them as the
  audit's evidence. This session's plan audit could run neither the detector
  nor screenshots, so this is the first rendered check.
- If the skill is not installed, install it with `npx impeccable install`. If
  its launcher cannot run, follow `reference/polish.md` and
  `reference/audit.md` from `github.com/pbakaus/impeccable` by hand, and say
  that the detector did not run.
- `DESIGN.md` is updated only for the nav fix above, and for any drift the
  audit proves.

## Acceptance criteria

- Every shortcut in the table works in its "active when" condition, and does
  nothing outside it.
- Ctrl+R and F5 do not reload the webview.
- Roving focus in the table works with Up and Down, and Enter and Delete act
  on the focused row, in the table and stacked layouts. The focused row shows
  its full Windows path and is never obscured by the sticky header or bar.
- The shortcuts help is reachable by keyboard and lists every shortcut,
  including F8.
- F8 moves focus to the error report when `errorCount > 0`, and does nothing
  otherwise. Ctrl+Enter builds a `canBuild: true` session that has errors,
  with no dialog.
- The `/impeccable audit` report (with detector output, or a stated reason it
  could not run) has no open P0 or P1 findings, and its health score is
  included in your report.
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
  session with errors), and suppression during the dialog and the build.
- `EntryTable.keyboard.test.tsx`: roving focus, Enter, and Delete; Escape
  with focus on the drop result's Dismiss moves focus to the tab-stop row,
  and Escape with focus elsewhere leaves focus where it was.
- `ShortcutsHelp.test.tsx`: opens from the button by keyboard, lists every
  shortcut, and closes on Escape, returning focus.
- `ToolNav.test.tsx` (extend U1's): the current item still has
  `aria-current="page"` after the style change.
- `TarpackView.a11y.test.tsx`: axe over every state.

## States covered

All states of the view.

## Out of scope

- New features or behaviour changes.
- Theme toggle (decided: the theme follows the system only).
- Packaging (M7).
