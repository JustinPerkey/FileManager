# M1a — Spike, part A: GPUI exe, platform integration, headless tests

Status: approved (2026-11-20; revised 2026-11-20: the human's answers on spike
location, size/build time, and minimum Windows version are folded in)
Project: gpui-migration   Depends on: none

## Goal

Prove or disprove, on Windows, that an app built with upstream Zed GPUI and
gpui-kit (`gpui-component`) can ship as FileManager ships today and reach the
platform features its tools use, and record every result as evidence for a
human go/no-go decision.

## Context

### Why this spike exists

The human has decided to replace FileManager's Tauri v2 + React frontend with
**upstream Zed GPUI + gpui-kit**, removing Tauri, React, TypeScript, ts-rs and
npm. Before any migration work, a time-boxed, throwaway spike must answer the
unknowns. This task is part A (build and platform). Part B (widgets, theme,
screenshots, and the final report) runs after it on the same branch and
extends the same spike app. This task writes no production code.

### The human's decisions that bind the spike

- **Location:** a new branch `spike/gpui-migration` with a standalone
  `spike/gpui/` workspace and its own workflow, never merged to `main`.
  Committing to and **pushing that branch is approved** (CI must run there).
  Do not push any other branch.
- **Windows 11 only.** Windows 10 is not a target: write no Windows 10
  fallbacks, and where an API needs Windows 11 that is acceptable (record
  it). GitHub has no hosted x64 Windows 11 client image, so CI runs on a
  runner **pinned to `windows-2025`** (Windows Server 2025, which shares the
  Windows 11 24H2 code base), never on the moving `windows-2025` label.
  The human runs the manual checklist on Windows 11.
- **Exe size and build time have no limit.** Measure and report them only.

### Known facts to start from (fetched 2026-11-19; verify, do not trust)

- `gpui-component` 0.10.0 (released 2026-11-16, Apache-2.0) is the crates.io
  name of longbridge's gpui-kit. It pins GPUI as the crates.io crate
  **`gpui-pre`** at an exact version (`=0.3.12` at 0.10.0). Use exactly the
  GPUI version that the chosen `gpui-component` release pins; read it from
  that release's `Cargo.toml`. The crates.io crate `gpui` 0.2.2 is a year old
  and is **not** the one to use.
- If 0.10.0 does not build, try the newest `gpui-component` release and
  record which one you used and why.
- Zed ships on Windows with a DirectX 11 / DirectWrite backend.
- API names below are the ones to look for in the pinned sources
  (`~/.cargo/registry/src/**/gpui-pre-*`, `gpui-component-*`); they are
  **unconfirmed**. Search the source, use what exists, and record the real
  names in the report: `prompt_for_paths` / `PathPromptOptions`,
  `prompt_for_new_path`, `ExternalPaths`, `on_drop`, `on_drag_move`,
  `on_window_should_close`, `open_with_system`, `reveal_path`,
  `#[gpui::test]`, `TestAppContext`, `VisualTestContext`, the `test-support`
  feature, `Window::appearance`.

### How FileManager ships today (the bar to meet)

Read these files in the repository; they are the reference, unchanged by
this task:

- `.cargo/config.toml`: `[target.x86_64-pc-windows-msvc] rustflags =
  ["-C", "target-feature=+crt-static"]` links the C runtime statically. A
  `RUSTFLAGS` env var would replace it; never set one. (`tauri.conf.json`'s
  `staticVCRuntime: false` is Tauri-specific and does not apply here.)
- `Cargo.toml`: `[profile.release] lto = true, codegen-units = 1`;
  `[workspace.lints.clippy] pedantic = warn`, CI runs clippy with
  `-D warnings`.
- `.github/workflows/ci.yml`, windows job, step "Exe must import no C
  runtime, zstd, liblzma, sqlite3, or WebView2Loader DLL": locates
  `dumpbin.exe` with `vswhere`, runs `dumpbin /dependents` on the exe, and
  fails on any line matching
  `vcruntime*.dll`, `msvcp*.dll`, `api-ms-win-crt-*`, `zstd*.dll`,
  `liblzma*.dll`, `sqlite3*.dll`, `WebView2Loader.dll`. Reuse that step's
  script verbatim, pointed at the spike exe.
- The window: title "FileManager", 1000×700, minimum 800×560.
- Domain crates (no UI dependency): `crates/fm-core`, `fm-tarpack` (zstd and
  liblzma compiled from C), `fm-schedule`, `fm-pipeline`, `fm-dbmanager`
  (rusqlite `bundled`, SQLite compiled from C). The spike exe must link all
  five, so the static-CRT check covers their C code together with GPUI.
  Call one cheap public function from each (for example
  `fm_pipeline::subdirectories` on a chosen folder; any constant or
  constructor from the others), so the linker keeps them.
