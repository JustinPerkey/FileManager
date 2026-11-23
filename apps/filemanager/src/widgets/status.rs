//! A status: an icon and a word in the colour of the state. Never colour
//! alone.

use gpui::prelude::*;
use gpui::{div, px, App, SharedString, Window};
use gpui_component::h_flex;

use super::icon::{icon, IconName};
use crate::theme::palette;
use crate::theme::tokens::SPACE_1;

/// Which colour a status takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusTone {
    Ok,
    Warn,
    Danger,
    /// Nothing to report: `text_muted`.
    Muted,
}

/// A status. Build it with [`status`].
#[derive(IntoElement)]
pub struct Status {
    tone: StatusTone,
    icon: IconName,
    word: SharedString,
}

/// The icon and `word` in the tone's colour, 4 px apart, on one line.
#[must_use]
pub fn status(tone: StatusTone, icon: IconName, word: impl Into<SharedString>) -> Status {
    Status {
        tone,
        icon,
        word: word.into(),
    }
}

impl RenderOnce for Status {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        let color = match self.tone {
            StatusTone::Ok => p.ok,
            StatusTone::Warn => p.warn,
            StatusTone::Danger => p.danger,
            StatusTone::Muted => p.text_muted,
        };
        h_flex()
            .gap(px(SPACE_1))
            .items_center()
            .flex_none()
            .whitespace_nowrap()
            .text_color(color)
            .child(icon(self.icon).text_color(color))
            .child(div().child(self.word))
    }
}
