# M12e — Database Manager: view copy into `text.rs`; `dismiss_state_warning` and `set_shortcuts_open`

Status: approved (2026-11-27; from the dbmanager U2 review). M12e landed as f9549e9 and is approved on condition of the P2 in "Review follow-up"; the next run does only that section.
Project: gpui-migration   Depends on: M12d (landed), dbmanager U2 (landed)

## Review follow-up (2026-11-28)

**The next run does only this section.**

### P2: guard the Escape key, not the buttons

- M12e put `!self.shortcuts_open` guards inside `dismiss_notice` and
  `cancel_leave`. That disables the notice's Dismiss button and the leave
  dialog's **Keep editing** while the shortcuts help is open. If
  `shortcuts_open` ever stuck at `true`, the only answer left to the leave
  dialog would be Discard, and the user's draft would be lost.
- Follow the Tar Packager's pattern (`tools/tarpack/model.rs`, around lines
  1717–1760), where only the **shortcut path** is guarded:
  - remove both guards: `dismiss_notice` and `cancel_leave` always work;
  - add one Escape entry point, `pub fn escape(&mut self, cx)` (or a
    `shortcut`/`run_shortcut` pair, as in the Tar Packager). It does
    nothing while `shortcuts_open`. Otherwise it cancels a pending leave
    first, else dismisses the notice;
  - point `escape_ignored_while_shortcuts_open` at `escape()`;
  - add `buttons_work_while_shortcuts_open`: with the help open,
    `cancel_leave` and `dismiss_notice` still act.
- The views bind the Escape **key** to `escape()` (dbmanager U12, and U2's
  follow-up if it wires Escape), never to `dismiss_notice` or
  `cancel_leave` directly.

Acceptance: the two tests pass. fmt (linux job), clippy pedantic (with and
without `--features shoot`), and `cargo test --workspace` pass.

## Goal

Move the Database Manager view's copy into the tool's `text.rs`, unchanged,
and add two small model methods the views need: `dismiss_state_warning` and
`set_shortcuts_open`.

## Context

The tool's copy belongs in `apps/filemanager/src/tools/dbmanager/text.rs`,
which the backend implementer owns and unit-tests. Views never write copy.
The dbmanager U2 view put some copy in `tools/dbmanager/view/copy.rs`. The
model is `tools/dbmanager/model.rs` (`DbmanagerModel`). Compare the Tar
Packager's model, which has `dismiss_state_warning` and
`set_shortcuts_open(bool)` (Escape does nothing while the shortcuts help is
open).

### What to do

1. **Move into `text.rs`, unchanged** (same text, same signatures; pure
   functions take plain data):
   - `RECENT`;
   - `READ_ONLY`;
   - the config line strings;
   - the severity words;
   - `backup_line`;
   - `problems_summary`;
   - `position`.

   The view imports them; remove the copies in `view/copy.rs`. Move their
   tests, if any, to `tools/dbmanager/tests.rs` under the same names.
2. **The missing-config line (dbmanager U2's item 2, decided 2026-11-27:
   the banner is dropped).** Change the value of `text::CONFIG_MISSING` to
   "No config file: everything is editable unless the database prevents
   it." No new constant is needed. Update any test that pins the old text.
3. **`dismiss_state_warning(&mut self, cx)`:** hides the current
   `state_warning` text until a different one arrives, as the Tar
   Packager's does. Test: `state_warning_dismissal_sticks_until_it_changes`.
4. **`set_shortcuts_open(&mut self, open: bool, cx)`:** while open, Escape's
   model actions (dismiss a result or notice, cancel a pending leave) do
   nothing, as in the Tar Packager. Test: `escape_ignored_while_shortcuts_open`.

### Rules that bind this task

- No visible text change for item 1.
- Views call only the model.
- fmt (linux job), clippy pedantic (with and without `--features shoot`),
  and `cargo test --workspace`.

## Files

Edit:

- `apps/filemanager/src/tools/dbmanager/{text.rs,model.rs,tests.rs}`;
- `apps/filemanager/src/tools/dbmanager/view/copy.rs` (removals) and the
  view files that import from it (imports only).

Do not touch: layout, other tools, `docs/plans/`.

## Acceptance criteria

- `view/copy.rs` holds no user-facing string, or is deleted.
- The two methods exist, each with a doc comment and a test.
- All tests pass.

## Tests proving completion

The moved tests, `state_warning_dismissal_sticks_until_it_changes`, and
`escape_ignored_while_shortcuts_open`.

## Out of scope

- The views that call the new methods: dbmanager U2's follow-up and
  dbmanager U12.
