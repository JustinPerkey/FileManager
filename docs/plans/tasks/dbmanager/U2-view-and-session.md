# U2 — Database Manager view scaffold and session

Status: awaiting approval
Project: dbmanager   Depends on: U1 (landed), M10 (landed, including the
`configFailClosed` field requested by the ui-designer; see Context)

## Goal

Add the Database Manager to the tool list. Build its view frame, its store,
and everything about the open session:

- the header with Open, Recent, Config, Reload and Close;
- the empty state;
- drag-and-drop;
- the session banners and the config-problems panel;
- the complete error copy.

## Context

**The product.** FileManager is a portable Windows desktop app: a Tauri v2
shell, with React + TypeScript + Vite in `apps/desktop/src/`. The **Database
Manager** (`dbmanager`) opens a SQLite file and, optionally, a TOML config.
The config marks tables, fields and many-to-many relations as editable,
read-only or hidden, and sets field bounds.

- **Anything the config does not mention is editable.**
- A config with a syntax error, an unsupported version, or a file that
  cannot be read makes **everything read-only** until it is fixed.
- Config problems ("diagnostics") never block browsing.

The tool must feel like an **object editor, not a grid**. This task builds the
frame and the session around it. Later tasks build the panes (U3), the sheet
(U4) and the editing (U5 onwards).

You own `apps/desktop/src/` **except `lib/`**: never edit `lib/` or
`lib/generated/`. Tools never import each other: import only from `app/`,
`lib/`, and `tools/dbmanager/`. Where another tool has a helper you need
(`fileName`, `cleanPath`), write the tool's own copy.

**Contract rules.** The UI renders the backend's snapshot and keeps no second
copy of the truth. Display strings for paths may contain U+FFFD; never parse
them beyond taking the last path component for display.

### The client (`apps/desktop/src/lib/dbmanager.ts`, from M10)

Every function returns a promise that rejects with a `DbmanagerError`, or
with something else on an IPC failure.

| Function | Command | Returns |
| --- | --- | --- |
| `session()` | `dbmanager_session` | `DbmanagerSession`; the first call restores the most recent pair |
| `open(database: string, config: string \| null)` | `dbmanager_open` | `DbmanagerSession` |
| `setConfig(config: string \| null)` | `dbmanager_set_config` | `DbmanagerSession` (reopens with the current database) |
| `openDropped(paths: string[])` | `dbmanager_open_dropped` | `DbmanagerSession`; `.toml` = config, any other file = database; more than one of either → `DropAmbiguous` |
| `reload()` | `dbmanager_reload` | `DbmanagerSession` |
| `close()` | `dbmanager_close` | `DbmanagerSession` |
| `isDbmanagerError(e)` | — | type guard |

**If M10 landed different function names, use the landed ones for the same
commands, and say so in your report.** The command names are the contract.

`lib/tauri.ts` provides:

- `openFileDialog({ filters, defaultPath })`, resolving to a path or `null`;
- `DialogFilter`;
- `onDragDrop`, which `app/DropZone.tsx` already wraps.

### Types (generated, `lib/generated/`)

```ts
type DbmanagerSession = {
  database: DbFileSlot | null; config: DbFileSlot | null;
  open: boolean;
  readOnlyReason: string | null;        // the database file or folder is read-only
  configChanged: boolean;               // the config file changed on disk since loading
  configFailClosed: string | null;      // REQUESTED in M10: reason everything is read-only
                                        // (syntax error, unsupported version, unreadable)
  diagnostics: ConfigDiagnostic[]; errorCount: number; warningCount: number;
  tables: TableView[];                  // listed tables, schema order
  backupPath: string | null;            // set after the session's first write
  recent: RecentPairView[];             // most recent first, at most 10
  stateWarning: string | null;          // e.g. the most recent pair could not be reopened
};
type DbFileSlot = { path: string; error: DbmanagerError | null };
type RecentPairView = { database: string; config: string | null };
type ConfigDiagnostic = { severity: "error" | "warning"; message: string;
  line: number | null; col: number | null; table: string | null; column: string | null };
type DbmanagerError = { kind: DbmanagerErrorKind; message: string;
  fieldErrors?: FieldError[]; linkErrors?: LinkError[]; current?: ObjectView;
  conflictLink?: LinkRef; impact?: DeleteImpact };
type DbmanagerErrorKind = "NoDatabase" | "NotAbsolute" | "NotAFile" | "DatabaseUnreadable"
  | "ConfigUnreadable" | "DropAmbiguous" | "UnknownTable" | "UnknownObject" | "UnknownRelation"
  | "NotEditable" | "ValidationFailed" | "Conflict" | "SchemaChanged" | "ConfigChangedOnDisk"
  | "DatabaseReadOnly" | "Busy" | "BackupFailed" | "ImpactChanged" | "DeleteBlocked"
  | "ConstraintFailed" | "Io";
```

