//! Per-user application directories.

use crate::error::{FmError, Result};
use std::path::{Path, PathBuf};

/// Root of all persisted app state; each tool gets its own subdirectory.
#[derive(Debug, Clone)]
pub struct AppDirs {
    root: PathBuf,
}

impl AppDirs {
    /// Resolve the per-user app data directory `FileManager` (on Windows,
    /// `%APPDATA%\FileManager`).
    pub fn from_system() -> Result<Self> {
        let base = directories::BaseDirs::new().ok_or(FmError::NoAppDataDir)?;
        Ok(Self {
            root: base.data_dir().join("FileManager"),
        })
    }

    /// Use an explicit root directory (used by tests).
    pub fn at(root: PathBuf) -> Self {
        Self { root }
    }

    /// The root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `<root>/<tool>/`. `tool` must match `[a-z][a-z0-9-]*`.
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
