# U4 — The object sheet: reading an object

Status: awaiting approval (revised 2026-11-16: pickable tables open read-only;
2026-11-18: FKs into hidden or unlisted tables are plain fields)
Project: dbmanager   Depends on: U3 (landed), M10 (landed)

## Goal

Render an opened object as a property sheet:

- every visible field with its typed value, its tags and hints, and any
  stored-value problem;
- references as followable links;
- a **Referenced by** section listing what points at the object.

## Context

**The product.** FileManager is a portable Windows desktop app (Tauri v2;
React + TypeScript + Vite in `apps/desktop/src/`). The **Database Manager**
(`dbmanager`) edits a SQLite file as an **object editor, not a grid**. The
object is the subject: its label is the title, its fields read like a
property sheet, and its neighbourhood (what it points to and what points to
it) sits on the same sheet. A foreign key is never a bare number: it shows the
referenced object's **label** as a link, and following it opens that object
(Back returns).

Earlier tasks built:

- U2: the view frame and module-level store (`useDbmanager()`), and
  `errorMessages.ts`;
- U3: the panes, places and history. `navigate({ push: place })` and the
  object sheet shell (`ObjectSheet.tsx`) load `object(table, key)` into the
  store's `current: ObjectView` and render the location bar, the eyebrow, the
  `h2` label and the Read-only tag. The helper is `objectId` (`keys.ts`).

This task renders the sheet's body. Editing comes in U5.

You own `apps/desktop/src/` **except `lib/`**: never edit it. Tools never
import each other.

**Contract rules.**

- The UI does no label, kind, unit or SQL logic. Display names, help, kinds,
  modes, bounds, units and labels all come from the views below.
- Keys are opaque.
- Hidden fields and tables never reach the UI.
- A FK into a hidden table, or into an unlisted junction table, arrives as a
  **plain read-only field**: no `foreignKey` on its `FieldView`, and no
  `link` on its value. Render it like any other read-only value of its kind.
- Every `ForeignKeyView` the UI receives therefore has a listed target,
  `navigable: true`, and a non-null `tableDisplayName`. The generated type
  still declares `navigable` and a nullable `tableDisplayName`; do not branch
  on them, and treat a `false` / `null` as a contract violation (render the
  plain value and log nothing user-facing).
- Read-only fields arrive with mode `read-only`; this includes the forced
  ones (rowid alias, generated, BLOB, composite FK, untyped).
- Values may carry a `problem` for legacy data: **show it, and never block
  anything else because of it.**

### The client (`lib/dbmanager.ts`)

`list(table, query: ListQuery): Promise<ObjectPage>`, with `query.referencing
= { relation, key }` for "Referenced by" lists. It rejects with a
`DbmanagerError`; `SchemaChanged` sets the store's `schemaChanged` flag (as in
U3).

### Types (generated)

```ts
type FieldView = { column: string; displayName: string; help: string | null;
  mode: "editable" | "read-only" | "create-only";   // never hidden
  kind: "integer" | "real" | "text" | "bool" | "duration" | "blob" | "any";
  declaredType: string; rule: FieldRule; isKey: boolean; requiredOnCreate: boolean;
  hasDefault: boolean; defaultValue: DbValue | null; multiline: boolean;
  foreignKey: ForeignKeyView | null };
type ForeignKeyView = { table: string; tableDisplayName: string | null; navigable: boolean };
type FieldRule = { kind: FieldKind; nullable: boolean; unit: DurationUnit | null;
  min: string | null; max: string | null;            // integer/real: decimal; duration: canonical text
  minLength: number | null; maxLength: number | null; pattern: string | null;
  allowed: AllowedValue[] | null; emptyIsNull: boolean };
type AllowedValue = { value: DbValue; label: string };
type DurationUnit = "milliseconds" | "seconds" | "minutes" | "hours" | "days";
type DbValue = { type: "null" } | { type: "integer"; value: string } | { type: "real"; value: string }
  | { type: "text"; value: string } | { type: "duration"; value: string }
  | { type: "blob"; value: { size: number } };
type ObjectView = { table; key: ObjectKey; label; version;
  fields: FieldValue[];               // in TableView.fields order, visible only
  referencedBy: RelationCount[];      // one per TableView.referencedBy, same order
  manyToMany: RelationCount[] };
type FieldValue = { column: string; value: DbValue;
  checked: boolean | null;            // bool kind: true/false for 1/0; null for NULL or a problem
  link: LinkTarget | null;            // FK fields with a non-NULL value
  problem: "wrongType" | "notBool" | "outOfBounds" | "danglingReference" | null };
type LinkTarget = { table: string; key: ObjectKey | null; label: string | null; navigable: boolean };
type ReverseRelationView = { relation: string; table: string; tableDisplayName: string; via: string };
type RelationCount = { relation: string; count: number };
```

