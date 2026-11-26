# M12b — Database Manager, part B: drafts, validation while typing, save, create, conflicts, close guard

Status: approved (2026-11-21)
Project: gpui-migration   Depends on: M12a (landed)

## Goal

Add field editing to the Database Manager's GPUI view-model: one draft at a
time held in the model, validated with `fm_dbmanager::rules::validate` as
the user types, saved explicitly in one transaction, with create, the
object-level conflict panel, the "leave a dirty draft?" confirmation, and
registration with the app's close guard.

## Context

Read first: `crates/fm-dbmanager/src/tool/` (M12a: `Core`, `DbmanagerSnapshot`,
`DbmanagerError` and its 22 kinds), `apps/filemanager/src/tools/dbmanager/`
(M12a: `backend.rs`, `model.rs` with `Place` and history, `text.rs`,
`fixtures.rs`), `crates/fm-dbmanager/src/session/` (`save` and its
`SaveRequest`/`FieldChange`/`SaveResult` types from dbmanager M7; the
request carries `config_hash`; the first write of a session takes a backup
`<file>.<UTC stamp>.bak` and reports `backup_created`), `rules/` (`validate(rule, input) -> Result<DbValue, Vec<FieldIssue>>`,
`duration::{parse, format}`), `value.rs` (`DbValue`: integers, reals and
durations as strings, which is what text inputs produce), `model/views.rs`
(`FieldView`: `kind`, `rule`, `mode`, `nullable`, `default_value`,
`has_default`, `required_on_create`, create-only), and `ObjectView`
(`version`, values with `FieldValue.problem`). The app's close guard:
`ToolHooks::unsaved_work` in `apps/filemanager/src/tools/mod.rs`, which the
Shell asks when the window is closed (`Shell::close_request`).

**The TypeScript validation mirror is dropped**: only Rust validates, and
the model calls `validate` directly while the user types.

### The confirmed behaviour (restated)

- **One draft at a time:** the open object's (or new object's) edits, keyed
  by column. A field is **changed** when its editor text, converted to a
  `DbValue` (and normalised by `validate` when valid), differs from the
  stored value: typing "1h 30m" over a stored "1h30m" is not a change.
  **Dirty** = any changed field (part C adds staged link changes).
- Editors produce: integer and real → trimmed text, empty = not set (null)
  when nullable; text → untrimmed; duration → text parsed with the field's
  unit, shown in canonical form after a valid entry loses focus
  (`duration::format`); bool → `1`/`0`, or null for "not set" on a nullable
  field; allowed-values → the chosen value or null ("Not set"); FK →
  a staged reference change (key or null). Blobs and "never editable"
  fields have no editor.
- **Validation** runs on every change; issues are **shown** after the field
  first loses focus or after a Save attempt. Stored-value problems
  (`FieldValue.problem`) never block saving other fields; editing the field
  replaces the value.
- **Escape** in a changed field reverts that field; **Discard changes**
  reverts everything without asking.
- **Leaving a dirty draft asks first:** opening another object, following
  a reference, Back/Forward, choosing a table, New, opening a database
  (Open, Recent, drop) and Close database. The question: "Discard unsaved
  changes?" / "Your changes to “<label>” are not saved. If you leave now,
  they are discarded." / **Keep editing** / **Discard changes**.
- **Save** (Ctrl+S or Save): if any changed field has issues, or a
  required-on-create field is missing, nothing is sent and every issue is
  shown (focus to the error summary). Otherwise the request has only the
  changed fields and `config_hash` = the snapshot's `config.hash` the draft
  was last checked against. While saving: editors read-only, navigation and
  other writes refused, the drop target disabled with "Wait for the save to
  finish". **Success:** the sheet takes `result.object`, the draft clears,
  announcement "Saved." (create: "Created <label>." and the place is
  **replaced** by the new object), the current list page and open
  checklists are re-read, and when `backup_created` the backup notice shows
  and the snapshot is re-read.
- **Errors on save:** `ValidationFailed` → the backend's issues replace the
  inline ones on those fields until edited again; `Conflict` (no link) →
  the conflict panel; `ConfigChangedOnDisk` → the backend already reloaded:
  re-read the snapshot, re-check the draft as for a reload (part D adds the
  watcher; the re-check itself is built here), and show "The config changed
  while you were editing and was reloaded. Nothing was saved. Check your
  changes and save again." — **never retry automatically**; `SchemaChanged`
  → a banner offering Reload; `Busy` → a banner offering Try again (repeats
  the same request); anything else → the error banner with details.
- **Conflict panel:** "“<label>” was changed by another program after you
  opened it. Nothing was saved."; per changed field "Yours: …" and "Now: …";
  "N other fields changed too" for unedited fields that differ;
  **Re-apply my changes** (rebase the draft onto `current`; the user saves
  again) and **Discard my changes** (load `current`); Save is refused until
  one is chosen, reason "Resolve the conflict first."
