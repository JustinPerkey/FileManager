//! The Tar Packager's Tauri surface: `tarpack_*` commands, the managed state,
//! and the two events. Commands are thin glue over [`core::Core`], which holds
//! the logic as plain functions. This module never touches the `Builder`;
//! `tools::register` wires it in.

mod core;
mod progress;
pub(crate) mod types;
mod watch;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use fm_core::AppDirs;
use fm_tarpack::archive::BuildSummary;
use fm_tarpack::format::ArchiveFormat;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_opener::OpenerExt;

use types::{DroppedAssignment, ManifestChanged, TarpackError, TarpackErrorKind, TarpackSession};

use self::core::Core;
use self::progress::coalescing;
use self::watch::ManifestWatcher;

const EVENT_MANIFEST_CHANGED: &str = "tarpack://manifest-changed";
const EVENT_BUILD_PROGRESS: &str = "tarpack://build-progress";

/// Tauri-managed state: the session and the manifest watcher.
pub struct TarpackState {
    core: Mutex<Core>,
    watcher: Mutex<Option<ManifestWatcher>>,
}

/// Constructs the managed state over the per-user app data directory.
pub fn state() -> TarpackState {
    let core = match AppDirs::from_system() {
        Ok(dirs) => Core::new(dirs),
        Err(e) => Core::unpersisted(format!("saved state is unavailable: {e}")),
    };
    TarpackState {
        core: Mutex::new(core),
        watcher: Mutex::new(None),
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Watches the loaded manifest. `force` replaces the watcher even when the
/// path is unchanged (open and reload); otherwise it is kept if it already
/// watches the loaded path.
fn arm_watcher(app: &AppHandle, state: &TarpackState, force: bool) {
    let Some(path) = lock(&state.core).manifest_path().map(PathBuf::from) else {
        *lock(&state.watcher) = None;
        return;
    };
    let mut watcher = lock(&state.watcher);
    if !force && watcher.as_ref().is_some_and(|w| w.path() == path) {
        return;
    }
    let handle = app.clone();
    let shown = path.to_string_lossy().into_owned();
    *watcher = match ManifestWatcher::start(&path, move || {
        let _ = handle.emit(
            EVENT_MANIFEST_CHANGED,
            ManifestChanged {
                path: shown.clone(),
            },
        );
    }) {
        Ok(w) => Some(w),
        Err(e) => {
            eprintln!("tarpack: cannot watch {}: {e}", path.display());
            None
        }
    };
}

#[tauri::command]
pub async fn tarpack_session(
    app: AppHandle,
    state: State<'_, TarpackState>,
) -> Result<TarpackSession, TarpackError> {
    let session = lock(&state.core).session();
    arm_watcher(&app, &state, false);
    Ok(session)
}

#[tauri::command]
pub async fn tarpack_open_manifest(
    app: AppHandle,
    state: State<'_, TarpackState>,
    path: PathBuf,
) -> Result<TarpackSession, TarpackError> {
    let session = {
        let mut core = lock(&state.core);
        core.open(&path)?;
        core.snapshot()
    };
    arm_watcher(&app, &state, true);
    Ok(session)
}

#[tauri::command]
pub async fn tarpack_reload_manifest(
    app: AppHandle,
    state: State<'_, TarpackState>,
) -> Result<TarpackSession, TarpackError> {
    let session = {
        let mut core = lock(&state.core);
        core.reload()?;
        core.snapshot()
    };
    arm_watcher(&app, &state, true);
    Ok(session)
}

#[tauri::command]
pub async fn tarpack_assign_dropped(
    state: State<'_, TarpackState>,
    paths: Vec<PathBuf>,
) -> Result<DroppedAssignment, TarpackError> {
    let mut core = lock(&state.core);
    let outcome = core.assign_dropped(&paths)?;
    Ok(DroppedAssignment {
        session: core.snapshot(),
        outcome,
    })
}

#[tauri::command]
pub async fn tarpack_assign(
    state: State<'_, TarpackState>,
    id: String,
    path: PathBuf,
) -> Result<TarpackSession, TarpackError> {
    let mut core = lock(&state.core);
    core.assign(&id, &path)?;
    Ok(core.snapshot())
}

#[tauri::command]
pub async fn tarpack_clear(
    state: State<'_, TarpackState>,
    id: String,
) -> Result<TarpackSession, TarpackError> {
    let mut core = lock(&state.core);
    core.clear(&id)?;
    Ok(core.snapshot())
}

#[tauri::command]
pub async fn tarpack_set_output(
    state: State<'_, TarpackState>,
    path: PathBuf,
) -> Result<TarpackSession, TarpackError> {
    let mut core = lock(&state.core);
    core.set_output(&path)?;
    Ok(core.snapshot())
}

#[tauri::command]
pub async fn tarpack_set_format(
    state: State<'_, TarpackState>,
    format: ArchiveFormat,
) -> Result<TarpackSession, TarpackError> {
    let mut core = lock(&state.core);
    core.set_format(format)?;
    Ok(core.snapshot())
}

/// Builds the passed entries into the session's output with the session's
/// format. The returned summary is the final report. `path` and
/// `extract_command` in it are display strings only.
#[tauri::command]
pub async fn tarpack_build(
    app: AppHandle,
    state: State<'_, TarpackState>,
    overwrite: bool,
) -> Result<BuildSummary, TarpackError> {
    let job = lock(&state.core).begin_build()?;
    let (job, result) = tauri::async_runtime::spawn_blocking(move || {
        let mut sink = coalescing(|event| {
            let _ = app.emit(EVENT_BUILD_PROGRESS, event);
        });
        let result = job.run(overwrite, &mut sink);
        (job, result)
    })
    .await
    .map_err(|e| {
        // The build thread panicked: release the session.
        lock(&state.core).abort_build();
        TarpackError::new(
            TarpackErrorKind::Io,
            format!("the build stopped unexpectedly: {e}"),
        )
    })?;
    lock(&state.core).finish_build(&job, &result);
    result
}

#[tauri::command]
pub async fn tarpack_recent_manifests(
    state: State<'_, TarpackState>,
) -> Result<Vec<String>, TarpackError> {
    Ok(lock(&state.core).recent_manifests())
}

#[tauri::command]
pub async fn tarpack_open_in_editor(
    app: AppHandle,
    state: State<'_, TarpackState>,
) -> Result<(), TarpackError> {
    let path = lock(&state.core).manifest_path_for_open()?;
    let path = path.to_str().ok_or_else(|| {
        TarpackError::new(
            TarpackErrorKind::OpenerFailed,
            "the manifest path is not valid Unicode, so it cannot be opened",
        )
    })?;
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| TarpackError::new(TarpackErrorKind::OpenerFailed, e.to_string()))
}

/// Reveals the file the last successful build in this session wrote. Takes no
/// argument: the path is held in state, never read back from a display string.
#[tauri::command]
pub async fn tarpack_reveal_output(
    app: AppHandle,
    state: State<'_, TarpackState>,
) -> Result<(), TarpackError> {
    let path = lock(&state.core).reveal_target()?;
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|e| TarpackError::new(TarpackErrorKind::OpenerFailed, e.to_string()))
}

#[tauri::command]
pub async fn tarpack_create_manifest_from_example(
    app: AppHandle,
    state: State<'_, TarpackState>,
    path: PathBuf,
) -> Result<TarpackSession, TarpackError> {
    let session = {
        let mut core = lock(&state.core);
        core.create_from_example(&path)?;
        core.snapshot()
    };
    arm_watcher(&app, &state, true);
    Ok(session)
}
