# U5 — Typed field editors, the draft, and inline validation

Status: awaiting approval (revised 2026-11-16: pickable tables)
Project: dbmanager   Depends on: U4 (landed), M10 (landed), M11 (landed)

## Goal

Make editable fields editable:

- a typed editor per field kind, with booleans as checkboxes and durations as
  text;
- a draft of the user's changes, with each changed field marked;
- inline validation from the TypeScript mirror.

Saving comes in U6.

## Context

**The product.** FileManager is a portable Windows desktop app (Tauri v2;
React + TypeScript + Vite in `apps/desktop/src/`). The **Database Manager**
(`dbmanager`) edits a SQLite file as an **object editor, not a grid**.
**Anything the config does not mention is editable**, so the editable state
must be calm (ordinary inputs), and read-only must be the clearly marked
exception (U4 built that: `--surface-sunken` values with a **Read-only** tag).

Earlier tasks built:

- U2: the store (`useDbmanager()`, `resetDbmanagerStore()`), the session,
  and `errorMessages.ts`;
- U3: places, history, `navigate`, and the sheet shell with `current:
  ObjectView`;
- U4: `FieldRow` (label, tags, hint line `fieldHints`, problem line
  `problemMessage`), `ValueDisplay`, `ReferenceField`, `ReferencedBy` and
  `values.ts`.

This task replaces `ValueDisplay` with an **editor** where the field can be
edited, and keeps U4's read representation everywhere else.

You own `apps/desktop/src/` **except `lib/`**: never edit it. Tools never
import each other.

**Contract rules.**

- The UI does no kind, unit or bound logic of its own: the rule data is
  `FieldView.rule`, and the logic is M11's mirror.
- Integers, reals and durations are **strings** (`DbValue`); never convert
  them to JS numbers.
- Durations are sent as the user's text in `{ type: "duration", value }`.
  On read they arrive in canonical form ("1h30m").
- Saving validates only changed fields, so legacy problems in untouched
  fields never block anything.
- The save result is authoritative (U6). Inline validation is feedback.

### The API used

`lib/dbmanager.ts` (M10):

- `boolValue(checked: boolean): DbValue` returns integer `"1"` / `"0"`;
- `durationValue(text: string): DbValue` returns `{ type: "duration",
  value: text }`.

`lib/dbmanagerValidation.ts` (M11):

```ts
type ValidationResult = { ok: true; value: DbValue } | { ok: false; issues: FieldIssue[] };
function validateField(rule: FieldRule, input: DbValue): ValidationResult;  // mirrors Rust validate
function canonicalDuration(text: string, unit: DurationUnit): string | null;
```

The validation rules, restated (Rust is authoritative):

- null is ok only if `rule.nullable` (else `required`);
- integer `^[+-]?[0-9]+$` in i64, then `belowMin`/`aboveMax`/`notAllowed`;
- real (finite), then the bounds;
- bool is integer 0/1 (`notBool`);
- duration has its grammar (`notDuration`), whole units
  (`durationPrecision`, with `limit` = the unit name), then the bounds;
- text: `emptyIsNull && nullable && ""` → null; else `tooShort`, `tooLong`
  (code points), `patternMismatch`, `notAllowed`.

### Types (generated, excerpt)

```ts
type FieldView = { column; displayName; help: string | null;
  mode: "editable" | "read-only" | "create-only"; kind: "integer" | "real" | "text" | "bool" | "duration" | "blob" | "any";
  rule: FieldRule; isKey: boolean; requiredOnCreate: boolean; hasDefault: boolean;
  defaultValue: DbValue | null; multiline: boolean; foreignKey: ForeignKeyView | null };
type FieldRule = { kind; nullable: boolean; unit: DurationUnit | null; min: string | null; max: string | null;
  minLength: number | null; maxLength: number | null; pattern: string | null;
  allowed: { value: DbValue; label: string }[] | null; emptyIsNull: boolean };
type FieldValue = { column; value: DbValue; checked: boolean | null; link: LinkTarget | null;
  problem: "wrongType" | "notBool" | "outOfBounds" | "danglingReference" | null };
type FieldIssue = { code: FieldIssueCode; message: string; limit: string | null };
type FieldIssueCode = "required" | "wrongType" | "notInteger" | "notReal" | "notBool" | "notDuration"
  | "durationPrecision" | "belowMin" | "aboveMax" | "tooShort" | "tooLong" | "patternMismatch"
  | "notAllowed" | "unique" | "referenceMissing" | "notEditable" | "unknownField";
```

