//! The run bar: Run next, Run all and why they may be disabled.

use gpui::prelude::*;
use gpui::{px, Context};
use gpui_component::h_flex;

use super::PipelineView;
use crate::app::probe::Tag as _;
use crate::theme::tokens::{SPACE_2, SPACE_3};
use crate::theme::Palette;
use crate::tools::pipeline::model::PipelineModel;
use crate::tools::pipeline::text;
use crate::widgets::{button, muted};

pub(super) fn run_bar(
    view: &PipelineView,
    model: &PipelineModel,
    _p: &Palette,
    _cx: &Context<PipelineView>,
) -> impl IntoElement {
    let next = model.next_step();
    let can_run = model.can_run();
    let next_label = next.map_or_else(|| text::RUN_NEXT_NONE.to_string(), text::run_next_label);
    let all_label = if model.running_all() {
        text::RUNNING_ALL
    } else {
        text::RUN_ALL
    };
    let on_next = view.model.clone();
    let on_all = view.model.clone();

    let run_next = button("pipeline-run-next", next_label)
        .primary()
        .busy(model.via_next().is_some())
        .disabled(!can_run || next.is_none())
        .track_focus(&view.run_next)
        .debug_selector("pipeline-run-next")
        .on_click(move |_, _, cx| {
            on_next.update(cx, PipelineModel::request_next);
        })
        .tag("pipeline-run-next");
    let run_all = button("pipeline-run-all", all_label)
        .busy(model.running_all())
        .disabled(!can_run)
        .debug_selector("pipeline-run-all")
        .on_click(move |_, _, cx| {
            on_all.update(cx, PipelineModel::request_all);
        })
        .tag("pipeline-run-all");
    let note = model
        .run_reason()
        .or_else(|| model.all_passed().then_some(text::ALL_PASSED));

    h_flex()
        .flex_wrap()
        .items_center()
        .gap_x(px(SPACE_3))
        .gap_y(px(SPACE_2))
        .child(run_next)
        .child(run_all)
        .children(note.map(muted))
}
