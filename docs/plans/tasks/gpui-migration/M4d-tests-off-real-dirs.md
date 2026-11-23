# M4d — Tests and screenshots never touch the real user data folder

Status: approved (2026-11-23; from the M4c review)
Project: gpui-migration   Depends on: M4c (landed)

## Goal

No test and no screenshot scenario reads or writes the real per-user data
folder (or, later, the system config folder). A guard makes any future slip
fail loudly instead of touching user files.

## Context

Invariant (`CLAUDE.md`): tests never touch real user files; every test that
exercises the filesystem runs inside a temporary directory it creates and
removes. Screenshots use fixture backends only.

The M4c review found a path that breaks it:

- `AppSetup::real(false)` (`apps/filemanager/src/lib.rs`) uses
  `BackendSet::default()`, so `tools::build` constructs every real backend.
- `RealTarpack::new()` (`src/tools/tarpack/backend.rs`, around line 211)
  calls `fm_core::AppDirs::from_system()`, then
  `Store::load(dirs, "tarpack", "state")`. That reads the real
  `<data_dir>/FileManager/tarpack/state.json`, and if the file is unusable,
  **renames it** (`keep_bad_file`).
- This happens in:
  - the shell tests (`app::shell::test_support::open` with `crate::tools::build`);
  - M4c's two new `#[gpui::test]` tests;
  - `shoot::run_scenario` for scenarios that keep the real backends, such as
    `shell-sample`. A developer's saved state therefore also leaks into
    screenshots.

The Pipeline and Schedule Creator backends do not have the problem:
`RealPipeline::new` and `RealSchedule::new` are `Self::default()`, and their
`Core`s hold no persisted state and call neither `AppDirs` nor `Store`.
Check that this is still true when you start. The Database Manager's coming
tool core (`Core::new(dirs, config_path)`, with
`fm_core::system_config_dir()`) must follow the same rule.
`RealTarpack::with_dirs(&AppDirs)` already exists.

### What to build

1. **One place resolves the real folders.** Add `src/app/dirs.rs` with
   `pub fn user_app_dirs() -> fm_core::Result<AppDirs>` (wrapping
   `AppDirs::from_system()`), and later `system_config_path()` for the
   Database Manager, wrapping `fm_core::system_config_dir()`.
   - **Guard:** in builds with `cfg(test)` or feature `shoot`, these
     functions **panic**, with the message "tests and screenshots must not
     use the real user folders; pass a temp-dir AppDirs or a fixture
     backend".
   - Only the release and dev app (`run()` with `AppSetup::real`) reach
     them.
   - `RealTarpack::new()` calls `user_app_dirs()`, not
     `AppDirs::from_system()` directly.
2. **Tests and scenarios default to safe backends.**
   - Add `BackendSet::fixtures()`, a fixture backend for every tool, with
     each tool's neutral state (for example `Empty`).
   - Add `AppSetup::for_tests()`, which uses `BackendSet::fixtures()` and
     `FakePlatform`.
   - `app::shell::test_support::open`, every `#[gpui::test]` that builds
     tools, and `shoot::run_scenario` use these, not `BackendSet::default()`.
     A test that needs a real backend builds it with `with_dirs` over a
     `tempfile::TempDir`.
   - `shell-sample` uses fixtures.
   - `BackendSet::default()` stays the real set, for `run()` only. Document
     that.
3. **A source guard.** Add the test `no_direct_system_dirs`. It scans
   `apps/filemanager/src/**/*.rs` and fails if `AppDirs::from_system` or
   `system_config_dir(` appears anywhere except `src/app/dirs.rs`. Use the
   code-lines-only scan of the old Tauri registry test: skip `//` lines.

### Rules that bind this task

- Tests never touch real user files; temp dirs only. Screenshots use
  fixtures only.
- The real app's behaviour is unchanged: `run()` still uses the real
  folders.
- The `fm-*` crates are not touched. `fm-core`'s own tests already use
  `AppDirs::at(temp)`.
- fmt (linux job), clippy pedantic (with and without `--features shoot`),
  and `cargo test --workspace` pass.

## Files

Create: `apps/filemanager/src/app/dirs.rs`.

Edit:

- `apps/filemanager/src/{lib.rs,app/mod.rs,app/shell.rs}` (test support);
- `apps/filemanager/src/tools/{mod.rs,tarpack/backend.rs}`;
- `apps/filemanager/src/shoot/{mod.rs,scenarios/mod.rs}` (`shell-sample`
  only);
- the tests that build tools;
- `CLAUDE.md`, a layout line for `app/dirs.rs`.

Do not touch: views, other scenarios (beyond switching their default
backends if they relied on the real set: say which), `crates/`,
`docs/plans/`.

## Acceptance criteria

- `cargo test --workspace` and the shoot tests run with no file created,
  read or renamed under the real data folder. Check it by pointing the
  data dir somewhere visible:
  - on Linux, `XDG_DATA_HOME=$(mktemp -d)`; after the run, assert that
    folder has no `FileManager` directory, in a CI step on the linux job;
  - on Windows, the guard panics if anything reaches the real folder.
- Calling `user_app_dirs()` in a test panics with the message above (test
  `user_app_dirs_panics_in_tests`, `#[should_panic]`).
- `no_direct_system_dirs` passes.
- The release app still restores the last manifest from the real folder
  (manual: run the dev build once).

## Tests proving completion

- `app::dirs::tests::user_app_dirs_panics_in_tests`.
- `app::dirs::tests::no_direct_system_dirs`.
- The existing shell, M4c and shoot tests, now on fixtures.
- The linux CI step checking that `XDG_DATA_HOME` stays empty.

## Out of scope

- The Database Manager's tool core (M12a). It must call
  `app::dirs::system_config_path()` when it lands; note that in M12a's PR.

## Risks

- A test that silently relied on a developer's saved state changes its
  result. That is a bug it had; fix the test to set up its own state.