`TableView.mode` is `"editable" | "read-only"`. The session (U2's store) has
`readOnlyReason` and `configFailClosed`.

### When a field is editable here

`canEdit(field, phase, table, session)` in `draft.ts` is true when all of
these hold:

- the session is writable: no `readOnlyReason` and no `configFailClosed`;
- `table.mode === "editable"`. This excludes `read-only` and `pickable`
  (lookup) tables, whose objects open fully read-only;
- the field's mode is `editable`, or `create-only` in the `create` phase
  (U8 uses the create phase);
- `field.foreignKey === null`: references are edited in U7;
- the kind is not `blob` or `any`.

Otherwise U4's read representation stays.

### The draft (`draft.ts`, pure, unit-tested)

```ts
type EditorValue =
  | { type: "text"; text: string }              // integer, real, text, duration editors
  | { type: "bool"; checked: boolean | null }    // null = not set
  | { type: "choice"; value: DbValue | null }    // allowed values; null = not set
  | { type: "null" };                            // explicitly cleared text (see below)
function initialEditorValue(field: FieldView, value: FieldValue | null): EditorValue;
function toDbValue(field: FieldView, ev: EditorValue): DbValue;
function isChanged(field: FieldView, base: FieldValue | null, ev: EditorValue): boolean;
function issuesFor(field: FieldView, ev: EditorValue): FieldIssue[];   // validateField on toDbValue
```

- **`initialEditorValue`:**
  - text-like kinds take the value's text, or `""` for NULL. A stored value
    of the wrong type is shown raw (`value.value`; blob as "");
  - bool takes `checked` (null for NULL or a problem);
  - `rule.allowed` takes `{ type: "choice", value }`.
- **`toDbValue`:**
  - integer, real and duration are **trimmed**; `""` → `{ type: "null" }`;
    otherwise `{ type: "integer" | "real", value }` or `durationValue(text)`;
  - text is **not trimmed**: `{ type: "text", value: text }` (the validator
    applies `emptyIsNull`), and `{ type: "null" }` for the explicit cleared
    state;
  - bool: `null` → `{ type: "null" }`, otherwise `boolValue(checked)`;
  - choice: its `value`, or `{ type: "null" }`.
- **`isChanged`:** normalise the edit (`validateField` ok → its `value`,
  otherwise the raw `toDbValue`), then compare with the stored `value` by type
  and value. Typing "1h 30m" over a stored "1h30m" is **not** a change, and
  neither is "007" over a stored "7".

The store's `draft`:

```ts
{ table: string; key: ObjectKey | null;
  edits: Record<column, EditorValue>;   // only fields whose editor value differs from initial
  touched: Record<column, true>;        // blurred at least once
  showAllIssues: boolean;               // set by U6 on a Save attempt
  serverIssues: Record<column, FieldIssue[]> }  // set by U6; cleared per field on edit
```

The draft belongs to the current object (`objectId`). The store exposes:

- `editField(column, ev)`, `revertField(column)`, `touchField(column)`;
- `discardDraft()`;
- `dirtyCount()`: the number of changed fields; U9 adds staged links.

U6 guards navigation; until then, navigating away discards the draft.

### The design (brief excerpt)

**Visual system** (`DESIGN.md`; no new tokens):

- editors use U1's `.input` (`--surface`, 1 px `--border`, `--radius`,
  `aria-invalid` → `--danger` border) and `.checkbox` (`accent-color:
  --accent`);
- inline issues: the `x-circle` icon and text in `--danger`, at
  `--font-size-sm`;
- problems: `alert-triangle` in `--warn` (U4);
- tags: U4's tag style;
- the **Edited** tag is a word, never colour alone.

**Editors by kind** (`FieldEditor.tsx`):

| Kind / rule | Editor |
| --- | --- |
| integer | `<input className="input num" type="text" inputMode="numeric" spellCheck={false}>` |
| real | the same with `inputMode="decimal"` |
| text | `<input className="input">`, or `<textarea className="input">` when `multiline`; with `maxLength`, a counter "12 / 200" (code points, `[...text].length`, `.num`, `--text-muted`) under it, not announced on each keystroke |
| `rule.allowed` (integer or text) | `<select className="input">` of the allowed labels; a nullable field gets a first option "Not set"; when the stored value matches no allowed value, an extra selected option "Stored: {raw} (not allowed)" that disappears once another option is chosen |
| bool | `BoolField` (below) |
| duration | `<input className="input" spellCheck={false}>`; on blur, if `canonicalDuration(text, rule.unit)` is non-null, the editor text becomes the canonical form |

