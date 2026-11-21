# M3c — Native Windows file dialogs with file-type filters, behind `Platform`

Status: approved (2026-11-21)
Project: gpui-migration   Depends on: M3b (landed)

## Goal

Replace the GPUI file dialogs behind `Platform::pick` and
`Platform::pick_save` with our own thin wrapper over the Windows common item
dialog (`IFileOpenDialog` / `IFileSaveDialog`, through the `windows`
crate), so open and save show the native Windows file picker **with
file-type filters, a default extension and an initial folder, as the Tauri
app does today**, and return paths exactly, as `OsString`.

## Context

FileManager is moving from Tauri + React to GPUI. M3b built
`apps/filemanager/src/platform/`: the `Platform` trait (`pick`,
`pick_save`, `open_with_system`, `reveal`, `copy_text`), `GpuiPlatform`
(whose dialogs call GPUI's `prompt_for_paths` / `prompt_for_new_path`),
`FakePlatform` (scripted answers and a call log), `PickError`, and
`path_loss.rs` (the drop notice). Read them first.

Why this milestone exists. The human decided that open and save must show
the Windows file picker with filters, as today (for example, the Tar
Packager's open dialog shows manifests). GPUI's Windows dialogs are the
common item dialog already, but GPUI exposes no filter, no default
extension, and converts every path through `String`: a name that is not
valid Unicode makes the whole selection fail, and the save dialog's
starting folder is altered. Our wrapper fixes both. **Do not use `rfd`**
(it panics on such names). Explorer drop is unchanged: it stays GPUI's, and
its lost names stay reported by `path_loss.rs`.

Today's dialogs, for reference (`apps/desktop/src/lib/tauri.ts` and the
views, while they exist): open one file with filters; open one folder;
save with a suggested path and filters. Windows matches only the last
extension of a filter (`zst` for `.tar.zst`); the backend normalises the
full extension of an archive name itself.

### What to build

**Trait shape (already in `src/platform/mod.rs` from M3b; keep it).** If a
detail must change for COM, change `FakePlatform` and every caller with it:

```rust
pub struct FileFilter { pub name: String, pub patterns: Vec<String> }   // patterns like "*.toml"
pub struct OpenSpec {
    pub kind: PickKind,                    // File | Folder
    pub filters: Vec<FileFilter>,          // ignored for Folder; empty = all files
    pub start_dir: Option<PathBuf>,
    pub title: Option<String>,
}
pub struct SaveSpec {
    pub filters: Vec<FileFilter>,
    pub default_extension: Option<String>, // without the dot, e.g. "toml"; Windows appends it when the user types none
    pub start_dir: Option<PathBuf>,
    pub suggested_name: Option<OsString>,
    pub title: Option<String>,
}
pub enum PickError { Failed { detail: String } }   // the dialog could not be shown or returned an error
fn pick(&self, spec: OpenSpec, cx: &mut App) -> Task<Result<Option<PathBuf>, PickError>>;
fn pick_save(&self, spec: SaveSpec, cx: &mut App) -> Task<Result<Option<PathBuf>, PickError>>;
```

`FakePlatform` records the spec of every call (tests assert the filters
and start folder a model passes) and returns scripted answers.

**`src/platform/win_dialog.rs` (`#[cfg(windows)]`).** About 150 lines of
COM over the `windows` crate (features for `Win32_UI_Shell`,
`Win32_UI_Shell_Common`, `Win32_System_Com`, `Win32_Foundation`; pin the
version in `[workspace.dependencies]`, preferably the one already in the
lock through GPUI):

- Each dialog runs on a **dedicated thread** initialised with
  `CoInitializeEx(COINIT_APARTMENTTHREADED)` and uninitialised at the end,
  so the GPUI UI thread is never blocked; the result comes back through a
  oneshot/`async_channel` as a GPUI `Task`.
- **Owner window:** the main window's `HWND`, captured once when the window
  opens (through `raw-window-handle` from the GPUI window; carry it across
  threads as an integer) and passed to `Show`, so the dialog is modal to
  FileManager as today.
- **Open:** `CoCreateInstance(FileOpenDialog)`; options from `GetOptions`
  plus `FOS_FORCEFILESYSTEM`, `FOS_PATHMUSTEXIST`, and `FOS_FILEMUSTEXIST`
  for files or `FOS_PICKFOLDERS` for folders; `SetFileTypes` with one
  `COMDLG_FILTERSPEC` per filter (name, patterns joined with `;`), then
  `SetFileTypeIndex(1)`; `SetFolder` from `SHCreateItemFromParsingName` of
  the start folder **as wide chars from `OsStr::encode_wide`** (if the folder
  no longer exists, skip `SetFolder` and let Windows choose); `SetTitle`.
- **Save:** `CoCreateInstance(FileSaveDialog)`, keep its default options
  (including Windows' own "replace?" prompt, as today's Tauri dialog does),
  plus `FOS_FORCEFILESYSTEM`; the same filters; `SetDefaultExtension`;
  `SetFolder`; `SetFileName` from the suggested name's wide chars.
- **Result:** `ERROR_CANCELLED` from `Show` → `Ok(None)`; otherwise
  `GetResult` → `IShellItem::GetDisplayName(SIGDN_FILESYSPATH)` → read the
  `PWSTR` up to its NUL, `OsString::from_wide`, `CoTaskMemFree`. **Never**
  convert through `String`. Any other `HRESULT` → `PickError::Failed`
  with the error text.
- Strings passed in (filter names, patterns, title) are built as
  NUL-terminated `Vec<u16>` kept alive for the call.

**`GpuiPlatform`** uses `win_dialog` on Windows. On other targets (Linux is
headless-only; no window ever opens there) it keeps GPUI's prompts so the
code compiles; document that it is unused in practice.

**Copy.** `PickError`'s copy in `src/app/text.rs` ("The file dialog could
not be shown. See Details.") stays; the drop notice is unchanged.

**Manual check (`apps/filemanager/examples/dialog_check.rs`).** A cargo
example (never part of the exe) that opens, in turn: an open dialog with a
"Manifest (*.toml)" filter starting in a given folder; a folder picker; a
save dialog with "Gzip archive (.tar.gz)" (`*.gz`), default extension `gz`
and suggested name `gateway.tar.gz`; and writes each result with
`{:?}` (so lone surrogates show) to `dialog-check.txt` beside it. Add its
steps to `docs/gpui-dialogs-check.md`: create a folder and a file whose
names contain a lone surrogate (PowerShell:
`New-Item -Path ("C:\check\lone-" + [char]0xD800 + ".toml") -ItemType File`),
start the dialogs in that folder, pick that file, confirm the filter list,
the default extension, the owner modality, and that the result shows
`\u{d800}`. The human runs it on Windows 11; record the result in the PR
when available (it does not block the merge; a FAIL reopens this
milestone).

### Rules that bind this task

- Paths are the platform's path type end to end through the dialogs: no
  `String` conversion of a path anywhere in `win_dialog.rs`.
- Every call stays behind `Platform`; models and tests never see COM; tests
  use `FakePlatform`.
- No blocking on the UI thread.
- Not `rfd`. No new DLL in the exe: the dumpbin check (forbidden
  `vcruntime*`, `msvcp*`, `api-ms-win-crt-*`, `zstd*`, `liblzma*`,
  `sqlite3*`, `WebView2Loader.dll`) must still pass; `comdlg32`/`shell32`/`ole32`
  are system DLLs and fine.
- `unsafe` is confined to `win_dialog.rs`, each block with a `// SAFETY:`
  comment; clippy pedantic `-D warnings` (the windows CI job lints
  `#[cfg(windows)]` code); scoped `#[expect]` only.
- The `fm-*` crates are not touched.

## Files

Create: `apps/filemanager/src/platform/win_dialog.rs`,
`apps/filemanager/examples/dialog_check.rs`, `docs/gpui-dialogs-check.md`.

Edit: `Cargo.toml`, `Cargo.lock`, `apps/filemanager/Cargo.toml`,
`apps/filemanager/src/platform/{mod.rs,gpui.rs,fake.rs}`,
`apps/filemanager/src/lib.rs` (capture
the owner window), `CLAUDE.md` (Layout line for `platform/`).

Do not touch: `path_loss.rs` (drop stays as it is), `crates/`, `apps/desktop/`, `docs/plans/`.

## Acceptance criteria

- `Platform::pick`/`pick_save` take `OpenSpec`/`SaveSpec` as above;
  `FakePlatform` logs them.
- On Windows the dialogs are the common item dialog with the given filters,
  default extension, start folder and owner; paths return as
  `OsString`-exact `PathBuf`s.
- No `rfd` in `Cargo.lock`; dumpbin and self-check pass; fmt, clippy pedantic
  (both jobs) and `cargo test --workspace` pass.
- `docs/gpui-dialogs-check.md` exists and the example builds
  (`cargo build -p filemanager-gpui --example dialog_check`).

## Tests proving completion

- `platform::win_dialog::tests` (`#[cfg(windows)]`, no dialog shown):
  `filter_spec_is_nul_terminated_wide`, `patterns_joined_with_semicolons`,
  `pwstr_to_os_string_keeps_lone_surrogate` (build a wide buffer with
  `0xD800`, convert, compare with `OsString::from_wide`),
  `default_extension_has_no_dot`.
- `platform::fake::tests::records_specs` and `answers_in_order_and_logs_calls`.
- The manual check above.

## Out of scope

- Explorer drop (unchanged; GPUI's, with the path-loss notice).
- Multi-select (no tool uses it).
- Tool-specific filters (each tool milestone passes its own).

## Risks

- COM threading: the dialog must run in an STA thread; the owner HWND
  belongs to the UI thread, which is allowed for an owner.
- CI cannot show a dialog; correctness of the live dialog rests on the
  manual check.
