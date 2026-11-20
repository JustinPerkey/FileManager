# GPUI spike report (part A)

Branch `spike/gpui-migration`, throwaway, never merged. Part B appends its own
items and the summary. Evidence: CI run
<https://github.com/JustinPerkey/FileManager/actions/runs/44798160684>
(workflow `spike-gpui`, commit `35efb41`, windows job on `windows-2025` =
Windows Server 2025, build 26100, no GPU: "Microsoft Hyper-V Video"; linux job
on `ubuntu-latest`). Artifact `fm-spike` holds `fm-spike.exe`, the dumpbin
output, DLL list, timings, licenses and the self-check result.

Effort: about one working session for part A, far under the 3-day box. Nothing
was patched in GPUI or gpui-kit. Windows behaviour that a person must see is
NEEDS-HUMAN until `CHECKLIST.md` is run; where the pinned source already
shows the answer, the source reading is recorded and the checklist step is
written to confirm it.

## Versions actually used

- `gpui-component` **0.10.0** (newest, 2026-11-16, Apache-2.0). No fallback to
  another release was needed.
- GPUI = crates.io **`gpui-pre` 0.3.12** (the exact pin of gpui-component
  0.10.0; lib name `gpui`). Newer `gpui-pre` 0.3.13 exists; not used.
- The platform backends are **separate crates** that the task text did not
  mention: `gpui-pre-platform` =0.3.12 (`gpui_platform::application()`),
  which pulls `gpui-pre-windows` (Windows), `gpui-pre-linux`, `-wgpu`, `-macos`
  and others. The spike depends on `gpui-pre-platform` with
  `default-features = false`. (`gpui-kit` 0.10.0 is an umbrella crate that
  re-exports all of this; not used.)
- `rustc 1.100.0` (stable at run time), edition 2024 in all GPUI crates, no
  `rust-version` declared by any of them. Edition 2024 needs Rust 1.85+; the
  repo's `rust-version = "1.90"` is enough. The spike builds with 1.100.

## A1 - Portable exe with a static C runtime: PASS (exe start: self-check only)

- `cargo build --release --locked` succeeds on `windows-2025`;
  root `.cargo/config.toml` (`+crt-static`) applies to `spike/gpui` (it is
  under the repo root): the verbatim dumpbin step passed, no `vcruntime*`,
  `msvcp*`, `api-ms-win-crt-*`, `zstd*`, `liblzma*`, `sqlite3*`,
  `WebView2Loader.dll`. The exe links all five `fm-*` crates (zstd, liblzma
  and SQLite statically; `domain::link_report` calls into each and the unit
  test `link_report_covers_all_five_crates` runs one SQLite open).
- `dumpbin /dependents` (full output in the artifact, `dumpbin-dependents.txt`):
  kernel32, advapi32, oleaut32, api-ms-win-core-synch-l1-2-0, user32, ole32,
  userenv, shell32, combase, **icuuc**, ntdll, uiautomationcore, dwrite, gdi32,
  comctl32, dwmapi, dxgi, d3d11, winmm, imm32, api-ms-win-shcore-scaling-l1-1-1,
  dcomp, api-ms-win-core-winrt-l1-1-0, api-ms-win-core-winrt-error-l1-1-0,
  bcryptprimitives, gdiplus. All are Windows system DLLs / API sets. Notes:
  `icuuc.dll` is the OS-provided ICU (Windows 10 1703+; no bundled copy);
  `uiautomationcore`, `dcomp`, `gdiplus` come from GPUI's accessibility and
  DirectComposition use. No delay-loaded imports.
- `d3dcompiler_47.dll` is **not** imported and the string does not occur in
  the exe: GPUI's `build.rs` (gpui-pre-windows) compiles the HLSL with `fxc.exe`
  from the Windows SDK at build time in release profiles (needs the SDK on the
  build machine; fine on CI), so no runtime shader compiler is needed.
