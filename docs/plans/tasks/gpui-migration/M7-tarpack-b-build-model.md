# M7b — Tar Packager, part B: model for output, format, build, progress, overwrite, results and shortcuts

Status: approved (2026-11-21)
Project: gpui-migration   Depends on: M7a (landed)

## Goal

Finish the Tar Packager's GPUI view-model: choosing the output and format,
building with progress off the UI thread, the confirmed overwrite, the
success and failure results with their report text, reveal and copy, the
developer-mode extras, and the shortcut predicates, with the copy ported
verbatim and fixtures for the build-side screenshots.

## Context

FileManager is moving from Tauri + React to GPUI. The backend implementer
owns each tool's **model** and **copy**; the ui-implementer later writes
**views** that only render the model, own focus, layout and key bindings,
and call model methods. Pin the API below; the UI task plans cite it.

What exists (read first):

- `apps/filemanager/src/tools/tarpack/` from M7a: `backend.rs` (the full
  `TarpackBackend` trait, including `set_output`, `set_format`,
  `begin_build` → `BuildWork`, `finish_build`, `abort_build`,
  `reveal_target`; `RealTarpack`), `model.rs` (`TarpackModel`, manifest
  side: `ViewError`, `error_generation`, `announcement`, `take_focus_hint`,
  `activated()`, `drop_target`, the watcher and `changed_on_disk`, `show_errors`,
  `can_edit`), `text.rs`, `fixtures.rs`, `tests.rs`. Extend these files.
- `crates/fm-tarpack/src/session/`: `ProgressCoalescer` and `coalescing`
  (at most one event per 50 ms, the final event always delivered),
  `BuildJob::run(overwrite, progress)`, `TarpackSession` fields used here
  (`format`, `formats: Vec<ArchiveFormatOption>`, `output_path`,
  `suggested_output_name`, `can_build`, `build_blocked_reason`,
  `ready_count`, `total_count`), `fm_tarpack::archive::{BuildSummary, Progress, BuildPhase}`.
- `apps/filemanager/src/jobs.rs` (`spawn_thread` with a progress channel),
  `src/platform/` (`pick_save`, `reveal`, `copy_text`, `FakePlatform`),
  `DevMode::is_on` and `cx.observe_global::<DevMode>`.
- Today's behaviour, which you port: `apps/desktop/src/tools/tarpack/TarpackView.tsx`
  (`runBuild`, `onChooseOutput` and its format notice "Format changed to …
  to match the file name.", `onFormatChange`, `onReveal`, `dismissResult`,
  the overwrite confirmation and `overwriteBody`, the shortcut options),
  `BuildBar.tsx` (`reasonText`, `leftOutNote`, the save dialog's start path
  `outputPath ?? suggestedOutputName`), `BuildResult.tsx` (sizes, copy
  buttons, reveal, dev-mode extraction command), `BuildReport.tsx`,
  `FailureList.tsx`, `FormatPicker.tsx`, `BuildProgress.tsx`,
  `reportText.ts` (`resultHeading`, `resultAnnouncement`, `reportText`),
  `useTarpackShortcuts.ts`, and their tests
  (`TarpackView.build.test.tsx`, `BuildBar.test.tsx`, `BuildResult.test.tsx`,
  `BuildReport.test.tsx`, `FailureList.test.tsx`, `FormatPicker.test.tsx`,
  `reportText.test.ts`, `useTarpackShortcuts.test.tsx`). Screenshot
  states in `docs/manual/screenshots/mock.js`/`shoot.js`:
  `tarpack-buildbar`, `-building`, `-replace`, `-result`,
  `-result-partial`, `-dev`, `-dev-result`, `-shortcuts`.
- `docs/gpui-test-ledger.md`: the rows marked M7b.

### What to build

**Model API added to `TarpackModel`.**

