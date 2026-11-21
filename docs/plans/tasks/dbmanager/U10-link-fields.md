# U10 — Link fields on many-to-many links

Status: **superseded (2026-11-21):** the UI moves to GPUI; the ui-designer rewrites this task for `apps/filemanager` (its design decisions carry over). Do not run this plan. Previous status: awaiting approval
Project: dbmanager   Depends on: U9 (landed), M10 (landed), M11 (landed)

## Goal

Let the user set and edit the **link fields** of many-to-many links (the
junction's extra columns, such as a contribution's role and position):

- on a link being added;
- on an existing link;
- removing a link that carries data shows, before Save, that its values will
  be lost, and stays reversible.

## Context

**The product.** FileManager is a portable Windows desktop app (Tauri v2;
React + TypeScript + Vite in `apps/desktop/src/`). The **Database Manager**
(`dbmanager`) edits a SQLite file as an **object editor**. A many-to-many
relation shows as a checklist of every object on the other side, with the
linked ones ticked (U9). Some junctions have **link fields**, which the human
decided are **editable in v1**:

- ticking an object adds a link whose fields the user sets before saving.
  Required ones must be filled; defaults are prefilled;
- a linked entry shows its link fields and lets the user edit them, per
  field mode. `create-only` link fields are settable only when the link is
  added;
- unticking a link that has link fields discards those values. This must be
  visible before Save, and reversible until Save.

Link fields use **the same typed editors, bounds and inline validation** as
object fields.

Earlier tasks built:

- U4: `FieldRow`, tags, hints (`fieldHints`), `problemMessage`,
  `ValueDisplay`;
- U5: `FieldEditor`, `BoolField`, `draft.ts` (`EditorValue`,
  `initialEditorValue`, `toDbValue`, `isChanged`, `issuesFor`, `canEdit`),
  and `issueMessage`;
- U7: `ReferenceField` and `ObjectPicker`, for FK link fields;
- U6: `saveDraft`, `ErrorSummary`, server issues;
- U9:
  - `M2mChecklist` (rows, the pinned **Not saved yet** group, staging);
  - `linkDraft.ts` (`StagedLink` add / update / remove, with `fields:
    Record<column, EditorValue>`, and `values: FieldValue[]` on update and
    remove);
  - `toggle`, `buildLinkChanges`, `rebaseLinkChange`;
  - `draft.links`, keyed by `linkKey(relation, other)`.

  Today adds carry `fields: {}`, and updates are never created.

You own `apps/desktop/src/` **except `lib/`**: never edit it. Tools never
import each other.

**Contract rules.**

- `M2mView.linkFields: FieldView[]` are the visible link fields, in junction
  column order. Their `requiredOnCreate` means **required when adding a
  link**. Hidden link fields never reach the UI.
- `M2mOption.linkValues: FieldValue[] | null` (linked options only, in
  `linkFields` order) are read exactly like object fields: values, problems,
  links, and canonical durations.
- `linkVersion` is opaque: echo it back in `update` and `remove`.
- On a read-only side, link fields are read-only too.

### The API used

`lib/dbmanagerValidation.ts` (M11):

- `validateField(rule, input)`, through U5's `issuesFor`;
- `missingRequiredOnCreate(fields: FieldView[], supplied: ReadonlySet<string>):
  string[]`, which works for `linkFields` when adding a link.

```ts
type LinkAdd = { other: ObjectKey; fields?: FieldChange[] };      // supplied link fields only
type LinkUpdate = { other: ObjectKey; version: string; fields: FieldChange[] };  // changed ones only
type FieldChange = { column: string; input: { type: "value"; value: DbValue } | { type: "reference"; key: ObjectKey | null } };
type LinkError = { relation: string; other: ObjectKey;
  fields: FieldError[];                   // issues on link fields: { column, issues }
  issues: FieldIssue[] };                 // issues on the link itself (U9 shows these)
```

Backend rules on link fields:

- **On add:** `editable` and `create-only` are accepted; read-only gives
  `notEditable`; a missing required field gives `required`.
- **On update:** only `editable` is accepted.

### Changes to `linkDraft.ts` and `draft.ts`

