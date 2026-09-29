//! Crash-safe file replacement.

use crate::error::{FmError, Result};
use std::io::Write;
use std::path::Path;

/// Write `bytes` to a temp file beside `path`, sync it, then rename it over
/// `path`. The parent directory must exist. On error the temp file is removed
/// and any existing `path` is untouched.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let mut tmp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| FmError::io("creating temp file in", parent, e))?;
    tmp.write_all(bytes)
        .map_err(|e| FmError::io("writing", tmp.path().to_path_buf(), e))?;
    tmp.flush()
        .map_err(|e| FmError::io("flushing", tmp.path().to_path_buf(), e))?;
    tmp.as_file()
        .sync_all()
        .map_err(|e| FmError::io("syncing", tmp.path().to_path_buf(), e))?;
    // On failure `persist` returns the temp file, which is deleted on drop.
    tmp.persist(path)
        .map_err(|e| FmError::io("replacing", path, e.error))?;
    Ok(())
}
