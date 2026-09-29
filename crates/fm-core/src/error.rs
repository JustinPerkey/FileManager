//! Error type shared by every tool.

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use std::path::PathBuf;

/// Result alias using [`FmError`].
pub type Result<T> = std::result::Result<T, FmError>;

/// Errors produced by `fm-core`. Serializes as `{ kind, message }`.
#[derive(Debug, thiserror::Error)]
pub enum FmError {
    /// A tool name did not match `[a-z][a-z0-9-]*`.
    #[error("invalid tool name: {0:?}")]
    InvalidToolName(String),
    /// A state-file name did not match `[a-z][a-z0-9-]*`.
    #[error("invalid state name: {0:?}")]
    InvalidStateName(String),
    /// A filesystem operation failed.
    #[error("{action} {}: {source}", path.display())]
    Io {
        /// What was being attempted.
        action: &'static str,
        /// The path involved.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },
    /// JSON could not be parsed or produced.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// The system app-data directory could not be determined.
    #[error("could not determine the app data directory")]
    NoAppDataDir,
}

impl FmError {
    /// Stable machine-readable discriminator for the UI.
    pub fn kind(&self) -> &'static str {
        match self {
            FmError::InvalidToolName(_) => "InvalidToolName",
            FmError::InvalidStateName(_) => "InvalidStateName",
            FmError::Io { .. } => "Io",
            FmError::Json(_) => "Json",
            FmError::NoAppDataDir => "NoAppDataDir",
        }
    }

    pub(crate) fn io(
        action: &'static str,
        path: impl Into<PathBuf>,
        source: std::io::Error,
    ) -> Self {
        FmError::Io {
            action,
            path: path.into(),
            source,
        }
    }
}

impl Serialize for FmError {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("FmError", 2)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}
