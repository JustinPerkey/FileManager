//! Tool registry.
//!
//! Tauri's `Builder::invoke_handler` and `Builder::setup` each replace any
//! earlier call, so a second tool calling them would silently drop the first
//! tool's commands. The rules:
//!
//! - `register` is the only place in this crate that calls `invoke_handler`
//!   or `setup` on the builder, and it calls each at most once.
//! - Each tool is a module `tools/<tool>.rs` contributing only its
//!   `#[tauri::command]` functions (named `<tool>_...`), a constructor for its
//!   managed state if it has any, and optionally `pub(super) fn setup(app)`.
//!   A tool module never receives or returns the `Builder`.
//! - Tools never import each other; shared code lives in `fm-core`.
//!
//! The shape, as `register` has it:
//!
//! ```ignore
//! builder
//!     .manage(tarpack::state())
//!     .invoke_handler(tauri::generate_handler![
//!         tarpack::tarpack_session,
//!         tarpack::tarpack_build,
//!     ])
//!     .setup(|app| {
//!         tarpack::setup(app)?;
//!         Ok(())
//!     })
//! ```
//!
//! The Tar Packager needs no `setup`: its watcher takes an `AppHandle` from
//! the commands that open or reload a manifest.

pub(crate) mod schedule;
pub(crate) mod tarpack;

use tauri::{Builder, Wry};

/// Registers every tool on the builder.
pub fn register(builder: Builder<Wry>) -> Builder<Wry> {
    builder
        .manage(tarpack::state())
        .manage(schedule::state())
        .invoke_handler(tauri::generate_handler![
            tarpack::tarpack_session,
            tarpack::tarpack_open_manifest,
            tarpack::tarpack_reload_manifest,
            tarpack::tarpack_assign_dropped,
            tarpack::tarpack_assign,
            tarpack::tarpack_clear,
            tarpack::tarpack_set_output,
            tarpack::tarpack_set_format,
            tarpack::tarpack_build,
            tarpack::tarpack_recent_manifests,
            tarpack::tarpack_open_in_editor,
            tarpack::tarpack_reveal_output,
            tarpack::tarpack_create_manifest_from_example,
            schedule::schedule_session,
            schedule::schedule_set_text,
            schedule::schedule_set_xml,
            schedule::schedule_set_dropped,
            schedule::schedule_apply,
        ])
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).expect("read_dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                rs_files(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    /// Lines of `text` that are not comments: every line whose trimmed start is
    /// `//` (which covers `//`, `///`, and `//!`) is dropped. Block comments are
    /// not handled.
    fn code_lines(text: &str) -> impl Iterator<Item = &str> {
        text.lines().filter(|l| !l.trim_start().starts_with("//"))
    }

    fn count(text: &str, hook: &str) -> usize {
        code_lines(text).map(|l| l.matches(hook).count()).sum()
    }

    /// Only `tools::register` may call `.invoke_handler(` or `.setup(`, and it
    /// may call each at most once.
    ///
    /// Comment lines are skipped in every file, so writing the method names in
    /// comments (like the `//!` example above) is fine. In `tools/mod.rs` only
    /// the text before `#[cfg(test)]` is scanned, so this test's own string
    /// literals are not counted.
    #[test]
    fn builder_hooks_only_in_tool_registry() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let registry = src.join("tools").join("mod.rs");
        let mut files = Vec::new();
        rs_files(&src, &mut files);
        assert!(files.contains(&registry));
        for file in files {
            let text = fs::read_to_string(&file).expect("read");
            if file == registry {
                let code = text.split("#[cfg(test)]").next().expect("split");
                for hook in [".invoke_handler(", ".setup("] {
                    assert!(
                        count(code, hook) <= 1,
                        "tools/mod.rs calls `{hook}` more than once"
                    );
                }
            } else {
                for hook in [".invoke_handler(", ".setup("] {
                    assert!(
                        count(&text, hook) == 0,
                        "{} calls `{hook}`; only tools::register may",
                        file.display()
                    );
                }
            }
        }
    }

    /// The capability file grants exactly what the webview uses.
    #[test]
    fn capabilities_are_minimal() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities/default.json");
        let json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(path).expect("read")).expect("parse");
        let mut granted: Vec<&str> = json["permissions"]
            .as_array()
            .expect("permissions")
            .iter()
            .map(|p| p.as_str().expect("string permission"))
            .collect();
        granted.sort_unstable();
        assert_eq!(
            granted,
            [
                "core:event:allow-listen",
                "core:event:allow-unlisten",
                "dialog:allow-open",
                "dialog:allow-save",
            ]
        );
    }
}
