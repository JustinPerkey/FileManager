//! Watches one file for changes made outside the app.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_mini::{
    new_debouncer, DebounceEventResult, DebouncedEvent, DebouncedEventKind, Debouncer,
};

/// Quiet time after the last change before an event is sent.
pub const DEBOUNCE: Duration = Duration::from_millis(300);

type Deb = Debouncer<RecommendedWatcher>;

/// What keeps the watch alive.
enum Keep {
    /// The parent directory existed: watched directly.
    Direct(#[expect(dead_code, reason = "held so that dropping it stops watching")] Deb),
    /// The parent directory is missing: a helper thread moves the watch down
    /// the chain. Dropping the debouncer ends the thread.
    Following(
        #[expect(dead_code, reason = "held so that dropping it stops watching")] Arc<Mutex<Deb>>,
    ),
}

/// Watches the parent directory of one file, never recursively. Dropping it
/// stops watching.
pub struct FileWatcher {
    path: PathBuf,
    _keep: Keep,
}

/// Where watches are placed: `notify`'s watcher, or a recorder in tests.
trait Mount {
    fn add(&mut self, dir: &Path, mode: RecursiveMode) -> Result<(), String>;
    fn remove(&mut self, dir: &Path);
}

impl Mount for dyn Watcher + '_ {
    fn add(&mut self, dir: &Path, mode: RecursiveMode) -> Result<(), String> {
        Watcher::watch(self, dir, mode).map_err(|e| e.to_string())
    }

    fn remove(&mut self, dir: &Path) {
        let _ = Watcher::unwatch(self, dir);
    }
}

/// Tracks which directory is watched: the deepest existing ancestor of the
/// file's parent, moving down as the chain appears. Always non-recursive.
struct Follower {
    parent: PathBuf,
    watched: Option<PathBuf>,
}

impl Follower {
    fn at_parent(&self) -> bool {
        self.watched.as_deref() == Some(self.parent.as_path())
    }

    /// Moves the watch to the deepest existing ancestor of the parent
    /// (watching the new directory before dropping the old one, so no change
    /// slips through).
    fn step(&mut self, mount: &mut (impl Mount + ?Sized)) -> Result<(), String> {
        let target = self
            .parent
            .ancestors()
            .find(|d| d.is_dir())
            .map(Path::to_path_buf)
            .ok_or_else(|| format!("{} does not exist", self.parent.display()))?;
        if self.watched.as_deref() == Some(target.as_path()) {
            return Ok(());
        }
        mount.add(&target, RecursiveMode::NonRecursive)?;
        if let Some(old) = self.watched.replace(target) {
            mount.remove(&old);
        }
        Ok(())
    }
}

