//! The empty state: no manifest is open.

use gpui::prelude::*;
use gpui::{px, Entity};
use gpui_component::{h_flex, v_flex};

use super::header::recent_menu;
use crate::app::probe::Tag as _;
use crate::theme::tokens::{SPACE_2, SPACE_3};
use crate::theme::INTRO_MAX_WIDTH;
use crate::tools::tarpack::model::TarpackModel;
use crate::tools::tarpack::text;
use crate::widgets::button;
use crate::widgets::text::{body, muted};

/// The sentence, the two ways to get a manifest, the hint and (when there are
/// recent manifests) the Recent menu.
pub fn empty_state(model: &Entity<TarpackModel>, recent: &[String]) -> impl IntoElement {
    let open = {
        let model = model.clone();
        button("tarpack-open", text::OPEN_MANIFEST)
            .primary()
            .shortcut("Ctrl+O")
            .debug_selector("tarpack-open")
            .on_click(move |_, _, cx| model.update(cx, TarpackModel::open))
            .tag("tarpack-open")
    };
    let create = {
        let model = model.clone();
        button("tarpack-create-example", text::CREATE_FROM_EXAMPLE)
            .debug_selector("tarpack-create-example")
            .on_click(move |_, _, cx| model.update(cx, TarpackModel::create_from_example))
            .tag("tarpack-create-example")
    };
    v_flex()
        .items_start()
        .max_w(px(INTRO_MAX_WIDTH))
        .gap(px(SPACE_3))
        .child(body(text::EMPTY_LEAD))
        .child(
            h_flex()
                .flex_wrap()
                .gap(px(SPACE_2))
                .child(open)
                .child(create),
        )
        .child(muted(text::EMPTY_HINT))
        .child(recent_menu(model, recent))
}
