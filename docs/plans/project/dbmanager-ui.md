# UI project plan: the Database Manager view

Status: **awaiting approval.** Written 2026-11-16 by the `ui-designer`.
Backend project plan: [`dbmanager.md`](dbmanager.md) (its §7 is the handoff
this plan answers). Task plans: [`../tasks/dbmanager/`](../tasks/dbmanager/)
`U1`–`U12`.

**Method.** `impeccable` 4.9.0. The launcher (`impeccable context`) ran from
`apps/desktop` and loaded the root `PRODUCT.md` and `DESIGN.md`; no surface
brief existed. `/impeccable shape` was followed for a *whole surface inside an
established world*: the visual system ("The Packing List", `DESIGN.md`) stays
fixed, and only the structure was open. `concept-seed --scope surface --mode
operate` ran and dealt three of the seven structures in §1.1.

**Substitution, stated up front.** This plan was written by a subagent with no
structured question tool and no decision page, so the discovery round and the
structure lock could not be put to the human. Discovery was answered from the
backend plan's §1 and §7 (the human's own answers are recorded there), and the
structure was chosen by the designer (§1.1). Assumptions are marked
*(assumed)*. §11 asks the human to confirm or steer them.

**Stack (decided).** Tauri v2 shell; React + TypeScript + Vite in
`apps/desktop/src/`; tokens in `apps/desktop/src/styles/tokens.css`. The
backend `implementer` owns `apps/desktop/src/lib/`; the `ui-implementer` owns
the rest of `apps/desktop/src/`. **No U task starts before M10 lands.** Inline
validation uses M11's `lib/dbmanagerValidation.ts`, so U5 onwards also depend
on M11. M12 (docs, manual, screenshots) runs after U12.

## 1. Design brief

1. **Job and audience.** A developer or tester keeps the data in a SQLite file
   correct: fixes a value, adds a row, links two objects, removes a stale one.
   They know the data; they do not want to write SQL or squint at a grid of
   foreign-key integers. They come back often, alongside their editor and a
   terminal *(assumed, from `PRODUCT.md` Users)*. **Visitor mode: Operate.**
2. **Outcome and proof.** The task is to find an object, see it whole, change
   it, and save it, without ever wondering what will be written. The proof on
   screen: every field typed and labelled by its display name, references
   shown as the referenced object's **label** (never a bare id), the object's
   whole neighbourhood (what it points to, what points to it, what it is linked
   with) on one sheet, unsaved changes marked field by field, and a delete that
   lists exactly what else goes with it. What is uniquely true here: the config
   decides what is editable, so the sheet must show *why* a field is read-only,
   and an unconfigured database is fully editable, so the editable state is the
   calm default and read-only is the marked exception.
3. **Selected direction.** Extend the established world. Structure (§1.1): a
   **master–detail editor** of up to three panes, *Tables | Objects | Object
   sheet*, collapsing to two and then one as the window narrows. Navigation is
   **history-first**: every place (a table, an object, a new-object form) is a
   history entry, so following a reference and coming back with **Back**
   (Alt+Left, the mouse back button) is the primary way to move. The focal
   point is the **object sheet**: an object's label as its title, a property
   sheet of typed fields, then its many-to-many checklists, then what
   references it. A sticky **save bar** at the foot of the sheet holds the
   unsaved-change count, **Discard changes**, and **Save**; **Delete…** sits in
   the sheet's header, far from Save.
4. **Scope and boundaries.** Production-ready Database Manager view with every
   state below, in twelve tasks. Unchanged: the tokens' values, the app shell,
   the backend contract (requests in §10 go to the planner), the other tools'
   views. Anti-goals:
   - a spreadsheet grid of rows and columns, or raw ids where a label exists;
   - SQL, column types, or affinity shown as the primary text (the declared
     type may appear as a muted hint, never as the label);
   - decoration, cards around panes, colored side stripes;
   - a modal for anything but a confirmation;
   - any second copy of the truth: labels, kinds, modes, bounds and units come
     from the backend views.
5. **States and ranges.**
   - Tables: 0 to about 50 listed; display names from 1 to about 40
     characters; some read-only.
   - Objects per table: 0 to millions; lists are paged (100 per page), never
     loaded whole. Labels from 1 character to a few hundred, possibly with
     U+FFFD where the stored text is not valid UTF-8.
   - Fields per object: 1 to about 60. Text values up to thousands of
     characters (multiline). Integers up to 19 digits (strings, never JS
     numbers).
   - Many-to-many: the other side may have thousands of objects (paged, 25 per
     page), any number linked, 0 to about 6 link fields.
   - Delete impact: 0 to 10 000+ affected rows across many groups.
   - Session states: nothing open, opening, open, read-only database, config
     diagnostics, config failed closed (everything read-only), config changed
     on disk, schema changed, busy, restore failed.
6. **Interaction and layout.** §2 (layout) and §3 (interaction model).
7. **Constraints and open decisions.** WCAG 2.2 AA; keyboard first; 800×560
   minimum window; 200% text; light and dark; the CSP and capabilities do not
   change (the Open dialog and drag-drop only). The jsx-a11y lint passes
   without disabled rules except the documented `ShortcutsHelp` exception. The
   open questions are in §11; the builder must not invent answers to them.

### 1.1 Structures considered (the surface roll)

Seven grounded structures, ordered by resonance:

1. **Master–detail panes**: Tables | Objects | Object sheet, like Explorer or
   Outlook.
2. **Drill-down navigator + sheet**: one left column that shows tables, then a
   table's objects, beside the sheet.
3. **Page per place, with history**: every object is a page; references are
   links; Back and Forward and a breadcrumb do the moving, like a browser or a
   wiki.
4. **Index-card drawer**: tables as tabs across the top, an alphabetical index
   strip, the object as a card.
5. **Multi-document tabs**: each opened object in a tab, a table tree on the
   left, like an IDE.
6. **Relationship map**: the object in the centre, its references and
   referrers around it as nodes; you move by following edges.
7. **List with inspector**: a full-width dense list; the object opens in a
   right-hand inspector drawer.

The roll dealt **6 (lead), 3 and 1**. With no human to lock a card, the
designer chose:

- **Frame: 1**, master–detail panes. It is the only dealt structure that
  carries thousands of objects, dense typed editing, keyboard-first movement,
  and the 200% text setting.
- **Raised by 3 (page per place):** history is the navigation model, not an
  afterthought. Every place is a history entry; following a reference pushes
  one; Back and Forward work by keyboard and mouse; the sheet shows a
  breadcrumb *Table / Object*. In the one-column layout, the panes *are*
  pages.
