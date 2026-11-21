# M4 — The screenshot harness: feature `shoot`, `filemanager-shoot`, scenario API, CI upload

Status: approved (2026-11-21)
Project: gpui-migration   Depends on: M3b (landed)

## Goal

Render the real GPUI app to PNG unattended, from named scenarios with
fixture backends and a scripted platform, through a separate binary that is
never part of the release exe, and run it on Windows CI with the PNGs
uploaded as an artifact.

## Context

FileManager is moving from Tauri + React to GPUI. The user manual's 30
screenshots (`docs/manual/images/*.png`) are produced today by Node scripts
(`docs/manual/screenshots/shoot.js`, `lib.js`, `mock.js`: Chromium,
Playwright, a mocked Tauri backend). The human decided the screenshots stay
generated from the real UI with fixture data, without Node, and that
**screenshot capture never ships in the release exe**.

What exists in `apps/filemanager` (package `filemanager-gpui`, lib `fm_app`):
`run()` in `src/lib.rs`, `Shell` (`src/app/shell.rs`: `select`, `current`,
`drag_over`, `notice`, `close_request`, …), the chrome slot
`src/app/view/mod.rs::chrome`, the tool registry `src/tools/mod.rs`
(`ToolId`, `ToolSlot`, `ToolHooks`, `build`), `Platform` with
`FakePlatform` (`src/platform/`), `jobs.rs`, assets and the embedded
Cascadia Mono (`src/fonts.rs`).

The spike proved the method (branch `spike/gpui-migration`; read with
`git show origin/spike/gpui-migration:spike/gpui/<path>`):
`src/shoot.rs` (open a window, wait for the frame to settle, call
`Window::render_to_image()`, save with `image::RgbaImage::save`), its CI job
`shoot` in `.github/workflows/spike-gpui.yml`, and `src/check.rs::diff` (a
byte/pixel comparison of two PNGs). Facts:

- `Window::render_to_image() -> RgbaImage` exists only with GPUI's
  **`test-support`** feature on `gpui` and `gpui_platform`. On Windows it
  draws GPUI's last scene into an offscreen texture: other windows never
  appear in the picture; no GPU is needed (WARP on the CI runner).
- Two runs on `windows-2025` gave byte-identical PNGs.
- The image is in device pixels: at a display scale of 125% a 1000×700
  window gives a larger PNG.
- GPUI records **debug selectors only in debug builds**, so a harness that
  clicks by selector must be a dev-profile build.

### What to build

**Cargo.** In `apps/filemanager/Cargo.toml`:

```toml
[features]
shoot = ["gpui/test-support", "gpui_platform/test-support", "dep:image"]

[[bin]]
name = "filemanager-shoot"
path = "src/bin/filemanager-shoot.rs"
required-features = ["shoot"]
```

`image = { workspace = true, optional = true }` with
`image = { version = "0.25", default-features = false, features = ["png"] }`
in `[workspace.dependencies]`. The default feature set stays empty.

