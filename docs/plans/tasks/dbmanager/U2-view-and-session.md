# U2 — Database Manager view scaffold and session

Status: **superseded (2026-11-21):** the UI moves to GPUI; the ui-designer rewrites this task for `apps/filemanager` (its design decisions carry over). Do not run this plan. Previous status: awaiting approval (revised 2026-11-16: the fixed app-wide config,
recent databases, automatic config reload, developer-mode Edit config; then
`ConfigNotDroppable` and the backup-path rules)
Project: dbmanager   Depends on: U1 (landed), M10 (landed), M13 (landed)

## Goal

Add the Database Manager to the tool list. Build its view frame, its store,
and everything about the session:

- the header, with the database and the config status;
- Open, Recent databases, Reload and Close;
- developer-mode Edit config;
- the empty state and drag-and-drop;
- the session banners, including no config, everything read-only, and config
  reloaded;
- the config-problems panel;
- the complete error copy.

## Context

**The product.** FileManager is a portable Windows desktop app: a Tauri v2
shell, with React + TypeScript + Vite in `apps/desktop/src/`. The **Database
Manager** (`dbmanager`) opens a SQLite file the user picks.

**One app-wide TOML config** decides what can be edited:

- it lives at `%ProgramData%\FileManager\dbmanager.toml`;
- the user never chooses it; the app only reads it, and **reloads it
  automatically** when the file changes;
- it marks tables (editable, read-only, hidden, pickable), fields and
  many-to-many relations, and sets field bounds.

What the config means for editing:

- **Anything the config does not mention is editable.**
- **With no config file**, everything is editable. The UI shows a calm,
  clear banner, with **no prompt**.
- **A config that cannot be used** (a syntax error, an unsupported version,
  an unreadable file, or a location that cannot be determined) makes
  **everything read-only** until it is fixed.
- Config problems ("diagnostics") never block browsing.

The tool must feel like an **object editor, not a grid**. This task builds the
frame and the session. Later tasks build the panes (U3), the sheet (U4) and
the editing (U5 onwards).

You own `apps/desktop/src/` **except `lib/`**: never edit `lib/` or
`lib/generated/`. Tools never import each other: import only from `app/`,
`lib/`, and `tools/dbmanager/`. Where another tool has a helper you need
(`fileName`), write the tool's own copy.

**Contract rules.**

- The UI renders the backend's snapshot and keeps no second copy of the
  truth.
- The config hash is opaque: the store keeps the session, and later tasks
  echo `session.config.hash` back on writes.
- Display strings for paths may contain U+FFFD. Never parse them beyond
  taking the last path component (or the folder) for display.

### The client (`apps/desktop/src/lib/dbmanager.ts`, M10 and M13; names pinned)

Every function returns a promise that rejects with a `DbmanagerError`, or
with something else on an IPC failure.

| Function | Command | Returns |
| --- | --- | --- |
| `session()` | `dbmanager_session` | `DbmanagerSession`. The first call loads the config and reopens the most recent database. Later calls are cheap and side-effect free, recompute `configChanged`, and are safe on window focus. |
| `open(database: string)` | `dbmanager_open` | `DbmanagerSession` |
| `openDropped(paths: string[])` | `dbmanager_open_dropped` | `DbmanagerSession`. Exactly one file; more than one gives `DropAmbiguous`. A single `.toml` is refused with `ConfigNotDroppable` before any open: nothing changes, and the current database stays open. |
| `reload()` | `dbmanager_reload` | `DbmanagerSession`. Reloads the config and, if a database is open, re-introspects it. Works with no database open. **Keeps `backupPath`**: no second backup is taken after a reload. |
| `close()` | `dbmanager_close` | `DbmanagerSession` (the config stays loaded). `close()` and `open()` start a fresh session: `backupPath` is null until that session's first write. |
| `editConfig()` | `dbmanager_edit_config` | `EditConfigOutcome { created: boolean; session: DbmanagerSession }`. Developer mode only, as the view gates it. Creates a commented, inert starter file if none exists, then opens the file in the system editor. Fails with `OpenerFailed` when the editor cannot start, or `Io` when the folder cannot be created. |
| `onConfigChanged(handler)` | event `dbmanager://config-changed` (`CONFIG_CHANGED_EVENT`) | `Promise<UnlistenFn>`. The payload is `DbmanagerConfigChanged { hash, status }`, emitted only **after** the reload completes, so `session()` called from the handler returns the new hash, diagnostics and tables. |
| `isDbmanagerError(e)` | — | type guard |

