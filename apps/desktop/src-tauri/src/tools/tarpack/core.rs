//! The Tar Packager's session logic as plain functions over `AppDirs`, with no
//! Tauri types, so it is tested without an app. The command handlers in
//! `mod.rs` are thin glue over [`Core`].

use std::ffi::OsStr;
use std::fs;
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};

use fm_core::{AppDirs, Store};
use fm_tarpack::archive::{ArchivePlan, BuildSummary, Progress};
use fm_tarpack::format::ArchiveFormat;
use fm_tarpack::manifest::{load, LoadedManifest, Manifest, ManifestView};
use fm_tarpack::sources::{
    apply, match_dropped, Assignments, DropOutcome, EntryStatus, RememberedState,
};

use super::types::{
    ArchiveFormatOption, BuildBlockedReason, SessionEntry, SessionManifest, TarpackError,
    TarpackErrorKind as K, TarpackSession,
};

/// The bundled example manifest written by `create_from_example`.
const EXAMPLE_MANIFEST: &str = include_str!("../../../../../../examples/tarpack/example.toml");

struct Loaded {
    manifest: LoadedManifest,
    assignments: Assignments,
    output: Option<PathBuf>,
    format: ArchiveFormat,
}

/// Session state: the loaded manifest, its assignments, the output and format,
/// and what is remembered between runs. Lives in Tauri-managed state.
pub struct Core {
    store: Option<Store<RememberedState>>,
    remembered: RememberedState,
    state_warning: Option<String>,
    restored: bool,
    loaded: Option<Loaded>,
    building: bool,
    last_built: Option<PathBuf>,
    /// Bumped whenever the loaded manifest or its assignments change, so a
    /// drop matched off-lock can tell it went stale.
    revision: u64,
}

/// Everything a drop needs, cloned out of the session so the folder walk
/// runs without it. `Send + 'static`.
pub struct DropJob {
    manifest: Manifest,
    assignments: Assignments,
    revision: u64,
}

impl DropJob {
    /// Walks and matches. Touches no session state.
    pub fn run(&self, paths: &[PathBuf]) -> DropOutcome {
        match_dropped(&self.manifest, &self.assignments, paths)
    }
}

/// A build that passed every check, ready to run without holding the session.
pub struct BuildJob {
    plan: ArchivePlan,
    out: PathBuf,
    format: ArchiveFormat,
}

impl BuildJob {
    /// Writes the archive. `progress` gets M4's raw events.
    pub fn run(
        &self,
        overwrite: bool,
        progress: impl FnMut(Progress),
    ) -> Result<BuildSummary, TarpackError> {
        fm_tarpack::write_archive(&self.plan, &self.out, self.format, overwrite, progress)
            .map_err(Into::into)
    }
}

fn display(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// `path` with its file name's archive suffix replaced by `format`'s
/// extension. Works on the encoded name, never lossily.
pub(super) fn with_format(path: &Path, format: ArchiveFormat) -> PathBuf {
    match path.file_name() {
        Some(name) => path.with_file_name(format.with_extension(name)),
        None => path.to_path_buf(),
    }
}

fn io_error(what: &str, path: &Path, e: &std::io::Error) -> TarpackError {
    TarpackError::new(K::Io, format!("{what} {}: {e}", path.display()))
}

/// Creates `path` (never replacing a file), runs `write` on it, and syncs it.
/// If `write` or the sync fails, removes the file it created.
pub(super) fn write_new_file(
    path: &Path,
    write: impl FnOnce(&mut fs::File) -> io::Result<()>,
) -> Result<(), TarpackError> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| match e.kind() {
            ErrorKind::AlreadyExists => {
                TarpackError::new(K::PathExists, format!("{} already exists", path.display()))
            }
            _ => io_error("creating", path, &e),
        })?;
    if let Err(e) = write(&mut file).and_then(|()| file.sync_all()) {
        drop(file);
        return Err(match fs::remove_file(path) {
            Ok(()) => io_error("writing", path, &e),
            Err(r) => TarpackError::new(
                K::Io,
                format!(
                    "writing {}: {e}; removing the incomplete file also failed ({r}), so it was left at {}",
                    path.display(),
                    display(path)
                ),
            ),
        });
    }
    Ok(())
}