- Manifest: **GPUI already embeds it** (`gpui-pre` default feature
  `windows-manifest`, via `embed-resource`, resource id 1): `asInvoker`,
  per-monitor-v2 DPI (`dpiAware true/pm` + `dpiAwareness PerMonitorV2`),
  SegmentHeap, Common-Controls 6, and `supportedOS` Windows 10/11 GUID
  `{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}`. So the spike embeds no manifest.
  Plan discrepancy: the task text gives `{8e0f7a12-bfb3-4fe8-b9a5-1d49d3a3d0bd}`
  as "the one GUID Windows 11 uses"; to my knowledge Windows 10 and 11 share
  the `...48fd50a15a9a` GUID and no Windows-11-only GUID exists. Either way
  there is no manifest field that refuses Windows 10.
- Icon: `build.rs` (crate `winresource`, needs `rc.exe` from the SDK) embeds
  `apps/desktop/src-tauri/icons/icon.ico` as resource id 1, which is where
  GPUI's `load_icon` looks. Build is green; that it displays is checklist
  step 28 (CI cannot see it).
- Windows-11-only API: `DWMWA_SYSTEMBACKDROP_TYPE` (build 22621+) is used by
  GPUI only for the optional blurred/mica window background and is guarded by
  a build check; the spike does not request a backdrop. The spike calls no
  other Windows-11-only API.
- Start from a folder holding only the exe: `--self-check` (opens the window,
  renders the first frame during `open_window`, quits after 500 ms) exited
  **0** (copied alone to `only-the-exe`); results file: "self-check: window opened and rendered one
  frame". So GPUI's DirectX 11 renderer starts on the runner's software
  adapter (WARP via Hyper-V Video, no GPU), and a GUI process gets a usable
  desktop session on GitHub's Windows runner. That answers both risks in the
  plan; it is a good sign that part B's screenshots can run in CI.
- Fail would have meant: no-go. Not triggered.

## A2 - Native file dialogs: PARTIAL (API fine; two gaps, one decisive for non-Unicode names)

GPUI API (`App`): `prompt_for_paths(PathPromptOptions { files, directories,
multiple, prompt }) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>>` and
`prompt_for_new_path(&Path directory, Option<&str> suggested_name) ->
oneshot::Receiver<Result<Option<PathBuf>>>`. Windows backend: the modern
`IFileOpenDialog` / `IFileSaveDialog`, owner window set, dialog runs without
blocking the event loop.

- Open file / folder / multiple: supported by `PathPromptOptions`
  (`FOS_PICKFOLDERS` for folders; files and folders cannot be mixed).
  NEEDS-HUMAN for the live run (checklist 4-6). Headless tests cannot show a
  native dialog.
- **Save: no extension filter.** The suggested name works
  (`SetFileName`), but `file_save_dialog` always sets one filter, "All
  files" `*.*`. The parameter list has no filter. Fallback tried: `rfd` 0.18
  (Windows only, `default-features = false`, no extra DLL: the dumpbin result
  above is with rfd linked in; `FileDialog::add_filter("Tar archive (gz)",
  &["gz"])` + `set_file_name("gateway.tar.gz")`), run on a worker thread so
  the GPUI loop is not blocked. Button "Save with filter (rfd)" (checklist 10).
