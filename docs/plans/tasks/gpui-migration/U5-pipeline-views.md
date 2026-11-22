# U5 — Pipeline views

Status: approved (2026-11-21)
Project: gpui-migration   Depends on: U4 (landed), M5 (landed)

## Goal

Replace the Pipeline's placeholder view with today's Pipeline screen,
rendering `PipelineModel` and calling its methods, keyboard operable and
matching today's layout and colours.

## Context

FileManager is moving from Tauri + React to GPUI + gpui-component. The
human decided the UI looks like today: same layout and token colours;
sampled colours within ±1 per channel; geometry within about 2 px of
today's `docs/manual/images/pipeline-*.png`; text widths may differ. Views
**render the model and call its methods**: behaviour, copy and predicates
are in the model and `text.rs` (owned by the backend implementer). A need
the model lacks is a gap to report, not view code. Every interactive
element is keyboard reachable and labelled.

The Pipeline: the user chooses one folder; four steps run over its
subfolders in order (check required files, run batch files, schedule, pack
archives), one at a time ("Run next"; each unlocks once the one before
passes) or all at once ("Run all…", stopping at the first that does not
pass); steps that change files ask first.

### Model API (`apps/filemanager/src/tools/pipeline/model.rs`, M5)

```rust
pub enum ViewError { Pipeline(PipelineError), Unexpected { message: String } }
pub enum Pending { Step { id: StepId, via_next: bool }, All }
pub enum StepStatus { Running, NotImplemented, Failed, Passed, Problems(u32), Waiting, NotRun }
impl PipelineModel {
    pub fn session(&self) -> Option<&PipelineSession>;   // directory slot, steps (StepView: id, report, unlocked, …)
    pub fn loading(&self) -> bool;  pub fn load_failed(&self) -> bool;  pub fn retry_load(&mut self, cx);
    pub fn error(&self) -> Option<&ViewError>;  pub fn error_generation(&self) -> u64;  pub fn dismiss_error(&mut self, cx);
    pub fn set_directory_text(&mut self, typed: &str, cx);  pub fn browse(&mut self, cx);  pub fn clear_directory(&mut self, cx);
    pub fn busy(&self) -> bool;  pub fn running(&self) -> Option<StepId>;  pub fn via_next(&self) -> Option<StepId>;
    pub fn run_reason(&self) -> Option<&'static str>;  pub fn can_run(&self) -> bool;
    pub fn next_step(&self) -> Option<StepId>;  pub fn all_passed(&self) -> bool;
    pub fn step_status(&self, id: StepId) -> StepStatus;
    pub fn request_step(&mut self, id: StepId, cx);  pub fn request_next(&mut self, cx);  pub fn request_all(&mut self, cx);
    pub fn pending(&self) -> Option<(&Pending, &str)>;     // and the folder named in the confirmation
    pub fn confirm(&mut self, cx);  pub fn cancel(&mut self, cx);
    pub fn announcement(&self) -> Option<&str>;            // not rendered (see below)
    pub fn focus_hint(&self) -> Option<FocusHint>;         // Step(id) | RunNext
}
```

Copy (`tools/pipeline/text.rs`): the header, intro, "Folder", placeholder,
"Browse…", the subfolder note, step titles/descriptions/confirm sentences,
status labels, run reasons, "Run next: <title>", "Run all…", "Run",
"Run…", "Try again", "All steps passed.", the confirmation title/body, the
lock line "Runs after <title> passes.", every error message. Use them; do
not write copy in the view.

**Focus hints.** The planner was asked (request R5) to make hints
consumable: `take_focus_hint(&mut self) -> Option<FocusHint>`. Use it if it
exists; otherwise act on `focus_hint()` only when it differs from the last
value acted on, and report the gap in the PR.

Announcements (`announcement()`) were visually hidden live regions; they are
**not rendered**.

### Widgets and tokens (`crate::widgets`, `crate::theme`)

`text::{h1, h2, intro, muted, small_muted, mono}`, `button` (primary,
secondary, `.busy`, `.disabled`), `text_input` (`.mono()`, `.invalid()`,
`.disabled()`), `Banner` (`Tone::Error`, `.details`, `.action`,
`.dismiss`), `status` with `StatusTone::{Ok, Warn, Danger, Muted}`,
`step_number`, `sync_confirm` + `ConfirmSpec`. Tokens by name: `bg`,
`surface`, `surface_sunken`, `border`, `text`, `text_muted`, `ok`, `warn`,
`danger`, `accent`; `SPACE_1..5`.

### Layout (today's `pipeline.css`; reference `pipeline-chosen.png`)

The view scrolls vertically as a whole, padding 16 top/bottom, 24
left/right; regions 16 px apart:

1. **Header**: h1 "Pipeline" (20 px, 700); 4 px below, the intro (muted,
   max 546 px). Measured: title at x 216, y 16 (with the 192 px sidebar).
