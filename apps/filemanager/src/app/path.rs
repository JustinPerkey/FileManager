//! Path text handling shared by the tools' models.

use std::path::PathBuf;

/// A typed or pasted path: trimmed, without the quotes Explorer's "Copy as
/// path" adds. Empty means none.
#[must_use]
pub fn clean_path(typed: &str) -> Option<PathBuf> {
    let trimmed = typed.trim();
    let unquoted = trimmed
        .strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .unwrap_or(trimmed)
        .trim();
    if unquoted.is_empty() {
        None
    } else {
        Some(PathBuf::from(unquoted))
    }
}
