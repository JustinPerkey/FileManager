//! A tag: a small outlined label, optionally with an icon.

use gpui::prelude::*;
use gpui::{div, px, App, SharedString, Window};
use gpui_component::h_flex;

use super::icon::{icon, IconName};
use crate::theme::palette;
use crate::theme::tokens::{FONT_SIZE_SM, RADIUS, REM, SPACE_1};

/// A tag. Build it with [`tag`].
#[derive(IntoElement)]
pub struct Tag {
    label: SharedString,
    icon: Option<IconName>,
}

/// A 13 px muted label in a 1 px `border` outline, with an optional icon
/// before it.
#[must_use]
pub fn tag(label: impl Into<SharedString>, icon: Option<IconName>) -> Tag {
    Tag {
        label: label.into(),
        icon,
    }
}

impl RenderOnce for Tag {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        h_flex()
            .gap(px(SPACE_1))
            .items_center()
            .px(px(SPACE_1))
            .text_size(px(FONT_SIZE_SM * REM))
            .text_color(p.text_muted)
            .border_1()
            .border_color(p.border)
            .rounded(px(RADIUS))
            .when_some(self.icon, |row, name| row.child(icon(name)))
            .child(div().whitespace_nowrap().child(self.label))
    }
}
