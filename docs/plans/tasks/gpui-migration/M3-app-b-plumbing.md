# M3b — App plumbing: `Platform`, path-loss reporting, `jobs.rs`, drop routing, close guard

Status: approved (2026-11-21)
Project: gpui-migration   Depends on: M3a (landed)

## Goal

Give the GPUI app (`apps/filemanager`, package `filemanager-gpui`) the
plumbing every tool needs: a `Platform` trait for dialogs, open, reveal and
clipboard (real and scripted fake), reporting of names GPUI cannot deliver,
background work that never blocks the UI thread, Explorer drop routing to
the current tool, and a close guard that holds the window while a tool has
unsaved work.

## Context

FileManager is moving from Tauri + React to GPUI with gpui-component. M3a
built `apps/filemanager`: `src/lib.rs` (`run`), `src/app/shell.rs` (`Shell`,
the window root view: current tool, `select`, `tools`, `tool_view`, dev
mode), `src/app/view/` (the chrome slot, a placeholder for now; it belongs to
the ui-implementer), `src/tools/mod.rs` (`ToolId`, `ToolSlot`, `ToolHooks`
with `activated`, `build`), placeholder tools, `src/app/dev_mode.rs`,
`src/app/text.rs`. Read them first.

The throwaway spike on branch `spike/gpui-migration` proved every API used
here; port from it by name with
`git show origin/spike/gpui-migration:spike/gpui/<path>`:
`src/app.rs` (dialogs: `prompt_for_paths`, `prompt_for_new_path`; drop:
`on_drop::<ExternalPaths>`, `group_drag_over`; close:
`on_window_should_close`, `remove_window`; `open_with_system`,
`reveal_path`; and its tests `external_drop_delivers_paths`,
`dropped_lone_surrogate_name_survives_routing`, `close_is_held_while_dirty`,
`build_progress_reaches_the_view`), `src/build_job.rs` (throttle, a job on a
`std::thread` sending progress through `async_channel`), `src/watch.rs`.

### Facts from the spike that bind the design

- `cx.prompt_for_paths(PathPromptOptions { files, directories, multiple, prompt })`
  and `cx.prompt_for_new_path(&Path, Option<&str>)` return a `oneshot`
  receiver of `Result<Option<…PathBuf…>>`. **Neither has a file-type
  filter** (which is why M3c replaces them).
- **Names that are not valid Unicode** (a lone UTF-16 surrogate): GPUI's
  open and save dialogs return `Err` for the whole selection; the save
  dialog's starting directory is converted lossily
  (`display().to_string()`). Explorer drop **skips** such a name and only
  logs `log::error!("dragged file name is not UTF-16")` (gpui-pre-windows
  `clipboard.rs`, `with_file_names`). Paths that do reach the app
  (including through `ExternalPaths` routing) stay exact, and
  `open_with_system` / `reveal_path` convert `&Path` losslessly.
- Drag enter, over, leave and drop are distinct platform events
  (`FileDropEvent::{Entered, Pending, Exited, Submit}`); elements get
  `on_drop::<ExternalPaths>` and `drag_over::<ExternalPaths>`
  / `group_drag_over`; `cx.has_active_drag()`.
- `window.on_window_should_close(cx, |window, cx| -> bool)` is called for the
  title-bar X and Alt+F4 (`WM_CLOSE`); returning false holds the close. It is
  registered once per window (a second call replaces the first).
  `window.remove_window()` closes without calling it. Session end cannot be
  held.
- Tests: `VisualTestContext::simulate_event` for drop events,
  `simulate_close()` returns whether the window would close. A test whose
  work runs on a real `std::thread` needs `cx.executor().allow_parking()` and
  a polling loop; work on `cx.background_spawn` is deterministic.

### The human's decisions on paths and dialogs (binding)

- **Explorer drop** is GPUI's, as shipped. **A dropped name that GPUI cannot
  deliver is reported to the user, never skipped silently** (the path-loss
  notice below).
- **File dialogs** must be the native Windows file picker **with file-type
  filters, a default extension and an initial folder**, returning paths as
  `OsString`, without `rfd`. GPUI's dialogs have no filters, so the next
  milestone (M3c) replaces them with our own wrapper over the Windows common
  item dialog. This milestone defines the `Platform` trait with the final
  dialog signatures, and its real implementation calls GPUI's prompts for
  now (ignoring filters), so the app is usable until M3c lands.