If a generated name differs from this excerpt, the generated file wins: use
it and say so.

### The design (brief excerpt)

**Visual system** (`DESIGN.md`; no new tokens):

- quiet, dense, flat; editable is the calm default, and read-only is the
  marked exception;
- tokens: `--surface`, `--surface-sunken`, `--border`, `--text`,
  `--text-muted`, `--warn`, `--accent` (focus only here), the spacing scale,
  `--font-size-sm/md/lg`, `--font-mono`;
- status hues always come with an icon and a word.

Shared pieces:

- `Button` (`quiet`);
- `Icon`: `arrow-right` (follow), `lock`, `alert-triangle` (problems),
  `chevron-down`;
- `.input` (U1; `[readonly]` gives `--surface-sunken`, no border, full-
  strength text) and `.checkbox`;
- `.num`, `.mono`, `.visually-hidden`.

**Sheet order:**

1. location bar, eyebrow, `h2`, tags (U3);
2. the read-only reason line;
3. **Fields**;
4. *(many-to-many sections, U9)*;
5. **Referenced by**.

**The read-only reason line,** under the `h2` in `--text-muted`, by the first
that applies:

1. the session's `readOnlyReason` → "The database is open read-only.";
2. `configFailClosed` → "The config has an error, so everything is
   read-only.";
3. the table's `mode` is `pickable` → "This is a lookup table: its objects
   can be picked, but not changed here.";
4. the table's `mode` is `read-only` → "This table is read-only in the
   config.".

`TableView.mode` is `"editable" | "read-only" | "pickable"`. A **pickable**
(lookup) table is never in the table list, but its objects open here when the
user follows a reference to one. Such an object is **entirely read-only**
(the backend already makes all its fields read-only): it has no editors, no
save bar, no Delete…, and its Referenced by lists are plain links. Find the
open object's `TableView` in `session.tables` by name; pickable tables are
included there.

**Fields: the property sheet.** The sheet pane is its own inline-size
container (`sheet`):

- **≥ 36rem:** two columns, the label column `11rem`, then the value column.
  Rows are separated by `--space-3`, with no row rules (it is a form, not a
  grid).
- **< 36rem:** the label above the value.

Each `FieldRow`:

- **The label:** `FieldView.displayName`, `--text`, weight 600. It is a
  `<label htmlFor>` for inputs, or an `id` referenced by `aria-labelledby` for
  grouped values.
- **Tags** after the label (`--font-size-sm`, `--text-muted`, 1 px
  `--border`, `--radius`, padding `0 var(--space-1)`), each a word, with an
  icon only where stated:
  - **Read-only** with the `lock` icon, when the table is editable and the
    field's mode is `read-only`. A read-only or pickable table already
    carries the tag in the header, so its fields do not repeat it.
  - **Set when created**, when the mode is `create-only` on an existing
    object.
  - **Key**, when `isKey`.
- **The value**, by kind. In this task **every field shows its read
  representation**; U5 replaces it with an editor where the field is
  editable.

  | Kind / case | Read representation |
  | --- | --- |
  | integer, real | `<input className="input num" readOnly value=…>` |
  | text | `<input className="input" readOnly>`, or `<textarea className="input" readOnly>` when `multiline` (rows fit the content up to 12, then it scrolls) |
  | integer/text with `rule.allowed` | the matching `AllowedValue.label` in a read-only input; when none matches, the raw value (and the problem line explains) |
  | bool | a `.checkbox` reflecting `checked`, with `aria-readonly="true"` and changes ignored, then the word "Yes", "No" or "Not set". If jsx-a11y rejects `aria-readonly` on a checkbox, instead render a check icon (`check`) or nothing in a 1rem bordered box plus the word, inside a focusable `<span tabIndex={0} role="img" aria-label="{field}: Yes">`, and say so in your report |
  | duration | the canonical text (`value.value`) in a read-only input |
  | FK (`foreignKey` set) | `ReferenceField` (below) |
  | blob | "{size} bytes" (`.num`) in a read-only input |
  | any | the value's text in a read-only input |
  | NULL (any kind except FK and bool) | an empty read-only input with the placeholder "Not set" |

  Read-only inputs are focusable and selectable, and are announced as read
  only. They never take the disabled look.
