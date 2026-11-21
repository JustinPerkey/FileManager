# GPUI spike report

Branch `spike/gpui-migration`, throwaway, never merged. Sections 1-6 are the
summary the human decides on; "Part A details" and "Part B details" follow.
Part A evidence: CI run
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

## 1. Versions

| Item | Value |
| --- | --- |
| `gpui-component` | 0.10.0 (Apache-2.0), exact pin |
| GPUI | crates.io `gpui-pre` 0.3.12 (lib name `gpui`), with `gpui-pre-platform` =0.3.12 and `gpui-kit-assets` =0.10.0 (the bundled icon set: checkbox mark, menu check, dialog close) |
| Rust | stable at run time: 1.100.0 (edition 2024 in all GPUI crates; spike `rust-version` 1.90) |
| CI runner | `windows-2025`: Windows Server 2025 Datacenter, 10.0.26100 (the Windows 11 24H2 code base), runner image `windows-2025-vs2026` version 20261116.315.1 (part B runs; part A ran on an earlier image of the same label), no GPU ("Microsoft Hyper-V Video", DirectX 11 on WARP) |
| Human's Windows 11 | NEEDS-HUMAN: edition, version and build go in the machine block of `CHECKLIST.md` |

Part B CI evidence (workflow `spike-gpui`, commit `ebe9d0e`): run <https://github.com/JustinPerkey/FileManager/actions/runs/44841007688>
(artifacts `fm-spike`, `fm-spike-shots`: both PNGs, the side-by-side image, the
colour and geometry check output, the determinism result). The run at commit `43cb9ef`, which also
prints the widget measurements: <https://github.com/JustinPerkey/FileManager/actions/runs/44844953255>.

## 2. Results

Hard / Soft per the plan. PASS means shown by a test or CI result as stated;
what only a person can see is NEEDS-HUMAN and has a `CHECKLIST.md` step.

| Item | Class | Status | Evidence |
| --- | --- | --- | --- |
| A1 portable exe | Hard | PASS (exe start: self-check; icon and desktop run NEEDS-HUMAN) | dumpbin clean, self-check drew the first frame; part A details |
| A2 file dialogs | Hard | PARTIAL | work for ordinary names; no save filter in GPUI's dialog (rfd fallback); non-Unicode names fail in GPUI and rfd (source), live NEEDS-HUMAN 4-10 |
| A3 drop from Explorer | Hard | PARTIAL | works for ordinary names (test `external_drop_delivers_paths`); non-Unicode names skipped silently (source), NEEDS-HUMAN 11-15 |
| A4 close interception | Hard | PASS headless; live NEEDS-HUMAN 16-20 | test `close_is_held_while_dirty` |
| A5 open / reveal | Soft | PASS in source; live NEEDS-HUMAN 21-24 | `open_with_system`, `reveal_path` |
| A6 headless tests on Windows CI | Hard | PASS | 36 tests pass on `windows-2025` three runs in a row (run 44841007688) |
| A7 background work, progress, watching | Hard | PASS headless; live NEEDS-HUMAN 25-27 | `build_progress_reaches_the_view`, `watcher_reports_edit` |
| A8 size, build time, licenses | Soft | PASS (report only) | section 3 |
| A9 Linux build | Soft | PASS (build and headless tests only) | linux job |
| A10 clippy pedantic | Hard | PASS | 5 scoped `#[expect]`, section 3 |
| B1 text input | Hard | PASS headless (placeholder and invalid-border looks NEEDS-HUMAN 39) | `input_paste_and_submit` |
| B2 checkbox with "not set" | Hard | PASS, 0 lines of custom widget code | `tri_state_checkbox_cycles`, `tri_state_checkbox_is_keyboard_operable` |
| B3 virtual table | Hard | PASS headless (smoothness NEEDS-HUMAN 41); variable row height NO, middle truncation custom | `table_keyboard_focus_moves_and_scrolls`, `table_scroll_steps_are_cheap` |
| B4 modal confirmation | Hard | PARTIAL: behaviour passes, the shadow cannot take `--shadow-overlay` | `dialog_focus_starts_on_cancel_and_escape_cancels`, `dialog_busy_is_inert` |
| B5 menu and popover | Soft | PASS behaviour; shadow not applied (no slot) | `menu_keys_wrap_and_escape_refocuses`, `popover_opens_under_its_button_and_escape_closes_it` |
| B6 themed screen | Hard | PASS in CI, light and dark; live theme switch NEEDS-HUMAN 31-33 | 0 failing checks, quoted in "Part B details" |
| B7 screenshot to PNG | Hard | PASS in CI (method 1); the human's machine and a non-100% scale NEEDS-HUMAN 34-38 | byte-identical across two runs, both themes |
| B8 keyboard and labels | Soft | PASS (reported) | "Part B details" |