- **Non-Unicode names (lone surrogate): FAIL in both**, from the source:
  GPUI's `file_open_dialog` does `item.GetDisplayName(SIGDN_FILESYSPATH)?
  .to_string()?`, and `PWSTR::to_string` errors on invalid UTF-16, so a
  selection containing such a name makes the **whole** dialog return `Err`
  (the user sees nothing happen; the caller gets an error). rfd's Windows
  backend does `String::from_utf16(slice).unwrap()`, so it **panics** (the
  spike wraps it in `catch_unwind` and reports "PANICKED"). Checklist steps
  7-8 confirm. The std route is fine: after choosing a folder the check
  screen lists it with `std::fs::read_dir`, which returns the name as an
  `OsString`; the Windows-only test `dropped_lone_surrogate_name_survives_routing`
  proves a path containing `U+D800` survives GPUI's `ExternalPaths` event
  routing and the view unchanged, and Open/Reveal take `&Path` and convert with
  `HSTRING::from(&OsStr)` (lossless). So the loss is only at the two OS
  boundaries GPUI converts through `String`.
- A true fix needs a small `windows`-crate dialog using `OsString::from_wide`
  (about 80 lines of COM) instead of GPUI's and rfd's: not built, over the
  one-fallback allowance, estimated half a day plus Windows testing.
- Fail means: not no-go (dialogs work for ordinary names); non-Unicode names
  are rare but the product rule ("paths stay platform path type") is not met
  by GPUI as shipped. Needs a decision.

## A3 - Drop from Explorer: PARTIAL (works for ordinary names; non-Unicode names are dropped silently)

- API: `ExternalPaths(SmallVec<[PathBuf; 2]>)` (`.paths()`), delivered by
  `InteractiveElement::on_drop::<ExternalPaths>(...)`. Overlay: the platform
  sends distinct `FileDropEvent::{Entered {position, paths}, Pending, Submit,
  Exited, Ended}` (Windows `IDropTarget`: `DragEnter` -> Entered, `DragOver` ->
  Pending, `DragLeave` -> Exited, `Drop` -> Submit), so enter and leave are
  **distinct events, not inferred**. The element API turns that into
  `drag_over::<T>` / `group_drag_over::<T>` styles and `cx.has_active_drag()`;
  the check screen's "Drop files here" overlay uses `group_drag_over`.
  Folders arrive as `CF_HDROP` paths like files. `DROPEFFECT_COPY` is always
  reported while a file drag is over the window.
- Headless: `external_drop_delivers_paths` drives Entered, Exited,
  Entered, Pending, Submit through `VisualTestContext::simulate_event`
  with synthetic `ExternalPaths`: both paths (a file and a folder) arrive,
  and `has_active_drag` is true after Entered and false after Exited and after
  Submit. Passes on Windows and Linux CI.
- **Lone surrogate: FAIL, from the source.** `with_file_names` (gpui-pre-windows
  `clipboard.rs`) converts each `DragQueryFileW` buffer with `String::from_utf16`
  and, on error, only `log::error!("dragged file name is not UTF-16")` and
  **skips the path**. The drop succeeds with that path missing, silently. Not
  confirmed live; checklist 14-15 confirm. Also the whole path goes through
  `PathBuf::from_str(&String)`, so even valid names are re-encoded from a
  `String`.
- Fix would be revoking GPUI's drop target and registering our own
  `IDropTarget` on the HWND (via `raw-window-handle`) that builds
  `OsString::from_wide`; not built (one-fallback allowance, cannot be tested
  without a desktop). Cost estimate one day with Windows testing.
- Fail means: no-go criterion is "drop does not work at all"; it works, so not
  no-go. The non-Unicode gap is a decision for the human (same as A2).

## A4 - Window-close interception: PASS headless; NEEDS-HUMAN live

- API: `Window::on_window_should_close(cx, Fn(&mut Window, &mut App) -> bool)`
  (false holds the close); Windows: `WM_CLOSE` is routed to it
  (`handle_close_msg` returns `Some(0)` when the callback says no), and both the
  title-bar X and Alt+F4 raise `WM_CLOSE`. Programmatic close:
  `Window::remove_window()` (does not call the hook). The check screen holds the
  close while "Dirty" is ticked and shows "Close without saving?" with
  **Keep editing** / **Close**; Close closes the window, and the process exits
  through `on_window_closed` + `cx.quit()`.
- Test `close_is_held_while_dirty` (`VisualTestContext::simulate_close`
  returns whether the window would close): clean = closes; dirty = held, the
  confirmation shows; Keep editing leaves one window open; Close removes the
  window (0 windows). Passes on Windows and Linux CI. This tests the hook,
  not the real `WM_CLOSE` delivery; checklist 16-20.
- The hook is registered per window, once (replaces the previous one). Session
  end (`WM_QUERYENDSESSION` always allowed) cannot be held through it.

## A5 - System editor and Explorer: PASS in source (NEEDS-HUMAN live)

- API: `App::open_with_system(&Path)` and `App::reveal_path(&Path)`. Windows:
  `ShellExecuteW("open", ...)`, and `SHOpenFolderAndSelectItems` (with a
  `ShellExecute` of the folder as fallback when the shell reports "file not
  found"); both run on the background executor; paths convert via
  `HSTRING::from(&Path/&OsStr)`, so non-Unicode names are fine here. So the
  `windows`-crate fallback of the plan is not needed. Checklist 21-24.
- Fail means: soft.

## A6 - Headless tests on Windows CI: PASS

- `cargo test --locked` (feature `test-support` as a dev-dependency) passed
  three runs in a row on `windows-2025` with no GPU: run 1 252.7 s (includes
  the test-profile compile), runs 2 and 3 2.1 s and 1.7 s. 12 tests, identical
  results on `ubuntu-latest` (0.62 s of test time).
- Tests: `job_finishes_on_main_thread` (model test: `#[gpui::test]`,
  `TestAppContext`; an entity method starts `cx.background_spawn`, state
  changes on the main thread after `run_until_parked`),
  `input_accepts_keystrokes` (gpui-component `InputState`, focus,
  `simulate_input("abc")`, value == "abc"), `button_click_runs_handler`
  (`debug_selector` + `debug_bounds` + `simulate_click`; works because tests
  are debug builds: selectors are not recorded in release),
  `external_drop_delivers_paths`, `dropped_lone_surrogate_name_survives_routing`
  (Windows only), `close_is_held_while_dirty`, `build_progress_reaches_the_view`,
  `progress_is_throttled_and_ordered`, `final_event_survives_a_huge_interval`,
  `throttle_admits_first_then_waits_for_interval`, `watcher_reports_edit`,
  `link_report_covers_all_five_crates`, `append_adds_lines`.
