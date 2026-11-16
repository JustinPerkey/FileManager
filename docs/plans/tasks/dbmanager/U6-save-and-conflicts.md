# U6 — Saving, conflicts, and leaving with unsaved changes

Status: awaiting approval
Project: dbmanager   Depends on: U5 (landed), M10 (landed), M11 (landed)

## Goal

Make edits saveable:

- an explicit, transactional Save with a visible dirty state;
- authoritative server validation with an error summary;
- a conflict panel when another program changed the object;
- a prompt before leaving unsaved changes;
- drafts that survive a reload;
- the first-write backup notice.

## Context

**The product.** FileManager is a portable Windows desktop app (Tauri v2;
React + TypeScript + Vite in `apps/desktop/src/`). The **Database Manager**
(`dbmanager`) edits a SQLite file as an **object editor**. Saves are
**explicit** (the user presses Save) and **transactional**: one save is one
database transaction, and any failure changes nothing. Before the first write
of each opened session, the backend copies the database to a backup beside it,
and the UI must show where.

Earlier tasks built:

- U2: the store (`useDbmanager()`), the session header and banners (including
  the `notices` list and the `schemaChanged` flag), and `errorMessages.ts`
  (`errorMessage(e, "read" | "write")`, `toDbmanagerError`, `issueMessage`,
  `problemMessage`). U2 also built the session actions: Open, Recent, drop,
  Choose config, Open without config, Reload and Close.
- U3: places, history, `navigate(intent)` (all navigation goes through it),
  the sheet shell, and `current: ObjectView`.
- U4: `FieldRow` and the read representations (`values.ts`:
  `displayValue`).
- U5: `draft.ts` (`EditorValue`, `initialEditorValue`, `toDbValue`,
  `isChanged`, `issuesFor`, `canEdit`), the store's `draft` (`edits`,
  `touched`, `showAllIssues`, `serverIssues`), `editField`, `revertField`,
  `discardDraft` and `dirtyCount()`.

You own `apps/desktop/src/` **except `lib/`**: never edit it. Tools never
import each other.

**Contract rules.**

- The save result is authoritative: the UI renders the returned object and
  keeps no second copy.
- Versions are opaque. Echo `current.version` back.
- Saving validates only changed fields, so send only changed fields.
- Every write failure means nothing was changed, and the copy says so.

### The API used

`lib/dbmanager.ts` (M10):

- `save(request: SaveRequest): Promise<SaveResult>`;
- `object(table, key)`;
- `session()`;
- the session actions U2 wraps.

`lib/dbmanagerValidation.ts` (M11):

- `validateField`, used by U5's `issuesFor`;
- `missingRequiredOnCreate(fields: FieldView[], supplied: ReadonlySet<string>):
  string[]` (U8 uses it on create; this task wires the check into the save
  flow).

```ts
type SaveRequest = { table: string; key: ObjectKey | null;   // null: create (U8)
  version: string | null;                                    // required when key is set
  changes: FieldChange[]; links?: LinkChange[] };            // links: U9/U10 (send [] or omit)
type FieldChange = { column: string; input: FieldInput };
type FieldInput = { type: "value"; value: DbValue } | { type: "reference"; key: ObjectKey | null };
type SaveResult = { object: ObjectView; backupCreated: string | null };   // display path, first write only
type DbmanagerError = { kind; message; fieldErrors?: FieldError[]; linkErrors?: LinkError[];
  current?: ObjectView; conflictLink?: LinkRef; impact?: DeleteImpact };
type FieldError = { column: string; issues: FieldIssue[] };
```

If the generated `links` field is required rather than optional, send `links:
[]`. If a name differs, the generated file wins: say so.

**Save errors that matter:**

| Kind | Payload | Meaning |
| --- | --- | --- |
| `ValidationFailed` | `fieldErrors` (always present), `linkErrors` (U10) | the save was refused |
| `Conflict` | `current` (the object as it is now); `conflictLink` (U9) | the object changed since it was read |
| `ConfigChangedOnDisk` | — | the config changed; writes are refused until a reload |
| `SchemaChanged` | — | the structure changed; reload |
| `Busy` | — | the database is locked |
| `DatabaseReadOnly`, `NotEditable`, `BackupFailed`, `ConstraintFailed`, `UnknownObject`, `Io` | — | shown as an error |

