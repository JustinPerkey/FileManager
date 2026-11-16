# U3 — Panes, table and object navigation, and history

Status: awaiting approval
Project: dbmanager   Depends on: U2 (landed), M10 (landed)

## Goal

Fill the Database Manager's workspace with the responsive panes:

- a table list;
- a paged, searchable object list;
- an object sheet shell that loads the chosen object;
- a history of places with Back and Forward.

## Context

**The product.** FileManager is a portable Windows desktop app (Tauri v2;
React + TypeScript + Vite in `apps/desktop/src/`). The **Database Manager**
(`dbmanager`) edits a SQLite file as an **object editor, not a grid**: pick a
table → its objects, listed by label → open one as a form. References between
objects are links you follow, and **Back** brings you back. U2 built the view
frame, its module-level store (`tools/dbmanager/store.ts`, read with
`useDbmanager()`, reset by `resetDbmanagerStore()`), the session header and
banners, and `errorMessages.ts` (`errorMessage(e, "read" | "write")`,
`toDbmanagerError`). U2 left a placeholder `div.dbmanager__workspace`; this
task replaces it.

You own `apps/desktop/src/` **except `lib/`**: never edit `lib/` or
`lib/generated/`. Tools never import each other.

**Contract rules.**

- Labels, display names and descriptions come from the backend; the UI
  computes none.
- `ObjectKey` is opaque. Echo it back, and never read inside it.
- To key objects in UI state, use the local helper `objectId(table, key) =
  table + "\u0000" + JSON.stringify(key)` in `tools/dbmanager/keys.ts`. It is
  the same technique as `lib`'s `linkKey`, and it treats the key as an opaque
  string.

### The client (`lib/dbmanager.ts`)

| Function | Returns |
| --- | --- |
| `list(table: string, query: ListQuery)` | `ObjectPage` |
| `object(table: string, key: ObjectKey)` | `ObjectView` |

Both reject with a `DbmanagerError`:

- `UnknownTable` and `UnknownObject`;
- `SchemaChanged`: set the store's `schemaChanged` flag, so U2's banner with
  Reload appears;
- `Busy` and `Io`.

If M10 named these differently, use the landed names for
`dbmanager_list` / `dbmanager_object`, and say so.

### Types (generated)

```ts
type TableView = { name: string; displayName: string; description: string | null;
  mode: "editable" | "read-only"; canCreate: boolean; canDelete: boolean; searchable: boolean;
  hasTriggers: boolean; fields: FieldView[]; referencedBy: ReverseRelationView[];
  manyToMany: M2mView[] };
type ListQuery = { search: string | null; offset: number; limit: number;
  referencing: ReferencingFilter | null };      // limit clamped to 1..=500 by the backend
type ObjectPage = { items: ObjectSummary[]; total: number; offset: number };
type ObjectSummary = { key: ObjectKey; label: string };
type ObjectKey = { type: "rowid"; value: string } | { type: "primary"; values: DbValue[] };  // opaque
type ObjectView = { table: string; key: ObjectKey; label: string; version: string;
  fields: FieldValue[]; referencedBy: RelationCount[]; manyToMany: RelationCount[] };
```

`DbmanagerSession.tables: TableView[]` (listed tables, schema order) and
`open` come from U2's store.

### The design (brief excerpt)

**Visual system** (`DESIGN.md`, `styles/tokens.css`; no new tokens):

- quiet, dense, flat;
- one accent (`--accent`) for the primary action, the current place
  (a 6 px dot before the current item's label, as in the app's tool
  navigation) and focus;
- panes on `--surface`, separated by 1 px `--border` rules: no cards, no pane
  radius, no shadows;
- current item: `--surface-sunken` fill + the accent dot + `aria-current`;
  hover: `--surface-sunken`;
- muted text `--text-muted`; type sizes `--font-size-sm/md/lg/xl`; the
  spacing scale;
- light and dark from tokens.

Shared components: `Button` (`quiet`, `icon`), `Icon` (`arrow-left`,
`arrow-right`, `search`, `chevron-left`, `chevron-right`, `lock`),
`.input`, `.visually-hidden`, `.num`.

**Panes and breakpoints.** `.dbmanager__workspace` is an inline-size
container (`container-type: inline-size`, named `workspace`), filling the
rest of the view (`flex: 1; min-height: 0`). Breakpoints are in rem, so 200%
text narrows the layout by itself:

| Workspace width | Layout |
| --- | --- |
| ≥ 68rem | three panes: **Tables** (12rem) · **Objects** (18rem) · **Sheet** (rest) |
| 46–68rem | two panes: **Navigator** (18rem: the table picker as a disclosure above the object list) · **Sheet** |
| < 46rem | one pane, chosen by the current place: `tables` → the table list; `table` → the object list (with the table disclosure on top); `object` / `new` → the sheet |