2. **Load failure** (`load_failed`): an error banner with the error copy and
   **Try again** (`retry_load`); nothing else renders. While `loading`
   with no session: nothing but the header.
3. **Folder**: "Folder" (16 px, 600); 8 px below, a row (12 px gap,
   wrapping): the folder input (mono, flex, min width 0, height 32 px; input
   at y 134 in the reference) and **Browse…** (tooltip "Browse for the
   folder"); 8 px below: the field error in `danger` (and the input
   `.invalid`) when the directory slot has an error, else the muted note
   ("12 subfolders"). The input shows the session's directory display path;
   the user's typing is kept until committed: **Enter** or losing focus
   commits (`set_directory_text`) when the text differs from the session's;
   **Escape** reverts the typing. Disabled while `busy`.
4. **Error banner** (`error()`), dismissible, with Details (the technical
   message); re-shown when `error_generation` changes even if the text is
   the same.
5. **Run bar** (12 px gap, wrapping): **Run next: <title>** (primary, busy
   when `via_next` is that step; disabled unless `can_run`; at y 209 in the
   reference), **Run all…** (secondary; disabled unless `can_run`), then the
   reason (`run_reason`) or "All steps passed." in muted.
6. **Steps**: a column with a 1 px `border` rule above and under each step;
   step padding 12/0, inner gap 8; head row (12 px gap): `step_number(n)`,
   the text column (title 16 px 600, description muted, and when locked the
   lock line sm muted 4 px below), the status (min width 144 px:
   `step_status` → Passed: ok + `check-circle`; Problems(n): warn +
   `alert-triangle`; Failed, NotImplemented: danger + `x-circle`; Running:
   muted + `circle`; Waiting, NotRun: muted + `circle`; label from
   `text.rs`), and the step's button: **Run** or **Run…** (when the step has
   confirm copy), disabled when locked or `!can_run`, busy when running.
   Under the head, indented 36 px: the report summary and the issues list
   (bulleted, 16 px indent, wrapping).
7. **Confirmation** (`pending()`): `sync_confirm` key `"pipeline-confirm"`
   with the title/body from `text.rs` naming the folder, Cancel focused,
   confirm primary with the step's confirm label; confirm → `confirm`,
   cancel/Escape → `cancel`.

Focus: after a run, `FocusHint::Step(id)` focuses that step's row (rows are
focusable for this, not in the Tab order); `RunNext` focuses Run next.

Debug selectors: `pipeline-folder-input`, `pipeline-browse`,
`pipeline-run-next`, `pipeline-run-all`, `pipeline-step-<n>` (1–4),
`pipeline-run-<n>`, `pipeline-error`, `pipeline-retry`.

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

Create: `apps/filemanager/src/tools/pipeline/view/{folder.rs,run_bar.rs,steps.rs,tests.rs}`.
Edit: `apps/filemanager/src/tools/pipeline/view/mod.rs` (replace the
placeholder), `docs/gpui-test-ledger.md` (Pipeline rows marked U).
Do not touch: `model.rs`, `text.rs`, `backend.rs`, `fixtures.rs`, other
tools, `crates/`, `apps/desktop/`, `docs/plans/`.

## Skill

None here; U6 critiques the screenshots.

## Acceptance criteria

- Every region above renders from the model, with copy only from `text.rs`.
- Keyboard: Tab reaches the input, Browse, Run next, Run all and every
  enabled step button; Enter/Escape in the input as specified; the
  confirmation starts on Cancel.
- Every control has a visible label or tooltip.
- No colour literal outside `theme`; no filesystem or domain-crate call.
- fmt; clippy pedantic `-D warnings`; `cargo test --workspace` on both jobs.

## Tests proving completion

`tools/pipeline/view/tests.rs` (`#[gpui::test]`, fixture backend,
`FakePlatform`; wiring, not pixels):

- `renders_each_fixture_state` (`empty`, `chosen`, `bad`, `ran`: the
  expected selectors exist; the error line shows for `bad`).
- `enter_commits_typed_folder`, `blur_commits_and_escape_reverts`.
- `browse_button_calls_platform_pick`.
- `run_next_click_and_keyboard_run_step`,
  `confirming_step_opens_dialog_and_cancel_runs_nothing`.
- `run_buttons_disabled_with_reason`.
- `error_banner_reshows_on_new_generation`, `retry_after_load_failure`.
- `focus_follows_hint_after_run_next`.

Ledger: settle every `PipelineView.test.tsx` row M5 marked U (rendering,
labels, focus placement) to these tests, or `dropped: <reason>` for
aria-live and aria-describedby rows.

## States covered

Loading, load failure, empty (no folder), folder chosen, folder error,
running (Run next busy, a step busy), ran (passed, problems, failed, not
implemented), all passed, confirmation, error banner.

## Out of scope

Scenarios (U6); model behaviour (M5).