- **Nullable text with `emptyIsNull: false`**: an empty input means the empty
  string. A quiet **Clear** button (accessible name "Clear {displayName}")
  sets `{ type: "null" }`; the input then shows the placeholder "Not set" and
  typing leaves the null state.
- **Other nullable fields:** emptying the input means not set; the hint adds
  "Leave empty for not set".
- **`aria-required="true"`** when `!rule.nullable` (and the kind is not
  bool).
- **`aria-describedby`** lists the hint, then the issues, then the problem.

**`BoolField`:**

- **Non-nullable:** a `.checkbox` labelled by the field, ticked or clear.
- **Nullable and null:** the checkbox is `indeterminate`, with the word **Not
  set** beside it, tied by `aria-describedby`. Click or Space ticks it.
- **Nullable and not null:** a quiet **Clear** button ("Clear {displayName}")
  returns it to not set.
- **`notBool`** (stored 2): indeterminate. U4's problem line stays until the
  user ticks or clears it.

**Changed state.**

- A changed field gets the **Edited** tag after its label.
- **Escape** in a changed field reverts that field, prevents the default and
  stops propagation, so no view shortcut also fires.
- The view announces "{displayName} reverted."

**Inline validation.**

- `issuesFor` runs on every change. Issues are **shown** once the field has
  been blurred (`touched`), or when `showAllIssues` is set. While a shown
  field is being edited, its issues update live.
- **Server issues** (U6) for a field are shown in place of the inline ones
  until that field is edited again.
- Issues render as a list under the field (`id` in `aria-describedby`), and
  the input gets `aria-invalid="true"`.
- Copy comes from `issueMessage(issue, field)` in `errorMessages.ts`.
  `{limit}` is `issue.limit`; `{unit}` is `rule.unit`.

  | Code | Copy |
  | --- | --- |
  | `required` | Enter a value. |
  | `wrongType` | This kind of value can't be entered here. |
  | `notInteger` | Enter a whole number, like 42. |
  | `notReal` | Enter a number, like 3.5. |
  | `notBool` | Tick or clear the box. |
  | `notDuration` | Enter a duration like 1h30m or 1h 30m, or a number of {unit}. |
  | `durationPrecision` | Must be whole {limit}. (e.g. "Must be whole minutes.") |
  | `belowMin` | Must be at least {limit}. |
  | `aboveMax` | Must be at most {limit}. |
  | `tooShort` | Must be at least {limit} characters. (limit "1": "Can't be empty.") |
  | `tooLong` | Must be at most {limit} characters. |
  | `patternMismatch` | Doesn't match the expected format. (+ ": {help}" when the field has help), then the pattern on its own line in `.mono`: {limit} |
  | `notAllowed` | Choose one of the listed values. |
  | `unique` | Another object already has this value. |
  | `referenceMissing` | The chosen object no longer exists. Choose another. |
  | `notEditable` | This field can't be changed. |
  | `unknownField` | This field is no longer available. Reload to continue. |

**Hints added for editors:**

- duration: "Like 1h30m or 1h 30m. A plain number counts {unit}.";
- "Leave empty for not set" where empty means null.

## Files

Create in `apps/desktop/src/tools/dbmanager/`:

- `draft.ts`, `FieldEditor.tsx`, `BoolField.tsx`;
- tests: `draft.test.ts`, `FieldEditor.test.tsx`, `BoolField.test.tsx`,
  `ObjectSheet.edit.test.tsx`.

Edit `FieldRow.tsx`, `ObjectSheet.tsx`, `store.ts`, `hints.ts`,
`errorMessages.ts` (add `issueMessage`), `fixtures.ts` and
`styles/dbmanager.css`.

