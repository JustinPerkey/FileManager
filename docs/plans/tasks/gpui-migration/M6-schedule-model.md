# M6 — Schedule Creator: backend trait, view-model, copy, fixtures

Status: approved (2026-11-21)
Project: gpui-migration   Depends on: M5 (landed)

## Goal

Make the Schedule Creator work in the GPUI app as a fully tested
view-model: a backend trait over `fm_schedule::session::Core`, a model
holding every piece of behaviour today's React view holds (files, drop,
confirmation, apply, result, duplicate warning), the copy ported verbatim,
and fixture backends for every Schedule Creator screenshot.

## Context

FileManager is moving from Tauri + React to GPUI. The backend implementer
owns each tool's **model** (behaviour and view state) and **copy**; the
ui-implementer later writes **views** that only render the model, map its
enums to icons and colours, own focus and layout, and call model methods.
Pin the API below: the UI task plans cite it.

**Follow the Pipeline's shape exactly.** `apps/filemanager/src/tools/pipeline/`
(M5) has `backend.rs` (trait + real impl), `model.rs` (the entity:
`ViewError`, `error_generation`, `load_failed`/`retry_load`, "only the newest
request applies", `announcement`, `take_focus_hint`, `activated()` clearing
transient state), `text.rs`, `fixtures.rs`, `tests.rs`. Reuse its patterns;
if a helper is needed by both tools (for example `clean_path`), move it to
`apps/filemanager/src/app/` rather than importing one tool from the other.

What exists:

- `crates/fm-schedule/src/session/` (M2a): `Core` (`default()`,
  `with_hooks(Hooks)`, `snapshot()`, `set_text(Option<&Path>)`,
  `set_xml(Option<&Path>)`, `set_dropped(&[PathBuf])`,
  `apply(expected_text: &str, expected_xml: &str) -> Result<ApplySummary, ScheduleError>`),
  and `ScheduleSession`, `FileSlot`, `ApplySummary`, `ScheduleError`,
  `ScheduleErrorKind`. Apply always writes a backup of the XML beside it
  before an atomic write; a display-path mismatch writes nothing.
- Today's behaviour: `apps/desktop/src/tools/schedule/ScheduleView.tsx`
  (`cleanPath`, `applyReason`, `resultSentence`, `dropSentence`, the "applied"
  duplicate guard, confirm with the paths shown, drop enabled/disabled
  reasons, restore failure and retry), `errorMessages.ts` (`errorMessage`,
  `fileName`, `toScheduleError`), and `ScheduleView.test.tsx`. Screenshot
  states: `docs/manual/screenshots/mock.js`, `shoot.js` (images
  `schedule-empty`, `-ready`, `-bad`, `-confirm`, `-notimpl`).
- `apps/filemanager`: `src/tools/mod.rs` (`ToolHooks`: `activated`,
  `drop_target` → `DropTarget::{None, Enabled { label }, Disabled { reason }}`,
  `dropped`, `unsaved_work`, `inject_file_changed`), `src/jobs.rs`
  (`Worker`), `src/platform/` (`OpenSpec`, `FileFilter`, `PickKind::File`,
  `PickError`, `FakePlatform`; native Windows dialogs with filters),
  `BackendSet`/`FixtureId` in `src/lib.rs`, placeholder
  `src/tools/schedule/{mod.rs,view/mod.rs}`.
- `docs/gpui-test-ledger.md`: rows for `tools/schedule/ScheduleView.test.tsx`.

### What to build

**`backend.rs`.**

```rust
pub trait ScheduleBackend: Send + 'static {
    fn session(&mut self) -> ScheduleSession;
    fn set_text(&mut self, path: Option<PathBuf>) -> ScheduleSession;
    fn set_xml(&mut self, path: Option<PathBuf>) -> ScheduleSession;
    fn set_dropped(&mut self, paths: Vec<PathBuf>) -> Result<ScheduleSession, ScheduleError>;
    fn apply(&mut self, expected_text: &str, expected_xml: &str) -> Result<ApplySummary, ScheduleError>;
}
pub struct RealSchedule { core: Core }
```