### The design (brief excerpt)

**Visual system** (`DESIGN.md`; no new tokens):

- `--accent` for the one primary action per region (**Save**);
- U1's `Button` variants (`primary`, `secondary`, `quiet`, `danger`) and
  `ConfirmDialog`;
- `Banner` (error banners are `role="alert"`);
- `--surface`, `--surface-sunken`, `--border`, `--text`, `--text-muted`,
  `--danger`, `--warn`; the spacing and type scales.

**The save bar** (`SaveBar`):

- sticky at the bottom of the sheet pane, offset by the pane's padding;
  static in windows under 34rem tall;
- `--surface` with a 1 px `--border` rule above;
- **left**, the status text in `--text-muted`, also announced:
  - "No unsaved changes";
  - "{n} unsaved changes" (or "1 unsaved change"), with the visually hidden
    suffix " (Ctrl+S to save)";
  - "Saving…";
  - "Saved." (until the next edit);
  - or the blocked reason, "Resolve the conflict first.";
- **right:**
  - **Discard changes** (secondary), enabled when dirty. It discards
    immediately, with no dialog: the edits are visible and nothing was
    written. The view announces "Changes discarded.";
  - **Save** (primary): `aria-keyshortcuts="Control+S"`, `title`
    "Shortcut: Ctrl+S", enabled when dirty and not blocked, busy while
    saving;
- shown only when at least one field on the sheet is editable (`canEdit`).
  U9 adds editable checklists to that test.

**The save flow** (a store action, `saveDraft()`; Ctrl+S calls it from
anywhere in the view, including inputs):

1. **Client check.** Every changed field's `issuesFor` must be empty.
   Otherwise:
   - set `showAllIssues`;
   - show the **error summary** with "Fix {n} problems before saving." (or
     "1 problem");
   - move focus to it;
   - send nothing.
2. **Request:** `{ table, key: current.key, version: current.version, changes,
   links: [] }`. `changes` holds one `{ column, input: { type: "value",
   value: toDbValue(field, edit) } }` per changed field (`isChanged`), in field
   order.
3. **While saving:**
   - every editor is read-only (`readOnly` / `aria-readonly`; checkboxes
     ignore toggles), so focus stays put;
   - Save is busy;
   - `navigate`, the session actions and Delete are disabled;
   - the drop zone says "Wait for the save to finish".
4. **Success:**
   - `current` becomes `result.object`, and the draft is cleared;
   - the status reads "Saved.", and the view announces "Saved {label}.";
   - the current object-list page is re-read (labels may change);
   - if `backupCreated` is set, push a notice "A backup of the database was
     saved before this first change: {backupCreated}." (dismissible, info),
     then re-read the session so the header shows its backup line;
   - **focus:** if it was on Save, which is now disabled, move it to the
     sheet's `h2`; otherwise leave it.
5. **Errors:**
   - **`ValidationFailed`:**
     - put `fieldErrors` into `serverIssues` and set `showAllIssues`;
     - show the error summary, "Nothing was saved. Fix {n} problems:", with
       one item per issue: "{displayName}: {issueMessage}". Each item is a
       button that focuses that field's control;
     - focus the summary;
     - a `fieldErrors` column that matches no visible field is listed with
       its column name.
   - **`Conflict`** without `conflictLink`: the **conflict panel** (below).
   - **`ConfigChangedOnDisk`:** re-read the session (U2's config-changed
     banner appears), and show a sheet banner with `errorMessage(e,
     "write")`.
   - **`SchemaChanged`:** set the store's `schemaChanged` flag (U2's banner,
     with Reload).
   - **`Busy`:** a sheet error banner with `errorMessage(e, "write")` and a
     **Try again** action that repeats `saveDraft()`.
   - **Any other kind** or *Unexpected*: a sheet error banner with
     `errorMessage(e, "write")` and Details (`e.message` in `.mono`).

   Sheet banners sit above the Fields section and are dismissible. A new
   save attempt clears them.

**The error summary** (`ErrorSummary`):

- a `div` with `tabIndex={-1}` and `role="group"`, labelled by its heading;
- a 1 px `--danger` border on `--surface`, with the `x-circle` icon;
- the heading, then a list of buttons styled as links.

It is not `role="alert"`, because focus moves to it and the heading is read.

