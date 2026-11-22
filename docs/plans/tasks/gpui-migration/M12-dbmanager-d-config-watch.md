# M12d — Database Manager, part D: config watcher, automatic reload, Edit config

Status: approved (2026-11-21)
Project: gpui-migration   Depends on: M12c (landed)

## Goal

Watch the app-wide Database Manager config and reload it automatically
when it changes, re-checking the open draft against the new rules, and add
developer mode's **Edit config**, which opens the config in the system
editor after creating a commented starter file if there is none.

## Context

The config is `fm_core::system_config_dir()?.join("dbmanager.toml")`
(`%ProgramData%\FileManager\dbmanager.toml` on Windows). The app only reads
it; the human decided it reloads automatically, and that developer mode
(Ctrl+Alt+Shift+D, app-wide, off at launch) offers Edit config.

Read first:

- `crates/fm-core/src/watch.rs`: `FileWatcher::start(path, on_change: Fn() + Send)`,
  `path()`, 300 ms debounce; it watches the file's **parent directory**,
  non-recursively; it fails if that directory does not exist.
- `crates/fm-dbmanager/src/tool/` (M12a–M12c): `Core::new(dirs, config_path)`,
  `reload_config()`, `save`/`delete` reload on `ConfigChangedOnDisk`;
  `DbmanagerSnapshot.config { path, display, status, hash }`,
  `config_changed`.
- `examples/dbmanager/starter.toml`: commented and inert (it parses with
  zero diagnostics to the editable defaults).
- `apps/filemanager/src/tools/dbmanager/model.rs`: `recheck_after_reload`
  (M12b: rebase the draft, drop edits that no longer apply, notices "The
  config was reloaded." / "The config was reloaded. These changes no longer
  apply and were removed: <names>."), `Platform::open_with_system`,
  `DevMode::is_on`, and how the Tar Packager wires its `FileWatcher`
  through an `async_channel` into its model
  (`apps/filemanager/src/tools/tarpack/model.rs`).

### What to build

- **Missing directory in `FileWatcher`** (`fm-core`): if the parent
  directory does not exist (`%ProgramData%\FileManager` before anyone
  creates a config), watch the deepest existing ancestor non-recursively;
  when the next path component appears, move the watch down, until the
  parent directory is watched; call `on_change` when the file itself
  appears. Never watch recursively (`%ProgramData%` is large). The Tar
  Packager's behaviour (parent exists) is unchanged.
- **Arming:** the model starts the watcher once per app run, after the first
  snapshot, when `config.path` is `Some`; if it cannot start, log it and
  carry on (`config_changed` in the snapshot and the write-path reload
  cover it). The callback sends into a channel; a foreground task asks the
  Worker to `reload_config()` (it runs after any write already queued, so a
  reload never interrupts a write), then, if the hash changed, refreshes the
  snapshot and calls `recheck_after_reload`. A reload never touches the
  database or the draft beyond that re-check.
- `ToolHooks::inject_file_changed` for this tool triggers the same path
  (screenshots and tests).
- **Edit config** (`Core::edit_config_target() -> Result<(PathBuf, bool), DbmanagerError>`):
  if the file is missing, create its directory and write
  `starter.toml`'s text (`include_str!`) with `create_new` (never
  overwrite; a race where it now exists is fine, `created = false`); return
  the path and whether it was created. A permission failure (another user's
  account owns the directory) is `Io` with a message saying so. The model's
  `edit_config(cx)` is allowed only when `DevMode::is_on`; it runs the core
  call on the Worker, then `Platform::open_with_system(path)`, which returns
  `Task<Result<(), OpenError>>` (added in the Tar Packager's M7a follow-up);
  an `Err` is the banner for kind `OpenerFailed` with the detail. It shows the
  notice "Created a starter config at <display>." when created. Test:
  `edit_config_opener_failure_shows_opener_failed` (`FakePlatform` answers
  `Err`).
- Model API: `can_edit_config(cx) -> bool`, `edit_config(cx)`,
  `config_reloaded_notice()` (the re-check notice is the M12b `notice`).
- Copy in `text.rs` for the sentences above.
- Fixtures for the UI's scenarios: a config with exactly one error and one
  warning (`dbmanager-diagnostics`), and a config with a syntax error, so
  everything is read-only with the reason (`dbmanager-readonly-config`);
  make the `dbmanager-config-reloaded` fixture of part B go through the
  watcher path (`inject_file_changed`).

**Fixture timing rule (planner, 2026-11-22, after M4).** The harness's
`Step::WaitIdle` is a short fixed settle, not idle detection: it does not
await `Worker` jobs, `spawn_thread` jobs or async loads. So every fixture
backend state a scenario shows must answer **synchronously** (the backend
method returns at once, so the Worker call resolves inside the settle), or
the scenario that shows it must add an explicit `Step::Wait(ms)` long
enough for the job. Fixtures that simulate long work (progress, a running
step) are timed on purpose; document each fixture's timing in its doc
comment so the scenario author knows which `Wait` it needs.

### Rules that bind this task

- **Tests never touch the real config location.** `Core` takes the config
  path; tests use temp-dir paths, including a missing directory chain under
  the temp dir.
- `fm-core` stays tool-agnostic; no GPUI or tauri dependency.
- Never overwrite an existing config; the starter is inert.
- No blocking I/O on the UI thread.
- fmt clean; clippy pedantic `-D warnings`; scoped `#[expect]` only.

## Files

Edit: `crates/fm-core/src/watch.rs`, `crates/fm-dbmanager/src/tool/{core.rs,tests.rs}`,
`apps/filemanager/src/tools/dbmanager/{model.rs,text.rs,mod.rs,fixtures.rs,tests.rs}`,
`CLAUDE.md` (Status: the Database Manager's model is complete).

Do not touch: the Tar Packager's code (its tests must pass unchanged),
`docs/plans/`.

## Acceptance criteria

- The watcher follows a directory chain that appears after it starts, and
  never watches recursively (asserted in the watch module's tests through
  the mode it passes to `notify`).
- An edited config reloads with no user action; the draft is re-checked;
  the notice appears.
- Edit config creates the starter only when missing and opens the file.
- fmt, clippy pedantic and `cargo test --workspace` pass on both jobs.

## Tests proving completion

- `fm-core` `watch` tests: `missing_parent_watches_ancestor_then_file_appears`,
  `existing_parent_unchanged_behaviour` (temp dirs, timing tolerant).
- `tool/tests.rs`: `edit_config_creates_starter_when_missing`,
  `edit_config_never_overwrites`, `starter_is_inert` (loads to the defaults
  with zero diagnostics).
- `tests.rs` (`#[gpui::test]`): `config_edit_reloads_and_rechecks_draft`
  (real `FileWatcher`, temp dir, `allow_parking`),
  `inject_file_changed_reloads`, `reload_drops_edits_that_no_longer_apply`,
  `reload_waits_for_running_save`, `edit_config_needs_dev_mode`,
  `edit_config_opens_with_system_and_notices_creation`,
`fixtures_build_diagnostics_and_readonly_config_states`.

## Out of scope

- Views and scenarios (UI tasks). The docs and manual chapter (the
  Database Manager docs milestone).

## Risks

- Watcher reliability varies (redirected folders); the snapshot's
  `config_changed` and the write-path reload are the fallbacks.
- `%ProgramData%` is shared by every user; Edit config may fail with a
  permission error, which the message names.
