//! Namespaced, versioned JSON state store.

use crate::atomic::atomic_write;
use crate::dirs::{is_valid_name, AppDirs};
use crate::error::{FmError, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A type that can be persisted in a [`Store`].
pub trait StateData: Serialize + DeserializeOwned + Default {
    /// Current schema version of this type's on-disk form.
    const SCHEMA_VERSION: u32;
}

/// Reported when an unusable state file was set aside and defaults were used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreWarning {
    /// Where the unusable file was kept.
    pub kept_file: PathBuf,
    /// Why it was set aside.
    pub reason: String,
}

#[derive(Deserialize)]
struct Envelope {
    schema_version: u32,
    data: serde_json::Value,
}

#[derive(Serialize)]
struct EnvelopeRef<'a, T> {
    schema_version: u32,
    data: &'a T,
}

/// A single JSON state file at `<root>/<tool>/<name>.json`.
#[derive(Debug)]
pub struct Store<T: StateData> {
    path: PathBuf,
    data: T,
}

impl<T: StateData> Store<T> {
    /// Load state, falling back to defaults (with a warning) for unusable files.
    /// Older schema versions are treated as unusable; see [`Store::load_with_migration`].
    pub fn load(dirs: &AppDirs, tool: &str, name: &str) -> Result<(Self, Option<StoreWarning>)> {
        Self::load_with_migration(dirs, tool, name, |_, _| None)
    }

    /// Like [`Store::load`], but `migrate(old_version, data)` may upgrade data
    /// from a lower schema version. Returning `None` keeps and resets the file.
    pub fn load_with_migration(
        dirs: &AppDirs,
        tool: &str,
        name: &str,
        migrate: impl FnOnce(u32, serde_json::Value) -> Option<T>,
    ) -> Result<(Self, Option<StoreWarning>)> {
        if !is_valid_name(name) {
            return Err(FmError::InvalidStateName(name.to_owned()));
        }
        let path = dirs.tool_dir(tool)?.join(format!("{name}.json"));
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok((Self::with(path, T::default()), None));
            }
            Err(e) => return Err(FmError::io("reading", path, e)),
        };

        let outcome: std::result::Result<T, String> =
            match serde_json::from_slice::<Envelope>(&bytes) {
                Err(e) => Err(format!("corrupt state file: {e}")),
                Ok(env) if env.schema_version > T::SCHEMA_VERSION => Err(format!(
                    "state file schema version {} is newer than supported {}",
                    env.schema_version,
                    T::SCHEMA_VERSION
                )),
                Ok(env) if env.schema_version == T::SCHEMA_VERSION => {
                    serde_json::from_value(env.data)
                        .map_err(|e| format!("state data does not match schema: {e}"))
                }
                Ok(env) => {
                    let v = env.schema_version;
                    migrate(v, env.data)
                        .ok_or_else(|| format!("no migration from schema version {v}"))
                }
            };

        match outcome {
            Ok(data) => Ok((Self::with(path, data), None)),
            Err(reason) => {
                let kept = keep_bad_file(&path, name)?;
                let warning = StoreWarning {
                    kept_file: kept,
                    reason,
                };
                Ok((Self::with(path, T::default()), Some(warning)))
            }
        }
    }

    fn with(path: PathBuf, data: T) -> Self {
        Self { path, data }
    }

    /// The current state.
    pub fn get(&self) -> &T {
        &self.data
    }

    /// Mutate the state in memory. Call [`Store::save`] to persist.
    pub fn update(&mut self, f: impl FnOnce(&mut T)) {
        f(&mut self.data);
    }

    /// Atomically persist the state.
    pub fn save(&self) -> Result<()> {
        let env = EnvelopeRef {
            schema_version: T::SCHEMA_VERSION,
            data: &self.data,
        };
        let bytes = serde_json::to_vec_pretty(&env)?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| FmError::io("creating", parent, e))?;
        }
        atomic_write(&self.path, &bytes)
    }
}