`TableView` is used here only for `tables.length`; U3 renders it.

**`configFailClosed` is a requested M10 addition.** If
`lib/generated/DbmanagerSession.ts` has no such field when you start, stop and
report it: the everything-read-only banner must not be inferred from
diagnostic text.

### The design (brief excerpt)

**Visual system** (`DESIGN.md`, `styles/tokens.css`; no new tokens):

- quiet and dense; neutrals plus one accent for the single primary action,
  the current place and focus;
- `--warn` and `--danger` appear only on status, always with an icon and a
  word;
- 1 px borders, 6 px radius, rem type; no shadows at rest;
- light and dark both first-class, and no hex outside `tokens.css`.

Tokens used here: `--surface`, `--surface-sunken`, `--border`, `--text`,
`--text-muted`, `--accent`, `--warn`, `--danger`, the spacing scale, the type
sizes and `--font-mono`.

**Shared components (from U1 and earlier, in `app/`):**

- `Button` (`primary | secondary | quiet | danger`, `icon`, `busy`);
- `Banner` (`tone: info | warn | error`, `message`, `action?: { label,
  onAction }`, `onDismiss?`, `children`; only `error` is `role="alert"`);
- `ConfirmDialog`;
- `DropZone` (`enabled`, `disabledReason`, `label`, `onDrop`);
- `MenuButton` (`label`, `menuLabel`, `items: { id, label, description?,
  title?, disabled?, onSelect }[]`, `disabled?`);
- `ShortcutsHelp` (`shortcuts: { keys: string[][]; action; when }[]`);
- `Icon` (incl. `lock`, `alert-triangle`, `x-circle`, `info`);
- the `.visually-hidden`, `.mono` and `.num` utilities.

**View layout, top to bottom.**

1. **Session header** (`SessionHeader`).
   - `h1`: the database file name (the last component of `database.path`), or
     "Database Manager" when nothing is open.
   - Under it:
     - the full database path, in `.mono`, `--font-size-sm`, `--text-muted`,
       wrapping with `overflow-wrap: anywhere`;
     - the **config line**: "Config: library.toml" (full path in `title`),
       or "No config: everything is editable unless the database prevents
       it", or, when `configFailClosed`, "Config: library.toml (has an error,
       so everything is read-only)";
     - a **Read-only** tag (lock icon + word, `title` = `readOnlyReason`)
       when `readOnlyReason` is set;
     - the **backup line** when `backupPath` is set: "Backup:" + file name,
       full path in `title` and in visually hidden text.
   - Actions on the right, wrapping onto a second line when narrow:
     - **Open database…**: Ctrl+O, `aria-keyshortcuts="Control+O"`, `title`
       "Shortcut: Ctrl+O";
     - **Recent**: a `MenuButton`, `menuLabel` "Recent databases";
     - **Config**: a `MenuButton` with "Choose config…" (Ctrl+Shift+O) and
       "Open without config" (only when a config is set). It is disabled when
       no database is open;
     - **Reload**: F5 or Ctrl+R;
     - **Close database**;
     - **Keyboard shortcuts**.

     Reload and Close appear only when a database is open.