`lib/tauri.ts` provides:

- `openFileDialog({ filters, defaultPath })`, resolving to a path or `null`;
- `DialogFilter`;
- `onDragDrop`, which `app/DropZone.tsx` already wraps.

`app/devMode.tsx` provides `useDevMode(): boolean`. That is the app-wide
developer mode, toggled with Ctrl+Alt+Shift+D, and the Tar Packager gates its
"Edit in editor" the same way.

### Types (generated, `lib/generated/`)

```ts
type DbmanagerSession = {
  database: DbFileSlot | null;
  config: DbConfigView;                 // always present, even with no database open
  open: boolean;
  readOnlyReason: string | null;        // the database file or folder is read-only
  configChanged: boolean;               // the file differs from what is loaded: normally only for a moment
                                        // before the automatic reload; a fallback signal
  configFailClosed: string | null;      // reason everything is read-only (syntax error, unsupported version,
                                        // unreadable, location unknown); null otherwise, including "missing"
  diagnostics: ConfigDiagnostic[];      // with a database: config + schema; without: config-only
  errorCount: number; warningCount: number;
  tables: TableView[];                  // includes pickable tables (U3 filters them out of the list)
  backupPath: string | null;            // set after the session's first write
  recent: string[];                     // recent DATABASE display paths, most recent first, at most 10
  stateWarning: string | null;          // e.g. the last database could not be reopened, and why
};
type DbFileSlot = { path: string; error: DbmanagerError | null };
type DbConfigView = { path: string | null;           // null: the location could not be determined
                      status: "loaded" | "missing" | "failClosed";
                      hash: string };                // opaque; echoed back on writes by later tasks
type DbmanagerConfigChanged = { hash: string; status: "loaded" | "missing" | "failClosed" };
type EditConfigOutcome = { created: boolean; session: DbmanagerSession };
type ConfigDiagnostic = { severity: "error" | "warning"; message: string;
  line: number | null; col: number | null; table: string | null; column: string | null };
type DbmanagerError = { kind: DbmanagerErrorKind; message: string;
  fieldErrors?: FieldError[]; linkErrors?: LinkError[]; current?: ObjectView;
  conflictLink?: LinkRef; impact?: DeleteImpact };
type DbmanagerErrorKind = "NoDatabase" | "NotAbsolute" | "NotAFile" | "DatabaseUnreadable"
  | "OpenerFailed" | "ConfigNotDroppable" | "DropAmbiguous" | "UnknownTable" | "UnknownObject" | "UnknownRelation"
  | "NotEditable" | "ValidationFailed" | "Conflict" | "SchemaChanged" | "ConfigChangedOnDisk"
  | "DatabaseReadOnly" | "Busy" | "BackupFailed" | "ImpactChanged" | "DeleteBlocked"
  | "ConstraintFailed" | "Io";
```

If a generated name or shape differs from this excerpt, the generated file
wins: use it and say so in your report. `TableView` is used here only to
count tables; U3 renders it.

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
- `ShortcutsHelp` (`shortcuts: { keys: string[][]; action; when; devOnly?
  }[]`; it lists `devOnly` rows only in developer mode);
- `Icon` (incl. `lock`, `alert-triangle`, `x-circle`, `info`);
- the `.visually-hidden`, `.mono` and `.num` utilities.