impl FileWatcher {
    /// Calls `on_change` once per burst of changes to the file at `path`
    /// (created, written, replaced, renamed into place, or deleted). The
    /// parent directory is watched, never recursively, because editors often
    /// save by replacing the file, which a watch on the file itself would lose.
    ///
    /// When the parent directory does not exist yet, the deepest existing
    /// ancestor is watched instead (still non-recursively) and the watch moves
    /// down as each missing directory appears; `on_change` is called when the
    /// file itself appears.
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
        if dir.is_dir() {
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
            return Ok(FileWatcher {
                path: path.to_path_buf(),
                _keep: Keep::Direct(debouncer),
            });
        }
        Self::start_following(path, name, dir, on_change)
    }

    fn start_following(
        path: &Path,
        name: std::ffi::OsString,
        parent: PathBuf,
        on_change: impl Fn() + Send + 'static,
    ) -> Result<FileWatcher, String> {
        let (tx, rx) = mpsc::channel::<bool>();
        let debouncer = new_debouncer(DEBOUNCE, move |res: DebounceEventResult| {
            let file = res.is_ok_and(|events| is_file_change(&events, &name));
            let _ = tx.send(file);
        })
        .map_err(|e| e.to_string())?;
        let debouncer = Arc::new(Mutex::new(debouncer));
        let mut follower = Follower {
            parent,
            watched: None,
        };
        follower.step(debouncer.lock().map_err(|e| e.to_string())?.watcher())?;
        let weak = Arc::downgrade(&debouncer);
        let file = path.to_path_buf();
        std::thread::Builder::new()
            .name("file-watch-follow".to_owned())
            .spawn(move || follow(&rx, &weak, follower, &file, &on_change))
            .map_err(|e| e.to_string())?;
        Ok(FileWatcher {
            path: path.to_path_buf(),
            _keep: Keep::Following(debouncer),
        })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// The helper thread: after every batch at the watched directory, moves the
/// watch down the chain, and reports the file when it changed or appeared.
fn follow(
    rx: &mpsc::Receiver<bool>,
    debouncer: &Weak<Mutex<Deb>>,
    mut follower: Follower,
    file: &Path,
    on_change: &dyn Fn(),
) {
    while let Ok(file_event) = rx.recv() {
        let was_at_parent = follower.at_parent();
        {
            let Some(arc) = debouncer.upgrade() else {
                return;
            };
            let Ok(mut guard) = arc.lock() else {
                return;
            };
            // A failed move keeps the old watch; the next event tries again.
            let _ = follower.step(guard.watcher());
        }
        let now_at_parent = follower.at_parent();
        if (was_at_parent && file_event) || (!was_at_parent && now_at_parent && file.exists()) {
            on_change();
        }
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
    use std::path::{Path, PathBuf};
    use std::sync::mpsc;
    use std::time::Duration;

    use notify_debouncer_mini::notify::RecursiveMode;

    use super::{is_file_change, FileWatcher, Follower, Mount};

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

        // Another file in the same directory is not the watched file.
        fs::write(dir.path().join("other.toml"), "x").unwrap();
        assert!(rx.recv_timeout(Duration::from_millis(900)).is_err());
    }

    /// Records every watch placed, as `notify` would be asked.
    #[derive(Default)]
    struct Recorder {
        calls: Vec<(String, PathBuf, Option<RecursiveMode>)>,
    }

    impl Mount for Recorder {
        fn add(&mut self, dir: &Path, mode: RecursiveMode) -> Result<(), String> {
            self.calls
                .push(("watch".into(), dir.to_path_buf(), Some(mode)));
            Ok(())
        }

        fn remove(&mut self, dir: &Path) {
            self.calls.push(("unwatch".into(), dir.to_path_buf(), None));
        }
    }

    #[test]
    fn follower_moves_down_and_never_recurses() {
        let dir = tempfile::tempdir().expect("tempdir");
        let parent = dir.path().join("a").join("b");
        let mut f = Follower {
            parent: parent.clone(),
            watched: None,
        };
        let mut rec = Recorder::default();
        f.step(&mut rec).unwrap();
        assert!(!f.at_parent());
        f.step(&mut rec).unwrap();
        assert_eq!(rec.calls.len(), 1, "nothing moved while nothing appeared");
        fs::create_dir(dir.path().join("a")).unwrap();
        f.step(&mut rec).unwrap();
        fs::create_dir(&parent).unwrap();
        f.step(&mut rec).unwrap();
        assert!(f.at_parent());
        let watched: Vec<&Path> = rec
            .calls
            .iter()
            .filter(|c| c.0 == "watch")
            .map(|c| c.1.as_path())
            .collect();
        assert_eq!(
            watched,
            [dir.path(), dir.path().join("a").as_path(), parent.as_path()]
        );
        assert_eq!(rec.calls.iter().filter(|c| c.0 == "unwatch").count(), 2);
        assert!(rec
            .calls
            .iter()
            .filter(|c| c.0 == "watch")
            .all(|c| c.2 == Some(RecursiveMode::NonRecursive)));
    }

    #[test]
    fn missing_parent_watches_ancestor_then_file_appears() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a").join("b").join("c.toml");
        let (tx, rx) = mpsc::channel();
        let _watcher = FileWatcher::start(&file, move || {
            let _ = tx.send(());
        })
        .expect("watch the ancestor");
        std::thread::sleep(Duration::from_millis(200));
        fs::create_dir(dir.path().join("a")).unwrap();
        std::thread::sleep(Duration::from_millis(900));
        assert!(rx.try_recv().is_err(), "a directory is not the file");
        fs::create_dir(dir.path().join("a").join("b")).unwrap();
        std::thread::sleep(Duration::from_millis(900));
        fs::write(&file, "x\n").unwrap();
        rx.recv_timeout(Duration::from_secs(5))
            .expect("the file appeared");
        // Edits of the file now arrive through the parent's watch.
        std::thread::sleep(Duration::from_millis(900));
        while rx.try_recv().is_ok() {}
        fs::write(&file, "y\n").unwrap();
        rx.recv_timeout(Duration::from_secs(5)).expect("an edit");
    }

    #[test]
    fn whole_chain_appearing_at_once_is_reported() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a").join("b").join("c.toml");
        let (tx, rx) = mpsc::channel();
        let _watcher = FileWatcher::start(&file, move || {
            let _ = tx.send(());
        })
        .expect("watch");
        std::thread::sleep(Duration::from_millis(200));
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "x\n").unwrap();
        rx.recv_timeout(Duration::from_secs(5))
            .expect("the file appeared");
    }

    #[test]
    fn existing_parent_unchanged_behaviour() {
        let dir = tempfile::tempdir().expect("tempdir");
        let m = dir.path().join("m.toml");
        let (tx, rx) = mpsc::channel();
        let w = FileWatcher::start(&m, move || {
            let _ = tx.send(());
        })
        .expect("watch");
        assert_eq!(w.path(), m);
        std::thread::sleep(Duration::from_millis(200));
        fs::write(&m, "one\n").unwrap();
        rx.recv_timeout(Duration::from_secs(5)).expect("created");
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