impl Core {
    pub fn new(dirs: AppDirs) -> Core {
        match Store::<RememberedState>::load(&dirs, "tarpack", "state") {
            Ok((store, warning)) => {
                let remembered = store.get().clone();
                Core::with(
                    Some(store),
                    remembered,
                    warning.map(|w| {
                        format!(
                            "{} (the unusable file was kept as {}).",
                            w.reason,
                            w.kept_file.display()
                        )
                    }),
                )
            }
            Err(e) => Core::unpersisted(format!("saved state could not be read: {e}.")),
        }
    }

    /// A session that remembers nothing between runs, with `warning` shown.
    pub fn unpersisted(warning: String) -> Core {
        Core::with(None, RememberedState::default(), Some(warning))
    }

    fn with(
        store: Option<Store<RememberedState>>,
        remembered: RememberedState,
        state_warning: Option<String>,
    ) -> Core {
        Core {
            store,
            remembered,
            state_warning,
            restored: false,
            loaded: None,
            building: false,
            last_built: None,
            revision: 0,
        }
    }

    fn persist(&mut self) {
        let Some(store) = self.store.as_mut() else {
            return;
        };
        let snapshot = self.remembered.clone();
        store.update(|s| *s = snapshot);
        if let Err(e) = store.save() {
            self.add_warning(format!("saved state could not be written: {e}."));
        }
    }

    /// Adds a warning sentence after any earlier one, once.
    pub(super) fn add_warning(&mut self, text: String) {
        match &mut self.state_warning {
            None => self.state_warning = Some(text),
            Some(existing) => {
                if !existing.contains(&text) {
                    existing.push(' ');
                    existing.push_str(&text);
                }
            }
        }
    }

    fn loaded(&self) -> Result<&Loaded, TarpackError> {
        self.loaded
            .as_ref()
            .ok_or_else(|| TarpackError::new(K::NoManifest, "no manifest is loaded"))
    }

    fn loaded_mut(&mut self) -> Result<&mut Loaded, TarpackError> {
        self.loaded
            .as_mut()
            .ok_or_else(|| TarpackError::new(K::NoManifest, "no manifest is loaded"))
    }

    /// The loaded manifest's path, for the watcher and the editor.
    pub fn manifest_path(&self) -> Option<&Path> {
        self.loaded.as_ref().map(|l| l.manifest.path.as_path())
    }

    /// The snapshot; on the first call, restores the most recent manifest.
    pub fn session(&mut self) -> TarpackSession {
        if !self.restored {
            self.restored = true;
            if let Some(recent) = self.remembered.recent_manifests.first().cloned() {
                if let Err(e) = self.load_manifest(&recent) {
                    // Nothing is loaded; say why. The recent list is kept: the
                    // file may be on a drive that is only temporarily away.
                    self.add_warning(format!(
                        "the last manifest, {}, could not be reopened: {}.",
                        recent.display(),
                        e.message.trim_end_matches('.')
                    ));
                }
            }
        }
        self.snapshot()
    }

    pub fn recent_manifests(&self) -> Vec<String> {
        self.remembered
            .recent_manifests
            .iter()
            .map(|p| display(p))
            .collect()
    }

    fn load_manifest(&mut self, path: &Path) -> Result<(), TarpackError> {
        let manifest =
            load(path).map_err(|e| TarpackError::new(K::ManifestUnreadable, e.to_string()))?;
        let report = &manifest.report;
        let assignments = if report.is_valid() {
            self.remembered.restore(path, &report.manifest)
        } else {
            // Do not erase a failed entry's remembered file over a typo.
            self.remembered.restore_all(path)
        };
        let format = self.remembered.restore_format(path, &report.manifest);
        let output = self
            .remembered
            .restore_output(path)
            .map(|p| with_format(&p, format));
        self.revision += 1;
        self.loaded = Some(Loaded {
            manifest,
            assignments,
            output,
            format,
        });
        self.remembered.touch_recent(path);
        self.persist();
        Ok(())
    }

    /// Loads `path`, restores what is remembered for it, and makes it recent.
    /// Succeeds for any readable file, whatever its errors.
    pub fn open(&mut self, path: &Path) -> Result<(), TarpackError> {
        self.restored = true;
        self.load_manifest(path)
    }