- Each pane scrolls on its own (`overflow: auto`). Under `@media (max-height:
  34rem)` the panes do not scroll separately and the whole view scrolls.
- Nothing scrolls horizontally at 800×560 or at 200% text.
- Both renderings of the table list exist in the DOM; container-query CSS
  shows one with `display: none` on the other, which also removes it from the
  accessibility tree. The disclosure's list is rendered only while expanded,
  so no ids are duplicated.

**Table list** (`TableNav`):

- **Rail** (three panes): `h2` "Tables" (visually hidden, labelling the
  `nav`), then `<nav aria-label="Tables"><ul>` of buttons.
  - Each button is the table's `displayName`, with `title` = `description`
    when set.
  - Read-only tables add a **Read-only** tag: the lock icon + the word, at
    `--font-size-sm`, in `--text-muted`, with a 1 px `--border` and
    `--radius`.
  - The current table has `aria-current="true"`, the fill and the dot.
  - **Roving tabindex:** one tab stop (the current table, else the first);
    Up/Down wrap, Home/End jump; Enter or click chooses.
- **Disclosure** (two panes and one pane): a full-width button "Table: Books"
  (`aria-expanded`, `aria-controls`) at the top of the navigator.
  - Expanded, it shows the same list inline (not a popover). Choosing a table
    collapses it.
  - In one pane, the `tables` place shows the list expanded and without the
    toggle.
- **No tables:** "This database has no tables to show. The config may hide
  them." in `--text-muted`.

**Object list** (`ObjectList`), for the current table:

- `h2` (`tabIndex={-1}`, it takes focus on arrival): the table's
  `displayName`, plus `description` under it in `--text-muted`.
- **Toolbar:**
  - a search input (`.input`, `type="search"`, a visible label "Search" with
    accessible name "Search {displayName}", a leading search icon), only when
    `searchable`;
  - Ctrl+F focuses it;
  - Enter searches now; typing searches after 250 ms; Escape clears a
    non-empty search.
  - (U8 adds **New** to this toolbar. Leave a place for it after the search.)
- **List:** `<nav aria-label="Objects in {displayName}"><ul>` of buttons, one
  per `ObjectSummary`, showing `label`.
  - Labels wrap (`overflow-wrap: anywhere`) and clamp at two lines
    (`line-clamp: 2`), with the full label in `title`. The accessible name is
    always the full label. Text with U+FFFD is shown as is.
  - The open object has `aria-current="true"`, the fill and the dot.
  - **Roving tabindex:** the tab stop is the open object if it is on the page,
    else the first item. Up/Down move focus (they do **not** open); Home/End
    jump; **PageUp/PageDown** load the previous/next page and focus its first
    item; **Enter**, Space or click opens.
- **Paging:** 100 per page.
  - `limit: 100`, `offset` from the per-table list state, `search` trimmed
    (null when empty), `referencing: null`.
  - Under the list, the pager: "1–100 of 12345" (`.num`), with **Previous
    page** and **Next page** quiet buttons (chevron icons, visible labels).
  - The buttons are disabled at the ends. The pager is hidden when `total`
    ≤ 100.
