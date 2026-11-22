//! Native Windows file dialogs: a thin wrapper over the common item dialog
//! (`IFileOpenDialog` / `IFileSaveDialog`).
//!
//! GPUI's own dialogs expose no file-type filter and no default extension, and
//! convert every path through `String`. This wrapper passes filters, a default
//! extension and a start folder, and returns paths as exact `OsString`s, so a
//! name that is not valid Unicode survives. Each dialog runs on its own
//! apartment-threaded thread, so the UI thread never blocks. All `unsafe` in
//! the app lives in this file.

use std::ffi::{c_void, OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicIsize, Ordering};

use gpui::{App, Task};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, IBindCtx,
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FileOpenDialog, FileSaveDialog, IFileDialog, IFileOpenDialog, IFileSaveDialog, IShellItem,
    SHCreateItemFromParsingName, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST,
    FOS_PICKFOLDERS, SIGDN_FILESYSPATH,
};

use super::{FileFilter, OpenSpec, PickError, PickKind, PickResult, SaveSpec};

/// The main window's `HWND`, as an integer so it can cross threads. Zero means
/// no owner: the dialog is then not modal to the app.
static OWNER: AtomicIsize = AtomicIsize::new(0);

/// Remembers `window`'s `HWND` as the owner of every dialog shown later.
pub fn capture_owner(window: &gpui::Window) {
    if let Ok(handle) = window.window_handle() {
        if let RawWindowHandle::Win32(h) = handle.as_raw() {
            OWNER.store(h.hwnd.get(), Ordering::Relaxed);
        }
    }
}

fn owner_hwnd() -> Option<HWND> {
    let raw = OWNER.load(Ordering::Relaxed);
    (raw != 0).then(|| HWND(std::ptr::with_exposed_provenance_mut(raw.cast_unsigned())))
}

/// `s` as a NUL-terminated UTF-16 buffer (lone surrogates are kept).
fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

/// The patterns of a filter, joined as the dialog wants them: `*.a;*.b`.
fn join_patterns(patterns: &[String]) -> String {
    patterns.join(";")
}

/// A filter as the two NUL-terminated buffers `COMDLG_FILTERSPEC` points at.
struct WideFilter {
    name: Vec<u16>,
    spec: Vec<u16>,
}

fn wide_filters(filters: &[FileFilter]) -> Vec<WideFilter> {
    filters
        .iter()
        .map(|f| WideFilter {
            name: wide(OsStr::new(&f.name)),
            spec: wide(OsStr::new(&join_patterns(&f.patterns))),
        })
        .collect()
}

/// The default extension as the dialog wants it: no leading dot.
fn default_extension(ext: &str) -> Vec<u16> {
    wide(OsStr::new(ext.trim_start_matches('.')))
}

/// Reads a NUL-terminated wide string into an `OsString`, keeping every unit.
///
/// # Safety
/// `p` must point at a readable, NUL-terminated buffer of `u16`.
unsafe fn pwstr_to_os_string(p: *const u16) -> OsString {
    let mut len = 0;
    // SAFETY: the caller guarantees the buffer is NUL-terminated, so every
    // read up to and including the terminator is in bounds.
    while unsafe { *p.add(len) } != 0 {
        len += 1;
    }
    // SAFETY: `len` units were just read from `p`.
    OsString::from_wide(unsafe { std::slice::from_raw_parts(p, len) })
}

fn failed(e: impl std::fmt::Display) -> PickError {
    PickError::Failed {
        detail: e.to_string(),
    }
}

/// Initialises COM for the current thread; uninitialises on drop.
struct ComApartment;

impl ComApartment {
    fn enter() -> Result<Self, PickError> {
        // SAFETY: no reserved argument; paired with `CoUninitialize` in `Drop`
        // on the same thread. Called once per dialog thread.
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
            .ok()
            .map_err(failed)?;
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        // SAFETY: balances the successful `CoInitializeEx` in `enter`.
        unsafe { CoUninitialize() };
    }
}

