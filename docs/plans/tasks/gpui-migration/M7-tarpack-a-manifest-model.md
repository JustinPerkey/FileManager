# M7a — Tar Packager, part A: backend trait; model for the manifest, recent list, watcher, assignments and drop

Status: approved (2026-11-21). **Review follow-up (2026-11-22):** the first run landed as f5542ff and 30bd7d7; the next run does only the section "Review follow-up" below.
Project: gpui-migration   Depends on: M6 (landed)

## Goal

Give the GPUI app the Tar Packager's full backend trait over
`fm_tarpack::session::Core`, and the first half of its view-model:
loading and restoring, opening (dialog, recent, create from example),
reloading, the "changed on disk" watcher, the manifest's errors, entry
assignment by browse, clear and drop, with the copy ported verbatim and
fixtures for the manifest-side screenshots. Part B adds the build side.

## Review follow-up (2026-11-22)

The review of f5542ff and 30bd7d7 did not approve M7a yet. **This run does
only this section.** Everything else in this plan is built. Read the code
first: `apps/filemanager/src/tools/tarpack/`, `src/platform/`, and
`crates/fm-tarpack/src/session/`.

### Decisions

1. **Opener and reveal failures are reported.**
   - Today the Tauri app reports `OpenerFailed` as "Windows could not open
     it.", but `Platform::open_with_system` and `Platform::reveal` return
     `()`, so a failure never reaches the model.
   - Change both to return `Task<Result<(), OpenError>>`, with
     `pub struct OpenError { pub detail: String }`, in
     `apps/filemanager/src/platform/mod.rs`.
   - **On Windows, our own thin wrapper**, `platform/win_shell.rs`
     (`#[cfg(windows)]`, the `windows` crate). GPUI's calls return nothing,
     so they cannot be used.
     - `open_with_system` uses `ShellExecuteW(None, "open", path, …, SW_SHOWNORMAL)`.
       A return value of 32 or less is an error; map the documented
       `SE_ERR_*` and `ERROR_*` codes to the detail text.
     - `reveal` uses `SHParseDisplayName` plus `SHOpenFolderAndSelectItems`.
       If that fails with "file not found", fall back to `ShellExecuteW` on
       the parent folder, as GPUI does. A failed `HRESULT` is the error.
     - Paths go in as wide strings from `OsStr::encode_wide`.
     - Both run on a background thread initialised with `CoInitializeEx`
       (apartment-threaded), never on the UI thread.
     - Each `unsafe` block has a `// SAFETY:` comment.
   - Off Windows, keep GPUI's calls and return `Ok(())`.
   - **`FakePlatform`** gains scripted answers for open and reveal
     (`answer_open(Err(..))`; the default is `Ok(())`), and still logs every
     call.
   - **Callers.** The model maps an `Err` to the banner
     `ViewError::Tarpack(TarpackError { kind: OpenerFailed, message: detail })`,
     with copy from `text.rs` ("Windows could not open it."). This milestone
     updates `edit` and every other existing caller. Part B (`reveal_output`)
     and the Database Manager (Edit config, kind `OpenerFailed`) use the same
     result.
2. **Browse starts in the assigned folder, from the stored path.**
   - Add `fn assigned_path(&self, id: &str) -> Option<PathBuf>` to the
     backend trait, backed by a new `Core::assigned_path`, which reads the
     `PathBuf` that `Assignments` already holds. That accessor and its test
     are the only `crates/` change allowed here.
   - `browse` starts in that path's parent.
   - Remove the use of the display string `SessionEntry::assigned` for this.
     The old React `assignedFolder` refused lossy (U+FFFD) display paths; with
     the real `PathBuf` there is nothing to refuse.
   - Port its other cases as model tests: drive root, nested, forward-slash
     and UNC parents, and "no parent" means no start folder.
   - In `docs/gpui-test-ledger.md`, move rows 501–503 and 505
     (assignedFolder/displayFolder) from `U` to `M7a`, with these tests.
     displayFolder's "shows a lossy folder as is" half stays `U` if a view
     renders it.
