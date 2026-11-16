# M12 — Tool documentation, user manual, and Windows release check

Status: awaiting approval — **depends on UI plan**: runs only after every
Database Manager UI task (`U<n>`) has landed. (Renumbered from M11 and
revised 2026-11-16: editable defaults, durations, link fields.)
Project: dbmanager   Depends on: M10 (landed), M11 (landed), all dbmanager U
tasks (landed)

## Goal

Document the finished Database Manager for maintainers and users, add its
screenshots, and prove the portable Windows exe works end to end with a real
SQLite file.

## Context

The Database Manager (`dbmanager`) is complete: the domain crate
`crates/fm-dbmanager` (open, introspection, config overlay, validation incl.
durations, reads, save with backup, many-to-many link writes with link fields,
delete with impact), the Tauri module
`apps/desktop/src-tauri/src/tools/dbmanager/`, the client
`apps/desktop/src/lib/dbmanager.ts`, the validation mirror
`apps/desktop/src/lib/dbmanagerValidation.ts`, and the views in
`apps/desktop/src/tools/dbmanager/`. The config format is documented in
`docs/dbmanager-config.md`; the example schema and config are in
`examples/dbmanager/`.

The existing tools set the documentation pattern: `docs/schedule.md` and
`docs/pipeline.md` (a maintainer's flow description: what the user does, what
each command does, where the logic lives), `docs/manual/user-manual.md`
(end-user chapters with screenshots), and `docs/manual/screenshots/`
(`shoot.js` drives the real frontend in Chromium; `mock.js` replaces the Tauri
backend with canned responses per scenario; see its `README.md`).

Facts to document (all already implemented; read the code to confirm
wording):

- Opening: absolute path to an existing SQLite file, never created; optional
  config; drop a `.db` and/or `.toml`; recent pairs; the most recent pair is
  reopened on start.
- **Defaults:** anything the config does not mention is editable (except
  keys, generated, BLOB, untyped and composite-FK columns); creating is on by
  default, deleting is off unless a table sets `allow_delete = true`; a
  one-line `[defaults]` makes everything read-only unless stated.
- Read-only database (the file or folder is read-only): browse only.
- Config diagnostics never block browsing; errors make the affected entry
  read-only; a config with a syntax error, an unsupported version, or one that
  cannot be read makes everything read-only until fixed.
- **Durations:** stored as whole numbers of the field's unit; typed and shown
  as `1h30m` (also `1h 30m`, or a bare number of the unit).
- **Many-to-many links with link fields:** ticking adds a link whose fields can
  be set; a link's fields can be edited; unticking removes the link and its
  field values (staged until Save; covered by the backup); a link changed by
  another program meanwhile is reported as a conflict.
- Writes: one transaction per save or delete; a failure changes nothing.
- **Recovery:** before the first write of each opened session, a backup
  `<file>.<UTC timestamp>.bak` is written beside the database (online backup
  API; never replaces a file; never deleted by the app). If it cannot be
  written, nothing is written. To restore: close the tool (or the app),
  replace the database with the backup copy, reopen.
- Concurrency: another program's changes appear on the next read; editing an
  object that changed meanwhile reports a conflict; a schema change or a
  config edit asks for a reload; a locked database reports "busy".
- Delete: confirmation shows the impact (cascades, set null, blocked, links
  and whether link data is lost), hidden tables as counts; blocked deletes
  cannot be confirmed.

Rules that bind this task:

- Documentation and screenshot tooling only. Do not change Rust, `lib/`, or
  the views. If the docs reveal a bug, report it; do not fix it here.
- `docs/manual/screenshots/mock.js`: add Database Manager scenarios to the
  scenario switches (do not edit the frontend to make a shot work).
- Do not touch anything under `docs/plans/`.

## Files

- Create `docs/dbmanager.md`: the flow (open → table → object → edit/create
  → links → delete), the command table (the twelve `dbmanager_*` commands
  and what each returns), the error kinds and when each occurs, the recovery
  story, the concurrency rules, and "where the logic lives" (module map of
  `crates/fm-dbmanager/src/`).