fn set_folder(dialog: &IFileDialog, dir: &std::path::Path) {
    let dir = wide(dir.as_os_str());
    // SAFETY: `dir` is NUL-terminated and outlives the call.
    let item: windows::core::Result<IShellItem> =
        unsafe { SHCreateItemFromParsingName(PCWSTR(dir.as_ptr()), None::<&IBindCtx>) };
    // A folder that no longer exists is skipped: Windows picks one.
    if let Ok(item) = item {
        // SAFETY: `item` is a live shell item.
        let _ = unsafe { dialog.SetFolder(&item) };
    }
}

fn set_filters(dialog: &IFileDialog, filters: &[FileFilter]) -> windows::core::Result<()> {
    if filters.is_empty() {
        return Ok(());
    }
    let wides = wide_filters(filters);
    let specs: Vec<COMDLG_FILTERSPEC> = wides
        .iter()
        .map(|f| COMDLG_FILTERSPEC {
            pszName: PCWSTR(f.name.as_ptr()),
            pszSpec: PCWSTR(f.spec.as_ptr()),
        })
        .collect();
    // SAFETY: every pointer in `specs` points into `wides`, which outlives
    // both calls; the dialog copies the strings.
    unsafe {
        dialog.SetFileTypes(&specs)?;
        dialog.SetFileTypeIndex(1)
    }
}

fn set_title(dialog: &IFileDialog, title: Option<&str>) {
    if let Some(title) = title {
        let title = wide(OsStr::new(title));
        // SAFETY: `title` is NUL-terminated and outlives the call.
        let _ = unsafe { dialog.SetTitle(PCWSTR(title.as_ptr())) };
    }
}

/// Shows `dialog` and reads the chosen path; `Ok(None)` when cancelled.
fn show_and_read(dialog: &IFileDialog) -> PickResult {
    // SAFETY: `dialog` is a live dialog; the owner is a window handle that
    // belongs to another thread, which is allowed for an owner.
    if let Err(e) = unsafe { dialog.Show(owner_hwnd()) } {
        return if e.code() == windows::core::HRESULT::from_win32(ERROR_CANCELLED.0) {
            Ok(None)
        } else {
            Err(failed(e))
        };
    }
    // SAFETY: `Show` succeeded, so a result exists.
    let item = unsafe { dialog.GetResult() }.map_err(failed)?;
    // SAFETY: `item` is live; on success the returned buffer is a NUL-terminated
    // wide string allocated by COM, which is freed right after the copy.
    unsafe {
        let name = item.GetDisplayName(SIGDN_FILESYSPATH).map_err(failed)?;
        let path = pwstr_to_os_string(name.0);
        CoTaskMemFree(Some(name.0.cast::<c_void>().cast_const()));
        Ok(Some(PathBuf::from(path)))
    }
}

/// Shows an open dialog and blocks until it closes. Call from a thread of
/// your own: the dialog must not run on the UI thread.
///
/// # Errors
/// [`PickError::Failed`] if the dialog could not be shown or returned an error.
pub fn run_open(spec: &OpenSpec) -> PickResult {
    let _com = ComApartment::enter()?;
    // SAFETY: COM is initialised on this thread.
    let open: IFileOpenDialog =
        unsafe { CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER) }.map_err(failed)?;
    let dialog: &IFileDialog = &open;
    let extra = match spec.kind {
        PickKind::File => FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | FOS_FILEMUSTEXIST,
        PickKind::Folder => FOS_FORCEFILESYSTEM | FOS_PICKFOLDERS,
    };
    // SAFETY: `dialog` is a live dialog.
    let options = unsafe { dialog.GetOptions() }.map_err(failed)?;
    // SAFETY: as above.
    unsafe { dialog.SetOptions(options | extra) }.map_err(failed)?;
    if spec.kind == PickKind::File {
        set_filters(dialog, &spec.filters).map_err(failed)?;
    }
    if let Some(dir) = &spec.start_dir {
        set_folder(dialog, dir);
    }
    set_title(dialog, spec.title.as_deref());
    show_and_read(dialog)
}