### What to build

**`src/platform/mod.rs`: the `Platform` trait.** Every OS interaction a model
needs, so models and screenshots can run against a fake:

```rust
pub enum PickKind { File, Folder }
pub struct FileFilter { pub name: String, pub patterns: Vec<String> }   // patterns like "*.toml"
pub struct OpenSpec { pub kind: PickKind, pub filters: Vec<FileFilter>, pub start_dir: Option<PathBuf>, pub title: Option<String> }
pub struct SaveSpec { pub filters: Vec<FileFilter>, pub default_extension: Option<String>,   // no dot
                      pub start_dir: Option<PathBuf>, pub suggested_name: Option<OsString>, pub title: Option<String> }
pub enum PickError { Failed { detail: String } }        // the dialog could not be shown or returned an error
pub trait Platform: 'static {
    fn pick(&self, spec: OpenSpec, cx: &mut App)
        -> Task<Result<Option<PathBuf>, PickError>>;      // one item; None = cancelled
    fn pick_save(&self, spec: SaveSpec, cx: &mut App)
        -> Task<Result<Option<PathBuf>, PickError>>;
    fn open_with_system(&self, path: &Path, cx: &mut App);
    fn reveal(&self, path: &Path, cx: &mut App);
    fn copy_text(&self, text: &str, cx: &mut App);
}
```

Hold it as a global (`PlatformHandle(Rc<dyn Platform>)`, `platform(cx)`), set
in `run()` to `GpuiPlatform` and in tests and screenshots to `FakePlatform`.
Keep these signatures: the tools' milestones and M3c rely on them.

- `GpuiPlatform` (`src/platform/gpui.rs`): open, reveal and clipboard wrap
  the GPUI calls (`cx.open_with_system`, `cx.reveal_path`,
  `cx.write_to_clipboard(ClipboardItem::new_string(..))`). Its dialogs are a
  **temporary** call to GPUI's `prompt_for_paths` / `prompt_for_new_path`
  that ignores filters and default extension (a `// Replaced in M3c` doc
  comment says so); a GPUI dialog `Err` becomes `PickError::Failed`.
- `FakePlatform` (`src/platform/fake.rs`, always compiled: screenshots use
  it): a script of answers pushed by tests (`answer_pick(Ok(Some(path)))`,
  `answer_pick(Err(..))`, …) consumed in order, and a log of every call
  (`calls() -> Vec<PlatformCall>`) including each dialog's full spec, open,
  reveal and copied text.
  A call with no scripted answer resolves to `Ok(None)` (cancelled).
- `PickError`'s user copy, in `src/app/text.rs`: "The file dialog could not
  be shown. See Details." Models show it with the `detail` as technical
  detail.

**`src/platform/path_loss.rs`: reporting dropped names GPUI skipped.**
Install a `log::Log` implementation at startup in `run()` (`log` is
already in GPUI's graph; add it as a direct dependency at the locked
version). It forwards nothing anywhere else except, in debug builds, to
stderr. It recognises the record GPUI writes when it skips a dropped name:
message exactly `PATH_LOSS_MESSAGE = "dragged file name is not UTF-16"`,
level error, target starting with `gpui`. On a match it sends `()` on an
`async_channel`. The Shell drains the channel on the foreground executor
and sets `Shell::notice` to `Some(ShellNotice::DroppedNamesUnreadable)`,
**coalescing** matches that arrive within one drag (from `Entered` to
`Submit`/`Exited`, or within 1 s if no drag events come) into one notice.
Copy in `src/app/text.rs`: "Some dropped items could not be read: their
names are not valid Unicode. Rename them and drop them again." The notice
stays until `Shell::dismiss_notice`. The logger must be safe to install
once (ignore `SetLoggerError` in tests that build several apps).

**`src/jobs.rs`: no blocking I/O on the UI thread.**

```rust
/// Owns a value off the main thread and runs closures on it one at a time,
/// in submission order, on GPUI's background executor.
pub struct Worker<T: Send + 'static> { … }
impl<T: Send + 'static> Worker<T> {
    pub fn new(value: T, cx: &App) -> Self;
    pub fn run<R: Send + 'static>(&self, f: impl FnOnce(&mut T) -> R + Send + 'static, cx: &App) -> Task<R>;
}

/// Runs a long job on its own std::thread. The job gets a progress sender;
/// the caller gets the receiver and a task for the result. A panic in the
/// job resolves the task to Err(JobPanicked) (message text inside).
pub fn spawn_thread<P: Send + 'static, R: Send + 'static>(
    name: &str,
    job: impl FnOnce(async_channel::Sender<P>) -> R + Send + 'static,
    cx: &App,
) -> (async_channel::Receiver<P>, Task<Result<R, JobPanicked>>);
```

