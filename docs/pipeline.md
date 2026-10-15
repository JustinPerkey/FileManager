# Pipeline

Runs a fixed sequence of steps over one folder the user chooses. Each
immediate subfolder of that folder is one unit of work.

The UI, the commands, and the sequencing are done. The four steps are stubs in
`crates/fm-pipeline` that return `StepError::NotImplemented`; the UI shows
"Not implemented" on a step when it is run.

## Flow

1. **Choose the folder.** Type or paste a path (set on Enter or when leaving
   the field; Explorer's quotes are removed; an empty field clears it; Escape
   restores it), or click **Browse…** for the folder dialog.
   `pipeline_set_directory` checks it is a full path to a folder whose
   contents can be listed, and counts its subfolders. A bad path stays in the
   field with its error and blocks running. Choosing a folder forgets every
   step's result. It is refused while a step runs.
2. **Run a step**, or **Run all…**. Each step has its own Run button.
   - Steps with a confirmation (`confirm` in
     `apps/desktop/src/tools/pipeline/steps.ts`: the batch, schedule and tar
     steps) ask first, naming the folder. Run all asks once, listing every
     step's confirmation.
   - `pipeline_run_step(step, expectedDirectory)` refuses to start if another
     step is running (`StepInProgress`) or the session holds a different folder
     than the one the UI showed (and confirmed, for steps that ask)
     (`DirectoryChanged`). Otherwise it lists the subfolders again, then calls
     the step's hook on a blocking worker thread, so the hook may take as long
     as it needs. Afterwards the folder is checked again: its subfolder count
     is updated, and if it has gone, the folder field shows why and running is
     blocked.
   - While a step runs, the folder field and every Run button are disabled and
     focus moves to the running step. If the user leaves the Pipeline and comes
     back mid-step, the view shows the step as running (from
     `session.running`) and polls `pipeline_session` until it finishes. Run all
     stops starting further steps once the user leaves the view.
   - The result is recorded on the step and returned in the session: a
     `report` (summary line, issues per subfolder, `passed` when there are no
     issues) or an `error` (`StepNotImplemented`, or `StepFailed` with its
     message under Details). The command itself fails only when the step could
     not start.
   - **Run all** is the UI calling `pipeline_run_step` for each step in order,
     stopping at the first step that does not pass (issues, error, or could not
     start).

## Where the work goes

| Step | Hook | File |
| --- | --- | --- |
| 1. Check required files | `verify::verify` | `crates/fm-pipeline/src/verify.rs` |
| 2. Run batch files | `batch::run_batch` | `crates/fm-pipeline/src/batch.rs` |
| 3. Schedule | `schedule::schedule` | `crates/fm-pipeline/src/schedule.rs` |
| 4. Pack archives | `tarpack::tarpack` | `crates/fm-pipeline/src/tarpack.rs` |

Every hook has the signature

```rust
fn(StepInput<'_>) -> Result<StepOutcome, StepError>
```

- `StepInput { root, subdirectories }`: the chosen folder (absolute) and its
  immediate subfolders, sorted, listed just before the step started.
- `StepOutcome { summary, issues }`: `summary` is one line shown under the
  step ("12 folders checked"). Each `Issue { subdirectory, message }` is a
  problem found; `subdirectory` must be one of `input.subdirectories` (the UI
  shows its name; name a file inside it in the message). No issues
  means the step passed, and Run all goes on to the next step.
- `StepError::Failed(message)`: the step could not go on. Say how far it got.
- To use the existing domain libraries, add `fm-schedule` / `fm-tarpack` to
  `crates/fm-pipeline/Cargo.toml` (crate-to-crate dependencies are fine; only
  the shell's tool modules must not import each other).
- Hooks that change files must keep the repository's invariants: never
  overwrite silently (back up, or write atomically with
  `fm_core::atomic_write`), and report a part-way failure with what was
  already done.
- To add, remove, or reorder steps: `Step` and `Step::ALL` in
  `crates/fm-pipeline/src/lib.rs`, `StepId` in
  `apps/desktop/src-tauri/src/tools/pipeline/types.rs` (then regenerate
  `lib/generated/`), and `stepCopy` in
  `apps/desktop/src/tools/pipeline/steps.ts`. A test in `fm-pipeline` fails if
  a `Step` variant is missing from `Step::ALL`; the Rust `From` impls fail to
  compile if `StepId` and `Step` differ; the typecheck fails if `stepCopy`
  does not cover `StepId`.
- The batch hook's doc comment has two Windows notes (`CREATE_NO_WINDOW`, and
  how `cmd.exe` parses `.bat` arguments).
- The steps' copy (titles, descriptions, confirmations) lives in `steps.ts`;
  adjust it once the steps do real work.
