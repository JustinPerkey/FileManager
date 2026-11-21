# U11 — Database Manager: delete with its impact

Status: awaiting approval (rewritten 2026-11-21 for GPUI; the React version is in git history)
Project: dbmanager (GPUI: gpui-migration)   Depends on: U6 (landed), gpui-migration M12c (landed, with `request_delete_of` of planner request R4)

## Goal

Add **Delete…** to the sheet header and the Delete key on a focused list
item, and render the delete confirmation the model drives: the impact
preview, the blocked form with only Close, impact changed, config changed,
busy, and the success notice.

## Context

FileManager (Windows 11, GPUI + gpui-component). In the Database Manager
deleting is opt-in per table (`TableView.can_delete`; never on pickable
tables) and **always shows its impact first**: what else is deleted,
which links and link data are removed, which references are cleared or
reset, rows in tables this tool does not show (as counts), and whether
triggers may do more. The model (gpui-migration M12c) runs the preview,
the confirmation, the stale-impact and config-changed cases; this task
renders them. Destructive: the dialog starts on Cancel and Delete is the
danger button.

### Model (M12c + R4)

```rust
pub enum DeleteState { Checking, Confirm(DeleteImpact), Blocked(DeleteImpact), Deleting }
impl DbmanagerModel {
    pub fn request_delete(&mut self, cx);                                   // the open object
    pub fn request_delete_of(&mut self, table: &str, key: ObjectKey, label: &str, cx);   // R4: a focused list item
    pub fn delete_state(&self) -> Option<&DeleteState>;
    pub fn confirm_delete(&mut self, cx);  pub fn cancel_delete(&mut self, cx);
    pub fn notice(&self) -> Option<&Notice>;                                // deleted notice, impact-changed and config-changed notices
}
```

(If `request_delete_of` is missing, the Delete key opens the focused item
first (`open_object`) and then calls `request_delete`; report the gap.)
Copy (`text.rs`): "Delete…", "Checking…", the title "Delete “<label>”?",
the description, the group lines ("Also deleted: …", "Links removed: …
The values stored on these links are lost.", "Reference cleared: …",
"Reference reset to its default: …", "Not shown in this tool: …", "(through
the objects above)", "at least 10000", "and N more"), the triggers warning,
"Nothing else references it.", "Your unsaved changes to it are
discarded.", blocked title "“<label>” can't be deleted." and description,
"If it could be deleted, this would also happen:", "Cancel", "Delete",
"Close", "What this delete affects changed. Review it again.", "The config
changed and was reloaded. Review the delete again.", "Deleted “<label>”
and N related rows.".

Focus hints (R5): `DeleteNotice` → the dialog's notice; `ListHeading` after
success.

### Widgets and tokens

`button` (`.danger()` with `trash` icon; `.busy`), `sync_confirm` with
`ConfirmSpec { wide: true, confirm: Some(Danger "Delete", trash) | None
(blocked: cancel_label "Close"), busy, extra }`, `Banner` (Warn for the
in-dialog notices and the triggers line), `text::*`. Tokens: `surface`,
`border`, `text`, `text_muted`, `warn`, `danger`.

### Look

- **Delete…** in the sheet header, right; danger variant with `trash`;
  busy "Checking…" while `Checking`; only when `can_delete` and the object
  exists.
- **Dialog** (`sync_confirm` key `"db-delete"`, wide 640 px): title;
  description; in the scrolling `extra` region: the notice (warn banner)
  when present, the dirty-draft line, the groups in order (group title
  14 px 600, lines 14 px, samples, "and N more"), the triggers warn line;
  footer Cancel (focused) + Delete (danger, busy while `Deleting`;
  Cancel/Escape inert then). **Blocked**: the blocked title and
  description, the blocking groups first, then "If it could be deleted…"
  and the rest, only **Close**.

### Keys

**Delete** on a focused object-list item (U3's list) → `request_delete_of`
when the table allows deleting. Add the help row "Delete · Delete the
focused object… · Focus is on an object and the table allows deleting".

Debug selectors: `db-delete`, `dialog-confirm`, `dialog-cancel`.

## Files

Create: `tools/dbmanager/view/delete.rs`.
Edit: `tools/dbmanager/view/{sheet_header.rs,object_list.rs,banners.rs,shortcuts.rs,tests.rs}`.
Do not touch: model, draft, text, fixtures, other tools, `crates/`,
`docs/plans/`.

## Skill

None here; U14 critiques `dbmanager-delete` and `-delete-blocked`.

## Acceptance criteria

- Delete… and the Delete key appear only when allowed; the dialog renders
  every `DeleteState` and notice; blocked offers only Close.
- Keyboard: the dialog starts on Cancel; Escape cancels (not while
  deleting).
- Labels/tooltips; no colour literal outside `theme`; fmt; clippy pedantic
  `-D warnings`; `cargo test --workspace`.

## Tests proving completion

`tools/dbmanager/view/tests.rs` (fixtures over a temp database):
`delete_button_only_when_allowed`, `delete_previews_then_dialog_starts_on_cancel`,
`blocked_dialog_offers_only_close`, `impact_changed_notice_in_dialog`,
`delete_key_on_list_item_opens_dialog`, `success_notice_and_focus_list`,
`dirty_draft_line_in_dialog`.

## States covered

Checking, confirmable, blocked, impact changed, config changed, deleting,
deleted; no impact.

## Out of scope

Scenarios (U14).
