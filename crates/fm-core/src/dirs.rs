//! Per-user application directories.

use crate::error::{FmError, Result};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Root of all persisted app state; each tool gets its own subdirectory.
#[derive(Debug, Clone)]
pub struct AppDirs {
    root: PathBuf,
}

impl AppDirs {
    /// Resolve the per-user app data directory `FileManager` (on Windows,
    /// `%APPDATA%\FileManager`).
    ///
    /// # Errors
    ///
    /// [`FmError::NoAppDataDir`] if the system reports no home directory.
    pub fn from_system() -> Result<Self> {
        let base = directories::BaseDirs::new().ok_or(FmError::NoAppDataDir)?;
        Ok(Self {
            root: base.data_dir().join("FileManager"),
        })
    }

    /// Use an explicit root directory (used by tests).
    #[must_use]
    pub fn at(root: PathBuf) -> Self {
        Self { root }
    }

    /// The root directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `<root>/<tool>/`. `tool` must match `[a-z][a-z0-9-]*`.
    ///
    /// # Errors
    ///
    /// [`FmError::InvalidToolName`] if `tool` does not match.
    pub fn tool_dir(&self, tool: &str) -> Result<PathBuf> {
        if !is_valid_name(tool) {
            return Err(FmError::InvalidToolName(tool.to_owned()));
        }
        Ok(self.root.join(tool))
    }
}

/// True if `name` matches `[a-z][a-z0-9-]*`. Shared by tool and state names.
pub(crate) fn is_valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some('a'..='z'))
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// The machine-wide `FileManager` configuration directory. The app only reads
/// from it (developer tooling may create files there).
///
/// - Windows: `<ProgramData>\FileManager`, where `<ProgramData>` is the
///   `ProgramData` environment variable (normally `C:\ProgramData`); it must be
///   an absolute path.
/// - Other platforms (development builds only): `/etc/filemanager`.
///
/// # Errors
///
/// [`FmError::NoProgramDataDir`] on Windows if the variable is unset, empty or
/// not absolute.
pub fn system_config_dir() -> Result<PathBuf> {
    #[cfg(windows)]
    {
        system_config_dir_from(std::env::var_os("ProgramData"))
    }
    #[cfg(not(windows))]
    {
        Ok(PathBuf::from("/etc/filemanager"))
    }
}

/// The Windows rule of [`system_config_dir`], compiled on every platform.
#[cfg_attr(
    all(not(windows), not(test)),
    expect(
        dead_code,
        reason = "used only by the Windows branch of system_config_dir and by tests"
    )
)]
fn system_config_dir_from(program_data: Option<OsString>) -> Result<PathBuf> {
    match program_data {
        Some(p) if !p.is_empty() && Path::new(&p).is_absolute() => {
            Ok(PathBuf::from(p).join("FileManager"))
        }
        _ => Err(FmError::NoProgramDataDir),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_config_dir_from_program_data() {
        let tmp = tempfile::tempdir().unwrap();
        let got = system_config_dir_from(Some(tmp.path().as_os_str().to_owned())).unwrap();
        assert_eq!(got, tmp.path().join("FileManager"));
        #[cfg(not(windows))]
        assert_eq!(
            system_config_dir().unwrap(),
            PathBuf::from("/etc/filemanager")
        );
    }

    #[test]
    fn system_config_dir_rejects_missing_or_relative() {
        for v in [
            None,
            Some(OsString::new()),
            Some(OsString::from("relative/dir")),
        ] {
            assert!(matches!(
                system_config_dir_from(v),
                Err(FmError::NoProgramDataDir)
            ));
        }
    }
}