- Note: tests for a view that uses a real thread need
  `cx.executor().allow_parking()` and a polling loop (see
  `build_progress_reaches_the_view`); pure GPUI-executor tests do not.
- `gpui_component::init(cx)` is required before any gpui-component widget;
  tests work without a `Root` wrapper.

## A7 - Background work, progress and watching: PASS headless

- Mechanism: a plain `std::thread` runs `run_job` and sends `Progress` through
  an `async_channel` (unbounded) with `send_blocking`; the view does
  `cx.spawn(async move |this, cx| { while let Ok(p) = rx.recv().await {
  this.update(cx, ...) } })`, so every UI mutation happens on the foreground
  executor and `this` is a `WeakEntity`. Constraints found: the worker closure
  must be `Send + 'static` (it owns only the sender); entities and `Window`
  are `!Send` and never cross; `Context::spawn` takes an `AsyncFnOnce`
  (`async move |this, cx|`), a 2024-edition-era API; the task must be kept or
  `.detach()`ed (the view stores them). `cx.background_spawn` is the
  GPUI-executor equivalent for non-looping jobs and is deterministic in tests.
- The job runs for wall-clock 5 s with 1 ms ticks and a 50 ms throttle
  (done = elapsed ms, because a 1 ms sleep really lasts ~15 ms on Windows).
  `progress_is_throttled_and_ordered`: strictly increasing, fewer than 16
  events for 300 ms at a 20 ms throttle, exactly one final event, always last.
  Typing stays responsive by construction (the worker never touches the UI
  thread); the live feel is checklist 25.
- Watcher: `notify-debouncer-mini` 0.7, 300 ms, parent directory, filtered to
  the file; callback runs on the watcher's thread and pushes into an
  `async_channel`; `watcher_reports_edit` (temp dir) passes on Windows and
  Linux. Live view update: checklist 26-27.

## A8 - Size, build time, toolchain, licenses: PASS (report only)

On `windows-2025` (rustc 1.100.0, 4-core standard runner):

| Measure | Value |
| --- | --- |
| Release exe size (LTO, 1 CGU, as the root profile) | 24,424,448 bytes (23.3 MiB) |
| Release exe size without LTO (16 CGUs) | 27,271,168 bytes |
| Clean release build (no cache, fat LTO, 1 CGU) | 619.6 s |
| Release rebuild after touching `src/app.rs` (deps built) | 222.8 s |
| Clean release build without LTO | 561.1 s |
| Debug build, deps from scratch | 86.4 s (test-profile compile 252.7 s inside test run 1) |
| Debug rebuild after touching `src/app.rs` | 8.6 s |