**`text.rs`.** Verbatim: every `ScheduleErrorKind` message of
`errorMessages.ts` (exhaustive `match`, including `ParseFailed`'s use of the
error's detail), the Unexpected copy, `fileName`, `applyReason` texts,
`resultSentence`, `dropSentence`, the drop label and disabled reasons ("Drop
a schedule file, an XML file, or both", "The XML file is being updated",
"Finish or cancel the update first", "Not ready yet"), the header and
intro, field labels and placeholders, browse labels, "Add to XML…" /
"Adding…", the confirmation title and body, the duplicate warning, "Try
again".

**`model.rs`: `ScheduleModel`.**

```rust
pub enum Slot { Text, Xml }
impl ScheduleModel {
    pub fn session(&self) -> Option<&ScheduleSession>;
    pub fn loading(&self) -> bool;  pub fn load_failed(&self) -> bool;  pub fn retry_load(&mut self, cx);
    pub fn error(&self) -> Option<&ViewError>;  pub fn error_generation(&self) -> u64;  pub fn dismiss_error(&mut self, cx);
    pub fn set_typed(&mut self, slot: Slot, typed: &str, cx);
    pub fn browse(&mut self, slot: Slot, cx);
    pub fn apply_reason(&self) -> Option<&'static str>;
    pub fn can_apply(&self) -> bool;
    pub fn applying(&self) -> bool;
    pub fn request_apply(&mut self, cx);                    // opens the confirmation
    pub fn pending(&self) -> Option<(&str, &str)>;          // text and XML display paths in the confirmation
    pub fn confirm(&mut self, cx);  pub fn cancel(&mut self, cx);
    pub fn result(&self) -> Option<&ApplySummary>;
    pub fn already_applied(&self) -> bool;                  // the same pair was applied before: duplicate warning
    pub fn announcement(&self) -> Option<&str>;
    pub fn take_focus_hint(&mut self) -> Option<FocusHint>; // cleared on read; Outcome | AddButton, after applying ends
}
```

Behaviour to port, each a case in `ScheduleView.test.tsx`: typed paths are
cleaned (as Pipeline); only the newest session command's response applies;
any successful session command clears the error and the previous result;
a typed path whose slot comes back with an error announces that error's
message; a browse uses `Platform::pick` with today's filters — the schedule
file: "Text" (`*.txt`) then "All files" (`*.*`); the XML file: "XML"
(`*.xml`) then "All files" (`*.*`) — starting in the folder of the slot's
current file if any; a pick `Err` shows the `PickError` copy; the confirmation records the two display paths
shown and `confirm` applies exactly those (`expected_text`/`expected_xml`);
the result sentence; the duplicate guard (`already_applied` when the current
pair equals the last successfully applied pair); errors from apply
(`ParseNotImplemented`, `MergeNotImplemented`, `FilesChanged`, …) are the
banner, and "nothing was changed" copy comes from `text.rs`.

**Drop.** `ToolHooks::drop_target` is `Enabled { label }` when a session is
loaded and neither applying nor confirming; otherwise `Disabled { reason }`
with today's reason. `dropped(paths)` calls `set_dropped` (a `.xml` file is
the XML file, any other the schedule file; more than allowed gives
`DropAmbiguous`) and announces `dropSentence`.