**View layout, top to bottom.**

1. **Session header** (`SessionHeader`).
   - `h1`: the database file name (the last component of `database.path`), or
     "Database Manager" when nothing is open.
   - Under it:
     - the full database path, in `.mono`, `--font-size-sm`, `--text-muted`,
       wrapping with `overflow-wrap: anywhere` (when a database is set);
     - the **config status line**, always shown. It is a status, not a
       control:

       | `config.status` | Line |
       | --- | --- |
       | `loaded` | "Config: " + `config.path` in `.mono` |
       | `missing` | "No config file: everything is editable unless the database prevents it." |
       | `failClosed` | "Config: {path} has a problem, so everything is read-only." When `config.path` is null: "Config location unknown, so everything is read-only." |

     - a **Read-only** tag (lock icon + word, `title` = `readOnlyReason`)
       when `readOnlyReason` is set;
     - the **backup line** when `backupPath` is set (it follows the snapshot:
       it stays across Reload, and Close or Open clears it): "Backup:" + file
       name,
       full path in `title` and in visually hidden text.
   - Actions on the right, wrapping onto a second line when narrow:
     - **Open database…**: Ctrl+O, `aria-keyshortcuts="Control+O"`, `title`
       "Shortcut: Ctrl+O";
     - **Recent**: a `MenuButton` with `menuLabel` "Recent databases". Each
       item's `label` is the file name, its `description` the folder, and its
       `title` the full path;
     - **Reload**: F5 or Ctrl+R. It reloads the config and the database.
       Shown when a database is open;
     - **Close database**: shown when a database is open;
     - **Edit config**: shown **only when `useDevMode()` is true**. Ctrl+E,
       `aria-keyshortcuts="Control+E"`, `title` "Shortcut: Ctrl+E". It is
       available with or without a database;
     - **Keyboard shortcuts**, last.
2. **Banners** (`SessionBanners`), in this fixed order. `Banner` states each
   tone in words.

   | Banner | Tone | When | Text | Action | Dismissible |
   | --- | --- | --- | --- | --- | --- |
   | Command error | error | the last session command failed | `errorMessage(error, "read")` + Details | — | yes |
   | Database problem | error | `!open && database?.error` | "{file name}: " + `errorMessage(database.error, "read")` + Details | — | no |
   | Everything read-only | error | `configFailClosed` | "Everything is read-only: {configFailClosed}. Fix the config file ({config.path, or "its location could not be determined"}); it reloads by itself." | **Edit config** in developer mode only | no |
   | No config file | info | `config.status === "missing"` | "No config file was found, so everything is editable unless the database prevents it. An administrator can add one at {config.path}." | — | yes, for the rest of the app run (keep it in the store) |
   | Config changed on disk | warn | `configChanged` (the fallback: the automatic reload missed it) | "The config file changed on disk. Reload to apply it." | **Reload** | no |
   | Schema changed | warn | the store's `schemaChanged` flag (set by later tasks when any read or write fails with `SchemaChanged`) | "The database's structure changed. Reload to continue." | **Reload** | no |
   | Read-only database | info | `readOnlyReason` | "This database is open read-only: {reason}. You can browse it but not change it." | — | yes (the header tag stays) |
   | State warning | warn | `stateWarning` | the text as given | — | yes |
   | Notices | info | the store's `notices` list (this task adds the config-reloaded and starter-created notices; later tasks add backup and delete notices) | each notice's text | — | yes |

   Only the command-error banner is a live region (`Banner` does that).
   Every other change is announced by the view's polite announcer.
3. **Config problems** (`ConfigProblems`), when `diagnostics.length > 0`,
   **whether or not a database is open**:
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
       book, field isbn" or "Table book" from `table`/`column`, with `.num`
       digits;
     - the message, wrapped, never truncated.
4. **Workspace**: the region U3 fills. In this task it holds only a
   placeholder `<div className="dbmanager__workspace">` with "{n} tables"
   when open.
