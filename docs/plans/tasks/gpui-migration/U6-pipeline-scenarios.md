# U6 — Pipeline screenshot scenarios

Status: approved (2026-11-21)
Project: gpui-migration   Depends on: U5 (landed), M4 (landed)

## Goal

Register the six Pipeline scenarios of the user manual in the GPUI
screenshot harness, remove the harness's placeholder sample, and judge the
renders against today's images.

## Context

FileManager's manual images (`docs/manual/images/*.png`) will be generated
by the Rust harness from the real GPUI app with fixture backends (a later
milestone replaces the PNGs; this task only registers scenarios). The human
decided: same layout and token colours as today (±1 per channel, geometry
within about 2 px; text widths may differ); CI uploads the PNGs and never
fails on image differences, but a scenario that cannot run fails the job.

The harness (M4; `apps/filemanager/src/shoot/`, feature `shoot`, binary
`filemanager-shoot`, dev profile):

```rust
pub struct Scenario { pub name: &'static str, pub size: (f32, f32),
                      pub setup: fn(&mut AppSetup), pub steps: &'static [Step] }
pub enum Step { SelectTool(ToolId), Click(&'static str), Keys(&'static str), Type(&'static str),
                DragEnter(&'static [&'static str]), Drop(&'static [&'static str]), Watcher(ToolId),
                Call(fn(&mut Window, &mut App)), WaitIdle, Wait(u64) }
pub fn scenarios() -> Vec<Scenario>;   // from scenarios/mod.rs
```

`AppSetup` carries the `FakePlatform` and a `BackendSet` with
`FixtureId::Pipeline(PipelineFixture)`; `apps/filemanager/src/tools/pipeline/fixtures.rs`
(M5) has `PipelineFixture::{Empty, Chosen, Bad, Ran, Demo, Slow}` (`Ran`:
Verify already finished as Not implemented; `Demo`: Verify passed, the batch
step found two problems; `Slow`: `Chosen` with 30 s steps) and configurable step
jobs (result and duration). Run:
`cargo run -p filemanager-gpui --features shoot --bin filemanager-shoot -- --out shots [--only pipeline-]` for light, and the same with `--dark --prefix dark-` for dark (without a prefix, the dark run overwrites the light PNGs)
on Windows at 100% display scale (or read the CI `screenshots` artifact).
`src/shoot/scenarios/mod.rs` holds M4's sample `shell-sample`.

Debug selectors (U5): `pipeline-run-next`, `pipeline-run-all`,
`dialog-confirm`, `dialog-cancel`. Window 1000×700 for every Pipeline shot.

| Scenario | Fixture | Steps | Today's image shows |
|---|---|---|---|
| `pipeline-empty` | `Empty` | `WaitIdle` | no folder; the run bar's reason "Choose a folder." |
| `pipeline-chosen` | `Chosen` (`C:\Deliveries\2026-10`, 12 subfolders) | `WaitIdle` | folder set, nothing run, steps 2–4 locked |
| `pipeline-bad` | `Bad` (`C:\Deliveries\2026-13`, not a folder) | `WaitIdle` | the field error and red input border |
| `pipeline-runall` | `Chosen` | `Click("pipeline-run-all")`, `WaitIdle` | the Run all confirmation over the scrim |
| `pipeline-running` | `Slow` | `Click("pipeline-run-next")`, `Wait(600)` | Run next busy, step 1 Running |
| `pipeline-ran` | `Chosen` (step 1 answers Not implemented) | `Click("pipeline-run-next")`, `Wait(200)` (the fixture's doc: Worker jobs are not awaited by `WaitIdle`), or `Ran` with only `WaitIdle` | step 1 "Not implemented", later steps still locked |

**Scenario tags (planner rule, 2026-11-22, after M4).** The screenshot
harness resolves `Step::Click` and `Scenario::clip` only by `.tag("<name>")`
names (`apps/filemanager/src/app/probe.rs`); GPUI's debug selectors cannot be
read outside `VisualTestContext`, so they do not work in the harness. Every
element a scenario clicks or clips therefore carries `.tag("<name>")` as the
**outermost (last) call of its builder chain**, using the names this plan
lists (where this plan says "debug selector", read "tag"). If a
`VisualTestContext` view test also clicks the element, add
`.debug_selector(..)` with the same name as well.

## Files

Create: `apps/filemanager/src/shoot/scenarios/pipeline.rs`.
Edit: `apps/filemanager/src/shoot/scenarios/mod.rs` (register the six;
**remove** `shell-sample`), `docs/gpui-test-ledger.md` if any row points to
a screenshot.
Do not touch: `src/shoot/` outside `scenarios/`, views, models, fixtures
(a missing fixture capability is a gap to report), `docs/manual/`,
`docs/plans/`.

## Skill

**Confirmation body (from the U5 review, 2026-11-23).** Today the
confirmation shows the folder in mono. U5's body is plain text. During the
critique, choose one:
(1) rich text: the shared widget change is planned in
`U3b-rich-text-and-dialog-defaults.md`, run as its own task once chosen; or
(2) the folder in the spec's `extra` line, rendered mono, with no widget
change, implemented here. Record the choice in the PR. The choice applies
to the Schedule Creator's paths too (U8).

`/impeccable critique` on the six light PNGs and their `dark-` versions (from the `--dark --prefix dark-` run),
against today's images: layout, colours, states, labels. Fix findings in
U5's view files within this task when they are view-only parity misses
(name each fix in the PR); report anything else.

## Acceptance criteria

- `filemanager-shoot --only pipeline-` writes the six PNGs at 1000×700 and
  exits 0 on Windows CI; run1 and run2 are byte-identical (reported).
- By eye, side by side with today's images: same regions, positions and
  colours; differences only in text width and the accepted list (fixed
  dialog shadow, focus ring style).
- `shell-sample` is gone and no test refers to it.
- fmt; clippy pedantic with and without `--features shoot`.

## Tests proving completion

- `shoot::tests::scenario_names_are_unique_and_valid_file_stems` (M4's,
  still passing).
- `shoot::scenarios::pipeline::tests::steps_play` (`#[gpui::test]`: each
  scenario's steps run in a `VisualTestContext` with its fixture and end
  idle; the dialog is open for `pipeline-runall`).
- CI: the harness runs twice, green.

## States covered

Empty, chosen, error, confirmation, running, ran (not implemented).

## Out of scope

Replacing the manual's PNGs and its text (M8).