**The conflict panel** (`ConflictPanel`), above Fields:

- the message: "“{label}” was changed by another program after you opened it.
  Nothing was saved." (warn tone, `alert-triangle` icon, a word-bearing
  heading);
- a list, one row per changed field: "{displayName} — Yours: {your value} ·
  Now: {current value}". Values use `displayValue`; empty or null show "Not
  set";
- when unedited fields also differ between the draft's base and `current`:
  "{n} other fields changed too.";
- the actions:
  - **Re-apply my changes** (secondary): `rebaseDraft` onto `current`. The
    panel closes, and the status shows the new dirty count;
  - **Discard my changes** (quiet): `current` replaces the base, and the
    draft clears;
- focus moves to the panel's heading (`tabIndex={-1}`);
- while the panel is shown, Save is blocked ("Resolve the conflict first.").

**`rebaseDraft(draft, table: TableView, fresh: ObjectView) => { draft,
dropped: string[] }`** in `draft.ts`, pure:

- keeps each edit whose field still exists and still `canEdit`;
- drops the others and returns their display names;
- clears `serverIssues`;
- the new base is `fresh`, with its `version`.

**The leave guard.**

- `navigate(intent)` and the session actions **Open database…**, **Recent**,
  **drop** and **Close database** check `dirtyCount() > 0` first.
- If dirty, they open a `ConfirmDialog`:
  - title "Discard unsaved changes?";
  - body "Your changes to “{label}” are not saved. If you leave now, they are
    discarded.";
  - `cancelLabel` "Keep editing" (focused first);
  - `confirmLabel` "Discard changes", `confirmVariant="danger"`.

  Confirm discards the draft and performs the pending intent. Cancel leaves
  everything as it was, and focus returns to the control that asked.
- A drop is checked **after** it arrives. A drop of only a `.toml` file is a
  config change (below) and does not ask.
- Switching to another tool does not ask: the store keeps the draft, and it
  is still there on return.

**Drafts survive config changes and reloads.**

- After a successful **Reload**, **Choose config…**, **Open without config**,
  a config-only drop, or a reload that resolves `schemaChanged`, the view
  does the following when a draft is dirty:
  1. Re-read the object with `object(table, key)`.
  2. **Same version:** `rebaseDraft` onto the fresh object. If anything was
     dropped, push a notice: "After the reload, these changes no longer apply
     and were removed: {names}."
  3. **Different version:** the conflict panel.
  4. **`UnknownObject` / `UnknownTable`:** drop the draft, and push a notice
     "“{label}” is no longer available; your unsaved changes were
     discarded."
- These actions never ask first. Saving is refused until the user reloads,
  so discarding on reload would punish the user for doing what the app
  asked.

**Shortcut row added** to `shortcuts.ts`:

| Keys | Action | Active when |
| --- | --- | --- |
| Ctrl+S | Save | The object has unsaved changes |

Ctrl+S always has its default prevented in the view.

## Files

Create in `apps/desktop/src/tools/dbmanager/`:

- `SaveBar.tsx`, `ErrorSummary.tsx`, `ConflictPanel.tsx`, `leaveGuard.tsx`
  (the dialog wiring);
- tests: `save.test.tsx`, `conflict.test.tsx`, `leaveGuard.test.tsx`,
  `rebase.test.ts`, `SaveBar.test.tsx`.

Edit:

- `store.ts`: `saveDraft`, `conflict` state, sheet banners, and guard
  wrapping of `navigate` and the session actions;
- `draft.ts`: `rebaseDraft`;
- `ObjectSheet.tsx`, `FieldRow.tsx` (server issues, read-only while saving),
  `DbmanagerView.tsx`;
- `shortcuts.ts`, `useDbmanagerShortcuts.ts`, `fixtures.ts`,
  `styles/dbmanager.css`.

## Skill

- Run `/impeccable harden` on the save flow: errors, slow saves (a 3-second
  backup), double submits and edge cases. Then run `/impeccable clarify` on
  the summary, conflict and guard copy (keep the meaning; report proposed
  changes). Run both from `apps/desktop` with `--target
  src/tools/dbmanager/SaveBar.tsx`.
- Read `reference/craft-floor.md` before the first UI edit.
- Extend the established world. The tarpack build bar is the incumbent for a
  sticky action bar.