- **Create** (only when `TableView.can_create`): pushes `Place::New`;
  editable and create-only fields are editors; read-only fields show "Set
  automatically" (or their known default); fields with `default_value` are
  prefilled; `has_default` without a known value shows "Database default";
  Cancel discards and goes Back; Create saves. The button and title read
  "New <objectName>" when `TableView.object_name` is set, else "New" /
  "New object".
- **Re-check after a reload** (used by `ConfigChangedOnDisk` here and the
  watcher in part D): re-read the snapshot and the object. Same version →
  rebase the draft: edits to fields that are gone or no longer editable are
  dropped and listed; a different version → the conflict panel; the object
  or table is gone → the draft is dropped with a notice. Notices: "The
  config was reloaded." / "The config was reloaded. These changes no longer
  apply and were removed: <names>."
- **Closing the window with a dirty draft asks:** `unsaved_work` returns
  one item, "Database Manager: your changes to “<label>” are not saved.",
  whatever tool is showing.

### What to build

- `fm_dbmanager::tool::Core::save(request)`: forwards to the session; on
  `ConfigChangedOnDisk` calls `reload_config()` **before** returning the
  error; maps errors as in M12a. Add it to the backend trait.
- `apps/filemanager/src/tools/dbmanager/draft.rs`: `Draft` (pure, no GPUI):
  per-field editor text, the stored value, `changed()`, `issues()`,
  `shown()` (blurred or save attempted), `revert(field)`,
  `revert_all()`, `to_request(config_hash) -> SaveRequest` with only changed
  fields, `rebase(new_view, new_fields) -> Rebase { dropped: Vec<String> }`,
  and the conflict diff (`yours`, `now`, `others_changed`). Unit-tested
  without GPUI.
- Model additions:

  ```rust
  impl DbmanagerModel {
      pub fn draft(&self) -> Option<&Draft>;  pub fn dirty(&self) -> bool;  pub fn dirty_count(&self) -> usize;
      pub fn edit(&mut self, field: &str, input: EditorInput, cx);
      // reference picker, held in the model with its own list query (it never
      // touches the table's list state of part A):
      pub fn open_picker(&mut self, field: &str, cx);
      pub fn picker(&self) -> Option<&PickerState>;            // field, search text, items, total, loading, error
      pub fn picker_search(&mut self, text: &str, cx);  pub fn picker_more(&mut self, cx);  // 25 more
      pub fn pick(&mut self, key: ObjectKey, cx);              // stages Reference { key }, closes, hint PickerTrigger(field)
      pub fn clear_reference(&mut self, field: &str, cx);      // stages Reference { key: None } (nullable only)
      pub fn close_picker(&mut self, cx);                      // hint PickerTrigger(field)
      pub fn blur(&mut self, field: &str, cx);  pub fn revert_field(&mut self, field: &str, cx);
      pub fn discard(&mut self, cx);
      pub fn save(&mut self, cx);  pub fn saving(&self) -> bool;
      pub fn conflict(&self) -> Option<&ConflictPanel>;  pub fn reapply(&mut self, cx);  pub fn discard_mine(&mut self, cx);
      pub fn new_object(&mut self, table: &str, cx);  pub fn cancel_new(&mut self, cx);
      pub fn leave_request(&self) -> Option<&LeaveRequest>;     // the pending navigation the dialog asks about
      pub fn confirm_leave(&mut self, cx);  pub fn cancel_leave(&mut self, cx);
      pub fn notice(&self) -> Option<&Notice>;  pub fn dismiss_notice(&mut self, cx);  // backup, reload, saved
      pub fn recheck_after_reload(&mut self, cx);               // pub(crate) is enough if only part D calls it
  }
  ```

  Every navigation method of M12a goes through `leave_request` when the
  draft is dirty. History gains **replace** (after create).
- `pub enum EditorInput { Text(String), Bool(Option<bool>), Choice(Option<String>) }`:
  `Text` for integer, real, text and duration editors (the draft converts it
  as above); `Bool(None)` = not set on a nullable bool; `Choice` for
  allowed-values selects (`None` = "Not set"). References change only
  through the picker methods.
- The picker lists the FK's target table with `list(target, ListQuery { search, offset, limit: 25 })`,
  25 per page with "show more"; only the newest picker request applies;
  pickable targets are listed. Picking or clearing makes the field changed
  like any edit.
- A refused save raises the `ErrorSummary` focus hint; a banner error
  `ErrorBanner`; create success `SheetHeading`.