**`activated()`** clears `error`, `pending`, `result`, `announcement`,
the pending focus hint (React's `useState`), keeps the "applied" pair only if today's
view keeps it across remounts (it does not: clear it too), and refreshes
the session. An apply in progress continues and its result lands.

**`fixtures.rs`:** `FixtureSchedule` constructors for `empty()`, `ready()`
(both files valid), `bad()` (a slot with an error), and an apply outcome
that is `MergeNotImplemented` (for `schedule-notimpl`); `confirm` is reached
by scenario steps from `ready()`. Fake display paths only.

**Fixture timing rule (planner, 2026-11-22, after M4).** The harness's
`Step::WaitIdle` is a short fixed settle, not idle detection: it does not
await `Worker` jobs, `spawn_thread` jobs or async loads. So every fixture
backend state a scenario shows must answer **synchronously** (the backend
method returns at once, so the Worker call resolves inside the settle), or
the scenario that shows it must add an explicit `Step::Wait(ms)` long
enough for the job. Fixtures that simulate long work (progress, a running
step) are timed on purpose; document each fixture's timing in its doc
comment so the scenario author knows which `Wait` it needs.

### Rules that bind this task

- Views never touch the filesystem or a domain crate. No blocking I/O on
  the UI thread: every backend call goes through the `Worker`.
- **Destructive operations are never silent:** apply updates an XML file;
  it is always confirmed with both paths shown, applies exactly those, and
  the backend writes a backup first. A failure says nothing was changed.
- Paths stay `PathBuf`; display strings only for display and the
  expected-path check.
- Tools never import each other (shared helpers go to `src/app/`).
- Tests touching the filesystem use temp dirs; `FakePlatform` for dialogs.
- Ownership: `backend.rs`, `model.rs`, `text.rs`, `fixtures.rs`, `mod.rs`,
  `tests.rs` are yours; `view/` beyond the placeholder is the
  ui-implementer's. No visual decisions.
- fmt clean; clippy pedantic `-D warnings`; scoped `#[expect]` only.

## Files

Create: `apps/filemanager/src/tools/schedule/{backend.rs,model.rs,text.rs,fixtures.rs,tests.rs}`.

Edit: `apps/filemanager/src/tools/schedule/{mod.rs,view/mod.rs}` (placeholder
only), `apps/filemanager/src/{lib.rs,tools/mod.rs}`, possibly
`apps/filemanager/src/app/` (a shared `clean_path`), the Pipeline module if
`clean_path` moves out of it, `docs/gpui-test-ledger.md` (Schedule rows),
`CLAUDE.md` (Status line).

Do not touch: `crates/` (report a needed change instead), `apps/desktop/`,
`docs/plans/`.

## Acceptance criteria

- The API above exists with doc comments.
- Every `ScheduleView.test.tsx` row of the ledger is `ported`, `U` or
  `dropped: <reason>`; none `open`.
- `text.rs` covers every string of `errorMessages.ts` (a test reads the TS
  file with `include_str!` while it exists and checks each quoted message).
- Fixtures reproduce the five Schedule screenshot states.
- fmt, clippy pedantic (with and without `--features shoot`), and
  `cargo test --workspace` pass on both jobs.

## Tests proving completion

`apps/filemanager/src/tools/schedule/tests.rs`:

- `text_covers_every_error_kind`, `text_matches_ts_sources`.
- `loads_session_and_retries_after_failure`.
- `typed_paths_set_slots`, `typed_path_error_is_announced`,
  `only_newest_command_applies`, `browse_sets_slot_and_cancel_does_nothing`,
  `browse_passes_todays_filters`, `browse_error_shows_dialog_failure_copy`.
- `apply_reason_and_can_apply`.
- `apply_confirms_with_shown_paths_and_applies_exactly_those`,
  `cancel_applies_nothing`, `result_sentence_after_apply`,
  `same_pair_twice_warns_duplicate`, `apply_error_is_banner_and_session_kept`,
  `session_command_clears_result_and_error`.
- `drop_target_enabled_and_disabled_reasons`, `drop_sets_slots_and_announces`,
  `ambiguous_drop_is_error`.
- `activated_clears_transient_state`.
- `real_backend_apply_writes_backup_in_temp_dir` (`RealSchedule` with hooks
  that succeed, temp dir: the XML is updated and a backup exists beside it).
- `fixtures_build_each_screenshot_state`.

## Out of scope

- Views, focus and scenarios (UI tasks). The parse/merge hooks (still
  stubs). The Tauri Schedule Creator (cutover).

## Risks

- A filter only guides the user ("All files" is always offered); the
  backend's checks (`is_xml`, parse) stay authoritative.
