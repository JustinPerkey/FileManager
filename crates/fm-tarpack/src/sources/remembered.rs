//! What the Tar Packager remembers between runs, per manifest.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fm_core::StateData;
use serde::{Deserialize, Serialize};

use super::assignments::Assignments;
use crate::format::ArchiveFormat;
use crate::manifest::Manifest;

/// The recent-manifests list holds at most this many paths.
pub const MAX_RECENT: usize = 10;

/// One manifest's remembered choices.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestMemory {
    #[serde(default)]
    pub sources: BTreeMap<String, PathBuf>,
    #[serde(default)]
    pub last_output: Option<PathBuf>,
    #[serde(default)]
    pub last_format: Option<ArchiveFormat>,
}

/// Persisted at `<root>/tarpack/state.json` through `fm_core::Store`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RememberedState {
    /// Keyed by canonical manifest path, so renaming or moving a manifest
    /// forgets its memory.
    #[serde(default)]
    pub per_manifest: BTreeMap<String, ManifestMemory>,
    /// Most recent first.
    #[serde(default)]
    pub recent_manifests: Vec<PathBuf>,
}

impl StateData for RememberedState {
    const SCHEMA_VERSION: u32 = 1;
}

/// The key for a manifest path: canonical (or the path as given when it
/// cannot be canonicalized), and on Windows without the `\\?\` / `\\?\UNC\`
/// prefix and lowercased. `None` when the path is not valid Unicode: such a
/// manifest has no memory. The string is a key, not for display.
pub(super) fn key(path: &Path) -> Option<String> {
    let canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let s = canon.to_str()?;
    if cfg!(windows) {
        let s = if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{rest}")
        } else {
            s.strip_prefix(r"\\?\").unwrap_or(s).to_owned()
        };
        Some(s.to_lowercase())
    } else {
        Some(s.to_owned())
    }
}

impl RememberedState {
    fn memory(&self, manifest_path: &Path) -> Option<&ManifestMemory> {
        self.per_manifest.get(&key(manifest_path)?)
    }

    /// Stores the sources, output path and format for this manifest. The
    /// sources replace what was there, which prunes ids the caller left out.
    ///
    /// Defence in depth: a source whose path is not valid Unicode is not
    /// stored, `last_output` is `None` for such an output path, and a manifest
    /// path that is not Unicode remembers nothing. The app's inputs are
    /// already Unicode and the drop boundary reports `NotUnicode`.
    pub fn remember(
        &mut self,
        manifest_path: &Path,
        assignments: &Assignments,
        output: Option<&Path>,
        format: ArchiveFormat,
    ) {
        let Some(k) = key(manifest_path) else { return };
        self.per_manifest.insert(
            k,
            ManifestMemory {
                sources: assignments
                    .map()
                    .iter()
                    .filter(|(_, p)| p.to_str().is_some())
                    .map(|(id, p)| (id.clone(), p.clone()))
                    .collect(),
                last_output: output
                    .filter(|p| p.to_str().is_some())
                    .map(Path::to_path_buf),
                last_format: Some(format),
            },
        );
    }

    /// Remembered sources for ids that are still in the manifest.
    pub fn restore(&self, manifest_path: &Path, manifest: &Manifest) -> Assignments {
        let mut out = Assignments::new();
        if let Some(mem) = self.memory(manifest_path) {
            for e in manifest.entries() {
                if let Some(p) = mem.sources.get(e.id()) {
                    out.insert(e.id(), p.clone());
                }
            }
        }
        out
    }

    /// Every remembered source, including ids the manifest no longer passes.
    /// Use for a manifest with errors, so a typo does not erase a location.
    pub fn restore_all(&self, manifest_path: &Path) -> Assignments {
        Assignments::from_map(
            self.memory(manifest_path)
                .map(|m| m.sources.clone())
                .unwrap_or_default(),
        )
    }

    pub fn restore_output(&self, manifest_path: &Path) -> Option<PathBuf> {
        self.memory(manifest_path)
            .and_then(|m| m.last_output.clone())
    }

    /// The remembered format, else the one the manifest's `output_name` implies.
    pub fn restore_format(&self, manifest_path: &Path, manifest: &Manifest) -> ArchiveFormat {
        self.memory(manifest_path)
            .and_then(|m| m.last_format)
            .unwrap_or_else(|| manifest.default_format())
    }

    /// Moves `path` to the front of the recent list, capped at [`MAX_RECENT`].
    ///
    /// A path that is not valid Unicode is ignored (defence in depth: the
    /// app's inputs are already Unicode, and the list must stay serializable).
    pub fn touch_recent(&mut self, path: &Path) {
        // Check the path as given: `key` looks at the canonical path, which
        // can be Unicode when a symlink to it is not.
        if path.to_str().is_none() {
            return;
        }
        let Some(k) = key(path) else { return };
        self.recent_manifests
            .retain(|p| key(p).as_deref() != Some(k.as_str()));
        self.recent_manifests.insert(0, path.to_path_buf());
        self.recent_manifests.truncate(MAX_RECENT);
    }
}
