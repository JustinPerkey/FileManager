//! Opening a file and revealing it in Explorer, with the failure reported.
//!
//! GPUI's `open_with_system` and `reveal_path` return nothing, so a file with
//! no handler would fail silently. This wrapper calls the shell directly:
//! `ShellExecuteExW` for opening, `SHParseDisplayName` plus
//! `SHOpenFolderAndSelectItems` for revealing. Paths go in as wide strings
//! from [`OsStr::encode_wide`], so a name that is not valid Unicode survives.
//! Each call runs on a background thread initialised for COM
//! (apartment-threaded), never on the UI thread. That thread ends right after
//! the call, so `ShellExecuteExW` runs with `SEE_MASK_NOASYNC`: without it the
//! shell may finish a DDE or shell-extension handler on a thread that no
//! longer exists. A `FALSE` return is the error, read from `GetLastError`;
//! Windows' own "no app is associated" dialog stays (no
//! `SEE_MASK_FLAG_NO_UI`). The live calls can only be checked on Windows: see
//! `docs/gpui-dialogs-check.md`.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};

use gpui::{App, Task};
use windows::core::{HRESULT, PCWSTR};
use windows::Win32::Foundation::{ERROR_BAD_FORMAT, ERROR_FILE_NOT_FOUND, ERROR_NO_ASSOCIATION};
use windows::Win32::System::Com::{
    CoInitializeEx, CoUninitialize, IBindCtx, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    ILFree, SHOpenFolderAndSelectItems, SHParseDisplayName, ShellExecuteExW, SEE_MASK_NOASYNC,
    SE_ERR_ACCESSDENIED, SE_ERR_ASSOCINCOMPLETE, SE_ERR_DDEBUSY, SE_ERR_DDEFAIL, SE_ERR_DDETIMEOUT,
    SE_ERR_DLLNOTFOUND, SE_ERR_FNF, SE_ERR_NOASSOC, SE_ERR_OOM, SE_ERR_PNF, SE_ERR_SHARE,
    SHELLEXECUTEINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use super::OpenError;

/// `s` as a NUL-terminated UTF-16 buffer (lone surrogates are kept).
fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

/// The sentence for a failed shell-execute code: a Win32 error as
/// `GetLastError` reports it, which for the small codes is the same number as
/// the `SE_ERR_*` value the older `ShellExecute` returned.
fn shell_execute_detail(code: usize) -> String {
    const BAD_FORMAT: u32 = ERROR_BAD_FORMAT.0;
    const NO_ASSOCIATION: u32 = ERROR_NO_ASSOCIATION.0;
    let known = u32::try_from(code).ok().and_then(|c| match c {
        0 | SE_ERR_OOM => Some("There was not enough memory or resources."),
        SE_ERR_FNF => Some("The file was not found."),
        SE_ERR_PNF => Some("The folder was not found."),
        SE_ERR_ACCESSDENIED => Some("Access was denied."),
        BAD_FORMAT => Some("The file is not a valid program."),
        SE_ERR_SHARE => Some("A sharing violation occurred."),
        SE_ERR_ASSOCINCOMPLETE => Some("The file association is incomplete or invalid."),
        SE_ERR_DDETIMEOUT => Some("The request timed out."),
        SE_ERR_DDEFAIL => Some("The request failed."),
        SE_ERR_DDEBUSY => Some("The request could not be completed: the application is busy."),
        SE_ERR_NOASSOC | NO_ASSOCIATION => {
            Some("No application is associated with this file type.")
        }
        SE_ERR_DLLNOTFOUND => Some("A required library was not found."),
        _ => None,
    });
    known.map_or_else(
        || format!("The shell reported error {code}."),
        str::to_string,
    )
}

/// The Win32 error code inside an `HRESULT` that wraps one.
fn win32_code(error: &windows::core::Error) -> Option<usize> {
    let hr = error.code().0.cast_unsigned();
    (hr >> 16 == 0x8007).then(|| usize::try_from(hr & 0xFFFF).unwrap_or(usize::MAX))
}

/// The detail of a failed `ShellExecuteExW`: the sentence for the
/// `GetLastError` code when it has one, else what Windows says for it.
fn shell_error_detail(error: &windows::core::Error) -> String {
    match win32_code(error) {
        Some(code) if !shell_execute_detail(code).starts_with("The shell reported") => {
            shell_execute_detail(code)
        }
        _ => error.message(),
    }
}

/// Runs `ShellExecuteExW` with the verb "open" on `target`, waiting for the
/// handler to take the request (`SEE_MASK_NOASYNC`).
fn shell_open(target: &OsStr) -> Result<(), OpenError> {
    let file = wide(target);
    let verb = wide(OsStr::new("open"));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: u32::try_from(std::mem::size_of::<SHELLEXECUTEINFOW>())
            .expect("SHELLEXECUTEINFOW is far smaller than u32::MAX"),
        fMask: SEE_MASK_NOASYNC,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..SHELLEXECUTEINFOW::default()
    };
    // SAFETY: `info` is a fully initialised `SHELLEXECUTEINFOW` whose
    // `cbSize` is its size; `verb` and `file` are NUL-terminated buffers that
    // outlive the call; the other pointers are null, which the API allows; no
    // window handle is passed, and `fMask` asks for no process handle, so
    // nothing is left to close.
    unsafe { ShellExecuteExW(&raw mut info) }.map_err(|e| OpenError {
        detail: shell_error_detail(&e),
    })
}

/// A parsed item id list, freed on drop.
struct ItemIdList(*mut ITEMIDLIST);