5. **Empty state** (`EmptyState`), when nothing is open:
   - `h2` "Open a SQLite database";
   - "Choose a database file. What can be edited is set by the app's config
     file; without one, everything is editable.";
   - **Open database…** (primary);
   - when `recent` is non-empty: `h3` "Recent", and a list of buttons, one
     per database. Each button shows the file name, then the folder on a
     second line in `--text-muted`, with the full path in `title`;
   - a muted hint: "You can also drop a database file on the window.".

**Behaviour.**

- **Loading.** On mount, call `session()` if the store has no session yet.
  - While it runs: the view's `aria-busy`, and a visually hidden "Loading".
  - If it fails: the command-error banner and a **Try again** button that
    repeats it.
- **Pending commands.** While a session command (open, drop, reload, close,
  edit config) runs:
  - the header actions are disabled;
  - the header shows "Opening library.db…" / "Reloading…" / "Closing…" /
    "Opening the config…" in `--text-muted`, which is also announced;
  - the view has `aria-busy`;
  - the drop zone is disabled with "Wait for the current action to finish".
- **Latest wins.** Only the newest session command's response is applied,
  with a counter as in `tools/schedule/ScheduleView.tsx`. A config-reload
  re-read counts as a session command for this.
- **Open database…** calls `openFileDialog` with filters `SQLite database`
  (`db`, `sqlite`, `sqlite3`, `db3`) and `All files` (`*`), then
  `open(path)`.
  - After opening, the view announces "Opened library.db.".
  - If the database slot has an error, it announces the error copy instead.
- **Recent** (menu or empty-state list): `open(path)`.
- **Reload** calls `reload()`. **Close database** calls `close()` and returns
  to the empty state.
- **Drop:** `openDropped(paths)`, label "Drop a SQLite database file".
  `DropAmbiguous`, `ConfigNotDroppable` and `DatabaseUnreadable` show the
  command-error banner. On `ConfigNotDroppable`, keep the current session as
  it is: the database stays open, and the store's session is not replaced.
- **Backup notices follow the session.** When a newly applied session has a
  different `database.path`, or `backupPath` becomes null (after Close or
  Open), remove any backup notice still in `notices`. Reload keeps it.
- **Edit config** (developer mode):
  1. Call `editConfig()` and apply `outcome.session`.
  2. If `created`, push the notice "A starter config was created at {path}
     and opened in your editor. It changes nothing until you edit it.".
     Otherwise announce "Opened the config in your editor.".
  3. On failure, show the command-error banner (`OpenerFailed`, `Io`).
- **Automatic config reload.** The store subscribes **once per app run**,
  with `onConfigChanged`, from the store module the first time the session
  loads. It is not tied to the view's mount, so it keeps working while
  another tool is showing; never unsubscribe on unmount. On each event:
  1. call `session()` and apply it;
  2. push the notice "The config was reloaded." (U6 extends this with the
     draft re-check);
  3. when the new status is `failClosed`, skip the notice, because the
     everything-read-only banner says it.

  If subscribing fails (no Tauri event source, for example in tests), ignore
  it.
- **Refresh on return.** When the window regains focus (`window` `focus`, or
  `visibilitychange` to visible) and no command is pending, call `session()`
  quietly, at most once per 500 ms. Apply the result; ignore its failures (the
  next real command reports them).
- **Announcements.** Announce through one always-mounted polite region
  (`role="status"`), with the same clear-then-set pattern as the Schedule
  view: opening, reload results, notices and banners appearing (other than
  errors), and "The config has 2 errors and 1 warning." once per new config
  hash.