## 3. Measurements (report only)

On `windows-2025` unless stated. Part B's build includes GPUI's `test-support`
feature (needed by `Window::render_to_image`) and gpui-kit's icon bundle; part
A's numbers (24.4 MB, 619.6 s) are from before that.

| Measure | Value |
| --- | --- |
| Release exe | 26,951,168 bytes (25.7 MiB), LTO, 1 CGU; 29,594,624 bytes without LTO |
| Clean release build, no cache | 643.7 s with LTO; 583.5 s without LTO |
| Release rebuild after touching one file | 267.6 s |
| Debug build, deps cached by the test build / rebuild | 11.9 s / 12.0 s |
| `cargo test`: first run (compiles test profile) / later runs | 236.4 s / 4.6 s and 4.4 s; 36 tests, 3.2 s of test time |
| Crates in `Cargo.lock` | 825 (all targets; a Windows build compiles far fewer) |
| Licenses | as part A: MIT / Apache-2.0 and combinations; MPL-2.0 x3 not in the Windows exe (A8); no GPL-only |
| `#[expect]` | 5 total: 3 from part A, `tokens.rs` (`Rgb::over` rounding cast), `check.rs` (`at`, pixel index cast) |
| Custom widget code | B2: 0 lines (a 12-line `TriState` enum in the app). B3: `middle_truncate`, about 15 lines. B4: about 70 lines wiring `open_dialog` (focus, busy, footer). B5: 0 custom widgets. Spike source in total: about 4,000 lines including tests |
| Table scroll timing, 10,000 rows | 100 jumps of 100 rows over 10,000 rows (test profile = unoptimised, headless, no GPU, `windows-2025`): median 17.1 ms, max 23.6 ms per jump for layout plus frame preparation. A release build is faster; GPUI exposes no frame-timing counter that the spike read, so real smoothness is checklist 41 |

## 4. Gaps found

Each: what GPUI / gpui-kit could not do that today's UI does, the workaround
used or proposed, its cost.

