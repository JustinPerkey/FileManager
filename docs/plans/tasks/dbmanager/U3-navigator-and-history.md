# U3 — Database Manager: panes, table nav, object list, Back/Forward, sheet shell

Status: approved (2026-11-21) (rewritten 2026-11-21 for GPUI; the React version is in git history)
Project: dbmanager (GPUI: gpui-migration)   Depends on: U2 (landed), gpui-migration M12a (landed)

## Goal

Fill the Database Manager's workspace: the width-based one/two/three-pane
layout, the table nav (pickable tables left out), the paged searchable
object list with keyboard movement, the location bar with Back/Forward and
the breadcrumb, and the sheet's shell (empty, loading, gone).

## Context

FileManager (Windows 11, GPUI + gpui-component). The Database Manager edits
a SQLite file as objects; **navigation is history-first**: every place (a
table, an object, a new-object form) is a history entry, and following a
reference and coming back with Back is the primary way to move. Pickable
(lookup) tables are not listed; their objects open read-only.

Rules: views render `DbmanagerModel` and call its methods; history, list
states and paging are the model's. A missing model need is a gap to report.
Every interactive element is keyboard reachable and labelled. No
filesystem access in views.

### Model (`tools/dbmanager/model.rs`, M12a)

```rust
pub enum Place { Tables, Table { table: String }, Object { table: String, key: ObjectKey, label: String }, New { table: String } }
impl DbmanagerModel {
    pub fn listed_tables(&self) -> Vec<&TableView>;      // not hidden, not pickable
    pub fn place(&self) -> &Place;  pub fn can_back(&self) -> bool;  pub fn can_forward(&self) -> bool;
    pub fn back(&mut self, cx);  pub fn forward(&mut self, cx);
    pub fn choose_table(&mut self, table: &str, cx);
    pub fn open_object(&mut self, table: &str, key: ObjectKey, cx);
    pub fn list_state(&self, table: &str) -> Option<&ListState>;   // search text, offset, page (items, total), loading, error
    pub fn set_search(&mut self, table: &str, text: &str, cx);  pub fn set_offset(&mut self, table: &str, offset: u64, cx);
    pub fn object(&self) -> Option<&ObjectLoad>;                   // Loading | Loaded(ObjectView) | Gone
    pub fn snapshot(&self) -> Option<&DbmanagerSnapshot>;          // tables: Vec<TableView> incl. pickable
}
```

`TableView`: `name`, `display_name`, `mode` (editable, read-only, hidden,
pickable), `object_name`, `can_create`, `can_delete`, searchable flag. Lists
are 25 per page; each table keeps its search and offset for the app run;
only the newest list request applies. Copy from `text.rs`: "This database
has no tables to show. The config may hide them.", "<Table> has no objects
yet.", "No objects match “<text>”." + "Clear search", list error + "Try
again", "Choose an object from the list" (+ "or create one with New"),
"This object no longer exists…" + Back, the pager "26–50 of 312", "Table:
<name>" and "(lookup)", "Lookup table", "Search <table>". If a string is
missing from `text.rs`, report it; do not write copy in the view.

Focus hints: `take_focus_hint()` if present (R5), else a changed
`focus_hint()`; this task handles `ListHeading` and `SheetHeading`.

### Widgets and tokens

`button` (quiet, `.icon_only`, `.icon(ArrowLeft|ArrowRight|ChevronLeft|ChevronRight)`),
`text_input` (search, with the `search` icon), `tag`, `skeleton_bar`,
`text::*`, `Disclosure`. Tokens: `surface`, `surface_sunken`, `border`,
`text`, `text_muted`, `accent`.

### Layout

Workspace width W = window width − sidebar (192 px, at most 40%) − 48 px:

| W | Panes |
|---|---|
| ≥ 1088 px | **Tables** (192 px) · **Objects** (288 px) · **Sheet** (rest) — 1366×800 |
| 736–1088 px | **Navigator** (288 px: the table picker as a `Disclosure` "Table: Books" above the object list) · **Sheet** — 1000×700 |
| < 736 px | one pane: the table list, the object list or the sheet, whichever the place needs; Back moves between them — 800×560 |

Panes on `surface`, separated by 1 px `border` rules; a 1 px rule above
the workspace; no radius, no shadow; each pane scrolls vertically on its
own; the workspace fills the remaining height.

- **Table nav**: items stacked 4 px apart, padding 8/12, radius 6, full
  width, wrapping; hover `surface_sunken`; current: `surface_sunken`, 600,
  6 px accent dot 8 px before the label; a **Read-only** tag on read-only
  tables. When the place is in a pickable table, no item is current and the
  two-pane disclosure reads "Table: <name> (lookup)". Empty: the no-tables
  sentence.