Fixtures (from the example database's book):

- `title`: text, max 200;
- `isbn`: create-only, pattern `97[89][0-9]{10}`, with help. Fixture
  patterns must stay in the Rust/JS-common subset: no `\d`, `\w`, `\s`,
  no `(?…)` groups, no `\A`, `\z` or `\p`, no backreferences, nested or
  POSIX classes, or set operations. A config pattern outside that subset
  makes its field read-only with an error diagnostic, so it never reaches an
  editor;
- `format`: allowed hardcover / paperback / ebook;
- `pages`: integer 1–10000, nullable;
- `price`: real 0–1000;
- `in_print`: bool, not null;
- `audio_length`: duration, seconds, `"1m"`–`"100h"`, nullable;
- `summary`: multiline, max 4000;

plus a nullable bool (`loan.returned` made nullable for the test) and a
nullable text field with `emptyIsNull: false`.

## Skill

- Run `/impeccable polish` on the field rows once the editors work: the
  focus, invalid, read-only and Edited states, side by side in light and
  dark. Run it from `apps/desktop` with `--target
  src/tools/dbmanager/FieldEditor.tsx`.
- Then run `/impeccable clarify` on the issue copy table above. Keep its
  meaning; report any change you propose rather than changing it silently.
- Read `reference/craft-floor.md` before the first UI edit.
- Extend the established world.
- Run `/impeccable audit` and the detector before done.
- If the skill is missing, install it (`npx impeccable install`) or follow
  its references by hand, and say so.

## Acceptance criteria

**Which fields are editors:**

- In an editable table with a writable session, Title, Pages, Price, In
  print, Format, Audiobook length and Summary are editors.
- ISBN (create-only), Revision and Id keep U4's read form.
- Author and Publisher (FKs) keep U4's form.
- An object of a read-only table, and a Publishers object (a pickable lookup
  table), have no editors.
- With `readOnlyReason` or `configFailClosed` set, no field anywhere is an
  editor.

**`draft.ts`:**

- `isChanged` is false for "1h 30m" over "1h30m", "007" over "7", and an
  unchanged checkbox.
- `isChanged` is true for "1h31m" and for clearing a nullable integer.
- `toDbValue` trims numbers and durations but not text.

**Editor behaviour:**

- Typing "0" in Pages, then blurring, shows "Must be at least 1.", with
  `aria-invalid` and the issue in `aria-describedby`.
- An issue is not shown before the first blur; after it, the issue updates
  live.
- "90ms" in Audiobook length shows "Must be whole seconds."
- "30s" shows "Must be at least 1m."
- "1h 30m" becomes "1h30m" on blur, with no Edited tag when the stored value
  was 1h30m.
- An ISBN-like pattern failure shows the copy and the pattern in mono.
- Title over 200 characters shows the counter "201 / 200" and "Must be at
  most 200 characters.".
- Format is a select of the labels. A stored "audiobook" shows "Stored:
  audiobook (not allowed)" selected.

**Booleans:**

- The nullable bool's null state is indeterminate with "Not set"; Space ticks
  it; Clear returns it to not set.
- The stored-2 (`notBool`) case is indeterminate with U4's problem line.

**Changed state:**

- A changed field shows **Edited**, and `dirtyCount()` counts it.
- Escape reverts it and announces "{field} reverted."
- The legacy `pages = "many"` problem does not mark the field changed or
  invalid until it is edited.

**Accessibility:**

- Every editor has its label, `aria-required` where the field is not
  nullable, and its hint.
- Everything is keyboard operable.
- jsx-a11y lint passes.
- At 200% text, editors fill the value column without horizontal scrolling.

## Tests proving completion

- `draft.test.ts`:
  - `initial values per kind`;
  - `toDbValue per kind`;
  - `isChanged normalises durations and integers`;
  - `explicit null text`;
  - `issuesFor uses validateField`;
  - `canEdit rules`.
- `FieldEditor.test.tsx`:
  - `integer issues after blur`;
  - `duration canonical on blur`;
  - `duration precision and min copy`;
  - `pattern copy with mono pattern`;
  - `text counter`;
  - `select with stored invalid option`;
  - `clear to not set`;
  - `escape reverts`;
  - `server issues replace inline until edited`.
- `BoolField.test.tsx`:
  - `plain checkbox`;
  - `nullable not set is indeterminate with words`;
  - `clear returns to not set`;
  - `notBool shows indeterminate`.
- `ObjectSheet.edit.test.tsx`:
  - `editors only where editable`;
  - `read-only session has no editors`;
  - `edited tags and dirty count`;
  - `legacy problem not dirty`.

Run: `npm run typecheck && npm run lint && npm run test`.

## States covered

- **Editors:** default, focus, changed (Edited), invalid (inline), invalid
  (server, prepared for U6), not set, a stored problem, read-only fallback.
- **Loading, empty, error:** inherited from U3/U4.

## Out of scope

- Saving, the save bar, Discard changes, the leave guard, conflicts (U6).
- Editing references (U7). Create-phase editors (U8 uses `canEdit`'s create
  phase). Link fields (U10).