/// Rename `path` to `<name>.bad-<UTC timestamp>.json` beside it, never
/// replacing an existing file.
fn keep_bad_file(path: &Path, name: &str) -> Result<PathBuf> {
    let now = time::OffsetDateTime::now_utc();
    let stamp = format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    );
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut target = dir.join(format!("{name}.bad-{stamp}.json"));
    let mut n = 1;
    while target.exists() {
        target = dir.join(format!("{name}.bad-{stamp}-{n}.json"));
        n += 1;
    }
    std::fs::rename(path, &target)
        .map_err(|e| FmError::io("keeping unusable state file", path, e))?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    struct Demo {
        items: Vec<String>,
    }
    impl StateData for Demo {
        const SCHEMA_VERSION: u32 = 2;
    }

    fn setup() -> (TempDir, AppDirs) {
        let td = TempDir::new().unwrap();
        let dirs = AppDirs::at(td.path().to_path_buf());
        (td, dirs)
    }

    fn bad_files(dir: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.to_string_lossy().contains(".bad-"))
            .collect()
    }

    #[test]
    fn store_round_trips() {
        let (_td, dirs) = setup();
        let (mut s, w) = Store::<Demo>::load(&dirs, "demo", "state").unwrap();
        assert!(w.is_none());
        s.update(|d| d.items.push("a".into()));
        s.save().unwrap();
        let (s2, w) = Store::<Demo>::load(&dirs, "demo", "state").unwrap();
        assert!(w.is_none());
        assert_eq!(s2.get().items, vec!["a".to_string()]);
    }

    #[test]
    fn missing_file_gives_default_without_warning() {
        let (_td, dirs) = setup();
        let (s, w) = Store::<Demo>::load(&dirs, "demo", "state").unwrap();
        assert_eq!(s.get(), &Demo::default());
        assert!(w.is_none());
    }

    #[test]
    fn corrupt_state_is_preserved_and_reset() {
        let (_td, dirs) = setup();
        let dir = dirs.tool_dir("demo").unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("state.json"), b"{ not json").unwrap();
        let (s, w) = Store::<Demo>::load(&dirs, "demo", "state").unwrap();
        assert_eq!(s.get(), &Demo::default());
        let kept = w.unwrap().kept_file;
        assert_eq!(std::fs::read(&kept).unwrap(), b"{ not json");
        assert!(!dir.join("state.json").exists());
    }

    #[test]
    fn newer_schema_is_not_overwritten() {
        let (_td, dirs) = setup();
        let dir = dirs.tool_dir("demo").unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        let body = br#"{"schema_version":99,"data":{"items":["x"]}}"#;
        std::fs::write(dir.join("state.json"), body).unwrap();
        let (s, w) = Store::<Demo>::load(&dirs, "demo", "state").unwrap();
        let kept = w.unwrap().kept_file;
        s.save().unwrap();
        assert_eq!(std::fs::read(kept).unwrap(), body);
        assert_eq!(bad_files(&dir).len(), 1);
    }

    #[test]
    fn older_schema_migrates_or_is_kept() {
        let (_td, dirs) = setup();
        let dir = dirs.tool_dir("demo").unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        let body = br#"{"schema_version":1,"data":["x"]}"#;
        std::fs::write(dir.join("state.json"), body).unwrap();
        let (s, w) = Store::<Demo>::load_with_migration(&dirs, "demo", "state", |v, d| {
            assert_eq!(v, 1);
            Some(Demo {
                items: serde_json::from_value(d).ok()?,
            })
        })
        .unwrap();
        assert!(w.is_none());
        assert_eq!(s.get().items, vec!["x".to_string()]);
        let (_s, w) = Store::<Demo>::load(&dirs, "demo", "state").unwrap();
        assert!(w.is_some()); // no migration supplied: kept and reset
    }

    #[test]
    fn namespaces_are_isolated() {
        let (_td, dirs) = setup();
        let (mut a, _) = Store::<Demo>::load(&dirs, "alpha", "state").unwrap();
        a.update(|d| d.items.push("a".into()));
        a.save().unwrap();
        let (b, _) = Store::<Demo>::load(&dirs, "beta", "state").unwrap();
        assert!(b.get().items.is_empty());
    }

    #[test]
    fn invalid_tool_name_rejected() {
        let (_td, dirs) = setup();
        for bad in ["", "Tar", "1a", "a/b", "..", "a_b", "a b"] {
            assert!(
                matches!(dirs.tool_dir(bad), Err(FmError::InvalidToolName(_))),
                "{bad}"
            );
            assert!(Store::<Demo>::load(&dirs, bad, "state").is_err());
        }
        assert!(dirs.tool_dir("tar-pack2").is_ok());
    }

    #[test]
    fn atomic_write_leaves_no_partial_file_on_error() {
        let (td, _dirs) = setup();
        let blocker = td.path().join("file");
        std::fs::write(&blocker, b"x").unwrap();
        assert!(atomic_write(&blocker.join("child.json"), b"data").is_err());
        // Persist failure: target is a non-empty directory.
        let target = td.path().join("dir");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("f"), b"x").unwrap();
        assert!(atomic_write(&target, b"data").is_err());
        let names: Vec<_> = std::fs::read_dir(td.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 2, "{names:?}");
    }

    fn temp_leftovers(dir: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.file_name().unwrap() != "state.json")
            .collect()
    }

    #[test]
    fn second_save_replaces_existing_file() {
        let (_td, dirs) = setup();
        let (mut s, _) = Store::<Demo>::load(&dirs, "demo", "state").unwrap();
        s.update(|d| d.items = vec!["one".into()]);
        s.save().unwrap();
        s.update(|d| d.items = vec!["two".into()]);
        s.save().unwrap();
        let (s2, w) = Store::<Demo>::load(&dirs, "demo", "state").unwrap();
        assert!(w.is_none());
        assert_eq!(s2.get().items, vec!["two".to_string()]);
        assert!(temp_leftovers(&dirs.tool_dir("demo").unwrap()).is_empty());
    }

    #[test]
    fn atomic_write_replaces_existing_file() {
        let td = TempDir::new().unwrap();
        let p = td.path().join("f.txt");
        std::fs::write(&p, b"old").unwrap();
        atomic_write(&p, b"new").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"new");
        assert_eq!(std::fs::read_dir(td.path()).unwrap().count(), 1);
    }

    #[test]
    fn failed_write_leaves_existing_target_intact() {
        // Parent directory is read-only-free approach: target is a directory
        // holding a file, so the final rename fails; its contents survive.
        let td = TempDir::new().unwrap();
        let target = td.path().join("dir");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("keep"), b"keep").unwrap();
        assert!(atomic_write(&target, b"data").is_err());
        assert_eq!(std::fs::read(target.join("keep")).unwrap(), b"keep");
    }

    #[test]
    fn mismatched_data_at_current_version_is_kept_and_reset() {
        let (_td, dirs) = setup();
        let dir = dirs.tool_dir("demo").unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        let body = br#"{"schema_version":2,"data":{"items":"not a list"}}"#;
        std::fs::write(dir.join("state.json"), body).unwrap();
        let (s, w) = Store::<Demo>::load(&dirs, "demo", "state").unwrap();
        assert_eq!(s.get(), &Demo::default());
        assert_eq!(std::fs::read(w.unwrap().kept_file).unwrap(), body);
        assert!(!dir.join("state.json").exists());
    }

    #[test]
    fn invalid_state_name_rejected() {
        let (_td, dirs) = setup();
        for bad in ["", "C:x", "a/b", "..", "A", "a.b"] {
            assert!(matches!(
                Store::<Demo>::load(&dirs, "demo", bad),
                Err(FmError::InvalidStateName(_))
            ));
        }
    }
}
