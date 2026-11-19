# U7 — Editing references with the object picker

Status: awaiting approval (revised 2026-11-16: pickable targets; 2026-11-18:
no non-navigable FKs)
Project: dbmanager   Depends on: U6 (landed), M10 (landed)

## Goal

Let the user change a foreign-key field by picking from the referenced table's
objects:

- a searchable, paged, keyboard-operable picker;
- clearing nullable references;
- saving the pick as a reference.

## Context

**The product.** FileManager is a portable Windows desktop app (Tauri v2;
React + TypeScript + Vite in `apps/desktop/src/`). The **Database Manager**
(`dbmanager`) edits a SQLite file as an **object editor**. A foreign key is
shown as the referenced object's **label**, never a bare id, and editing it
means **picking another object**, not typing a number. Picking from a
read-only table is allowed. So is picking from a **pickable** (lookup) table
such as Publishers: it is not in the table list, but it is in
`session.tables` with `mode: "pickable"`, and its objects are listed in
pickers like any other.

Earlier tasks built:

- U2: the store and errors;
- U3: places, `navigate`, and the object list's search and paging patterns;
- U4: `ReferenceField` in read form (the label link that follows with
  `navigate`; dangling and NULL forms), `FieldRow`, hints and
  tags;
- U5: `draft.ts` (`EditorValue`, `canEdit`, `isChanged`, the store's
  `editField` / `revertField`), and the **Edited** tag;
- U6: `saveDraft()`, which builds `changes`; the error summary; the server
  issues; the leave guard.

You own `apps/desktop/src/` **except `lib/`**: never edit it. Tools never
import each other.

**Contract rules.**

- Keys are opaque: compare them only through `objectId(table, key)`
  (`keys.ts`).
- Labels come from the backend.
- A FK into a hidden table or an unlisted junction is not a reference for
  the UI: it arrives as a plain read-only field (no `foreignKey`, no
  `link`), so it never gets **Change…**. Every `ForeignKeyView` that does
  arrive has a listed target, `navigable: true` and a `tableDisplayName`.

### The API used

`lib/dbmanager.ts`: `list(table, { search, offset, limit, referencing: null
})` → `ObjectPage { items: { key, label }[]; total; offset }`.

`save` takes a reference as `FieldChange { column, input: { type:
"reference", key: ObjectKey | null } }`. `key: null` stores NULL. The backend
checks that the picked object still exists. If it does not, the result is
`ValidationFailed` with the issue `referenceMissing` on that column; U6
already shows server issues. Its copy is "The chosen object no longer exists.
Choose another.".

### Types (excerpt)

```ts
type FieldView = { column; displayName; mode; kind; rule: FieldRule;   // rule.nullable matters here
  foreignKey: { table: string; tableDisplayName: string | null; navigable: boolean } | null; … };
type FieldValue = { column; value: DbValue; link: { table; key: ObjectKey | null; label: string | null;
  navigable: boolean } | null; problem: … };
type TableView = { name; displayName; searchable: boolean; mode; … };   // from session.tables
```

### Draft changes (`draft.ts`)

- Add `{ type: "reference"; key: ObjectKey | null; label: string | null }` to
  `EditorValue`.
- `initialEditorValue` for a FK field: the link's `key` and `label`, or
  `null` / `null` for NULL. A dangling link has `key: null` but a non-NULL
  value, so record it as "unchanged" and **never** send it unless the user
  picks.
- `isChanged`: true when the user picked or cleared, and
  `objectId(fk.table, picked)` (or null) differs from the base link's.
- `canEdit` now admits FK fields (`foreignKey` set) whose target table is in
  `session.tables` (any mode, pickable included); the other conditions from
  U5 stay. Do not test `foreignKey.navigable`: it is always true.
- The save request already carries `configHash` (U6); references change
  nothing about that.
- U6's `saveDraft` sends `{ type: "reference", key }` for changed FK fields,
  and `{ type: "value", … }` for every other field.

### The design (brief excerpt)

**Visual system** (`DESIGN.md`; no new tokens):

