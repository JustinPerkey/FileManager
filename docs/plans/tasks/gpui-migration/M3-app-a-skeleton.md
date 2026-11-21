# M3a — The GPUI app skeleton: crate, window, assets, fonts, Shell, tool registry, dev mode, CI

Status: approved (2026-11-21)
Project: gpui-migration   Depends on: M2b (landed)

## Goal

Create `apps/filemanager`, the GPUI app (package and binary
`filemanager-gpui`), beside the shipping Tauri app: pinned GPUI and
gpui-component, a window that opens and renders, embedded icons and the
Cascadia Mono font, the `Shell` root view with a chrome slot for the
ui-implementer, the tool registry with placeholder tools, the app-wide
developer mode, a CI build and dumpbin check of the new exe, and the test
ledger that tracks how today's frontend tests are ported.

## Context

FileManager (a portable Windows 11 exe) is moving its UI from Tauri + React
to **GPUI** (Zed's native Rust UI framework) with the **gpui-component**
widget library. A throwaway spike on branch `spike/gpui-migration`
(directory `spike/gpui/`, never merged) proved the stack; you may port its
files by name: `git show origin/spike/gpui-migration:spike/gpui/<path>`.
Its `REPORT.md` there records the versions and API names used below.

The new app is built beside `apps/desktop` (Tauri) until a later cutover
milestone deletes `apps/desktop` and renames this package and binary to
`filemanager`. Until then the Tauri exe ships, and both must build.

### Versions (exact pins, no vendoring; `Cargo.lock` is committed)

Add to the root `[workspace.dependencies]`:

```toml
gpui = { package = "gpui-pre", version = "=0.3.12" }
gpui_platform = { package = "gpui-pre-platform", version = "=0.3.12", default-features = false }
gpui-component = "=0.10.0"
gpui-kit-assets = "=0.10.0"
async-channel = "2"
```

`gpui_platform` must have `default-features = false` at the workspace level
(a member cannot turn defaults off). On Linux this compiles a headless-only
backend with no apt packages; that is intended: Linux runs tests only.
GPUI's crates are edition 2024; this crate uses the workspace's edition 2021
and `rust-version` 1.90, as the spike did.

### What to build

**Workspace and crate.** Add `apps/filemanager` to `members`. Package
`filemanager-gpui`, `[lib] name = "fm_app"` (the lib name stays stable
across the later rename), binary `filemanager-gpui` (`src/main.rs`),
`[lints] workspace = true`, the workspace `version`/`edition`/`license`.
Dependencies: the five `fm-*` crates by path, `gpui`, `gpui_platform`,
`gpui-component`, `gpui-kit-assets`, `async-channel`. Dev-dependencies:
`gpui = { workspace = true, features = ["test-support"] }`, `tempfile`.

**`build.rs`.** Port `spike/gpui/build.rs`: on a Windows target, embed
`assets/icon.ico` as resource id 1 with `winresource`
(`[target.'cfg(windows)'.build-dependencies] winresource = "0.1"`), which is
where GPUI's Windows backend loads the window icon. Copy
`apps/desktop/src-tauri/icons/icon.ico` to `apps/filemanager/assets/icon.ico`.
GPUI already embeds the application manifest (DPI awareness, Common Controls
6); do not add one.

**`src/main.rs`.** `#![windows_subsystem = "windows"]`; parses one optional
flag, `--self-check` (open the window, let it render its first frame, quit
after 500 ms with exit code 0; exit non-zero if the window cannot open), and
calls `fm_app::run`.

**`src/lib.rs`, `run()`.** `gpui_platform::application().with_assets(Assets).run(|cx| …)`:
`gpui_component::init(cx)` first, register the fonts, initialise dev mode,
build the tools from the registry, open the main window with
`WindowOptions { window_bounds: centred 1000×700, window_min_size: 800×560,
titlebar title "FileManager", .. }`, root view
`gpui_component::Root::new(shell, window, cx)` (dialogs and popovers need
it). When the main window closes, quit (`on_window_closed` → `cx.quit()`).
Leave gpui-component's default theme; a UI task replaces it.

**`src/assets.rs`.** Port `spike/gpui/src/assets.rs`: our icons first, then
`gpui_kit_assets::Assets` (gpui-component's checkbox mark, menu check and
dialog close load from there). The icons are the ten of
`apps/desktop/src/app/icons.tsx` (`check-circle`, `alert-triangle`,
`circle`, `x-circle`, `info`, `chevron-down`, `folder`, `copy`, `x`,
`keyboard`); port `spike/gpui/assets/icons/*.svg` into
`apps/filemanager/assets/icons/`. Keep `icon_path(name)` and `ICON_NAMES`.

**Fonts (`src/fonts.rs`, `assets/fonts/`).** The human decided to embed
**Cascadia Mono** (SIL Open Font License 1.1) so text renders the same on
every machine. Download the latest release of `microsoft/cascadia-code` from
GitHub and take the static `CascadiaMono-Regular.ttf` and
`CascadiaMono-Bold.ttf` and the release's `OFL.txt`. Commit them under
`apps/filemanager/assets/fonts/` with a `README.md` stating the release tag,
the download URL and each file's SHA-256. Embed both fonts with
`include_bytes!` and register them at startup with GPUI's text system
(`cx.text_system().add_fonts(..)`; confirm the exact name in the pinned
`gpui-pre` source). Embed `OFL.txt` too (`pub const CASCADIA_LICENSE: &str =
include_str!(..)`) so the licence travels with the exe. Expose
`pub const MONO_FAMILY: &str = "Cascadia Mono";`. Segoe UI is a Windows 11
system font and is not embedded.

**Shell (`src/app/shell.rs`).** The window's root view, an entity, owned by
you (the backend implementer): window-level behaviour only. It holds the
current tool (`ToolId`), the tool slots from the registry, and the chrome
view. It renders a root `div` with key context `"Shell"` and the dev-mode
handling, with the chrome view as its only child. Public API (the UI
contract; later milestones add to it):

```rust
impl Shell {
    pub fn current(&self) -> ToolId;
    pub fn select(&mut self, id: ToolId, window: &mut Window, cx: &mut Context<Self>); // calls the tool's activated()
    pub fn tools(&self) -> &[ToolSlot];                  // registry order
    pub fn tool_view(&self, id: ToolId) -> AnyView;      // for the chrome to place
}
```

**Chrome slot (`src/app/view/mod.rs`).** `pub fn chrome(shell: Entity<Shell>,
window: &mut Window, cx: &mut App) -> AnyView`: the visual shell (sidebar,
current marker, the current tool's view, overlays). It belongs to the
ui-implementer, who replaces its body. You provide a deliberately plain
placeholder (`src/app/view/placeholder.rs`): a column of one gpui-component
`Button` per tool (label from the registry, debug selector
`"tool-nav-<id>"`) and the current tool's view. No styling beyond defaults.

**Tool registry (`src/tools/mod.rs`).** The single list of tools, in today's
order and with today's labels: Pipeline (`pipeline`), Tar Packager
(`tarpack`), Schedule Creator (`schedule`).

```rust
pub enum ToolId { Pipeline, Tarpack, Schedule }        // Copy, Eq, Debug; id() -> &'static str
pub struct ToolSlot { pub id: ToolId, pub label: &'static str, pub view: AnyView, pub hooks: Box<dyn ToolHooks> }
pub trait ToolHooks { fn activated(&self, window: &mut Window, cx: &mut App); }
pub fn build(window: &mut Window, cx: &mut App) -> Vec<ToolSlot>;
```

`ToolHooks` grows in the next milestone (drop target, unsaved work); give it
default methods so placeholder tools stay small. Each tool gets a module
`src/tools/<tool>/mod.rs` with a placeholder model and
`src/tools/<tool>/view/mod.rs` with a placeholder view showing
"<label>: not ported yet". Later milestones replace the model, and UI tasks
the view. Adding a tool is: a module, an entry in `build`, a `ToolId`
variant.

**Developer mode (`src/app/dev_mode.rs`).** App-wide, **off at every
launch**, toggled by **Ctrl+Alt+Shift+D** (deliberately absent from any
shortcuts list). Today's behaviour is `apps/desktop/src/app/devMode.tsx`:
the chord toggles; a held key's auto-repeat does not toggle again; the chord
works whatever has focus. Build: a global `DevMode(bool)` with
`DevMode::is_on(cx: &App) -> bool` and `DevMode::toggle(cx)`, which notifies
global observers (views and models use `cx.observe_global::<DevMode>`).
Bind the chord with `gpui::actions!` and `KeyBinding::new("ctrl-alt-shift-d", ToggleDevMode, None)`,
handled on the Shell. If an action handler cannot tell an auto-repeated key
from a fresh press, handle the chord in an `on_key_down` listener on the
Shell instead (`KeyDownEvent::is_held`), and say so in the PR. Put the two
strings today's view announces ("Developer mode on.", "Developer mode off.")
in `src/app/text.rs` as constants; whether and how they show is the UI's
decision.

**CI (`.github/workflows/ci.yml`).**

- Pin the windows job to `runs-on: windows-2025`.
- Windows: after the Tauri build, `cargo build --release -p filemanager-gpui --locked`;
  run the existing dumpbin check on **both** `target\release\filemanager.exe`
  and `target\release\filemanager-gpui.exe` (same forbidden list:
  `vcruntime*`, `msvcp*`, `api-ms-win-crt-*`, `zstd*`, `liblzma*`,
  `sqlite3*`, `WebView2Loader.dll`); then copy `filemanager-gpui.exe` alone
  into an empty folder and run it with `--self-check` (`Start-Process
  -PassThru`, 60 s timeout; port the step from the spike workflow
  `.github/workflows/spike-gpui.yml` on the spike branch); fail on a
  non-zero exit. Upload `filemanager-gpui.exe` as an extra artifact
  `FileManager-gpui-<version>-x64.exe` (version from `tauri.conf.json` as
  today).
- Linux: no new step; `cargo clippy --workspace` and `cargo test --workspace`
  now include the new crate, headless.
- Replace the "fm-* crates must not depend on tauri" step with one that
  checks **all five** crates (`fm-core`, `fm-tarpack`, `fm-schedule`,
  `fm-pipeline`, `fm-dbmanager`) against **each of** `tauri`, `gpui-pre`,
  `gpui-pre-platform`, `gpui-component`, with the same "did not match any
  packages" logic as today.

**Test ledger (`docs/gpui-test-ledger.md`).** Every frontend test case under
`apps/desktop/src/**/*.test.ts(x)` (about 395; include `it.each` rows as one
case each, named as the `it.each` title) in a table per file:
`Case | Owner | New test | Status`. Owner is the proposal below; Status is
`open` except for the rows this milestone settles. Owner proposal by file:
`app/devMode.test.tsx` → M3a; `app/DropZone.test.tsx` → M3b (enabled/drop
routing) or U (overlay look); `app/AppShell.test.tsx`, `app/ToolNav.test.tsx`
→ U (shell), except registry-order cases → M3a; `App.test.tsx`,
`app/{Button,Banner,ConfirmDialog}.test.tsx`, `styles/tokens.test.ts` → U;
`tools/pipeline/*` → M5 or U; `tools/schedule/*` → M6 or U;
`tools/tarpack/{errorMessages,DropResult,ManifestHeader,ManifestErrors,DiagnosticList}*`
and `TarpackView.{states,assign,errors}*` → M7a or U;
`tools/tarpack/{reportText,BuildBar,BuildResult,BuildReport,FailureList,FormatPicker,useTarpackShortcuts}*`
and `TarpackView.build*` → M7b or U; `tools/tarpack/{pathParts,MiddlePath,EntryTable*,ShortcutsHelp,EntryStatus}*`
→ U; `TarpackView.a11y*` → dropped unless a case checks keyboard operation
or a label (then U); `lib/*.test.ts` → dropped ("IPC wrapper; no IPC in the
GPUI app"). A one-paragraph header explains the columns and that the cutover
milestone refuses to start while any row is `open`. This file lives outside
`docs/plans/` so both implementer roles can read and update it.

Settle in this milestone: the `devMode.test.tsx` cases (ported as model
tests below, or dropped with a reason such as "aria-live announcer"), the
registry-order cases, and every `lib/*.test.ts` row (dropped).

### Rules that bind this task

- The `fm-*` crates never depend on GPUI or tauri; nothing under `crates/`
  changes in this milestone.
- Tools never import each other: `tools/<tool>/` modules do not reference
  each other.
- No filesystem or other blocking I/O on the UI thread (none is needed here).
- Ownership: you own everything this task creates **except** the body of
  `src/app/view/` beyond the placeholder, and `src/tools/<tool>/view/`
  beyond the placeholder; those belong to the ui-implementer later. Make no
  visual decisions: no colours, spacing or theme.
- Every interactive element is keyboard reachable and labelled: the
  placeholder buttons are gpui-component `Button`s with their label.
- GPUI's `test-support` feature must not reach the release exe: it is a
  dev-dependency feature only here. Check with
  `cargo tree --locked -p filemanager-gpui -e features --target x86_64-pc-windows-msvc -i gpui-pre`:
  no `test-support` appears.
- fmt clean; clippy `pedantic` with `-D warnings`; scoped `#[expect]` only.
  GPUI code trips `too_many_lines` in big `render` functions: split them.
- Tests touching the filesystem use temp dirs (none needed here).
- The Tauri app keeps building, testing and passing dumpbin unchanged.

## Files

Create: `apps/filemanager/{Cargo.toml,build.rs}`,
`apps/filemanager/assets/{icon.ico,icons/*.svg,fonts/CascadiaMono-Regular.ttf,fonts/CascadiaMono-Bold.ttf,fonts/OFL.txt,fonts/README.md}`,
`apps/filemanager/src/{main.rs,lib.rs,assets.rs,fonts.rs}`,
`apps/filemanager/src/app/{mod.rs,shell.rs,dev_mode.rs,text.rs}`,
`apps/filemanager/src/app/view/{mod.rs,placeholder.rs}`,
`apps/filemanager/src/tools/{mod.rs,pipeline/mod.rs,pipeline/view/mod.rs,tarpack/mod.rs,tarpack/view/mod.rs,schedule/mod.rs,schedule/view/mod.rs}`,
`docs/gpui-test-ledger.md`.

Edit: `Cargo.toml`, `Cargo.lock`, `.github/workflows/ci.yml`, `CLAUDE.md`
(Status: "a GPUI app is being built beside the Tauri shell in
`apps/filemanager` (package `filemanager-gpui`); the Tauri exe still ships";
Layout: the `apps/filemanager/` tree as built; Commands:
`cargo run -p filemanager-gpui`).

Do not touch: `crates/`, `apps/desktop/`, `docs/plans/`.

## Acceptance criteria

- `cargo build --release -p filemanager-gpui --locked` succeeds on
  Windows CI; the dumpbin step passes for both exes; `--self-check` from a
  folder holding only the exe exits 0.
- The `cargo tree … -i gpui-pre` features check shows no `test-support` for
  the release graph.
- The five-crate dependency check passes for all four forbidden crates.
- Both CI jobs run fmt, clippy pedantic and `cargo test --workspace` green,
  with the new crate's tests on Linux (headless) and Windows.
- `docs/gpui-test-ledger.md` lists every case of every frontend test file;
  a reviewer can check the count against
  `grep -cE "^\s*(it|test)(\.each)?\(" ` per file (state your counting rule
  in the header). The rows settled here say where they went.
- `CLAUDE.md` records the new crate, its package name, run command and
  that the Tauri exe still ships.

## Tests proving completion

In `apps/filemanager` (`cargo test -p filemanager-gpui`):

- `assets::tests::every_icon_loads_and_is_svg` (ported from the spike),
  plus `gpui_kit_assets_are_the_fallback` (`icons/check.svg` loads).
- `fonts::tests::cascadia_mono_is_registered` (`#[gpui::test]`: after
  registration, `cx.text_system().all_font_names()` contains
  `MONO_FAMILY`) and `license_is_embedded` (`CASCADIA_LICENSE` contains
  "SIL OPEN FONT LICENSE").
- `tools::tests::registry_order_and_labels` (Pipeline, Tar Packager,
  Schedule Creator; ids `pipeline`, `tarpack`, `schedule`; unique ids).
- `app::shell::tests::select_changes_current_and_activates` (a counting
  `ToolHooks`; `select` calls `activated` once on the newly current tool;
  the first tool is current at start).
- `app::shell::tests::placeholder_nav_selects_by_click` (`VisualTestContext`,
  `simulate_click` on `"tool-nav-schedule"`; `current()` is `Schedule`).
- `app::dev_mode::tests::off_at_launch`, `chord_toggles`,
  `held_chord_does_not_toggle_again`, `chord_works_without_focus` (no
  element focused in a fresh window), `observers_are_notified`.

Commands: `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`; on Windows also the release build and dumpbin.

## Out of scope

- `Platform`, `jobs.rs`, drop routing, the close guard, the logger (next
  milestone).
- The screenshot harness and the `shoot` feature (M4).
- Theme, tokens, styled widgets, the real sidebar (UI tasks).
- Any tool behaviour (M5 onward).

## Risks

- The release build grows CI time by about ten minutes (two release exes);
  accepted (no limit on build time).
- `winresource` needs `rc.exe` from the Windows SDK; it is on the runner.
  GPUI's build script compiles HLSL with `fxc.exe` from the SDK in release
  builds; same.
- Font registration API names may differ from Zed's current GPUI; read the
  pinned `gpui-pre` 0.3.12 source.
- Cargo feature unification: if any workspace member enabled `gpui`'s
  `test-support` as a normal dependency, `cargo build --workspace` would put
  it in every exe. Keep it in `[dev-dependencies]` only.
