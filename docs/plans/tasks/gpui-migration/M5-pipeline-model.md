# M5 — Pipeline: backend trait, view-model, copy, fixtures

Status: awaiting approval
Project: gpui-migration   Depends on: M4 (landed)

## Goal

Make the Pipeline tool work in the GPUI app as a fully tested view-model:
a backend trait over `fm_pipeline::session::Core`, a model entity holding
every piece of behaviour today's React view holds, the tool's copy ported
verbatim, and fixture backends for every Pipeline screenshot, so that the
UI task can build the views as pure rendering.

## Context

FileManager is moving from Tauri + React to GPUI. In the GPUI app the
backend implementer owns each tool's **model** (behaviour and view state)
and **copy**; the ui-implementer later writes the **views**, which only
render the model, map its status enums to icons and colours, own focus and
layout, and call model methods. A view-level need the model lacks is a gap
to report, so pin the API below carefully: the UI task plans cite it.

What exists:

- `crates/fm-pipeline/src/session/` (moved there in M2a): `Core`
  (`default()` = the real hooks, `with_hooks(HookFor)`, `snapshot()`,
  `set_directory(Option<Dir>)`, `begin(Step, expected_display: &str) -> Result<Job, PipelineError>`,
  `finish(Step, Finished)`), `Dir::check(&Path)` (does I/O), `Job::run(self) -> Finished`,
  and the types `PipelineSession`, `StepView`, `StepReport`, `StepIssue`,
  `DirectorySlot`, `StepId`, `PipelineError`, `PipelineErrorKind`.
- `apps/desktop/src-tauri/src/tools/pipeline/mod.rs`: how the Tauri commands
  call it today (`Dir::check` before taking the lock; a step runs on a
  blocking worker; a panicked step becomes a `StepFailed` result).
- Today's behaviour, which you port: `apps/desktop/src/tools/pipeline/PipelineView.tsx`
  (state, `cleanPath`, `stepStatus`, `runReason`, run one / run next / run
  all, confirmation, "only the newest folder change is shown", restore
  failure and retry), `errorMessages.ts`, `steps.ts`, and their tests
  `PipelineView.test.tsx`. Screenshot states: `docs/manual/screenshots/mock.js`
  and `shoot.js` (images `pipeline-empty`, `-chosen`, `-bad`, `-runall`,
  `-running`, `-ran`).
- `apps/filemanager` (package `filemanager-gpui`, lib `fm_app`):
  `src/tools/mod.rs` (`ToolId::Pipeline`, `ToolSlot`, `ToolHooks` with
  `activated`, `drop_target`, `dropped`, `unsaved_work`,
  `inject_file_changed`), placeholder `src/tools/pipeline/{mod.rs,view/mod.rs}`,
  `src/jobs.rs` (`Worker<T>`, `spawn_thread`), `src/platform/` (`Platform`,
  `PickKind::Folder`, `PickError`, `FakePlatform`), `AppSetup`/`BackendSet`
  in `src/lib.rs`, the harness in `src/shoot/`.
- `docs/gpui-test-ledger.md`: rows for `tools/pipeline/*.test.tsx`.

### What to build

**`src/tools/pipeline/backend.rs`.**

```rust
pub trait PipelineBackend: Send + 'static {
    fn session(&mut self) -> PipelineSession;
    fn set_directory(&mut self, path: Option<PathBuf>) -> Result<PipelineSession, PipelineError>;
    fn begin_step(&mut self, step: StepId, expected_directory: &str) -> Result<Box<dyn StepJob>, PipelineError>;
    fn finish_step(&mut self, step: StepId, finished: Finished) -> PipelineSession;
}
pub trait StepJob: Send + 'static { fn run(self: Box<Self>) -> Finished; }
pub struct RealPipeline { core: Core }            // Core::default()
```

`RealPipeline::set_directory` runs `Dir::check` then `Core::set_directory`
(it runs on the Worker, off the UI thread). A step job that panics becomes
`Finished { result: Err(StepFailed "the step stopped unexpectedly: …"), directory: None }`,
as today.

