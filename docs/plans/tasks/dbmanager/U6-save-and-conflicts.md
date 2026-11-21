# U6 — Database Manager: save bar, error summary, conflict panel, leave dialog, notices

Status: approved (2026-11-21) (rewritten 2026-11-21 for GPUI; the React version is in git history)
Project: dbmanager (GPUI: gpui-migration)   Depends on: U5 (landed), gpui-migration M12b (landed)

## Goal

Render the save flow the model runs: the save bar (count, Discard changes,
Save, Ctrl+S), the error summary, the object conflict panel, the "Discard
unsaved changes?" leave dialog, the save-related banners and notices
(backup, schema changed, busy, config changed, config reloaded), and check
the Database Manager's line in the app's close prompt.

## Context

FileManager (Windows 11, GPUI + gpui-component). Database Manager saves are
explicit and transactional; the first write of a session takes a backup
beside the database; a failure changes nothing; leaving or closing with a
dirty draft asks first; after a config change the model re-checks the draft
and **never retries a save on its own**. All of that is the model's
(gpui-migration M12b); this task renders it.

Rules: views render `DbmanagerModel` and call its methods; copy from
`text.rs`; destructive choices (Discard changes in the leave dialog) use
the danger button; every control keyboard reachable and labelled.

### Model (M12b)

```rust
impl DbmanagerModel {
    pub fn draft(&self) -> Option<&Draft>;  pub fn dirty(&self) -> bool;  pub fn dirty_count(&self) -> usize;
    pub fn discard(&mut self, cx);  pub fn save(&mut self, cx);  pub fn saving(&self) -> bool;
    pub fn conflict(&self) -> Option<&ConflictPanel>;     // rows: field, yours, now; others_changed
    pub fn reapply(&mut self, cx);  pub fn discard_mine(&mut self, cx);
    pub fn leave_request(&self) -> Option<&LeaveRequest>; // label of the object being left
    pub fn confirm_leave(&mut self, cx);  pub fn cancel_leave(&mut self, cx);
    pub fn notice(&self) -> Option<&Notice>;  pub fn dismiss_notice(&mut self, cx);   // backup, reload, saved
    pub fn error(&self) -> Option<&ViewError>;           // ValidationFailed, SchemaChanged (+ Reload), Busy (+ Try again), …
    pub fn reload(&mut self, cx);
}
```