- **Shortcuts** (a `useDbmanagerShortcuts` hook on the view root):
  - Ctrl+O, F5, Ctrl+R, F8 and Ctrl+E (Ctrl+E only in developer mode;
    otherwise it is not handled);
  - F5 and Ctrl+R always have their default prevented (the webview must not
    reload), even when nothing is open;
  - shortcuts are ignored while a modal dialog is open (`document.querySelector
    ("dialog[open]")`).

  `shortcuts.ts` exports `SHORTCUTS` with these rows for `ShortcutsHelp`:

  | Keys | Action | Active when | devOnly |
  | --- | --- | --- | --- |
  | Ctrl+O | Open database… | Always | |
  | F5 or Ctrl+R | Reload the config and the database | A database is open | |
  | F8 | Show config problems | The config has problems | |
  | Ctrl+E | Edit config | Developer mode is on | yes |

  Later tasks append rows.

### The store (`tools/dbmanager/store.ts`)

The view's state lives in a module-level store, so switching to another tool
and back keeps it:

- it is a plain object with `getState`, `subscribe` and actions;
- components read it with `useSyncExternalStore` through `useDbmanager()`;
- it exports `resetDbmanagerStore()` for tests. The reset also clears the
  config-event subscription flag.

In this task it holds:

- `session: DbmanagerSession | null`;
- `loadState: "loading" | "failed" | "ready"`;
- `pending: null | "open" | "drop" | "reload" | "close" | "editConfig"`;
- `commandError: ViewError | null`, plus a failure counter (used as the
  banner `key`);
- `schemaChanged: boolean`;
- `notices: { id: string; tone: "info"; message: string; detail?: string }[]`;
- `dismissed` banner ids (the no-config dismissal lasts for the app run);
- `diagnosticsExpanded: boolean`.

