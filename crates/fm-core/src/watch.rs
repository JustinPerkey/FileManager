//! Watches one file for changes made outside the app.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{
    new_debouncer, DebounceEventResult, DebouncedEvent, DebouncedEventKind, Debouncer,
};

/// Quiet time after the last change before an event is sent.
pub const DEBOUNCE: Duration = Duration::from_millis(300);

/// Watches the parent directory of one file, never recursively. Dropping it
/// stops watching.
pub struct FileWatcher {
    path: PathBuf,
    _debouncer: Debouncer<RecommendedWatcher>,
}

impl FileWatcher {
    /// Calls `on_change` once per burst of changes to the file at `path`
    /// (created, written, replaced, renamed into place, or deleted). The
    /// parent directory is watched, never recursively, because editors often
    /// save by replacing the file, which a watch on the file itself would lose.
    ///
    /// # Errors
    ///
    /// Returns the watcher's message when `path` has no file name or the
    /// directory cannot be watched.
    pub fn start(
        path: &Path,
        on_change: impl Fn() + Send + 'static,
    ) -> Result<FileWatcher, String> {
        let name = path
            .file_name()
            .ok_or_else(|| "the path has no file name".to_owned())?
            .to_owned();
        let dir = match path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
            _ => PathBuf::from("."),
        };
        let mut debouncer = new_debouncer(DEBOUNCE, move |res: DebounceEventResult| {
            if let Ok(events) = res {
                if is_file_change(&events, &name) {
                    on_change();
                }
            }
        })
        .map_err(|e| e.to_string())?;
        debouncer
            .watcher()
            .watch(&dir, RecursiveMode::NonRecursive)
            .map_err(|e| e.to_string())?;
        Ok(FileWatcher {
            path: path.to_path_buf(),
            _debouncer: debouncer,
        })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Whether a batch ends a burst of changes to the file named `name`.
/// `AnyContinuous` reports a burst that is still going; the debouncer always
/// follows it with an `Any` for the same path, so only `Any` counts.
pub(crate) fn is_file_change(events: &[DebouncedEvent], name: &OsStr) -> bool {
    events
        .iter()
        .any(|e| e.kind == DebouncedEventKind::Any && e.path.file_name() == Some(name))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::time::Duration;

    use super::{is_file_change, FileWatcher};

    #[test]
    fn watcher_emits_once_per_edit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let m = dir.path().join("m.toml");
        fs::write(&m, "one\n").unwrap();
        let (tx, rx) = mpsc::channel();
        let _watcher = FileWatcher::start(&m, move || {
            let _ = tx.send(());
        })
        .expect("watch");
        // Let the watch settle, then edit once.
        std::thread::sleep(Duration::from_millis(200));
        fs::write(&m, "one\n# edit\n").unwrap();
        rx.recv_timeout(Duration::from_secs(5)).expect("one event");
        assert!(
            rx.recv_timeout(Duration::from_millis(900)).is_err(),
            "exactly one event"
        );

        // Another file in the same directory is not this manifest.
        fs::write(dir.path().join("other.toml"), "x").unwrap();
        assert!(rx.recv_timeout(Duration::from_millis(900)).is_err());
    }

    #[test]
    fn only_a_settled_change_to_the_manifest_counts() {
        use notify_debouncer_mini::{DebouncedEvent, DebouncedEventKind as Kind};
        use std::ffi::OsStr;

        let ev = |p: &str, k| DebouncedEvent::new(PathBuf::from(p), k);
        let name = OsStr::new("m.toml");
        assert!(is_file_change(&[ev("/w/m.toml", Kind::Any)], name));
        assert!(!is_file_change(
            &[ev("/w/m.toml", Kind::AnyContinuous)],
            name
        ));
        assert!(!is_file_change(&[ev("/w/other.toml", Kind::Any)], name));
        assert!(is_file_change(
            &[ev("/w/other.toml", Kind::Any), ev("/w/m.toml", Kind::Any)],
            name
        ));
    }
}
