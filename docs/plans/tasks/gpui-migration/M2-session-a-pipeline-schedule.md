# M2a — Move the Pipeline and Schedule Creator session logic into their crates

Status: approved (2026-11-21)
Project: gpui-migration   Depends on: none (the GPUI go decision is taken)

## Goal

Move the Pipeline's and the Schedule Creator's session logic and session
types out of the Tauri shell into `fm-pipeline::session` and
`fm-schedule::session`, with their tests, leaving the Tauri commands as thin
glue and `apps/desktop/src/lib/generated/` byte-identical.

## Context

FileManager is moving its UI from Tauri + React to GPUI (a native Rust UI).
The new UI will call each tool's session logic directly, so that logic must
live in the tool's own domain crate, not in the Tauri shell. This milestone
moves two tools; the Tar Packager follows in the next milestone. No GPUI code
is written here, and the shipping Tauri app must behave exactly as before.

What exists:

- `apps/desktop/src-tauri/src/tools/pipeline/`:
  - `core.rs`: `Core` (`with_hooks`, `snapshot`, `set_directory`, `begin`,
    `finish`), `Dir::check`, `Job::run`, `Finished`, `HookFor`, helpers
    (`count_subdirectories`, `list`, `issue_view`, `report`, `display`,
    `index`). Items are `pub(super)`.
  - `types.rs`: `StepId`, `DirectorySlot`, `StepIssue`, `StepReport`,
    `StepView`, `PipelineSession`, `PipelineErrorKind`, `PipelineError`
    (serde `Serialize`/`Deserialize` with camelCase, `ts_rs::TS`).
  - `mod.rs`: the `pipeline_*` commands and `PipelineState { core: Mutex<Core> }`.
  - `tests.rs`: drives `Core` in temp dirs.
- `apps/desktop/src-tauri/src/tools/schedule/`: the same shape: `core.rs`
  (`Core`, `Hooks`, `ParseFn`, `MergeFn`, `Slot`, `read_text`,
  `require_file`, `is_xml`, `backup`), `types.rs` (`FileSlot`,
  `ScheduleSession`, `ApplySummary`, `ScheduleErrorKind`, `ScheduleError`),
  `mod.rs`, `tests.rs`.
- `apps/desktop/src-tauri/src/generated_types.rs`: `EXPORTERS` lists, among
  others, `crate::tools::schedule::types::ScheduleSession`,
  `crate::tools::schedule::types::ApplySummary` and
  `crate::tools::pipeline::types::PipelineSession`; the test
  `generated_types_are_current` fails if `lib/generated/` differs from a
  fresh export.
- `crates/fm-pipeline` (no dependencies today) holds `Step`, `StepFn`,
  `StepInput`, `StepOutcome`, `Issue`, `StepError`, `subdirectories()` and
  the four stub hooks. `crates/fm-schedule` (no dependencies) holds
  `Schedule`, `parse`, `merge`, `MergeOutcome`, `ParseError`, `MergeError`.

### What to build

- `crates/fm-pipeline/src/session/` with `mod.rs` (re-exports), `core.rs`,
  `types.rs`, `tests.rs`: the code of `tools/pipeline/{core,types,tests}.rs`
  moved, not rewritten. Visibility becomes `pub` for what the shell (and,
  later, the GPUI app) calls: `Core`, `Dir`, `Job`, `Finished`, `HookFor`,
  the types, and the methods listed above. Helpers stay private.
- `crates/fm-schedule/src/session/` the same way, from `tools/schedule/`.
- `fm-pipeline` and `fm-schedule` gain `serde` (derive) and
  `ts-rs = { workspace = true }`, and `tempfile = { workspace = true }` as a
  dev-dependency where the tests need it. Use the versions the workspace
  already locks. `fm-schedule` also gains `fm-core` (path dependency): the
  moved `core.rs` writes the XML with `fm_core`'s atomic write
  (`atomic_write`). *(Recorded 2026-11-21: the implementer found and added
  this; the plan had missed it.)*
- `tools/pipeline/mod.rs` and `tools/schedule/mod.rs` import from
  `fm_pipeline::session` / `fm_schedule::session` and keep only the
  `#[tauri::command]` functions and the managed-state constructor. Delete the
  shell's `core.rs`, `types.rs` and `tests.rs` for both tools. If a shell
  test exercised the command glue itself (not `Core`), keep it in the shell.
