# U8 — Creating objects

Status: awaiting approval
Project: dbmanager   Depends on: U7 (landed), M10 (landed), M11 (landed)

## Goal

Add **New**, which opens a create form:

- defaults prefilled;
- required fields marked and checked;
- create-only fields editable;
- **Create** saves it and opens the new object.

## Context

**The product.** FileManager is a portable Windows desktop app (Tauri v2;
React + TypeScript + Vite in `apps/desktop/src/`). The **Database Manager**
(`dbmanager`) edits a SQLite file as an **object editor**. Creating is offered
only where `TableView.canCreate` is true, which is on by default. A create
form is the same sheet as an edit form:

- fields that are `create-only` (for example a text primary key, or an ISBN)
  are editable **only** here;
- fields that the database fills in (a rowid id, a generated column, a
  read-only field with a default) are shown as filled automatically;
- after a successful create, the new object opens.

Earlier tasks built:

- U3: places, including `{ kind: "new"; table }`, which U3 routes to the sheet
  pane; `navigate`, `replacePlace`, and the object list toolbar with its
  search;
- U4: `FieldRow` and tags;
- U5: `draft.ts` (`canEdit(field, phase, table, session)` with a `"create"`
  phase; `EditorValue`; `initialEditorValue`; `toDbValue`; `isChanged`;
  `issuesFor`), and the editors;
- U6: `saveDraft`, `SaveBar`, `ErrorSummary`, the leave guard and the backup
  notice;
- U7: references and `ReferenceField` (it renders **Choose…** for a required
  empty reference).

You own `apps/desktop/src/` **except `lib/`**: never edit it. Tools never
import each other.

**Contract rules.**

- Defaults come from `FieldView.defaultValue` (already a `DbValue`, durations
  canonical); the UI computes none.
- `requiredOnCreate` means NOT NULL, no default, and not a rowid alias.
- Send only the fields the user changed; the database applies its own
  defaults to the rest.

### The API used

- `save({ table, key: null, version: null, changes, links: [] })` →
  `SaveResult { object, backupCreated }`. `key: null` means create. On create,
  the backend reports a missing `requiredOnCreate` field as `ValidationFailed`
  with `required`.
- `missingRequiredOnCreate(fields: FieldView[], supplied: ReadonlySet<string>):
  string[]` from `lib/dbmanagerValidation.ts` (M11) returns the columns of
  required fields that are absent from `supplied`.

```ts
type FieldView = { column; displayName; mode: "editable" | "read-only" | "create-only"; kind;
  rule: FieldRule; isKey: boolean; requiredOnCreate: boolean; hasDefault: boolean;
  defaultValue: DbValue | null; foreignKey: ForeignKeyView | null; … };
type TableView = { name; displayName; mode; canCreate: boolean; fields: FieldView[]; … };
```

### The design (brief excerpt)

**Visual system** (`DESIGN.md`; no new tokens):

- **New** is a secondary `Button` with the `plus` icon. It is not primary:
  the region's one primary action is on the sheet;
- **Create** is the sheet's primary action;
- tags use U4's style; the **Required** tag is a word;
- tokens: `--surface`, `--surface-sunken`, `--border`, `--text`,
  `--text-muted`, `--accent`, `--danger`, the spacing and type scales.

**New:**

- in the object list toolbar, after the search;
- shown only when `table.canCreate`, the table is editable, and the session
  is writable (no `readOnlyReason`, no `configFailClosed`);
- visible label "New", accessible name "New object in {displayName}",
  `aria-keyshortcuts="Control+N"`, `title` "Shortcut: Ctrl+N";
- **Ctrl+N** works anywhere in the view, including inside inputs, when New
  is shown;
- it calls `navigate({ push: { kind: "new", table } })`, so U6's guard asks
  when a draft is dirty;
- U3's empty-table line adds "Create one with New." when New is shown.

**The create sheet** (place `new`):