- **`canEditLinkField(field, phase: "add" | "update", sideEditable)`:**
  `sideEditable` and (`editable`, or `create-only` when `phase === "add"`).
  Kinds `blob` / `any` are never editable. FK link fields (`foreignKey` set) are editable
  when the target is in `session.tables` (U7's rule). A link field whose FK
  points into a hidden table arrives as a plain read-only field.
- **Staging an add** (`toggle` on an unlinked option): `fields` starts from
  each editable field's `defaultValue`, through `initialEditorValue` with a
  synthetic `FieldValue`. Fields with no default start empty. Prefilled
  defaults the user does not change are **not** sent (the database applies
  them).
- **Editing an existing link's field:** create or modify an `update` with
  `version: option.linkVersion`, `values: option.linkValues`, and the edited
  `fields`. If every edit is reverted, unstage the update.
- **`buildLinkChanges`:**
  - `add.fields`: the fields changed from their initial (default) editor
    value;
  - `update.fields`: the fields changed from `values` (`isChanged`);
  - `FieldChange` inputs follow U5/U7: `{ type: "value", value:
    toDbValue(...) }`, or `{ type: "reference", key }`.
- **Validation:**
  - `linkIssues(view, staged)` returns per-column issues: `issuesFor` on each
    changed field, plus, for an add, a `required` issue for each column in
    `missingRequiredOnCreate(linkFields that canEditLinkField(add),
    supplied)`, where `supplied` = the columns with a non-null editor value
    (default included);
  - the save flow's client check (U6) includes these. Any issue blocks the
    save and is listed in the error summary.

### The design (brief excerpt)

**Visual system** (`DESIGN.md`; no new tokens). The same editors as object
fields (U1's `.input` and `.checkbox`), the same tags (**Required**,
**Edited**, **Set when linked**), the same issue and problem lines. Tokens:
`--surface`, `--surface-sunken`, `--border`, `--text`, `--text-muted`,
`--danger`, `--warn`, the spacing and type scales.

**`LinkFieldEditors`** (inside a checklist row, after the label):

- Shown on rows that are **linked** or **staged for add**. Unlinked rows and
  staged removes show none.
- A compact inline group: each field is a short visible label (`displayName`)
  and its control, laid out in a wrapping row (`flex-wrap`, gap `--space-2
  --space-3`).
  - When the sheet is narrower than 36rem, or at 200% text, the group wraps
    under the row's label, indented to align with the label text.
  - Inputs use the field's natural width: about 8rem for numbers and
    durations, 14rem for text and selects. They never cause horizontal
    scrolling.
- **Labels include the object.** The visible label is "Role". The
  accessible name is "Role for {object label}", from a visually hidden "
  for {label}" inside the `<label>`. Hints, issues and problems are wired by
  `aria-describedby` as in U5, and the hint line shows under the group only
  for the focused field (to stay dense), in `--text-muted`.
- **Per field:**
  - editable here: the editor (U5's kind table; FK link fields use
    `ReferenceField` with the picker);
  - create-only on an existing link: read-only, with **Set when linked**;
  - read-only, or a read-only side: U4's read form (`--surface-sunken`);
  - `requiredOnCreate` on an add: **Required** and `aria-required`;
  - a changed field: **Edited**. A row with any changed link field also
    shows **Edited** after its label.
- **Problems** on existing link values (`FieldValue.problem`) show U4's
  problem line, and never block anything.
- **Escape** in a changed link field reverts it.

**Removing a link that carries data** (a staged remove where
`view.linkFields.length > 0` and `values` holds any non-null value):

- the row keeps the struck-through label and the **Will be removed** tag;
- in place of the editors, a line: "Will be removed with its fields: Role
  translator, Position 1." (each value through `displayValue`; null fields
  are omitted);
- **Undo** (quiet, `undo` icon, "Undo removing {label}") restores the link
  with its values and version. Ticking the box again does the same.

Pinned rows (**Not saved yet**, U9) render the same editors or the same
line.

**Save errors** (`ValidationFailed.linkErrors[].fields`):

- each `{ column, issues }` shows on that link field's editor, as a server
  issue (it replaces the inline issues until edited, as U6 does), with
  `aria-invalid`;
- the error summary item reads "{heading}: {label}: {field displayName}:
  {issueMessage}". Its button focuses that control. The row is always
  rendered, in place or pinned (U9).

**Conflicts.** U9's `rebaseLinkChange` now carries fields: re-applying an
add that is now linked stages an update with the user's field values against
the fresh version. Its notice is U9's.

## Files

Create in `apps/desktop/src/tools/dbmanager/`:

- `LinkFieldEditors.tsx`;
- tests: `LinkFieldEditors.test.tsx`, `linkFields.staging.test.tsx`,
  `linkFields.save.test.tsx`.

Edit `M2mChecklist.tsx`, `linkDraft.ts` and `linkDraft.test.ts`, `store.ts`
(the client check, error mapping), `ErrorSummary.tsx` (item labels),
`fixtures.ts` and `styles/dbmanager.css`.

Fixtures from the example:

- `m2m:contribution` on the book "Dune": heading "Contributors", other
  `author`;
