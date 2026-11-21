# M13 — Database Manager: tool documentation, user manual chapter, screenshots, Windows release check

Status: awaiting approval. **Depends on the UI plan:** runs only after every
Database Manager view and scenario U task has landed.
Project: gpui-migration   Depends on: M12d and M8 (landed), every Database Manager U task (landed)

## Goal

Document the finished Database Manager for maintainers and users, add its
screenshots from the GPUI harness, and prove the portable Windows exe works
end to end with a real SQLite file.

## Context

The Database Manager is complete in the GPUI app: the domain crate
`crates/fm-dbmanager` (open, introspection, config overlay, validation incl.
durations, reads, save with backup, many-to-many link writes with link
fields, delete with impact, and `tool/`: the app-wide config, recent
databases, snapshot, errors), and in `apps/filemanager/src/tools/dbmanager/`
the backend, model (`model.rs`, `draft.rs`), copy (`text.rs`), fixtures and
views. The config format is documented in `docs/dbmanager-config.md`;
examples are in `examples/dbmanager/`.

Documentation pattern: `docs/schedule.md` and `docs/pipeline.md` (a
maintainer's flow: what the user does, what each model operation does, where
the logic lives), `docs/manual/user-manual.md` (end-user chapters with
screenshots). Screenshots come from the Rust harness:
`cargo run -p filemanager --features shoot --bin filemanager-shoot -- --out <dir> [--only …]`
(use `-p filemanager-gpui` if the cutover has not happened yet), on Windows
at 100% display scale, or from the CI `screenshots` artifact; scenarios are
in `apps/filemanager/src/shoot/scenarios/`; the test
`shoot::tests::every_manual_image_has_a_scenario` checks the manual against
them.

Facts to document (all implemented; read the code to confirm wording):

- Opening: an absolute path to an existing SQLite file, never created; drop
  one database file; recent databases; the most recent reopens on start
  (if missing, the app starts empty and says why). Names that are not valid
  Unicode cannot be picked or dropped; FileManager says so.
- **The config:** one file for the whole app at
  `%ProgramData%\FileManager\dbmanager.toml`, never chosen by the user; the
  app only reads it and reloads it automatically (unsaved edits are kept and
  re-checked; a save prepared under the old rules is refused once and can be
  repeated); missing → everything editable by default with a notice;
  unusable (syntax error, unsupported version, unreadable) → everything
  read-only with the reason; developer mode (Ctrl+Alt+Shift+D) offers **Edit
  config**, creating a commented starter file if there is none.
  Administrators deploy the file: document where and how.
- **Pickable tables:** not listed; offered in pickers; opened read-only from
  links.
- **Closing the window with unsaved edits asks first**, whatever tool shows.
- Defaults: anything the config does not mention is editable (except keys,
  generated, BLOB, untyped and composite-FK columns); create on by default;
  delete off unless a table sets `allow_delete = true`; `[defaults]` can make
  everything read-only unless stated.
- Read-only database file or folder: browse only.
- Durations: stored as whole numbers of the field's unit; typed and shown as
  `1h30m` (also `1h 30m`, or a bare number of the unit).
- Many-to-many links with link fields: staged until Save; removal of a link
  with fields is shown before Save; conflicts on a link are reported and can
  be re-applied.
- Writes: one transaction per save or delete; a failure changes nothing.
- **Recovery:** before the first write of each opened session, a backup
  `<file>.<UTC timestamp>.bak` is written beside the database; never
  replaced, never deleted by the app; if it cannot be written, nothing is
  written. To restore: close the database (or the app), replace the file with
  the backup, reopen.
- Concurrency: another program's changes appear on the next read; editing a
  changed object is a conflict; a schema change asks for a reload; a locked
  database reports busy.
- Delete: the confirmation shows the impact (cascades, set null, blocked,
  links and link data; hidden tables as counts); blocked deletes cannot be
  confirmed.
- The keyboard shortcuts the views implement (read them from the view code
  and its shortcuts help).

### What to do

- Create `docs/dbmanager.md`: the flow (open → table → object → edit or
  create → links → delete), a table of the model's public operations
  (`apps/filemanager/src/tools/dbmanager/model.rs`) and what each does, the
  22 error kinds and when each occurs, the recovery story, the concurrency
  rules, and "where the logic lives" (module map of `crates/fm-dbmanager/src/`
  and the tool's app modules).
- Edit `docs/manual/user-manual.md`: a "Database Manager" chapter (after
  Pipeline, before "Entering paths") and its Contents entry, covering the
  facts above, with the images the dbmanager scenarios produce
  (`dbmanager-*.png`), generated and committed into `docs/manual/images/`.
- Edit `CLAUDE.md` (Status: the Database Manager is complete; Layout:
  `docs/dbmanager.md`) and `README.md` if it lists the tools.
- Add to `docs/tarpack-e2e.md`'s sibling a manual check,
  `docs/dbmanager-e2e.md`: on Windows 11, with the release exe from CI
  copied alone into a folder, open a copy of a database built from
  `examples/dbmanager/schema.sql` with `example.toml` deployed to
  `%ProgramData%\FileManager\`; edit, save (backup appears), add and remove a
  link with fields, delete with its impact, edit the config while the app
  runs (it reloads), close with unsaved edits (it asks). Record the build
  number and results in the PR.

### Rules that bind this task

- Documentation and screenshots only. Do not change Rust code, views or
  scenarios; a bug found while documenting is reported, not fixed here.
- Screenshots use fixture backends only; never real user files.
- The release exe never contains the harness.
- Do not touch `docs/plans/`.

## Files

Create: `docs/dbmanager.md`, `docs/dbmanager-e2e.md`,
`docs/manual/images/dbmanager-*.png`.

Edit: `docs/manual/user-manual.md`, `CLAUDE.md`, `README.md` (if needed).

## Acceptance criteria

- `docs/dbmanager.md` names every public operation of the dbmanager model
  and every error kind of `DbmanagerErrorKind`.
- `every_manual_image_has_a_scenario` and `no_orphan_manual_images` pass
  with the new chapter.
- The manual chapter states the config location, the automatic reload, the
  backup and restore steps, and the close prompt.
- The Windows end-to-end check is recorded in the PR.
- CI green on both jobs.

## Tests proving completion

- `cargo test -p filemanager --features shoot` (or `-p filemanager-gpui`):
  the two manual-image tests.
- The manual Windows check in `docs/dbmanager-e2e.md`, results in the PR.

## Out of scope

- Any code change; the config reference (`docs/dbmanager-config.md`)
  except a link to the new chapter.

## Risks

- The scenario list comes from the UI tasks; a missing image is a gap in a
  UI task, reported, not patched here.
