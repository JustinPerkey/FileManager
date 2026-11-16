# U9 — Many-to-many checklists

Status: awaiting approval
Project: dbmanager   Depends on: U8 (landed), M10 (landed)

## Goal

Show each many-to-many relation on an object as a checklist of **every**
object on the other side:

- searchable and paged, with the linked ones ticked and a "linked only"
  filter;
- ticks staged until Save, then saved in the same transaction as the field
  changes;
- read-only relations shown read-only;
- link conflicts recovered without losing the user's other edits.

## Context

**The product.** FileManager is a portable Windows desktop app (Tauri v2;
React + TypeScript + Vite in `apps/desktop/src/`). The **Database Manager**
(`dbmanager`) edits a SQLite file as an **object editor**. A many-to-many
relation (for example, a book's genres through a `book_genre` junction table)
shows on the object as a **checklist of all objects of the other table, with
the linked ones ticked** (the human's explicit choice).

- Ticking adds a link, and unticking removes it.
- Nothing is written until **Save**, which commits the field changes and the
  link changes in **one transaction**.
- Unsaved changes are visible, and reversible until Save.

Some junctions carry **link fields** (extra columns, such as a contribution's
role and position). U10 adds their editors. This task builds the checklist and
the add/remove staging, and leaves a seam for link fields.

Earlier tasks built:

- U2: the store, errors, and banners (`notices`);
- U3: `navigate` (guarded by U6), `objectId`, the list paging and search
  patterns;
- U4: the sheet's section order, Fields then *(many-to-many)* then
  Referenced by, and `ReferencedBy`'s disclosure and list patterns;
- U5: `draft.ts`, `EditorValue`, `dirtyCount()`;
- U6: `saveDraft` (it sends `links: []` today), `ErrorSummary`, the conflict
  panel, `rebaseDraft`, the leave guard, the save bar (shown when something is
  editable);
- U8: the create form (`key: null`).

You own `apps/desktop/src/` **except `lib/`**: never edit it. Tools never
import each other.

**Contract rules.**

- Relation ids, object keys and link versions are **opaque**. Echo them back
  verbatim.
- Key staged link edits by `linkKey(relation, other)` from `lib/dbmanager.ts`.
- **Strict links:**
  - adding an existing link, removing a missing one, or touching a changed
    one is a `Conflict` naming the link (`conflictLink`);
  - the UI reloads that link's state and keeps the user's other edits.

### The API used

`lib/dbmanager.ts`:

- `m2mOptions(table: string, key: ObjectKey | null, relation: string, query:
  M2mQuery): Promise<M2mPage>`. `key: null` is the create form: nothing is
  linked, and `linkedOnly` yields an empty page.
- `save(request)`, now with `links`.
- `linkKey(relation, other): string`.

```ts
type M2mView = { relation: string; otherTable: string; heading: string;
  mode: "editable" | "read-only";            // this side
  linkFields: FieldView[] };                 // U10
type M2mQuery = { search: string | null; offset: number; limit: number; linkedOnly: boolean };
type M2mPage = { items: M2mOption[]; total: number; linkedTotal: number; offset: number };
type M2mOption = { key: ObjectKey; label: string; linked: boolean;
  linkValues: FieldValue[] | null;           // linked only ([] for no link fields)
  linkVersion: string | null };              // linked only
type ObjectView = { …; manyToMany: RelationCount[] };   // one per M2mView: { relation, count } = linked count
type LinkChange = { relation: string; add?: LinkAdd[]; update?: LinkUpdate[]; remove?: LinkRemove[] };
type LinkAdd = { other: ObjectKey; fields?: FieldChange[] };
type LinkUpdate = { other: ObjectKey; version: string; fields: FieldChange[] };
type LinkRemove = { other: ObjectKey; version: string | null };
type LinkRef = { relation: string; other: ObjectKey };
type LinkError = { relation: string; other: ObjectKey; fields: FieldError[]; issues: FieldIssue[] };
```

`TableView.manyToMany: M2mView[]` gives the relations of the open table; the
other table's `TableView` (for `searchable` and `displayName`) is
`session.tables.find(t => t.name === view.otherTable)`. If the generated
optional/required shapes differ (for example, a required `add: []`), the
generated file wins.

**Save errors** for links:

- `ValidationFailed.linkErrors`: link-level `issues`, for example
  `notAllowed` "listed more than once" or `referenceMissing`. The `fields`
  part is U10's.
- `Conflict` with `conflictLink` and `current`.

### Staged links (`linkDraft.ts`, pure, unit-tested)

```ts
type StagedLink =
  | { op: "add"; relation: string; other: ObjectKey; label: string; fields: Record<string, EditorValue> }
  | { op: "update"; relation: string; other: ObjectKey; label: string; version: string;
      values: FieldValue[]; fields: Record<string, EditorValue> }           // U10 creates these
  | { op: "remove"; relation: string; other: ObjectKey; label: string;
      version: string | null; values: FieldValue[] };
function toggle(staged: StagedLink | undefined, option: M2mOption, relation: string): StagedLink | undefined;
function isTicked(option: M2mOption, staged: StagedLink | undefined): boolean;   // linked XOR (add) / not remove
function buildLinkChanges(staged: StagedLink[]): LinkChange[];                     // grouped by relation
function rebaseLinkChange(staged: StagedLink, fresh: M2mOption | null):
  { next: StagedLink | undefined; note: "reapplied" | "alreadyLinked" | "alreadyGone" };
```

**`toggle`:**

| Option | Staged | Result |
| --- | --- | --- |
| unlinked | none | stage `add`, with `fields` from U10's defaults (`{}` for now) |
| any | `add` | unstage |
| linked | none or `update` | stage `remove` with `version: linkVersion` and `values: linkValues` (an update's edits are dropped) |
| any | `remove` | unstage, restoring exactly the original link |

**`buildLinkChanges`:** adds become `{ other, fields }` (U10 fills `fields`);
updates become `{ other, version, fields }`; removes become `{ other, version
}`.

**`rebaseLinkChange`** after a link conflict, where `fresh` is the link's
current option (`null` = the other object is gone):

| Staged | Fresh | Next | Note |
| --- | --- | --- | --- |
| add | linked | `update` with the user's fields against `fresh.linkVersion`; `undefined` when the relation has no link fields | reapplied / alreadyLinked |
| update | not linked | `add` with the user's fields | reapplied |
| update | linked | `update` against the fresh version | reapplied |
| remove | not linked | `undefined` | alreadyGone |
| remove | linked | `remove` with the fresh version | reapplied |
| any | `null` | `undefined` | alreadyGone |

The draft gains `links: Record<linkKey, StagedLink>`. `dirtyCount()` adds
`Object.keys(links).length`. `discardDraft`, the leave guard and
`rebaseDraft` treat staged links as part of the draft:

- `rebaseDraft` keeps staged links whose relation still exists and is still
  editable;
- it drops the others, listed by "{heading}: {label}".

### The design (brief excerpt)

**Visual system** (`DESIGN.md`; no new tokens):

- U1's `.checkbox` (`accent-color: --accent`);
- tags in U4's style: **Will be added**, **Will be removed**, **Linked**,
  **Read-only** (lock icon);
- the struck-through label (`text-decoration: line-through`) plus the word
  for removals;
- a 1 px `--border` rule between rows; row hover `--surface-sunken`;
- tokens: `--surface`, `--surface-sunken`, `--border`, `--text`,
  `--text-muted`, `--danger`, `--warn`, the spacing and type scales.

**Section** (`M2mChecklist`), one per `M2mView`, in order, after Fields and
before Referenced by:

- **Header:**
  - `h3` = `heading`;
  - the meta line (`.num`): "{n} linked", plus " · {a} to add" and " · {r} to
    remove" when staged. `n` comes from `ObjectView.manyToMany`'s count for
    the relation; it is 0 on create;
  - a **Read-only** tag when the side is read-only (below).
- **Toolbar:**
  - search (an `.input`, `type="search"`, accessible name "Search {heading}";
    only when the other table is `searchable`; 250 ms debounce, Enter now,
    Escape clears; latest wins);
  - **Linked only** (a `.checkbox` with a visible label; hidden on the create
    form);
  - the pager: "26–50 of 312" (`.num`) with **Previous page** / **Next page**
    (quiet, chevrons), hidden when the total ≤ 25.
- **Query:** `{ search, offset, limit: 25, linkedOnly }`. The search, offset
  and linked-only state are kept per relation while the object stays open,
  and reset when another object opens.
- **The side is read-only** when `view.mode === "read-only"`, the session is
  not writable (`readOnlyReason` or `configFailClosed`), or the table is
  read-only. Then:
  - **Linked only** starts on;
  - rows have no checkbox: linked rows show a `check` icon + **Linked** and
    the label; unlinked rows show just the label;
  - nothing can be staged.

**Rows** (a `ul` with `aria-label` "{heading}" and the visible total in the
meta line):

- **Editable:** `<label>` wrapping a `.checkbox` and the option's label, ticked
  per `isTicked`. Labels wrap; U+FFFD is shown as is.
- **Tags:** **Will be added** (staged add), **Will be removed** (staged
  remove, with the label struck through).
- **Open:** linked rows (and staged-remove rows) end with a quiet **Open**
  (`arrow-right` icon; accessible name "Open {label} in {other table display
  name}") that pushes the object place for the other table. U6's guard asks
  when dirty. There is no Open when the other table is not in
  `session.tables`.
- **Toggling** (click, or Space on the checkbox) calls `toggle`. Focus stays on
  the checkbox. The view announces "{label} will be added." / "will be
  removed." / "change undone.".
- **A staged row is never off screen.**
  - A staged link whose option is on the current page renders in place.
  - Every other staged link of this relation renders in a **Not saved yet**
    group at the top of the list: a subheading in `--text-muted` and the same
    row, using the stored `label`, with an **Undo** button (quiet, `undo` icon;
    "Undo change to {label}").
  - **Each staged link renders exactly once.** Do not duplicate ids.
- **States:**
  - **first load:** 5 skeleton rows, `aria-busy`;
  - **reloading:** the old rows stay, with "Loading…";
  - **empty:** "{other display name} has no objects.";
  - **linked only, none:** "Nothing is linked yet.";
  - **no match:** "No {heading} match “{search}”." with Clear search;
  - **error:** an inline `x-circle` line with `errorMessage(e, "read")` and
    **Try again**. `SchemaChanged` sets the store flag.

**Save integration** (U6's `saveDraft`):

- `links: buildLinkChanges(Object.values(draft.links))`.
- A links-only save is a valid save: the bar counts staged links.
- **On success:** staged links clear; every open checklist and
  `ObjectView.manyToMany` refresh from the returned object and a re-read of
  each section's current page.
- **On create** (U8), the staged adds go in the same request.
- **The save bar** is shown when any checklist on the sheet is editable, even
  if no field is.
- **`ValidationFailed.linkErrors`:** each error's link-level `issues` show on
  its row (wherever it renders), as an `x-circle` line:
  - `referenceMissing`: "This object no longer exists.";
  - other codes: `issue.message`.

  The error summary lists "{heading}: {label}: {message}". Its button focuses
  the row's checkbox (or the Undo button in the pinned group).
- **`Conflict` with `conflictLink`:**
  1. Find the fresh option: re-read the relation's current page. If the link
     is not on it, page through `m2mOptions(..., { search: null, offset,
     limit: 500, linkedOnly: true })` until it is found (a linked option, by
     `linkKey`) or the pages run out (not linked). The other object's
     existence is unknown in that case: treat it as not linked.
  2. Rebase that one link with `rebaseLinkChange`, but **set it aside**:
     show a warn notice in the section, "{label}'s link was changed by
     another program. Nothing was saved. Your change to it was set aside.",
     with **Re-apply my change** (stages `next`) and **Discard it** (drops
     it). For `alreadyLinked` / `alreadyGone` the notice instead reads "…
     {label} is already linked." / "… {label}'s link is already gone." with
     a single **OK**.
  3. Keep every other edit.
  4. `current` replaces the base through `rebaseDraft`. If `current.version`
     differs from the draft's base version, U6's object conflict panel also
     shows.
  5. Focus moves to the notice (`tabIndex={-1}`).

**Shortcuts:** no new rows. Space toggles a focused checkbox; Tab moves
through rows.

## Files

Create in `apps/desktop/src/tools/dbmanager/`:

- `M2mChecklist.tsx`, `linkDraft.ts`;
- tests: `linkDraft.test.ts`, `M2mChecklist.test.tsx`,
  `M2mChecklist.staging.test.tsx`, `M2mChecklist.save.test.tsx`.

Edit `ObjectSheet.tsx`, `store.ts` (`draft.links`, `dirtyCount`, save, the
link conflict), `draft.ts` (`rebaseDraft` keeps links), `SaveBar.tsx` (the
editable test), `leaveGuard.tsx`, `fixtures.ts` and `styles/dbmanager.css`.

Fixtures from the example:

- book "Dune" with `m2m:book_genre` (heading "Genres", other `genre`, no
  link fields), with 40 genres generated, 3 linked;
- the genre side ("Books");
- a read-only relation;
- the create form for a book.

## Skill

- Run `/impeccable layout` on the checklist at the 38rem and 28rem sheet
  widths and at 200% text, then `/impeccable clarify` on the staging and
  conflict copy (keep the meaning; report proposed changes). Run both from
  `apps/desktop` with `--target src/tools/dbmanager/M2mChecklist.tsx`.
- Read `reference/craft-floor.md` before the first UI edit.
- Extend the established world. A changed item reflows in place with words
  saying what will happen, never colour alone.
- Run `/impeccable audit` and the detector before done.
- If the skill is missing, install it (`npx impeccable install`) or follow
  its references by hand, and say so.

## Acceptance criteria

**Listing:**

- "Genres" shows "3 linked".
- The first page requests `{ search: null, offset: 0, limit: 25, linkedOnly:
  false }` with the book's key. 25 rows render, linked ones ticked, with the
  pager "1–25 of 40".
- Next page requests offset 25. Linked only requests `linkedOnly: true`.
  Search follows the debounce and latest-wins rules.

**Staging:**

- Ticking "Poetry" tags it **Will be added**, the meta line reads "3 linked
  · 1 to add", the save bar reads "1 unsaved change", and the view announces
  it.
- Unticking "Fantasy" tags it **Will be removed** with the label struck
  through.
- Toggling either again restores it exactly.
- After paging away, both appear in **Not saved yet** with Undo, once each.
  Paging back renders them in place and not in the group.

**Saving:**

- Save sends `links: [{ relation: "m2m:book_genre", add: [{ other: poetryKey,
  fields: [] }], remove: [{ other: fantasyKey, version: <its linkVersion> }]
  }]` with the field changes.
- A links-only save works.
- On success the staged links clear and the counts refresh.
- On create, ticked genres go in the create request as adds.

**Read-only:**

- A read-only relation has no checkboxes, starts with Linked only on, shows
  **Linked** words, and has a Read-only tag.
- In a read-only session every checklist is read-only, and the save bar does
  not appear for checklists.

**Open and errors:**

- Open on a linked genre navigates to it, through the guard when dirty.
- A link error "listed more than once" shows on its row and in the summary;
  the summary button focuses the checkbox.

**Link conflicts:**

- A `Conflict` naming Poetry (staged add, now linked by someone else) shows
  the "already linked" notice.
- A `Conflict` on a staged remove whose version changed shows Re-apply,
  which stages a remove with the fresh version.
- Every other staged link and every field edit is kept in both cases.

**Accessibility and layout:**

- Every checkbox is labelled by its object.
- Open buttons name the object and table.
- Status is in words.
- jsx-a11y lint passes. No horizontal scrolling at 200% text.

## Tests proving completion

- `linkDraft.test.ts`:
  - `toggle table`;
  - `isTicked`;
  - `buildLinkChanges groups by relation`;
  - `rebaseLinkChange table, every row`.
- `M2mChecklist.test.tsx`:
  - `first page and counts`;
  - `paging and linked only`;
  - `search debounce and stale`;
  - `states: loading, empty, none linked, no match, error`;
  - `read-only rendering`;
  - `open navigates`.
- `M2mChecklist.staging.test.tsx`:
  - `tick and untick tag and count`;
  - `undo exact`;
  - `pinned group once each`;
  - `announcements`;
  - `create form adds`.
- `M2mChecklist.save.test.tsx`:
  - `request links shape`;
  - `links-only save`;
  - `success refresh`;
  - `link-level errors on row and summary`;
  - `conflict already linked`;
  - `conflict reapply remove`;
  - `other edits kept`.

Run: `npm run typecheck && npm run lint && npm run test`.

## States covered

- **Checklist:** loading, empty, none linked, no match, error, read-only,
  page.
- **Staging:** staged add, staged remove, pinned, undone.
- **Save:** success, link errors, link conflict (each rebase note).

## Out of scope

- Link-field editors, defaults, create-only link fields, "removed with its
  fields", and link-field errors (U10).
- Self-referential relations: not in v1.
