//! The one place that resolves the real per-user folders. Tests and the
//! screenshot harness must never reach them, so in those builds these
//! functions panic; only `run()` with the real backends gets here. The main
//! `filemanager-gpui` binary built with `--features shoot` therefore panics at
//! startup by design (it would reach the real folders); only
//! `filemanager-shoot` is meant to run with that feature.

use std::path::PathBuf;

use fm_core::AppDirs;

#[cfg(any(test, feature = "shoot"))]
const GUARD: &str = "tests and screenshots must not use the real user folders; pass a temp-dir AppDirs or a fixture backend";

/// The per-user app data folders.
///
/// # Errors
/// If the system has no per-user data directory.
///
/// # Panics
/// In test and screenshot builds, always.
pub fn user_app_dirs() -> fm_core::Result<AppDirs> {
    #[cfg(any(test, feature = "shoot"))]
    panic!("{GUARD}");
    #[cfg(not(any(test, feature = "shoot")))]
    AppDirs::from_system()
}

/// The system-wide config folder (for the Database Manager's config file).
///
/// # Errors
/// If the system has no config folder.
///
/// # Panics
/// In test and screenshot builds, always.
pub fn system_config_path() -> fm_core::Result<PathBuf> {
    #[cfg(any(test, feature = "shoot"))]
    panic!("{GUARD}");
    #[cfg(not(any(test, feature = "shoot")))]
    fm_core::system_config_dir()
}

/// The user's Documents folder, else their home folder.
///
/// In test and screenshot builds this returns a fixed fake path and never
/// reads the real user-dirs configuration (view tests open dialogs, so it
/// must not panic).
#[must_use]
pub fn user_documents_dir() -> Option<PathBuf> {
    #[cfg(any(test, feature = "shoot"))]
    {
        Some(PathBuf::from(if cfg!(windows) {
            r"C:\Users\fixture\Documents"
        } else {
            "/home/fixture/Documents"
        }))
    }
    #[cfg(not(any(test, feature = "shoot")))]
    {
        let dirs = directories::UserDirs::new()?;
        Some(dirs.document_dir().map_or_else(
            || dirs.home_dir().to_path_buf(),
            std::path::Path::to_path_buf,
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    #[test]
    #[should_panic(expected = "tests and screenshots must not use the real user folders")]
    fn user_app_dirs_panics_in_tests() {
        let _ = super::user_app_dirs();
    }

    #[test]
    fn user_documents_dir_is_fake_in_tests() {
        let dir = super::user_documents_dir().expect("fake dir");
        assert!(dir.ends_with("Documents"));
        assert!(dir.to_string_lossy().contains("fixture"));
    }

    fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("read dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                rs_files(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    #[test]
    fn no_direct_system_dirs() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        rs_files(&src, &mut files);
        let me = src.join("app").join("dirs.rs");
        let mut bad = Vec::new();
        for file in files.into_iter().filter(|f| *f != me) {
            let text = std::fs::read_to_string(&file).expect("read source");
            for (n, line) in text.lines().enumerate() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                if [
                    "AppDirs::from_system",
                    "system_config_dir(",
                    "UserDirs::new",
                    "BaseDirs::new",
                ]
                .iter()
                .any(|name| line.contains(name))
                {
                    bad.push(format!("{}:{}", file.display(), n + 1));
                }
            }
        }
        assert!(bad.is_empty(), "use app::dirs instead: {bad:?}");
    }
}