Worker: one long-lived background task draining an unbounded channel of
boxed closures; each `run` returns a task resolving to the closure's result.
A panic inside a closure must not kill the worker for later calls: catch it
(`catch_unwind` with `AssertUnwindSafe`) and resolve that call's task to an
error the caller can see (make `run` return `Task<Result<R, JobPanicked>>` if
that is simplest; document the choice). Models own their tool's backend in a
`Worker` and use `spawn_thread` for archive builds, folder walks and pipeline
steps. Write the module doc comment stating the rule: models never call a
backend or the filesystem on the UI thread.

**`src/app/drop.rs` and the Shell: drop routing.** Today's behaviour is
`apps/desktop/src/app/DropZone.tsx`: a full-window target; while a drag is
over the window an overlay shows the current tool's label, or, when the
tool cannot accept a drop now, its reason; a drop on a disabled target is
ignored; an empty drop is ignored.

- `ToolHooks` gains (with defaults meaning "no drop target"):

  ```rust
  fn drop_target(&self, cx: &App) -> DropTarget;          // DropTarget::{None, Enabled { label }, Disabled { reason }}
  fn dropped(&self, paths: Vec<PathBuf>, window: &mut Window, cx: &mut App);
  ```

  (labels and reasons are `SharedString`s from the tool's copy).
- The Shell's root `div` handles `on_drop::<ExternalPaths>` and drag
  enter/leave, keeps `Shell::drag_over() -> Option<DropTarget>` (Some while a
  file drag is over the window and the current tool has a target), notifies
  on change, and on drop calls the current tool's `dropped` only if its
  target is `Enabled` and the path list is non-empty. The chrome (UI) renders
  the overlay from `drag_over()`.

**`src/app/guard.rs` and the Shell: close guard.** Only the tools know about
unsaved work (the Database Manager's drafts, later).

- `ToolHooks` gains `fn unsaved_work(&self, cx: &App) -> Vec<SharedString>`
  (default empty): one short description per item.
- In `run()`, register `on_window_should_close` once for the main window.
  It asks every tool; if all are empty, the window closes. Otherwise it
  returns false and sets `Shell::close_request() -> Option<&CloseRequest>`
  (`CloseRequest { items: Vec<(ToolId, SharedString)> }`) and notifies.
- `Shell::request_close(window, cx)` runs **the same path** as the
  window's should-close hook (ask every tool; close at once if nothing is
  unsaved, otherwise set the close request), so screenshot scenarios and
  view tests can raise the prompt without a real `WM_CLOSE`. The hook
  itself calls the same internal function.
- `Shell::confirm_close(window, cx)` closes the window (`remove_window`),
  which quits the app; `Shell::cancel_close(cx)` clears the request. A second
  close attempt while a request is showing keeps the existing request (no
  duplicate). The prompt itself (copy, buttons, focus) is the UI's; it calls
  these two methods.

### Rules that bind this task

- No filesystem or other blocking I/O on the UI thread.
- Destructive operations are never silent: closing with unsaved work is
  held and confirmed; a name lost by a drop is reported.
- Paths stay `Path`/`PathBuf`/`OsStr`; only Explorer drop is lossy (GPUI's),
  and it is reported as above. The temporary GPUI dialogs are lossy until
  M3c replaces them; do not add code that relies on that.
- Tests touching the filesystem use temp dirs; tests never use real user
  files. `FakePlatform` never touches the filesystem.
- You own `src/platform/`, `src/jobs.rs`, `src/app/*.rs`, `src/tools/mod.rs`.
  Do not style anything; extend the placeholder chrome only enough to show
  the drag overlay text, the notice and the close request as plain text with
  plain buttons ("Keep editing", "Close") so the app is usable before the
  UI tasks.
- The `fm-*` crates are not touched. Tools never import each other.
- `test-support` stays a dev-dependency feature only.
- fmt clean; clippy pedantic `-D warnings`; scoped `#[expect]` only.

## Files

Create: `apps/filemanager/src/platform/{mod.rs,gpui.rs,fake.rs,path_loss.rs}`,
`apps/filemanager/src/jobs.rs`, `apps/filemanager/src/app/{drop.rs,guard.rs}`.

Edit: `apps/filemanager/{Cargo.toml,src/lib.rs,src/app/mod.rs,src/app/shell.rs,src/app/text.rs,src/app/view/placeholder.rs,src/tools/mod.rs}`,
`Cargo.lock`, `docs/gpui-test-ledger.md` (the `DropZone.test.tsx` rows this
milestone ports), `CLAUDE.md` (layout lines for `platform/`, `jobs.rs`,
`app/`).

Do not touch: `crates/`, `apps/desktop/`, `docs/plans/`.

## Acceptance criteria

- The Shell's public API includes `drag_over`, `notice`, `dismiss_notice`,
  `close_request`, `request_close`, `confirm_close`, `cancel_close`, as specified, each with
  a doc comment; `ToolHooks` has `activated`, `drop_target`, `dropped`,
  `unsaved_work`.
- No model-facing API in `platform/` or `jobs.rs` performs I/O on the
  calling (UI) thread.
- The path-loss pin test passes on Windows CI.
- Every `DropZone.test.tsx` row of the ledger is settled (ported to a named
  test here, or marked U for the overlay's look).
- fmt, clippy pedantic and `cargo test --workspace` pass on both jobs; the
  dumpbin and self-check steps still pass.

## Tests proving completion

`cargo test -p filemanager-gpui`:

- `jobs::tests::worker_runs_in_order` (100 closures append to a `Vec`;
  order preserved), `worker_result_arrives_on_main_thread`,
  `worker_survives_a_panicking_closure`,
  `spawn_thread_progress_is_ordered_and_final_arrives` (uses
  `allow_parking`), `spawn_thread_panic_is_reported`.
- `platform::fake::tests::answers_in_order_and_logs_calls`,
  `unscripted_pick_is_cancelled`.
- `platform::fake::tests::records_dialog_specs`.
- `platform::path_loss::tests::matching_record_signals`,
  `other_records_do_not_signal`, `matches_within_one_drag_coalesce`.
- `platform::path_loss::tests::gpui_message_text_is_pinned`
  (`#[cfg(windows)]`): find the `gpui-pre-windows` package's manifest path
  with `cargo metadata --format-version 1 --locked` (run `env!("CARGO")`
  with `current_dir` at the workspace root), and assert some `.rs` file in
  its `src/` contains the literal `PATH_LOSS_MESSAGE`. Its failure message
  says: GPUI changed how it reports undeliverable dropped names; update
  `path_loss.rs` before upgrading.
- `app::shell::tests::drop_reaches_current_tool_when_enabled`,
  `drop_ignored_when_disabled`, `empty_drop_ignored`,
  `drag_over_tracks_enter_and_exit`, `drop_goes_to_the_current_tool_only`
  (drive `FileDropEvent` through `VisualTestContext::simulate_event`, as the
  spike's `external_drop_delivers_paths` does).
- `app::shell::tests::dropped_non_unicode_path_survives_routing`
  (`#[cfg(windows)]`, port of the spike's test).
- `app::shell::tests::path_loss_shows_one_notice_per_drag`.
- `app::guard::tests::close_without_unsaved_work_closes`,
  `close_with_unsaved_work_is_held_and_requested`,
  `cancel_keeps_window_open`, `confirm_closes_window`,
  `request_close_matches_the_hook` (same outcome as `simulate_close`, clean
  and dirty),
  `second_close_while_requested_does_not_duplicate`
  (`simulate_close`, as the spike's `close_is_held_while_dirty`).

## Out of scope

- The look of the overlay, notice and prompt (UI tasks).
- Any tool model (M5 on). The screenshot harness (M4).
- The native Windows dialog wrapper with filters (M3c).
- File watching (each tool milestone wires `fm_core::watch::FileWatcher`
  through a channel into its model).

## Risks

- The path-loss detection relies on GPUI's log text; the pin test turns an
  upgrade that changes it into a CI failure, not silence.
- The live behaviour (real dialogs, Explorer drop, the X button, Alt+F4)
  cannot be seen on CI. The human's Windows 11 checklist run covers it; a
  FAIL there reopens this milestone.
- A modal native dialog is asynchronous in GPUI; a model must not assume the
  session is unchanged when the answer arrives (re-check state then).
