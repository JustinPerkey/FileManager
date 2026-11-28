# U12 — Database Manager: Edit config, reload notices, the complete keyboard map and help

Status: approved (2026-11-21) (rewritten 2026-11-21 for GPUI; scope reduced by the human's decision: no screen-reader, 200% text or `/impeccable audit` work; the React version is in git history)
Project: dbmanager (GPUI: gpui-migration)   Depends on: U2–U11 (landed), gpui-migration M12d (landed)

## Goal

Finish the Database Manager's views: developer mode's **Edit config**, the
starter-config and config-reloaded notices, and the complete keyboard map
with its shortcuts help, checked end to end by keyboard alone.

## Context

FileManager (Windows 11, GPUI + gpui-component). The Database Manager's one
app-wide config (`%ProgramData%\FileManager\dbmanager.toml`) reloads by
itself when it changes; the model then re-checks the open draft and shows a
notice (gpui-migration M12d). Developer mode (Ctrl+Alt+Shift+D, app-wide,
off at launch, never listed in help) adds **Edit config**, which opens the
config in the system editor after creating a commented starter file if
there is none.

Rules: views render and call; copy from `text.rs`; every interactive
element keyboard reachable and labelled; plain keys act only on the focused
list; Ctrl/Alt chords work inside inputs; held keys do not repeat actions.

### Model (M12d, plus earlier parts)

```rust
impl DbmanagerModel {
    pub fn can_edit_config(&self, cx: &App) -> bool;  pub fn edit_config(&mut self, cx);
    pub fn config_reloaded_notice(&self) -> Option<&str>;
    pub fn notice(&self) -> Option<&Notice>;  pub fn dismiss_notice(&mut self, cx);
}
```

Developer mode: `crate::app::dev_mode::DevMode::is_on(cx)`; observe with
`cx.observe_global::<DevMode>`. Copy (`text.rs`): "Edit config", "Created a
starter config at <display>.", "The config was reloaded." / "…These
changes no longer apply and were removed: <names>.", the Edit-config
failure messages.

### The keyboard map (bind in key context `Dbmanager`)

| Keys | Action | When |
|---|---|---|
| Ctrl+O | Open database… | always |
| F5 or Ctrl+R | Reload | a database is open |
| Ctrl+E | Edit config | developer mode is on (row shown only then) |
| F8 | Show config problems | the config has problems |
| Ctrl+S | Save (Create on a new object) | the sheet has unsaved changes |
| Ctrl+N | New object | the table allows creating |
| Ctrl+F | Search objects | the table is searchable |
| Alt+Left, Alt+Right (and mouse back/forward) | Back, Forward | history allows |
| F6, Shift+F6 | Move between Tables, Objects and the sheet | a database is open |
| Up, Down, Home, End | Move in the table list and the object list | focus is in a list |
| PageUp, PageDown | Previous / next page | focus is in the object list |
| Enter | Open the focused table or object | focus is on a list item |
| Delete | Delete the focused object… | focus on an object and the table allows deleting |
| Esc | Undo the edit in the focused field; close the picker or a menu | a changed field, or an open picker or menu, has focus |

Most rows exist from U2–U11; this task adds Ctrl+E, checks every row, and
makes `view/shortcuts.rs` hold exactly this table for `ShortcutsHelp`.

### Look

- **Edit config**: a header action after Close database (secondary),
  shown only when `can_edit_config(cx)`; tooltip "Shortcut: Ctrl+E". Also
  the action of the "config failed closed" banner in developer mode.
- **Notices** (U6's notice banner slot): the starter-config notice and the
  config-reloaded notice (Info, dismissible). When the reload ends in
  fail-closed, the failed-closed banner replaces the notice.

Debug selectors: `db-edit-config`.

**Before this runs (from the U2 review, 2026-11-27).**

- The model gains `set_shortcuts_open(bool)`, as the Tar Packager's has, in
  the backend task `M12e-dbmanager-view-copy.md`. Call it from
  `ShortcutsHelp::on_open_change`, so Escape does nothing in the view while
  the help is open.
- **Escape** calls the model's `escape()` (M12e's follow-up), which does
  nothing while the help is open. Never bind the key to `dismiss_notice`
  or `cancel_leave` directly: those are the buttons' handlers and always
  work.
- Shortcuts work wherever focus is in the window: see
  `U4b-tool-shortcuts-window-wide.md`, which must land first. Bind this
  tool's shortcuts the way that task describes, not on the tool's own root.

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

Edit: `tools/dbmanager/view/{header.rs,banners.rs,keys.rs,shortcuts.rs,tests.rs}`.
Do not touch: model, draft, text, fixtures, other tools, `crates/`,
`docs/plans/`.

## Skill

None here; U14 critiques `dbmanager-edit-config`, `-config-reloaded` and
`-shortcuts`.

## Acceptance criteria

- Edit config follows developer mode live; Ctrl+E works only then.
- Every row of the map works as stated (one test per row); the help shows
  the table, with the Ctrl+E row only in developer mode and no
  Ctrl+Alt+Shift+D row.
- A keyboard-only walk (open a database from Recent, choose a table, open
  an object, edit, save, follow a reference, Back, open a checklist row,
  start a delete and cancel) needs no mouse: written as one test.
- No colour literal outside `theme`; fmt; clippy pedantic `-D warnings`;
  `cargo test --workspace`.

## Tests proving completion

`tools/dbmanager/view/tests.rs`: `edit_config_follows_dev_mode`,
`edit_config_opens_with_system` (`FakePlatform` logs the open),
`starter_and_reloaded_notices_render`, one `key_<row>` test per map row,
`help_lists_the_map_without_dev_chord`, `keyboard_only_walk`.

## States covered

Developer mode on/off; starter created; config reloaded (with and without
dropped edits); reload to fail-closed.

## Out of scope

Scenarios (U14).
