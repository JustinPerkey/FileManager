//! The Tar Packager's backend as the model sees it: a trait over
//! [`fm_tarpack::session::Core`], so the model runs against the real session
//! or a fixture. Every method may block (the real one reads and writes
//! files), so the model calls them only through its [`crate::jobs::Worker`];
//! the work objects ([`DropWork`], [`BuildWork`]) run on a thread of their own.

use std::any::Any;
use std::path::{Path, PathBuf};

use fm_core::AppDirs;
use fm_tarpack::archive::{BuildSummary, Progress};
use fm_tarpack::format::ArchiveFormat;
use fm_tarpack::session::{
    BuildJob, Core, DropJob, DroppedAssignment, TarpackError, TarpackErrorKind, TarpackSession,
};
use fm_tarpack::sources::DropOutcome;

/// A drop's folder walk. [`TarpackBackend::begin_drop`] makes it, the model
/// runs it on a thread, and [`TarpackBackend::finish_drop`] takes it back
/// with its outcome.
pub trait DropWork: Send + 'static {
    /// Walks and matches `paths`. Touches no session state.
    fn run(&mut self, paths: &[PathBuf]);

    /// For the backend that made the work to get its own type back in
    /// `finish_drop`.
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

/// A build's write. [`TarpackBackend::begin_build`] makes it, the model runs
/// it on a thread, and [`TarpackBackend::finish_build`] takes it back with its
/// result. Part B drives it.
pub trait BuildWork: Send + 'static {
    /// Writes the archive. `progress` gets the raw events.
    fn run(&mut self, overwrite: bool, progress: &mut dyn FnMut(Progress));

    /// See [`DropWork::into_any`].
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

/// What the Tar Packager model needs from its session: the operations of the
/// Tauri commands, over one session.
pub trait TarpackBackend: Send + 'static {
    /// The current session. The first call restores the most recent
    /// manifest.
    fn session(&mut self) -> TarpackSession;

    /// The recent manifests as display strings, most recent first.
    fn recent_manifests(&mut self) -> Vec<String>;

    /// Loads the manifest at `path`.
    ///
    /// # Errors
    /// `ManifestUnreadable` when the file cannot be read.
    fn open(&mut self, path: &Path) -> Result<TarpackSession, TarpackError>;

    /// Loads the recent manifest at position `index` of
    /// [`TarpackBackend::recent_manifests`], by its stored path, but only if
    /// that entry's display string is still `expected_display`: the model's
    /// copy of the list may be older than the backend's. `Ok(None)` when it
    /// is not (nothing was opened): the model refreshes its list.
    ///
    /// # Errors
    /// `ManifestUnreadable` when there is no such position or the file cannot
    /// be read.
    fn open_recent(
        &mut self,
        index: usize,
        expected_display: &str,
    ) -> Result<Option<TarpackSession>, TarpackError>;

    /// Re-reads the loaded manifest, keeping assignments by id.
    ///
    /// # Errors
    /// `NoManifest`, or `ManifestUnreadable`.
    fn reload(&mut self) -> Result<TarpackSession, TarpackError>;

    /// Writes the bundled example to a new file and loads it. Never replaces
    /// a file.
    ///
    /// # Errors
    /// `PathExists` when `path` exists, or `Io`.
    fn create_from_example(&mut self, path: &Path) -> Result<TarpackSession, TarpackError>;

    /// The file assigned to the entry `id`, as the stored path (a dialog
    /// starts in its folder). `None` when nothing is loaded or it has none.
    fn assigned_path(&self, id: &str) -> Option<PathBuf>;

    /// Assigns the file `path` to the entry `id`.
    ///
    /// # Errors
    /// `NoManifest`, `UnknownEntry` or `NotAFile`.
    fn assign(&mut self, id: &str, path: &Path) -> Result<TarpackSession, TarpackError>;

    /// Clears the assignment of the entry `id`.
    ///
    /// # Errors
    /// `NoManifest` or `UnknownEntry`.
    fn clear(&mut self, id: &str) -> Result<TarpackSession, TarpackError>;

    /// Clones out what a drop needs.
    ///
    /// # Errors
    /// `NoManifest`.
    fn begin_drop(&mut self) -> Result<Box<dyn DropWork>, TarpackError>;

    /// Applies a drop whose work has run. `Ok(None)` when the session changed
    /// meanwhile: nothing was applied, the model tries again.
    ///
    /// # Errors
    /// `NoManifest`, or `Io` when the work never ran.
    fn finish_drop(
        &mut self,
        work: Box<dyn DropWork>,
    ) -> Result<Option<DroppedAssignment>, TarpackError>;

    /// Sets the output (part B).
    ///
    /// # Errors
    /// `NoManifest` or `Io`.
    fn set_output(&mut self, path: &Path) -> Result<TarpackSession, TarpackError>;

    /// Sets the archive format (part B).
    ///
    /// # Errors
    /// `NoManifest`.
    fn set_format(&mut self, format: ArchiveFormat) -> Result<TarpackSession, TarpackError>;

    /// Checks everything a build needs and marks it running (part B).
    ///
    /// # Errors
    /// See `Core::begin_build`.
    fn begin_build(&mut self) -> Result<Box<dyn BuildWork>, TarpackError>;

    /// Ends a build whose work has run (part B).
    ///
    /// # Errors
    /// The build's own error.
    fn finish_build(&mut self, work: Box<dyn BuildWork>) -> Result<BuildSummary, TarpackError>;

    /// Releases the session after a build that never reported back (part B).
    fn abort_build(&mut self);

    /// The loaded manifest's path, for the watcher.
    fn manifest_path(&self) -> Option<PathBuf>;

    /// The manifest path for opening in an editor.
    ///
    /// # Errors
    /// `NoManifest`.
    fn manifest_path_for_open(&self) -> Result<PathBuf, TarpackError>;

    /// The path the last successful build wrote (part B).
    ///
    /// # Errors
    /// `NoOutput`.
    fn reveal_target(&self) -> Result<PathBuf, TarpackError>;
}

