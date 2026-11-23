# U8b — View tests for the loading state, using the Worker gate

Status: approved (2026-11-23; from the U7 review)
Project: gpui-migration   Depends on: M4e (landed)

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

### Rules that bind this task

- Views and models are not changed. If a view needs a change to make the
  state testable, add a tag only.
- Tags go last in a builder chain.
- fmt (linux job), clippy pedantic `-D warnings`, `cargo test --workspace`.

## Files

Edit: `apps/filemanager/src/tools/{pipeline,schedule,tarpack}/view/tests.rs`.
Add a tag in a view file if one is missing.

## Skill

None. Do not run `impeccable`.

## Acceptance criteria

- The three tests pass.
- Each fails if its view renders the body while loading. Check this with a
  scratch change and say so in the PR.

## Tests proving completion

The three named tests.

## States covered

Loading (and loaded after the gate opens).

## Out of scope

- The gate itself (M4e).