Disabling LTO barely helps a clean build (most time is compiling ~800 crates);
LTO costs about 220 s on every release rebuild. Linux (`ubuntu-latest`):
clippy 2 m 04 s, debug build 2 m 11 s, test compile 60 s.

- Crates in `Cargo.lock`: **825** (all targets: it includes macOS, Linux, iOS,
  Android, wasm crates that a Windows build does not compile).
- Licenses (`licenses.txt`, via `cargo metadata`): overwhelmingly MIT and
  Apache-2.0 and their OR combinations, plus Unicode-3.0, BSD-2/3, ISC, Zlib,
  0BSD, CC0, Unlicense, **MPL-2.0 (3 crates)**. Copyleft strings appear in
  exactly three entries, all **dual-licensed with a permissive alternative**
  (choice is ours): `r-efi` (`MIT OR Apache-2.0 OR LGPL-2.1-or-later`, listed
  twice for two versions) and `self_cell` (`Apache-2.0 OR GPL-2.0-only`). No
  crate is GPL/AGPL/LGPL only. MPL-2.0 is file-level copyleft: list for the
  human. Zero unknown licenses.
- Toolchain: GPUI crates declare no `rust-version`, all are edition 2024
  (Rust 1.85+); builds with the repo's `rust-version = "1.90"` floor
  and with 1.100. The spike's own `rust-version` is 1.90.

## A9 - Linux build: PASS

`ubuntu-latest` (a non-blocking job): `cargo fmt --check`, clippy,
`cargo build` and `cargo test` all pass **with no apt packages installed**
(the runner's default image only). The Linux `gpui-pre-linux` backend compiles
without webkit/gtk dev packages (X11/Wayland by dlopen). The 12 tests run on
the headless test platform. Not tested: running a Linux window.

## A10 - clippy pedantic: PASS with 3 scoped `#[expect]`s

`cargo clippy --all-targets --locked -- -D warnings` is clean on both jobs.
Scoped `#[expect]`s (all in `src/app.rs`, none on macro-generated code, no
crate-wide `allow`):

1. `clippy::struct_excessive_bools` on `CheckScreen` (view flags).
2. `clippy::unnecessary_debug_formatting` on one helper `shown(&Path)`: the
   escaped `{:?}` form is deliberately used to display non-Unicode names. A
   production app would use `Path::display` and not need it.
3. `clippy::cast_precision_loss` on the progress percentage.

Lints the GPUI/gpui-component style tripped, fixed in code rather than
silenced: `too_many_lines` (a single `render` over 100 lines: split into row
helper functions), `unused_self` (button helpers made associated functions),
`semicolon_if_nothing_returned` (closure bodies written with `;`),
`redundant_closure*` (method paths), `needless_borrow`. GPUI macros
(`#[gpui::test]`) triggered nothing. Takeaway: pedantic is workable;
`too_many_lines` pushes large `render` functions into small helpers.

## Other findings worth carrying to part B

- Theme/Root: `gpui_component::init(cx)` and wrap the root view in
  `gpui_component::Root::new(view, window, cx)`; `ActiveTheme::theme()` gives
  colours; `h_flex`/`v_flex` come from gpui-component. Built-in controls used:
  `Button`, `Checkbox`, `Input` + `InputState`, `Progress`.
- `Window` title/size: `WindowOptions { window_bounds, window_min_size,
  titlebar }` (1000x700, min 800x560 as in Tauri). Minimum size is applied by the
  platform; not verified live (checklist step 3).
- The `#![windows_subsystem = "windows"]` exe has no console: the check
  screen and `fm-spike-results.txt` are the only output (the CI step prints the
  file).

## Files

`spike/gpui/` (Cargo.toml, Cargo.lock, build.rs, src/, ci/licenses.py,
CHECKLIST.md, REPORT.md) and `.github/workflows/spike-gpui.yml`. Nothing else
changed.