Copy (`text.rs`): "3 unsaved changes" (and 1), "No unsaved changes",
"Saving…", "Save", "Discard changes", "Resolve the conflict first.", the
error summary heading ("Nothing was saved. Fix the problems below and save
again.") and item lines, the conflict message ("“Dune” was changed by
another program after you opened it. Nothing was saved."), "Yours:",
"Now:", "N other fields changed too", "Re-apply my changes", "Discard my
changes", the leave dialog title "Discard unsaved changes?", body "Your
changes to “<label>” are not saved. If you leave now, they are
discarded.", "Keep editing", "Discard changes", the banners (backup "A
backup of the database was saved before this first change: <path>.";
schema changed + "Reload"; busy + "Try again"; config changed while
editing), the reload notices ("The config was reloaded." / "…These
changes no longer apply and were removed: <names>.").

The close prompt (gpui-migration U4) lists every tool's `unsaved_work`;
the model registers "Database Manager: your changes to “<label>” are not
saved." (M12b). Nothing to render here; this task only tests it.

Focus hints (`take_focus_hint()`, R5, or a changed `focus_hint()`):
`ErrorSummary` → the summary; `ErrorBanner` → the banner; `SheetHeading`
→ the h2 (after a save that disabled the focused Save).

### Widgets and tokens

`button` (primary, `.danger()`, `.busy`, `.shortcut("Ctrl+S")`), `Banner`
(`.action`, `.dismiss`, `.details`), `sync_confirm` (`cancel_label`
"Keep editing", `ConfirmVariant::Danger`), `text::*`, `icon`. Tokens:
`surface`, `border`, `text`, `text_muted`, `danger`, `warn`.

### Layout

- **Save bar**: the last item of the sheet pane, outside its scroll region:
  `surface`, 1 px `border` rule above, padding 12/24, a row (8/16 gaps):
  status text (14 px; "3 unsaved changes", or the blocked reason muted),
  then right-aligned **Discard changes** (secondary; disabled when clean)
  and **Save** (primary; busy "Saving…"; disabled when clean or a conflict
  is unresolved; tooltip "Shortcut: Ctrl+S"). Hidden when nothing on the
  sheet is editable.
- **Error summary** (above the fields, under the sheet header): `surface`,
  1 px `danger` border, radius 6, padding 8/12, `x-circle`, heading, one
  quiet button per issue ("<Field>: <message>") that focuses that field's
  editor; focusable.
- **Conflict panel** (same place): `surface`, 1 px `warn` border, radius 6,
  padding 12; `alert-triangle`; the message; per changed field "<Field>"
  600, "Yours: …", "Now: …" (values mono where the field is); the "others
  changed" line; **Re-apply my changes** (primary) and **Discard my
  changes**.
- **Leave dialog**: `sync_confirm` key `"db-leave"` while `leave_request()`
  is `Some`: title and body above, Cancel = "Keep editing" (focused),
  confirm "Discard changes" danger → `confirm_leave`; Keep editing/Escape →
  `cancel_leave`.
- **Banners** in U2's order, adding: schema changed (Warn, **Reload**)
  after "config changed on disk", busy (Warn, **Try again** → `save`)
  after the command error; **notices** (Info, dismissible → `dismiss_notice`)
  last: backup, config reloaded (with dropped names), saved.

### Keys

Ctrl+S → `save` when `dirty()` (works inside inputs). Escape stays the
field revert (U5).

Debug selectors: `db-save`, `db-discard`, `db-save-status`,
`db-error-summary`, `db-conflict`, `db-reapply`, `db-discard-mine`,
`db-notice`.

## Files

Create: `tools/dbmanager/view/{save_bar.rs,error_summary.rs,conflict.rs,leave.rs}`.
Edit: `tools/dbmanager/view/{sheet.rs,banners.rs,keys.rs,shortcuts.rs,mod.rs,tests.rs}`
(add the help row "Ctrl+S · Save (Create on a new object) · The sheet has
unsaved changes").
Do not touch: model, draft, text, fixtures, other tools, `app/`, `crates/`,
`docs/plans/`.

## Skill

None here; U14 critiques the screenshots.

## Acceptance criteria

- The bar, summary, panel, dialog and banners render every M12b state;
  copy from `text.rs`.
- Keyboard: Ctrl+S saves; every button reachable; the leave dialog starts
  on Keep editing; summary items move focus to their field.
- No colour literal outside `theme`; fmt; clippy pedantic `-D warnings`;
  `cargo test --workspace`.

## Tests proving completion

`tools/dbmanager/view/tests.rs` (`#[gpui::test]`, fixtures over a temp
database): `save_bar_counts_and_saves_with_ctrl_s`,
`discard_reverts_without_asking`, `refused_save_focuses_error_summary`,
`summary_item_focuses_field`, `conflict_panel_reapply_and_discard`,
`save_disabled_until_conflict_resolved`, `leave_dialog_keep_and_discard`,
`backup_notice_and_header_line_after_first_save`,
`busy_banner_try_again_saves_again`, `config_changed_banner_no_retry`,
`close_prompt_lists_database_manager_line` (through the Shell:
`simulate_close()` with a dirty draft; the prompt's body contains the
line).

## States covered

Clean, dirty, saving, saved, blocked by conflict; error summary; conflict;
leave dialog; backup, reload, saved notices; schema changed; busy; config
changed while editing.

## Out of scope

References (U7), create (U8), links (U9, U10), delete (U11).
