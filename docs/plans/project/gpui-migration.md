# Project plan: migrating the UI from Tauri + React to GPUI + gpui-kit

Status: **approved (2026-11-21).** The spike is done, go is decided, and the human approved this plan and its post-spike task plans after answering Q14–Q16 (D24–D26).

History:

- 2026-11-19: written by the planner, after the human's decisions on
  [`gpui-ce-assessment.md`](gpui-ce-assessment.md) (§1.1).
- 2026-11-20: the human answered Q4, Q5, Q6 and Q12 (D6–D9).
- 2026-11-21: the spike (M1a, M1b) finished on branch `spike/gpui-migration`
  (`spike/gpui/REPORT.md`: go-with-conditions). The human decided **go**,
  settled the report's conditions and answered Q1–Q3, Q7–Q11 and Q13
  (D10–D23, §14.1). The planner folded the spike's facts and those decisions
  into §2–§13, re-planned the Database Manager's UI-facing milestones (§11),
  and wrote the task plans for every backend milestone after the spike (§9).
- 2026-11-21, later: the human answered Q14–Q16 (D24–D26: our own Windows
  file dialogs with filters, the full Rust `regex` syntax in dbmanager
  patterns, the drop log match accepted) and approved the plan and its task
  plans. M3c was added for the dialogs.
- 2026-11-21, later still: the ui-designer's plan landed and was approved;
  its requests R1–R7 were applied to M3a, M3b, M4, M5–M7b, M8 and M12a–M12d.
  Live tests found the agent-frontmatter hook never fires in the cloud
  runtime; the human chose a settings-file hook filtered by the calling
  agent (D27, M11b).

