//! The real [`Platform`], over GPUI's own calls.

use std::path::Path;

use ::gpui::{App, ClipboardItem, PathPromptOptions, Task};

use super::{OpenSpec, PickError, PickKind, PickResult, Platform, SaveSpec};

/// `Platform` over GPUI.
///
/// Replaced in M3c: the dialogs here are GPUI's prompts, which have no
/// file-type filter and no default extension, so those parts of the specs are
/// ignored for now. They also fail the whole selection on a name that is not
/// valid Unicode.
pub struct GpuiPlatform;

fn failed(e: impl std::fmt::Display) -> PickError {
    PickError::Failed {
        detail: format!("{e:#}"),
    }
}

impl Platform for GpuiPlatform {
    // Replaced in M3c: filters and title are ignored.
    fn pick(&self, spec: OpenSpec, cx: &mut App) -> Task<PickResult> {
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

    // Replaced in M3c: filters, default extension and title are ignored, and
    // the start directory is converted lossily by GPUI.
    fn pick_save(&self, spec: SaveSpec, cx: &mut App) -> Task<PickResult> {
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