- Edit `docs/manual/user-manual.md`: a "Database Manager" chapter (after
  Pipeline, before "Entering paths") and its entry in Contents: opening a
  database and config, browsing tables and objects, following links and going
  back, editing and saving, booleans, durations, many-to-many checklists and
  link fields, creating,
  deleting and its confirmation, backups and restoring, messages, a short
  "Writing a config" section pointing to `docs/dbmanager-config.md`, and the
  keyboard shortcuts the views implement.
- Edit `docs/manual/screenshots/mock.js` and `shoot.js`: scenarios and shots
  named `dbmanager-*`, at least: `dbmanager-empty` (nothing open),
  `dbmanager-tables` (table list), `dbmanager-object` (an object form with a FK
  link, a bool checkbox, an enum and a duration), `dbmanager-m2m` (checklist),
  `dbmanager-link-fields` (a contribution link with role and position being
  added),
  `dbmanager-validation` (inline issues), `dbmanager-delete` (confirmation
  with cascades), `dbmanager-delete-blocked`, `dbmanager-diagnostics` (config
  warnings/errors), `dbmanager-backup` (the backup notice). Generate the PNGs
  into `docs/manual/images/` with the commands in the screenshots README.
- Edit `CLAUDE.md`: Status (the Database Manager is complete), Layout
  (`docs/dbmanager.md`), and any command or prerequisite the work added.
- Edit `README.md` if it lists the tools.

## Acceptance criteria

- `docs/dbmanager.md` lists every `dbmanager_*` command registered in
  `apps/desktop/src-tauri/src/tools/mod.rs` and every `DbmanagerErrorKind`
  (compare against `apps/desktop/src/lib/generated/DbmanagerErrorKind.ts`).
- The manual chapter covers every item in the Files list, with the
  screenshots embedded, and its text matches the UI's actual labels (check
  against `apps/desktop/src/tools/dbmanager/`).
- `node docs/manual/screenshots/shoot.js dbmanager` regenerates all
  `dbmanager-*` images without console errors.
- **Windows end-to-end check** (manual; record the result in the hand-off): on
  the CI artifact `FileManager-<version>-x64.exe` on a Windows machine
  without developer tools:
  1. the Windows CI job is green, including the dumpbin check (no
     `sqlite3*.dll`, no CRT DLLs);
  2. open `examples/dbmanager/schema.sql` loaded into a fresh `.db` (create it
     with any SQLite tool beforehand) with `example.toml`; no diagnostics;
  3. edit an author, save: a `.bak` appears beside the database; a second save
     adds none;
  4. create a book with an author picked from the list, two genres ticked, a
     contributor added as translator with position 1, and an audiobook length
     typed as `1h 30m` (shown afterwards as `1h30m`);
  5. follow the book's author link and go back;
  6. delete an author: the confirmation lists books, contributions (with
     link data lost), genre links and loans; confirm; they are gone;
  7. try to delete a publisher with books (with a config allowing it): the
     confirmation shows it is blocked;
  8. edit the config in an editor: the app asks to reload; saving is refused
     until it does; introduce a TOML syntax error and reload: everything is
     read-only and the error is shown;
  9. open the database in another SQLite tool, hold a write transaction, save
     in the app: "busy" after a few seconds, nothing changed;
  10. restore the backup by copying it over the database and reopening: the
      data is as before the first save.
- `npm run typecheck && npm run lint && npm run test` and `cargo test
  --workspace` pass (nothing in code changed).

## Tests proving completion

No new automated tests; the screenshot run and the manual checklist above.
Run the commands in `CLAUDE.md` to show nothing regressed.

## Out of scope

- Any code change, including fixes found while documenting (report them).
- Design changes to the views.

## Risks

- Screenshot scenarios depend on the views' accessible names; if the views
  change later, the shots need updating (as for the other tools).
- The scenario list above is the minimum; if the UI tasks added states worth
  showing (the planner amends this list when the UI plan names them), shoot
  those too.
