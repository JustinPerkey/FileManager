//! Watches the loaded manifest file for edits made outside the app.

use std::path::{Path, PathBuf};
use std::time::Duration;

use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{new_debouncer, DebounceEventResult, DebouncedEventKind, Debouncer};

/// Quiet time after the last change before an event is sent.
pub const DEBOUNCE: Duration = Duration::from_millis(300);

/// Dropping it stops watching.
pub struct ManifestWatcher {
    path: PathBuf,
    _debouncer: Debouncer<RecommendedWatcher>,
}

impl ManifestWatcher {
    /// Calls `on_change` once per burst of changes to the file at `path`. The
    /// parent directory is watched, because editors often save by replacing
    /// the file, which a watch on the file itself would lose.
    pub fn start(
        path: &Path,
        on_change: impl Fn() + Send + 'static,
    ) -> Result<ManifestWatcher, String> {
        let name = path
            .file_name()
            .ok_or_else(|| "the manifest path has no file name".to_owned())?
            .to_owned();
        let dir = match path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
            _ => PathBuf::from("."),
        };
        let mut debouncer = new_debouncer(DEBOUNCE, move |res: DebounceEventResult| {
            if let Ok(events) = res {
                // `AnyContinuous` reports a burst that is still going, and can
                // arrive just before the `Any` that ends it; only `Any` counts.
                if events
                    .iter()
                    .any(|e| e.kind == DebouncedEventKind::Any && e.path.file_name() == Some(&name))
                {
                    on_change();
                }
            }
        })
        .map_err(|e| e.to_string())?;
        debouncer
            .watcher()
            .watch(&dir, RecursiveMode::NonRecursive)
            .map_err(|e| e.to_string())?;
        Ok(ManifestWatcher {
            path: path.to_path_buf(),
            _debouncer: debouncer,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