- **Object list**: heading h2 (16 px 600) = table display name (+ **Lookup
  table** tag), focusable for focus hints; toolbar: the search input (shown
  only when the table is searchable; placeholder "Search <table>";
  `set_search` as the user types); rows 32 px, padding 0/12, the label with
  end ellipsis and its full text as tooltip; current object's row
  `surface_sunken` + accent dot; hover `surface_sunken`; pager under the
  rows: the range text (13 px muted, tabular) and icon-only Previous/Next
  page buttons (tooltips "Previous page", "Next page"). First load: 6
  skeleton rows; later loads keep the old rows and show "Loading…" muted;
  empty, no-match (+ **Clear search**), error (+ **Try again**).
- **Location bar** (top of the sheet pane, 8 px gaps): quiet **Back** and
  **Forward** with arrow icons and visible labels, tooltips "Back to <place>
  (Alt+Left)" / "Forward to <place> (Alt+Right)", disabled per
  `can_back`/`can_forward`; the breadcrumb "<Table> / <Object>" (13 px),
  the table part a quiet button → `choose_table`.
- **Sheet shell**: under the location bar: no object → the "Choose an
  object…" sentence (muted); `Loading` → four skeleton field rows; `Gone`
  → the gone sentence + **Back**; `Loaded` → a slot U4 fills.

### Keyboard

- Object list and table nav are each one Tab stop with a focused row: Up,
  Down, Home, End move; **Enter** opens (`open_object` / `choose_table`);
  PageUp/PageDown change page (`set_offset`). In two or three panes focus
  stays on the row after Enter; in one pane it goes to the sheet heading.
- Alt+Left / Alt+Right and the mouse back/forward buttons
  (`MouseButton::Navigate`) → `back`/`forward` when allowed, from anywhere
  in the view.
- Ctrl+F focuses the search input (searchable tables).
- F6 / Shift+F6 move focus between the table nav, the object list and the
  sheet's first control (one pane: no-op).

Debug selectors: `db-table-<name>`, `db-list`, `db-list-row-<n>`,
`db-search`, `db-page-prev`, `db-page-next`, `db-back`, `db-forward`,
`db-crumb-table`, `db-sheet`.

**Hosting rules (after the U3 and U4 reviews, 2026-11-23).**

- **Sizing.** The chrome hosts the tool view in a `flex_1` / `min_h_0`
  column and cannot style it. So this task's root view is `size_full`, adds
  its own padding (16 px vertical, 24 px horizontal: `SPACE_4` / `SPACE_6`
  from `theme::tokens`), and handles its own scrolling (an
  `overflow_y_scroll` region inside the root, not the root's parent).
- **Confirmations.** If this view shows a confirmation, call `sync_confirm`
  only from a model observer (`cx.observe(&model, …)`), never inside
  `render`. Opening or closing a dialog during render re-enters the window.

**Scenario tags (planner rule, 2026-11-22, after M4).** The screenshot
harness resolves `Step::Click` and `Scenario::clip` only by `.tag("<name>")`
names (`apps/filemanager/src/app/probe.rs`); GPUI's debug selectors cannot be
read outside `VisualTestContext`, so they do not work in the harness. Every
element a scenario clicks or clips therefore carries `.tag("<name>")` as the
**outermost (last) call of its builder chain**, using the names this plan
lists (where this plan says "debug selector", read "tag"). If a
`VisualTestContext` view test also clicks the element, add
`.debug_selector(..)` with the same name as well.

## Files

Create: `apps/filemanager/src/tools/dbmanager/view/{workspace.rs,table_nav.rs,object_list.rs,location_bar.rs,sheet.rs}`.
Edit: `tools/dbmanager/view/{mod.rs,keys.rs,tests.rs}`.
Do not touch: model, draft, text, fixtures, other tools, `crates/`,
`docs/plans/`.

## Skill

None here; U14 critiques the screenshots.

## Acceptance criteria

- The pane count follows W exactly as tabled; at 1366×800 the panes are
  192 / 288 / rest px.
- Pickable tables never appear in the table nav.
- Keyboard: every list is reachable and operable as specified; Back/Forward
  by keys and mouse buttons.
- Labels or tooltips everywhere; no colour literal outside `theme`.
- fmt; clippy pedantic `-D warnings`; `cargo test --workspace`.

## Tests proving completion

`tools/dbmanager/view/tests.rs` (`#[gpui::test]`, fixture backend, temp
database): `pane_count_follows_width` (1366, 1000, 800 px),
`table_nav_omits_pickable`, `enter_opens_table_and_object`,
`arrow_keys_move_in_list`, `page_keys_and_buttons_page`,
`search_types_into_model`, `alt_left_right_navigate`,
`mouse_navigate_buttons_navigate`, `breadcrumb_table_goes_to_table`,
`gone_object_shows_back`, `f6_cycles_regions`.

## States covered

No tables, table list, first load, loading with kept rows, empty table, no
match, list error, page, current item, lookup table place, no object,
loading object, gone object; one, two and three panes.

## Out of scope

Field values (U4), editing (U5+), New (U8), Delete key (U11).
