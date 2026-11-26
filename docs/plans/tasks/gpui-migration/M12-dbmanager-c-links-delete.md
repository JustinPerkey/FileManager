# M12c — Database Manager, part C: many-to-many staging with link fields, link conflicts, delete

Status: approved (2026-11-21)
Project: gpui-migration   Depends on: M12b (landed)

## Goal

Complete the Database Manager's write behaviour in the GPUI view-model:
staged many-to-many link changes with link fields saved with the draft,
the link-conflict rebase, and the confirmed delete with its impact preview.

## Context

Read first: `apps/filemanager/src/tools/dbmanager/` (M12a reads, places and
history; M12b `draft.rs`, save, conflict panel, leave confirmation, close
guard), `crates/fm-dbmanager/src/tool/` (`Core`, `DbmanagerError` with
`link_errors`, `conflict_link`, `impact`), and in `crates/fm-dbmanager/src/session/`
the link types of dbmanager M8 (`SaveRequest.links: Vec<LinkChange>` with
`add` (other key + fields), `update` (with `link_version`), `remove`
(with `link_version`); `LinkError`, `LinkRef`) and the delete API of
dbmanager M9 (`delete_preview(table, key) -> DeleteImpact` with `version`,
`token`, `blocked`, groups with counts, `links`, `link_data` and hidden
tables as counts; `delete(table, key, version, token, config_hash) -> DeleteResult`;
errors `ImpactChanged`, `DeleteBlocked`, `Conflict`, `ConfigChangedOnDisk`).
`M2mView` has `link_fields`; `M2mOption` has `linked`, `link_values`,
`link_version`; `M2mQuery.only` fetches one option.

### The confirmed behaviour (restated)

**Staging (nothing is written until Save):**

- Ticking an unlinked row stages an **add** ("Will be added"), prefilled
  from each link field's `default_value`; unticking a linked row stages a
  **remove** ("Will be removed"); toggling again undoes the staged change
  exactly, keeping the link's original values and version. Staged changes
  are keyed by relation and the other object's key.
- Editing a link field of a linked row stages an **update** with its
  `link_version` ("Edited"); create-only link fields are editable only on a
  staged add; `required_on_create` link fields are required on an add.
  Link fields use the same editors and `validate` as fields.
- Unticking a link that has link fields shows, before Save, "Will be removed
  with its fields: <Name value>, …", reversible with Undo or by ticking
  again.
- **Staged rows are never off screen:** a staged row whose option is not on
  the current page appears in a "Not saved yet" group (its editors and Undo);
  each staged row appears exactly once.
- Counts line: "N linked", adjusted: "3 linked · 2 to add · 1 to remove".
- Dirty includes staged link changes; `dirty_count` counts them.
- Create forms stage adds too.
- Save sends every staged change in `SaveRequest.links` with the field
  changes, one transaction; `link_errors` attach to their row (field by
  field, plus link-level issues) and to the error summary.

**Link conflict** (`Conflict` with `conflict_link`): re-read that one option
(`only`) and the relation's current page; rebase the staged change for that
link and keep every other edit; notice on the section "<label>'s link was
changed by another program. Nothing was saved. Your change to it was set
aside." with **Re-apply my change** and **Discard it**:

| Staged | The link now is | Re-apply stages |
|---|---|---|
| add | linked | an update of the user's link-field values against the fresh `link_version` (no fields: nothing; the notice says it is already linked) |
| update | not linked | an add with the user's values |
| update | linked, new version | an update against the fresh version |
| remove | not linked | nothing (already gone; the notice says so) |
| remove | linked, new version | a remove with the fresh version |

**Delete** (only when `TableView.can_delete`; never on pickable tables):

- Starting it calls `delete_preview` (busy: "Checking…"), then opens the
  delete confirmation with the impact; it starts on Cancel. If the object
  has a dirty draft, it adds "Your unsaved changes to it are discarded."
- `impact.blocked`: the confirmation says why, lists the blocking groups
  first, and offers only Close.