- link fields:
  - `role`: text with allowed editor / translator / illustrator, labelled
    "Editor", "Translator", "Illustrator"; `NOT NULL DEFAULT 'editor'`, so
    `hasDefault`, `defaultValue` "editor", not `requiredOnCreate`;
  - `position`: integer 1–99, nullable;
- one existing contribution (translator, position 1);
- from the author side, the heading "Contributions";
- test variants:
  - `position` create-only;
  - a required link field with no default;
  - a duration link field (unit minutes, `"1m"`–`"10h"`);
  - an FK link field;
  - a read-only relation.

## Skill

- Run `/impeccable layout` on a checklist row with link fields at 38rem and
  28rem sheet widths and at 200% text. Then run `/impeccable harden` on
  add, edit, remove and undo combinations and on errors on pinned rows. Run
  both from `apps/desktop` with `--target
  src/tools/dbmanager/LinkFieldEditors.tsx`.
- Read `reference/craft-floor.md` before the first UI edit.
- Extend the established world. Link fields must read as part of the link,
  not as a grid.
- Run `/impeccable audit` and the detector before done.
- If the skill is missing, install it (`npx impeccable install`) or follow
  its references by hand, and say so.

## Acceptance criteria

**Adding a link:**

- Ticking "Le Guin, Ursula" in Contributors shows Role prefilled "Editor"
  and an empty Position.
- The controls are named "Role for Le Guin, Ursula" and "Position for Le
  Guin, Ursula".
- Choosing Translator and typing "1", then Save, sends `add: [{ other,
  fields: [{ column: "role", input: { type: "value", value: { type: "text",
  value: "translator" } } }, { column: "position", input: { type: "value",
  value: { type: "integer", value: "1" } } }] }]`.
- Leaving Role at Editor sends no `role` change.
- Position "0" shows "Must be at least 1." after blur and blocks the save,
  with a summary item "Contributors: Le Guin, Ursula: Position: Must be at
  least 1.".
- With a required no-default link field, adding without it blocks the save
  with "Enter a value.".

**Editing an existing link:**

- Changing Position 1 → 2 stages an update with its `linkVersion`. The row
  shows **Edited**, and the bar counts one change.
- Reverting it with Escape unstages it.
- Save sends `update: [{ other, version, fields: [position] }]`.
- With `position` create-only, the existing link shows Position read-only
  with **Set when linked**, while a new add can set it.

**Removing a link with data:**

- Unticking the translator link shows "Will be removed with its fields:
  Role translator, Position 1." with Undo.
- Undo restores the values and the version exactly.
- Save sends `remove: [{ other, version }]`.

**Errors and conflicts:**

- `linkErrors` with `position: aboveMax` show on that field, wherever the
  row renders, and in the summary.
- Re-applying an add after a conflict where the link now exists stages an
  update carrying the user's field values.

**Other link-field kinds:**

- A duration link field behaves like an object duration field: canonical
  form on blur, `durationPrecision` copy.
- An FK link field picks through the picker.

**Read-only:**

- On a read-only relation, link values show read-only. Nothing is editable or
  staged.

**Accessibility and layout:**

- Every link-field control is named with both the field and the object.
- Everything is keyboard reachable in visual order.
- jsx-a11y lint passes. There is no horizontal scrolling at 800×560 or at
  200% text.

## Tests proving completion

- `linkDraft.test.ts`:
  - `add starts from defaults`;
  - `update created and removed by edits`;
  - `buildLinkChanges sends only changed link fields`;
  - `linkIssues required and bounds`;
  - `rebase add to update keeps fields`.
- `LinkFieldEditors.test.tsx`:
  - `labels include the object`;
  - `modes: editable, create-only on add vs existing, read-only side`;
  - `required tag on add`;
  - `problem lines`;
  - `escape reverts`;
  - `duration and reference link fields`.
- `linkFields.staging.test.tsx`:
  - `tick shows editors with defaults`;
  - `edit existing stages update`;
  - `untick with data shows loss line and undo restores`;
  - `pinned rows render editors`.
- `linkFields.save.test.tsx`:
  - `add request fields`;
  - `update request`;
  - `remove request`;
  - `client issues block with summary`;
  - `server link field errors on row and summary`.

Run: `npm run typecheck && npm run lint && npm run test`.

## States covered

- **Link fields:** default-prefilled, empty required, edited, invalid
  (client and server), read-only, create-only (add vs existing), problem.
- **Removal:** with data (loss line, undo) and without.
- **Loading, empty and error:** U9's.

## Out of scope

- Self-referential relations (not in v1).
- Editing link fields of a link that is not on any loaded page and not
  staged. Use search to reach it.
