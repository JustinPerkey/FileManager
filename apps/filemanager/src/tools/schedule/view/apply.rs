//! The Add row, with the reason it is disabled, and the result banner.

use gpui::prelude::*;
use gpui::{div, px};
use gpui_component::{h_flex, v_flex};

use super::ScheduleView;
use crate::app::probe::Tag as _;
use crate::theme::tokens::{SPACE_2, SPACE_3};
use crate::theme::Palette;
use crate::tools::schedule::model::ScheduleModel;
use crate::tools::schedule::text;
use crate::widgets::{button, muted, Banner, Tone};

pub(super) fn apply_row(
    view: &ScheduleView,
    model: &ScheduleModel,
    p: &Palette,
) -> impl IntoElement {
    let applying = model.applying();
    let this = view.model.clone();
    let add = button(
        "schedule-apply",
        if applying { text::ADDING } else { text::ADD },
    )
    .primary()
    .busy(applying)
    .disabled(!model.can_apply())
    .track_focus(&view.add)
    .debug_selector("schedule-apply")
    .on_click(move |_, _, cx| {
        this.update(cx, ScheduleModel::request_apply);
    })
    .tag("schedule-apply");
    let reason = model.apply_reason().map(|r| {
        div()
            .child(muted(r))
            .debug_selector(|| "schedule-reason".into())
    });

    h_flex()
        .flex_wrap()
        .items_center()
        .gap_x(px(SPACE_3))
        .gap_y(px(SPACE_2))
        .pt(px(SPACE_3))
        .border_t_1()
        .border_color(p.border)
        .child(add)
        .children(reason)
}

pub(super) fn result(view: &ScheduleView, model: &ScheduleModel) -> Option<impl IntoElement> {
    let summary = model.result()?;
    let this = view.model.clone();
    let message = format!(
        "{}\n{}{}",
        text::result_sentence(summary),
        text::BACKUP_PREFIX,
        summary.backup_path
    );
    Some(
        v_flex()
            .id("schedule-result-focus")
            .track_focus(&view.outcome)
            .child(
                Banner::new(Tone::Info, message)
                    .id("schedule-result-banner")
                    .dismiss(move |_, cx| {
                        this.update(cx, ScheduleModel::dismiss_result);
                    }),
            )
            .debug_selector(|| "schedule-result".into())
            .tag("schedule-result"),
    )
}