- Platform features the tools use today, from
  `apps/desktop/src-tauri/src/tools/*/mod.rs` and `apps/desktop/src/`:
  - open-file and open-folder dialogs; a save dialog with a suggested file
    name and an extension filter (the Tar Packager's output uses filters
    such as `tar`, `gz`, `zst`, `xz`, see `ArchiveFormatOption` in
    `apps/desktop/src-tauri/src/tools/tarpack/types.rs`);
  - files and folders dropped from Explorer, with an overlay shown while
    dragging over the window (drag-enter / leave / drop with paths);
  - "Edit in editor" opens the manifest in the default app; "Reveal" opens
    Explorer with the output selected;
  - background work with progress: the Tar Packager's build reports
    progress, throttled to one event per 50 ms by
    `apps/desktop/src-tauri/src/tools/tarpack/progress.rs`;
  - a manifest file watcher:
    `apps/desktop/src-tauri/src/tools/tarpack/watch.rs` (notify-debouncer-mini,
    300 ms debounce, watches the parent directory);
  - planned: holding a window close to ask about unsaved edits.

### Rules that bind this task

- Paths are `Path`/`PathBuf`/`OsString`, never strings, until rendered; the
  renderer may convert lossily. A file name that is not valid UTF-16
  (a lone surrogate) must survive every path route this task tests.
- Tests that touch the filesystem run in a temp dir they create
  (`tempfile`), never on real user files.
- Spike code is still `cargo fmt` clean and clippy `pedantic` with
  `-D warnings`: whether that is workable on GPUI code is itself item A10.
  Silence only with scoped `#[expect(clippy::<lint>, reason = "...")]` and
  count each one in the report.
- Never set `RUSTFLAGS`. No secrets.
- Do not modify anything outside `spike/gpui/` and the spike workflow file.
  The production app, crates, CI and docs stay untouched.

### Where the spike lives

- Branch `spike/gpui-migration`, created from `main`. Never merged.
- Directory `spike/gpui/`, a **standalone** Cargo workspace (its own
  `[workspace]` table, its own `Cargo.lock`), so the root workspace and its
  lock file do not change. It depends on the five `fm-*` crates by path
  (`../../crates/fm-core`, …). Cargo still applies the root
  `.cargo/config.toml` (static CRT) because the directory is under the repo
  root; confirm this in the report.
- One package `fm-spike`, binary `fm-spike`; the same `[profile.release]`
  and `[workspace.lints.clippy]` as the root `Cargo.toml`, with
  `[lints] workspace = true`.
- Workflow `.github/workflows/spike-gpui.yml`, triggered on push to
  `spike/gpui-migration` and on `workflow_dispatch`.

### The time box

About three working days of effort for this part. Per item: try GPUI's own
API first, then at most one fallback (named per item). If an item still
fails, stop, mark it FAIL with what was tried and the error, and move on.
Do not patch GPUI or gpui-kit sources.

### The interactive limit

You work on Linux and through Windows CI; you have no interactive Windows
desktop. Items that need a person (a native dialog, an Explorer drag, the
close button, the system editor) are verified by the human with the CI exe.
For those, build an in-app **check screen** and a checklist so the human's
run is quick and its results are recorded:

- One window with a button per manual item. Each action prints its outcome
  in the window (paths received, `{:?}` of each `PathBuf`, booleans) and
  appends it to `fm-spike-results.txt` beside the exe.
- `spike/gpui/CHECKLIST.md`: numbered steps per manual item, each with the
  expected outcome and a blank result line (PASS / FAIL / note), run on
  **Windows 11**; its header asks for the edition, version and build
  (`winver`) and the display scale of the test machine; plus
  instructions to create the lone-surrogate test file (a PowerShell
  one-liner using `[char]0xD800` in the name).

## Items, with pass criteria and what failure means

For each: **Pass** is the criterion; **Evidence** is what goes in the report;
**Fail means** is the consequence for the migration.

**A1 — Portable exe with a static C runtime.** `cargo build --release` of
`fm-spike` on `windows-2025`. Embed an application manifest from `build.rs`
with a resource compiler crate (no runtime DLL), unless GPUI already embeds
one (record which): `supportedOS` Windows 10/11 (the one GUID Windows 11
uses, `{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}`), per-monitor-v2 DPI
awareness if GPUI does not set it at runtime, and the app icon from
`apps/desktop/src-tauri/icons/icon.ico`. Windows has no manifest field that
refuses Windows 10; the Windows 11 minimum is a support policy, not a load
check. Record any API the spike uses that exists only on Windows 11.
- Pass: the dumpbin step (verbatim copy, pointed at
  `spike/gpui/target/release/fm-spike.exe`) passes, and every DLL
  `dumpbin /dependents` lists is a Windows system DLL (for example
  `KERNEL32`, `USER32`, `GDI32`, `d3d11`, `dxgi`, `dwrite`, `ole32`,
  `shell32`, `d3dcompiler_47`). The exe starts from a folder containing only
  itself (the CI job copies it to an empty folder and starts it with a
  `--self-check` flag that opens the window, renders one frame and exits 0).