impl Drop for ItemIdList {
    fn drop(&mut self) {
        // SAFETY: the pointer came from a successful `SHParseDisplayName`,
        // which allocates it for the caller to free with `ILFree`, and it is
        // freed only here.
        unsafe { ILFree(Some(self.0.cast_const())) };
    }
}

fn parse_display_name(path: &OsStr) -> windows::core::Result<ItemIdList> {
    let name = wide(path);
    let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();
    // SAFETY: `name` is a NUL-terminated buffer that outlives the call, `pidl`
    // is a valid out pointer, and no bind context or attributes are asked for.
    unsafe {
        SHParseDisplayName(
            PCWSTR(name.as_ptr()),
            None::<&IBindCtx>,
            &raw mut pidl,
            0,
            None,
        )
    }?;
    Ok(ItemIdList(pidl))
}

fn select_in_explorer(path: &OsStr) -> windows::core::Result<()> {
    let item = parse_display_name(path)?;
    // SAFETY: `item.0` is a valid item id list owned by `item`, which lives
    // past the call. With no child items the shell opens the parent folder
    // and selects `item` itself.
    unsafe { SHOpenFolderAndSelectItems(item.0.cast_const(), None, 0) }
}

/// Opens `path` with its default application.
fn open_blocking(path: &Path) -> Result<(), OpenError> {
    shell_open(path.as_os_str())
}

/// Shows `path` in Explorer with the file selected. When the item cannot be
/// parsed because it does not exist, opens its parent folder instead, as GPUI
/// does; any other failed `HRESULT` is the error.
fn reveal_blocking(path: &Path) -> Result<(), OpenError> {
    match select_in_explorer(path.as_os_str()) {
        Ok(()) => Ok(()),
        Err(e) if e.code() == HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0) => {
            match path.parent().filter(|p| !p.as_os_str().is_empty()) {
                Some(parent) => shell_open(parent.as_os_str()),
                None => Err(OpenError {
                    detail: e.message(),
                }),
            }
        }
        Err(e) => Err(OpenError {
            detail: e.message(),
        }),
    }
}

/// Initialises COM for the current thread; uninitialises on drop.
struct ComApartment;

impl ComApartment {
    fn enter() -> Result<Self, OpenError> {
        // SAFETY: no reserved argument; paired with `CoUninitialize` in `Drop`
        // on the same thread. Called once per shell thread.
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) }
            .ok()
            .map_err(|e| OpenError {
                detail: e.message(),
            })?;
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        // SAFETY: balances the successful `CoInitializeEx` in `enter`.
        unsafe { CoUninitialize() };
    }
}

/// Runs `job` on a COM thread of its own and answers when it is done.
fn run_shell(
    job: impl FnOnce() -> Result<(), OpenError> + Send + 'static,
    cx: &App,
) -> Task<Result<(), OpenError>> {
    let (tx, rx) = async_channel::bounded(1);
    let spawned = std::thread::Builder::new()
        .name("shell-open".to_string())
        .spawn(move || {
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                let _apartment = ComApartment::enter()?;
                job()
            }))
            .unwrap_or_else(|_| {
                Err(OpenError {
                    detail: "the shell thread panicked".to_string(),
                })
            });
            let _ = tx.send_blocking(outcome);
        });
    if let Err(e) = spawned {
        return Task::ready(Err(OpenError {
            detail: e.to_string(),
        }));
    }
    cx.foreground_executor().spawn(async move {
        rx.recv().await.unwrap_or_else(|_| {
            Err(OpenError {
                detail: "the shell thread stopped".to_string(),
            })
        })
    })
}

/// Opens `path` with the system's default application.
pub fn open(path: &Path, cx: &App) -> Task<Result<(), OpenError>> {
    let path: PathBuf = path.to_path_buf();
    run_shell(move || open_blocking(&path), cx)
}

/// Shows `path` in Explorer, selected.
pub fn reveal(path: &Path, cx: &App) -> Task<Result<(), OpenError>> {
    let path: PathBuf = path.to_path_buf();
    run_shell(move || reveal_blocking(&path), cx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documented_codes_have_their_own_text() {
        for code in [0, 2, 3, 5, 8, 11, 26, 27, 28, 29, 30, 31, 32, 1155] {
            assert!(
                !shell_execute_detail(code).starts_with("The shell reported"),
                "code {code} is documented"
            );
        }
        assert_eq!(
            shell_execute_detail(31),
            "No application is associated with this file type."
        );
    }

    #[test]
    fn a_failed_shell_execute_is_described_from_its_last_error() {
        let failed = |code: u32| {
            shell_error_detail(&windows::core::Error::from_hresult(HRESULT::from_win32(
                code,
            )))
        };
        assert_eq!(
            failed(ERROR_NO_ASSOCIATION.0),
            "No application is associated with this file type."
        );
        assert_eq!(failed(ERROR_FILE_NOT_FOUND.0), "The file was not found.");
        // A code with no sentence of its own falls back to what Windows says.
        let unknown = windows::core::Error::from_hresult(HRESULT::from_win32(1223));
        assert_eq!(shell_error_detail(&unknown), unknown.message());
    }

    #[test]
    fn an_unknown_code_is_named() {
        assert_eq!(shell_execute_detail(17), "The shell reported error 17.");
    }

    #[test]
    fn wide_keeps_lone_surrogates_and_terminates() {
        use std::os::windows::ffi::OsStringExt;
        let s = std::ffi::OsString::from_wide(&[0x61, 0xD800, 0x62]);
        assert_eq!(wide(&s), [0x61, 0xD800, 0x62, 0]);
    }
}
