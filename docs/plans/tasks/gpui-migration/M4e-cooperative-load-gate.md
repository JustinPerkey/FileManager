# M4e — A cooperative gate for background work, so tests can see the loading state

Status: approved (2026-11-23; from the U7 review). M4e landed and is approved; the next backend run does only the section "Review follow-up" below.
Project: gpui-migration   Depends on: M7b and M4d (landed)

## Review follow-up (2026-11-23)

M4e is approved. **The next run does only this section.** It changes doc
comments in `apps/filemanager/src/jobs.rs` only; no behaviour changes.

**Accepted deviation, recorded:** `WorkerGate::handle(cx)` returns the
installed gate's handle (so a test that installed the gate through
`AppSetup::for_tests().with_closed_worker_gate()` can reach it). It is kept.

1. **`release_one`'s doc comment** must say:
   - a release made while no job is waiting is kept as a **credit**, and the
     next queued job runs at once;
   - `open` and `close` do not clear credits;
   - which Worker takes a released job is not fixed, when several Workers
     share the gate.
2. **The `jobs.rs` module doc** must say: awaiting gated work while the gate
   is closed (for example `run_until_parked` followed by awaiting a Worker
   task's result in a test) panics with GPUI's "Parking forbidden", because
   nothing can make progress. Open the gate first, or use `release_one`.

Acceptance: the doc comments read as above; fmt (linux job), clippy
pedantic and `cargo test --workspace` pass.

## Goal

Let a test hold a tool's background work at a point it chooses, without
blocking any thread and without test code in the models, so that the
loading state of the Pipeline, Schedule Creator and Tar Packager can be
observed and asserted.

## Context

Each tool model (`apps/filemanager/src/tools/<tool>/model.rs`) owns its
backend in a `jobs::Worker<T>` (`apps/filemanager/src/jobs.rs`). The model
queues closures with `worker.run(f, cx)`. The Worker's loop runs them in
order on GPUI's background executor. The first one is the initial session
load or restore, and the model is `loading()` until its result arrives.

Under `TestAppContext` the background executor is deterministic. By the
first frame a view test can observe, the loop has already run the load, so
`loading()` is false. A gate that **blocks** the closure, such as a
`Condvar` inside a fixture backend method, hangs the test, because the
executor runs the closure inline. So no view test can see the loading state
today. The Tar Packager's U9 test `skeleton_only_when_model_says` hides this:
it checks the skeleton only inside `if model.loading()`, so it passes
without proving anything.

**Decision (planner, 2026-11-23): the gate lives in `jobs::Worker`, not in
each fixture backend.** A backend method is synchronous and cannot await, so
a per-backend gate would block. The Worker's loop is async and can await a
gate cooperatively. One gate in `jobs.rs` serves every tool, and the models
stay untouched.

### What to build

- **`jobs::WorkerGate`**, compiled only with `cfg(any(test, feature = "shoot"))`,
  as a GPUI global:

  ```rust
  pub struct WorkerGate { /* … */ }
  impl WorkerGate {
      /// Installs a closed gate on this app: Workers created afterwards wait
      /// before running each queued closure until the gate opens.
      pub fn install_closed(cx: &mut App) -> WorkerGateHandle;
  }
  pub struct WorkerGateHandle { /* … */ }
  impl WorkerGateHandle {
      pub fn open(&self);              // lets every waiting and future job run
      pub fn close(&self);             // holds jobs again (optional)
      pub fn release_one(&self);       // lets exactly one queued job run
  }
  ```

  The waits are async (for example an `async_channel` the loop `recv`s on,
  or an `event_listener`), never a blocking call.
- **`Worker::new(value, cx)`** reads `cx.try_global::<WorkerGate>()` when the
  gate exists (test and `shoot` builds only). Its loop awaits the gate
  before each job. In release builds the code is compiled out and the loop
  is unchanged.
- The gate is per app (a GPUI global), so tests running in parallel do not
  affect each other.
- **Docs:** the `jobs.rs` module doc says how tests use the gate. Install it
  before the tools are built (`AppSetup::for_tests()` gains
  `.with_closed_worker_gate()`, or the test calls `install_closed` before
  building), then assert `loading()`, then `open()` and `run_until_parked`.

### Rules that bind this task

- No test-only code in any model. Only `jobs.rs` and the test-setup helpers
  change.
- No blocking waits on the executor.
- The release exe never contains test-support code. The gate is behind
  `cfg(any(test, feature = "shoot"))`.
- fmt (linux job), clippy pedantic (with and without `--features shoot`),
  `cargo test --workspace`.

## Files

Edit: `apps/filemanager/src/jobs.rs` (and its tests),
`apps/filemanager/src/lib.rs` (`AppSetup::for_tests` option),
`apps/filemanager/src/app/shell.rs` (test support only, if needed).

Model tests are added in each tool's `tests.rs`: test code only, no
`model.rs` changes.

Do not touch: `model.rs` files, views, `crates/`, `docs/plans/`.

## Acceptance criteria

- With a closed gate, each tool model reports `loading()` after its window
  opens and the executor is parked. After `open()` and `run_until_parked`,
  it is loaded.
- No model source changes.

## Tests proving completion

- `jobs::tests::closed_gate_holds_jobs_without_blocking` (closures stay
  queued while parked), `release_one_runs_exactly_one`, and
  `open_gate_runs_queued_jobs_in_order`.
- `tools::pipeline::tests::loading_is_observable_with_closed_gate`, and the
  same for `schedule` and `tarpack`.

## Out of scope

- The view tests that use the gate: `U8b-loading-view-tests.md` (the
  ui-implementer's) follows this task.
