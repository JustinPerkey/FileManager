# U6 — Keyboard shortcuts, final polish, and audit

Status: awaiting approval
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
- `EntryTable.tsx`, with per-row Browse… and Clear, `EntryStatus.tsx`, and
  `EolMarker.tsx` (the non-interactive "CRLF → LF" marker on rows whose line
  endings are converted);
- `DropResult.tsx`;
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
  hover fill. Do not add a colored side stripe.
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
  its report, and the sticky bar all at once. The view scrolls, the table
  and every control stay reachable, and nothing is clipped or overlaps the
  bar.
- No color-only meaning anywhere. Re-check the status icons, the CRLF → LF
  marker, the progress step label, banners, overlay, the error region, the
  bar's left-out note, and the result heading with files left out.
- Long unbroken strings wrap or truncate as designed at 800 px and 200% text:
  Windows paths (middle-truncated), long-name warning messages, error
  messages in both reports, and the extraction command (wrapped, never
  truncated). Both reports stay within `40vh` and scroll inside themselves.
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
  on the focused row.
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
- `EntryTable.keyboard.test.tsx`: roving focus, Enter, and Delete.
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
