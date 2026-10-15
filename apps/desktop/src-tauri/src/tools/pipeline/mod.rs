//! The Pipeline's Tauri surface: `pipeline_*` commands and the managed state.
//! Commands are thin glue over [`core::Core`]. This module never touches the
//! `Builder`; `tools::register` wires it in.
//!
//! The steps' work is done by hooks in the `fm-pipeline` crate, one per step.
//! A step runs on a blocking worker thread, so the session stays responsive
//! (`pipeline_session` reports which step is running) while it works.

mod core;
pub(crate) mod types;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use tauri::State;

use types::{PipelineError, PipelineErrorKind, PipelineSession, StepId};

use self::core::{Core, Dir, Finished};

/// Tauri-managed state: the session.
pub struct PipelineState {
    core: Mutex<Core>,
}

pub fn state() -> PipelineState {
    PipelineState {
        core: Mutex::new(Core::default()),
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

#[tauri::command]
pub async fn pipeline_session(
    state: State<'_, PipelineState>,
) -> Result<PipelineSession, PipelineError> {
    Ok(lock(&state.core).snapshot())
}

/// Sets the directory, or clears it with `null`. Every step's result is
/// forgotten. Refused while a step runs.
#[tauri::command]
pub async fn pipeline_set_directory(
    state: State<'_, PipelineState>,
    path: Option<PathBuf>,
) -> Result<PipelineSession, PipelineError> {
    // Checked before locking: a slow or unreachable share must not block the
    // session.
    let directory = path.as_deref().map(Dir::check);
    let mut core = lock(&state.core);
    core.set_directory(directory)?;
    Ok(core.snapshot())
}

/// Runs one step over the chosen directory and returns the session with its
/// result. A step that fails or is not implemented is recorded in the
/// session, not returned as an error; the command fails only when the step
/// could not start. `expected_directory` is the display path the UI showed;
/// a mismatch runs nothing.
///
/// "Run all" is the UI calling this for each step in order, stopping at the
/// first that does not pass.
#[tauri::command]
pub async fn pipeline_run_step(
    state: State<'_, PipelineState>,
    step: StepId,
    expected_directory: String,
) -> Result<PipelineSession, PipelineError> {
    let job = lock(&state.core).begin(step.into(), &expected_directory)?;
    let finished = tauri::async_runtime::spawn_blocking(move || job.run())
        .await
        .unwrap_or_else(|e| Finished {
            result: Err(PipelineError::new(
                PipelineErrorKind::StepFailed,
                format!("the step stopped unexpectedly: {e}"),
            )),
            directory: None,
        });
    let mut core = lock(&state.core);
    core.finish(step.into(), finished);
    Ok(core.snapshot())
}