struct RealDropWork {
    job: DropJob,
    outcome: Option<DropOutcome>,
}

impl DropWork for RealDropWork {
    fn run(&mut self, paths: &[PathBuf]) {
        self.outcome = Some(self.job.run(paths));
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

struct RealBuildWork {
    job: BuildJob,
    result: Option<Result<BuildSummary, TarpackError>>,
}

impl BuildWork for RealBuildWork {
    fn run(&mut self, overwrite: bool, progress: &mut dyn FnMut(Progress)) {
        self.result = Some(self.job.run(overwrite, progress));
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

fn foreign_work(what: &str) -> TarpackError {
    TarpackError::new(
        TarpackErrorKind::Io,
        format!("the {what} was not made by this session"),
    )
}

/// The real session: [`Core`] over the per-user app data directory.
pub struct RealTarpack {
    core: Core,
}

impl RealTarpack {
    /// The session over the system's app data directory; without one it
    /// remembers nothing and says so as its state warning.
    #[must_use]
    pub fn new() -> Self {
        let core = match AppDirs::from_system() {
            Ok(dirs) => Core::new(&dirs),
            Err(e) => Core::unpersisted(format!("saved state is unavailable: {e}.")),
        };
        Self { core }
    }

    /// The session remembering in `dirs` (tests use a temporary directory).
    #[must_use]
    pub fn with_dirs(dirs: &AppDirs) -> Self {
        Self {
            core: Core::new(dirs),
        }
    }
}

impl Default for RealTarpack {
    fn default() -> Self {
        Self::new()
    }
}

impl TarpackBackend for RealTarpack {
    fn session(&mut self) -> TarpackSession {
        self.core.session()
    }

    fn recent_manifests(&mut self) -> Vec<String> {
        self.core.recent_manifests()
    }

    fn open(&mut self, path: &Path) -> Result<TarpackSession, TarpackError> {
        self.core.open(path)?;
        Ok(self.core.snapshot())
    }

    fn open_recent(
        &mut self,
        index: usize,
        expected_display: &str,
    ) -> Result<Option<TarpackSession>, TarpackError> {
        let path = self
            .core
            .recent_paths()
            .into_iter()
            .nth(index)
            .ok_or_else(|| {
                TarpackError::new(
                    TarpackErrorKind::ManifestUnreadable,
                    "that manifest is no longer in the recent list",
                )
            })?;
        let shown = self.core.recent_manifests();
        if shown.get(index).map(String::as_str) != Some(expected_display) {
            return Ok(None);
        }
        self.open(&path).map(Some)
    }

    fn reload(&mut self) -> Result<TarpackSession, TarpackError> {
        self.core.reload()?;
        Ok(self.core.snapshot())
    }

    fn create_from_example(&mut self, path: &Path) -> Result<TarpackSession, TarpackError> {
        self.core.create_from_example(path)?;
        Ok(self.core.snapshot())
    }

    fn assigned_path(&self, id: &str) -> Option<PathBuf> {
        self.core.assigned_path(id)
    }

    fn assign(&mut self, id: &str, path: &Path) -> Result<TarpackSession, TarpackError> {
        self.core.assign(id, path)?;
        Ok(self.core.snapshot())
    }

    fn clear(&mut self, id: &str) -> Result<TarpackSession, TarpackError> {
        self.core.clear(id)?;
        Ok(self.core.snapshot())
    }

    fn begin_drop(&mut self) -> Result<Box<dyn DropWork>, TarpackError> {
        let job = self.core.begin_drop()?;
        Ok(Box::new(RealDropWork { job, outcome: None }))
    }

    fn finish_drop(
        &mut self,
        work: Box<dyn DropWork>,
    ) -> Result<Option<DroppedAssignment>, TarpackError> {
        let work = work
            .into_any()
            .downcast::<RealDropWork>()
            .map_err(|_| foreign_work("drop"))?;
        let outcome = work.outcome.ok_or_else(|| {
            TarpackError::new(TarpackErrorKind::Io, "the dropped files were never matched")
        })?;
        Ok(self
            .core
            .finish_drop(&work.job, outcome)?
            .map(|outcome| DroppedAssignment {
                session: self.core.snapshot(),
                outcome,
            }))
    }

    fn set_output(&mut self, path: &Path) -> Result<TarpackSession, TarpackError> {
        self.core.set_output(path)?;
        Ok(self.core.snapshot())
    }

    fn set_format(&mut self, format: ArchiveFormat) -> Result<TarpackSession, TarpackError> {
        self.core.set_format(format)?;
        Ok(self.core.snapshot())
    }

    fn begin_build(&mut self) -> Result<Box<dyn BuildWork>, TarpackError> {
        let job = self.core.begin_build()?;
        Ok(Box::new(RealBuildWork { job, result: None }))
    }

    fn finish_build(&mut self, work: Box<dyn BuildWork>) -> Result<BuildSummary, TarpackError> {
        let Ok(work) = work.into_any().downcast::<RealBuildWork>() else {
            self.core.abort_build();
            return Err(foreign_work("build"));
        };
        let result = work.result.unwrap_or_else(|| {
            Err(TarpackError::new(
                TarpackErrorKind::Io,
                "the build never ran",
            ))
        });
        self.core.finish_build(&work.job, &result);
        result
    }

    fn abort_build(&mut self) {
        self.core.abort_build();
    }

    fn manifest_path(&self) -> Option<PathBuf> {
        self.core.manifest_path().map(Path::to_path_buf)
    }

    fn manifest_path_for_open(&self) -> Result<PathBuf, TarpackError> {
        self.core.manifest_path_for_open()
    }

    fn reveal_target(&self) -> Result<PathBuf, TarpackError> {
        self.core.reveal_target()
    }
}
