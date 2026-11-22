//! The real [`Platform`], over GPUI's own calls.

use std::path::Path;

#[cfg(not(windows))]
use ::gpui::PathPromptOptions;
use ::gpui::{App, ClipboardItem, Task};

use super::{OpenSpec, PickResult, Platform, SaveSpec};
#[cfg(not(windows))]
use super::{PickError, PickKind};

/// `Platform` over GPUI, with the file dialogs on Windows through
/// [`super::win_dialog`] (filters, default extension, start folder, exact
/// paths).
///
/// On other targets the dialogs fall back to GPUI's prompts, which ignore the
/// filters, the default extension and the title. That code only keeps the
/// crate compiling: Linux builds are headless and never open a window, so it
/// is unused in practice.
pub struct GpuiPlatform;

#[cfg(not(windows))]
fn failed(e: impl std::fmt::Display) -> PickError {
    PickError::Failed {
        detail: format!("{e:#}"),
    }
}

#[cfg(windows)]
fn open_dialog(spec: &OpenSpec, cx: &mut App) -> Task<PickResult> {
    super::win_dialog::pick(spec.clone(), cx)
}

#[cfg(windows)]
fn save_dialog(spec: SaveSpec, cx: &mut App) -> Task<PickResult> {
    super::win_dialog::pick_save(spec, cx)
}

// The fallbacks below are unused in practice (see `GpuiPlatform`).
#[cfg(not(windows))]
fn open_dialog(spec: &OpenSpec, cx: &mut App) -> Task<PickResult> {
    let rx = cx.prompt_for_paths(PathPromptOptions {
        files: spec.kind == PickKind::File,
        directories: spec.kind == PickKind::Folder,
        multiple: false,
        prompt: None,
    });
    cx.spawn(async move |_cx| match rx.await {
        Ok(Ok(paths)) => Ok(paths.and_then(|p| p.into_iter().next())),
        Ok(Err(e)) => Err(failed(e)),
        Err(e) => Err(failed(e)),
    })
}

#[cfg(not(windows))]
fn save_dialog(spec: SaveSpec, cx: &mut App) -> Task<PickResult> {
    let dir = spec.start_dir.unwrap_or_default();
    let name = spec
        .suggested_name
        .map(|n| n.to_string_lossy().into_owned());
    let rx = cx.prompt_for_new_path(&dir, name.as_deref());
    cx.spawn(async move |_cx| match rx.await {
        Ok(Ok(path)) => Ok(path),
        Ok(Err(e)) => Err(failed(e)),
        Err(e) => Err(failed(e)),
    })
}

impl Platform for GpuiPlatform {
    fn pick(&self, spec: OpenSpec, cx: &mut App) -> Task<PickResult> {
        open_dialog(&spec, cx)
    }

    fn pick_save(&self, spec: SaveSpec, cx: &mut App) -> Task<PickResult> {
        save_dialog(spec, cx)
    }

    fn open_with_system(&self, path: &Path, cx: &mut App) {
        cx.open_with_system(path);
    }

    fn reveal(&self, path: &Path, cx: &mut App) {
        cx.reveal_path(path);
    }

    fn copy_text(&self, text: &str, cx: &mut App) {
        cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
    }
}
