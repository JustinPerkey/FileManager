//! The Schedule Creator's Tauri surface: `schedule_*` commands and the
//! managed state. Commands are thin glue over [`core::Core`]. This module
//! never touches the `Builder`; `tools::register` wires it in.
//!
//! The parsing and the XML update are hooks in the `fm-schedule` crate
//! (`fm_schedule::parse` and `fm_schedule::merge`).

mod core;
pub(crate) mod types;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use tauri::State;

use types::{ApplySummary, ScheduleError, ScheduleSession};

use self::core::Core;

/// Tauri-managed state: the session.
pub struct ScheduleState {
    core: Mutex<Core>,
}

pub fn state() -> ScheduleState {
    ScheduleState {
        core: Mutex::new(Core::default()),
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

#[tauri::command]
pub async fn schedule_session(
    state: State<'_, ScheduleState>,
) -> Result<ScheduleSession, ScheduleError> {
    Ok(lock(&state.core).snapshot())
}

#[tauri::command]
pub async fn schedule_open_text(
    state: State<'_, ScheduleState>,
    path: PathBuf,
) -> Result<ScheduleSession, ScheduleError> {
    let mut core = lock(&state.core);
    core.open_text(&path)?;
    Ok(core.snapshot())
}

#[tauri::command]
pub async fn schedule_reload_text(
    state: State<'_, ScheduleState>,
) -> Result<ScheduleSession, ScheduleError> {
    let mut core = lock(&state.core);
    core.reload_text()?;
    Ok(core.snapshot())
}

#[tauri::command]
pub async fn schedule_open_xml(
    state: State<'_, ScheduleState>,
    path: PathBuf,
) -> Result<ScheduleSession, ScheduleError> {
    let mut core = lock(&state.core);
    core.open_xml(&path)?;
    Ok(core.snapshot())
}

/// Adds the previewed schedule to the chosen XML file. The UI confirms first;
/// a backup of the current file is always saved beside it.
#[tauri::command]
pub async fn schedule_apply(
    state: State<'_, ScheduleState>,
) -> Result<ApplySummary, ScheduleError> {
    lock(&state.core).apply()
}