- Evidence: the full dumpbin output, the list of DLLs, whether
  `d3dcompiler_47.dll` or shader compilation is needed at runtime.
- Fail means: no-go unless a non-system DLL can be removed by configuration;
  the product is a single portable exe.

**A2 — Native file dialogs.** Check-screen buttons: Open file, Open folder,
Open multiple files, Save with suggested name `gateway.tar.gz` and a filter
"Tar archive (gz)" `*.gz`.
- Pass: each returns `PathBuf`s (or a clean cancel); the lone-surrogate
  file chosen in Open file comes back with an identical `OsString`; the
  save dialog shows the suggested name and filter. Fallback if GPUI lacks
  filters or names: the `rfd` crate beside GPUI; then also confirm it does
  not block GPUI's event loop or add a DLL (re-run A1).
- Evidence: API used, human's checklist results.
- Fail means: no-go only if neither GPUI nor a fallback works; every tool
  uses these dialogs.

**A3 — Drop from Explorer.** A drop zone covering the window.
- Pass: dragging files and a folder over the window is observable (an
  overlay appears; it disappears on leave and on drop); dropping delivers
  every path, folder paths included, lone-surrogate name intact. A
  headless test also drives the drop path with synthetic `ExternalPaths`
  (or whatever GPUI uses) to prove routing.
- Evidence: API used, whether drag-enter/leave are distinct events or must
  be inferred, human's results.
- Fail means: no-go; the Tar Packager's assignment and every tool's file
  choice depend on drop.

**A4 — Window-close interception.** A "dirty" checkbox on the check screen.
- Pass: with dirty on, closing via the title-bar X and via Alt+F4 does not
  close; the app shows a confirmation ("Close without saving?" with Keep
  editing / Close); Keep editing leaves the window open; Close closes it and
  the process exits. With dirty off, the window closes at once. A headless
  test proves the hold and the programmatic close, if the test platform
  supports it (say if not).
- Evidence: API used, human's results.
- Fail means: no-go for the Database Manager's unsaved-edits prompt unless
  a Win32 subclass fallback works; record the cost.

**A5 — System editor and Explorer.** Buttons: open a `.toml` in the default
app; reveal a file in Explorer.
- Pass: the default handler opens the file; Explorer opens with it selected.
  Fallback: `ShellExecuteW` / `SHOpenFolderAndSelectItems` via the `windows`
  crate.
- Evidence: API or fallback used, human's results.
- Fail means: soft; the fallback is a few lines.

**A6 — Headless tests on Windows CI.** With the `test-support` feature:
- a model test (`#[gpui::test]`, `TestAppContext`): an entity whose method
  starts a background job and whose state changes when it finishes;
- a view test (visual test context): type "abc" into a gpui-kit text input
  with simulated keystrokes and assert the input's value; click a button
  (by debug selector or bounds) and assert the handler ran;
- a drop-routing test (from A3) and, if possible, a close-hold test (A4).
- Pass: `cargo test` in `spike/gpui` passes on `windows-2025` three runs
  in a row (use a job matrix or `workflow_dispatch` reruns) with no GPU.
- Evidence: test names, run links, duration.
- Fail means: no-go; behaviour parity for ~395 ported frontend tests relies
  on headless tests.

**A7 — Background work, progress and watching.** Check-screen buttons:
"Simulated build" runs a 5 s job on a background thread sending progress
every 1 ms through a 50 ms throttle into the UI (a progress bar and a
counter); while it runs, typing into a text input stays responsive.
"Watch file" starts a watcher (copy `watch.rs`'s approach:
notify-debouncer-mini, 300 ms, parent directory) on a temp file; editing it
shows "changed on disk" in the window.
- Pass: progress arrives in order, the final event always shows, input is
  not blocked; the watcher event reaches the view from the watcher's thread.
  Covered by headless tests too (the watcher test in a temp dir).
- Evidence: the mechanism (channel + foreground task, or GPUI's executor
  API) and any `Send`/`'static` constraint found.
- Fail means: no-go; builds and pipeline steps are long.