2. **Banners** (`SessionBanners`), in this fixed order. Each tone is said in
   words by `Banner`.

   | Banner | Tone | When | Text | Action | Dismissible |
   | --- | --- | --- | --- | --- | --- |
   | Command error | error | the last session command failed | `errorMessage(error, "read")` + Details | — | yes |
   | Database problem | error | `!open && database?.error` | "{file name}: " + `errorMessage(database.error, "read")` + Details | — | no |
   | Everything read-only | error | `configFailClosed` | "Everything is read-only: {configFailClosed}. Fix the config and reload, or open without a config." + (config slot error in Details) | **Reload**, plus a quiet **Open without config** in the body | no |
   | Config changed | warn | `configChanged` | "The config file changed on disk. Reload to apply it; saving is paused until you do." | **Reload** | no |
   | Schema changed | warn | the store's `schemaChanged` flag (set by later tasks when any read or write fails with `SchemaChanged`) | "The database's structure changed. Reload to continue." | **Reload** | no |
   | Read-only database | info | `readOnlyReason` | "This database is open read-only: {reason}. You can browse it but not change it." | — | yes (the header tag stays) |
   | State warning | warn | `stateWarning` | the text as given | — | yes |
   | Notices | info | the store's `notices` list (later tasks add backup and delete notices) | each notice's text | — | yes |

   Only the command-error banner is a live region (`Banner` does that). Every
   other change is announced by the view's polite announcer.
3. **Config problems** (`ConfigProblems`), when `diagnostics.length > 0`:
   - a `section` labelled by its summary: "Config: 2 errors, 1 warning.
     Entries with errors are read-only." (or "Config: 1 warning."), with the
     `x-circle` icon in `--danger` when there are errors, else
     `alert-triangle` in `--warn`;
   - a **Show problems** / **Hide problems** button (`aria-expanded`,
     `aria-controls`), collapsed by default; **F8** expands the list and
     focuses it;
   - the list (`ul`, `aria-label="Config problems"`). Each item has:
     - the severity icon and word ("Error" / "Warning");
     - the position: "Line 12, column 3" when `line` is set, else "Table
       book, field isbn" or "Table book" from `table`/`column`, with
       `.num` digits;
     - the message, wrapped, never truncated.
4. **Workspace**: the region U3 fills. In this task it holds only a
   placeholder `<div className="dbmanager__workspace">` with "{n} tables"
   when open.
5. **Empty state** (`EmptyState`), when nothing is open:
   - `h2` "Open a SQLite database";
   - "Choose a database file, and optionally a TOML config that sets what can
     be edited. Without a config, everything is editable.";
   - **Open database…** (primary);
   - when `recent` is non-empty: `h3` "Recent" and a list of buttons, one per
     pair. Each button shows the database file name, then a second line in
     `--text-muted` ("with library.toml" or "no config"), and has the full
     paths in `title`;
   - a muted hint: "You can also drop a database file, a .toml config, or both
     on the window."

**Behaviour.**

- **Loading.** On mount, call `session()`.
  - While it runs: the view's `aria-busy`, and a visually hidden "Loading".
  - If it fails: the command-error banner and a **Try again** button that
    repeats it.
- **Pending commands.** While a session command (open, set config, drop,
  reload, close) runs:
  - the header actions are disabled;
  - the header shows "Opening library.db…" / "Reloading…" / "Closing…" in
    `--text-muted`, which is also announced;
  - the view has `aria-busy`;
  - the drop zone is disabled with "Wait for the current action to finish".
- **Latest wins.** Only the newest session command's response is applied,
  with a counter as in `tools/schedule/ScheduleView.tsx`.