    /// Re-reads the loaded manifest, keeping assignments by id.
    pub fn reload(&mut self) -> Result<(), TarpackError> {
        let path = self.loaded()?.manifest.path.clone();
        let manifest =
            load(&path).map_err(|e| TarpackError::new(K::ManifestUnreadable, e.to_string()))?;
        let valid = manifest.report.is_valid();
        let l = self.loaded_mut()?;
        if valid {
            let mut kept = Assignments::new();
            for e in manifest.report.manifest.entries() {
                if let Some(p) = l.assignments.get(e.id()) {
                    kept.insert(e.id(), p);
                }
            }
            l.assignments = kept;
        }
        l.manifest = manifest;
        self.revision += 1;
        self.remember();
        Ok(())
    }

    fn remember(&mut self) {
        if let Some(l) = &self.loaded {
            self.remembered.remember(
                &l.manifest.path,
                &l.assignments,
                l.output.as_deref(),
                l.format,
            );
        }
        self.persist();
    }

    fn require_entry(&self, id: &str) -> Result<(), TarpackError> {
        let l = self.loaded()?;
        if l.manifest
            .report
            .manifest
            .entries()
            .iter()
            .any(|e| e.id() == id)
        {
            Ok(())
        } else {
            Err(TarpackError::for_entry(
                K::UnknownEntry,
                format!("the manifest has no entry `{id}` that can be built"),
                id,
            ))
        }
    }

    pub fn assign(&mut self, id: &str, source: &Path) -> Result<(), TarpackError> {
        self.require_entry(id)?;
        if !source.is_file() {
            return Err(TarpackError::for_entry(
                K::NotAFile,
                format!("{} is not an existing file", source.display()),
                id,
            ));
        }
        self.loaded_mut()?.assignments.insert(id, source);
        self.revision += 1;
        self.remember();
        Ok(())
    }

    pub fn clear(&mut self, id: &str) -> Result<(), TarpackError> {
        self.require_entry(id)?;
        self.loaded_mut()?.assignments.remove(id);
        self.revision += 1;
        self.remember();
        Ok(())
    }

    /// Clones out what a drop needs. Pair with [`Core::finish_drop`].
    pub fn begin_drop(&self) -> Result<DropJob, TarpackError> {
        let l = self.loaded()?;
        Ok(DropJob {
            manifest: l.manifest.report.manifest.clone(),
            assignments: l.assignments.clone(),
            revision: self.revision,
        })
    }

    /// Applies a drop matched off-lock. `Ok(None)` when the session changed
    /// meanwhile: nothing is applied or persisted.
    pub fn finish_drop(
        &mut self,
        job: &DropJob,
        outcome: DropOutcome,
    ) -> Result<Option<DropOutcome>, TarpackError> {
        let revision = self.revision;
        let l = self.loaded_mut()?;
        if revision != job.revision {
            return Ok(None);
        }
        apply(&mut l.assignments, &outcome);
        self.revision += 1;
        self.remember();
        Ok(Some(outcome))
    }

    pub fn set_format(&mut self, format: ArchiveFormat) -> Result<(), TarpackError> {
        let l = self.loaded_mut()?;
        l.format = format;
        l.output = l.output.take().map(|p| with_format(&p, format));
        self.remember();
        Ok(())
    }

    /// Sets the output. A name that carries a known archive suffix switches
    /// the format to it (the user typed that name); otherwise the current
    /// extension is appended. Either way the path ends with the extension of
    /// the resulting format.
    pub fn set_output(&mut self, path: &Path) -> Result<(), TarpackError> {
        let l = self.loaded_mut()?;
        let Some(name) = path.file_name() else {
            return Err(TarpackError::new(
                K::Io,
                format!("{} has no file name", path.display()),
            ));
        };
        if let Some(f) = ArchiveFormat::from_file_name(name) {
            l.format = f;
        }
        l.output = Some(with_format(path, l.format));
        self.remember();
        Ok(())
    }

    /// Writes the bundled example manifest to a new file and opens it.
    pub fn create_from_example(&mut self, path: &Path) -> Result<(), TarpackError> {
        write_new_file(path, |f| f.write_all(EXAMPLE_MANIFEST.as_bytes()))?;
        self.open(path)
    }