**A8 — Size, build time, toolchain, licenses.**
- Report only: the human set **no limit** on size or build time. Measure on
  `windows-2025`: release exe size (bytes); clean release build
  wall time (no cache); cached release build time; debug build time after
  touching one view file. Record `rustc -V`, the minimum Rust version
  GPUI/gpui-kit need (their `rust-version`/edition) against the repo's
  `rust-version = "1.90"`, and the number of crates in the lock file.
- Licenses: list every dependency license (`cargo metadata` script or
  `cargo-license` installed in CI); flag any GPL, AGPL or LGPL.
- Pass: every number measured and reported; no copyleft dependency. There
  are no size or time thresholds.
- Fail means: a copyleft dependency is no-go unless replaceable. Size and
  time cannot fail this item.

**A9 — Linux build.** A non-blocking linux job (`continue-on-error: true`):
try `cargo build` and `cargo test` of `spike/gpui`.
- Pass: reported either way, with the apt packages it needed.
- Fail means: soft; the migrated CI would scope Linux to the domain crates.

**A10 — clippy pedantic.** `cargo clippy --all-targets -- -D warnings` in
`spike/gpui` on both jobs.
- Pass: clean, with a counted, short list of scoped `#[expect]`s (each with
  its lint and reason), none on macro-generated code we cannot avoid
  without a module-wide allow.
- Evidence: the list; any lint GPUI macros trigger.
- Fail means: if clean code needs crate-wide allows, the human must accept
  a change to the clippy invariant.

## Files

Create, on branch `spike/gpui-migration` only:

- `spike/gpui/Cargo.toml`, `spike/gpui/Cargo.lock`, `spike/gpui/src/**`
  (`main.rs` with `#![windows_subsystem = "windows"]`, the check screen,
  the tests).
- `spike/gpui/CHECKLIST.md` (manual items A2–A5, A7's watcher, with result
  lines).
- `spike/gpui/REPORT.md`: start it with a section per item A1–A10 (result,
  evidence, notes, effort spent). Part B adds its items and the summary.
- `.github/workflows/spike-gpui.yml`: windows job (fmt is linux-only; clippy,
  test ×3, release build, timings, size, dumpbin, licenses, self-check run,
  upload `fm-spike.exe` and `fm-spike-results.txt` if any as artifacts) and
  the non-blocking linux job (fmt, clippy, build, test).

Do not edit anything else: not `crates/`, `apps/`, the root `Cargo.toml`,
`Cargo.lock`, `.cargo/config.toml`, `ci.yml`, or any doc.

## Acceptance criteria

- The spike workflow runs on the branch; its windows job is green except
  where an item is reported FAIL (then the failing step is
  `continue-on-error` and named in the report).
- `fm-spike.exe` is an uploaded artifact; it links all five `fm-*` crates.
- `REPORT.md` has a filled section for each of A1–A10: PASS / FAIL /
  PARTIAL / NEEDS-HUMAN (manual items until the human runs the checklist),
  with evidence (CI run links, dumpbin output, measurements, API names).
- `CHECKLIST.md` lets the human verify every manual item in under 30
  minutes with the CI exe.
- No file outside `spike/gpui/` and `.github/workflows/spike-gpui.yml`
  changed.

## Tests proving completion

Run from `spike/gpui/` (on Windows CI; on Linux if A9 passes):

```sh
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo build --release --locked
```

Named tests (rename to fit, keep the meaning): `job_finishes_on_main_thread`,
`progress_is_throttled_and_ordered`, `input_accepts_keystrokes`,
`button_click_runs_handler`, `external_drop_delivers_paths`,
`close_is_held_while_dirty` (if supported), `watcher_reports_edit` (temp dir).

## Out of scope

- gpui-kit widgets beyond a text input and a button, theming, the Pipeline
  screen, PNG capture, the summary recommendation: part B
  (`M1-spike-b-ui.md`).
- Any production code, any change to the Tauri app, any migration
  milestone.
- Accessibility (screen readers, high contrast): not a gate for this
  migration.

## Risks

- `windows-2025` has no GPU: GPUI's DirectX renderer must start on WARP
  for `--self-check` and real-window work. If it does not, record it; it
  decides whether part B's screenshots can run in CI.
- `windows-2025` is a server SKU: Explorer drag, theme switching and
  bundled fonts may differ from Windows 11 client; only the manual
  checklist proves client behaviour.
- GitHub's Windows runner may not give a GUI app a usable desktop session;
  record what `--self-check` does there.
- `gpui-pre` may need a newer Rust than the repo's `rust-version`; record it
  rather than changing the root.
- LTO with `codegen-units = 1` may make the release build very slow; report
  it and, separately, the time without LTO.