- `generated_types.rs` names the types at their new paths
  (`fm_pipeline::session::PipelineSession`, …). The ts-rs attributes are
  copied exactly, so the export is byte-identical.
- Each new `session` module starts with a doc comment saying what it is:
  the tool's session state and operations, with no UI types, used by the
  Tauri shell now and the GPUI app later.

Rules that bind this task:

- **The `fm-*` crates never depend on tauri** (nor on any GPUI crate). The
  CI job "fm-* crates must not depend on tauri" must still pass.
- **A type that crosses the core/UI boundary is defined once and mirrored
  explicitly**: the generated TS files are the mirror. They must not change.
  Do not run the regenerate command to make the test pass; if the test
  fails, the move changed a type.
- Tools never import each other. `fm-pipeline` must not depend on
  `fm-schedule` or `fm-tarpack` (or the reverse).
- Tests that touch the filesystem run in a temp dir they create.
- fmt clean, clippy `pedantic` with `-D warnings`; silence a lint only with
  a scoped `#[expect(clippy::…, reason = "…")]`. Moving code into a library
  crate can surface new pedantic lints (`must_use_candidate`,
  `missing_errors_doc`, `missing_panics_doc`): fix them (add `#[must_use]`,
  `# Errors` sections) rather than silencing.
- Paths stay `Path`/`PathBuf`; display strings only for display.

## Files

Create: `crates/fm-pipeline/src/session/{mod,core,types,tests}.rs`,
`crates/fm-schedule/src/session/{mod,core,types,tests}.rs`.

Edit: `crates/fm-pipeline/{Cargo.toml,src/lib.rs}`,
`crates/fm-schedule/{Cargo.toml,src/lib.rs}`,
`apps/desktop/src-tauri/src/tools/{pipeline,schedule}/mod.rs`,
`apps/desktop/src-tauri/src/generated_types.rs`, `Cargo.lock`, `CLAUDE.md`
(layout lines for `fm-pipeline`, `fm-schedule` and `src-tauri/src/tools/{pipeline,schedule}/`).

Delete: `apps/desktop/src-tauri/src/tools/{pipeline,schedule}/{core,types,tests}.rs`.

Do not touch: `apps/desktop/src/` (no frontend change at all), the Tar
Packager, `fm-dbmanager`, `docs/plans/`.

## Acceptance criteria

- `git diff --stat main -- apps/desktop/src/lib/generated/` is empty.
- `generated_types_are_current` passes without `UPDATE_GENERATED`.
- Every test that was in the two shell `tests.rs` files exists, under the
  same name, in the crate's `session/tests.rs`, and passes there
  (`cargo test -p fm-pipeline -p fm-schedule`). None was dropped; any
  rename is listed in the PR description with its reason.
- `apps/desktop/src-tauri/src/tools/pipeline/` and `…/schedule/` contain
  only `mod.rs`.
- The command signatures and their behaviour are unchanged; the frontend
  tests (`npm run test`) pass untouched.
- `cargo tree -p fm-pipeline -i tauri` and the same for `fm-schedule` find
  nothing.
- fmt, clippy pedantic (`cargo clippy --workspace --all-targets -- -D warnings`)
  and `cargo test --workspace` pass on both CI jobs.
- `CLAUDE.md`'s layout block names `src/session/` in both crates and no
  longer lists `core.rs`, `types.rs`, `tests.rs` under the two shell tools.

## Tests proving completion

- The moved tests in `crates/fm-pipeline/src/session/tests.rs` and
  `crates/fm-schedule/src/session/tests.rs`.
- `generated_types_are_current` (unchanged).
- Commands: `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, `npm run typecheck && npm run lint && npm run test`.

## Out of scope

- The Tar Packager (next milestone) and the file watcher.
- Any GPUI code, any new behaviour, any change to the TS frontend.
- Removing ts-rs or serde (they go at the cutover).

## Risks

- ts-rs output depends on attributes and, for doc comments, on the comment
  text: copy both verbatim, or the generated files change.
- `pub(super)` items becoming `pub` in a library trigger `pedantic` lints the
  binary crate did not.
- Relative `include_str!` or test-fixture paths change depth when files move
  (`../../../../../../examples/…` becomes `../../../examples/…`).