- **Raised by 6 (relationship map):** the object's neighbourhood is
  first-class. Links out sit on their fields as followable labels; many-to-many
  checklists and "Referenced by" sit on the same sheet, each with its count, so
  the whole neighbourhood is visible without leaving the object.
- **6 declined as the frame:** a node layout fails density, keyboard order and
  200% text on a working screen with typed forms.
- **Catalog challengers declined:** the vertical media feed, the zoo guide
  map, the Crouwel grid specimen and the boarding-pass board are visual worlds.
  At surface scope the world is settled, so none applies. Kept from the
  boarding-pass board: *a changed item reflows in place*. Staged link changes
  stay in place in their row, with words saying what will happen.

The human may steer this choice (§11, question 1).

## 2. Layout

### 2.1 Regions, top to bottom

1. **Session header.**
   - `h1`: the database file name ("library.db"), or "Database Manager" when
     nothing is open.
   - Under it: the full database path, in mono at `--font-size-sm` and
     `--text-muted`, wrapping (`overflow-wrap: anywhere`); the config line
     ("Config: library.toml" or "No config: everything is editable unless
     the database prevents it"); a **Read-only** tag when the database is open
     read-only; and the backup line once a backup exists ("Backup:
     library.db.20261116T101500Z.bak", full path in `title` and visually hidden
     text).
   - Actions on the right, wrapping onto a second line when narrow: **Open
     database…**, **Recent** (menu), **Config** (menu: Choose config…, Open
     without config), **Reload**, **Close database**, **Keyboard shortcuts**.
2. **Banners**, in a fixed order: command error; config failed closed
   (everything read-only); config changed on disk; schema changed; read-only
   database; state warning; backup created; deleted. Then the **config
   problems** panel (diagnostics) when there are any.
3. **Workspace**, the only region that grows: the panes (2.2).

### 2.2 Panes and breakpoints

The workspace is an inline-size container (`container-type: inline-size`).
Breakpoints are in `rem`, so the 200% text setting moves to a narrower layout
on its own.

| Workspace width | Layout | At the default 1000×700 window |
| --- | --- | --- |
| ≥ 68rem | three panes: **Tables** (12rem) · **Objects** (18rem) · **Sheet** (the rest) | at 1366 px and wider |
| 46–68rem | two panes: **Navigator** (18rem: the table picker as a disclosure above the object list) · **Sheet** | yes (≈ 47.5rem) |
| < 46rem | one pane: whichever the current place needs: the table list, the object list, or the sheet; Back moves between them | at 800×560, and at 200% text |

- Panes sit on `--surface` and are separated by 1 px `--border` rules: no
  cards, no radius on the panes, no shadow.
- Each pane scrolls on its own. In windows under 34rem tall, the whole view
  scrolls instead, the panes do not scroll separately, and the save bar is
  static.
- The sheet's property layout is two columns (label 11rem | value) when the
  sheet is at least 36rem wide (its own container), and stacked (label above
  value) below that.
- The **save bar** is sticky at the bottom of the sheet pane, offset by the
  pane's padding as the tarpack build bar is.
- Nothing scrolls horizontally at page level at 800×560 or at 200% text.

### 2.3 The sheet, top to bottom

1. A **location bar**: **Back** and **Forward** (quiet, arrow icons, labelled
   "Back" / "Forward", with `title` "Back to Dune (Alt+Left)"), and the
   breadcrumb *Books / Dune*. The table part is a button that goes to the
   table's place.
2. A **sheet header**:
   - an eyebrow: the table's display name;
   - `h2`: the object's label, or "New object" on a create form;
   - tags: **Read-only** (lock icon + word, when the table or the database is
     read-only, with a one-line reason under the title) and **Not saved yet**
     on a create form;
   - **Delete…**, when `TableView.canDelete` and the object exists.
3. The **conflict panel** or the **error summary**, when there is one.
4. **Fields**: the property sheet (§3.4).
5. **Many-to-many sections**, one per `M2mView`, in order (§3.6).
6. **Referenced by**: one disclosure per `ReverseRelationView` (§3.5).
7. The **save bar** (sticky): the status text, **Discard changes** and
   **Save** (**Create** on a create form). It is hidden when nothing on the
   sheet is editable.

## 3. Interaction model

### 3.1 Places and history

```ts
type Place =
  | { kind: "tables" }                                   // no table chosen
  | { kind: "table"; table: string }                     // a table's objects, no object open
  | { kind: "object"; table: string; key: ObjectKey; label: string }
  | { kind: "new"; table: string };
```

- History is `{ entries: Place[]; index }`, capped at 50 entries (the oldest
  is dropped).
- **Push:** choosing a table, opening an object, following a reference, New.
- **Replace:** after a create (new → the created object), after a delete (the
  object → its table).
- Pushing truncates the forward entries.
- Opening a database resets history to its first listed table (or `tables`
  when it lists none).
- Each table remembers its own list state (search text, offset) for the whole
  session, so coming back to a table shows it as it was left.
- Objects are identified in UI state by `objectId(table, key)`. This is a
  local helper, `table + "\u0000" + JSON.stringify(key)`, the same technique
  as `lib`'s `linkKey`. Keys stay opaque; nothing reads inside them.
- **Back** / **Forward**: the buttons, Alt+Left / Alt+Right, and the mouse
  back/forward buttons (`mouseup` with `button` 3 / 4, default prevented).
- A history entry whose object has since been deleted opens the "no longer
  exists" state; it is not removed.

### 3.2 The view store

The view's state lives in a module-level store
(`tools/dbmanager/store.ts`, read through `useSyncExternalStore`), not in
component state. Switching to another tool and back keeps the place, the
history and any unsaved draft. Nothing is lost, so no prompt is needed. Tests
call `resetDbmanagerStore()`.

### 3.3 Drafts, dirty state, and leaving

- One draft at a time: the open object's (or new object's) edits, keyed by
  column, plus staged link changes (§3.6).
- A field is **changed** when its editor value, converted to a `DbValue` (and
  normalised by `validateField` when valid), differs from the stored value.
  Typing "1h 30m" over a stored "1h30m" is not a change.
- **Dirty** = any changed field or any staged link change. The save bar says
  "3 unsaved changes", and each changed field shows an **Edited** tag.
- **Escape** in a changed field reverts that field.
- **Discard changes** reverts everything, without asking: the edits are on
  screen, and nothing has been written.
- **Leaving a dirty draft asks first**: opening another object, following a
  reference, Back/Forward, choosing a table, New, opening a database (Open,
  Recent, drop), and Close database. The dialog is the shared
  `ConfirmDialog`:
  - title "Discard unsaved changes?";
  - body "Your changes to “Dune” are not saved. If you leave now, they are
    discarded.";
  - **Keep editing** (focused), **Discard changes** (danger variant).