```rust
pub enum BuildOutcome { Ok(BuildSummary), Err(TarpackError) }
pub enum Shortcut { Open, Reload, Edit, Build, ShowErrors, Escape }
impl TarpackModel {
    pub fn choose_output(&mut self, cx);              // Platform::pick_save
    pub fn format_notice(&self) -> Option<&str>;
    pub fn set_format(&mut self, format: ArchiveFormat, cx);
    pub fn build_reason(&self) -> Option<String>;     // reasonText
    pub fn left_out_note(&self) -> Option<String>;    // leftOutNote
    pub fn can_build(&self) -> bool;                  // session can_build and not building/confirming
    pub fn building(&self) -> bool;
    pub fn progress(&self) -> Option<&Progress>;
    pub fn progress_unavailable(&self) -> bool;
    pub fn build(&mut self, cx);                      // overwrite = false
    pub fn confirming_overwrite(&self) -> Option<&str>; // the output display path named in the dialog
    pub fn confirm_overwrite(&mut self, cx);  pub fn cancel_overwrite(&mut self, cx);
    pub fn result(&self) -> Option<&BuildOutcome>;  pub fn dismiss_result(&mut self, cx);
    pub fn reveal_output(&mut self, cx);
    pub fn copy_report(&mut self, cx);                // Platform::copy_text(report_text)
    pub fn copy_extraction_command(&mut self, cx);    // dev mode only
    pub fn shortcut(&self, s: Shortcut, cx: &App) -> bool; // whether the key acts now
    pub fn run_shortcut(&mut self, s: Shortcut, window: &mut Window, cx);
}
```

**Behaviour to port:**

- **Output.** The save dialog starts in the folder of the current output
  path, else of the loaded manifest, else Documents; the suggested name is
  the current output's file name, else `suggested_output_name`. Paths come
  from state as `PathBuf` (add `Core::output_path() -> Option<PathBuf>` to
  `fm_tarpack::session` if it is missing; that and its test are the only
  allowed crate change). The filter is today's: name
  "<format name> archive (<extension>)", pattern `*.<filter_extension>`
  (`ArchiveFormat::filter_extension`, the last suffix, e.g. `zst` for
  `.tar.zst`), and `default_extension` = the same suffix. The backend still
  normalises the full extension to the format (`with_format`). When the
  chosen name changes the format, `format_notice` says "Format changed to
  <name> to match the file name."; any later format or output change, a
  reload, or a new build clears it as today.
