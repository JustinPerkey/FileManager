//! Remembered state: the recent-databases list.

use fm_core::StateData;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Most recent databases kept.
pub const MAX_RECENT: usize = 10;

/// Persisted as `dbmanager/state` in `fm-core`'s store.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RememberedState {
    /// Recent databases, most recent first, unique by path, at most
    /// [`MAX_RECENT`].
    pub recent: Vec<PathBuf>,
}

impl StateData for RememberedState {
    const SCHEMA_VERSION: u32 = 1;
}

fn same(a: &Path, b: &Path) -> bool {
    #[cfg(windows)]
    {
        match (a.to_str(), b.to_str()) {
            (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
            _ => a == b,
        }
    }
    #[cfg(not(windows))]
    {
        a == b
    }
}

impl RememberedState {
    /// Move `database` to the front, removing any equal entry, and truncate.
    /// Paths that are not valid Unicode are ignored.
    pub fn touch(&mut self, database: &Path) {
        if database.to_str().is_none() {
            return;
        }
        self.forget(database);
        self.recent.insert(0, database.to_path_buf());
        self.recent.truncate(MAX_RECENT);
    }

    /// Remove `database` from the list.
    pub fn forget(&mut self, database: &Path) {
        self.recent.retain(|p| !same(p, database));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fm_core::{AppDirs, Store};

    #[test]
    fn recent_touch_orders_dedupes_and_caps() {
        let mut s = RememberedState::default();
        for i in 0..12 {
            s.touch(Path::new(&format!("/d/{i}.db")));
        }
        let paths = |ids: &[usize]| -> Vec<PathBuf> {
            ids.iter()
                .map(|i| PathBuf::from(format!("/d/{i}.db")))
                .collect()
        };
        assert_eq!(s.recent, paths(&[11, 10, 9, 8, 7, 6, 5, 4, 3, 2]));
        s.touch(Path::new("/d/5.db"));
        assert_eq!(s.recent, paths(&[5, 11, 10, 9, 8, 7, 6, 4, 3, 2]));
        s.touch(Path::new("/d/0.db"));
        assert_eq!(s.recent, paths(&[0, 5, 11, 10, 9, 8, 7, 6, 4, 3]));
    }

    #[cfg(unix)]
    #[test]
    fn recent_touch_ignores_non_unicode() {
        use std::os::unix::ffi::OsStrExt;
        let mut s = RememberedState::default();
        s.touch(Path::new("/a.db"));
        s.touch(Path::new(std::ffi::OsStr::from_bytes(b"/tmp/\xff.db")));
        assert_eq!(s.recent, vec![PathBuf::from("/a.db")]);
    }

    #[cfg(windows)]
    #[test]
    fn recent_dedupes_case_insensitively_on_windows() {
        let mut s = RememberedState::default();
        s.touch(Path::new(r"C:\A.db"));
        s.touch(Path::new(r"c:\a.db"));
        assert_eq!(s.recent, vec![PathBuf::from(r"c:\a.db")]);
    }

    #[test]
    fn recent_forget_removes() {
        let mut s = RememberedState::default();
        s.touch(Path::new("/a.db"));
        s.touch(Path::new("/b.db"));
        s.forget(Path::new("/a.db"));
        assert_eq!(s.recent, vec![PathBuf::from("/b.db")]);
    }

    #[test]
    fn recent_round_trips_through_store() {
        let tmp = tempfile::tempdir().unwrap();
        let dirs = AppDirs::at(tmp.path().to_path_buf());
        let (mut store, _) = Store::<RememberedState>::load(&dirs, "dbmanager", "state").unwrap();
        store.update(|s| {
            s.touch(Path::new("/a.db"));
            s.touch(Path::new("/b.db"));
        });
        store.save().unwrap();
        let (again, warn) = Store::<RememberedState>::load(&dirs, "dbmanager", "state").unwrap();
        assert!(warn.is_none());
        assert_eq!(again.get().recent, store.get().recent);
        assert!(tmp.path().join("dbmanager").join("state.json").exists());
    }
}