1. **Non-Unicode paths at three OS boundaries (A2, A3):** open/save dialogs
   convert through `String` (including the save dialog's initial directory),
   and Explorer drops skip names that are not valid UTF-16. Fix: own
   `IFileOpenDialog` / `IDropTarget` code with `OsString::from_wide`, about
   1.5 days with Windows testing. Not built.
2. **Save dialog has no file-type filter.** Workaround: `rfd` 0.18 on a worker
   thread (it panics on non-Unicode names; wrap or replace with gap 1's code).
3. **Dialog shadow is fixed in gpui-kit's source** (two 10%-alpha black
   shadows set after the caller's style), so `--shadow-overlay` cannot be
   applied. The scrim does take the token (`theme.overlay`). Fix: build the
   modal on `gpui_base::Dialog` (unstyled host) with our surface, roughly 60
   lines, or ask upstream.
4. **No shadow slot for menus and popovers either:** `theme.shadow` is a
   boolean; `--shadow-overlay` (and its dark variant) has no home. Same fix
   family as 3.
5. **Table rows have one fixed height** (`uniform_list`), so today's
   stack-the-row-into-lines layout below 64rem cannot be done with the table.
   Options: a custom virtual list (`v_virtual_list` supports variable
   heights; rows, header and keyboard focus then become our code, roughly a
   day) or drop the reflow.
6. **No middle truncation:** `text_ellipsis` cuts the end only. The spike's
   `middle_truncate` counts characters, so it is not pixel-exact for
   proportional fonts; exact fitting needs text measurement.
7. **Mouse press does not focus a Button,** and Enter/Space activate a
   Button or Checkbox only from a key-down plus key-up pair (a lone key-down
   does nothing). Consequences: focus returns to the opener after a dialog
   only when the opener was focused (opened from the keyboard); tests must
   send both events.
8. **`Input::h()` is not `Styled::h()`:** it is the multi-line height. A
   single-line input's height comes from its size (2rem at the 16 px rem);
   gpui-component's `Root` makes `Theme::font_size` the window rem, so
   setting it to the 14 px body size shrinks every rem-based control (an error
   of the spike's first run, fixed).
9. **Theme slots with no home** (set on the element instead): the input fill
   (`input_background()` is `--bg` in light, not `--surface`), disabled opacity
   (55%), text colour on a selection, focus ring thickness (only its colour,
   `ring`), `--shadow-overlay`, hover colours that CSS derives with
   `color-mix` (computed in `theme.rs`), `65ch` (CSS `ch` unit: approximated
   as 65 x 0.6 x font size = 546 px), and a font fallback list (the code picks
   the first installed of `Cascadia Mono`, `Consolas`).
10. **Text is rendered with subpixel (ClearType-style) antialiasing,** so
    glyph edges carry colour fringes; per the human's rule this is not a
    failure, and the colour check takes the exact-token pixel of a text region.
11. **Exe grows by the test-support feature** that exposes
    `render_to_image` (GPUI gates it behind `test-support`, which also pulls
    `proptest`-family crates and `wayland`/`x11` features into the build
    graph). Alternative: a separate `shoot` binary or feature built only for
    screenshots in CI.
12. **Linux is headless-only** with the spike's feature set (A9).

## 5. API notes

Real names, for the migration task plans.

- App and windows: `gpui_platform::application().with_assets(impl AssetSource).run(|cx| ..)`;
  `cx.open_window(WindowOptions { window_bounds, window_min_size, titlebar, .. }, |window, cx| ..)`
  returning `WindowHandle<V>`; `gpui_component::init(cx)` first; wrap the root
  view in `gpui_component::Root::new(view, window, cx)` (dialogs, popovers,
  notifications need it); `Root::view()`.
- Theme: `gpui_component::{Theme, ThemeMode, ActiveTheme}`; `Theme::change(mode, None, cx)`
  then `Theme::update(cx, |th| ..)` (a mode change reloads colours, so set
  colours in the second call); colour slots are fields (`background`,
  `foreground`, `border`, `input`, `muted`, `primary`, `sidebar`, `popover`,
  `overlay`, `table_*`, `ring`, ...), `th.colors.list` for the list colour
  (`th.list` is the list settings); `radius`, `font_family`,
  `mono_font_family`, `font_size` (= rem), `shadow: bool`.
- Appearance, live: `window.appearance()`, `window.observe_window_appearance(cb).detach()`,
  `cx.window_appearance()`; `WindowAppearance::{Light, Dark, VibrantLight, VibrantDark}`.
- Capture: `Window::render_to_image() -> RgbaImage` (feature `test-support` on
  `gpui` and `gpui_platform`; the Windows renderer draws the last scene into an
  offscreen texture and reads it back); `window.scale_factor()`,
  `window.viewport_size()`; `image::RgbaImage::save` (png feature).
- Tests: `#[gpui::test]`, `TestAppContext`, `cx.add_window_view`,
  `VisualTestContext::{simulate_input, simulate_keystrokes, simulate_click,
  simulate_event, simulate_resize, debug_bounds, run_until_parked}`,
  `KeyDownEvent` + `KeyUpEvent` for button activation, `window.focus_next(cx)`
  for the first Tab (nothing has focus in a fresh window).
- Inputs: `InputState::new(window, cx)`, `.placeholder`, `set_value`,
  `set_readonly`, `value`, `selected_value`, `cursor`, `focus`; events
  `InputEvent::{Change, PressEnter, Focus, Blur}` via `cx.subscribe_in`;
  element `Input::new(&state)` (`Styled`: fill, border colour, font family).
- Checkbox: `Checkbox::new(id).checked(b).indeterminate(b).accessibility_label(..).on_change(..)`
  (controlled; "Activating it requests `true`").
- Table: `TableState::new(delegate, window, cx)`, `TableDelegate` (`columns_count`,
  `rows_count`, `column` -> `Column::new(key, name).width(px)`, `render_td`),
  `DataTable::new(&state).accessibility_label(..)`, `selected_row()`,
  `visible_range().rows()`, `scroll_to_row`.
- Dialog: `window.open_dialog(cx, |dialog, window, cx| ..)` (the builder runs
  on every render, so closures can read live state), `Dialog::{title, w,
  keyboard(bool), overlay_closable, content, footer}`, `DialogFooter`,
  `window.close_dialog(cx)`, `window.has_active_dialog(cx)`; focus return is
  built in (`previous_focused_handle`).
- Menu and popover: `Button::dropdown_menu(|menu, w, cx| menu.item(PopupMenuItem::element(..).on_click(..)))`
  (Up/Down wrap, Enter picks, Escape closes, focus returns to the trigger);
  `Popover::new(id).trigger(button).content(|state, w, cx| ..)`.
- Keys: `gpui::actions!`, `KeyBinding::new("ctrl-alt-shift-d", Action, Some("Context"))`,
  `.key_context("..")` + `.on_action(cx.listener(..))`.
- Part A names: `prompt_for_paths`, `prompt_for_new_path`, `ExternalPaths`
  with `on_drop`, `on_window_should_close`, `open_with_system`, `reveal_path`.

## 6. Recommendation

**Go-with-conditions** (no Hard item FAILs; two Hard items are PARTIAL and
every Windows-only observation a person must make is still open).

By the plan's rule, all Hard PASS gives go and any Hard FAIL gives no-go.
Here A2, A3 and B4 are PARTIAL, not FAIL, and none of the three is a no-go
criterion in the plan (dialogs and drop work for ordinary names; a modal with
focus control was built). The headline results are strong: the themed screen
matches the tokens exactly (32 of 32 colour samples, 16 per theme; no geometry miss
outside text width), the screenshots render unattended in CI and are byte-identical
across runs, and every widget the plan names has a gpui-component
implementation with 0 to 70 lines of glue.

Conditions, to settle before the migration plans are written:

1. **Decide on non-Unicode paths** (gap 1): accept the loss, or budget about
   1.5 days for own dialog and drop-target code. This is the one place the
   product rule "paths stay `OsString`" is not met by GPUI as shipped.
2. **Decide the overlay shadow** (gaps 3, 4): accept GPUI's fixed shadow,
   or build modals and popovers on `gpui_base`.
3. **Decide the Tar Packager table's narrow layout** (gap 5).
4. **Run `CHECKLIST.md` on Windows 11** (the NEEDS-HUMAN items: live theme
   switch, drop and dialogs, close, local `--shoot` including a display scale
   other than 100%, how 10,000 rows feel). Any FAIL there reopens the call.

# Part A details

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
  bcryptprimitives, gdiplus. All are Windows system DLLs / API sets. The workflow's "every imported DLL is
  a system DLL" step only prints a warning for a non-System32 name and never
  fails; the PASS rests on the log showing no such warning, not on the step
  going red. Notes:
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
  frame". What that proves: the D3D11 device was created (WARP, the software
  adapter behind "Microsoft Hyper-V Video", no GPU) and the first `draw()` ran
  inside `open_window`, so the window drew its first frame; it does not prove
  the frame was presented to a visible desktop. A GUI process did get a window
  on GitHub's Windows runner. Part B's PNG capture (B7) settles the rest.
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
- GPUI's save dialog has two more lossy `String` boundaries: the initial
  directory goes through `full_path.display().to_string()` (gpui-pre-windows
  `platform.rs` ~1643-1644, lossy: a non-Unicode directory name is altered
  with U+FFFD before it reaches the dialog), and the chosen path through
  `pwstr.to_string()?` (`platform.rs` ~1679, an error on invalid UTF-16, like
  the open dialog). So every path crossing GPUI's dialogs (open, save,
  initial directory) is converted through `String`.
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
  the test-profile compile), runs 2 and 3 2.1 s and 1.7 s. 13 tests on Windows
  (including the Windows-only `dropped_lone_surrogate_name_survives_routing`)
  and 12 on `ubuntu-latest` (0.62 s of test time), same results otherwise.
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

