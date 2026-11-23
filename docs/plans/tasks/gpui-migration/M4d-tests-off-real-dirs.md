# M4d — Tests and screenshots never touch the real user data folder

Status: approved (2026-11-23; from the M4c review). **Review follow-up (2026-11-23):** M4d is approved; the next run does only the section "Review follow-up" below.
Project: gpui-migration   Depends on: M4c (landed)

## Review follow-up (2026-11-23)

M4d is approved. **This run does only this section.**

1. **Explicit fixtures in shell test support.** Make
   `app::shell::test_support::open` set `BackendSet::fixtures()`
   explicitly, as this plan asked. **Accepted deviation, kept:** in test and
   `shoot` builds, `BackendSet::get` falls back to the fixture backend for a
   tool with no entry. It is a safety net; say so in its doc comment.
2. **CI: one temp data folder for both test steps.**
   - In the linux job, set `XDG_DATA_HOME` to a fresh temp folder **once**,
     before both the workspace test step and the `--features shoot` test
     step.
   - Check after **both** that it contains no `FileManager` folder.
   - Drop the duplicate shoot test run.
   - Note in the workflow comment that
     `apps/desktop/src-tauri/src/tools/tarpack/mod.rs` (around line 31) still
     calls `AppDirs::from_system()` unguarded. No test reaches it, and it is
     removed at the cutover.
3. **`default_start_dir` through `app::dirs`.** `tools/tarpack/model.rs`
   (around line 320) calls `directories::UserDirs::new()` directly, which
   reads the real user-dirs configuration in tests.
   - Add `app::dirs::user_documents_dir() -> Option<PathBuf>` (Documents,
     else home), with the same guard. In test and `shoot` builds it must not
     touch the real folders: return a fixed fake path such as
     `C:\Users\fixture\Documents` (or `/home/fixture/Documents`), rather
     than panicking, because view tests open dialogs.
   - Make `default_start_dir` use it.
   - Add `UserDirs::new` and `BaseDirs::new` to `no_direct_system_dirs`'s
     scan.
   - Test: `user_documents_dir_is_fake_in_tests`.
4. **Document the shoot panic.** Add one sentence to `src/app/dirs.rs`'s
   module doc and to `CLAUDE.md`'s `app/dirs.rs` layout line: the main
   `filemanager-gpui` binary built with `--features shoot` panics at
   startup by design (it would reach the real folders). Only
   `filemanager-shoot` is meant to run with that feature.

Note: the review's finding 1 (`only_the_current_item_is_a_tab_stop` failing)
came from U7's work in progress. U7's own commit adjusts that test; it is
not part of this run.

### Acceptance

- The named test passes, and `no_direct_system_dirs` covers the new names.
- The linux CI log shows a single `XDG_DATA_HOME` set-up, one shoot test
  run, and the empty-folder check after both test steps.
- fmt (linux job), clippy pedantic (with and without `--features shoot`) and
  `cargo test --workspace` pass.

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