3. **`open_recent` cannot open the wrong manifest.**
   - The signature becomes
     `open_recent(index, expected_display: &str) -> Result<Option<TarpackSession>, TarpackError>`.
   - The backend checks that the display string of `recent_paths()[index]`
     equals `expected_display`. If it does not (the list was reordered before
     the model's copy refreshed), it returns `Ok(None)` and opens nothing.
   - The model then refreshes its recent list and shows the banner
     `ViewError::RecentChanged`, a new variant whose copy in `text.rs` is
     "The recent list changed. Choose the manifest again."
   - Successful results that change the recent list also refresh the model's
     copy.
4. **Screenshot fixtures match the reference images.**
   - Today's mock gateway manifest
     (`docs/manual/screenshots/mock.js`) has 5 entries: the example's 3, plus
     `gateway-ca` and `gateway-unit`, with their specific diagnostics.
   - The fixture manifest for every `tarpack-*` state appends those two
     entries and diagnostics, as `gateway_with_errors` already does, so the
     screenshots match the geometry target. Port the entries and diagnostic
     texts exactly from `mock.js`.

### Also in this run

- Tests for:
  - discarding a stale command result (only the newest applies);
  - refetching the session after a command panics: add a command-panic hook
    to `FixtureTarpack`, e.g. `panic_on(Command)`;
  - refetching after a drop panics;
  - the watcher re-arming on open, reload and create;
  - ignoring events from a superseded watcher (an event from the old
    watcher after a re-arm changes nothing).
- Move `default_start_dir()` (the Documents and home lookup) off the UI
  thread: compute it on the Worker or with `cx.background_spawn`, before
  opening the dialog.
- Move `directories` to `[workspace.dependencies]` and use
  `{ workspace = true }` in every crate that uses it.
- Fix the `DETAILS` doc comment so it says what the constant is for.
- In `docs/gpui-test-ledger.md`, the row "open and reload announce, repeat
  on same count, and clear announces No errors" should name the model test
  that proves the **repeat** (a reload with the same count announces again,
  through `announcement_generation`, not only the text). Add that test if it
  is missing.

### Follow-up acceptance

- Every item above has a named test, apart from the live Windows shell
  calls. For those, add steps to `docs/gpui-dialogs-check.md`: Edit with no
  `.toml` handler shows the banner, and Reveal opens Explorer with the file
  selected.
- `cargo tree -p filemanager-gpui -i rfd` still finds nothing. fmt (linux
  job), clippy pedantic (both jobs, with and without `--features shoot`),
  and `cargo test --workspace` pass.

## Context

FileManager is moving from Tauri + React to GPUI. The backend implementer
owns each tool's **model** (behaviour and view state) and **copy**; the
ui-implementer later writes **views** that only render the model, map its
enums to icons and colours, own focus, layout and key bindings, and call
model methods. Pin the API below; the UI task plans cite it.

**Follow the shape of the Pipeline and Schedule Creator models**
(`apps/filemanager/src/tools/pipeline/` and `…/schedule/`: `backend.rs`,
`model.rs` with `ViewError`, `error_generation`, `announcement`,
`take_focus_hint`, `activated()`, "only the newest request applies", `text.rs`,
`fixtures.rs`, `tests.rs`). Shared helpers live in `apps/filemanager/src/app/`.

What exists:

- `crates/fm-tarpack/src/session/` (M2b): `Core` (`new(&AppDirs)`,
  `unpersisted(String)`, `manifest_path`, `session` (first call restores the
  most recent manifest), `recent_manifests`, `open`, `reload`, `assign`,
  `clear`, `begin_drop` → `DropJob`, `finish_drop(&job, outcome) -> Result<Option<DropOutcome>>`
  (`None` = the session changed meanwhile), `set_format`, `set_output`,
  `create_from_example`, `begin_build` → `BuildJob`, `finish_build`,
  `abort_build`, `manifest_path_for_open`, `reveal_target`, `snapshot`),
  `DropJob::run(&[PathBuf])`, `BuildJob::run(overwrite, impl FnMut(Progress))`,
  `ProgressCoalescer`/`coalescing`, and the types `TarpackSession`,
  `SessionManifest`, `SessionEntry`, `ArchiveFormatOption`,
  `BuildBlockedReason`, `TarpackError`, `TarpackErrorKind`,
  `DroppedAssignment`. `fm_tarpack::sources::DropOutcome`,
  `fm_tarpack::archive::{BuildSummary, Progress}`.
- `crates/fm-core/src/watch.rs`: `FileWatcher::start(path, on_change: Fn() + Send)`,
  `path()`; 300 ms debounce; watches the parent directory.
- `apps/desktop/src-tauri/src/tools/tarpack/mod.rs`: how the commands use
  `Core` today, in particular `arm_watcher` (re-arm on open, reload and
  create; keep the watcher when the path is unchanged on `session`) and
  `tarpack_assign_dropped` (up to 3 attempts when the session changed while
  the walk ran, then the error "the session changed while the dropped files
  were being matched; drop them again").
- Today's behaviour, which you port: `apps/desktop/src/tools/tarpack/TarpackView.tsx`
  (`apply` with its `Origin` = load / reload / other, `manifestSentence`,
  the skeleton after 150 ms, restore failure, the changed-on-disk flag and
  its one-time announcement, state warning and its dismissal, the errors
  section expanded state and `showErrors`, `onOpen`, `onOpenRecent`,
  `onReload`, `onEdit`, `onCreate`, `onBrowse`, `onClear`, `onDrop`,
  `dropAvailability`), `DropResult.tsx` (`buildLines`, `itemText`,
  `lineText`, `dropResultText`, the cap on listed items), `ManifestHeader.tsx`,
  `ManifestErrors.tsx`, `DiagnosticList.tsx`, `errorMessages.ts`, and the
  tests `TarpackView.{states,assign,errors}.test.tsx`, `DropResult.test.tsx`,
  `ManifestHeader.test.tsx`, `ManifestErrors.test.tsx`,
  `DiagnosticList.test.tsx`, `errorMessages.test.ts`. Screenshot states in
  `docs/manual/screenshots/mock.js`/`shoot.js`: `tarpack-empty`, `-recent`,
  `-loaded`, `-errors`, `-missing`, `-partial`, `-changed`, `-drag`,
  `-dropped`, `-withheld`, `-ready`.
- `apps/filemanager`: `ToolHooks` (`activated`, `drop_target`, `dropped`,
  `unsaved_work`, `inject_file_changed`), `jobs::{Worker, spawn_thread}`,
  `Platform` (`pick`, `pick_save`, `open_with_system`, `reveal`,
  `copy_text`; `OpenSpec`, `SaveSpec`, `FileFilter`, `PickError`; native
  Windows dialogs with filters), `DevMode::is_on`, `BackendSet`/`FixtureId`,
  placeholder `src/tools/tarpack/{mod.rs,view/mod.rs}`.
- `docs/gpui-test-ledger.md`: the Tar Packager rows marked M7a.

### What to build

**`backend.rs`: the whole trait** (part B uses the build half).

```rust
pub trait TarpackBackend: Send + 'static {
    fn session(&mut self) -> TarpackSession;
    fn recent_manifests(&mut self) -> Vec<String>;
    fn open(&mut self, path: &Path) -> Result<TarpackSession, TarpackError>;
    fn open_recent(&mut self, index: usize) -> Result<TarpackSession, TarpackError>; // by position, see below
    fn reload(&mut self) -> Result<TarpackSession, TarpackError>;
    fn create_from_example(&mut self, path: &Path) -> Result<TarpackSession, TarpackError>;
    fn assign(&mut self, id: &str, path: &Path) -> Result<TarpackSession, TarpackError>;
    fn clear(&mut self, id: &str) -> Result<TarpackSession, TarpackError>;
    fn begin_drop(&mut self) -> Result<Box<dyn DropWork>, TarpackError>;
    fn finish_drop(&mut self, work: Box<dyn DropWork>) -> Result<Option<DroppedAssignment>, TarpackError>;
    fn set_output(&mut self, path: &Path) -> Result<TarpackSession, TarpackError>;
    fn set_format(&mut self, format: ArchiveFormat) -> Result<TarpackSession, TarpackError>;
    fn begin_build(&mut self) -> Result<Box<dyn BuildWork>, TarpackError>;
    fn finish_build(&mut self, work: Box<dyn BuildWork>) -> Result<BuildSummary, TarpackError>;
    fn abort_build(&mut self);
    fn manifest_path(&self) -> Option<PathBuf>;            // for the watcher
    fn manifest_path_for_open(&self) -> Result<PathBuf, TarpackError>;
    fn reveal_target(&self) -> Result<PathBuf, TarpackError>;
}
pub trait DropWork: Send + 'static { fn run(&mut self, paths: &[PathBuf]); }
pub trait BuildWork: Send + 'static { fn run(&mut self, overwrite: bool, progress: &mut dyn FnMut(Progress)); }
pub struct RealTarpack { core: Core }   // Core::new(&AppDirs::from_system()) or unpersisted(reason), as `state()` does today
```

Shape the work objects as you see fit (they carry the job and its outcome
between `run` on a thread and `finish_*` on the Worker); keep the semantics.
**Recent manifests are opened by the stored path, never by re-parsing a
display string:** if `Core` only returns display strings today, add a
`recent_paths()` accessor to `fm_tarpack::session::Core` returning the
stored `PathBuf`s (the only `crates/` change allowed here, with a test), and
open by index into that list.

**`text.rs`.** Verbatim: every `TarpackErrorKind` message of
`errorMessages.ts` (exhaustive `match`; entry-specific messages use the
entry's file name as today), the Unexpected copy, `manifestSentence`, the
drop result's lines and `dropResultText` (port `buildLines`, `itemText`,
`lineText` as pure functions over `DropOutcome` and the entries, with the
same cap), the drop label and disabled reasons of `dropAvailability`, "The
manifest changed on disk.", and every string the manifest header, errors
section, diagnostics list and empty state render. Part B adds the build
strings.

**`model.rs`: `TarpackModel`, manifest side.**

```rust
pub enum DropState { Pending, Result(DropReport) }        // DropReport: outcome, entries, has_failed_entries, lines
impl TarpackModel {
    pub fn session(&self) -> Option<&TarpackSession>;
    pub fn loading(&self) -> bool;  pub fn show_skeleton(&self) -> bool;  // true once loading lasted 150 ms
    pub fn load_failed(&self) -> bool;
    pub fn error(&self) -> Option<&ViewError>;  pub fn error_generation(&self) -> u64;  pub fn dismiss_error(&mut self, cx);
    pub fn state_warning(&self) -> Option<&str>;             // None once dismissed for this text
    pub fn dismiss_state_warning(&mut self, cx);
    pub fn changed_on_disk(&self) -> bool;
    pub fn errors_expanded(&self) -> bool;  pub fn set_errors_expanded(&mut self, on: bool, cx);
    pub fn show_errors(&mut self, cx);                       // expand + a `Report` focus hint; no-op with 0 errors
    pub fn recent(&self) -> &[String];  pub fn open_recent(&mut self, index: usize, cx);
    pub fn open(&mut self, cx);                              // Platform::pick, filter "Manifest" (*.toml)
    pub fn reload(&mut self, cx);
    pub fn can_edit(&self, cx: &App) -> bool;                // dev mode on and a manifest loaded
    pub fn edit(&mut self, cx);                              // Platform::open_with_system(manifest_path_for_open)
    pub fn create_from_example(&mut self, cx);               // Platform::pick_save, "Manifest" (*.toml), default extension toml
    pub fn browse(&mut self, entry_id: &str, cx);            // pick File, starting in the assigned folder
    pub fn clear(&mut self, entry_id: &str, cx);
    pub fn drop_state(&self) -> Option<&DropState>;  pub fn dismiss_drop_result(&mut self, cx);
    pub fn drop_pending_shown(&self) -> bool;
    pub fn announcement(&self) -> Option<&str>;
    pub fn take_focus_hint(&mut self) -> Option<FocusHint>; // cleared on read; Report | Reload | AfterResult, as today's focus moves
}
```

Behaviour to port (each a test case today):

- `apply(next, origin)`: every success clears the error; load and reload
  also clear `changed_on_disk`, drop a drop **result** (not a pending drop),
  and expand the errors section when the manifest has errors; reload also
  clears the build result and the format notice (part B's fields: add them
  now as `Option`s if needed, or leave a hook part B fills) and gives
  a `Reload` focus hint if a result held focus. One announcement per result:
  a new state warning (not the dismissed one) followed by the manifest
  sentence.
- Watcher: after a load, reload or create, (re)start a `FileWatcher` on
  `manifest_path()` (start it on the Worker; keep it in the model; drop it
  when no manifest is loaded). Its callback sends into an `async_channel`; a
  foreground task sets `changed_on_disk` and announces "The manifest changed
  on disk." once until the next load or reload.
  `ToolHooks::inject_file_changed` does the same (screenshots and tests).
- Create from example: start in the loaded manifest's folder, else the
  user's Documents folder (`directories::UserDirs`), else the home folder;
  suggested name `manifest.toml`; filter "Manifest" (`*.toml`), default
  extension `toml` (today's filter; Windows appends `.toml` when the user
  types no extension). The backend writes with `create_new` and
  never overwrites; an existing file is its `PathExists` error.
- Open uses the filter "Manifest" (`*.toml`), as today. Browse (assigning
  a source file) has no filter, as today, and starts in the entry's assigned
  folder when it has one.
- Drop: `drop_target` per `dropAvailability` (enabled with the label when a
  manifest with entries is loaded and nothing blocks it; disabled with
  today's reasons while building, confirming or a drop is pending).
  `dropped(paths)`: ignored while a drop is pending; sets `Pending`
  (`drop_pending_shown()` turns true after 150 ms, and after 1 s the model
  announces "Matching dropped files…", as today);
  `begin_drop` on the Worker, `DropWork::run` on `spawn_thread`,
  `finish_drop` on the Worker; `None` (stale) retries up to 3 times in all,
  then the error above; success applies the session and sets `Result` with
  its lines and announces `dropResultText`; an error clears the drop state
  and shows the banner.
- Dialog `PickError` → banner with the `PickError` copy from `app::text`.
- `activated()` clears `error`, `drop_state` (unless a drop is pending),
  `announcement`, the pending focus hint; keeps `changed_on_disk` (it is about the
  file) and the session; refreshes the session.

**`fixtures.rs`.** `FixtureTarpack` constructors for every manifest-side
screenshot state listed above, built from `examples/tarpack/` manifests
(through `include_str!` and `fm_tarpack::manifest` parsing, never reading
files), fake Windows display paths, a recent list, and drop outcomes for
`-dropped` and `-withheld`. Part B adds the build fixtures.

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
  the UI thread: every backend call goes through the `Worker`; the drop walk
  through `spawn_thread`.
- Destructive operations are never silent: create-from-example never
  overwrites; a failed drop is reported; a stale drop is retried, then
  reported.
- Paths stay `PathBuf`; recent manifests open by stored path, not by a
  display string; display strings only for display.
- Tools never import each other; `fm-tarpack` and `fm-core` gain no GPUI or
  tauri dependency.
- Tests touching the filesystem use temp dirs; `AppDirs::at(tempdir)` for
  the store; `FakePlatform` for dialogs, open and reveal.
- Ownership: `src/tools/tarpack/{backend,model,text,fixtures,mod,tests}.rs`;
  `view/` beyond the placeholder is the ui-implementer's. No visual
  decisions.
- fmt clean; clippy pedantic `-D warnings`; scoped `#[expect]` only.

## Files

Create: `apps/filemanager/src/tools/tarpack/{backend.rs,model.rs,text.rs,fixtures.rs,tests.rs}`.

Edit: `apps/filemanager/src/tools/tarpack/{mod.rs,view/mod.rs}` (placeholder
only), `apps/filemanager/{Cargo.toml,src/lib.rs,src/tools/mod.rs}`,
possibly `crates/fm-tarpack/src/session/core.rs` (only `recent_paths`),
`docs/gpui-test-ledger.md`, `CLAUDE.md` (Status).

Do not touch: other crates, `apps/desktop/`, `docs/plans/`.

## Acceptance criteria

- `backend.rs` has the whole trait above and `RealTarpack`; the model has
  the manifest-side API above, with doc comments.
- Every ledger row marked M7a is `ported`, `U` or `dropped: <reason>`.
- `text.rs` covers every `errorMessages.ts` string (test against the TS
  source with `include_str!` while it exists).
- The fixtures produce the eleven manifest-side screenshot states.
- fmt, clippy pedantic (with and without `--features shoot`) and
  `cargo test --workspace` pass on both jobs.

## Tests proving completion

`apps/filemanager/src/tools/tarpack/tests.rs` (`#[gpui::test]` unless noted):

- `text_covers_every_error_kind`, `text_matches_ts_sources`,
  `drop_lines_match_today` (plain tests over `DropResult.test.tsx`'s
  outcomes, including the cap).
- `restores_last_manifest_on_start` (real backend, temp `AppDirs`),
  `skeleton_only_after_150ms`, `restore_failure_sets_load_failed`.
- `open_uses_platform_and_announces`, `open_cancel_does_nothing`,
  `open_passes_manifest_filter`, `open_error_shows_dialog_failure_copy`, `open_recent_uses_stored_path`.
- `reload_clears_changed_and_results`, `load_expands_errors_when_present`,
  `show_errors_expands_and_hints_focus`, `show_errors_noop_without_errors`.
- `state_warning_announced_once_and_dismissal_sticks`.
- `watcher_marks_changed_on_disk_once` (real backend and a real
  `FileWatcher` in a temp dir; `allow_parking` and polling),
  `inject_file_changed_marks_changed`.
- `can_edit_needs_dev_mode_and_manifest`, `edit_opens_manifest_path`.
- `create_from_example_writes_and_loads`, `create_from_example_refuses_existing_file`.
- `browse_starts_in_assigned_folder`, `assign_and_clear_update_entries`.
- `drop_target_reasons`, `drop_assigns_and_reports_lines`,
  `drop_ignored_while_pending`, `pending_shown_after_150ms_and_announced_after_1s`,
  `stale_drop_retries_then_reports`,
  `drop_error_clears_pending`.
- `activated_clears_transient_state`.
- `fixtures_build_each_manifest_state`.

## Out of scope

- Output, format, build, progress, overwrite, results, report text,
  shortcuts predicates (part B).
- Views, the entry table, middle truncation, scenarios (UI tasks).

## Risks

- `FileWatcher` must be `Send` to start on the Worker; if it is not, start
  it on the UI thread only after the path is known (registration is cheap)
  and say so in the PR.
- The real watcher test is timing-based; keep generous timeouts.
- A filter only guides the user; a non-TOML file still fails to parse with a
  diagnostic, as a bad manifest does today.