On `windows-2025` (rustc 1.100.0, GitHub-hosted runner; core count not recorded):

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
  crate is GPL/AGPL/LGPL only. MPL-2.0 is file-level copyleft; the three
  MPL-2.0 crates are `cbindgen`, `dwrote` and `option-ext`. None ships in the
  Windows exe: `cbindgen` is a build-dependency of `gpui-pre-apple` and
  `dwrote` comes from `zed-font-kit` under `gpui-pre-apple`, both macOS/iOS
  only (`cargo tree --target x86_64-pc-windows-msvc -i dwrote` is empty);
  `option-ext` comes through `dirs` -> `shellexpand` -> `rust-embed-impl`, a
  proc-macro that runs on the build machine only. Zero unknown licenses.
- Toolchain: GPUI crates declare no `rust-version`, all are edition 2024
  (Rust 1.85+); builds with the repo's `rust-version = "1.90"` floor
  and with 1.100. The spike's own `rust-version` is 1.90.

## A9 - Linux build: PASS (build and headless tests; no X11/Wayland backend compiled)

`ubuntu-latest` (a non-blocking job): `cargo fmt --check`, clippy,
`cargo build` and `cargo test` all pass **with no apt packages installed**
(the runner's default image only). The Linux `gpui-pre-linux` backend compiles
without webkit/gtk dev packages. The 12 tests run on
the headless test platform. `gpui-pre-platform` pulls `gpui_linux` with
`default-features = false` and the spike enables neither `x11` nor `wayland`,
so the Linux build is headless-only and cannot open a window. A real Linux
window needs those two features and probably the xkbcommon, wayland and X11
development packages on the build machine. Not tested: running a Linux window.

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

# Part B details

Effort: about one long session, far under the 3-day box. Nothing was patched
in GPUI or gpui-kit. CI iterations: two runs of the screenshot job to get the
check green (see B6 for what the first run found).

## B1 - Text input: PASS headless

Test `input_paste_and_submit`: typing (`simulate_input`), Ctrl+A selects all
(`selected_value`), Home and Right move the caret (`cursor`), Ctrl+V pastes
the clipboard at the caret, Enter raises `InputEvent::PressEnter` which the
model records, a read-only input ignores typing and still selects all. Not
covered by a headless test (a person must look, checklist 39): placeholder
colour (`muted_foreground`, mapped to `--text-muted`), the `--danger` border of
the invalid state (`Input` is `Styled`, so `.border_color(danger)` overrides
the frame; the focus ring still shows over it), the mono font (`.font_family`).
Min height 2rem is the input's own size at a 16 px rem (gap 8).

## B2 - Checkbox with "not set": PASS, no custom widget

gpui-component's `Checkbox` has `.indeterminate(true)` (a dash, reported as
mixed to assistive technology) and is controlled: activation requests `true`
and the owner decides. The tri-state is the app's 12-line `TriState` enum
(unchecked, checked, not set) cycling in `on_change`, with the word "Not set"
beside. Medium indicator size is 1rem (source); checked colour is
`primary` = `--accent`. Tests: clicks cycle through the three states;
Tab reaches the box as the 4th tab stop and Space (key down + key up) cycles it.