- **Reload, a config change (Choose config…, Open without config, a dropped
  `.toml`) and resolving a schema change keep the draft.** The view re-reads
  the object afterwards:
  - same version → the draft is rebased; edits to fields that are gone or no
    longer editable are dropped, and a notice lists them;
  - a different version → the conflict panel;
  - the object or table is gone → the draft is dropped with a notice.

  The view is built this way because saving is refused until the user reloads
  (`ConfigChangedOnDisk`), so a reload that discarded edits would punish the
  user for doing what the app asked.
- Closing the app window with a dirty draft loses it without a prompt (the
  window close event needs a capability the CSP plan does not grant; nothing
  is written, so this is the safe direction). §11, question 4.

### 3.4 Fields

Every field row has a label (`FieldView.displayName`), tags, the editor or the
value, a hint line, and then issues and problems.

- **Tags:**
  - **Read-only**, with a lock icon. It appears only when the table is
    editable and this field is not; in a read-only table, the header carries
    it once.
  - **Set when created**, on create-only fields of an existing object.
  - **Key**, on `isKey` fields (muted).
  - **Required**, on a create form for `requiredOnCreate`.
  - **Edited**, on a changed field.
- **The calm editable state:** inputs on `--surface` with a 1 px `--border`.
- **The marked read-only state:** the value in a native read-only input (or a
  read-only text block for multiline) on `--surface-sunken`, with no border,
  at full text strength. It stays focusable and selectable, and the lock tag
  says why. It never takes the disabled look.
- **Editors by kind** (`FieldView.kind`, `rule`):

  | Kind / rule | Editor | Notes |
  | --- | --- | --- |
  | integer | text input, `inputmode="numeric"`, tabular numerals | trimmed; empty means not set |
  | real | text input, `inputmode="decimal"` | trimmed; empty means not set |
  | text | text input, or a textarea when `multiline` | not trimmed; a live counter "12 / 200" when `maxLength` is set |
  | integer or text with `rule.allowed` | `<select>` of the allowed labels | nullable adds "Not set"; a stored value outside the list appears as a selected extra option "Stored: X (not allowed)" |
  | bool | a checkbox labelled by the field (§3.4.1) | |
  | duration | text input | hint "Like 1h30m or 1h 30m. A plain number counts {unit}."; on blur, a valid entry is shown in canonical form (`canonicalDuration`) |
  | FK (`foreignKey` set) | the reference control (§3.4.2) | |
  | blob | read-only "{n} bytes" | never editable |
  | any | read-only text of the value | never editable |

- **Hints** (`--text-muted`, `--font-size-sm`, joined by " · "):
  `FieldView.help`; bounds ("1 to 10000", "at least 0", "up to 200
  characters", durations as "1m to 100h"); the unit for durations; "Leave
  empty for not set" on nullable fields where empty means null.
- **Inline validation** (M11 `validateField`):
  - runs on every change;
  - is shown after the field first loses focus, or after a Save attempt;
  - issues show as an `x-circle` icon and text in `--danger`, with
    `aria-invalid` on the field and the issue tied to it through
    `aria-describedby`.
- **Stored-value problems** (`FieldValue.problem`) show as an
  `alert-triangle` icon in `--warn` and text in `--text`. They never block
  saving other fields. Editing the field replaces the value, and the problem
  line is then replaced by the new value's own validation.

#### 3.4.1 Booleans

- A non-nullable bool is a plain checkbox. Its label is the field name, and
  its state is ticked or clear.
- **A nullable bool that is null** shows the checkbox in the *indeterminate*
  state with the word **Not set** beside it, tied to the checkbox by
  `aria-describedby`. Clicking it, or pressing Space, ticks it. A quiet
  **Clear** button (accessible name "Clear {field}") sets it back to not set;
  it is shown only when the field is nullable, editable, and not already null.
- **notBool** (a stored value other than 0/1): the checkbox is indeterminate,
  and the problem line reads "Stored value is {raw}, not 0 or 1. Tick or clear
  the box to replace it."

#### 3.4.2 References (FK fields)

- **The value** is the referenced object's label as a followable link: a
  button styled as an underlined `--text` link with an arrow icon, accessible
  name "Open Herbert, Frank in Authors". Following it pushes a place.
- **Dangling reference:** the stored value in mono and "Missing object",
  plus the problem line.
- **Not navigable** (the target is hidden): the stored value in mono and "(in
  a table that is not shown)". There is no link.
