//! `rfd` fallback for the save dialog's extension filter, which GPUI's
//! `prompt_for_new_path` cannot express (item A2). Windows only: `rfd` is a
//! Windows-only dependency of the spike.

use std::path::PathBuf;

/// Outcome of a fallback dialog, as plain data for the check screen.
#[derive(Debug)]
pub enum Outcome {
    Picked(Vec<PathBuf>),
    Cancelled,
    /// The backend panicked (rfd unwraps `String::from_utf16`): the message.
    Panicked(String),
    Unavailable,
}

#[cfg(windows)]
fn guarded(f: impl FnOnce() -> Option<Vec<PathBuf>> + std::panic::UnwindSafe) -> Outcome {
    match std::panic::catch_unwind(f) {
        Ok(Some(paths)) => Outcome::Picked(paths),
        Ok(None) => Outcome::Cancelled,
        Err(payload) => Outcome::Panicked(
            payload
                .downcast_ref::<&str>()
                .map(ToString::to_string)
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panic".into()),
        ),
    }
}

/// Save dialog with the suggested name `gateway.tar.gz` and the filter
/// "Tar archive (gz)" `*.gz`. Blocks: call from a worker thread.
#[must_use]
pub fn save_with_filter() -> Outcome {
    #[cfg(windows)]
    {
        guarded(|| {
            rfd::FileDialog::new()
                .add_filter("Tar archive (gz)", &["gz"])
                .set_file_name("gateway.tar.gz")
                .save_file()
                .map(|p| vec![p])
        })
    }
    #[cfg(not(windows))]
    {
        Outcome::Unavailable
    }
}

/// Open-file dialog through rfd, to compare path fidelity with GPUI's.
/// Blocks: call from a worker thread.
#[must_use]
pub fn open_file() -> Outcome {
    #[cfg(windows)]
    {
        guarded(|| rfd::FileDialog::new().pick_file().map(|p| vec![p]))
    }
    #[cfg(not(windows))]
    {
        Outcome::Unavailable
    }
}
