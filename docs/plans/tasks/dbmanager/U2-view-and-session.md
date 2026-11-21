# U2 — Database Manager: view frame, session header, empty state, banners, config problems

Status: awaiting approval (rewritten 2026-11-21 for GPUI; the React version is in git history)
Project: dbmanager (GPUI: gpui-migration)   Depends on: gpui-migration U4 (landed), gpui-migration M12a (landed)

## Goal

Replace the Database Manager's placeholder view in the GPUI app with its
frame and everything about the session: the header with the database and
config status, Open database / Recent / Reload / Close database, the empty
state, the banners, the config problems panel and the first shortcuts; and
add the ten icons the Database Manager needs.

## Context

FileManager (a portable Windows 11 exe, GPUI + gpui-component) hosts
several tools. The **Database Manager** edits a SQLite file as objects
rather than a grid: each object is a form of typed fields, references are
the referenced object's label, many-to-many relations are checklists, a
delete lists its impact first. **One config for the whole app**,
`%ProgramData%\FileManager\dbmanager.toml`, decides what is editable; the
user never chooses it; it reloads by itself. Missing config = everything
editable, with a calm notice; an unusable config **fails closed**
(everything read-only, with the reason).

Rules: views render `DbmanagerModel` and call its methods; behaviour, copy
(`text.rs`) and drafts are the model's (backend implementer). A missing
model need is a gap to report, never view code. Views never touch the
filesystem. Every interactive element is keyboard reachable and labelled
(visible label, or a tooltip for icon-only controls); no screen-reader work.
The look is the app's ("The Packing List": tonal layers, 1 px `border`
rules, radius 6, one accent).

### Model (`apps/filemanager/src/tools/dbmanager/model.rs`, M12a)

```rust
impl DbmanagerModel {
    pub fn snapshot(&self) -> Option<&DbmanagerSnapshot>;  pub fn loading(&self) -> bool;
    pub fn error(&self) -> Option<&ViewError>;  pub fn error_generation(&self) -> u64;  pub fn dismiss_error(&mut self, cx);
    pub fn listed_tables(&self) -> Vec<&TableView>;
    pub fn open(&mut self, cx);                    // Windows picker, "SQLite database" + "All files"
    pub fn open_recent(&mut self, index: usize, cx);
    pub fn reload(&mut self, cx);  pub fn close_database(&mut self, cx);
    pub fn place(&self) -> &Place;                 // U3 renders places
}
pub struct DbmanagerSnapshot {
    pub database: Option<DbFileSlot>,              // path, display, error
    pub config: DbConfigView,                      // path, display, status: Loaded | Missing | FailClosed, hash
    pub open: bool, pub read_only_reason: Option<String>, pub config_changed: bool,
    pub config_fail_closed: Option<String>, pub diagnostics: Vec<ConfigDiagnostic>,
    pub error_count: usize, pub warning_count: usize, pub tables: Vec<TableView>,
    pub backup_path: Option<PathBuf>, pub recent: Vec<String>, pub state_warning: Option<String>,
}
```

Copy in `tools/dbmanager/text.rs` (M12a): every error kind's sentence (the
backend message is the Details), the config status lines ("Config: <path>";
"No config file: everything is editable unless the database prevents it.";
the fail-closed line), "The config changed on disk." (+ Reload), the empty
state, the drop label and reasons, the read-only reasons, button labels.
Drop is the Shell's (overlay from the model's drop target).

Focus hints: use `take_focus_hint()` if the model has it (planner request
R5), else act on a changed `focus_hint()`; report the gap. Announcements are
not rendered.

### Widgets and tokens (`crate::widgets`, `crate::theme`)

`text::{h1, small_muted, mono_small_muted, muted}`, `button` (primary,
quiet, `.icon`, `.shortcut`), `MenuButton`, `ShortcutsHelp`, `Banner`
(`Tone::{Info, Warn, Error}`, `.action`, `.dismiss`, `.details`), `tag`,
`Disclosure`, `skeleton_bar`, `icon`. Tokens by name: `bg`, `surface`,
`surface_sunken`, `border`, `text`, `text_muted`, `accent`, `warn`, `danger`.

### Icons

Add ten SVGs to `apps/filemanager/assets/icons/` and their names to the
`icons!` list in `apps/filemanager/src/assets.rs` (planner request R2 grants
the ui-implementer these two edits; **if it is not granted when you start,
stop and report**): `arrow-left`, `arrow-right`, `lock`, `plus`, `trash`,
`search`, `chevron-left`, `chevron-right`, `undo`, `check`. Same style as
the existing ten: 16×16 viewBox, stroke `currentColor` 1.5, round caps and
joins, no fill. Add them to `widgets::IconName`.

### Layout (`dbmanager-ui` design; at 1366×800)

View padding 16/24; regions 12 px apart, top to bottom:

1. **Session header**: left: h1 (20 px 700) = the database file name, or
   "Database Manager" when nothing is open; under it the full path (mono
   13 px muted, wrapping anywhere); the config line (13 px; the path in
   mono) per `config.status`; a **Read-only** tag (`lock`) when
   `read_only_reason` is set; the backup line when `backup_path` is set
   ("Backup: <file name>", 13 px muted, full path as tooltip). Right, 8 px
   apart, wrapping under the title when narrow: **Open database…**
   (shortcut Ctrl+O), **Recent** (`MenuButton`; items = `recent`, label the
   file name, detail its folder; select → `open_recent(i)`), **Reload**
   (shortcut "F5 or Ctrl+R"; only with a database open), **Close database**,
   **Keyboard shortcuts** (quiet, last). (Edit config comes in U12.)
2. **Banners**, 8 px apart, in this fixed order: command error (`error()`,
   Error, Details, dismissible, re-shown on a new generation); config failed
   closed (Error, not dismissible); no config file (Info, dismissible for
   the app run: keep the dismissal in the view); config changed on disk
   (`config_changed`, Warn, action **Reload**); read-only database (Warn);
   state warning (Warn, dismissible). (Schema changed and notices come in
   U6/U12.)
3. **Config problems** (when `diagnostics` is not empty, with or without a
   database): a bordered `surface` box, padding 8/12, a `Disclosure` whose
   summary is "Config: 2 errors, 1 warning. Entries with errors are
   read-only." (from `text.rs`); open: one row per diagnostic: the severity
   word with its icon (`x-circle` danger / `alert-triangle` warn), the
   position ("Line 12, column 3" or "Table book, field isbn") in mono muted,
   the message. Expanded state is view state; F8 opens it and focuses it.
4. **Workspace** (fills the rest): when nothing is open, the **empty
   state** (max 546 px, 12 px gaps): "Open a SQLite database" sentence,
   **Open database…** (primary), the recent databases as quiet buttons
   (file name + folder detail), the drop hint (muted), the config line.
   While `loading` with nothing shown: skeleton bars after 150 ms. With a
   database open: an empty slot `workspace_slot` that U3 fills.

### Keys (this task's rows; U12 completes the map)

Key context `Dbmanager`: Ctrl+O → `open`; F5 and Ctrl+R → `reload` (with a
database); F8 → open and focus config problems (when any). Ctrl chords
work inside inputs; a held key does not repeat. Register the shortcuts help
rows for these three (the full table is U12's): "Ctrl+O · Open database… ·
Always", "F5 or Ctrl+R · Reload · A database is open", "F8 · Show config
problems · The config has problems".

Debug selectors: `db-open`, `db-recent`, `db-reload`, `db-close`,
`db-shortcuts`, `db-config-line`, `db-config-problems`, `db-error`,
`db-empty`.

## Files

Create: `apps/filemanager/src/tools/dbmanager/view/{header.rs,banners.rs,empty.rs,config_problems.rs,keys.rs,shortcuts.rs,tests.rs}`,
`apps/filemanager/assets/icons/{arrow-left,arrow-right,lock,plus,trash,search,chevron-left,chevron-right,undo,check}.svg`.
Edit: `apps/filemanager/src/tools/dbmanager/view/mod.rs` (replace the
placeholder), `apps/filemanager/src/assets.rs` (the `icons!` list only),
`apps/filemanager/src/widgets/icon.rs` (the new `IconName` variants),
`docs/gpui-test-ledger.md` if it has Database Manager rows (it may not).
Do not touch: `model.rs`, `draft.rs`, `text.rs`, `backend.rs`,
`fixtures.rs`, other tools, `crates/`, `docs/plans/`.

## Skill

None here; U14 critiques the screenshots.

## Acceptance criteria

- Header, banners (in order), config problems and empty state render every
  snapshot state from the model; copy from `text.rs`.
- Keyboard: every header action, banner action, the disclosure and the
  empty state's buttons are Tab reachable and operable; Recent works by
  keys; Ctrl+O, F5/Ctrl+R, F8 act as specified.
- Labels or tooltips everywhere; the new icons load
  (`assets::tests::every_icon_loads_and_is_svg` passes with them).
- No colour literal outside `theme`; fmt; clippy pedantic `-D warnings`;
  `cargo test --workspace`.

## Tests proving completion

`tools/dbmanager/view/tests.rs` (`#[gpui::test]`, the fixture backend over
a temp database, `FakePlatform`): `renders_nothing_open_with_recent`,
`renders_open_database_header`, `config_line_per_status` (loaded, missing,
fail-closed fixtures), `banners_in_fixed_order`,
`no_config_banner_dismisses_for_the_run`, `config_problems_expand_with_f8`,
`open_and_recent_call_model` (`FakePlatform` logs the pick),
`reload_shortcut_needs_database`.

## States covered

Loading, nothing open (with/without recent), open, read-only database,
backup line, config loaded / missing / failed closed, config changed on
disk, state warning, command error, config problems.

## Out of scope

Panes and lists (U3), the sheet (U4+), Edit config and reload notices
(U12), scenarios (U14).
