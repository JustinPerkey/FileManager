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
  report** for the rest. It has a notice strip that is always visible while
  `session.manifest.errorCount > 0` ("3 errors in this manifest", with
  **Show errors** / **Hide errors** and Edit in editor), and a collapsible,
  focusable report body (`id="manifest-error-report"`) listing the
  whole-manifest errors and each failed entry with all of its errors. A
  polite live-region announcer in `TarpackView` announces the error count
  after each open, reload, and session restore. `TarpackView` has a
  `showErrors()` handler that expands the report and moves focus to it;
- `EntryTable.tsx`, with per-row Browse… and Clear, `EntryStatus.tsx`, and
  `EolMarker.tsx` (the non-interactive "CRLF → LF" marker on rows whose line
  endings are converted);
- `DropResult.tsx`;
- `BuildBar.tsx` (output path with Choose…, the **Format** picker — a labelled
  native `<select>` over the four formats tar, gzip, zstd, xz — the disabled
  reason, which for manifest errors reads "Fix N manifest errors first" with a
  **Show errors** button, and the **Create archive** button), `FormatPicker.tsx`, and `BuildProgress.tsx`
  (two labelled steps, "Writing" then "Verifying", each 0 → 100%);
- `BuildResult.tsx`: on success, the SHA-256 and the target extraction command
  (for example `tar --zstd --no-overwrite-dir -xpPf gateway.tar.zst`), each
  with a **Copy** button, the list of entries whose line endings were
  converted, and Show in folder; on error, a message and a Details
  disclosure.
- `errorMessages.ts`: the single, exhaustive
  `Record<TarpackErrorKind, …>` of user-facing error copy (17 kinds). When
  re-reading copy, edit it there; no component holds its own error strings.

Under `src/app/`:

- `AppShell.tsx`, `ToolNav.tsx`, `Banner.tsx`, `DropZone.tsx`, and
  `ConfirmDialog.tsx`.

All actions already exist as handlers in `TarpackView`. This task adds keyboard
access to them; it adds no new behaviour.

### Shortcuts

| Keys | Action | Active when |
| --- | --- | --- |
| Ctrl+O | Open manifest… | always |
| F5 or Ctrl+R | Reload manifest | a manifest is loaded |
| Ctrl+E | Edit in editor | a manifest is loaded |
| Ctrl+Enter | Create archive | `session.canBuild` |
| F8 | Show errors: expand the error report and move focus to it (`showErrors()`) | `session.manifest.errorCount > 0` |
| Up / Down | Move between table rows | focus is in the table |
| Enter | Browse… for the focused row | a row is focused |
| Delete | Clear the focused row | the row is assigned |
| Escape | Dismiss the drop result or build result | one is visible |

- Shortcuts are ignored while `ConfirmDialog` is open or a build is running.
- Ctrl+R and F5 must not reload the webview; call `preventDefault`.
- The table uses a roving `tabindex`: one row is in the tab order, and the
  arrow keys move between rows.
- The Format picker gets no shortcut of its own: it is reached with Tab and
  operated with the native `<select>` keys (arrows, type-ahead, Alt+Down).
  Verify that order: Choose…, Format, Create archive; with manifest errors,
  Choose…, Format, Show errors (Create archive is disabled and skipped).
- F8 is the conventional "next error" key in Windows developer tools. From
  anywhere in the view it lands on the error report; the report body is one
  tab stop, and Tab or Shift+Tab leaves it as usual. F8 never moves focus
  when there are no errors.
- A **Keyboard shortcuts** help button in the header opens a popover listing
  the table above. Visible shortcut hints (`title` and `aria-keyshortcuts`) go
  on the corresponding buttons, including both **Show errors** buttons
  (notice strip and build bar: `aria-keyshortcuts="F8"`).

### Polish targets

- A visible focus state on every interactive element, in `--focus-ring`, from
  `src/styles/base.css`.
- The layout is intact at 200% text size and at the 800×560 minimum.
- No color-only meaning anywhere. Re-check the status icons, the CRLF → LF
  marker, the progress step label, banners, the error notice strip (its
  `--danger` edge and icon must be backed by the count text), and overlay.
- Long unbroken strings wrap or truncate as designed at 800 px and 200% text:
  Windows paths (middle-truncated), long-name warning messages, error
  messages and long entry ids in the error report, and the extraction command
  (wrapped, never truncated).
- The error report with many failed entries stays within its height cap and
  scrolls inside its region at 800×560 and at 200% text.
- Consistent spacing on the `--space-*` scale, with no one-off pixel values.
- All copy is plain and short. Re-read every string. Keep the no-manifest build bar
  honest: the Format picker (showing "tar (.tar)") and Choose… stay disabled.

**Rules that bind this task.**

- Tokens only.
- Do not change behaviour or the `lib/` API. If polish reveals a missing
  capability, report it as a blocker for the backend implementer.

## Files

- `src/tools/tarpack/useTarpackShortcuts.ts`
- `src/tools/tarpack/ShortcutsHelp.tsx`
- Touch-ups across `src/tools/tarpack/` and `src/app/`
- Tests

## Skill

`impeccable:polish`, then `impeccable:audit`. Fix every P0 and P1 finding, and
list any P2 findings you deliberately defer in your report.

## Acceptance criteria

- Every shortcut in the table works in its "active when" condition, and does
  nothing outside it.
- Ctrl+R and F5 do not reload the webview.
- Roving focus in the table works with Up and Down, and Enter and Delete act
  on the focused row.
- F8 expands a collapsed error report and focuses it when
  `errorCount > 0`, and does nothing when it is 0.
- The shortcuts help is reachable by keyboard and lists every shortcut.
- The `impeccable:audit` report has no open P0 or P1 findings.
- Axe passes with no violations on every `TarpackView` state: no manifest,
  manifest errors with passed entries (report expanded and collapsed),
  manifest errors with entries withheld, partial, ready, building (writing and verifying), success (with the
  extraction command and normalised entries), and error.
- Tab order through the build bar is Choose…, Format, Create archive (with
  manifest errors: Choose…, Format, Show errors), and the Format picker is
  fully usable without a mouse.

## Tests proving completion

`npm run test`:

- `useTarpackShortcuts.test.tsx`: each shortcut (F8 included), its active
  and inactive conditions, and suppression during the dialog and the build.
- `EntryTable.keyboard.test.tsx`: roving focus, Enter, and Delete.
- `ShortcutsHelp.test.tsx`
- `TarpackView.a11y.test.tsx`: axe over every state.

## States covered

All states of the view.

## Out of scope

- New features or behaviour changes.
- Theme toggle (decided: the theme follows the system only).
- Packaging (M7).
