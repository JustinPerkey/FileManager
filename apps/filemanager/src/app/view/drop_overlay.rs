//! The drop overlay: over the whole window while a file drag is over it, a
//! dashed accent frame and a plate saying what a drop does, or why it cannot.
//! Not focusable and takes no pointer events (it has no listeners and does
//! not occlude); the Shell tracks the drag itself.

use gpui::prelude::*;
use gpui::{div, px, SharedString};
use gpui_component::h_flex;

use crate::app::drop::DropTarget;
use crate::app::probe::Tag as _;
use crate::theme::tokens::{FONT_SIZE_LG, RADIUS, REM, SPACE_2, SPACE_3, SPACE_4};
use crate::theme::Palette;
use crate::widgets::{icon, IconName};

/// Inset of the dashed frame from the window edge.
const FRAME_INSET: f32 = 8.0;
/// The plate's widest.
const PLATE_MAX: f32 = 640.0;

/// What the plate says, with its icon; `None` for a tool that takes no drops.
#[must_use]
pub(crate) fn content(target: &DropTarget) -> Option<(IconName, SharedString)> {
    match target {
        DropTarget::None => None,
        DropTarget::Enabled { label } => Some((IconName::Folder, label.clone())),
        DropTarget::Disabled { reason } => Some((IconName::Info, reason.clone())),
    }
}

/// The overlay for `target`, if there is something to say.
pub fn drop_overlay(target: &DropTarget, p: &Palette) -> Option<impl IntoElement> {
    let (name, text) = content(target)?;
    let plate = h_flex()
        .gap(px(SPACE_2))
        .items_center()
        .max_w(px(PLATE_MAX))
        .px(px(SPACE_4))
        .py(px(SPACE_3))
        .rounded(px(RADIUS))
        .bg(p.surface)
        .text_size(px(FONT_SIZE_LG * REM))
        .text_color(p.text)
        .child(icon(name).flex_none())
        .child(div().min_w_0().child(text));
    Some(
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(p.drop_overlay)
            .child(
                div()
                    .absolute()
                    .top(px(FRAME_INSET))
                    .left(px(FRAME_INSET))
                    .right(px(FRAME_INSET))
                    .bottom(px(FRAME_INSET))
                    .border_2()
                    .border_dashed()
                    .border_color(p.accent)
                    .rounded(px(RADIUS)),
            )
            .child(
                div()
                    .absolute()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(plate),
            )
            .debug_selector(|| "drop-overlay".to_string())
            .tag("drop-overlay"),
    )
}
