//! The two steps: a numbered label, the path field with Browse, and under
//! them the slot's error.

use gpui::prelude::*;
use gpui::{div, px, Context};
use gpui_component::input::Escape;
use gpui_component::{h_flex, v_flex};

use super::{index, ScheduleView};
use crate::app::probe::Tag as _;
use crate::theme::tokens::{SPACE_2, SPACE_3, SPACE_4};
use crate::theme::Palette;
use crate::tools::schedule::model::{ScheduleModel, Slot};
use crate::tools::schedule::text;
use crate::widgets::{button, h2, muted, step_number, text_input};

/// The least width the field asks for before Browse wraps below it (24rem).
const FIELD_BASIS: f32 = 384.0;

pub(super) fn steps(
    view: &ScheduleView,
    model: &ScheduleModel,
    p: &Palette,
    cx: &Context<ScheduleView>,
) -> impl IntoElement {
    v_flex()
        .gap(px(SPACE_4))
        .child(step(view, model, Slot::Text, p, cx))
        .child(step(view, model, Slot::Xml, p, cx))
}

fn step(
    view: &ScheduleView,
    model: &ScheduleModel,
    slot: Slot,
    p: &Palette,
    cx: &Context<ScheduleView>,
) -> impl IntoElement {
    let (n, label, browse_label, input_tag, browse_tag, error_tag) = match slot {
        Slot::Text => (
            1,
            text::TEXT_LABEL,
            text::TEXT_BROWSE_LABEL,
            "schedule-text-input",
            "schedule-text-browse",
            "schedule-text-error",
        ),
        Slot::Xml => (
            2,
            text::XML_LABEL,
            text::XML_BROWSE_LABEL,
            "schedule-xml-input",
            "schedule-xml-browse",
            "schedule-xml-error",
        ),
    };
    let file = model.session().and_then(|s| match slot {
        Slot::Text => s.text.as_ref(),
        Slot::Xml => s.xml.as_ref(),
    });
    let error = file.and_then(|f| f.error.as_ref());
    let busy = model.applying();
    let this = view.model.clone();

    let field = div()
        .flex_grow(1.0)
        .flex_shrink(1.0)
        .flex_basis(px(FIELD_BASIS))
        .min_w_0()
        .on_action(cx.listener(move |this, _: &Escape, window, cx| this.revert(slot, window, cx)))
        .child(
            text_input(&view.fields[index(slot)].input)
                .mono()
                .invalid(error.is_some())
                .disabled(busy)
                .label(label),
        )
        .debug_selector(move || input_tag.into())
        .tag(input_tag);
    let browse = button(browse_tag, text::BROWSE)
        .tooltip(browse_label)
        .disabled(busy)
        .debug_selector(browse_tag)
        .on_click(move |_, _, cx| {
            this.update(cx, |m, cx| m.browse(slot, cx));
        })
        .tag(browse_tag);

    let error_line = error.map(|e| {
        div()
            .child(muted(text::error_message(e)).text_color(p.danger))
            .debug_selector(move || error_tag.into())
    });

    v_flex()
        .gap(px(SPACE_2))
        .child(
            h_flex()
                .items_center()
                .gap(px(SPACE_2))
                .child(step_number(n))
                .child(h2(label)),
        )
        .child(
            h_flex()
                .flex_wrap()
                .items_center()
                .gap_x(px(SPACE_3))
                .gap_y(px(SPACE_2))
                .child(field)
                .child(browse),
        )
        .children(error_line)
}