- `ToolHooks::unsaved_work` for the tool.
- Copy in `text.rs` for every sentence above.
- Fixtures: a dirty draft, a validation error, a conflict, a create form,
  a saved-with-backup notice, an open picker (for the UI's scenarios); and,
  for the `dbmanager-config-reloaded` scenario, a fixture step that rewrites
  the temp config so book `pages` becomes read-only and then injects the
  file-changed signal (part D wires the signal; until then the fixture calls
  `recheck_after_reload` directly).

**Fixture timing rule (planner, 2026-11-22, after M4).** The harness's
`Step::WaitIdle` is a short fixed settle, not idle detection: it does not
await `Worker` jobs, `spawn_thread` jobs or async loads. So every fixture
backend state a scenario shows must answer **synchronously** (the backend
method returns at once, so the Worker call resolves inside the settle), or
the scenario that shows it must add an explicit `Step::Wait(ms)` long
enough for the job. Fixtures that simulate long work (progress, a running
step) are timed on purpose; document each fixture's timing in its doc
comment so the scenario author knows which `Wait` it needs.

**From the M12a review (2026-11-26).**

- `DbmanagerError::conflict_link` is `Option<Box<LinkRef>>`.
- **A reload after a failed first save must not lose the backup
  announcement (decided: pass it through).**
  - Today `reload()` calls `Session::adopt_backup(path)`, which marks the
    backup as announced. So if the first save took the backup and failed,
    and then a reload happened, the next successful save never reports
    `backup_created`.
  - Carry the session's announced state across the reload: add
    `Session::backup_announced() -> bool`, and give `adopt_backup` the
    announced state, as `adopt_backup(path, announced: bool)`. This is the
    only `crates/fm-dbmanager/src/session/` change this task may make,
    with its test in the crate.
  - `Core::reload` passes the old session's state through.
  - Test: `reload_after_failed_first_save_still_announces_backup`.

### Rules that bind this task

- **Destructive operations are never silent:** saves are explicit and
  transactional; the first write of a session takes a backup (the backend
  refuses the write if the backup fails); a failure rolls everything back
  and says nothing was changed; leaving or closing with a dirty draft asks.
- A write never applies rules the user has not seen: no automatic retry
  after `ConfigChangedOnDisk`.
- Validation is `fm_dbmanager::rules::validate` only; no second
  implementation.
- No blocking I/O on the UI thread (saves and backups on the `Worker`).
- Tests use temp dirs and temp config paths only.
- Ownership as in M12a; no visual decisions (the conflict panel, dialogs and
  tags are the UI's).
- fmt clean; clippy pedantic `-D warnings`; scoped `#[expect]` only.

## Files

Create: `apps/filemanager/src/tools/dbmanager/draft.rs`.

Edit: `crates/fm-dbmanager/src/tool/{core.rs,tests.rs}`,
`apps/filemanager/src/tools/dbmanager/{backend.rs,model.rs,text.rs,fixtures.rs,mod.rs,tests.rs}`,
`CLAUDE.md` (Status).

Do not touch: `crates/fm-dbmanager/src/{session,rules,model}/` (report a
needed domain change instead; the one exception is the `adopt_backup`
announced flag above), other tools, `docs/plans/`.

## Acceptance criteria

- The model API above exists with doc comments; `Draft` is GPUI-free.
- Every behaviour in "The confirmed behaviour" has a test below.
- fmt, clippy pedantic and `cargo test --workspace` pass on both jobs.

## Tests proving completion

`crates/fm-dbmanager/src/tool/tests.rs`: `stale_config_hash_reloads_and_refuses`,
`changed_file_without_reload_refuses_and_reloads`, `retry_with_new_hash_succeeds`,
`backup_path_after_first_save`, `reload_keeps_backup`, `open_again_resets_backup`
(count `.bak` files in the temp dir).

`draft.rs` unit tests: `canonical_duration_is_not_a_change`,
`empty_nullable_is_null`, `text_is_not_trimmed`, `issues_shown_after_blur_or_save`,
`revert_field_and_all`, `request_has_only_changed_fields`,
`rebase_drops_gone_and_read_only_fields`, `conflict_diff_yours_now_others`.

`tests.rs` (`#[gpui::test]`, fixture backend over a temp database):
`edit_validates_on_change`, `save_refused_with_issues_shows_all`,
`required_on_create_missing_refuses`, `save_sends_changed_fields_with_hash`,
`saving_refuses_navigation_and_disables_drop`,
`save_success_clears_draft_and_rereads`, `first_save_shows_backup_notice`,
`server_validation_replaces_inline_until_edited`,
`conflict_panel_reapply_and_discard`, `config_changed_on_save_rechecks_without_retry`,
`schema_changed_and_busy_banners`, `busy_try_again_repeats_request`,
`dirty_navigation_asks_and_confirm_discards`, `cancel_leave_keeps_draft`,
`create_prefills_defaults_and_replaces_place`, `new_button_noun_uses_object_name`,
`cancel_new_goes_back`, `edit_inputs_convert_per_kind`,
`picker_has_its_own_query_and_keeps_list_state`, `pick_stages_reference_and_hints_trigger`,
`clear_reference_only_when_nullable`, `unsaved_work_reports_draft_for_close_guard`,
`close_guard_holds_window_with_dirty_draft` (through the Shell and
`simulate_close`).

## Out of scope

- Many-to-many link staging, link fields, link conflicts and delete (part C).
- The config watcher and Edit config (part D). Views and scenarios.

## Risks

- A backup of a large database takes time; it runs on the Worker and the UI
  shows saving.
- Rebase logic is subtle; keep it in `Draft` with unit tests, not in the
  entity.