- **Build.** `build` refuses silently unless `can_build`. It clears the
  previous result and progress, sets `building`; `begin_build` on the
  Worker; `BuildWork::run` on `spawn_thread`, with the progress callback
  wrapped in `coalescing` and sending into the channel; a foreground task
  drains progress into the model (`notify` per event); then `finish_build`
  on the Worker. A panic in the job calls `abort_build` and is an Unexpected
  failure ("the build stopped unexpectedly"). If the progress channel cannot
  be read, `progress_unavailable` is true (today's flag) and the build still
  finishes.
- **Outcomes.** Success: `result = Ok(summary)`, announcement
  `resultAnnouncement`. `OutputExists` without overwrite: no result; opens
  the overwrite confirmation naming the output (`overwriteBody`); confirm
  rebuilds with `overwrite = true`; cancel writes nothing. `ManifestChangedOnDisk`:
  sets `changed_on_disk` (announced once), no result. Any other error:
  `result = Err(e)`, announcement = its message from `text.rs`.
- **Results.** `reveal_output` uses `reveal_target()` (the path held in
  state, never a display string) and `Platform::reveal`; a failure is the
  `OpenerFailed` banner. `copy_report` copies `reportText`. The extraction
  command is shown and copyable only while developer mode is on; turning
  developer mode off hides it at once (observe the global).
- **Shortcuts** (`useTarpackShortcuts.ts`): all inactive while building or
  confirming, except as below. Ctrl+O open; F5 or Ctrl+R reload (only with a
  manifest); Ctrl+E edit (only in developer mode with a manifest);
  Ctrl+Enter build (only when `can_build`); F8 show errors (only with
  errors); Escape dismisses the drop result first, else the build result,
  and does nothing while the shortcuts popover is open (the view tells the
  model with `set_shortcuts_open(bool)`). Auto-repeat never runs an action
  twice (the view passes only fresh presses). The webview-reload guard of
  today is dropped (there is no webview); record it in the ledger.
- **Activation.** `activated()` also clears `format_notice`,
  `confirming_overwrite` and `announcement`; a running build continues and
  its result stays until dismissed; a result that finished while the tool
  was not current is kept.

**`text.rs` additions.** Verbatim: `reasonText`, `leftOutNote`,
`resultHeading`, `resultAnnouncement`, `reportText`, `overwriteBody` and the
overwrite dialog title and buttons, the format names, the format notice,
size formatting of `BuildResult.tsx`, copy-button labels and their "Copied"
state text, progress phase labels, and every other string the build bar,
result, report, failure list, format picker and shortcuts help render
(the shortcuts list itself, without the developer-mode chord).

**`fixtures.rs` additions.** Build fixtures: a build that takes long enough
to capture progress (`tarpack-building`: progress events at fixed points,
then waits until released), an `OutputExists` first answer
(`tarpack-replace`), full and partial successful summaries
(`tarpack-result`, `-result-partial`), the dev-mode states (`-dev`,
`-dev-result`), and the ready state for `-buildbar` and `-shortcuts`.

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

- **Destructive operations are never silent:** an existing output is
  replaced only after the confirmation, which names the file; the writer is
  atomic; a failed or partial build is reported with every failure.
- No blocking I/O on the UI thread; the build on `spawn_thread`, the rest
  on the Worker.
- Paths stay `PathBuf`; reveal uses the held path; display strings only for
  display.
- Developer mode is app-wide and off at launch; dev-only actions check it.
- Tests touching the filesystem use temp dirs; `FakePlatform` for dialogs,
  reveal and clipboard.
- Ownership as in M7a; no visual decisions.
- fmt clean; clippy pedantic `-D warnings`; scoped `#[expect]` only.

## Files

Edit: `apps/filemanager/src/tools/tarpack/{model.rs,text.rs,fixtures.rs,tests.rs,mod.rs}`,
possibly `crates/fm-tarpack/src/session/core.rs` (only `output_path`),
`docs/gpui-test-ledger.md`, `CLAUDE.md` (Status: the Tar Packager model is
ported).

Do not touch: other crates, `apps/desktop/`, `view/` beyond the placeholder,
`docs/plans/`.

## Acceptance criteria

- The API above exists with doc comments.
- Every ledger row marked M7b is `ported`, `U` or `dropped: <reason>`; after
  this milestone no Tar Packager row is owned by M7a or M7b and still
  `open`.
- `text.rs` covers `reportText.ts`, `reasonText` and `leftOutNote` (tests
  compare against today's test vectors).
- The fixtures produce the eight build-side screenshot states.
- fmt, clippy pedantic (with and without `--features shoot`) and
  `cargo test --workspace` pass on both jobs.

## Tests proving completion

In `apps/filemanager/src/tools/tarpack/tests.rs`:

- `report_text_matches_today`, `reason_and_left_out_note_cases` (plain tests,
  ported from `reportText.test.ts` and `BuildBar.test.tsx`).
- `choose_output_starts_in_output_folder_with_name`, `choose_output_passes_format_filter`,
  `output_name_changing_format_sets_notice`, `notice_cleared_by_later_change`.
- `build_runs_off_thread_with_ordered_progress` (real backend, temp dir,
  `allow_parking`), `build_success_sets_result_and_announces`,
  `existing_output_asks_and_confirm_overwrites`,
  `cancel_overwrite_writes_nothing` (the existing file's bytes unchanged),
  `changed_on_disk_during_build_marks_changed`,
  `build_error_is_result_with_message`, `panicked_build_aborts_and_reports`.
- `reveal_uses_held_path`, `copy_report_copies_report_text`,
  `extraction_command_only_in_dev_mode`, `dev_mode_off_hides_extraction_command`.
- `shortcuts_gating_matches_today` (one case per row of
  `useTarpackShortcuts.test.tsx` that is about gating),
  `escape_dismisses_drop_result_then_build_result`,
  `escape_ignored_while_shortcuts_open`.
- `activated_keeps_running_build_and_result`.
- `fixtures_build_each_build_state`.

## Out of scope

- The build bar, progress, result, report, overwrite dialog and shortcuts
  help views, key bindings, focus, and scenarios (UI tasks).
- Removing the Tauri Tar Packager (cutover).

## Risks

- Progress throttling must keep the final event; the coalescer guarantees
  it, and the test checks the last event is final.
- Closing the window during a build ends the process as today; the atomic
  writer leaves at most a temp file.
- A FAIL in the human's Windows 11 checklist on the "feel" of a large table
  belongs to the table's UI task, not here.