**Fixture switch.** Tool fixture backends (written by each tool's milestone)
are compiled under `#[cfg(any(test, feature = "shoot"))]`. Add
`fm_app::run_with(AppSetup)`, used by `run()` and by the harness, where
`AppSetup` carries the `Platform` to install and a `BackendSet` choice
(`Real` or `Fixture(FixtureId)` per tool). `run()` passes `GpuiPlatform` and
real backends. Until tools have backends, `BackendSet` covers only the
placeholders; each tool milestone adds its fixtures to it.

**Harness (`src/shoot/`, feature `shoot`).** You own all of it except
`src/shoot/scenarios/`, which the ui-implementer fills; you write the API
and one sample scenario.

```rust
pub struct Scenario {
    pub name: &'static str,                 // = the PNG stem, e.g. "pipeline-chosen"
    pub size: (f32, f32),                   // logical px; 1000×700 default, 1366×800 Tar Packager
    pub setup: fn(&mut AppSetup),           // fixtures, scripted platform answers
    pub steps: &'static [Step],
}
pub enum Step {
    SelectTool(ToolId),
    Click(&'static str),                    // debug selector
    Keys(&'static str),                     // GPUI keystroke syntax, "ctrl-alt-shift-d"
    Type(&'static str),                     // into the focused input
    DragEnter(&'static [&'static str]),     // paths, for the drop overlay
    Drop(&'static [&'static str]),
    Watcher(ToolId),                        // inject a "file changed on disk" signal into the tool's model
    Call(fn(&mut Window, &mut App)),        // escape hatch: drive a model directly
    WaitIdle,                               // run until no task is pending and no frame is dirty
    Wait(u64),                              // ms, for timed states (progress)
}
pub fn scenarios() -> Vec<Scenario>;        // from scenarios/mod.rs, registry order
```

`Watcher(ToolId)` dispatches through a `ToolHooks` method
`fn inject_file_changed(&self, window, cx)` added here with a default that
does nothing; the Tar Packager milestone implements it. Adjust `Step` if a
variant cannot be done in GPUI; record the change in the module doc.

`src/bin/filemanager-shoot.rs`: `filemanager-shoot --out <dir> [--only <name>,…] [--prefix <p>] [--dark]`.
For each scenario: start the app with its setup (light theme, or dark with
`--dark` by forcing the window appearance, if the theme follows it), open the
window at its size, play the steps, wait for idle plus a settle time (start
from the spike's 1.5 s for the first frame, then render as soon as idle),
call `render_to_image`, and write `<out>/<prefix><name>.png`. It fails that
scenario, and exits non-zero at the end, if a selector does not resolve, a
wait times out (30 s), the capture fails, or `window.scale_factor() != 1.0`
(message: "display scale is not 100%; screenshots are taken at 100%"). It
prints one line per scenario (`ok <name>` or `FAIL <name>: <reason>`).
Whether one process per scenario or one app with a fresh window each is up to
you; scenarios must not see each other's state.

**Sample scenario (`src/shoot/scenarios/mod.rs`).** `shell-sample`: the
placeholder shell at 1000×700 with the Schedule Creator selected. The
ui-implementer replaces or removes it.

**CI (`.github/workflows/ci.yml`, windows job).**

- `cargo clippy -p filemanager-gpui --features shoot --all-targets --locked -- -D warnings`
  (the workspace clippy run does not build the harness).
- `cargo build -p filemanager-gpui --features shoot --bin filemanager-shoot --locked`
  (dev profile), then run it twice into `shots/run1` and `shots/run2`, and
  compare each pair byte for byte; write `shots/determinism.txt`. A
  difference is **reported, not failed**: the human decided CI never fails
  on image differences. A scenario that fails to run **does** fail the job.
- Upload `shots/` as artifact `screenshots` (`if: always()`).
- The release-graph check: `cargo tree --locked -p filemanager-gpui -e features --target x86_64-pc-windows-msvc -i gpui-pre`
  must not mention `test-support`; fail otherwise. Also assert that
  `target\release\` contains no `filemanager-shoot.exe` after the release
  build.

**Docs.** `docs/manual/screenshots/README.md` gains a section "GPUI harness"
(the command, where PNGs go, that it needs Windows at 100% display scale,
that CI uploads them). The Node scripts stay until the manual cutover
milestone.

### Rules that bind this task

- **`test-support` and the harness never reach the release exe.** The
  release build is `cargo build --release -p filemanager-gpui` with default
  features.
- Screenshots use fixture backends and `FakePlatform`: no real files, no
  real dialogs. The harness writes only into `--out`.
- No blocking I/O on the UI thread (the harness may block its own main
  between scenarios, outside the GPUI run loop).
- Ownership: `src/shoot/scenarios/` is the ui-implementer's after the sample;
  everything else here is yours. No visual decisions.
- fmt clean; clippy pedantic `-D warnings` with and without `--features shoot`;
  scoped `#[expect]` only.

## Files

Create: `apps/filemanager/src/shoot/{mod.rs,steps.rs,scenarios/mod.rs}`,
`apps/filemanager/src/bin/filemanager-shoot.rs`.

Edit: `Cargo.toml`, `Cargo.lock`, `apps/filemanager/{Cargo.toml,src/lib.rs,src/tools/mod.rs}`,
`.github/workflows/ci.yml`, `docs/manual/screenshots/README.md`,
`CLAUDE.md` (Layout: `src/shoot/`, `src/bin/`; Commands: the harness
command; Invariants: add "The release exe never contains GPUI's
`test-support` or the screenshot harness").

Do not touch: `crates/`, `apps/desktop/`, `docs/manual/images/`,
`docs/plans/`.

## Acceptance criteria

- `cargo run -p filemanager-gpui --features shoot --bin filemanager-shoot -- --out shots`
  on Windows writes `shots/shell-sample.png` at 1000×700 and exits 0.
- The CI windows job runs the harness twice, uploads both runs and the
  determinism file, and is green; run1 and run2 are byte-identical for the
  sample on CI (report it in the PR).
- The release-graph check and the "no `filemanager-shoot.exe` in release"
  check pass.
- `--only` and `--prefix` work; an unknown `--only` name is an error.
- A broken scenario (a selector that does not exist) makes the binary exit
  non-zero with a `FAIL` line (tested).

## Tests proving completion

Under `cargo test -p filemanager-gpui --features shoot` (add this command to
the windows CI job, and to the linux job if the `shoot` feature builds there;
the capture itself is Windows-only, so capture tests are `#[cfg(windows)]`):

- `shoot::tests::scenario_names_are_unique_and_valid_file_stems`.
- `shoot::tests::steps_play_against_the_shell` (`#[gpui::test]`: the sample
  scenario's steps run in a `VisualTestContext`, ending with Schedule
  current).
- `shoot::tests::missing_selector_fails_the_scenario`.
- `shoot::tests::capture_writes_png` (`#[cfg(windows)]`, temp dir).
- CI: the two runs and the determinism comparison.

## Out of scope

- Tool scenarios (UI tasks) and tool fixtures (tool milestones).
- Replacing the manual's images and deleting the Node scripts (manual
  cutover milestone).
- Pixel or colour checks against references (no CI failure on images).

## Risks

- Dev-profile GPUI renders slower; the settle and idle waits must not be
  tuned to the developer's machine. Use idle detection, not fixed sleeps,
  except where a scenario shows a timed state.
- A developer's machine at 125% scale cannot produce manual images; the
  harness refuses rather than writing wrong sizes. The human's Windows 11
  checklist run covers local `--shoot`; a FAIL there reopens this milestone.
- Feature unification: building the workspace with `--features shoot` for
  clippy must not be the build that produces the release exe (separate
  command, as above).