- Confirm calls `delete(table, key, impact.version, impact.token, snapshot.config.hash)`.
- `ImpactChanged`: stays open with the new impact, a notice "What this
  delete affects changed. Review it again.", and the next confirm uses the
  new token. `DeleteBlocked`: switches to the blocked state. Its impact
  may be `blocked: true` with no group marked `blocks` (an unpredicted FK
  failure, or groups past the cap; dbmanager M9's doc). Then the blocked
  state shows a generic "This object cannot be deleted." line from
  `text.rs` instead of a blocking group. `Conflict`:
  closes; the sheet shows the current object with "“<label>” was changed by
  another program. Nothing was deleted. Review it and try again."
  `ConfigChangedOnDisk`: re-read the snapshot, run the preview again, stay
  open with "The config changed and was reloaded. Review the delete again."
- Success: the place is **replaced** by the table; notice "Deleted “<label>”
  and N related rows." plus the backup line when `backup_created`; the list
  is re-read.

### What to build

- `Core::delete_preview`, `Core::delete` (on `ConfigChangedOnDisk`, reload
  the config before returning, as `save` does); backend trait methods.
- `draft.rs`: `StagedLinks` (pure): stage/unstage add, remove, update; link
  field edits with validation; `to_changes() -> Vec<LinkChange>`;
  `rebase_link(staged, now: Option<M2mOption>) -> Rebased` per the table;
  removal summary text inputs.
- Model additions:

  ```rust
  impl DbmanagerModel {
      pub fn toggle_link(&mut self, relation: &str, other: &ObjectKey, cx);
      pub fn edit_link_field(&mut self, relation: &str, other: &ObjectKey, field: &str, input: EditorInput, cx);
      pub fn undo_link(&mut self, relation: &str, other: &ObjectKey, cx);
      pub fn staged_rows(&self, relation: &str) -> Vec<StagedRow>;     // with on_page / off_page
      pub fn link_counts(&self, relation: &str) -> LinkCounts;
      pub fn link_conflict(&self, relation: &str) -> Option<&LinkConflict>;
      pub fn reapply_link(&mut self, relation: &str, cx);  pub fn discard_link(&mut self, relation: &str, cx);
      pub fn request_delete(&mut self, cx);                              // the open object
      pub fn request_delete_of(&mut self, table: &str, key: ObjectKey, label: &str, cx); // a list item that is not the open object (Delete key)
      pub fn delete_state(&self) -> Option<&DeleteState>; // Checking | Confirm(impact) | Blocked(impact) | Deleting
      pub fn confirm_delete(&mut self, cx);  pub fn cancel_delete(&mut self, cx);
  }
  ```

- `request_delete_of` runs the same flow for an object that is not open
  (no draft, so no "unsaved changes" line); on success the list re-reads and
  the place stays. Impact changed and delete done raise the `DeleteNotice`
  focus hint.
- Copy in `text.rs` for every sentence above, the impact group lines, and
  "Will be added", "Will be removed", "Not saved yet".
- Fixtures: staged adds/removes with link fields, a link conflict, a delete
  confirmation, and for the `dbmanager-delete-blocked` scenario a config in
  which `publisher` is editable with `allow_delete = true` while a book still
  references the publisher being deleted (a blocked delete).

**Fixture timing rule (planner, 2026-11-22, after M4).** The harness's
`Step::WaitIdle` is a short fixed settle, not idle detection: it does not
await `Worker` jobs, `spawn_thread` jobs or async loads. So every fixture
backend state a scenario shows must answer **synchronously** (the backend
method returns at once, so the Worker call resolves inside the settle), or
the scenario that shows it must add an explicit `Step::Wait(ms)` long
enough for the job. Fixtures that simulate long work (progress, a running
step) are timed on purpose; document each fixture's timing in its doc
comment so the scenario author knows which `Wait` it needs.

**From the M12a review (2026-11-26).** `DbmanagerError::conflict_link` is
`Option<Box<LinkRef>>`; match on it accordingly.

### Rules that bind this task

- **Destructive operations are never silent:** delete is previewed and
  confirmed with its impact (including hidden tables as counts and link
  data); removing a link with fields is staged and visible before Save; the
  first write takes a backup; a failure rolls back and says nothing changed.
- No automatic retry after `ConfigChangedOnDisk`.
- Link edits are strict: they carry `link_version`.
- No blocking I/O on the UI thread. Tests in temp dirs only.
- Ownership as before; no visual decisions.
- fmt clean; clippy pedantic `-D warnings`; scoped `#[expect]` only.

## Files

Edit: `crates/fm-dbmanager/src/tool/{core.rs,tests.rs}`,
`apps/filemanager/src/tools/dbmanager/{draft.rs,backend.rs,model.rs,text.rs,fixtures.rs,tests.rs}`.

Do not touch: `crates/fm-dbmanager/src/{session,rules,model}/`, other
tools, `docs/plans/`.

## Acceptance criteria

- The API above exists with doc comments.
- Every behaviour above has a test; each rebase table row has its own case.
- fmt, clippy pedantic and `cargo test --workspace` pass on both jobs.

## Tests proving completion

`tool/tests.rs`: `delete_stale_config_hash_reloads_and_refuses`,
`save_with_links_through_core`.

`draft.rs`: `toggle_twice_undoes_exactly`, `add_prefills_defaults`,
`update_carries_link_version`, `create_only_link_field_editable_only_on_add`,
`link_field_validation`, `rebase_add_now_linked`, `rebase_update_now_unlinked`,
`rebase_update_new_version`, `rebase_remove_now_unlinked`,
`rebase_remove_new_version`.

`tests.rs` (`#[gpui::test]`, temp database):
`staged_rows_off_page_shown_once`, `counts_line_reflects_staging`,
`removal_with_fields_is_announced_before_save`,
`save_sends_links_with_fields_in_one_request`, `link_errors_attach_to_rows`,
`link_conflict_keeps_other_edits_and_offers_reapply`,
`delete_previews_then_confirms`, `blocked_delete_offers_only_close`,
`impact_changed_keeps_dialog_with_new_token`, `delete_conflict_shows_current`,
`delete_config_changed_reruns_preview`, `delete_success_replaces_place_and_notices`,
`delete_with_dirty_draft_warns`, `pickable_tables_never_delete`,
`request_delete_of_list_item_keeps_place`, `delete_notice_hint_is_raised`.

## Out of scope

- The config watcher and Edit config (part D). Views and scenarios.

## Risks

- Strict link conflicts cost the user a round trip; they protect link data.
- Delete impact cannot see what triggers do; the domain flags their presence
  and the copy shows it.