**`src/tools/pipeline/text.rs`.** Verbatim copy: every `PipelineErrorKind`
message of `errorMessages.ts` (an exhaustive `match`), the "Unexpected" copy
"Something went wrong. See Details.", every `stepCopy` entry (title,
description, confirm-or-none), status labels ("Running…", "Not
implemented", "Failed", "Passed", "N problem(s)", "Waiting", "Not run"),
run reasons ("A step is running.", "Choose a folder.", "Fix the folder path
above first."), the announcements ("<title>: <status>.", "All steps
passed.", "Stopped at <title>."), and every other string `PipelineView.tsx`
renders (header, intro, button labels incl. "Run next" / "Run all…",
confirmation title and body, "Try again"). Group them as constants or small
functions; the UI imports them from here.

**`src/tools/pipeline/model.rs`: `PipelineModel` (an entity).**

State, all private, read through methods:

```rust
pub enum ViewError { Pipeline(PipelineError), Unexpected { message: String } }
pub enum Pending { Step { id: StepId, via_next: bool }, All }
pub enum StepStatus { Running, NotImplemented, Failed, Passed, Problems(u32), Waiting, NotRun }

impl PipelineModel {
    pub fn session(&self) -> Option<&PipelineSession>;     // None while loading or after a failed load
    pub fn loading(&self) -> bool;
    pub fn load_failed(&self) -> bool;  pub fn retry_load(&mut self, cx);
    pub fn error(&self) -> Option<&ViewError>;  pub fn error_generation(&self) -> u64;  pub fn dismiss_error(&mut self, cx);
    pub fn set_directory_text(&mut self, typed: &str, cx);   // clean_path, then set
    pub fn browse(&mut self, cx);                            // Platform folder pick, start at the current folder
    pub fn clear_directory(&mut self, cx);
    pub fn busy(&self) -> bool;                              // a step runs (this model's or session.running) or run-all
    pub fn running(&self) -> Option<StepId>;
    pub fn via_next(&self) -> Option<StepId>;                // the step "Run next" started (only that button is busy)
    pub fn run_reason(&self) -> Option<&'static str>;
    pub fn can_run(&self) -> bool;
    pub fn next_step(&self) -> Option<StepId>;
    pub fn all_passed(&self) -> bool;
    pub fn step_status(&self, id: StepId) -> StepStatus;     // label via text.rs
    pub fn request_step(&mut self, id: StepId, cx);          // confirms first if the step has confirm copy
    pub fn request_next(&mut self, cx);
    pub fn request_all(&mut self, cx);                       // confirms if any step has confirm copy
    pub fn pending(&self) -> Option<(&Pending, &str)>;       // with the folder named in the confirmation
    pub fn confirm(&mut self, cx);  pub fn cancel(&mut self, cx);
    pub fn announcement(&self) -> Option<&str>;              // the latest outcome sentence
    pub fn focus_hint(&self) -> Option<FocusHint>;           // Step(id) | RunNext: where today's view moves focus after a run
}
```

Emit `cx.notify()` on every change. Behaviour to port (each is a case in
`PipelineView.test.tsx`):

- Startup / `activated()`: fetch the session through the Worker; a failure
  sets `load_failed` and the error; `retry_load` refetches. `activated()`
  clears `error`, `pending`, `announcement` and `focus_hint` (the state the
  React view held), then refreshes the session; a running step or run-all
  continues and its result lands in the model.
- Folder: typed text is cleaned (trimmed, surrounding `"` removed, empty =
  none) before `set_directory`. Only the newest folder change is applied:
  a response to an older request is discarded (sequence number). A folder
  with an error is shown via the snapshot's `directory.error`; the
  announcement says its message. A pick `Err` from `Platform` becomes
  `ViewError::Unexpected` with the path-loss copy from `fm_app`'s
  `app::text` (the `PickError` message) and its detail. A cancelled pick
  changes nothing.
- Running: `request_*` refuse silently when `!can_run()`. A step with
  confirm copy (or "Run all" when any step has one) sets `pending` with the
  folder **display path shown at that moment**; `confirm` runs exactly that
  folder (passed as `expected_directory`); `cancel` clears it. A run:
  `begin_step` on the Worker, `StepJob::run` on `spawn_thread`, then
  `finish_step` on the Worker, then the snapshot. A `begin_step` error
  (`DirectoryChanged`, `StepInProgress`, `PreviousStepNotPassed`, …) is the
  error banner; a step that fails or is not implemented is recorded in the
  session, not a banner. Run all runs the steps in order and stops at the
  first that does not pass; announcement "All steps passed." or
  "Stopped at <title>.". A single run announces "<title>: <status label>.".
  `focus_hint` after a "Run next" run is `RunNext` when another step can run
  next, otherwise `Step(id)`.
- Any command failing in an unexpected way (a panicked job) is
  `ViewError::Unexpected`. Each new error increments `error_generation` (the
  view re-shows a banner for a repeated error).
- Pipeline has no drop target, no unsaved work.

Dropped on purpose (record in the ledger): the "orphan" polling (GPUI models
outlive views), `aria-live`/`aria-describedby` wiring, DOM focus calls (the
view uses `focus_hint`).

**`src/tools/pipeline/mod.rs`.** Registers the tool: the model is created in
`tools::build` with `RealPipeline` or a fixture per `BackendSet`; `ToolHooks`
forwards `activated`. The placeholder view stays (shows the model's
directory and step statuses as plain text) until the UI task replaces
`view/`.

**`src/tools/pipeline/fixtures.rs`** (`#[cfg(any(test, feature = "shoot"))]`):
`FixturePipeline` with constructors for each screenshot state of `mock.js`:
`empty()`, `chosen()` (a valid folder with N subfolders, nothing run),
`bad()` (directory error), `ran()` (first steps passed, one with problems,
one not implemented), and step jobs whose result and duration are
configurable (`running` needs a step that takes long enough to capture,
e.g. 30 s, released when the app quits). Add them to `BackendSet` as
`FixtureId::Pipeline(PipelineFixture)`. Paths in fixtures are fake display
strings (`C:\Builds\gateway`), never touched on disk.

### Rules that bind this task

- Views never touch the filesystem or a domain crate; this model is their
  only contact. No blocking I/O on the UI thread: every backend call goes
  through the `Worker`; steps through `spawn_thread`.
- Destructive operations are never silent: steps whose copy has a confirm
  sentence are confirmed, naming the folder, and run on exactly that folder.
- Paths stay `PathBuf` (typed text becomes a `PathBuf` only via
  `clean_path`); display strings only for display and for the
  `expected_directory` check the session already uses.
- Tools never import each other.
- Tests that touch the filesystem use temp dirs; model tests use fixtures or
  `RealPipeline` over a temp dir, and `FakePlatform`.
- Ownership: `backend.rs`, `model.rs`, `text.rs`, `fixtures.rs`, `mod.rs`
  are yours; `view/` beyond the placeholder is the ui-implementer's. No
  visual decisions.
- fmt clean; clippy pedantic `-D warnings`; scoped `#[expect]` only.

## Files

Create: `apps/filemanager/src/tools/pipeline/{backend.rs,model.rs,text.rs,fixtures.rs,tests.rs}`.

Edit: `apps/filemanager/src/tools/pipeline/{mod.rs,view/mod.rs}` (placeholder
only), `apps/filemanager/src/{lib.rs,tools/mod.rs}` (fixture ids),
`docs/gpui-test-ledger.md` (Pipeline rows), `CLAUDE.md` (Status: the Pipeline
model is ported; Layout line).

Do not touch: `crates/` (if `fm-pipeline::session` needs a small addition,
stop and report it), `apps/desktop/`, `docs/plans/`.

## Acceptance criteria

- The public API above exists with doc comments (names may gain parameters
  GPUI needs, such as `&mut Window`; no name is dropped).
- Every row of `PipelineView.test.tsx` in the ledger is `ported` (to a named
  test below or another named model test) or marked `U` (rendering, focus
  placement, labels) or `dropped: <reason>`. None stays `open`.
- `text.rs` contains every string of `errorMessages.ts` and `steps.ts`
  (checked by a test that compares against the TS sources while they exist:
  read them with `include_str!` from `apps/desktop/src/tools/pipeline/` and
  assert each quoted message appears in `text.rs`'s values).
- Fixtures reproduce the six Pipeline screenshot states.
- fmt, clippy pedantic (with and without `--features shoot`) and
  `cargo test --workspace` pass on both jobs.

## Tests proving completion

`apps/filemanager/src/tools/pipeline/tests.rs` (`#[gpui::test]` unless
noted):

- `text_covers_every_error_kind_and_step` (plain test), `text_matches_ts_sources`.
- `clean_path_trims_and_unquotes` (plain test; today's `cleanPath` cases).
- `loads_session_on_start`, `load_failure_shows_retry_and_retry_loads`.
- `typed_folder_sets_directory`, `bad_folder_shows_slot_error_and_announces`,
  `only_newest_folder_change_applies`, `browse_uses_platform_and_cancel_changes_nothing`,
  `browse_error_shows_unreadable_copy`.
- `step_status_covers_every_case`, `run_reason_and_can_run`.
- `step_without_confirm_runs_immediately`,
  `step_with_confirm_waits_and_uses_the_confirmed_folder`,
  `cancel_runs_nothing`, `run_all_confirms_once_and_stops_at_first_failure`,
  `run_all_all_passed_announces`, `run_next_busy_only_for_its_step`,
  `focus_hint_after_run_next`.
- `begin_error_is_a_banner_and_step_failure_is_not`,
  `repeated_error_bumps_generation`, `dismiss_error_clears_it`,
  `panicked_step_is_unexpected_failure`.
- `activated_clears_transient_state_and_keeps_running_step`.
- `real_backend_runs_verify_in_temp_dir` (`RealPipeline`, temp dir with two
  subfolders; the stub hook gives "Not implemented").
- `fixtures_build_each_screenshot_state`.

Command: `cargo test -p filemanager-gpui`, then the workspace commands.

## Out of scope

- The Pipeline views, focus handling and screenshot scenarios (UI tasks).
- The four step hooks themselves (they stay stubs).
- Removing the Tauri Pipeline (cutover).

## Risks

- `Worker` serialises calls; `set_directory` on a slow share delays a
  session refresh queued behind it, never the UI thread.
- The React view's announcement and focus timing are approximated by
  `announcement` and `focus_hint`; the UI task decides how they show.
