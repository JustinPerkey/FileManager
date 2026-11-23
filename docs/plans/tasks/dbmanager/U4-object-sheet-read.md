# U4 — Database Manager: the object sheet, read view

Status: approved (2026-11-21) (rewritten 2026-11-21 for GPUI; the React version is in git history)
Project: dbmanager (GPUI: gpui-migration)   Depends on: U3 (landed), gpui-migration M12a (landed)

## Goal

Render an opened object's sheet read-only: the header (eyebrow, label,
tags, read-only reason), the property sheet of typed values with hints and
stored-value problems, references as followable links, pickable objects
read-only, and the "Referenced by" disclosures.

## Context

FileManager (Windows 11, GPUI + gpui-component). The Database Manager shows
an object as a form: every field labelled by its display name, references
shown as the referenced object's **label** (never a bare id) and followable,
the object's neighbourhood (what it points to, what points to it) on one
sheet. Read-only is the **marked** exception: a read-only value is shown on
`surface_sunken` with no border at full text strength, still focusable and
selectable, with a lock tag saying why; never the disabled look.

Rules: views render `DbmanagerModel`; copy from `text.rs`; no filesystem
access; every control keyboard reachable and labelled (visible label or
tooltip).

### Model (M12a)

```rust
impl DbmanagerModel {
    pub fn object(&self) -> Option<&ObjectLoad>;        // Loaded(ObjectView): label, version, values (FieldValue: value, problem), referenced_by counts
    pub fn place(&self) -> &Place;
    pub fn follow(&mut self, fk_field: &str, cx);       // pushes the referenced object's place
    pub fn referenced_by(&self, relation: &str) -> Option<&RefList>;   // items, total, loading, error
    pub fn open_referenced_by(&mut self, relation: &str, cx);  pub fn show_more_referenced_by(&mut self, relation: &str, cx);
    pub fn open_object(&mut self, table: &str, key: ObjectKey, cx);
    pub fn snapshot(&self) -> Option<&DbmanagerSnapshot>; // tables (TableView with fields: FieldView, reverse relations)
}
```

`FieldView`: `name`, `display_name`, `kind` (integer, real, text,
duration, bool, blob, other), `rule` (bounds, allowed values, max length,
multiline, unit, pattern help), `mode` (editable, read-only, create-only,
never), `nullable`, `is_key`, `foreign_key: Option<ForeignKeyView>`
(`table`, `table_display_name`), `help`. `ReverseRelationView`: `table`,
`via` display, count from the object. Copy in `text.rs`: hint fragments
(bounds "1 to 10000", "at least 0", "up to 200 characters", durations
"1m to 100h", the unit, "Leave empty for not set"), "Not set", "Missing
object", "{n} bytes", problem lines (`wrongType`, `notBool`, `outOfBounds`,
`danglingReference`) with "Other fields can still be saved." on the first
only, tag words, "This is a lookup table: its objects can be picked, but
not changed here.", the read-only reasons, "Books · via Author · 2",
"none", "Show 25 more".

Focus hints: `take_focus_hint()` (R5) or a changed `focus_hint()`:
`SheetHeading` focuses the h2.

### Widgets and tokens

`text_input(..).read_only(true)` (and `.multiline` for long text),
`checkbox` (disabled-looking is wrong here: render read-only booleans as a
checkbox that does not toggle, plus the word), `tag` (Read-only + `lock`,
Key, Set when created), `button` (quiet link style), `Disclosure`,
`icon(ArrowRight|AlertTriangle)`, `text::*`. Tokens: `surface`,
`surface_sunken`, `border`, `text`, `text_muted`, `warn`, `danger`,
`accent`.

### Layout (the sheet pane, under U3's location bar; 8 px gaps)

1. **Header**: eyebrow (13 px muted) = table display name; h2 (20 px 600) =
   the label, focusable; tags: **Read-only** (lock) when the table or
   database is read-only, with the one-line reason under the title (13 px
   muted), **Lookup table** for pickable tables (reason line above).
   (Delete… is U11's.)
2. **Property sheet**: one row per field, 8 px apart. Two columns (label
   176 px | value) when the sheet is ≥ 576 px wide, stacked below. Label:
   14 px 600, then tags (**Read-only** with lock only when the table is
   editable and this field is not; **Key** muted on key fields; **Set when
   created** on create-only fields). Value by kind: text/integer/real/
   duration → a read-only input (`surface_sunken`, no border, mono for
   integer and real with tabular figures; multiline text a read-only
   multi-line input); bool → the checkbox state and word; blob → "{n}
   bytes"; null → "Not set" muted; FK → the **reference link**: a quiet
   button showing the label underlined in `text` with an `arrow-right`
   icon, tooltip "Open <label> in <table>" → `follow(field)`; dangling →
   the stored value mono + "Missing object". Hint line 13 px muted
   (fragments joined " · "). Problem line: `alert-triangle` in `warn`, text
   in `text`.
3. **Referenced by**: one `Disclosure` per reverse relation ("Books · via
   Author · 2"); count 0 renders as plain muted text ending "none"; opening
   → `open_referenced_by`; inside: links (each a reference-style button →
   `open_object`), "Loading…", an error + **Try again**, and **Show 25
   more** while more remain. Closed by default.

Pickable objects: everything read-only, no save bar, no Delete; Referenced
by read-only.

Debug selectors: `db-sheet-title`, `db-field-<name>`, `db-follow-<name>`,
`db-refby-<relation>`, `db-refby-more-<relation>`.

**Hosting rules (after the U3 and U4 reviews, 2026-11-23).**

- **Sizing.** The chrome hosts the tool view in a `flex_1` / `min_h_0`
  column and cannot style it. So this task's root view is `size_full`, adds
  its own padding (16 px vertical, 24 px horizontal: `SPACE_4` / `SPACE_5`
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

Create: `tools/dbmanager/view/{sheet_header.rs,field_row.rs,values.rs,reference.rs,referenced_by.rs}`.
Edit: `tools/dbmanager/view/{sheet.rs,mod.rs,tests.rs}`.
Do not touch: model, draft, text, fixtures, other tools, `crates/`,
`docs/plans/`.

## Skill

None here; U14 critiques the screenshots.

## Acceptance criteria

- Every kind renders as specified; read-only values are selectable read-only
  inputs, never faded; references show labels, never ids (except a dangling
  value).
- Keyboard: reference links, Referenced by disclosures, their links and
  Show more are Tab reachable and operable; read-only inputs are focusable.
- Labels/tooltips everywhere; no colour literal outside `theme`.
- fmt; clippy pedantic `-D warnings`; `cargo test --workspace`.

## Tests proving completion

`tools/dbmanager/view/tests.rs` (`#[gpui::test]`, fixture over the example
schema): `book_sheet_renders_kinds` (Dune: Author link, Format, In print,
Audiobook length `1h30m`, Revision read-only),
`reference_follow_pushes_place`, `dangling_reference_shows_missing`,
`problem_line_first_has_other_fields_sentence`,
`pickable_object_is_read_only_with_reason`,
`referenced_by_closed_until_opened_and_shows_more`,
`zero_count_relation_is_text`, `layout_stacks_under_576px`.

## States covered

Read, read-only table, read-only database, pickable, problems, dangling,
null, blob, Referenced by: none, closed, loading, list, more, error.

## Out of scope

Editors (U5), saving (U6), the picker (U7), create (U8), checklists (U9),
delete (U11).
