//! Manifest-style file watcher (item A7), copied from the approach of
//! `apps/desktop/src-tauri/src/tools/tarpack/watch.rs`: notify-debouncer-mini,
//! 300 ms debounce, watching the parent directory, filtering to the file.

use std::path::{Path, PathBuf};
use std::time::Duration;

use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebounceEventResult, Debouncer};

pub const DEBOUNCE: Duration = Duration::from_millis(300);

/// Keeps the watcher alive; dropping it stops watching.
pub struct Watch {
    _debouncer: Debouncer<notify_debouncer_mini::notify::RecommendedWatcher>,
}

/// Starts watching `file`'s parent directory. `on_change` runs on the
/// watcher's own thread, once per debounced batch that touches `file`.
///
/// # Errors
/// When the parent directory cannot be watched.
pub fn watch_file(
    file: &Path,
    on_change: impl Fn() + Send + 'static,
) -> Result<Watch, notify_debouncer_mini::notify::Error> {
    let target: PathBuf = file.to_path_buf();
    let parent = target
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let mut debouncer = new_debouncer(DEBOUNCE, move |result: DebounceEventResult| {
        if let Ok(events) = result {
            if events.iter().any(|e| e.path == target) {
                on_change();
            }
        }
    })?;
    debouncer
        .watcher()
        .watch(&parent, RecursiveMode::NonRecursive)?;
    Ok(Watch {
        _debouncer: debouncer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn watcher_reports_edit() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("manifest.toml");
        std::fs::write(&file, "a = 1\n").unwrap();
        let (tx, rx) = mpsc::channel();
        let _watch = watch_file(&file, move || {
            let _ = tx.send(());
        })
        .unwrap();
        std::thread::sleep(Duration::from_millis(200));
        std::fs::write(&file, "a = 2\n").unwrap();
        rx.recv_timeout(Duration::from_secs(10))
            .expect("a change event within 10 s");
    }
}