    /// Checks everything a build needs and marks it running. Pair with
    /// [`Core::finish_build`].
    pub fn begin_build(&mut self) -> Result<BuildJob, TarpackError> {
        let l = self.loaded()?;
        if l.manifest.report.manifest.entries().is_empty() {
            return Err(TarpackError::new(
                K::NoEntries,
                "no entry passed validation, so there is nothing to build",
            ));
        }
        if self.building {
            return Err(TarpackError::new(
                K::BuildInProgress,
                "a build is already running",
            ));
        }
        // The user must get the archive they saw.
        let current = load(&l.manifest.path).map_err(|e| {
            TarpackError::new(
                K::ManifestChangedOnDisk,
                format!("the manifest can no longer be read: {e}"),
            )
        })?;
        if current.sha256 != l.manifest.sha256 {
            return Err(TarpackError::new(
                K::ManifestChangedOnDisk,
                "the manifest changed on disk since it was loaded; reload it",
            ));
        }
        let out = l
            .output
            .clone()
            .ok_or_else(|| TarpackError::new(K::NoOutput, "no output path is set"))?;
        let plan = ArchivePlan::new(&l.manifest.report, &l.assignments)?;
        let job = BuildJob {
            plan,
            out,
            format: l.format,
        };
        self.building = true;
        Ok(job)
    }

    /// Ends a build begun by [`Core::begin_build`]; on success keeps the path
    /// written, for `reveal_output`.
    pub fn finish_build(&mut self, job: &BuildJob, result: &Result<BuildSummary, TarpackError>) {
        self.building = false;
        if result.is_ok() {
            self.last_built = Some(job.out.clone());
        }
    }

    /// Releases the session after a build that never reported back.
    pub fn abort_build(&mut self) {
        self.building = false;
    }

    /// The manifest path for opening in an editor.
    pub fn manifest_path_for_open(&self) -> Result<PathBuf, TarpackError> {
        Ok(self.loaded()?.manifest.path.clone())
    }

    /// The path the last successful build wrote, never a string from the UI.
    pub fn reveal_target(&self) -> Result<PathBuf, TarpackError> {
        self.last_built.clone().ok_or_else(|| {
            TarpackError::new(K::NoOutput, "no archive has been built in this session")
        })
    }

    pub fn snapshot(&self) -> TarpackSession {
        let formats = ArchiveFormatOption::all();
        let state_warning = self.state_warning.clone();
        let Some(l) = &self.loaded else {
            return TarpackSession {
                manifest: None,
                output_path: None,
                format: ArchiveFormat::Tar,
                formats,
                suggested_output_name: None,
                ready_count: 0,
                total_count: 0,
                can_build: false,
                build_blocked_reason: Some(BuildBlockedReason::NoManifest),
                state_warning,
            };
        };
        let report = &l.manifest.report;
        let view = ManifestView::from(&report.manifest);
        let stem = l.manifest.path.file_stem();
        let name = if view.name.is_empty() {
            stem.map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        } else {
            view.name
        };
        let suggested_base: &OsStr = match (&view.output_name, stem) {
            (Some(n), _) => n.as_ref(),
            (None, Some(s)) => s,
            (None, None) => OsStr::new("archive"),
        };
        let suggested = l.format.with_extension(suggested_base);

        let entries: Vec<SessionEntry> = view
            .entries
            .into_iter()
            .map(|v| SessionEntry {
                assigned: l.assignments.get(&v.id).map(display),
                status: l.assignments.status(&v.id),
                view: v,
            })
            .collect();
        let total = entries.len() as u32;
        let ready = entries
            .iter()
            .filter(|e| e.status == EntryStatus::Ready)
            .count() as u32;
        let reason = if entries.is_empty() {
            Some(BuildBlockedReason::NoEntries)
        } else if entries.iter().any(|e| e.status == EntryStatus::Missing) {
            Some(BuildBlockedReason::EntriesNotReady)
        } else if ready == 0 {
            Some(BuildBlockedReason::NothingLoaded)
        } else if l.output.is_none() {
            Some(BuildBlockedReason::NoOutput)
        } else {
            None
        };
        TarpackSession {
            manifest: Some(SessionManifest {
                path: display(&l.manifest.path),
                name,
                output_name: view.output_name,
                hash: l
                    .manifest
                    .sha256
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect(),
                entries,
                entries_withheld: report.entries_withheld,
                errors: report.errors.clone(),
                failed_entries: report.failures.clone(),
                error_count: report.error_count(),
                warnings: report.warnings.clone(),
            }),
            output_path: l.output.as_deref().map(display),
            format: l.format,
            formats,
            suggested_output_name: Some(suggested.to_string_lossy().into_owned()),
            ready_count: ready,
            total_count: total,
            can_build: reason.is_none(),
            build_blocked_reason: reason,
            state_warning,
        }
    }
}