The visual half (theme, shared widgets, the shell's look, per-tool views,
screenshot scenarios) belongs to the `ui-designer` (§13). It writes
`docs/plans/project/gpui-migration-ui.md` and the `U<n>` task plans under
`docs/plans/tasks/gpui-migration/`. It exists and is **approved**
([`gpui-migration-ui.md`](gpui-migration-ui.md): U1–U12 here, the Database
Manager's U2–U12 and U14 rewritten in place under `tasks/dbmanager/`); its
requests R1–R7 to this plan are applied (§13.1). The backend milestones below
are ordered so that the U tasks can interleave (§9.2).

Task plans: [`../tasks/gpui-migration/`](../tasks/gpui-migration/).

## 1. Request

Rewrite FileManager's UI in **upstream Zed GPUI** with the longbridge
**gpui-kit** component library (crate `gpui-component`). Remove Tauri, React,
the TypeScript frontend, ts-rs and npm/Node from the application entirely.

### 1.1 The human's decisions

| # | Date | Decision |
|---|---|---|
| D1 | 11-19 | **Full rewrite.** Every tool (Pipeline, Tar Packager, Schedule Creator, and the Database Manager's coming UI) moves to GPUI. Tauri, React, the TS frontend and npm/Node are removed. |
| D2 | 11-19 | **Upstream Zed GPUI + gpui-kit**, not GPUI CE. |
| D3 | 11-19 | **The UI looks similar to today:** same layout, tokens, components and states as `DESIGN.md` and `apps/desktop/src/styles/tokens.css` describe. Light and dark, following the system. |
| D4 | 11-19 | **The manual's screenshots stay auto-generated** from the real UI with a fixture backend, without Node. |
| D5 | 11-19 | **Accessibility is no longer a hard requirement** beyond keyboard operation and labels (made exact by D15). |
| D6 | 11-20 | Spike on branch `spike/gpui-migration`, standalone `spike/gpui/` workspace, never merged. |
| D7 | 11-20 | **Similarity:** same layout and token colours; sampled colours ±1 per channel, geometry within about 2 px of today's images; text rendering may differ. |
| D8 | 11-20 | **Exe size and build time:** no limit; measured and reported only. |
| D9 | 11-20 | **Windows 11 only.** |
| D10 | 11-21 | **Go.** The human's Windows 11 run of `spike/gpui/CHECKLIST.md` is still outstanding; it does not block planning. Any FAIL found there reopens the milestone that owns the failing behaviour (§12, risk 1). |
| D11 | 11-21 | **Non-Unicode paths: accept the loss** (report condition 1). Explorer drop is used as shipped (no own `IDropTarget`), and no `rfd`. A dropped name that GPUI cannot deliver is **reported to the user, never skipped silently**. The path invariant is reworded for drop (§10.3). *(The dialog half of D11 is superseded by D24.)* |
| D12 | 11-21 | **Overlay shadow** (condition 2): gpui-component's fixed dialog and popover shadow is accepted. The `--shadow-overlay` token is not applied. |
| D13 | 11-21 | **Tar Packager table in narrow windows** (condition 3): it **scrolls sideways**. It does not stack rows into lines (today's below-64rem reflow is dropped). |
| D14 | 11-21 | **Screenshot capture never ships in the release exe** (condition 5). `render_to_image` needs GPUI's `test-support`; it is only in a separate cargo feature and binary built for CI and docs. |
| D15 | 11-21 | **Q1: the accessibility invariant replacement is approved:** every interactive element is keyboard reachable and labelled; no screen-reader or axe checks (§10.3). |
| D16 | 11-21 | **Q2: `impeccable` stays for `shape` and `critique` on screenshots only.** `audit` and `polish` are dropped from the UI roles. |
| D17 | 11-21 | **Q3: `.claude/hooks/task-plan-only.mjs` is ported to a small Rust binary in the workspace**, called from the agent settings, with identical behaviour. |
| D18 | 11-21 | **Q7: Cascadia Mono (OFL) is embedded in the exe.** |
| D19 | 11-21 | **Q8: CI uploads the screenshots and never fails on image differences.** |
| D20 | 11-21 | **Q9: the dbmanager screen-reader and 200% text work is dropped from U12;** keyboard operation and the shortcuts help stay. |
| D21 | 11-21 | **Q10: M5's regex shorthand-class ban is lifted once only Rust validates** (widened by D25 to every old restriction). |
| D22 | 11-21 | **Q11: no vendoring.** crates.io with the committed `Cargo.lock` and exact pins. |
| D23 | 11-21 | **Q13: the Linux CI job builds and runs the headless tests,** GPUI included. There is no Linux window backend. |
| D24 | 11-21 | **Q14: "This should open the windows file picker."** Open and save show the native Windows file picker **with file-type filters**, as today (for example the Tar Packager's open dialog filters to manifests), plus a default extension and an initial folder. Since GPUI exposes no filters, FileManager has its own thin wrapper over the Windows common item dialog (`IFileOpenDialog` / `IFileSaveDialog`, `windows` crate), returning paths as `OsString`, behind `Platform` with a fake for tests (M3c). Not `rfd`. Side effect: dialogs no longer lose non-Unicode names; drop stays as D11. |
| D25 | 11-21 | **Q15: lift all the old Rust/JavaScript pattern restrictions.** The full Rust `regex` syntax is allowed in dbmanager config patterns (dbmanager M15). |
| D26 | 11-21 | **Q16: accept** the drop path-loss detection by GPUI's log message, guarded by the pin test. |
| — | 11-21 | **The plan and its post-spike task plans are approved.** |
| — | 11-21 | **The UI plan (`gpui-migration-ui.md`) and every U task are approved as written**, including V1–V8 and 25 objects per page. |
| D27 | 11-21 | **The task-plan guard moves to a `.claude/settings.json` `PreToolUse` hook** that applies only when the hook input's `agent_type` is `implementer` or `ui-implementer` (the human's condition: the input must identify the calling subagent; Claude Code's hook reference says it does, §7). The agent-frontmatter hook is removed once the live check passes (M11b). |

### 1.2 What does not change

- The domain crates (`fm-core`, `fm-tarpack`, `fm-schedule`, `fm-pipeline`,
  `fm-dbmanager`) and their behaviour.
- The product: the tools, their flows, copy and confirmations (`PRODUCT.md`);
  a portable Windows `.exe` with no installer; statically linked C runtime;
  the `dumpbin` dependency check.
- The invariants on destructive operations, temp-dir tests, `cargo fmt` +
  clippy `pedantic`, no secrets, agent-role sync and review. The path
  invariant keeps its rule everywhere except the two GPUI entry points (D11).
- The Database Manager's domain milestones M7–M9 (§11).

## 2. Facts from the spike

Source: `spike/gpui/REPORT.md` on branch `spike/gpui-migration` (CI runs
44798160684, 44841007688, 44844953255). Restated so this plan stands alone.

**Versions.** `gpui-component` **=0.10.0** (Apache-2.0); GPUI is the
crates.io snapshot **`gpui-pre` =0.3.12** (lib name `gpui`), with its
platform crate **`gpui-pre-platform` =0.3.12** (lib `gpui_platform`,
`default-features = false`) and **`gpui-kit-assets` =0.10.0** (the icon
bundle gpui-component's checkbox, menu and dialog load). All GPUI crates are
edition 2024; the repo's `rust-version = "1.90"` and edition 2021 are enough.

**What passed.** Portable exe with `+crt-static`, dumpbin clean, GPUI embeds
its own application manifest (per-monitor-v2 DPI, Common Controls 6), the
icon is embedded with `winresource` as resource id 1 (A1). Close
interception (`on_window_should_close`), open in system editor and reveal
(`open_with_system`, `reveal_path`, lossless `&Path`), background work with
progress on a `std::thread` plus `async_channel`, the
`notify-debouncer-mini` watcher (A4, A5, A7). `#[gpui::test]` on
`windows-2025` with no GPU, three runs, no flake; Linux builds and tests
headless with no apt packages (A6, A9). clippy pedantic with 5 scoped
`#[expect]`s (A10). Text input, tri-state checkbox (`.indeterminate`),
10,000-row virtual table, modal dialog with focus and busy control, menu and
popover (B1–B5). The themed Pipeline screen matches the tokens exactly (32 of
32 colour samples) and the reference geometry (B6). `render_to_image`
produces byte-identical PNGs across runs on CI (B7).

**Gaps (REPORT §4), and what this plan does with them.**

| Gap | Fate |
|---|---|
| 1 Non-Unicode names lost by GPUI's dialogs (whole selection errors) and by drop (name skipped, `log::error!`) | Dialogs: fixed by our own dialog wrapper (D24, M3c). Drop: accepted and reported (D11, D26, §3.7). |
| 2 Save dialog has no file-type filter (nor does the open dialog: `PathPromptOptions` has no filter) | Fixed by our own wrapper over the common item dialog with `SetFileTypes`, default extension and initial folder (D24, M3c). |
| 3, 4 Dialog, menu and popover shadow fixed | Accepted (D12). |
| 5 Table rows have one fixed height | Accepted: no reflow, sideways scroll (D13). |
| 6 No middle truncation | A shared widget (ui-designer). |
| 7 Mouse press does not focus a Button; Enter/Space need key-down + key-up | Tests send both events; focus return after a dialog is guaranteed only for keyboard-opened dialogs (accepted difference, ui-designer's parity list). |
| 8 `Theme::font_size` is the window rem; keep 16 px | Binding on the theme U task. |
| 9 Theme slots with no home | Set on elements (ui-designer's theme mapping). |
| 10 Subpixel text antialiasing | Not a failure (D7). |
| 11 `test-support` grows the exe | Feature `shoot` and a separate binary (D14, §5). |
| 12 Linux headless only | Accepted (D23). |

**API names** used by the task plans (REPORT §5): `gpui_platform::application().with_assets(..).run(..)`,
`cx.open_window(WindowOptions { window_bounds, window_min_size, titlebar, .. }, ..)`,
`gpui_component::init(cx)` and `Root::new(view, window, cx)`, `Theme::change`
then `Theme::update`, `window.observe_window_appearance`, `Window::render_to_image()`,
`#[gpui::test]`, `TestAppContext`, `VisualTestContext::{simulate_input, simulate_keystrokes, simulate_click, simulate_event, simulate_close, run_until_parked}`,
`cx.prompt_for_paths(PathPromptOptions { files, directories, multiple, prompt })`,
`cx.prompt_for_new_path(&Path, Option<&str>)`, `ExternalPaths` with `on_drop`
and `FileDropEvent::{Entered, Pending, Submit, Exited}`, `on_window_should_close`,
`window.remove_window()`, `cx.open_with_system`, `cx.reveal_path`,
`gpui::actions!`, `KeyBinding::new(..)`, `.key_context(..)`, `.on_action(..)`,
`cx.spawn(async move |this, cx| ..)`, `cx.background_spawn(..)`.

**Measured (D8, report only).** Release exe 25.7 MiB with LTO and the
`test-support` feature (24.4 MB without it); clean release build about
640 s on `windows-2025`; release rebuild after one file 268 s; debug rebuild
12 s; 825 crates in the lock (all targets); licences MIT/Apache-2.0 family,
no GPL-only crate, the three MPL-2.0 crates are not in the Windows exe.

**Still open: the human's `CHECKLIST.md` run on Windows 11** (live dialogs,
drop, close, theme switch, `--shoot` at a display scale other than 100%, the
feel of 10,000 rows). D10 governs it.

## 3. Architecture after the migration *(decided)*

### 3.1 Layout

```
Cargo.toml                      workspace: members crates/*, apps/filemanager,
                                devtools/* (today devtools/task-plan-guard;
                                apps/desktop/src-tauri until M9);
                                [workspace.dependencies] pins gpui, gpui_platform,
                                gpui-component, gpui-kit-assets (exact), async-channel,
                                notify-debouncer-mini, image, tempfile; ts-rs removed in M9
.cargo/config.toml              static CRT for x86_64-pc-windows-msvc (unchanged);
                                TS_RS_EXPORT_DIR removed in M9
crates/
  fm-core/                      + watch.rs: FileWatcher (M2b, from tarpack/watch.rs)
  fm-tarpack/                   + src/session/: Core, DropJob, BuildJob, ProgressCoalescer,
                                  session types (M2b, from src-tauri tools/tarpack)
  fm-schedule/                  + src/session/ (M2a, from src-tauri tools/schedule)
  fm-pipeline/                  + src/session/ (M2a, from src-tauri tools/pipeline)
  fm-dbmanager/                 + src/tool/: Core (config path, recent list, restore,
                                  snapshot, errors) (M12a)
devtools/task-plan-guard/       the PreToolUse guard for implementer subagents (M11)
apps/filemanager/               package filemanager-gpui until M9, then filemanager
  Cargo.toml, build.rs          build.rs embeds the icon (winresource); GPUI embeds the
                                manifest
  assets/icon.ico, assets/icons/*.svg, assets/fonts/ (Cascadia Mono + OFL.txt)
  src/main.rs                   #![windows_subsystem = "windows"]; calls run()
  src/lib.rs                    app init, assets, fonts, logger, window (1000×700, min 800×560)
  src/assets.rs                 AssetSource: our icons, then gpui-kit-assets
  src/platform/                 trait Platform (dialogs, open, reveal, clipboard): real impl
                                (win_dialog.rs: our common item dialog wrapper; GPUI for the rest)
                                and a scripted FakePlatform; path_loss.rs (§3.7)
  src/jobs.rs                   Worker<T> (serial, off the main thread) and spawn_thread
                                (long jobs with a progress channel)
  src/app/                      Shell (window root: close hook, drop routing, dev mode,
                                current tool, notices), guard.rs, dev_mode.rs, drop.rs
  src/app/view/                 shell views: sidebar, close prompt, drop overlay (ui)
  src/theme/                    tokens.rs (normative token values), gpui-kit theme mapping,
                                live light/dark (ui)
  src/widgets/                  shared widgets over gpui-kit (ui)
  src/tools/mod.rs              the tool registry (one list)
  src/tools/<tool>/backend.rs   trait <Tool>Backend + the real impl over the crate's Core
  src/tools/<tool>/model.rs     the tool's view-model entity (§3.3)
  src/tools/<tool>/text.rs      the tool's user-facing copy, ported verbatim
  src/tools/<tool>/fixtures.rs  fixture backend (cfg(test) or feature "shoot")
  src/tools/<tool>/view/        the tool's views (ui)
  src/shoot/                    screenshot harness (feature "shoot"); scenarios/ (ui)
  src/bin/filemanager-shoot.rs  harness entry point (required-features = ["shoot"])
docs/gpui-test-ledger.md        every vitest case: ported where, or dropped and why (M3a on)
docs/manual/screenshots/        README.md only after M8
```

The new app is built **beside** `apps/desktop` (package and binary
`filemanager-gpui`) until the cutover (M9), which deletes `apps/desktop` and
renames the package and binary to `filemanager`. Until then the Tauri exe is
the shipping one, and CI builds and dumpbin-checks both.

### 3.2 Where the session logic goes

Each tool's session logic (`apps/desktop/src-tauri/src/tools/<tool>/core.rs`,
`types.rs`, tarpack's `progress.rs`) moves into a `session` module of the
tool's own domain crate, with its tests (M2a, M2b). The Tauri commands become
thin wrappers over the moved code, so the move is proved by the existing tests
and by `lib/generated/` staying byte-identical before any GPUI code depends
on it. The Database Manager's tool-level core is written directly in
`fm_dbmanager::tool` (M12a), since its Tauri contract was never built.

Reasons: one crate per tool keeps "tools never import each other"; the logic
stays testable without any UI; a shared session crate would import every
tool.

### 3.3 Models are view-models *(planner decision, 2026-11-21)*

Today a large part of each tool's behaviour lives in React state: which
confirmation is pending, "Run all" sequencing, the reasons a button is
disabled, status labels, which error banner shows, shortcut gating. In the
GPUI app:

- A tool's **model** (`tools/<tool>/model.rs`, a GPUI entity, owned by the
  backend `implementer`) holds the session snapshot **and** every piece of
  view state that is behaviour: pending confirmation, busy flags, the
  current error, results, derived predicates (`can_build`, `run_reason`, …)
  and status enums. Its public API is the UI contract; each tool milestone
  pins it.
- **Copy** (error messages, step titles, status labels, confirmation text,
  report text) is ported verbatim from today's `errorMessages.ts`,
  `steps.ts`, `reportText.ts` and the views into `tools/<tool>/text.rs`,
  owned by the implementer and unit-tested.
- A tool's **views** (`tools/<tool>/view/`, `ui-implementer`) render the
  model, map status enums to icons and tones, own focus and layout, bind
  keys to model methods, and own purely visual state (hover, scroll
  position, which popover is open).
- Views never touch the filesystem or a domain crate; they call the model.
- Model behaviour is tested with `#[gpui::test]` and a fixture or temp-dir
  backend; view tests prove wiring only.

Reason: the behaviour React held is what the ~395 vitest tests check; in the
model it is testable without rendering, the U tasks become rendering work,
and the backend/UI split of `CLAUDE.md` survives without a TS mirror.

**Lifetime.** Models are created once, at startup, by the tool registry, and
live for the app's lifetime. When a tool becomes current, its model's
`activated()` reproduces what a fresh React mount did: it clears the state
the React top-level view held in `useState` (each tool milestone lists the
fields) and refreshes the snapshot. A job that was running when the user
switched away keeps running and its result is kept until dismissed. (Today's
"orphan" polling exists only because React views unmount; it is dropped.)

### 3.4 Threading *(planner decision)*

**No filesystem or other blocking I/O on the main thread.** A slow share
must not freeze the window, as today's async commands guarantee.

- `jobs::Worker<T>` owns a value `T: Send + 'static` (a tool's backend) and
  runs closures `FnOnce(&mut T) -> R` on it **one at a time, in submission
  order, off the main thread**, returning a GPUI `Task<R>` that resolves on
  the main thread. It runs on GPUI's background executor, so it is
  deterministic under `TestAppContext`. It replaces today's
  `Mutex<Core>` in Tauri state: no lock, no poisoning, ordering guaranteed.
- `jobs::spawn_thread` runs a long `Send + 'static` job (archive build,
  folder walk, pipeline step) on its own `std::thread`, with an
  `async_channel` progress sender, and returns the progress receiver and a
  `Task` for the result (the spike's A7 pattern). Long jobs never occupy the
  Worker, so the session stays responsive while they run.
- The existing begin / `Job::run` / finish split (tarpack `DropJob`,
  `BuildJob`; pipeline `Job`) stays: begin and finish run on the Worker,
  `run` on `spawn_thread`.
- Progress: tarpack's `ProgressCoalescer` (50 ms) is unchanged; its sink
  sends into the channel; a foreground task drains it into the model.
- Watching: `fm_core::watch::FileWatcher` calls an `Fn() + Send` callback on
  its thread; the callback sends into a channel that a foreground task turns
  into a model call.

### 3.5 The typed boundary disappears

Removed in M9: ts-rs (workspace dependency and every derive),
`generated_types.rs`, `lib/generated/`, `lib/*.ts`, the currency test,
`TS_RS_EXPORT_DIR`, the `.gitattributes` line, the Tauri command layer,
`tauri.conf.json`, the CSP, the capability file and its test, and the
`tools::register` single-`invoke_handler` rule and test. `serde` derives stay
only where persistence needs them.

The new **core/UI boundary** is each tool's `backend.rs`, `model.rs` and
`text.rs`, plus `src/platform/`, `src/jobs.rs` and the non-view parts of
`src/app/`. Domain types are used directly (counts as integers, paths as
`PathBuf`); display strings exist only for display. The dbmanager's
`DbValue` keeps its string forms (they are what text inputs produce).

Ownership:

| Owner | Paths |
|---|---|
| `implementer` | `crates/**`, `devtools/**`, `apps/filemanager/{Cargo.toml,build.rs,assets/}`, `src/{main.rs,lib.rs,assets.rs,jobs.rs}`, `src/platform/`, `src/app/*.rs` (not `src/app/view/`), `src/tools/mod.rs`, `src/tools/<tool>/{backend,model,text,fixtures}.rs`, `src/shoot/` except `scenarios/`, `src/bin/`, CI, docs |
| `ui-implementer` | `src/theme/`, `src/widgets/`, `src/app/view/`, `src/tools/<tool>/view/`, `src/shoot/scenarios/` |

### 3.6 Tests

- Domain and session tests stay plain `cargo test` in the crates (temp dirs).
- Model tests use `#[gpui::test]` / `TestAppContext` with the fixture backend,
  or the real backend inside a temp dir, and `FakePlatform`.
- View tests use `VisualTestContext` (keystrokes, clicks on debug
  selectors) to prove wiring, not pixels. Enter/Space activation needs a
  key-down **and** a key-up event (gap 7).
- Today's ~395 vitest cases are ported **by behaviour**. `docs/gpui-test-ledger.md`
  (created in M3a) lists every case with its new home (a named Rust test in
  a model milestone or a U task) or "dropped: <reason>" (axe, ARIA
  attribute, DOM structure, CSS class, IPC wrapper, orphan polling). M9
  refuses to start while a row is open.

### 3.7 File dialogs and paths GPUI cannot deliver (D11, D24, D26)

- **Dialogs (D24, M3c).** `Platform::pick(OpenSpec)` and
  `Platform::pick_save(SaveSpec)` carry file-type filters, a default
  extension, a start folder, a suggested name and a title. On Windows they
  are `platform/win_dialog.rs`, our thin wrapper over `IFileOpenDialog` /
  `IFileSaveDialog` (`windows` crate): `SetFileTypes`, `SetFileTypeIndex`,
  `SetDefaultExtension`, `SetFolder`, `SetFileName`, run on a dedicated STA
  thread, modal to the main window, results read with
  `GetDisplayName(SIGDN_FILESYSPATH)` into `OsString::from_wide`. Paths never
  pass through `String`, so dialogs lose no name. Each tool passes today's
  filters (Tar Packager "Manifest" `*.toml`, output "<format> archive"
  `*.<last suffix>`; Schedule "Text"/"XML" with "All files"; Database
  Manager "SQLite database" with "All files"). A failed dialog is
  `PickError::Failed` with the copy "The file dialog could not be shown. See
  Details." Tests use `FakePlatform`, which records every spec. Not `rfd`.
- **Drop (D11, D26).** GPUI skips a name that is not valid UTF-16 and only
  logs `log::error!("dragged file name is not UTF-16")`. FileManager installs
  its own `log` logger at startup; `platform::path_loss` recognises that
  record and sends a signal on a channel; the Shell shows a notice ("Some
  dropped items could not be read: their names are not valid Unicode.
  Rename them and drop them again."), coalesced per drag. Detection depends
  on GPUI's exact message text, so a Windows-only test pins the text against
  the `gpui-pre-windows` source in the lock file; a GPUI upgrade that changes
  it fails that test. Accepted (D26).
- Everything else stays exact: dialog results, `read_dir` results, stored
  state, typed and pasted paths, open and reveal (`HSTRING::from(&OsStr)`).

## 4. Design-system port *(details are the ui-designer's)*

- `tokens.css` is replaced by `apps/filemanager/src/theme/tokens.rs`, one
  struct per theme, every token by the same name. It becomes `DESIGN.md`'s
  normative source. The spike's `spike/gpui/src/tokens.rs` (with its
  `tokens_match_tokens_css` test) and `theme.rs` (84 gpui-component fields)
  are the starting point.
- `--shadow-overlay` is not applied (D12). The scrim takes `--scrim`
  (`theme.overlay`).
- `Theme::font_size` stays 16 px (the window rem); body text size is set on
  elements (gap 8).
- The theme follows the system appearance live, with no toggle.
- Fonts: Segoe UI (system, always present on Windows 11) and **Cascadia Mono
  embedded** (D18). M3a embeds the font files and registers them with GPUI's
  text system; the theme U task names the family.
- The token contrast test of `tokens.test.ts` is kept as a cheap guard (not
  an accessibility gate).

## 5. Docs and screenshot pipeline

- Feature **`shoot`** on `filemanager-gpui` turns on `gpui/test-support`,
  `gpui_platform/test-support` and `image`; binary **`filemanager-shoot`**
  has `required-features = ["shoot"]`. The release build
  (`cargo build --release -p filemanager-gpui`) has neither (D14); CI checks
  that with `cargo tree`.
- The harness runs in the **dev profile**: GPUI records debug selectors only
  in debug builds, and the dev profile reuses the test build's artifacts.
- It opens the real `Shell` with fixture backends and `FakePlatform`, plays a
  scenario (select tool, click by debug selector, keys, inject a drop or a
  watcher event, script dialog answers, wait until idle), renders with
  `Window::render_to_image`, and writes `<out>/<name>.png`. It takes an
  optional name filter and prefix.
- Window sizes as today: 1366×800 for the Tar Packager, 1000×700 for the
  others; scale factor 1 (the harness reports a different scale and fails
  that scenario).
- Screenshots are generated on Windows (CI and the developer's machine).
  CI uploads them as an artifact and **never fails on image differences**
  (D19). A scenario that cannot run (missing selector, timeout, capture
  error) does fail the job: that is a broken harness, not an image diff.
- Scenario names stay those of `docs/manual/images/*.png`, so
  `user-manual.md` needs no image renames. M8 adds a test that every image
  the manual references has a registered scenario.

## 6. CI after the migration

- **windows** job on `windows-2025` (pinned in M3a): clippy (`-D warnings`)
  with and without `--features shoot`, `cargo test --workspace --locked`,
  release build, dumpbin check (forbidden list unchanged), the "release exe
  has no `test-support`" check, the screenshot harness and upload, and the
  `FileManager-<version>-x64.exe` artifact with the version from
  `cargo metadata` (M9).
- **linux** job: `cargo fmt --all --check`, clippy, `cargo test --workspace
  --locked`, all headless (D23). The webkit2gtk packages stay until M9 (the
  Tauri shell still builds there), then go.
- Removed in M9: `actions/setup-node`, `npm ci`, the frontend build,
  typecheck, lint, vitest, the webkit2gtk packages.
- The crate check covers **all five `fm-*` crates** and forbids `tauri`,
  `gpui-pre`, `gpui-pre-platform` and `gpui-component` (M3a).

## 7. Agent roles, `impeccable`, the hook

Changed in M10 (roles, docs) and M11 (hook):

- **`impeccable`** stays enabled in `.claude/settings.json` for `shape`
  (design briefs) and `critique` (of generated screenshots). `audit`,
  `polish` and the detectors are removed from the role definitions (D16).
  `PRODUCT.md` "Platform: web" becomes "desktop (native, GPUI)" (ui-designer).
- **ui-designer:** theme mapping and component inventory against gpui-kit;
  acceptance is keyboard reachability and labels (D15).
- **ui-implementer:** owns the paths in §3.5; proves work with view tests and
  regenerated screenshots against the parity checklist; no `npx`.
- **implementer:** owns the paths in §3.5.
- **reviewer:** no axe/jsx-a11y; checks keyboard reachability and labels;
  checks screenshots against the parity list; checks that release builds
  carry no `test-support`.
- **planner:** its accessibility rule follows the new invariant.
- **The hook (D17).** `devtools/task-plan-guard`, a workspace binary with the
  exact behaviour of `task-plan-only.mjs`. The agents' hook command is
  `cargo run --quiet --locked --manifest-path "$CLAUDE_PROJECT_DIR/devtools/task-plan-guard/Cargo.toml" --target-dir "$CLAUDE_PROJECT_DIR/target/task-plan-guard"`.
  Claude Code runs it with the repository's toolchain, which every
  contributor already has; the first call builds it (seconds: its only
  dependency is `serde_json`), later calls start the cached binary. The
  separate target directory means it never waits on the build lock of a
  `cargo build` the agent is running (it can still wait briefly on the shared
  `$CARGO_HOME` package-cache lock while another cargo is downloading or
  updating the registry). A build failure exits non-zero but not
  2, so Claude Code reports it and the call proceeds: the guard fails open,
  as the `.mjs` does on bad input. CI builds and tests it on both jobs.
- **The hook never fired here (finding, 2026-11-21).** Live tests in the
  cloud runtime showed that the agents' frontmatter `hooks:` block never
  runs: implementer subagents read project plans with both the Node and the
  Rust guard, with no hook error. The guard has never been enforced in this
  runtime. Claude Code's hook reference (code.claude.com/docs/en/hooks,
  sub-agents, permissions; read 2026-11-21, Claude Code 2.1.330 here)
  explains it: frontmatter hooks of a **project** subagent run only after
  the workspace-trust dialog is accepted for that folder; `-p`, SDK and
  cloud sessions never show it, and then Claude Code skips them silently
  (a debug-log line only). **Hooks in settings files do run** there (the
  table "What runs before you trust a folder": used under `-p`/SDK). And
  "hooks from settings files … also run inside subagents … the input carries
  the `agent_id` and `agent_type` common input fields that identify the
  subagent"; for a custom subagent `agent_type` is its frontmatter `name`
  (`implementer`, `ui-implementer`); it is absent in the main conversation
  unless the session runs with `--agent`. So a settings-file hook can apply
  the guard to exactly the two roles (D27, M11b).
- Each `.claude/agents/*.md` change is mirrored in `.codex/agents/*.toml`.
- `docs/plans/README.md` is out of the implementers' reach (the guard blocks
  it). The UI task template was updated by the planner on 2026-11-21 (tokens
  in `theme/tokens.rs`, the model API, `shape`/`critique`, keyboard and
  labels). Its hook sentence names `devtools/task-plan-guard/` (planner,
  2026-11-21).

## 8. The spike (M1) — done

Both parts ran on branch `spike/gpui-migration` (never merged; its code may
be ported by name, with `git show origin/spike/gpui-migration:spike/gpui/<file>`):

- [`M1-spike-a-platform.md`](../tasks/gpui-migration/M1-spike-a-platform.md): done.
- [`M1-spike-b-ui.md`](../tasks/gpui-migration/M1-spike-b-ui.md): done.

Result: no Hard item failed; A2, A3 and B4 were PARTIAL (the gaps of §2); the
recommendation was go-with-conditions; the human decided go (D10–D14).

## 9. Milestone index

### 9.1 Backend milestones

| # | Task plan | Goal | Depends on | UI |
|---|---|---|---|---|
| M1a | [`M1-spike-a-platform.md`](../tasks/gpui-migration/M1-spike-a-platform.md) | Spike: exe, platform, tests, CI | none | done |
| M1b | [`M1-spike-b-ui.md`](../tasks/gpui-migration/M1-spike-b-ui.md) | Spike: widgets, theme, screenshots, report | M1a | done |
| G | — | Human go/no-go | M1b | **go** (D10) |
| M2a | [`M2-session-a-pipeline-schedule.md`](../tasks/gpui-migration/M2-session-a-pipeline-schedule.md) | Move Pipeline and Schedule Creator session logic into their crates | G | no |
| M2b | [`M2-session-b-tarpack-watch.md`](../tasks/gpui-migration/M2-session-b-tarpack-watch.md) | Move Tar Packager session logic into `fm-tarpack`; `fm_core::watch::FileWatcher` | M2a | no |
| M3a | [`M3-app-a-skeleton.md`](../tasks/gpui-migration/M3-app-a-skeleton.md) | `apps/filemanager` crate, pins, window, assets and fonts, Shell, tool registry, dev mode, placeholder chrome, CI, test ledger | M2b | contract for the U shell and theme tasks |
| M3b | [`M3-app-b-plumbing.md`](../tasks/gpui-migration/M3-app-b-plumbing.md) | `Platform` (real + fake), path-loss reporting, `jobs.rs`, drop routing, close guard | M3a | contract for the U shell tasks |
| M3c | [`M3-app-c-file-dialogs.md`](../tasks/gpui-migration/M3-app-c-file-dialogs.md) | Our own Windows common item dialog wrapper (filters, default extension, initial folder, `OsString` paths) behind `Platform` (D24) | M3b | no |
| M4 | [`M4-screenshot-harness.md`](../tasks/gpui-migration/M4-screenshot-harness.md) | Feature `shoot`, `filemanager-shoot`, scenario API, CI upload | M3b (may run beside M3c) | contract for every U scenario task |
| M5 | [`M5-pipeline-model.md`](../tasks/gpui-migration/M5-pipeline-model.md) | Pipeline backend, model, copy, fixtures | M4, M3c | **depends on UI plan** (views) |
| M6 | [`M6-schedule-model.md`](../tasks/gpui-migration/M6-schedule-model.md) | Schedule Creator backend, model, copy, fixtures | M5 | **depends on UI plan** |
| M7a | [`M7-tarpack-a-manifest-model.md`](../tasks/gpui-migration/M7-tarpack-a-manifest-model.md) | Tar Packager backend; model for manifest, recent, watcher, assignments, drop | M6 | **depends on UI plan** |
| M7b | [`M7-tarpack-b-build-model.md`](../tasks/gpui-migration/M7-tarpack-b-build-model.md) | Tar Packager model for output, format, build, progress, overwrite, results, dev mode, shortcuts | M7a | **depends on UI plan** |
| M8 | [`M8-manual-cutover.md`](../tasks/gpui-migration/M8-manual-cutover.md) | Manual and screenshots from the harness; Node screenshot scripts removed | M5–M7b and every U task of the three tools | **depends on UI plan** |
| M9 | [`M9-cutover-removal.md`](../tasks/gpui-migration/M9-cutover-removal.md) | Remove Tauri, React, npm, ts-rs; rename to `filemanager`; CI cleanup | M8 | no |
| M10 | [`M10-docs-governance.md`](../tasks/gpui-migration/M10-docs-governance.md) | `CLAUDE.md`, docs, invariants (§10.3), agent roles in both runtimes (plans README text proposed to the planner) | M9, M11 | partly (`DESIGN.md`/`PRODUCT.md` are the ui-designer's) |
| M11 | [`M11-task-plan-guard.md`](../tasks/gpui-migration/M11-task-plan-guard.md) | The hook as a Rust binary (D17) | none (run early) | no (landed) |
| M11b | [`M11b-settings-hook.md`](../tasks/gpui-migration/M11b-settings-hook.md) | The guard as a `.claude/settings.json` hook filtered by `agent_type` (D27); frontmatter hooks removed after the live check | M11 | no |
| M12a | [`M12-dbmanager-a-tool-core.md`](../tasks/gpui-migration/M12-dbmanager-a-tool-core.md) | `fm_dbmanager::tool::Core`, snapshot, errors, backend trait; model: places, history, lists, objects, referenced-by, m2m pages | dbmanager M9, M7b | **depends on UI plan** |
| M12b | [`M12-dbmanager-b-edits.md`](../tasks/gpui-migration/M12-dbmanager-b-edits.md) | Drafts in the model, validation while typing, save, create, object conflicts, leave and close guards | M12a | **depends on UI plan** |
| M12c | [`M12-dbmanager-c-links-delete.md`](../tasks/gpui-migration/M12-dbmanager-c-links-delete.md) | Many-to-many staging with link fields, link conflicts, confirmed delete | M12b | **depends on UI plan** |
| M12d | [`M12-dbmanager-d-config-watch.md`](../tasks/gpui-migration/M12-dbmanager-d-config-watch.md) | Config watcher (missing-directory chain), automatic reload with draft re-check, Edit config with the starter | M12c | **depends on UI plan** |
| M13 | [`M13-dbmanager-docs.md`](../tasks/gpui-migration/M13-dbmanager-docs.md) | Database Manager docs, manual chapter, screenshots, Windows check | M12d, M8, every dbmanager U task | **depends on UI plan** |

Also, in the dbmanager plan:
[`../tasks/dbmanager/M15-pattern-syntax.md`](../tasks/dbmanager/M15-pattern-syntax.md)
allows the full Rust `regex` syntax in patterns (D21, D25); it can run any time, since the TS
validation mirror (dbmanager M11) is dropped and nothing but Rust validates.

### 9.2 Execution order and interleaving with the UI pipeline

1. **M11** (landed), then **M11b** next: it makes the guard actually run (§7). M11 was recommended first, because it changes the hook every
   later implementer run uses.
2. **M2a → M2b → M3a → M3b → (M3c, M4 in either order) → M5.** The U theme and shared-widget tasks may
   start after **M3a** (they need the crate, assets and fonts). The U shell
   tasks (sidebar, close prompt, drop overlay, notices, dev-mode surface)
   need **M3b**. No U scenario task starts before **M4**.
3. **M5 → U Pipeline views and scenarios; M6 → U Schedule; M7a → M7b → U Tar
   Packager.** Each tool's U tasks start after that tool's last M lands. The
   M tasks may run ahead of the U tasks of earlier tools.
4. **M8** after every U task of the three tools; **M9** after M8; **M10**
   after M9 and M11.
5. **Database Manager:** dbmanager M7 → M8 → M9 (domain) any time; M15 any
   time. **M12a** after dbmanager M9 and this plan's M7b (the patterns have
   settled), then **M12b → M12c → M12d**, each followed by the dbmanager U
   tasks that consume it; **M13** last. M12a–M13 may run before or after
   M9/M10 of this plan: their task plans say what to do in either case.

## 10. Cross-cutting rules for every post-spike task plan

1. The `fm-*` crates never depend on `gpui-pre`, `gpui-pre-platform`,
   `gpui-component` or tauri.
2. Views never touch the filesystem or call a domain crate; they call the
   tool's model (§3.3).
3. Tools never import each other; shared code goes in `fm-core` (domain) or
   `apps/filemanager/src/{widgets,platform,jobs,app}` (UI side).
4. GPUI crates and gpui-component are pinned exactly in
   `[workspace.dependencies]`; `Cargo.lock` is committed; CI uses
   `--locked`; no vendoring (D22). An upgrade is its own reviewed change.
5. No blocking I/O on the main thread (§3.4).
6. Destructive operations stay confirmed or recoverable, and batch failures
   are reported: archive overwrite, apply to an XML, pipeline steps that
   change files, dbmanager save/delete, closing with unsaved edits.
7. Tests that touch the filesystem run in a temp dir; screenshots and view
   tests use fixture backends and `FakePlatform`, never real files.
8. Paths per §10.3's reworded invariant: `Path`/`PathBuf`/`OsString`
   everywhere, dialogs included (D24); Explorer drop is the only lossy entry
   point, and a lost name is reported (§3.7).
13. File dialogs go through `Platform` with today's filters; never `rfd`
    (D24).
9. fmt and clippy `pedantic` with `-D warnings`; scoped `#[expect]` only.
10. Every interactive element is keyboard reachable and labelled (D15).
11. `test-support` and the harness never reach the release exe (D14).
12. Until M9, ts-rs and the Tauri rules of `CLAUDE.md` still bind code under
    `apps/desktop/` and any boundary type it exports.

### 10.3 `CLAUDE.md` invariant changes (approved; applied in M10)

| Today | New text |
|---|---|
| "Accessibility is a requirement, not a polish item. Every interactive element is keyboard reachable and labelled." | "Keyboard operation and labels are requirements. Every interactive element is keyboard reachable and labelled (a visible label, or a tooltip for an icon-only control). Screen-reader support, axe-style audits, high contrast and text scaling are not requirements." (D15) |
| "Paths are handled as the platform's path type, not as display strings. Code that renders a path does not assume it is valid UTF-8." | "Paths are handled as the platform's path type, not as display strings. Code that renders a path does not assume it is valid UTF-8. File dialogs are our own wrapper over the Windows common item dialog and return paths exactly. Exception: Explorer drop is GPUI's and delivers only names that are valid Unicode; a dropped name it cannot deliver is reported to the user, never skipped silently." (D11, D24) |
| Boundary types defined once and mirrored | Removed. Views use the domain crates' session types through the tool's model (§3.5). |
| `lib/generated/` and ts-rs rules | Removed. |
| `tools::register` is the only `invoke_handler`/`setup` caller | Replaced: tools are added to the single list in `apps/filemanager/src/tools/mod.rs`. |
| Webview CSP is a reviewed decision | Removed (no webview). |
| Core/UI boundary is `apps/desktop/src/lib/` | The boundary is each tool's `backend.rs`, `model.rs` and `text.rs`, plus `platform/`, `jobs.rs` and the non-view `app/` files (§3.5). |
| `fm-*` never depend on tauri | `fm-*` never depend on `gpui-pre`, `gpui-pre-platform` or `gpui-component`. |
| (new) | "The release exe never contains GPUI's `test-support` or the screenshot harness." (D14) |
| (new) | "No filesystem or other blocking I/O on the UI thread." (§3.4) |

Until M10 lands, the current `CLAUDE.md` binds every task plan, read with
these approved changes for code under `apps/filemanager/`.

## 11. The Database Manager under the migration

**Unchanged:** dbmanager M7 (save), M8 (links), M9 (delete) in
`fm-dbmanager`. If they land before this plan's M9, they keep deriving
`ts_rs::TS` and appending to `EXPORTERS`; if after, they skip both (their
task plans say so). M9 removes the derives either way.

**Re-planned** (applied 2026-11-21 to [`dbmanager.md`](dbmanager.md) and its
task plans):

| dbmanager task | Fate | Replaced by |
|---|---|---|
| M10 Tauri contract | Superseded | **M12a** (tool core, snapshot, errors, backend trait, model reads), **M12b** (drafts, save) and **M12c** (links, delete). No commands, no `lib/dbmanager.ts`, no generated types. Integers in the snapshot are integers; `DbValue` keeps its string forms. |
| M11 TS validation mirror | **Dropped** | The model calls `fm_dbmanager::rules::validate` while the user types (M12b). `validation-cases.json` stays as the Rust test vectors. |
| M5's pattern dialect | Every Rust/JavaScript restriction lifted (D21, D25) | **dbmanager M15** (any time): full Rust `regex` syntax; the scanner and the `.` warning go. |
| M12 Docs, manual, Windows check | Superseded | **M13** of this plan (screenshots via the Rust harness, after this plan's M8). |
| M13 Config watch and Edit config | Superseded, shrunk | `FileWatcher` moves in **M2b**; the missing-directory ancestor watch, automatic reload, re-checking drafts and Edit config are **M12d**. |
| M14 Window-close guard | **Dropped** | This plan's **M3b** close guard; **M12b** registers dbmanager's unsaved drafts with it. No capability exists. |
| U1 Shared controls | Merged | The ui-designer's shared-widget U tasks of this plan. |
| U2–U11, U13 | To be rewritten for GPUI by the ui-designer | Layout, interaction and copy decisions in `dbmanager-ui.md` carry over; files, tests and acceptance are redone against M12a–M12d. U13 shrinks to the close prompt's dbmanager copy, if any. |
| U12 Keyboard and audit | Reduced (D20) | Keyboard map and shortcuts help only, folded into the rewritten U tasks; no a11y sweep, axe, `/impeccable audit` or 200% text pass. |

**Drafts live in the model** (planner decision): the object sheet's draft
values, staged link edits and their field issues are dbmanager model state
(M12b, M12c), so validation, conflicts, the close guard and the config reload's
re-check are model behaviour with model tests. The ui-designer confirms this
in the rewritten UI plan.

## 12. Risks

1. **The Windows 11 checklist is outstanding (D10).** Any FAIL reopens the
   owning milestone: dialogs → M3c; drop, path-loss notice, close, open/reveal → M3b;
   live theme switch → the theme U task; `--shoot` at a non-100% scale →
   M4; table feel → the Tar Packager table U task. The planner amends that
   milestone's task plan when a FAIL is reported.
2. **Churn.** gpui-kit pins a GPUI snapshot that changes weekly. Exact pins,
   deliberate upgrade milestones, and a thin widget layer.
3. **Single publisher / supply chain** for `gpui-pre` and `gpui-component`.
   Committed lock, `--locked`, no vendoring (D22).
4. **Path-loss detection depends on a GPUI log message** (§3.7). Pinned by a
   test; an upgrade that changes it fails CI rather than going silent
   (accepted, D26).
5. **Own COM dialog code** (D24, M3c): about 150 lines of `unsafe` COM that CI
   cannot show; a manual check (`docs/gpui-dialogs-check.md`, run by the
   human on Windows 11) covers filters, default extension, modality and a
   lone-surrogate name. A FAIL reopens M3c.
6. **Parity gaps** the ui-designer's checklist must settle: middle truncation
   (widget), sticky header inside a view-level scroll, dashed 2 px drop
   border, selectable read-only text, `tabular-nums`, focus return after a
   mouse-opened dialog (gap 7), no row reflow (D13).
7. **Screenshot determinism.** Byte-identical on CI with Segoe UI and the
   embedded Cascadia Mono; every image changes once (Chromium/Inter →
   DirectWrite/Segoe UI). A developer machine at a non-100% scale gives
   different pixel sizes; the harness refuses it.
8. **Build time and exe size** grow (D8): CI builds two release exes until M9.
9. **Test rewrite cost.** ~395 vitest cases ported by behaviour; the ledger
   keeps the drop list honest.
10. **Two apps during the migration** (M3a–M8): tool behaviour changes in
    that window go into the session layer (M2) so both apps get them.
11. **CI is not Windows 11.** `windows-2025` shares the 24H2 code base; client
    behaviour is proved by the checklist.
12. **Closing during a build** kills the worker thread at exit, as today; the
    atomic writer leaves at most a temp file. Unchanged behaviour.
13. **The task-plan guard has never been enforced in the cloud runtime**
    (§7 finding): project-subagent frontmatter hooks are skipped without
    workspace trust, silently. Until M11b lands and its live check passes,
    the only protection is the agents' own instructions, and every
    implementer run so far may have read project plans. M11b moves the guard
    to `.claude/settings.json`; if `agent_type` ever stops arriving, the
    guard allows everything (fails open), so M11b's live check is repeated
    after each Claude Code upgrade that touches hooks.

## 13. Handoff to ui-designer

```
HANDOFF: ui-designer — GPUI migration (gpui-migration)

When
  Now. The spike passed and the human decided go (2026-11-21). Read
  spike/gpui/REPORT.md on branch spike/gpui-migration (git show
  origin/spike/gpui-migration:spike/gpui/REPORT.md) and the spike's
  tokens.rs, theme.rs, pipeline_screen.rs, widgets.rs.

Request
  Port FileManager's existing UI to GPUI + gpui-kit so it looks similar to
  today: same layout, tokens, components and states (DESIGN.md,
  apps/desktop/src/styles/*.css, the React views). Not a redesign. Write
  docs/plans/project/gpui-migration-ui.md and the U<n> task plans under
  docs/plans/tasks/gpui-migration/. Also rewrite the implementation parts of
  docs/plans/project/dbmanager-ui.md for GPUI and its U tasks (as U tasks of
  this plan, or as replacements in docs/plans/tasks/dbmanager/ — your call;
  the old U1–U13 are marked superseded).

Must produce
  1. Theme mapping: every token in tokens.css (light and dark) to a field of
     apps/filemanager/src/theme/tokens.rs, and each gpui-kit theme slot to a
     token or "unused"; the gap-9 slots set on elements; scrim; rem 16 px;
     fonts (Segoe UI; Cascadia Mono embedded, family name as M3a registers
     it); live system light/dark. Keep the tokens test and the contrast-pair
     test.
  2. Component inventory: every shared and tool-local React component mapped
     to a gpui-kit component, a wrapper in src/widgets/, or a custom widget,
     with its states (hover, active, focus, disabled, busy, read-only,
     invalid, current). Include middle-truncated path, tri-state checkbox,
     danger button, extended confirm dialog, menu button, shortcuts help,
     tags, and the dbmanager form controls.
  3. Per-screen parity checklists (shell, Pipeline, Schedule Creator, Tar
     Packager, Database Manager): regions, geometry, states, copy source
     (the model's text.rs), keyboard map, and the accepted differences:
     overlay shadow fixed (D12), no row reflow and sideways scroll (D13),
     focus return after mouse-opened dialogs (gap 7), and the items of §12
     risk 6.
  4. Screenshot scenarios: every image in docs/manual/images/ (30) as a named
     scenario with its fixture state and steps; the dbmanager images the
     manual chapter needs.
  5. U task plans: theme + tokens tests; shared widgets; shell views (sidebar
     with current marker, placeholder replacement, close prompt, drop
     overlay, path-loss notice, dev-mode surface); then each tool's views;
     each tool's scenarios. Each U task fills its rows of
     docs/gpui-test-ledger.md.
  6. DESIGN.md and PRODUCT.md updates for M10: normative source
     theme/tokens.rs; platform "desktop (native, GPUI)"; Windows 11 only;
     the accessibility section per D15.

Backend milestones the UI depends on (ordering)
  - M3a (crate, assets, fonts, Shell with its chrome slot, tool registry,
    dev mode) before the theme, widget and shell U tasks.
  - M3b (Platform, jobs, drop routing, close guard, path-loss notice state)
    before the shell U tasks that render the overlay, prompt and notice.
  - M4 (harness and scenario API) before any scenario task.
  - M5 / M6 / M7a+M7b (each tool's backend, model, text.rs, fixtures) before
    that tool's view tasks. Order: Pipeline, Schedule, Tar Packager.
  - M12a / M12b / M12c / M12d before the dbmanager view tasks that consume
    them (reads and navigation; drafts, save, create, close guard; links and
    delete; config reload and Edit config).
  - M8 consumes all scenario tasks of the three tools; M13 the dbmanager ones.

Constraints already established
  - Similar, not redesigned (D3, D7).
  - Light and dark follow the system; no toggle.
  - Keyboard reachable and labelled; no screen-reader, axe or 200% text
    work (D15, D20).
  - Views render the model and call its methods; behaviour, copy and
    predicates are in model.rs / text.rs (§3.3). A view-level need the model
    lacks is a request to the planner, not view code.
  - Confirmations as today: overwrite, apply, pipeline steps that change
    files, dbmanager save/delete, close with unsaved edits.
  - impeccable: shape and critique on screenshots only (D16).
  - Windows 11 only; scale factor 1 for screenshots; 1000×700 default,
    1366×800 for Tar Packager shots, min window 800×560.
  - Dialogs are the native Windows picker with today's filters (D24, M3c);
    path-loss notice copy (drop only) is in §3.7.
```

### 13.1 The ui-designer's requests, applied (2026-11-21)

| # | Request | Applied in |
|---|---|---|
| R1 | `Scenario::clip` (crop to a debug selector's bounds) | M4; M8's size check reads "size or clip" |
| R2 | ui-implementer may add icon SVGs and `icons!` lines | M3a ownership rule |
| R3 | `--dark` calls `fm_app::theme::set_override(Some(ThemeMode::Dark), cx)` | M4 |
| R4 | `EditorInput`; reference picker in the model with its own list query; `request_delete_of` | M12b, M12c |
| R5 | consumable `take_focus_hint`; dbmanager hints `SheetHeading`, `ListHeading`, `ErrorSummary`, `ErrorBanner`, `PickerTrigger(field)`, `DeleteNotice` | M5, M6, M7a, M7b, M12a (enum), M12b, M12c |
| R6 | dbmanager fixtures: config reload making `pages` read-only; deletable but referenced `publisher`; one error + one warning; syntax error | M12b, M12c, M12d |
| R7 | `Shell::request_close(window, cx)`, same path as the should-close hook | M3b |

Also settled: the Database Manager's list page is **25** objects
(`dbmanager-ui.md` §1.5 said 100 in its first draft; M12a, the U tasks and
the approved UI plan use 25).

## 14. Questions

### 14.1 Answered

- **Q4 →** D6, **Q5 →** D7, **Q6 →** D8, **Q12 →** D9 (2026-11-20).
- **Go/no-go →** D10; report conditions 1, 2, 3, 5 → D11, D12, D13, D14;
  condition 4 (the checklist) → D10 (2026-11-21).
- **Q1 →** D15, **Q2 →** D16, **Q3 →** D17, **Q7 →** D18, **Q8 →** D19,
  **Q9 →** D20, **Q10 →** D21, **Q11 →** D22, **Q13 →** D23 (2026-11-21).
- **Q14 →** D24, **Q15 →** D25, **Q16 →** D26; plan approved (2026-11-21).

### 14.2 Open

None.