- **States:**
  - **First load:** 6 skeleton rows (`--surface-sunken` bars, no animation)
    and `aria-busy`.
  - **Reloading** (search, page): the old items stay, `aria-busy`, and a
    muted "Loading…" line.
  - **Empty table:** "{displayName} has no objects yet." (U8 adds "Create one
    with New.").
  - **No match:** "No objects match “{search}”." with a **Clear search**
    button.
  - **Error:** an inline line with the `x-circle` icon in `--danger`,
    `errorMessage(e, "read")` and **Try again**.
- **Latest wins:** only the newest list request's response is applied.

**Places and history** (in the store):

```ts
type Place =
  | { kind: "tables" }
  | { kind: "table"; table: string }
  | { kind: "object"; table: string; key: ObjectKey; label: string }
  | { kind: "new"; table: string };          // U8 renders it; U3 only routes it
```

- History is `{ entries: Place[]; index }`, capped at 50 entries (the oldest
  is dropped).
- **Push:** choosing a table pushes `table`; opening an object pushes
  `object`.
- **Replace:** the store exposes `replacePlace(place)` for later tasks (after
  create and after delete).
- Pushing truncates the forward entries.
- Each table keeps a **list state** `{ search, offset }` for the session.
  Returning to a table, by any route, restores it.
- **Opening a database** (a new `database.path`) resets history to `[{ kind:
  "table", table: tables[0].name }]`, or `[{ kind: "tables" }]` when there
  are no tables, and clears the list states.
- **A session update keeps history.** If the current place's table is no
  longer listed, the store pushes `tables` and adds a notice: "{table} is no
  longer available."
- **Back / Forward:**
  - the `LocationBar` buttons at the top of the sheet pane: quiet, with the
    `arrow-left` / `arrow-right` icons and the visible labels "Back" /
    "Forward";
  - `title`: "Back to {place name} (Alt+Left)", where the place name is the
    object's label, the table's display name, or "Tables";
  - `aria-keyshortcuts="Alt+ArrowLeft"` / `"Alt+ArrowRight"`;
  - also **Alt+Left / Alt+Right** anywhere in the view, and the mouse
    back/forward buttons (`mouseup` with `event.button` 3 / 4 on the view
    root, `preventDefault`);
  - each is disabled at its end of the history.
- **Breadcrumb** beside them: `<nav aria-label="Location">`: the table's
  display name as a quiet button (going to the table's place), "/", then the
  object's label as plain text with `aria-current="page"`.
- **Later tasks guard navigation:** U6 asks before leaving a dirty draft. Put
  every navigation through one store action, `navigate(intent)`, with intents
  `push(place)`, `back`, `forward` and `replace(place)`, so U6 can guard it in
  one place.

**The sheet shell** (`ObjectSheet`, in the sheet pane). U4 renders the fields;
this task renders the frame and the loading states.

- `table` or `tables` place: an empty state, "Choose an object from the
  list." (or "Choose a table." for `tables`).
- `object` place:
  1. `object(table, key)` loads it, on entry and whenever the place changes.
     **Latest wins.**
  2. While loading: the location bar, the eyebrow, the `h2` from the place's
     `label`, and 5 skeleton field rows.
  3. Loaded:
     - the **eyebrow** (the table's display name, `--font-size-sm`,
       `--text-muted`);
     - the **`h2`** = `ObjectView.label` (`--font-size-xl`, weight 600,
       `tabIndex={-1}`, wrapping);
     - if `ObjectView.label` differs from the place's, the history entry's
       label is updated;
     - a **Read-only** tag when the table's `mode` is `read-only`.
  4. `UnknownObject`: "This object no longer exists. Another program may have
     deleted it." with **Back**. The history entry is kept.
  5. Other errors: an inline error with **Try again**.
- The store keeps the loaded `ObjectView` as `current` (U4–U11 read it).

**Focus rules.**

| Event | Focus goes to |
| --- | --- |
| Enter on a list item, with two or three panes | stays on the item; the sheet loads beside it; announce "Opened {label}." |
| Enter on a list item, one pane | the sheet's `h2` |
| Back, Forward, the breadcrumb | the destination's heading: the sheet's `h2`, or the object list's `h2` for a table place, or the table list for `tables` |
| Choosing a table | the object list's `h2` |

**F6 / Shift+F6** move focus between the visible regions in order: Tables
(rail) → Objects → Sheet. Each region's target is its roving tab stop, or the
sheet's `h2`. In the two-pane layout the Navigator is one region; in one pane,
F6 does nothing.

**Shortcut rows added** to `shortcuts.ts`:

| Keys | Action | Active when |
| --- | --- | --- |
| Alt+Left, Alt+Right | Back, Forward (also the mouse back and forward buttons) | There is somewhere to go |
| F6, Shift+F6 | Move between tables, objects and the object | A database is open |
| Ctrl+F | Search objects | The table is searchable |
| Up, Down, Home, End | Move in the table and object lists | Focus is in a list |
| PageUp, PageDown | Previous or next page of objects | Focus is in the object list |
| Enter | Open the focused table or object | Focus is on a list item |

## Files

Create in `apps/desktop/src/tools/dbmanager/`:

- `keys.ts`, `Workspace.tsx`, `TableNav.tsx`, `ObjectList.tsx`,
  `LocationBar.tsx`, `ObjectSheet.tsx`;
- tests: `keys.test.ts`, `history.test.ts` (the store's place and history
  actions), `TableNav.test.tsx`, `ObjectList.test.tsx`,
  `ObjectList.keyboard.test.tsx`, `DbmanagerView.navigation.test.tsx`.

Edit:

- `store.ts`: places, history, list states, `current`, `navigate`;
- `DbmanagerView.tsx`: render `Workspace` instead of the placeholder;
- `shortcuts.ts`, `useDbmanagerShortcuts.ts`, `fixtures.ts`;
- `styles/dbmanager.css`;
- the root `DESIGN.md`: in the Layout section, change "Planned (dbmanager
  U3): the Database Manager's panes" to "Landed (dbmanager U3)", and correct
  any value that differs from what you built. Change nothing else there.

Fixtures: the example database's tables (Authors with `searchable`; Books;
Genres; Members; Loans; Publishers read-only), with 250 authors generated for
the paging tests and one label of 300 characters.