/// Shows a save dialog and blocks until it closes. Windows asks "replace?"
/// itself when the file exists.
///
/// # Errors
/// [`PickError::Failed`] if the dialog could not be shown or returned an error.
pub fn run_save(spec: &SaveSpec) -> PickResult {
    let _com = ComApartment::enter()?;
    // SAFETY: COM is initialised on this thread.
    let save: IFileSaveDialog =
        unsafe { CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER) }.map_err(failed)?;
    let dialog: &IFileDialog = &save;
    // SAFETY: `dialog` is a live dialog.
    let options = unsafe { dialog.GetOptions() }.map_err(failed)?;
    // SAFETY: as above.
    unsafe { dialog.SetOptions(options | FOS_FORCEFILESYSTEM) }.map_err(failed)?;
    set_filters(dialog, &spec.filters).map_err(failed)?;
    if let Some(ext) = &spec.default_extension {
        let ext = default_extension(ext);
        // SAFETY: `ext` is NUL-terminated and outlives the call.
        unsafe { dialog.SetDefaultExtension(PCWSTR(ext.as_ptr())) }.map_err(failed)?;
    }
    if let Some(dir) = &spec.start_dir {
        set_folder(dialog, dir);
    }
    if let Some(name) = &spec.suggested_name {
        let name = wide(name);
        // SAFETY: `name` is NUL-terminated and outlives the call.
        unsafe { dialog.SetFileName(PCWSTR(name.as_ptr())) }.map_err(failed)?;
    }
    set_title(dialog, spec.title.as_deref());
    show_and_read(dialog)
}

/// Runs `dialog` on a dedicated thread; the task resolves with its result.
fn on_dialog_thread(
    name: &str,
    dialog: impl FnOnce() -> PickResult + Send + 'static,
    cx: &App,
) -> Task<PickResult> {
    let (tx, rx) = async_channel::bounded(1);
    let spawned = std::thread::Builder::new()
        .name(name.to_string())
        .spawn(move || {
            let _ = tx.send_blocking(dialog());
        });
    if let Err(e) = spawned {
        return Task::ready(Err(failed(e)));
    }
    cx.foreground_executor().spawn(async move {
        rx.recv()
            .await
            .unwrap_or_else(|_| Err(failed("the dialog thread stopped")))
    })
}

/// Shows an open dialog without blocking the caller.
#[must_use]
pub fn pick(spec: OpenSpec, cx: &App) -> Task<PickResult> {
    on_dialog_thread("open-dialog", move || run_open(&spec), cx)
}

/// Shows a save dialog without blocking the caller.
#[must_use]
pub fn pick_save(spec: SaveSpec, cx: &App) -> Task<PickResult> {
    on_dialog_thread("save-dialog", move || run_save(&spec), cx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filter(name: &str, patterns: &[&str]) -> FileFilter {
        FileFilter {
            name: name.into(),
            patterns: patterns.iter().map(|p| (*p).to_string()).collect(),
        }
    }

    #[test]
    fn filter_spec_is_nul_terminated_wide() {
        let wides = wide_filters(&[filter("Manifest", &["*.toml"])]);
        let expect_name: Vec<u16> = "Manifest".encode_utf16().chain([0]).collect();
        let expect_spec: Vec<u16> = "*.toml".encode_utf16().chain([0]).collect();
        assert_eq!(wides[0].name, expect_name);
        assert_eq!(wides[0].spec, expect_spec);
        assert_eq!(wides[0].name.iter().filter(|u| **u == 0).count(), 1);
    }

    #[test]
    fn patterns_joined_with_semicolons() {
        assert_eq!(
            join_patterns(&["*.tar.gz".into(), "*.tgz".into()]),
            "*.tar.gz;*.tgz"
        );
        assert_eq!(join_patterns(&["*.toml".into()]), "*.toml");
    }

    #[test]
    fn pwstr_to_os_string_keeps_lone_surrogate() {
        let buffer: Vec<u16> = vec![u16::from(b'a'), 0xD800, u16::from(b'.'), 0, 0x41];
        // SAFETY: `buffer` holds a NUL-terminated string.
        let got = unsafe { pwstr_to_os_string(buffer.as_ptr()) };
        assert_eq!(got, OsString::from_wide(&buffer[..3]));
        assert!(got.to_str().is_none());
    }

    #[test]
    fn default_extension_has_no_dot() {
        let expect: Vec<u16> = "gz".encode_utf16().chain([0]).collect();
        assert_eq!(default_extension("gz"), expect);
        assert_eq!(default_extension(".gz"), expect);
    }
}