- **Open database…** calls `openFileDialog` with filters `SQLite database`
  (`db`, `sqlite`, `sqlite3`, `db3`) and `All files` (`*`). If `recent`
  holds a pair for exactly that database path, it opens with that pair's
  config, else with none.
  - After opening, the view announces "Opened library.db." (or "Opened
    library.db with library.toml.").
  - If the database slot has an error, it announces the error copy instead.
- **Recent** (menu or empty-state list): `open(pair.database, pair.config)`.
- **Choose config…** calls `openFileDialog` with filters `TOML config`
  (`toml`) and `All files`, then `setConfig(path)`. **Open without config**
  calls `setConfig(null)`.
- **Reload** calls `reload()`. **Close database** calls `close()` and returns
  to the empty state.
- **Drop:** `openDropped(paths)`, label "Drop a SQLite database, a .toml
  config, or both". `DropAmbiguous` shows the command-error banner.
- **Refresh on return.** When the window regains focus (`window` `focus`, or
  `visibilitychange` to visible), a database is open, and no command is
  pending, call `session()` quietly, at most once per 500 ms. Apply the
  result; ignore its failures (the next real command reports them). This
  surfaces "config changed" after the user edits the config elsewhere.
- **Announcements.** Announce through one always-mounted polite region
  (`role="status"`), with the same clear-then-set pattern as the Schedule
  view: opening, reload results, banners appearing (other than errors), and
  "The config has 2 errors and 1 warning." once per new session.
- **Shortcuts** (a `useDbmanagerShortcuts` hook on the view root):
  - Ctrl+O, Ctrl+Shift+O, F5, Ctrl+R and F8;
  - F5 and Ctrl+R always have their default prevented (the webview must not
    reload), even when nothing is open;
  - shortcuts are ignored while a modal dialog is open (`document.querySelector
    ("dialog[open]")`).

  `shortcuts.ts` exports `SHORTCUTS` with these rows for `ShortcutsHelp`:

  | Keys | Action | Active when |
  | --- | --- | --- |
  | Ctrl+O | Open database… | Always |
  | Ctrl+Shift+O | Choose config… | A database is open |
  | F5 or Ctrl+R | Reload | A database is open |
  | F8 | Show config problems | The config has problems |

  Later tasks append rows.

### The store (`tools/dbmanager/store.ts`)

The view's state lives in a module-level store, so switching to another tool
and back keeps it:

- it is a plain object with `getState`, `subscribe` and actions;
- components read it with `useSyncExternalStore` through `useDbmanager()`;
- it exports `resetDbmanagerStore()` for tests.

In this task it holds:

- `session: DbmanagerSession | null`;
- `loadState: "loading" | "failed" | "ready"`;
- `pending: null | "open" | "config" | "drop" | "reload" | "close"`;
- `commandError: ViewError | null`, plus a failure counter (used as the
  banner `key`);
- `schemaChanged: boolean`;
- `notices: { id: string; tone: "info"; message: string; detail?: string }[]`;
- `dismissed` banner ids for this session;
- `diagnosticsExpanded: boolean`.

Its actions wrap the client functions above and apply latest-wins. Later
tasks add places, history, list state and drafts to the same store. Keep the
actions small and the state serialisable (no DOM nodes).

### Error copy (`tools/dbmanager/errorMessages.ts`)

- `errorMessage(error: ViewError, context: "read" | "write"): string` holds
  one entry per kind in a `Record<DbmanagerErrorKind, …>`, so a missing or
  extra kind fails the typecheck.
- `toDbmanagerError(e: unknown): ViewError` accepts a value only when `kind`
  is a known kind and `message` is a string. Otherwise it returns `{ kind:
  "Unexpected", message }`.

These are the same patterns as `tools/schedule/errorMessages.ts`. Copy them;
do not import them. The copy:

| Kind | Copy |
| --- | --- |
| `NoDatabase` | Open a database first. |
| `NotAbsolute` | Enter the full path, starting with the drive letter, for example C:\\. |
| `NotAFile` | No file was found at this path. |
| `DatabaseUnreadable` | This file could not be opened as a SQLite database. Check that it is a SQLite file and that you can read it. |
| `ConfigUnreadable` | The config file could not be read, so everything is read-only. Check that it exists and is saved as UTF-8 text, then reload. |
| `DropAmbiguous` | Drop one database file, one .toml config, or one of each. Nothing was changed. |
| `UnknownTable` | This table is no longer available. Reload to see the current tables. |
| `UnknownObject` | This object no longer exists. Another program may have deleted it. |
| `UnknownRelation` | This relation is no longer available. Reload to continue. |
| `NotEditable` | This is read-only, so it can't be changed. Nothing was changed. |
| `ValidationFailed` | Nothing was saved. Fix the problems below and save again. |
| `Conflict` | This object was changed by another program after you opened it. Nothing was saved. |
| `SchemaChanged` | The database's structure changed. Reload to continue. Nothing was changed. |
| `ConfigChangedOnDisk` | The config file changed. Reload it before saving. Nothing was changed. |
| `DatabaseReadOnly` | The database is open read-only, so it can't be changed. Nothing was changed. |
| `Busy` | read: "The database is busy: another program is using it. Try again in a moment." write: "The database is busy: another program is using it. Nothing was changed. Try again in a moment." |
| `BackupFailed` | The backup copy could not be written, so nothing was changed. Check that the database's folder is writable and has free space. |
| `ImpactChanged` | What this delete affects changed. Nothing was deleted. Review it again. |
| `DeleteBlocked` | Other objects still reference this one, and the database does not allow deleting it while they do. Nothing was deleted. |
| `ConstraintFailed` | The database refused the change: one of its own rules (a constraint or trigger) failed. Nothing was changed. See Details. |
| `Io` | read: "The database could not be read. See Details." write: "The change could not be written. Nothing was changed. See Details." |
| Unexpected | Something went wrong. See Details. |

Later tasks may sharpen a message with context (for example, a label in the
`Conflict` copy) by wrapping `errorMessage`, never by editing the table's
meaning. Error banners carry a **Details** disclosure with `error.message` in
`.mono`, exactly as the Schedule view does.

### CSS

- Create `styles/dbmanager.css`, every selector scoped under `.dbmanager`,
  and import it in `App.tsx` after `pipeline.css`.
- Create `styles/dbmanager.css.test.ts` asserting the scoping and that no
  rule has a `box-shadow`. (U7 adds the picker popover as the one allowed
  shadow, and updates the test.)
- The view is a flex column filling the main area: `.tool-view.dbmanager`,
  with `display: flex; flex-direction: column; gap: var(--space-3)`.
- The header wraps: the title block is `flex: 1 1 20rem`, and the actions
  wrap to a second line.
- Paths and messages wrap; nothing scrolls horizontally at 800×560 or at
  200% text.

## Files

Create in `apps/desktop/src/tools/dbmanager/`:

- `DbmanagerView.tsx`, `store.ts`, `errorMessages.ts`, `paths.ts`
  (`fileName`);
- `SessionHeader.tsx`, `SessionBanners.tsx`, `ConfigProblems.tsx`,
  `EmptyState.tsx`;
- `shortcuts.ts`, `useDbmanagerShortcuts.ts`;
- `fixtures.ts`: sessions built from the example schema below;
- tests: `errorMessages.test.ts`, `store.test.ts`,
  `DbmanagerView.session.test.tsx`, `ConfigProblems.test.tsx`,
  `useDbmanagerShortcuts.test.tsx`.

Also create `apps/desktop/src/styles/dbmanager.css` and
`styles/dbmanager.css.test.ts`.

Edit:

- `apps/desktop/src/tools/registry.ts`: append `{ id: "dbmanager", label:
  "Database Manager", view: DbmanagerView }`;
- `apps/desktop/src/App.tsx`: import the CSS.

Do not touch `lib/`, `app/`, the other tools, or `tokens.css`.

**Fixture data** follows the example database. Tables, by display name:

- Authors, Books, Genres, Members, Loans: editable;
- Publishers: read-only.

Example paths are `C:\Data\library.db` and `C:\Data\library.toml`.

## Skill

- Run `/impeccable shape`'s output (this brief) through `/impeccable layout`
  on the header and empty state once they render: run it from `apps/desktop`
  with `--target src/tools/dbmanager/DbmanagerView.tsx`.
- Read `reference/craft-floor.md` before the first UI edit.
- **Extend the established world.** Use the tarpack and schedule headers as
  incumbent references for rhythm and density.
- Before declaring done, run `/impeccable audit` on `tools/dbmanager/` and
  `styles/dbmanager.css`, then the detector once on the same paths.
- If the skill is missing, install it with `npx impeccable install`, or follow
  the command references from `github.com/pbakaus/impeccable` by hand, and say
  so.

## Acceptance criteria

**Navigation entry:**

- "Database Manager" appears in the tool navigation, after Schedule Creator.
- Selecting it renders the view. Switching tools and back keeps the session
  state, with no second `session()` load call.

**Session states:**

- **Nothing open** (`open: false`, no slots): the empty state. Two recent
  pairs render as buttons showing the names and the config lines. Activating
  one calls `open(database, config)`.
- **Open:**
  - `h1` is "library.db", and the path is shown in full;
  - the config line reads "Config: library.toml";
  - Reload and Close database are present;
  - the workspace placeholder says "6 tables".
- **No config:** "No config: everything is editable unless the database
  prevents it".
- **`readOnlyReason`:** the Read-only header tag (with `title`) and the info
  banner. Dismissing the banner keeps the tag.
- **`configFailClosed` = "line 3, column 7: expected `=`":**
  - the error banner "Everything is read-only: line 3, column 7: expected
    `=`. …", with **Reload** and **Open without config**;
  - it is not dismissible;
  - the config line marks the error.
- **`configChanged`:** the warn banner with **Reload**, which calls
  `reload()`.
- **Database slot error** (`DatabaseUnreadable`, not open): the error banner
  with the file name, the copy and Details.
- **`stateWarning`:** a dismissible warn banner.
- **Diagnostics** (1 error with line/col, 1 warning with table/column):
  - the summary "Config: 1 error, 1 warning. Entries with errors are
    read-only.";
  - Show problems expands the list; F8 expands it and focuses the list;
  - the items read "Error", "Line 3, column 7", and "Warning", "Table book,
    field isbn".

**Commands:**

- Open database… passes the filters, and reuses a recent pair's config for
  the same database path.
- Choose config… calls `setConfig(path)`, and Open without config calls
  `setConfig(null)`.
- Drop calls `openDropped(paths)`; `DropAmbiguous` shows its copy.
- While a command is pending, the actions are disabled, `aria-busy` is set,
  the status text shows, and the drop zone is disabled with its reason.
- Responses arrive out of order: only the newest is applied.
- A window `focus` with a database open calls `session()` and shows a
  `configChanged` banner that appears.

**Errors:**

- `errorMessage` has an entry for every one of the 21 kinds; the typecheck
  enforces it.
- `Busy` and `Io` differ by context.
- An IPC rejection that is not a `DbmanagerError` shows "Something went
  wrong. See Details."

**Keyboard and accessibility:**

- Ctrl+O, Ctrl+Shift+O, F5, Ctrl+R and F8 work from anywhere in the view.
- F5 and Ctrl+R are prevented even with nothing open.
- No shortcut fires while a modal dialog is open.
- Every control is reachable by Tab, in visual order, and labelled.
- The menus follow U1's `MenuButton` keyboard behaviour.
- The Keyboard shortcuts popover lists the four rows.
- jsx-a11y lint passes.
- At 800×560 and at 200% text, the header wraps with no horizontal
  scrolling. Paths and banner text wrap.
- Light and dark both render from tokens; `dbmanager.css` has no hex.

## Tests proving completion

- `errorMessages.test.ts`:
  - `every kind has copy`;
  - `busy and io differ by context`;
  - `unexpected errors claim nothing`;
  - `toDbmanagerError rejects unknown shapes`.
- `store.test.ts`:
  - `latest response wins`;
  - `pending blocks and clears`;
  - `reset restores the initial state`.
- `DbmanagerView.session.test.tsx` (mock `lib/dbmanager` and `lib/tauri` with
  `vi.mock`):
  - `empty state with recent pairs`;
  - `open via dialog reuses recent config`;
  - `open shows header and config line`;
  - `no config line`;
  - `read-only database tag and banner`;
  - `config fail-closed banner`;
  - `config changed banner reloads`;
  - `database slot error`;
  - `state warning`;
  - `drop calls openDropped`;
  - `drop ambiguous`;
  - `pending disables actions`;
  - `focus refresh shows config changed`;
  - `restore failure offers try again`;
  - `state survives remount`.
- `ConfigProblems.test.tsx`:
  - `summary counts`;
  - `expand and F8 focus`;
  - `positions by line or table`.
- `useDbmanagerShortcuts.test.tsx`:
  - `ctrl+O opens`;
  - `F5 and ctrl+R reload and are always prevented`;
  - `ignored while a dialog is open`.
- `dbmanager.css.test.ts`: `selectors scoped`, `no shadows`.

Run: `npm run typecheck && npm run lint && npm run test`.

## States covered

- **Loading:** initial session load, pending commands.
- **Empty:** nothing open, with and without recent pairs.
- **Error:** restore failure, a command error, a database slot error, config
  failed closed, `stateWarning`.
- **Success:** open, with or without config, read-only database, config
  changed, diagnostics.

## Out of scope

- The table list, object list, panes and history (U3); the object sheet (U4
  onwards).
- Asking before leaving unsaved edits. U6 wraps Open, Recent, drop and Close
  with the leave guard, so keep each of those actions as one store action that
  U6 can wrap.
- The backup and delete notices' content (U6, U11). This task only renders
  the `notices` list.