## Skill

- Run `/impeccable layout` on the workspace once it renders, at 1366×800,
  1000×700 and 800×560, and at 200% text. Run it from `apps/desktop` with
  `--target src/tools/dbmanager/Workspace.tsx`.
- Read `reference/craft-floor.md` before the first UI edit.
- Extend the established world: the app's tool navigation is the reference
  for list items, and the tarpack table's sticky and scroll rules for panes.
- Run `/impeccable audit` on `tools/dbmanager/` and the detector before done.
- If the skill is missing, install it (`npx impeccable install`) or follow
  its references by hand, and say so.

## Acceptance criteria

**Layout:**

- With the workspace ≥ 68rem: three panes. At 46–68rem: two, with the "Table:
  Books" disclosure. Below 46rem: one pane per place. Tests set the class or
  container width the CSS keys on; check the CSS by reading it in the CSS
  test.

**Table list:**

- It shows every listed table by display name. Publishers shows "Read-only"
  as a word, not only an icon.
- The current table has `aria-current`.
- Up/Down/Home/End rove, and Enter chooses.

**Object list:**

- It requests `{ search: null, offset: 0, limit: 100, referencing: null }`.
- With 250 authors, the pager reads "1–100 of 250"; Next page requests
  offset 100; PageDown does the same and focuses the first item; the
  buttons disable at the ends.
- Search sends the trimmed text after 250 ms (immediately on Enter).
- A late response for an older search is ignored.
- Escape clears the search; Ctrl+F focuses it.
- Empty table, no match (with Clear search), error (with Try again) and first
  load (skeleton, `aria-busy`) each render.
- A 300-character label wraps and clamps; its accessible name and `title`
  are the full label.
- Up/Down move focus without opening; Enter opens.

**Places and history:**

- Choosing Authors, then opening "Herbert, Frank", gives history `[table
  Books, table Authors, object Herbert]`.
- Back (button, Alt+Left, mouse button 3) returns to Authors, and Forward
  returns to the object. The buttons disable at the ends.
- Pushing after Back drops the forward entries. History never exceeds 50.
- Each table's search and offset are restored on return.
- Opening a different database resets history and list states.
- A reload in which the current table disappears goes to `tables`, with the
  notice.

**Sheet shell:**

- An object place loads with skeletons, then shows the eyebrow, the label
  `h2` and the Read-only tag for Publishers.
- `UnknownObject` shows its state with Back.
- `SchemaChanged` from `list` or `object` sets the schema-changed banner.

**Focus and keyboard:**

- Focus follows the focus-rules table.
- F6 cycles the visible regions.
- Alt and Ctrl shortcuts work from inside the search input.

**Accessibility:**

- jsx-a11y lint passes.
- Every list is a labelled `nav` with `ul` and buttons.
- Read-only is stated in words.
- Nothing scrolls horizontally at 800×560 or at 200% text.

## Tests proving completion

- `keys.test.ts`:
  - `objectId is stable and distinguishes tables`.
- `history.test.ts`:
  - `push, back, forward`;
  - `push truncates forward`;
  - `cap at 50`;
  - `replace`;
  - `list state per table`;
  - `new database resets`;
  - `missing table after reload`.
- `TableNav.test.tsx`:
  - `rail lists tables with read-only words`;
  - `roving keys`;
  - `disclosure expands and collapses`;
  - `no tables`.
- `ObjectList.test.tsx`:
  - `first page query`;
  - `paging and pager text`;
  - `search debounce and enter`;
  - `stale search ignored`;
  - `empty, no match, error, loading`;
  - `long labels clamp with full name`.
- `ObjectList.keyboard.test.tsx`:
  - `arrows move without opening`;
  - `enter opens`;
  - `page keys load pages`;
  - `ctrl+F and escape`.
- `DbmanagerView.navigation.test.tsx`:
  - `open object loads sheet`;
  - `back and forward by button, alt+arrows and mouse`;
  - `unknown object state`;
  - `schema changed sets banner`;
  - `focus after navigation`;
  - `F6 cycles regions`;
  - `one-pane place routing`.

Run: `npm run typecheck && npm run lint && npm run test`.

## States covered

- **Loading:** table list (with the session), object list (first load and
  reload), sheet.
- **Empty:** no tables, empty table, no match, no object chosen.
- **Error:** list error, object error, object gone, schema changed.
- **Success:** lists, paging, current item, history.

## Out of scope

- The fields, relations and editing in the sheet (U4 onwards).
- **New** (U8) and **Delete** (U11).
- Guarding navigation when a draft is dirty (U6): only route it through
  `navigate`.
