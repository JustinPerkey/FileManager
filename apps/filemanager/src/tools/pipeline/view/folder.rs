//! The folder region: its label, the field and Browse, and under them the
//! field's error or the subfolder count.

use gpui::prelude::*;
use gpui::{div, px, Context};
use gpui_component::input::Escape;
use gpui_component::{h_flex, v_flex};

use super::PipelineView;
use crate::app::probe::Tag as _;
use crate::theme::tokens::{SPACE_2, SPACE_3};
use crate::theme::Palette;
use crate::tools::pipeline::model::PipelineModel;
use crate::tools::pipeline::text;
use crate::widgets::{button, h2, muted, text_input};

/// The least width the field asks for before Browse wraps below it (24rem).
const FIELD_BASIS: f32 = 384.0;

pub(super) fn folder(
    view: &PipelineView,
    model: &PipelineModel,
    p: &Palette,
    cx: &Context<PipelineView>,
) -> impl IntoElement {
    let slot = model.session().and_then(|s| s.directory.as_ref());
    let error = slot.and_then(|d| d.error.as_ref());
    let busy = model.busy();
    let this = view.model.clone();

    let field = div()
        .flex_grow(1.0)
        .flex_shrink(1.0)
        .flex_basis(px(FIELD_BASIS))
        .min_w_0()
        .on_action(cx.listener(|this, _: &Escape, window, cx| this.revert(window, cx)))
        .child(
            text_input(&view.input)
                .mono()
                .invalid(error.is_some())
                .disabled(busy)
                .label(text::FOLDER_LABEL),
        )
        .debug_selector(|| "pipeline-folder-input".into())
        .tag("pipeline-folder-input");
    let browse = button("pipeline-browse", text::BROWSE)
        .tooltip(text::BROWSE_ARIA)
        .disabled(busy)
        .debug_selector("pipeline-browse")
        .on_click(move |_, _, cx| {
            this.update(cx, PipelineModel::browse);
        })
        .tag("pipeline-browse");

    let note = match (slot, error) {
        (_, Some(e)) => Some((
            "pipeline-field-error",
            muted(text::error_message(e.kind)).text_color(p.danger),
        )),
        (Some(d), None) => Some((
            "pipeline-folder-note",
            muted(text::subfolders(d.subdirectories)),
        )),
        (None, None) => None,
    }
    .map(|(name, line)| div().child(line).debug_selector(move || name.into()));

    v_flex()
        .gap(px(SPACE_2))
        .child(h2(text::FOLDER_LABEL))
        .child(
            h_flex()
                .flex_wrap()
                .items_center()
                .gap_x(px(SPACE_3))
                .gap_y(px(SPACE_2))
                .child(field)
                .child(browse),
        )
        .children(note)
}