## B3 - Virtual table: PASS headless

`DataTable` over `TableState<Rows>` with 10,000 rows is virtualised
(`uniform_list` inside) with custom cells: icon plus word in a status colour
(`success`, `warning`, `danger` = `--ok`, `--warn`, `--danger`), mono cells,
fixed column widths in rem (Status 7.25, Mode 8.25, Owner 5), header outside
the scrolling body, 1 px row rules (`table_row_border`), hover
(`table_hover` = `--surface-sunken`). Test `table_keyboard_focus_moves_and_scrolls`:
60 presses of Down select row 59 and `visible_range()` contains it; Up moves
back one. `table_scroll_steps_are_cheap` measures scroll steps: see section 3
(headless, unoptimised: median 17.1 ms, max 23.6 ms per 100-row jump; how it feels is checklist 41).
Variable row height: **no**, one fixed height (gap 5). Middle truncation: not
native, `middle_truncate` keeps the drive and the whole file name (gap 6).

## B4 - Modal confirmation: PARTIAL

Passing, by test: `window.open_dialog` over a scrim (`theme.overlay` =
`--scrim`, set in `theme.rs`); title, description, a scrolling region (24
lines), Cancel and Run in normal (primary) and danger variants (the danger
variant uses `--danger` fill with `--accent-text`, since `tokens.css` has no
text-on-danger token and today's UI has no danger button). Focus starts on
Cancel (the spike focuses it explicitly after opening: the dialog's own focus
node is what gets it otherwise). Escape cancels. Focus returns to the opener
when the opener had focus (opened with Enter). Busy: Run sets a flag, the
dialog stays open, `keyboard(false)` makes Escape inert and Cancel is
disabled and ignores clicks, until `finish_confirm` closes it (the builder
closure runs on every render, so it sees the live flag). **Not met:** the
`--shadow-overlay` token (gap 3): the dialog's two black shadows are fixed in
gpui-kit and applied after the caller's style.

## B5 - Menu and popover: PASS behaviour

"Recent": a `Button` with our chevron icon and `dropdown_menu`; items are
`PopupMenuItem::element` (name plus a muted second line). By test, driven only
by keys: Tab reaches the button (8th tab stop), Enter opens, Down moves, a
third Down on two items wraps to the first, Enter picks it, Escape closes, and
after both pick and Escape the trigger has focus again (Enter reopens).
"Shortcuts": `Popover` under its button, anchored below it, closed by Escape.
Surface colours come from the theme (`popover` = `--surface`, `border`,
`radius`); the shadow token has no slot (gap 4). Looks: checklist 43.

## B6 - Themed screen: PASS in CI

`tokens.rs` transcribes `tokens.css` (test `tokens_match_tokens_css` parses the
CSS and compares every colour, spacing, radius, type size and both fonts;
light and dark). `theme.rs` maps it onto 84 gpui-component theme fields.
`pipeline_screen.rs` rebuilds sidebar plus Pipeline view from `app.css`,
`controls.css`, `pipeline.css` and `PipelineView.tsx`, with the SVG paths of
`icons.tsx` in `assets/icons`. The screen records its own boxes at paint time
(`measure.rs`), so geometry is checked on a release build. The check
(`fm-spike --verify`, `check.rs`) runs on the B7 PNGs in CI.

How it is judged (all of it visible in `check.rs`): colours at 16 named
points per theme must equal the token value (after the stated opacity) within
+-1 per channel; points hang off the recorded boxes, so a different text width
cannot put a point on glyphs. Text colours take the pixel of the text region
closest to the token colour (antialiasing and the subpixel colour fringes
never reach it; full-coverage pixels do). 1 px lines (input border,
secondary-button border, row rule) take the best of the three pixel rows
around the expected one, because the lower boxes sit one pixel lower than in
the reference (box deltas of +1, within the 2 px tolerance; the input's own top
edge has delta 0): a sample at the reference's exact row would test position,
which the geometry check already does. The strict per-row result was not
recorded in the final run. Boxes: x, y, w, h within 2 px of the reference image's, measured by
pixel inspection of `docs/manual/images/pipeline-chosen.png`; a miss on a
value that follows the text width is printed as `TEXT` and not counted
(the human's rule: text widths may differ). The checker itself is tested
against the reference image (it passes all colour points; the dark tokens fail
on it), and the reference box constants are asserted against the image.

First CI run (commit `b79fdd0`) printed 28 failing checks. Causes, all mine and
fixed: (1) `Theme::font_size` set to the 14 px body size became the window's
rem, so every rem-based control and `text_sm` was 12.25 px (input height 28
instead of 32; fix: keep it 16 px, gap 8); (2) the disabled-button style was
not applied (fix: `.btn:disabled` transcribed as bg, text, border and 55%
opacity on the element); (3) sample points fixed at reference coordinates
landed on glyphs when text was narrower (fix: points relative to boxes); (4)
text-ink samples used the farthest pixel, which is a colour fringe under
subpixel antialiasing (fix: closest pixel).

Final CI run (run 44841007688, commit `ebe9d0e`), `check-result.txt`, quoted:

```
PASS colour window ground            at (600,650): want #f7f7f8 got #f7f7f8
PASS colour sidebar fill             at (100,650): want #eef0f2 got #eef0f2
PASS colour sidebar border           at (191,650): want #d9dce1 got #d9dce1
PASS colour current nav fill         at (160,30): want #ffffff got #ffffff
PASS colour accent dot               at (24,31): want #2457c5 got #2457c5
PASS colour input fill               at (843,150): want #ffffff got #ffffff
PASS colour input border             at (549,133..135) line, best of 3 rows: want #d9dce1 got #d9dce1
PASS colour primary button fill      at (221,225): want #2457c5 got #2457c5
PASS colour primary label            at (230,220) closest ink pixel of region: want #ffffff got #ffffff
PASS colour secondary button fill    at (440,225): want #ffffff got #ffffff
PASS colour secondary button border  at (474,209..211) line, best of 3 rows: want #d9dce1 got #d9dce1
PASS colour disabled button (55%)    at (920,368): want #f2f3f5 got #f2f3f5
PASS colour row rule                 at (500,322..324) line, best of 3 rows: want #d9dce1 got #d9dce1
PASS colour step number fill         at (219,290): want #eef0f2 got #eef0f2
PASS colour muted text               at (226,180) closest ink pixel of region: want #5b616b got #5b616b
PASS colour text                     at (259,23) closest ink pixel of region: want #1b1d21 got #1b1d21
(dark: the same 16 points, all PASS: ground #16171a, sidebar #121316, border #33363d,
 nav #1e2024, dot #7aa2ff, input #1e2024, primary #7aa2ff, label #0d1530,
 secondary #1e2024, disabled #141518, step number #121316, muted text #a3a8b1, text #e8e9ec)
Boxes with a non-zero delta (all others 0.0):
 PASS +1.0: browse.h, primary.y, secondary.y, run1.x, run1.y, run1.w (-1.0),
            run2.y, step1.y .. step4.y
 TEXT (text width): input.w +6, browse.x +6, browse.w -6, primary.w -18,
            secondary.x -18, secondary.w -3, run2.x +3, run2.w -3
0 failing check(s)
```

Reference vs GPUI, side by side (reference | light | dark): artifact
`fm-spike-shots`, `run1/side-by-side.png`. By eye the layout, colours, the
accent dot, the numbered circles, the disabled buttons and the status icons
match; text is narrower (Segoe UI against the reference's Inter), so buttons
are 3 to 18 px narrower. Dark has no reference image, so its judgment is the
token colours and the same boxes.

Honest limits: the check proves the tokens reach the pixels and the boxes
match; it does not prove that hover, focus, pressed and busy states look right
(not rendered), nor the live switch between themes (checklist 32). Slots that
could not be set are gap 9.

## B7 - Screenshot to PNG, unattended: PASS in CI, method 1

`fm-spike --shoot <dir> [--dark]` opens the B6 screen with fixture data (no
filesystem), waits 1.5 s for the first frame to settle, and writes
`<dir>/pipeline-chosen[-dark].png`, `*.boxes.txt` and `*.shoot-result.txt`,
then exits (exit code 0 on success).

- **Method 1 worked: GPUI's `Window::render_to_image`.** Found by searching
  the pinned source: it is behind the `test-support` feature, and the Windows
  backend implements it (offscreen D3D11 render of the last scene, staging
  texture read-back, BGRA to RGBA). It captures GPUI's own scene, not the
  screen. The spike turns the feature on with its default `shoot` feature.
  Methods 2 (Win32 `PrintWindow` and friends) and 3 (screen capture) were not
  needed and not tried.
- On `windows-2025` (no GPU, WARP) the PNGs are 1000x700 device px at scale
  factor 1; two consecutive runs are **byte-identical** (SHA-256 equal) for
  both themes (`determinism.txt`). Uploaded as artifacts, with the
  side-by-side image.
- Fonts on the runner: Segoe UI, Cascadia Mono and Consolas are all
  installed (`shoot-result.txt`); used: Segoe UI, Cascadia Mono. The human's
  machine: NEEDS-HUMAN 37. Embedding the fonts would make output identical
  across machines whatever is installed; not tried (a font file in the repo and
  `AssetSource` / text-system registration, an estimated half day, and a
  licence check for Cascadia Mono, which is OFL).
- Not settled in CI: the human's own machine, and a display scale other than
  100%. From the source the image is the window's device pixels, so at 125%
  the PNG is larger than 1000x700 (the layout is in logical px); the check
  reports "image size ... display scale is not 1?" if so (checklist 34-36).
  A hidden or off-screen window was not needed on the runner.

## B8 - Keyboard and labels, as found

| Widget | Tab | Activation | Label |
| --- | --- | --- | --- |
| Input | yes (tab stop) | typing; Enter raises `PressEnter` | placeholder is the accessible-name fallback; `aria_label` available |
| Checkbox | yes, 4th stop | Space (key down + key up), click | visible text only if added; `accessibility_label` set in the spike |
| Button | yes | Enter, Space (pair), click; mouse press does not focus it | its label; `tooltip` available |
| Table | yes (one stop) | Up/Down select and scroll into view | `accessibility_label` on `DataTable` |
| Dialog | focus trap in the dialog | Escape; Cancel and Run buttons | title; close icon labelled "Close" |
| Menu / popover | trigger is a Button | Down/Up/Enter/Escape in the menu; Escape in the popover | item text |

The developer-mode chord Ctrl+Alt+Shift+D binds with `KeyBinding` to an action
in a key context and toggles a flag (test `developer_mode_chord_toggles_a_flag`).

## Files (part B)

`spike/gpui/`: `src/tokens.rs`, `theme.rs`, `assets.rs`, `measure.rs`,
`pipeline_screen.rs`, `widgets.rs`, `shoot.rs`, `check.rs`, `demo.rs`,
`main.rs` (new flags `--shoot`, `--verify`, `--diff`, `--pipeline`,
`--widgets`), `assets/icons/*.svg`, `Cargo.toml` (feature `shoot`, deps
`image`, `gpui-kit-assets`), `CHECKLIST.md` (steps 31-44),
`.github/workflows/spike-gpui.yml` (job `shoot`; `cargo fmt --check` in the
windows job). Nothing else changed.
