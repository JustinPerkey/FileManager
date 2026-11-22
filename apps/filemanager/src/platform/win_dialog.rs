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
use std::sync::atomic::{AtomicIsize, AtomicU64, Ordering};

use raw_window_handle::RawWindowHandle;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, IBindCtx,
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FileOpenDialog, FileSaveDialog, IFileDialog, IFileOpenDialog, IFileSaveDialog, IShellItem,
    SHCreateItemFromParsingName, FILEOPENDIALOGOPTIONS, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM,
    FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, SIGDN_FILESYSPATH,
};

use super::{FileFilter, OpenSpec, PickError, PickKind, PickResult, SaveSpec};

/// The main window's `HWND`, as an integer so it can cross threads. Zero means
/// no owner: the dialog is then not modal to the app.
static OWNER: AtomicIsize = AtomicIsize::new(0);
/// The id of the window `OWNER` belongs to.
static OWNER_WINDOW: AtomicU64 = AtomicU64::new(0);

/// Remembers `window`'s `HWND` as the owner of every dialog shown later.
pub fn capture_owner(window: &gpui::Window) {
    // `gpui::Window` has its own `window_handle`, which would shadow the
    // `raw_window_handle` one in method syntax.
    if let Ok(handle) = raw_window_handle::HasWindowHandle::window_handle(window) {
        if let RawWindowHandle::Win32(h) = handle.as_raw() {
            OWNER.store(h.hwnd.get(), Ordering::Relaxed);
            OWNER_WINDOW.store(
                window.window_handle().window_id().as_u64(),
                Ordering::Relaxed,
            );
        }
    }
}

/// Forgets the owner when its window closes, so no dialog is ever given a
/// stale (possibly recycled) `HWND`.
pub fn release_owner(closed: gpui::WindowId) {
    if OWNER_WINDOW.load(Ordering::Relaxed) == closed.as_u64() {
        OWNER.store(0, Ordering::Relaxed);
        OWNER_WINDOW.store(0, Ordering::Relaxed);
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
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) }
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

/// Every string a dialog is given, as NUL-terminated wide buffers. The caller
/// keeps this value alive until `Show` returns, so no pointer handed to the
/// dialog dangles whether or not the dialog copies it.
struct DialogStrings {
    filters: Vec<WideFilter>,
    specs: Vec<COMDLG_FILTERSPEC>,
    extension: Option<Vec<u16>>,
    folder: Option<Vec<u16>>,
    file_name: Option<Vec<u16>>,
    title: Option<Vec<u16>>,
}

impl DialogStrings {
    fn new(
        filters: &[FileFilter],
        extension: Option<&str>,
        folder: Option<&std::path::Path>,
        file_name: Option<&OsStr>,
        title: Option<&str>,
    ) -> Self {
        let filters = wide_filters(filters);
        let specs = filters
            .iter()
            .map(|f| COMDLG_FILTERSPEC {
                pszName: PCWSTR(f.name.as_ptr()),
                pszSpec: PCWSTR(f.spec.as_ptr()),
            })
            .collect();
        Self {
            filters,
            specs,
            extension: extension.map(default_extension),
            folder: folder.map(|p| wide(p.as_os_str())),
            file_name: file_name.map(wide),
            title: title.map(|t| wide(OsStr::new(t))),
        }
    }

    /// Hands the strings to `dialog`.
    fn apply(&self, dialog: &IFileDialog) -> windows::core::Result<()> {
        debug_assert_eq!(self.filters.len(), self.specs.len());
        if !self.specs.is_empty() {
            // SAFETY: every pointer in `self.specs` points into
            // `self.filters`, and `self` outlives `Show` (see the type docs).
            unsafe {
                dialog.SetFileTypes(&self.specs)?;
                dialog.SetFileTypeIndex(1)?;
            }
        }
        if let Some(ext) = &self.extension {
            // SAFETY: NUL-terminated, owned by `self`, which outlives `Show`.
            unsafe { dialog.SetDefaultExtension(PCWSTR(ext.as_ptr()))? };
        }
        if let Some(folder) = &self.folder {
            // SAFETY: NUL-terminated, owned by `self`.
            let item: windows::core::Result<IShellItem> =
                unsafe { SHCreateItemFromParsingName(PCWSTR(folder.as_ptr()), None::<&IBindCtx>) };
            // A folder that no longer exists is skipped: Windows picks one.
            if let Ok(item) = item {
                // SAFETY: `item` is a live shell item.
                let _ = unsafe { dialog.SetFolder(&item) };
            }
        }
        if let Some(name) = &self.file_name {
            // SAFETY: NUL-terminated, owned by `self`, which outlives `Show`.
            unsafe { dialog.SetFileName(PCWSTR(name.as_ptr()))? };
        }
        if let Some(title) = &self.title {
            // SAFETY: NUL-terminated, owned by `self`, which outlives `Show`.
            let _ = unsafe { dialog.SetTitle(PCWSTR(title.as_ptr())) };
        }
        Ok(())
    }
}

/// Shows `dialog` and reads the chosen path; `Ok(None)` when cancelled. The
/// caller still holds the [`DialogStrings`] the dialog was given.
fn show_and_read(dialog: &IFileDialog, _strings: &DialogStrings) -> PickResult {
    // SAFETY: `dialog` is a live dialog. The owner, if any, is cleared when
    // its window closes and belongs to another thread, which is allowed for
    // an owner. The strings passed to the dialog outlive this call (`_strings`).
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

/// Adds `extra` to the dialog's default options.
fn add_options(dialog: &IFileDialog, extra: FILEOPENDIALOGOPTIONS) -> Result<(), PickError> {
    // SAFETY: `dialog` is a live dialog.
    let options = unsafe { dialog.GetOptions() }.map_err(failed)?;
    // SAFETY: as above.
    unsafe { dialog.SetOptions(options | extra) }.map_err(failed)
}

/// Shows an open dialog and blocks until it closes. Run it on a thread of
/// your own (see [`super::gate::run_dialog`]): never on the UI thread.
///
/// # Errors
/// [`PickError::Failed`] if the dialog could not be shown or returned an error.
pub fn run_open(spec: &OpenSpec) -> PickResult {
    let _com = ComApartment::enter()?;
    // SAFETY: COM is initialised on this thread.
    let open: IFileOpenDialog =
        unsafe { CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER) }.map_err(failed)?;
    let dialog: &IFileDialog = &open;
    let (extra, filters): (_, &[FileFilter]) = match spec.kind {
        PickKind::File => (
            FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | FOS_FILEMUSTEXIST,
            &spec.filters,
        ),
        PickKind::Folder => (FOS_FORCEFILESYSTEM | FOS_PICKFOLDERS, &[]),
    };
    add_options(dialog, extra)?;
    let strings = DialogStrings::new(
        filters,
        None,
        spec.start_dir.as_deref(),
        None,
        spec.title.as_deref(),
    );
    strings.apply(dialog).map_err(failed)?;
    show_and_read(dialog, &strings)
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
    add_options(dialog, FOS_FORCEFILESYSTEM)?;
    let strings = DialogStrings::new(
        &spec.filters,
        spec.default_extension.as_deref(),
        spec.start_dir.as_deref(),
        spec.suggested_name.as_deref(),
        spec.title.as_deref(),
    );
    strings.apply(dialog).map_err(failed)?;
    show_and_read(dialog, &strings)
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