Its actions wrap the client functions above and apply latest-wins. Later
tasks add places, history, list state and drafts to the same store. Keep the
actions small and the state serialisable (no DOM nodes). Keep
`onConfigChanged` handling in one named action, `configReloaded()`, which U6
extends.

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
| `OpenerFailed` | The config file could not be opened in an editor. Open it yourself: {path}. (`{path}` is the session's `config.path`; pass it in, or say "the config file" when null.) |
| `DropAmbiguous` | Drop one database file at a time. Nothing was changed. |
| `ConfigNotDroppable` | The config can't be chosen by dropping it: the app always uses {path}. Nothing was changed. (`{path}` is `session.config.path`; pass it in, or say "its fixed config file" when null.) |
| `UnknownTable` | This table is no longer available. Reload to see the current tables. |
| `UnknownObject` | This object no longer exists. Another program may have deleted it. |
| `UnknownRelation` | This relation is no longer available. Reload to continue. |
| `NotEditable` | This is read-only, so it can't be changed. Nothing was changed. |
| `ValidationFailed` | Nothing was saved. Fix the problems below and save again. |
| `Conflict` | This object was changed by another program after you opened it. Nothing was saved. |
| `SchemaChanged` | The database's structure changed. Reload to continue. Nothing was changed. |
| `ConfigChangedOnDisk` | The config changed while you were editing and was reloaded. Nothing was changed. Check your changes and save again. |
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
  (`fileName`, `folderName`);
- `SessionHeader.tsx`, `SessionBanners.tsx`, `ConfigProblems.tsx`,
  `EmptyState.tsx`;
- `shortcuts.ts`, `useDbmanagerShortcuts.ts`;
- `fixtures.ts`: sessions built from the example schema below;
- tests: `errorMessages.test.ts`, `store.test.ts`,
  `DbmanagerView.session.test.tsx`, `DbmanagerView.config.test.tsx`,
  `ConfigProblems.test.tsx`, `useDbmanagerShortcuts.test.tsx`.

Also create `apps/desktop/src/styles/dbmanager.css` and
`styles/dbmanager.css.test.ts`.

Edit:

- `apps/desktop/src/tools/registry.ts`: append `{ id: "dbmanager", label:
  "Database Manager", view: DbmanagerView }`;
- `apps/desktop/src/App.tsx`: import the CSS.

Do not touch `lib/`, `app/`, the other tools, or `tokens.css`.

**Fixture data** follows the example database. Tables, by display name:

- Authors, Books, Genres, Members, Loans: editable;
- Publishers: `mode: "pickable"`.

Example paths:

- the database: `C:\Data\library.db` (recent: it and
  `D:\Archive\old.db`);
- the config: `C:\ProgramData\FileManager\dbmanager.toml`.

## Skill

- Run `/impeccable layout` on the header and empty state once they render:
  run it from `apps/desktop` with `--target
  src/tools/dbmanager/DbmanagerView.tsx`.
- Then run `/impeccable clarify` on the config status line and the banner
  copy. Keep the meaning, and report proposed changes.
- Read `reference/craft-floor.md` before the first UI edit.
- **Extend the established world.** Use the tarpack and schedule headers as
  incumbent references for rhythm and density, and tarpack's developer-mode
  "Edit in editor" for gating.
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

- **Nothing open, two recent databases:**
  - the empty state lists them by file name and folder;
  - activating one calls `open(path)`;
  - the config status line shows.
- **Open, config `loaded`:**
  - `h1` is "library.db", and the path is shown in full;
  - the line reads "Config: C:\ProgramData\FileManager\dbmanager.toml";
  - Reload and Close database are present;
  - the placeholder says "6 tables".
- **Config `missing`:**
  - the line reads "No config file: …";
  - the info banner reads "No config file was found, … at {path}."; it is
    dismissible and stays dismissed across a remount;
  - nothing prompts.
- **`configFailClosed` = "line 3, column 7: expected `=`":**
  - the error banner "Everything is read-only: line 3, column 7: expected
    `=`. Fix the config file (…); it reloads by itself.";
  - it is not dismissible;
  - Edit config appears in it only in developer mode;
  - the status line marks the problem.
- **`configFailClosed` with `config.path: null`:** the line and the banner
  say the location could not be determined.
- **`configChanged`** (the fallback): the warn banner with **Reload**, which
  calls `reload()`.
- **Database slot error** (`DatabaseUnreadable`, not open): the error banner
  with the file name, the copy and Details.
- **`stateWarning`:** a dismissible warn banner.
- **Diagnostics** (1 error with line/col, 1 warning with table/column), with
  a database open **and** without:
  - the summary "Config: 1 error, 1 warning. Entries with errors are
    read-only.";
  - Show problems expands the list; F8 expands it and focuses the list;
  - the items read "Error", "Line 3, column 7", and "Warning", "Table book,
    field isbn".

**Commands:**

- Open database… passes the filters and calls `open(path)` with no config
  argument.
- Drop calls `openDropped(paths)`. `DropAmbiguous` shows "Drop one database
  file at a time. Nothing was changed.".
- A `.toml` drop rejected with `ConfigNotDroppable` shows "The config can't
  be chosen by dropping it: the app always uses
  C:\ProgramData\FileManager\dbmanager.toml. Nothing was changed.". The
  open database and its header stay as they were.
- After Reload, the backup line stays. After Close and then Open, it is gone,
  along with any backup notice.
- While a command is pending, the actions are disabled, `aria-busy` is set,
  the status text shows, and the drop zone is disabled with its reason.
- Responses arrive out of order: only the newest is applied.
- A window `focus` calls `session()` and applies it.

**Edit config:**

- It is absent until developer mode is turned on, and then appears with
  Ctrl+E.
- `created: true` shows the starter notice; `created: false` announces
  "Opened the config in your editor.".
- `OpenerFailed` shows its copy with the path.

**Automatic reload:**

- An `onConfigChanged` event calls `session()`, applies the new config line
  and diagnostics, and shows "The config was reloaded.".
- With the new status `failClosed`, the event shows the failed-closed banner
  instead of the notice.
- The subscription is made once, survives unmounting the view, and works
  while another tool is showing.

**Errors:**

- `errorMessage` has an entry for every one of the 22 kinds, including
  `OpenerFailed` and `ConfigNotDroppable`; the typecheck enforces it. There is no `ConfigUnreadable`.
- `Busy` and `Io` differ by context.
- An IPC rejection that is not a `DbmanagerError` shows "Something went
  wrong. See Details."

**Keyboard and accessibility:**

- Ctrl+O, F5, Ctrl+R, F8 and (in developer mode) Ctrl+E work from anywhere in
  the view.
- F5 and Ctrl+R are prevented even with nothing open.
- No shortcut fires while a modal dialog is open.
- Every control is reachable by Tab, in visual order, and labelled.
- The menu follows U1's `MenuButton` keyboard behaviour.
- The Keyboard shortcuts popover lists the rows, and the Ctrl+E row only in
  developer mode.
- jsx-a11y lint passes.
- At 800×560 and at 200% text, the header wraps with no horizontal
  scrolling. Paths and banner text wrap.
- Light and dark both render from tokens; `dbmanager.css` has no hex.

## Tests proving completion

- `errorMessages.test.ts`:
  - `every kind has copy`;
  - `busy and io differ by context`;
  - `opener failed names the path`;
  - `config not droppable names the path`;
  - `unexpected errors claim nothing`;
  - `toDbmanagerError rejects unknown shapes`.
- `store.test.ts`:
  - `latest response wins`;
  - `pending blocks and clears`;
  - `config event subscribes once and survives unmount`;
  - `reset restores the initial state`.
- `DbmanagerView.session.test.tsx` (mock `lib/dbmanager`, `lib/tauri` and,
  where needed, `app/devMode` with `vi.mock`):
  - `empty state with recent databases`;
  - `open via dialog`;
  - `recent opens the path`;
  - `open shows header and config line`;
  - `read-only database tag and banner`;
  - `database slot error`;
  - `state warning`;
  - `drop calls openDropped`;
  - `drop ambiguous`;
  - `toml drop shows config not droppable and keeps the database`;
  - `backup line kept on reload and cleared on close/open`;
  - `pending disables actions`;
  - `focus refresh`;
  - `restore failure offers try again`;
  - `state survives remount`.
- `DbmanagerView.config.test.tsx`:
  - `loaded line`;
  - `missing banner without prompt and dismissal persists`;
  - `fail-closed banner and line`;
  - `location unknown`;
  - `fallback config changed banner reloads`;
  - `automatic reload notice`;
  - `reload to fail-closed shows banner not notice`;
  - `edit config only in developer mode`;
  - `starter created notice`;
  - `opener failed`.
- `ConfigProblems.test.tsx`:
  - `summary counts`;
  - `expand and F8 focus`;
  - `positions by line or table`;
  - `shown without a database`.
- `useDbmanagerShortcuts.test.tsx`:
  - `ctrl+O opens`;
  - `F5 and ctrl+R reload and are always prevented`;
  - `ctrl+E only in developer mode`;
  - `ignored while a dialog is open`.
- `dbmanager.css.test.ts`: `selectors scoped`, `no shadows`.

Run: `npm run typecheck && npm run lint && npm run test`.

## States covered

- **Loading:** initial session load, pending commands.
- **Empty:** nothing open, with and without recent databases.
- **Error:** restore failure, a command error (including `OpenerFailed`), a
  database slot error, config failed closed, `stateWarning`.
- **Success:** open; config loaded, missing or reloaded; read-only database;
  diagnostics; starter created.

## Out of scope

- The table list, object list, panes and history (U3); the object sheet (U4
  onwards).
- Asking before leaving unsaved edits, and re-checking drafts on a reload.
  U6 wraps Open, Recent, drop and Close with the leave guard, and extends
  `configReloaded()` and Reload with the draft re-check. Keep each of those
  actions as one store action that U6 can wrap.
- The window-close prompt (U13).
- The backup and delete notices' content (U6, U11). This task only renders
  the `notices` list.
- Choosing a config file: there is no such feature. The config is fixed.
