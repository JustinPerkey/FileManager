//! `fm-spike-results.txt` beside the exe: one line per check-screen action.

use std::io::Write;
use std::path::PathBuf;

/// Where results go: beside the exe, falling back to the current directory.
#[must_use]
pub fn results_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf))
        .unwrap_or_default()
        .join("fm-spike-results.txt")
}

/// Appends one line. Failures are ignored: the check screen still shows it.
pub fn append(path: &std::path::Path, line: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{line}");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn append_adds_lines() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("r.txt");
        super::append(&p, "one");
        super::append(&p, "two");
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "one\ntwo\n");
    }
}
