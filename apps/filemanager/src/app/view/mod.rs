//! The visual shell. This module belongs to the ui-implementer, who replaces
//! the body of [`chrome`]: sidebar, current marker, the current tool's view,
//! overlays. The backend only provides the placeholder.

mod placeholder;

use gpui::{AnyView, App, Entity, Window};

use crate::app::shell::Shell;

/// Builds the chrome view for `shell`.
pub fn chrome(shell: Entity<Shell>, window: &mut Window, cx: &mut App) -> AnyView {
    crate::theme::init(window, cx);
    placeholder::chrome(shell, window, cx)
}