- **NULL:** "Not set".
- **Editable:** **Change…** opens the **object picker** (a non-modal popover
  with a search combobox over `list(targetTable, …)`, 25 per page, "Show
  more", and **Clear** for nullable fields). Picking stages a `Reference`
  change, and the link shows the new label with the **Edited** tag.
  **Clear** stages `Reference { key: null }`.
- A required reference on a create form shows **Choose…** in place of a link.

### 3.5 Referenced by

- One disclosure per `ReverseRelationView`: "Books · via Author · 2". The
  count comes from `ObjectView.referencedBy`.
- Count 0 renders as plain text ending "none", not as a button.
- Opening a disclosure loads `list(rel.table, { referencing: { relation,
  key }, offset, limit: 25 })`. Each item is a followable link, followed by
  "Show 25 more" while more remain.
- Closed by default, so opening an object costs one read, not one per
  relation.

### 3.6 Many-to-many checklists and link fields

- **Section header:** `M2mView.heading` (`h3`), the line "3 linked" (from
  `ObjectView.manyToMany`, adjusted by staged changes: "3 linked · 2 to add ·
  1 to remove"), and a **Read-only** tag for read-only relations.
- **Toolbar:**
  - search, shown only when the other table is searchable;
  - a **Linked only** checkbox. It is off by default on editable relations
    and on by default on read-only ones (nothing can be ticked there);
  - the pager: "26–50 of 312" with Previous/Next page buttons.
- **Rows** come from `m2mOptions(table, key, relation, { search, offset,
  limit: 25, linkedOnly })`:
  - **Editable:** a native checkbox labelled by the option's label. Linked
    rows also get a quiet **Open** button ("Open Fantasy in Genres").
  - **Read-only:** no checkbox. Linked rows show a check icon and the word
    **Linked** plus **Open**; unlinked rows show only their label.
- **Staging** (nothing is written until Save):
  - Ticking an unlinked row stages an **add** and tags the row **Will be
    added**.
  - Unticking a linked row stages a **remove** and tags it **Will be
    removed**, with the label struck through.
  - Toggling again undoes the staged change exactly, keeping the link's
    original values and version.
  - Staged changes are keyed by `linkKey(relation, other)`.
- **Staged rows are never off screen.** A staged row renders in its place when
  it is on the current page. Otherwise it renders in a **Not saved yet**
  group at the top of the list, with its full editors and an **Undo** button.
  Each staged row renders exactly once.
- **Link fields** (`M2mView.linkFields`):
  - Rows that are linked or staged for add show their link-field editors
    inline after the label, each with a visible short label ("Role",
    "Position") and an accessible name that adds the object ("Role for Ada
    Lovelace"). They wrap under the label when the sheet is narrow.
  - The editors are the same as the field editors (§3.4), with the same hints
    and inline validation.
  - A new add is prefilled from each link field's `defaultValue`;
    `requiredOnCreate` link fields are marked **Required**.
  - Create-only link fields are editable only on a staged add; on an existing
    link they are read-only with **Set when linked**.
  - An edited existing link is tagged **Edited**, and stages an **update**
    carrying its `linkVersion`.
  - Unticking a link that has link fields shows, before Save: "Will be removed
    with its fields: Role translator, Position 1." The line is reversible with
    **Undo** or by ticking again.
- **Create forms** show the checklists too: nothing is linked, so ticking
  stages adds.
- **Save** sends every staged change in `SaveRequest.links`, in one
  transaction with the field changes.
- **`linkErrors`** attach to their row: by `linkKey`, field by field, and
  link-level `issues` on the row itself. They also appear in the sheet's
  error summary.
- **A `Conflict` with `conflictLink`:**
  1. The view reloads that relation's page and re-reads its option.
  2. It **rebases** the staged change for that link (table below).
  3. It keeps every other edit, and shows a notice on the section: "Ada
     Lovelace's link was changed by another program. Nothing was saved. Your
     change to it was set aside." with **Re-apply my change** and **Discard
     it**.

  | Staged change | The link now is | Re-apply stages |
  | --- | --- | --- |
  | add | linked | an update of the user's link-field values against the fresh `linkVersion` (no fields: nothing to do; the notice says it is already linked) |
  | update | not linked | an add with the user's values |
  | update | linked, new version | an update against the fresh version |
  | remove | not linked | nothing (already gone; the notice says so) |
  | remove | linked, new version | a remove with the fresh version |

### 3.7 Saving

1. **Ctrl+S** or **Save**. If any changed field or staged link has inline
   issues, or `missingRequiredOnCreate` reports a missing field, nothing is
   sent. Every issue is shown, and the error summary takes focus.
2. Otherwise the view builds the `SaveRequest`: only the changed fields, plus
   `links`. While saving:
   - editors are read-only, so focus stays where it is;
   - Save is busy ("Saving…");
   - navigation and other writes are disabled;
   - the drop zone says "Wait for the save to finish".
3. **Success:**
   - the sheet takes `result.object`, and the draft clears;
   - the view announces "Saved." (create: "Created {label}.", and the place is
     replaced by the new object);
   - the current list page and the open checklists are re-read, because labels
     may have changed;
   - if `backupCreated` is set, the **backup** banner appears and the session
     is re-read so the header shows the backup line.
4. **Errors** (copy in §7.1):
   - **`ValidationFailed`:** the server's issues replace the inline ones on
     those fields until they are edited again, and the error summary takes
     focus.
   - **`Conflict`:** the conflict panel (below), or the link rebase (§3.6)
     when `conflictLink` is set.
   - **`ConfigChangedOnDisk`:** the session is re-read, so the "config
     changed" banner shows.
   - **`SchemaChanged`:** a banner with **Reload**.
   - **`Busy`:** a banner with **Try again**, which repeats the same save.
   - **Anything else:** the error banner, with Details.
5. **The conflict panel** (object changed elsewhere):
   - the message: "“Dune” was changed by another program after you opened it.
     Nothing was saved.";
   - a list of the user's changed fields, each with "Yours: …" and "Now: …",
     and "2 other fields changed too" for unedited fields that differ;
   - **Re-apply my changes** (rebase onto `current`, then Save again) and
     **Discard my changes** (load `current`);
   - Save is disabled until one of the two is chosen, with the reason "Resolve
     the conflict first.".
6. **Focus after Save:** it stays where it was. If it was on Save, which
   disables once nothing is dirty, it moves to the sheet's `h2`.

### 3.8 Creating

- **New** (plus icon; accessible name "New object in Books"; Ctrl+N) sits in
  the object list's toolbar, only when `TableView.canCreate`.
- It pushes `{ kind: "new", table }`. The sheet shows "New object" under the
  table eyebrow, with a **Not saved yet** tag.
- **Fields on the create form:**
  - editable and create-only fields are editors;
  - read-only fields show "Set automatically" (or their known default);
  - fields with a `defaultValue` are prefilled with it;
  - fields with `hasDefault` but no known value show the placeholder
    "Database default".
- The checklists are available. "Referenced by" is not shown.
- The bar shows "Not saved yet", **Cancel** (discards and goes Back) and
  **Create** (primary, Ctrl+S).

### 3.9 Deleting

- **Delete…** (danger variant, trash icon) is in the sheet header. The
  **Delete** key also starts it on a focused object-list item. Both are shown
  only when `TableView.canDelete`.
- It first calls `deletePreview`: the button is busy, "Checking…". Then it
  opens the **delete dialog**: a wide `ConfirmDialog` that starts on **Cancel**
  and has **Delete** in the danger variant. Its contents are in §7.3.
- **Blocked** (`impact.blocked`): the dialog says why, lists the blocking
  groups first, and offers only **Close**.
- **`ImpactChanged`:** the dialog stays open with `error.impact`, a warn
  notice takes focus ("What this delete affects changed. Review it again."),
  and Delete uses the new token.
- **`DeleteBlocked`:** the dialog switches to the blocked state.
- **`Conflict`:** the dialog closes and the sheet shows the current object
  with "“Dune” was changed by another program. Nothing was deleted. Review it
  and try again.".
- **Success:**
  - the place is replaced by the table;
  - the banner reads "Deleted “Lovelace, Ada” and 9 related rows." plus the
    backup line when `backupCreated` is set;
  - the list re-reads, and focus moves to the object list heading.
- If the object being deleted has a dirty draft, the dialog adds "Your unsaved
  changes to it are discarded."

### 3.10 Focus rules

| Event | Focus goes to |
| --- | --- |
| Enter on a list item (two or three panes) | stays on the item; the sheet loads beside it; announce "Opened Dune." |
| Enter on a list item (one pane) | the sheet's `h2` |
| Following a reference, Back, Forward, a breadcrumb | the destination's heading: the sheet's `h2`, or the object list's `h2` for a table place |
| Choosing a table | the object list's `h2` (the table's display name) |
| A save or command error | the error summary, or the error banner (as the Schedule view does) |
| A dialog opens / closes | the dialog's first button (Cancel) / the control that opened it (shared `ConfirmDialog`) |
| The picker closes | the **Change…** button |
| A delete succeeds | the object list's `h2` |

Headings that take focus have `tabIndex={-1}`. The view has one
always-mounted polite announcer, as the other tools do.

### 3.11 Keyboard map

| Keys | Action | Active when |
| --- | --- | --- |
| Ctrl+O | Open database… | always |
| Ctrl+Shift+O | Choose config… | a database is open |
| F5 or Ctrl+R | Reload | a database is open |
| F8 | Show config problems | the config has problems |
| Ctrl+S | Save (Create on a new object) | the sheet has unsaved changes |
| Ctrl+N | New object | the table allows creating |
| Ctrl+F | Search objects | the table is searchable |
| Alt+Left, Alt+Right (and the mouse back/forward buttons) | Back, Forward | history allows |
| F6, Shift+F6 | Move between Tables, Objects and the sheet | a database is open |
| Up, Down, Home, End | Move in the table list and the object list | focus is in a list |
| PageUp, PageDown | Previous / next page of objects | focus is in the object list |
| Enter | Open the focused table or object | focus is on a list item |
| Delete | Delete the focused object… (opens the confirmation) | focus is on an object-list item and the table allows deleting |
| Esc | Undo the edit in the focused field; close the picker or a menu | a changed field, or an open picker or menu, has focus |

- Every button with a shortcut has `aria-keyshortcuts` and a `title`
  "Shortcut: …".
- Shortcuts with Ctrl or Alt work from anywhere in the view, including inside
  inputs, except Ctrl+N and Ctrl+F, which also work there. Plain keys
  (Delete, Enter, PageUp/PageDown) act only on the list that has focus.
- F5 and Ctrl+R always have their default prevented, so the webview never
  reloads the page.

## 4. Design tokens touched

**No new tokens.** Every color is an existing token, used for these roles:

| Token | Use in this view |
| --- | --- |
| `--surface` | panes, editable inputs, menus, picker, dialogs |
| `--surface-sunken` | read-only values, the current list item, row hover, skeleton bars, `kbd` |
| `--border` | pane rules, input outlines, tags |
| `--text` / `--text-muted` | labels and values / hints, eyebrow, paths, counts |
| `--accent` | Save/Create (the one primary per region), the current-item dot, checkbox `accent-color`, focus ring |
| `--danger` | the danger button variant (Delete, Discard changes in the leave dialog), inline issues, the invalid input border |
| `--warn` | stored-value problems, warnings, the trigger notice |
| `--ok` | not used |
| `--shadow-overlay` | the object picker popover, the shared menu and shortcuts popover, dialogs |
| `--scrim` | the dialog backdrop |

The light and dark values are unchanged (see `DESIGN.md` and `tokens.css`).
U1 adds one assertion to `tokens.test.ts`: `--danger` on `--surface-sunken`
meets 4.5:1 in both themes, because the danger button's hover sits on it
(light ≈ 6.0:1, dark ≈ 8:1).

`DESIGN.md` is updated in this commit with the shared components below,
marked **Planned (dbmanager U1)**. U1 flips them to **Landed**.

## 5. Component inventory

Shared components are in `apps/desktop/src/app/`, with CSS in
`styles/controls.css`. View-local components are in
`apps/desktop/src/tools/dbmanager/`, with CSS in `styles/dbmanager.css`, every
selector scoped under `.dbmanager`. **Tools never import each other**: nothing
here imports from `tools/tarpack/`. The Tar Packager keeps its own
`ShortcutsHelp` and Recent menu, and migrating it to the shared ones is a
later, separate change.

### 5.1 Shared (U1)

| Component | File | Props | States |
| --- | --- | --- | --- |
| `Button` (extended) | `app/Button.tsx` | `variant` gains `"danger"` | default, hover, active, focus, disabled, busy |
| `ConfirmDialog` (extended) | `app/ConfirmDialog.tsx` | adds `cancelLabel?`, `confirmLabel: string \| null` (null: only the cancel button, e.g. "Close"), `confirmVariant?: "primary" \| "danger"`, `confirmIcon?`, `confirmBusy?` (Cancel and Escape do nothing while busy), `size?: "default" \| "wide"`, `children?` (extra content below the description, in a scrollable region, not part of `aria-describedby`) | open, busy, closed; unchanged when the new props are omitted |
| `MenuButton` | `app/MenuButton.tsx` | `label`, `menuLabel`, `items: { id, label, description?, title?, disabled?, onSelect }[]`, `disabled?`, `keyshortcut?` | closed, open, empty (not rendered), item disabled |
| `ShortcutsHelp` | `app/ShortcutsHelp.tsx` | `shortcuts: { keys: string[][]; action; when; devOnly? }[]`, `onOpenChange?` | closed, open |
| `Icon` (extended) | `app/icons.tsx` | new names: `arrow-left`, `arrow-right`, `lock`, `plus`, `trash`, `search`, `chevron-left`, `chevron-right`, `undo`, `check` | — |
| Form controls | `styles/controls.css` | classes `.input` (input, textarea, select), `.checkbox` | default, hover, focus, invalid, read-only, disabled |

### 5.2 View-local

| Component | File | Props | States | Task |
| --- | --- | --- | --- | --- |
| `DbmanagerView` | `DbmanagerView.tsx` | — | loading, restore failed, nothing open, opening, open | U2 |
| store | `store.ts` | `useDbmanager()`, actions, `resetDbmanagerStore()` | — | U2 (places U3, drafts U5) |
| error copy | `errorMessages.ts` | `errorMessage(e, context)`, `toDbmanagerError(e)`, `issueMessage(issue, field)`, `problemMessage(fieldValue, field)` | one entry per kind/code | U2, U4, U5 |
| `SessionHeader` | `SessionHeader.tsx` | `session`, the action callbacks | nothing open, open, read-only database, backup | U2 |
| `EmptyState` | `EmptyState.tsx` | `recent`, `onOpen`, `onOpenRecent` | no recent, recent list | U2 |
| `SessionBanners` | `SessionBanners.tsx` | `session`, notices, the actions | each banner of §2.1 | U2 |
| `ConfigProblems` | `ConfigProblems.tsx` | `diagnostics`, `errorCount`, `warningCount`, `expanded`, `onToggle` | none, warnings only, errors | U2 |
| `Workspace` | `Workspace.tsx` | `session` | three / two / one pane | U3 |
| `TableNav` | `TableNav.tsx` | `tables`, `current`, `onChoose`, `variant` | none listed, list, read-only items; rail or disclosure | U3 |
| `ObjectList` | `ObjectList.tsx` | `table: TableView`, list state, `currentId`, `onOpen`, `onNew`, `onDelete` | first load (skeleton), loading (kept items), empty table, no match, error, page | U3 |
| `LocationBar` | `LocationBar.tsx` | history, `onBack`, `onForward`, `onTable` | back/forward enabled or not | U3 |
| `ObjectSheet` | `ObjectSheet.tsx` | place, `TableView`, `ObjectView` | empty (no object), loading, gone, error, read, edit, create | U3 (shell), U4 (read), U5–U8 |
| `FieldRow` | `FieldRow.tsx` | `field: FieldView`, `value: FieldValue \| null`, editing props | read-only, editable, changed, invalid, problem | U4, U5 |
| `FieldEditor` | `FieldEditor.tsx` | `field`, `value: EditorValue`, `onChange`, `onRevert`, `labelSuffix?`, `describedBy` | per kind (§3.4) | U5 |
| `BoolField` | `BoolField.tsx` | `field`, `checked: boolean \| null`, `problem?`, `onChange` | ticked, clear, not set, notBool | U5 |
| `ReferenceField` | `ReferenceField.tsx` | `field`, `value`, staged pick, `onFollow`, `onPick`, `onClear` | linked, not set, dangling, not navigable, staged | U4 (read), U7 (edit) |
| `ObjectPicker` | `ObjectPicker.tsx` | `table: TableView`, `currentId?`, `nullable`, `onPick`, `onClear`, `onClose` | loading, results, no match, error, more | U7 |
| `ReferencedBy` | `ReferencedBy.tsx` | `relations`, `counts`, `objectKey`, `onFollow` | none, closed, loading, list, more, error | U4 |
| `SaveBar` | `SaveBar.tsx` | dirty count, `saving`, `mode: "edit" \| "create"`, `blockedReason?`, `onSave`, `onDiscard` | clean, dirty, saving, saved, blocked | U6 |
| `ErrorSummary` | `ErrorSummary.tsx` | `items: { label, targetId }[]`, `heading` | hidden, shown (focused) | U6 |
| `ConflictPanel` | `ConflictPanel.tsx` | `rows`, `otherChanged`, `onReapply`, `onDiscard` | shown | U6 |
| `M2mChecklist` | `M2mChecklist.tsx` | `table`, `objectKey \| null`, `view: M2mView`, `linkedCount`, staged links, `onStage`, `onOpen` | loading, empty, no match, error, read-only, page, staged | U9, U10 |
| `LinkFieldEditors` | `LinkFieldEditors.tsx` | `fields: FieldView[]`, values, `objectLabel`, `phase: "add" \| "update"`, `onChange` | add, existing, read-only, invalid | U10 |
| `DeleteDialog` | `DeleteDialog.tsx` | `impact`, `notice?`, `busy`, `dirty`, `firstWrite`, `onConfirm`, `onCancel` | confirmable, blocked, impact changed, busy | U11 |
| shortcuts | `shortcuts.ts`, `useDbmanagerShortcuts.ts` | `SHORTCUTS` (the §3.11 table) | — | U2 (first rows), U12 (complete) |
| keys and drafts | `keys.ts`, `draft.ts`, `linkDraft.ts` | `objectId`; `toDbValue`, `isChanged`, `rebaseDraft`; `rebaseLinkChange` | pure | U3, U5, U9 |

## 6. States, by surface

| Surface | Loading | Empty | Error | Success |
| --- | --- | --- | --- | --- |
| Session | `aria-busy`, "Opening library.db…", header actions disabled | "Open a SQLite database" + recent pairs + drop hint | restore failed (warn banner + Try again); the database slot error (error banner); config unreadable or failed closed (error banner, everything read-only) | the header and workspace |
| Table list | (comes with the session) | "This database has no tables to show. The config may hide them." | — | the list, with Read-only tags |
| Object list | 6 skeleton rows on first load; afterwards the old items stay with `aria-busy` and "Loading…" | "Books has no objects yet." + New; "No objects match “xyz”." + Clear search | an inline error line + Try again | the page, pager, current item |
| Sheet | skeleton field rows | "Choose an object from the list" (+ "or create one with New" when allowed) | "This object no longer exists…" + Back; a read error + Try again | read, edit, create |
| Referenced by | "Loading…" in the open disclosure | "none" | an inline error + Try again | links + Show more |
| Checklist | skeleton rows | "Genres has no objects." / "Nothing is linked yet." (linked only) / "No genres match" | an inline error + Try again | the page, staged rows |
| Picker | "Loading…" | "No objects match" / "Authors has no objects." | an inline error + Try again | options + Show more |
| Delete dialog | Delete… busy "Checking…" | — (no impact: "Nothing else references it.") | a preview error goes to the banner; ImpactChanged / DeleteBlocked in the dialog | confirmable or blocked |

## 7. Copy

Voice, per `PRODUCT.md`: plain, short and exact. Every write failure says
nothing was changed.

### 7.1 Errors (`DbmanagerErrorKind`)

`errorMessage(error, context)` keeps one entry per kind in a `Record` over the
closed union. `context` is `"read"` or `"write"`; only the entries marked
differ by context. The technical `message` goes in a **Details** disclosure, as
in the other tools.

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
| `Conflict` | “{label}” was changed by another program after you opened it. Nothing was saved. *(delete: Nothing was deleted.)* |
| `SchemaChanged` | The database's structure changed. Reload to continue. Nothing was changed. |
| `ConfigChangedOnDisk` | The config file changed. Reload it before saving. Nothing was changed. |
| `DatabaseReadOnly` | The database is open read-only, so it can't be changed. Nothing was changed. |
| `Busy` | The database is busy: another program is using it. *(write: Nothing was changed.)* Try again in a moment. |
| `BackupFailed` | The backup copy could not be written, so nothing was changed. Check that the database's folder is writable and has free space. |
| `ImpactChanged` | What this delete affects changed. Nothing was deleted. Review it again. |
| `DeleteBlocked` | Other objects still reference this one, and the database does not allow deleting it while they do. Nothing was deleted. |
| `ConstraintFailed` | The database refused the change: one of its own rules (a constraint or trigger) failed. Nothing was changed. See Details. |
| `Io` | *(read)* The database could not be read. See Details. *(write)* The change could not be written. Nothing was changed. See Details. |
| *Unexpected* | Something went wrong. See Details. |

### 7.2 Field issues (`FieldIssueCode`) and stored-value problems

`issueMessage(issue, field)`, one entry per code. `{limit}` is `issue.limit`.

| Code | Copy |
| --- | --- |
| `required` | Enter a value. *(bool: not reached; reference: Choose an object.)* |
| `wrongType` | This kind of value can't be entered here. |
| `notInteger` | Enter a whole number, like 42. |
| `notReal` | Enter a number, like 3.5. |
| `notBool` | Tick or clear the box. |
| `notDuration` | Enter a duration like 1h30m or 1h 30m, or a number of {unit}. |
| `durationPrecision` | Must be whole {limit}. *(for example "Must be whole minutes.")* |
| `belowMin` | Must be at least {limit}. |
| `aboveMax` | Must be at most {limit}. |
| `tooShort` | Must be at least {limit} characters. *(limit 1: Can't be empty.)* |
| `tooLong` | Must be at most {limit} characters. |
| `patternMismatch` | Doesn't match the expected format{help ? ": " + help : ""}. The pattern itself is shown in mono below: `{limit}`. |
| `notAllowed` | Choose one of the listed values. *(a link-level issue: the issue's own `message`)* |
| `unique` | Another object already has this value. |
| `referenceMissing` | The chosen object no longer exists. Choose another. *(link level: This object no longer exists.)* |
| `notEditable` | This field can't be changed. |
| `unknownField` | This field is no longer available. Reload to continue. |

`problemMessage(fieldValue, field)` covers `ValueProblem`:

| Problem | Copy |
| --- | --- |
| `wrongType` | The stored value is not {a whole number \| a number \| text \| a yes/no value \| a duration}: {raw}. |
| `notBool` | Stored value is {raw}, not 0 or 1. Tick or clear the box to replace it. |
| `outOfBounds` | The stored value is outside this field's limits. |
| `danglingReference` | Refers to a missing object ({raw}). |

Every problem line ends with "Other fields can still be saved." only the first
time it appears on a sheet.

### 7.3 Confirmations and notices

- **Leave a dirty draft:** §3.3.
- **Delete, confirmable.**
  - Title: Delete “{label}”?
  - Description: This deletes it from {table display name}{ and changes N
    other rows}. A backup of the database is saved first. *(when a backup
    already exists: This session's backup is {file name}.)*
  - The impact, grouped and in this order (each line names the count, the
    table and the via):
    - **Also deleted:** "2 objects in Books (via Author): Dune, Emma";
    - **Links removed:** "Contributions: 1 link (Dune). The values stored on
      these links are lost." (the second sentence only when `linkData`);
    - **Reference cleared:** "3 objects in Loans will have Member cleared:
      …";
    - **Reference reset to its default:** the same shape;
    - **Not shown in this tool:** "2 rows in audit_log (via author_id)", with
      no samples.
  - A group with depth ≥ 2 ends "(through the objects above)". A truncated
    count reads "at least 10000". Samples are followed by "and N more".
  - `hasTriggers`: a warn line, "This database has triggers. They may change
    or delete more than is listed here."
  - Buttons: **Cancel** · **Delete**.
- **Delete, blocked.**
  - Title: “{label}” can't be deleted.
  - Description: Other objects still reference it, and the database does not
    allow deleting it while they do. Change or delete those first.
  - **Blocking:** the blocking groups first; then "If it could be deleted,
    this would also happen:" and the rest.
  - Button: **Close**.
- **No impact:** "Nothing else references it."
- **Banners:**
  - Backup: "A backup of the database was saved before this first change:
    {path}."
  - Config failed closed: "Everything is read-only: {configFailClosed}. Fix
    the config and reload, or open without a config." Actions: **Reload**,
    **Open without config**.
  - Config changed: "The config file changed on disk. Reload to apply it;
    saving is paused until you do." Action: **Reload**.
  - Schema changed: "The database's structure changed. Reload to continue."
    Action: **Reload**.
  - Read-only database: "This database is open read-only: {reason}. You can
    browse it but not change it."
  - State warning: the backend's `stateWarning` text.
  - Deleted: "Deleted “{label}”{ and N related rows}."
  - Reload notice (dropped edits): "After the reload, these changes no longer
    apply and were removed: {field names}."
- **Config problems:** the summary reads "Config: 2 errors, 1 warning.
  Entries with errors are read-only." Each item shows the severity word and
  icon, then "Line 12, column 3" (or "Table book, field isbn"), then the
  message.

## 8. UI task index

| # | Task plan | Goal | Depends on |
| --- | --- | --- | --- |
| U1 | [`U1-shared-controls.md`](../tasks/dbmanager/U1-shared-controls.md) | Shared vocabulary: danger button, extended `ConfirmDialog`, `MenuButton`, generic `ShortcutsHelp`, new icons, form-control styles | M10 |
| U2 | [`U2-view-and-session.md`](../tasks/dbmanager/U2-view-and-session.md) | Registry entry, view scaffold, store, session header, empty state, open/recent/config/reload/close, drop, banners, config problems, error copy | U1, M10 |
| U3 | [`U3-navigator-and-history.md`](../tasks/dbmanager/U3-navigator-and-history.md) | Responsive panes, table nav, paged searchable object list, places and Back/Forward, sheet shell | U2, M10 |
| U4 | [`U4-object-sheet-read.md`](../tasks/dbmanager/U4-object-sheet-read.md) | The object sheet read view: typed values, tags, problems, followable references, Referenced by | U3, M10 |
| U5 | [`U5-field-editors.md`](../tasks/dbmanager/U5-field-editors.md) | Typed field editors, the draft, dirty markers, inline validation | U4, M10, M11 |
| U6 | [`U6-save-and-conflicts.md`](../tasks/dbmanager/U6-save-and-conflicts.md) | Save bar, save flow, error summary, conflict panel, leave guard, reload rebase, backup notice | U5, M10, M11 |
| U7 | [`U7-reference-picker.md`](../tasks/dbmanager/U7-reference-picker.md) | Editing FK fields with the object picker | U6, M10 |
| U8 | [`U8-create.md`](../tasks/dbmanager/U8-create.md) | New object: create form, defaults, required, open after create | U7, M10, M11 |
| U9 | [`U9-m2m-checklist.md`](../tasks/dbmanager/U9-m2m-checklist.md) | Many-to-many checklists: paging, search, linked only, staged add/remove, saving links, link conflicts | U8, M10 |
| U10 | [`U10-link-fields.md`](../tasks/dbmanager/U10-link-fields.md) | Link-field editors on links: defaults, create-only, removal with data, link errors | U9, M10, M11 |
| U11 | [`U11-delete.md`](../tasks/dbmanager/U11-delete.md) | Delete with impact confirmation, blocked, impact changed | U6, M10 |
| U12 | [`U12-keyboard-and-audit.md`](../tasks/dbmanager/U12-keyboard-and-audit.md) | Complete shortcuts and help, F6 regions, 200%/800×560 pass, `/impeccable audit` + detector, a11y sweep | U1–U11 |

**Backend alignment:**

- Every U task depends on **M10**. U5, U6, U8 and U10 also use **M11**.
- U2's config-failed-closed banner needs the M10 revision requested in §10.1.
- **M12** runs after U12.
- U11 can run in parallel with U7–U10 once U6 lands.

## 9. Screenshot scenarios for M12

`mock.js` scenarios named `dbmanager-*`, built on the example schema and
config (`examples/dbmanager/`). The windows are 1366×800 (three panes) unless
noted; 1000×700 is the default window (two panes).

| Shot | Window | Shows |
| --- | --- | --- |
| `dbmanager-empty` | 1000×700 | nothing open: the empty state with two recent pairs |
| `dbmanager-tables` | 1366×800 | the three panes: the table rail (Publishers tagged Read-only), Authors' object list, no object open |
| `dbmanager-object` | 1366×800 | book "Dune": Author as a link, Publisher, Format (enum), In print (checkbox), Audiobook length `1h30m` (duration with hint), Revision read-only, the Genres checklist below |
| `dbmanager-object-two-pane` | 1000×700 | the same book in the two-pane layout |
| `dbmanager-narrow` | 800×560 | one pane: the sheet with Back |
| `dbmanager-picker` | 1366×800 | the Author picker open on a book, searching "her" |
| `dbmanager-validation` | 1366×800 | inline issues: ISBN pattern, Pages "0" below the minimum, Audiobook length "90ms" (`durationPrecision`), with the Edited tags and the save bar "3 unsaved changes" |
| `dbmanager-m2m` | 1366×800 | the Genres checklist with one staged add, one staged remove, "2 to add · 1 to remove" |
| `dbmanager-link-fields` | 1366×800 | the Contributors checklist on a book: an author ticked, being added as translator with position 1; an existing contributor's fields |
| `dbmanager-link-remove` | 1366×800 | a contributor unticked: "Will be removed with its fields: Role translator, Position 1" + Undo |
| `dbmanager-create` | 1366×800 | a new book: defaults prefilled, Required tags, the Create button |
| `dbmanager-conflict` | 1366×800 | the conflict panel after a save: Yours / Now |
| `dbmanager-referenced-by` | 1366×800 | an author with "Books · via Author · 2" opened |
| `dbmanager-delete` | 1366×800 | the delete dialog for an author: also deleted, links removed with link data lost, through the objects above |
| `dbmanager-delete-blocked` | 1366×800 | the blocked dialog for a publisher (config allowing delete) |
| `dbmanager-diagnostics` | 1366×800 | the config problems panel expanded: one error with line/col, one warning |
| `dbmanager-readonly-config` | 1366×800 | a config with a syntax error: the everything-read-only banner and read-only fields |
| `dbmanager-config-changed` | 1366×800 | the config-changed banner with Reload |
| `dbmanager-backup` | 1366×800 | the backup banner after the first save, the backup line in the header |
| `dbmanager-leave` | 1366×800 | the "Discard unsaved changes?" dialog |
| `dbmanager-shortcuts` | 1366×800 | the keyboard shortcuts popover |

Accessible names the shots rely on:

- **Header and session:** "Open database…", "Recent", "Config", "Reload",
  "Close database", "Keyboard shortcuts".
- **Navigation:** "Back", "Forward", "New" (accessible name "New object in
  {table}"), "Search {table}", "Linked only".
- **Sheet:** "Save", "Create", "Discard changes", "Delete…", "Change…",
  "Clear", "Show more".
- **Dialogs and banners:** "Delete", "Cancel", "Close", "Keep editing",
  "Re-apply my changes", "Discard my changes".

## 10. Requests to the planner (contract)

These are requests, not edits to the backend plans.

1. **Required by U2: a flag for "config failed closed".** The UI must say
   prominently when a config syntax error, an unsupported `version`, or an
   unreadable file has made *everything* read-only. Today it can tell only
   the unreadable case (the config slot's `ConfigUnreadable`). A syntax error
   arrives as one error diagnostic, and the UI must not infer the
   whole-config consequence from message text.
   - Please add `DbmanagerSession.configFailClosed: string | null` (Rust
     `config_fail_closed: Option<String>`): a short reason for display ("line
     3, column 7: expected `=`", "unsupported version 2", "the file could not
     be read"), `null` otherwise.
   - Pin it in M10's snapshot tests.
2. **Pin the client function names in M10** so the task plans name them
   exactly. U tasks assume `session`, `open`, `setConfig`, `openDropped`,
   `reload`, `close`, `list`, `object`, `m2mOptions`, `save`,
   `deletePreview` and `deleteObject`, plus the helpers `boolValue`,
   `durationValue`, `linkKey`, `isConfirmDeleteAllowed` and
   `isDbmanagerError`. If M10 lands other names, the ui-designer updates the
   task plans before they run.
3. **Confirm that `dbmanager_session()` is cheap and side-effect free after
   the first call** and recomputes `configChanged`. The view calls it when the
   window regains focus, so editing the config in an editor surfaces the
   banner without a click (M12 checklist step 8).
4. **Optional:** an object noun per table (for example `TableView.objectName`
   from a config key `object_name = "book"`). Display names are often plural
   ("Authors"), so v1 copy avoids "New Author" and uses "New object in
   Authors" / "New object". With a noun, the copy becomes "New book". Not
   needed for v1.

5. **Optional: find one link by key.** After a `Conflict` with
   `conflictLink`, U9 needs that link's fresh `linkVersion` and
   `linkValues`. Without a lookup, it pages through `linkedOnly` results
   (limit 500) until it finds the link, which is linear in the number of
   links.
   - An `M2mQuery.only: ObjectKey[] | null` filter (or the fresh option
     carried in the error, for example `DbmanagerError.conflictOption`) would
     make this one call.
   - U9 works without it.

## 11. Open questions for the human

1. **Structure.** The designer chose three panes (Tables | Objects | Object
   sheet) collapsing to two and one, with Back/Forward history as the main way
   to move (§1.1), without a human lock on the roll. Confirm, or steer: for
   example, objects opening in tabs, or a single page per object.
2. **Opening an object from the list.** Up/Down moves focus and Enter opens
   (predictable, and no prompt storm when a draft is dirty). Some editors open
   the focused item as you arrow ("selection follows focus"). Keep Enter?
3. **Delete shortcut.** The Delete key on a focused object-list item opens
   the confirmation (never deletes directly). Keep it, or button only?
4. **Closing the window with unsaved edits** loses them silently (nothing is
   written). Prompting needs the window-close capability. Worth a capability
   change (a reviewed CSP/capability decision)?
5. **Discarding on Open without config.** "Open without config" makes every
   table editable at once. Should it ask first?