- **The hint line** (`--text-muted`, `--font-size-sm`, joined by " · "),
  from `fieldHints(field)` in `hints.ts`:
  - `help`;
  - integer and real bounds: "1 to 10000", "at least 0", "at most 99";
  - text lengths: "1 to 100 characters", "up to 200 characters", "at least
    1 character";
  - duration bounds as written ("1m to 100h") and the unit ("in seconds");
  - for read-only fields, the bounds still show, because they explain
    problem lines.

  The hint's `id` is in the field's `aria-describedby`.
- **The problem line,** when `FieldValue.problem` is set: the
  `alert-triangle` icon in `--warn`, then text in `--text` from
  `problemMessage(fieldValue, field)` in `errorMessages.ts`. It is in the
  field's `aria-describedby`.

  | Problem | Copy |
  | --- | --- |
  | `wrongType` | The stored value is not {a whole number \| a number \| text \| a yes/no value \| a duration}: {raw}. |
  | `notBool` | Stored value is {raw}, not 0 or 1. Tick or clear the box to replace it. |
  | `outOfBounds` | The stored value is outside this field's limits. |
  | `danglingReference` | Refers to a missing object ({raw}). |

  `{raw}` is the stored value's text (`value.value`; blob as "{size}
  bytes"). The first problem line on a sheet also ends "Other fields can
  still be saved."

**`ReferenceField`** (FK fields, read form):

- `link` with `navigable` and a `key`: a button styled as a link. It shows
  the target's `label` with an `arrow-right` icon after it, in `--text`
  with an underline (not colour alone). Its accessible name is "Open {label}
  in {tableDisplayName}".
  - Activating it calls `navigate({ push: { kind: "object", table:
    link.table, key: link.key, label: link.label } })`.
  - Focus then goes to the new sheet's `h2` (U3's rule).
- `link` with `navigable` and `key: null` (dangling): the stored value in
  `.mono` and the words "Missing object", plus the `danglingReference`
  problem line.
- A link into a **pickable** table is an ordinary navigable link. Following
  it opens the lookup object read-only (above).
- `value` NULL (no `link`): "Not set" in `--text-muted`.
- The label is tied to the control by `aria-labelledby` (the label `id` plus
  the button's own text).

**Referenced by** (`ReferencedBy`), shown only when the table has
`referencedBy` entries:

- `h3` "Referenced by".
- For each `ReverseRelationView`, take the count from
  `ObjectView.referencedBy` (matching `relation`):
  - **count > 0:** a disclosure button, "{tableDisplayName} · via {via} ·
    {count}" (`.num` count), with `aria-expanded`, `aria-controls` and a
    chevron that rotates.
    - Opening it loads `list(rel.table, { search: null, offset: 0, limit: 25,
      referencing: { relation: rel.relation, key: current.key } })`.
    - The list shows each `ObjectSummary` as a link button like
      `ReferenceField`'s ("Open {label} in {tableDisplayName}").
    - "Show 25 more" (quiet) appends the next page while `items.length <
      total`.
    - Loading: "Loading…" in `--text-muted` with `aria-busy`.
    - Errors: an inline line with **Try again**.
  - **count 0:** plain text "{tableDisplayName} · via {via} · none".
- The disclosures are closed by default and reset when the object changes.
- The latest response wins per disclosure.

**Focus and keyboard.**

- Every link, read-only input, checkbox and disclosure is in the Tab order, in
  visual order.
- Following a link moves focus to the new sheet's `h2`; Back returns there
  (U3).

## Files

Create in `apps/desktop/src/tools/dbmanager/`:

- `FieldRow.tsx`, `ValueDisplay.tsx`, `ReferenceField.tsx`,
  `ReferencedBy.tsx`;
- `hints.ts`, `values.ts` (`displayValue(field, value)`, `rawText(value)`);
- tests: `hints.test.ts`, `values.test.ts`, `FieldRow.test.tsx`,
  `ReferenceField.test.tsx`, `ReferencedBy.test.tsx`,
  `ObjectSheet.read.test.tsx`.

Edit `ObjectSheet.tsx`, `errorMessages.ts` (add `problemMessage`),
`fixtures.ts` and `styles/dbmanager.css`.

**Fixture: the example book "Dune".**

| Field | Value or attribute |
| --- | --- |
| `title` | text, max 200 |
| `author_id` | "Author", FK → `author`, link "Herbert, Frank" |
| `isbn` | create-only, "ISBN", with help |
| `format` | allowed hardcover / paperback / ebook with labels; value "paperback" → "Paperback" |
| `publisher_id` | FK → Publishers (a pickable table), link "Acme Press" |
| `pages` | 1–10000 |
| `price` | real 0.0–1000.0 |
| `in_print` | bool, checked true |
| `audio_length` | duration, seconds, `"1m"`–`"100h"`, value "1h30m" |
| `summary` | multiline, max 4000 |
| `revision` | read-only |
| `id` | read-only, key |

Problem fixtures:

- `in_print` stored 2 (`notBool`, `checked: null`);
- `pages` stored "many" (`wrongType`);
- `pages` 0 (`outOfBounds`);
- a dangling `author_id` 99;
- a FK column into a hidden table, arriving as a plain read-only integer
  field (`foreignKey: null`, `link: null`).

Author "Herbert, Frank" has a reverse relation "Books · via Author · 2".

## Skill

- Run `/impeccable layout` on the sheet at a 38rem and a 28rem sheet width
  and at 200% text, then `/impeccable typeset` on the label, value and hint
  hierarchy. Run them from `apps/desktop` with `--target
  src/tools/dbmanager/ObjectSheet.tsx`.
- Read `reference/craft-floor.md` before the first UI edit.
- Extend the established world. **The anti-goal is a database viewer:**
  no column types as labels, no grid rules, no raw ids where a label
  exists.
- Run `/impeccable audit` and the detector before done.
- If the skill is missing, install it (`npx impeccable install`) or follow
  its references by hand, and say so.

## Acceptance criteria

**The book "Dune":**

- Fields appear in `TableView.fields` order, labelled by display name.
- `in_print` is a ticked checkbox with "Yes".
- Format reads "Paperback".
- Audiobook length reads "1h30m", with the hint "1m to 100h · in seconds".
- Revision and Id carry **Read-only**, and Id carries **Key**.
- ISBN carries **Set when created**.
- Summary is a read-only textarea.

**References and problems:**

- Author is a link "Herbert, Frank" with accessible name "Open Herbert,
  Frank in Authors". Activating it opens the author, and Back returns to
  Dune.
- Dangling and NULL references render as specified, with no link.
- The FK into a hidden table renders as a plain read-only integer, with no
  link, no "Missing object" and no reference styling.
- Each problem fixture shows its icon, the word-bearing copy and its raw
  value. The first one adds "Other fields can still be saved.".

**Read-only tables:**

- An object of a `read-only` table shows the reason line "This table is
  read-only in the config." and **no** per-field Read-only tags.
- **Pickable objects:**
  - Following the book's Publisher link opens "Acme Press" (Publishers,
    pickable);
  - it shows "This is a lookup table: its objects can be picked, but not
    changed here.", with no per-field tags;
  - Back returns to the book.