- Run `/impeccable audit` and the detector before done.
- If the skill is missing, install it (`npx impeccable install`) or follow
  its references by hand, and say so.

## Acceptance criteria

**The save bar:**

- Editing Title and Pages shows "2 unsaved changes", and both Save and
  Discard changes are enabled.
- Discard changes clears both and announces "Changes discarded.".

**Saving:**

- Save (button or Ctrl+S, from inside an input) sends exactly `{ table:
  "book", key, version, changes: [title, pages], links: [] }`. Unchanged and
  legacy-problem fields are not sent.
- While the save is pending:
  - editors are read-only and keep focus;
  - Back, list items and header actions are disabled;
  - a second Ctrl+S sends nothing.
- On success: "Saved." is shown and announced, the sheet shows the returned
  object, and the list re-reads.
- If `backupCreated` is set, the notice shows the path and the header gains
  its backup line (after a session re-read). A second save adds no notice.

**Errors:**

- With "0" in Pages, Save sends nothing. The summary "Fix 1 problem before
  saving." takes focus; its item focuses Pages.
- `ValidationFailed` with `isbn: unique`:
  - the summary "Nothing was saved. Fix 1 problem:" with "ISBN: Another
    object already has this value.";
  - the issue shows on the field until it is edited.
- `Conflict` shows the panel:
  - Yours and Now for each edited field, and the "other fields" count;
  - Save is blocked with the reason;
  - Re-apply keeps the edits on the new version; Discard loads `current`.
- `Busy` shows a banner whose Try again repeats the save. `ConfigChangedOnDisk`
  re-reads the session. `SchemaChanged` sets the flag. Every write error's
  copy says nothing was changed.

**The leave guard:**

- With a dirty draft, following a reference, Back, choosing a table, opening
  a list item, Open database…, Recent, a database drop and Close each open
  "Discard unsaved changes?".
- Keep editing changes nothing and returns focus.
- Discard changes performs the action.
- A clean draft never asks. Switching tools never asks, and the draft is
  still there on return.

**Reloads:**

- Reload with a dirty draft and an unchanged version keeps the edits.
- A field that became read-only in the new config is dropped, with the
  notice listing it.
- A changed version shows the conflict panel.
- A deleted object drops the draft, with its notice.

**Accessibility:**

- The summary, the conflict panel and the dialog are reachable and announced
  as specified. Focus moves follow the rules above.
- jsx-a11y lint passes. The save bar wraps at 200% text without covering
  content.

## Tests proving completion

- `rebase.test.ts`:
  - `keeps editable edits`;
  - `drops removed and read-only fields`;
  - `clears server issues`.
- `SaveBar.test.tsx`:
  - `status texts`;
  - `discard`;
  - `blocked reason`;
  - `hidden when nothing editable`.
- `save.test.tsx`:
  - `request holds only changed fields`;
  - `client issues block and focus summary`;
  - `success updates sheet and list`;
  - `backup notice once and header refresh`;
  - `validation failed maps server issues`;
  - `busy retry`;
  - `config changed refreshes session`;
  - `schema changed flag`;
  - `pending blocks navigation and double submit`;
  - `focus after save`.
- `conflict.test.tsx`:
  - `panel lists yours and now`;
  - `reapply rebases`;
  - `discard loads current`;
  - `save blocked meanwhile`.
- `leaveGuard.test.tsx`:
  - `each leaving action asks when dirty`;
  - `keep editing`;
  - `discard proceeds`;
  - `clean never asks`;
  - `tool switch keeps draft`;
  - `reload keeps draft`;
  - `reload drops fields with notice`;
  - `reload conflict`;
  - `reload object gone`.

Run: `npm run typecheck && npm run lint && npm run test`.

## States covered

- **Save bar:** clean, dirty, saving, saved, blocked.
- **Errors:** client invalid, server invalid, conflict, busy, config changed,
  schema changed, any other write error.
- **Success:** saved, saved with backup.
- **Leave:** asked, kept, discarded.

## Out of scope

- Reference edits (U7) and their `reference` inputs: the shape is ready.
- Create (U8): `key: null`, `Create` label, `missingRequiredOnCreate`.
- Link changes and `linkErrors` / `conflictLink` (U9, U10).
- Delete (U11).