- the picker is an overlay: `--surface`, 1 px `--border`, `--radius`,
  `--shadow-overlay`. It is the **only** shadowed element in
  `dbmanager.css`; update `dbmanager.css.test.ts` to allow exactly
  `.dbmanager .picker`;
- the active option: a `--surface-sunken` fill plus the global focus ring
  inset (`outline-offset: -2px`);
- muted counts in `--text-muted`, `.num`;
- tokens: `--surface`, `--surface-sunken`, `--border`, `--text`,
  `--text-muted`, `--danger`, `--shadow-overlay`, `--focus-ring`, the
  spacing and type scales.

**`ReferenceField`, editable form** (when `canEdit`):

- The current value as in U4: the label link (it follows; with a dirty draft,
  U6's guard asks), "Not set", or the dangling form.
- After it:
  - **Change…** (secondary; accessible name "Change {displayName}"), which
    opens the picker;
  - **Clear** (quiet; "Clear {displayName}"), only when `rule.nullable` and
    the value is not NULL. It stages `{ key: null, label: null }`.
- When the field has no value and is required: **Choose…** instead of
  **Change…** (this matters on U8's create form).
- After a pick, the link shows the picked label with the **Edited** tag.
  Escape on the field's buttons reverts it (U5's rule).

**`ObjectPicker`** (a non-modal overlay):

- **Placement.** It renders in a portal on `document.body`, positioned under
  the Change… button with `position: fixed` from the button's
  `getBoundingClientRect()`:
  - flipped above when less than 20rem remains below;
  - clamped to the viewport with `--space-4` margins;
  - width `min(28rem, 100vw - 2 × --space-4)`, max height `min(24rem, 70vh)`.

  It repositions on resize and on scroll of any ancestor (capture listener).
- **Header:** "Choose {tableDisplayName}" (`h2`, `--font-size-md`, weight
  600). The picker has `role="dialog"`, `aria-modal="false"` and
  `aria-labelledby` pointing at the header.
- **Search** (only when the target table is `searchable`): an `.input` with
  `role="combobox"`, `aria-expanded="true"`, `aria-controls` (the listbox),
  `aria-autocomplete="list"`, `aria-activedescendant` (the active option),
  and the accessible name "Search {tableDisplayName}". It is focused on open.
  Typing searches after 200 ms. **Latest wins.**
- **Results:** `role="listbox"` (accessible name `tableDisplayName`), with up
  to 25 `role="option"` items per page showing `label`.
  - Long labels wrap; U+FFFD is shown as is.
  - The current value's option ends with the word "Current" (`--text-muted`)
    and has `aria-selected="true"`.
  - When the table is not searchable, the listbox itself takes focus
    (`tabIndex={0}`, `aria-activedescendant`).
- **Footer:**
  - "Showing 25 of 312" (`.num`);
  - **Show more** (quiet), which appends the next 25 while more remain;
  - **Clear**, when the field is nullable ("Set to not set").
- **Keyboard:**
  - Down/Up move the active option and stop at the ends; Home/End jump (in the
    listbox);
  - Enter picks the active option;
  - **Escape** closes without a change;
  - Tab moves through the footer buttons.

  Focus leaving the picker, or a `mousedown` outside it, closes it. On every
  close, focus returns to **Change…**.
- **Picking:** `editField(column, { type: "reference", key, label })`. The
  picker closes, and the view announces "{displayName}: {label}.".
- **States:**
  - **loading:** "Loading…" with `aria-busy` on the listbox; earlier options
    stay while more load;
  - **empty table:** "{tableDisplayName} has no objects.";
  - **no match:** "No objects match “{search}”.";
  - **error:** an inline `x-circle` line with `errorMessage(e, "read")` and
    **Try again**. `SchemaChanged` also sets the store flag.

## Files

Create in `apps/desktop/src/tools/dbmanager/`:

- `ObjectPicker.tsx`;
- tests: `ObjectPicker.test.tsx`, `ObjectPicker.keyboard.test.tsx`,
  `ReferenceField.edit.test.tsx`.

Edit:

- `ReferenceField.tsx`;
- `draft.ts` and `draft.test.ts` (the reference value);
- `store.ts` (`saveDraft` sends references);
- `fixtures.ts`;
- `styles/dbmanager.css`, `styles/dbmanager.css.test.ts`;
- the root `DESIGN.md`: in Elevation & Depth, mark the object picker as
  landed: "the Database Manager's object picker (landed, dbmanager U7)".
  Change nothing else there.

Fixtures:

- the book "Dune": Author → Authors (250 generated, searchable); Publisher →
  Publishers (`mode: "pickable"`, 2 objects, nullable);
- a field whose target is not searchable;
- a dangling Author.

## Skill

- Run `/impeccable polish` on the picker: placement at the window edges,
  flipping, 200% text, and light and dark. Then run `/impeccable harden`:
  slow results, stale responses, an empty table, a deleted target. Run them
  from `apps/desktop` with `--target src/tools/dbmanager/ObjectPicker.tsx`.
- Read `reference/craft-floor.md` before the first UI edit.
- Extend the established world: the shared shortcuts popover is the
  incumbent overlay.
- Run `/impeccable audit` and the detector before done.
- If the skill is missing, install it (`npx impeccable install`) or follow
  its references by hand, and say so.

## Acceptance criteria

**The editable field:**

- Author on an editable book shows the link, **Change…**, and no Clear (not
  nullable). Publisher (nullable) shows Clear.
- A FK column into a hidden table (arriving with `foreignKey: null`) is a
  plain read-only value with no Change….

**The picker:**

- It opens with the search focused. It lists 25 authors with "Showing 25 of
  250", and the current author is marked "Current".
- Typing "her" requests `{ search: "her", offset: 0, limit: 25, referencing:
  null }` after 200 ms; a stale response is ignored.
- Down/Up/Home/End and Enter pick. Escape closes and focuses Change…, and
  outside clicks and focus loss also close it.
- Show more appends offset 25.
- A non-searchable target opens with the listbox focused.
- Empty, no match, loading and error states each render.

**Picking and saving:**

- Picking "Le Guin, Ursula" stages the change: the link shows the new label,
  **Edited**, and the save bar counts it.
- Save sends `{ column: "author_id", input: { type: "reference", key } }`.
  Clear on Publisher sends `key: null`.
- A dangling Author that the user does not touch is not sent.
- A `referenceMissing` server issue shows under the field with its copy.

**Accessibility and layout:**

- The combobox and listbox ARIA is as specified, with labelled controls and
  visible focus.
- The picker never leaves the viewport at 800×560, and is not clipped by any
  pane.
- jsx-a11y lint passes.
- The CSS test allows exactly one shadowed selector, `.dbmanager .picker`.

## Tests proving completion

- `draft.test.ts`:
  - `reference initial and changed`;
  - `dangling unchanged is not sent`.
- `ReferenceField.edit.test.tsx`:
  - `change and clear buttons by nullability`;
  - `hidden target not editable`;
  - `pick stages and marks edited`;
  - `escape reverts`;
  - `save sends reference input`;
  - `reference missing issue`.
- `ObjectPicker.test.tsx`:
  - `first page and current marker`;
  - `search debounce and stale`;
  - `show more`;
  - `clear`;
  - `states: loading, empty, no match, error`;
  - `non-searchable target`.
- `ObjectPicker.keyboard.test.tsx`:
  - `arrows, home, end, enter`;
  - `escape and outside close return focus`;
  - `tab through footer`.
- `dbmanager.css.test.ts`:
  - `only the picker has a shadow`.

Run: `npm run typecheck && npm run lint && npm run test`.

## States covered

- **Picker:** loading, results, more, empty, no match, error.
- **Field:** linked, not set, dangling, staged pick, cleared, server issue.

## Out of scope

- The create form's use of Choose… (U8 wires it; this task renders it).
- References in link fields (U10 reuses `ReferenceField` and the picker).
- Creating a new target object from the picker: not in v1.