**Referenced by:**

- "Books · via Author · 2" opens to two links, loaded with the right
  `referencing` filter.
- A relation with 30 rows shows "Show 25 more", then all 30.
- A count of 0 reads "none" and is not a button.

**Layout:**

- The sheet is two-column at ≥ 36rem and stacked below that, checked in the
  CSS test.
- No horizontal scrolling at 800×560 or at 200% text.

**Accessibility:**

- Every value has an accessible name (its label) and its hint and problem in
  `aria-describedby`.
- Read-only values are focusable.
- Nothing is conveyed by colour alone: problems have an icon and words, and
  links have an underline and an icon.
- jsx-a11y lint passes.

## Tests proving completion

- `hints.test.ts`:
  - `integer and real bounds`;
  - `text lengths`;
  - `duration bounds and unit`;
  - `help first`.
- `values.test.ts`:
  - `display per kind`;
  - `allowed label`;
  - `null shows not set`;
  - `blob size`.
- `FieldRow.test.tsx`:
  - `tags by mode and key`;
  - `read-only table has no field tags`;
  - `problem lines`;
  - `describedby wiring`.
- `ReferenceField.test.tsx`:
  - `navigable link follows`;
  - `dangling`;
  - `fk into a hidden table is a plain value`;
  - `null`.
- `ReferencedBy.test.tsx`:
  - `lazy load with referencing filter`;
  - `show more`;
  - `count zero`;
  - `error and retry`.
- `ObjectSheet.read.test.tsx`:
  - `book Dune renders as a property sheet`;
  - `reason line precedence, including pickable`;
  - `publisher link opens the lookup object read-only`;
  - `follow and back`.

Run: `npm run typecheck && npm run lint && npm run test`.

## States covered

- **Loading:** sheet skeleton (U3), Referenced-by lists.
- **Empty:** NULL values, count-0 relations, a table with no reverse
  relations.
- **Error:** stored-value problems, list errors, dangling references.
- **Success:** every kind's read representation.

## Out of scope

- Editors and drafts (U5), saving (U6), editing references (U7), creating
  (U8), many-to-many (U9, U10), delete (U11).