- the location bar (Back returns to the table; the breadcrumb ends "New
  object");
- the eyebrow (the table's display name), the `h2` "New object"
  (`tabIndex={-1}`, focused on arrival), and a **Not saved yet** tag;
- no **Delete…** and no **Referenced by**; the many-to-many sections come in
  U9;
- **Fields**, all of `table.fields` in order:
  - `canEdit(field, "create", …)`: an editor. Its initial value comes from
    `defaultValue` when set (through `initialEditorValue` with a synthetic
    `FieldValue { value: defaultValue, checked: (bool) value === "1",
    link: null, problem: null }`), otherwise empty.
    - `hasDefault && !defaultValue`: an empty editor with the placeholder
      "Database default".
    - `requiredOnCreate`: the **Required** tag, and `aria-required="true"`.
    - A required empty reference shows **Choose…**.
  - Not editable on create (read-only, forced): U4's read form showing
    `defaultValue` when known, else the muted text "Set automatically".
    There is no Read-only tag here: the text explains.
- **The save bar** in create mode:
  - status "Not saved yet" (plus " · {n} fields filled in" when dirty);
  - **Cancel** (secondary): discards without asking and calls `navigate({
    back })` (or pushes the table's place when there is no back entry). It is
    an explicit discard, like Discard changes;
  - **Create** (primary): Ctrl+S, `title` "Shortcut: Ctrl+S", always
    enabled; the checks run on press.

**The create flow** (`saveDraft` in the create phase):

1. **Client check:**
   - the changed fields' `issuesFor`;
   - plus `missingRequiredOnCreate(fields that canEdit in create, new
     Set(changed columns))`: each missing column gets a `required` issue,
     whose copy is "Enter a value." (for a reference: "Choose an object.").

   Any issue shows them all and focuses the error summary ("Fix {n} problems
   before creating."); nothing is sent.
2. **Request:** `{ table, key: null, version: null, changes, links: [] }`,
   with only the changed fields. A prefilled default the user did not change
   is **not** sent; the database applies it.
3. **Success:**
   - `replacePlace({ kind: "object", table, key: result.object.key, label:
     result.object.label })`: the create entry is replaced, so Back goes to
     where the user was before New;
   - `current` = `result.object`; the draft is cleared;
   - the view announces "Created {label}.";
   - focus moves to the new sheet's `h2`;
   - the object list re-reads;
   - the backup notice appears when `backupCreated` is set (U6).
4. **Errors:** as U6 (`ValidationFailed` with server issues, `Busy` with Try
   again, `ConfigChangedOnDisk`, `SchemaChanged`, the others), with "Nothing
   was created." in the summary heading. A `Conflict` cannot happen on
   create.

**Leaving.** A dirty create form is guarded by U6's dialog, with the body
"Your new object is not saved. If you leave now, it is discarded.".

**Shortcut row added** to `shortcuts.ts`:

| Keys | Action | Active when |
| --- | --- | --- |
| Ctrl+N | New object | The table allows creating |

## Files

Edit in `apps/desktop/src/tools/dbmanager/`:

- `ObjectList.tsx` (New, the empty-state line);
- `ObjectSheet.tsx` (the create place);
- `FieldRow.tsx` (the Required tag, "Set automatically", the "Database
  default" placeholder);
- `SaveBar.tsx` (create mode);
- `store.ts` (the create phase of `saveDraft`);
- `draft.ts` (initial values from defaults);
- `leaveGuard.tsx` (create body);
- `shortcuts.ts`, `useDbmanagerShortcuts.ts`, `fixtures.ts`,
  `styles/dbmanager.css`.

Create `create.test.tsx` and `ObjectList.new.test.tsx`.

Fixtures from the example config:

- **book:** `title` and `isbn` required (ISBN create-only, with a pattern),
  `author_id` a required FK, `format` defaulting to `"paperback"`, `in_print`
  defaulting to `"1"`, `revision` read-only with default `"1"`, `id` the rowid
  alias (read-only);
- **genre:** `code` create-only (a text key, required);
- **publisher:** read-only, `canCreate: false`;
- a field with `hasDefault: true, defaultValue: null`.

## Skill

- Run `/impeccable onboard` on the create form: an empty table's first
  object, with the defaults and the required fields legible at a glance. Then
  run `/impeccable clarify` on the create copy (keep the meaning; report
  proposed changes). Run both from `apps/desktop` with `--target
  src/tools/dbmanager/ObjectSheet.tsx`.
- Read `reference/craft-floor.md` before the first UI edit.
- Extend the established world.
- Run `/impeccable audit` and the detector before done.
- If the skill is missing, install it (`npx impeccable install`) or follow
  its references by hand, and say so.

## Acceptance criteria

**New:**

- It is shown for Books, Authors and Genres in a writable session.
- It is not shown for Publishers, nor for any table when `readOnlyReason` or
  `configFailClosed` is set.
- Its accessible name is "New object in Books". Ctrl+N works from inside the
  search input.
- An empty table says "… has no objects yet. Create one with New.".

**The create form for a book:**

- `h2` "New object", the Not saved yet tag, and focus on the `h2`.
- Format is prefilled "Paperback" and In print is ticked.
- Revision shows "1" read-only; Id shows "Set automatically".
- Title, ISBN and Author carry **Required**; Author shows **Choose…**.
- ISBN is editable (create-only).
- There is no Delete and no Referenced by.

**Creating:**

- Create with nothing filled in sends nothing. The summary lists Title,
  ISBN and Author (Author: "Choose an object.") and takes focus.
- Filling Title, ISBN and Author, then pressing Create, sends `{ table:
  "book", key: null, version: null, changes: [title, isbn, author_id
  (reference)], links: [] }`. Format and In print are **not** sent.
- On success: the place is replaced by the new object, so Back returns to
  the list place before New; focus is on its `h2`; "Created Dune." is
  announced; the list re-reads.
- A server `unique` on ISBN shows under ISBN, with the summary heading
  "Nothing was created. …".
- Genre's create form has an editable Code.

**Cancel and the leave guard:**

- Cancel discards without asking and goes back.
- Leaving a dirty create form by any other route asks with the create body.

**Accessibility:**

- Required is a word as well as `aria-required`.
- Every control is labelled and reachable.
- jsx-a11y lint passes. No horizontal scrolling at 200% text.

## Tests proving completion

- `ObjectList.new.test.tsx`:
  - `new shown only when allowed and writable`;
  - `accessible name and ctrl+N`;
  - `empty table hint`.
- `create.test.tsx`:
  - `defaults prefilled and not sent`;
  - `set automatically and database default`;
  - `required tags and choose`;
  - `missing required blocks and focuses summary`;
  - `request shape`;
  - `success replaces place and focuses`;
  - `server unique on create`;
  - `cancel discards without asking`;
  - `guard asks with create body`;
  - `create-only key editable`.

Run: `npm run typecheck && npm run lint && npm run test`.

## States covered

- **Create form:** empty, prefilled, invalid (client and server), creating,
  created.
- **New:** shown, hidden.
- **Leave:** guarded, cancelled.

## Out of scope

- Many-to-many checklists on the create form (U9) and link fields (U10).
- Delete (U11).
