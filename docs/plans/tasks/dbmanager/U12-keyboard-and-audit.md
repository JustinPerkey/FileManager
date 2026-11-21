# U12 — Keyboard completeness, layout pass, and audit

Status: **superseded (2026-11-21):** the UI moves to GPUI; the ui-designer rewrites this task for `apps/filemanager` (its design decisions carry over). Do not run this plan. Previous status: awaiting approval (revised 2026-11-16: the fixed config, Edit config,
pickable tables, the window-close prompt)
Project: dbmanager   Depends on: U1–U11 (landed), U13 (landed), M10 (landed),
M11 (landed), M13 (landed), M14 (landed)

## Goal

Finish the Database Manager to its accessibility and keyboard bar:

- every shortcut in the keyboard map works and is listed in the help;
- every region and state passes axe;
- the layout holds at 800×560, at 200% text, and in both themes;
- `/impeccable audit` and the detector find nothing outstanding.

## Context

**The product.** FileManager is a portable Windows desktop app (Tauri v2;
React + TypeScript + Vite in `apps/desktop/src/`). The **Database Manager**
(`dbmanager`) is an **object editor** over a SQLite file, with one app-wide
config that the user never chooses and that reloads by itself. U2–U11 and
U13 built it, in `apps/desktop/src/tools/dbmanager/` and, for the
window-close prompt, `app/closeGuard.tsx`:

- the session header with the config status, and developer-mode Edit config;
- the banners: no config, everything read-only, config reloaded;
- the panes (Tables · Objects · Sheet) with Back/Forward history;
- the object sheet with typed editors, references and their picker,
  "Referenced by", many-to-many checklists with link fields;
- save with conflicts and the leave guard;
- pickable (lookup) objects opened read-only;
- create and delete;
- the close prompt when the window closes with unsaved edits.

Its shortcuts are collected in `shortcuts.ts` (`SHORTCUTS`) and handled in
`useDbmanagerShortcuts.ts`. The shared `ShortcutsHelp` (`app/ShortcutsHelp.tsx`,
U1) renders them. This task completes and verifies; it adds no new feature.

You own `apps/desktop/src/` **except `lib/`**: never edit it. Tools never
import each other.

Accessibility is a requirement, not polish (`CLAUDE.md`, `PRODUCT.md`):

- WCAG 2.2 AA;
- every interactive element keyboard reachable and labelled, with a visible
  focus state;
- nothing conveyed by colour or icon alone;
- the layout holds at 200% text and at the 800×560 minimum window;
- motion respects `prefers-reduced-motion`.

### The keyboard map (the complete list; `SHORTCUTS` must equal it)

| Keys | Action | Active when |
| --- | --- | --- |
| Ctrl+O | Open database… | Always |
| F5 or Ctrl+R | Reload the config and the database | A database is open |
| F8 | Show config problems | The config has problems |
| Ctrl+E | Edit config (a `devOnly` row: listed and handled only in developer mode) | Developer mode is on |
| Ctrl+S | Save (Create on a new object) | The object has unsaved changes |
| Ctrl+N | New object | The table allows creating |
| Ctrl+F | Search objects | The table is searchable |
| Alt+Left, Alt+Right | Back, Forward (also the mouse back and forward buttons) | There is somewhere to go |
| F6, Shift+F6 | Move between tables, objects and the object | A database is open |
| Up, Down, Home, End | Move in the table and object lists | Focus is in a list |
| PageUp, PageDown | Previous or next page of objects | Focus is in the object list |
| Enter | Open the focused table or object | Focus is on a list item |
| Delete | Delete the focused object… (asks first) | Focus is on an object in the list, and the table allows deleting |
| Esc | Undo the edit in the focused field; close the picker or a menu | A changed field, the picker or a menu has focus |

**Rules:**

- Ctrl and Alt shortcuts work from anywhere in the view, including inside
  inputs.
- Plain keys (Delete, Enter, Page keys) act only on the list that has focus,
  and never inside a text input.
- F5, Ctrl+R and Ctrl+S always have their default prevented in the view.
- No view shortcut fires while a modal dialog is open; the dialog's own keys
  work.
- Every button that has a shortcut carries `aria-keyshortcuts` (for example
  `"Control+S"`, `"Alt+ArrowLeft"`, `"F5 Control+R"`) and a `title`
  "Shortcut: …".
- The **Keyboard shortcuts** button is the last header action.

### Focus rules (verify every row)

| Event | Focus goes to |
| --- | --- |
| Enter on a list item, two or three panes | stays on the item; "Opened {label}." announced |
| Enter on a list item, one pane | the sheet's `h2` |
| Following a reference, Back, Forward, the breadcrumb | the destination's heading |
| Choosing a table | the object list's `h2` |
| A save error | the error summary, or the sheet error banner |
| A conflict | the conflict panel's heading, or the link notice |
| A dialog opens / closes | Cancel (or the single button) / the opener |
| The picker closes | Change… |
| A delete succeeds | the object list's `h2` |
| Save succeeds with focus on Save | the sheet's `h2` |
| The close prompt opens / Keep editing | Keep editing / the element focused before the close request |
| `ConfigChangedOnDisk` on save | the sheet banner |

### Layout targets

