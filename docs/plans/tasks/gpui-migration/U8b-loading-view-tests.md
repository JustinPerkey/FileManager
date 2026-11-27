# U8b — View tests for the loading state, using the Worker gate

Status: approved (2026-11-23; from the U7 review). U8b landed as c2d5912 and is **not approved yet**; the next run does only the section "Review follow-up" below.
Project: gpui-migration   Depends on: M4e (landed)

## Review follow-up (2026-11-26)

**The next run does only this section.**

1. **The loading tests must catch a visible loading line.** The Pipeline
   and Schedule `loading_renders_header_only` tests do not catch the bug they
   exist for: the reviewer re-added U7's visible "Loading" line, and the
   Schedule test still passed.
   - Give each page column a debug selector: `pipeline-page` and
     `schedule-page`.
   - While loading, assert that the page's content height equals the
     header's height plus the page's vertical padding, within 1 px.
     Equivalently, the header's bottom edge is the page's last content edge.
   - **Revert check:** re-add a visible "Loading" line in a scratch change.
     The test must fail in both views. Show this in the PR.
2. **`init_app_with_closed_gate_shows_loading`:**
   - its polling budget of 50 × 2 ms is too short for Windows CI. Poll
     until a deadline of about 5 s instead (for example with
     `Instant::now() + Duration::from_secs(5)`), and fail with a clear
     message at the deadline;
   - its "loading: no body" assertion uses the same height check as item 1.

Acceptance: both items' tests pass on both CI jobs, and the revert check
fails as described.

## Goal

Prove, in view tests, what each tool shows while it loads: the header only,
for the Pipeline and the Schedule Creator, and the skeleton for the Tar
Packager.

## Context

M4e added `jobs::WorkerGate` (test and `shoot` builds only), a per-app gate
that holds each tool's background work cooperatively. Install it closed
before the tools are built: `AppSetup::for_tests().with_closed_worker_gate()`,
or `WorkerGate::install_closed(cx)`, as `jobs.rs`'s doc describes. While the
gate is closed, a model reports `loading()`. After `handle.open()` and
`run_until_parked`, it has loaded. Read `jobs.rs` for the exact API.

The views:

- Pipeline: `tools/pipeline/view/`. While loading, it renders the header
  and intro only.
- Schedule Creator: `tools/schedule/view/`. U7's follow-up decided the
  same, header only.
- Tar Packager: `tools/tarpack/view/`. While loading, it shows the skeleton
  after 150 ms (`show_skeleton()`). Its test `skeleton_only_when_model_says`
  in `view/tests.rs` checks only inside `if model.loading()`, so it proves
  nothing.

### What to build

- `tools::pipeline::view::tests::loading_renders_header_only` and
  `tools::schedule::view::tests::loading_renders_header_only`:
  - with the gate closed, assert `model.loading()`;
  - assert the header's tag exists and none of the body's tags (folder or
    file fields, run bar or apply row) exist;
  - open the gate, then `run_until_parked`, and assert the body is drawn.

  If the Schedule test from U7's follow-up already exists in a weaker form,
  replace it.
- `tools::tarpack::view::tests::skeleton_only_when_model_says`: rewrite it
  so it **asserts** `model.loading()` first, with the gate closed. Then:
  - advance the clock past 150 ms and assert the skeleton is shown;
  - open the gate and run until parked, and assert the skeleton is gone.

- `app::tests::init_app_with_closed_gate_shows_loading` (from the M4e
  review, 2026-11-23). It covers the `init_app` route, which nothing tests
  yet:
  - run `init_app(AppSetup::for_tests().with_closed_worker_gate())` and open
    the main window as `run()` does;
  - assert one tool's model (for example the Pipeline's) is `loading()`;
  - open the gate through `WorkerGate::handle(cx)`, then
    `run_until_parked`, and assert it has loaded.

  Open the gate before awaiting any gated work: awaiting it while the gate
  is closed panics with GPUI's "Parking forbidden".

### Rules that bind this task

- Views and models are not changed. If a view needs a change to make the
  state testable, add a tag only.
- Tags go last in a builder chain.
- fmt (linux job), clippy pedantic `-D warnings`, `cargo test --workspace`.

## Files

Edit: `apps/filemanager/src/tools/{pipeline,schedule,tarpack}/view/tests.rs`, and the app-level test file that holds `init_app` tests (`apps/filemanager/src/tests.rs` or the shell's view tests). Tests only.
Add a tag in a view file if one is missing.

## Skill

None. Do not run `impeccable`.

## Acceptance criteria

- The three tests pass.
- Each fails if its view renders the body while loading. Check this with a
  scratch change and say so in the PR.

## Tests proving completion

The three view tests and `init_app_with_closed_gate_shows_loading`.

## States covered

Loading (and loaded after the gate opens).

## Out of scope

- The gate itself (M4e).
