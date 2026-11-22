//! Tar Packager: placeholder model until the tool is ported.

pub mod view;

use gpui::{App, AppContext as _, Window};

use crate::tools::{ToolHooks, ToolId, ToolSlot};

/// Placeholder model; a later milestone replaces it.
pub struct TarpackModel;

struct Hooks;

impl ToolHooks for Hooks {}

pub(super) fn slot(_window: &mut Window, cx: &mut App) -> ToolSlot {
    ToolSlot {
        id: ToolId::Tarpack,
        label: "Tar Packager",
        view: cx.new(|_| view::TarpackView).into(),
        hooks: Box::new(Hooks),
    }
}