| Window | Expected |
| --- | --- |
| 1366×800 | three panes; the sheet's two-column property layout |
| 1000×700 (the default) | two panes; the sheet stacked (label above value) |
| 800×560 | one pane; header actions wrapped; nothing scrolls horizontally; the save bar sticky and not covering the focused field |
| 800×560 at 200% text | one pane; the view scrolls as a whole when under 34rem tall; dialogs and the picker fit the window |

Both themes take every colour from tokens. `dbmanager.css` contains no hex,
and only `.dbmanager .picker` has a shadow. Skeletons and transitions are
static under `prefers-reduced-motion`. Motion anywhere is limited to 120–200
ms state changes.

### Tokens (no changes)

`--bg`, `--surface`, `--surface-sunken`, `--border`, `--text`,
`--text-muted`, `--accent`, `--accent-text`, `--ok`, `--warn`, `--danger`,
`--focus-ring`, `--shadow-overlay`, `--scrim`, `--radius`, `--space-1…6`,
`--font-size-sm/md/lg/xl`, `--font-ui`, `--font-mono`.

## Files

Edit as findings require, within `apps/desktop/src/tools/dbmanager/` and
`apps/desktop/src/styles/dbmanager.css`:

- `shortcuts.ts`: the complete table above;
- `useDbmanagerShortcuts.ts`;
- any component.

Create:

- `DbmanagerView.a11y.test.tsx` (`vitest-axe`, in the style of
  `tools/tarpack/TarpackView.a11y.test.tsx`);
- `DbmanagerView.keyboard.test.tsx`;
- `shortcuts.test.ts`.

Do not edit `app/` here. If a shared component needs a fix, report it, or
change it minimally and update `DESIGN.md` in the same change. Do not edit
`lib/`, other tools, or `tokens.css`.

## Skill

1. Run `/impeccable audit` on `apps/desktop/src/tools/dbmanager/` and
   `styles/dbmanager.css`, from `apps/desktop` with `--target
   src/tools/dbmanager/DbmanagerView.tsx`. It covers a11y, performance
   (no list renders more than its page; no request storms while typing),
   theming and responsive behaviour.
2. Run `/impeccable critique` on the whole view against the brief's
   anti-goals: no grid feel, no raw ids where a label exists, editable calm
   and read-only marked, one blue action per region, status in words.
3. Fix what they find, then run `/impeccable polish` on what changed.
4. Read `reference/craft-floor.md` before the first UI edit.
5. Run the detector once at the end:
   `"<skill dir>/scripts/impeccable" detect --json
   apps/desktop/src/tools/dbmanager apps/desktop/src/styles/dbmanager.css`.
   Fix or justify each finding in your report.

If the skill is missing, install it (`npx impeccable install`) or follow its
references by hand, and say so.

## Acceptance criteria

**Shortcuts:**

- `SHORTCUTS` equals the table above, row for row, and the help popover lists
  it.
- Each shortcut works in its "active when" condition and does nothing
  outside it.
- Ctrl and Alt shortcuts work from inside inputs. Plain keys do not fire
  inside text inputs.
- No shortcut fires while a dialog is open.
- Every button with a shortcut has `aria-keyshortcuts` and the "Shortcut:"
  title.

**Focus:**

- Every row of the focus-rules table holds, each in a test.
- F6 cycles the visible regions in the three-, two- and one-pane layouts.

**axe:**

- No violations in these states:
  - nothing open;
  - open with diagnostics;
  - config missing (the no-config banner);
  - config failed closed;
  - config reloaded, with a dropped-edits notice;
  - developer mode with Edit config;
  - a pickable object opened read-only;
  - a book open (two panes);
  - editing with inline issues and the error summary;
  - the conflict panel;
  - the picker open;
  - a checklist with staged links and link fields;
  - the create form;
  - the delete dialog (confirmable and blocked);
  - the leave dialog;
  - the close prompt.

**Layout:**

- At the four layout targets, a manual check (record it in your report,
  with screenshots from `npm run dev` in Chromium) shows the expected layout,
  no horizontal page scrolling, and the save bar never covering the focused
  control.
- Both themes are checked.

**Lint and audit:**

- `/impeccable audit` and the detector report nothing unaddressed.
- jsx-a11y lint passes, and the only disable comment is in the shared
  `ShortcutsHelp`.
- `npm run typecheck && npm run lint && npm run test` pass.

## Tests proving completion

- `shortcuts.test.ts`:
  - `table is complete and ordered`.
- `DbmanagerView.keyboard.test.tsx`:
  - `each shortcut in and out of its condition`;
  - `alt and ctrl work in inputs, plain keys do not`;
  - `no shortcuts under a dialog`;
  - `reload and save keys prevent defaults`;
  - one test per focus-rules row;
  - `F6 per layout`.
- `DbmanagerView.a11y.test.tsx`:
  - one `axe` test per state listed above.

Run: `npm run typecheck && npm run lint && npm run test`.

## States covered

Every state of U2–U11, re-verified for keyboard and axe: loading, empty,
error and success for the session, lists, sheet, checklists, picker and
dialogs.

## Out of scope

- New features or copy changes beyond what the audit requires. Report copy
  changes rather than making them, unless they fix a real defect.
- The user manual and screenshots (M12, which runs after this task).
