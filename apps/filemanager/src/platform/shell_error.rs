//! The sentences for a failed `ShellExecuteExW`, as a pure mapping so it is
//! testable on every platform. The codes are Win32 errors as `GetLastError`
//! reports them, not the old `SE_ERR_*` values `ShellExecuteW` returned.

use super::OpenError;

/// `ERROR_CANCELLED`: the user dismissed Windows' "How do you want to open
/// this file?" dialog.
pub(crate) const ERROR_CANCELLED: u32 = 1223;

/// The sentence for a Win32 error code `ShellExecuteExW` can report.
pub(crate) fn shell_error_detail(code: u32) -> String {
    match code {
        2 => "The file was not found.".to_string(),
        3 => "The folder was not found.".to_string(),
        5 => "Access was denied.".to_string(),
        8 => "There was not enough memory or resources.".to_string(),
        32 => "A sharing violation occurred.".to_string(),
        1155 => "No application is associated with this file type.".to_string(),
        1156 => "The request to the application failed.".to_string(),
        1157 => "A required library was not found.".to_string(),
        n => format!("Windows error {n}"),
    }
}

/// The result of an open that failed with Win32 error `code`. Cancelling the
/// "open with" dialog is not a failure.
pub(crate) fn open_failure(code: u32) -> Result<(), OpenError> {
    if code == ERROR_CANCELLED {
        Ok(())
    } else {
        Err(OpenError {
            detail: shell_error_detail(code),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_error_detail_maps_shellexecuteex_codes() {
        for (code, text) in [
            (2, "The file was not found."),
            (3, "The folder was not found."),
            (5, "Access was denied."),
            (8, "There was not enough memory or resources."),
            (32, "A sharing violation occurred."),
            (1155, "No application is associated with this file type."),
            (1156, "The request to the application failed."),
            (1157, "A required library was not found."),
        ] {
            assert_eq!(shell_error_detail(code), text, "code {code}");
        }
        // 31 was SE_ERR_NOASSOC for ShellExecuteW; here it is not that.
        assert_eq!(shell_error_detail(31), "Windows error 31");
        assert_eq!(shell_error_detail(26), "Windows error 26");
    }

    #[test]
    fn cancelled_open_is_ok() {
        assert_eq!(open_failure(ERROR_CANCELLED), Ok(()));
        assert!(open_failure(5).is_err());
    }
}
