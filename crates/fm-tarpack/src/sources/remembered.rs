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

/// The key for a manifest path: canonical, and on Windows without the `\\?\`
/// prefix and lowercased. The string is a key, not for display.
fn key(path: &Path) -> String {
    let canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let s = canon.to_string_lossy().into_owned();
    if cfg!(windows) {
        s.strip_prefix(r"\\?\").unwrap_or(&s).to_lowercase()
    } else {
        s
    }
}

impl RememberedState {
    /// Stores the sources, output path and format for this manifest. The
    /// sources replace what was there, which prunes ids the caller left out.
    pub fn remember(
        &mut self,
        manifest_path: &Path,
        assignments: &Assignments,
        output: Option<&Path>,
        format: ArchiveFormat,
    ) {
        self.per_manifest.insert(
            key(manifest_path),
            ManifestMemory {
                sources: assignments.map().clone(),
                last_output: output.map(Path::to_path_buf),
                last_format: Some(format),
            },
        );
    }

    /// Remembered sources for ids that are still in the manifest.
    pub fn restore(&self, manifest_path: &Path, manifest: &Manifest) -> Assignments {
        let mut out = Assignments::new();
        if let Some(mem) = self.per_manifest.get(&key(manifest_path)) {
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
            self.per_manifest
                .get(&key(manifest_path))
                .map(|m| m.sources.clone())
                .unwrap_or_default(),
        )
    }

    pub fn restore_output(&self, manifest_path: &Path) -> Option<PathBuf> {
        self.per_manifest
            .get(&key(manifest_path))
            .and_then(|m| m.last_output.clone())
    }

    /// The remembered format, else the one the manifest's `output_name` implies.
    pub fn restore_format(&self, manifest_path: &Path, manifest: &Manifest) -> ArchiveFormat {
        self.per_manifest
            .get(&key(manifest_path))
            .and_then(|m| m.last_format)
            .unwrap_or_else(|| manifest.default_format())
    }

    /// Moves `path` to the front of the recent list, capped at [`MAX_RECENT`].
    pub fn touch_recent(&mut self, path: &Path) {
        let k = key(path);
        self.recent_manifests.retain(|p| key(p) != k);
        self.recent_manifests.insert(0, path.to_path_buf());
        self.recent_manifests.truncate(MAX_RECENT);
    }
}
