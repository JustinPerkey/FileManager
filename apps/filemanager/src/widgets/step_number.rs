//! The number of a step: a 24 px circle.

use gpui::prelude::*;
use gpui::{div, px, App, Window};

use crate::theme::palette;
use crate::theme::tokens::{FONT_SIZE_SM, REM, SPACE_5};

/// A step number. Build it with [`step_number`].
#[derive(IntoElement)]
pub struct StepNumber(usize);

/// A 24 px circle on `surface_sunken` with a 1 px `border`, the number in
/// 13 px text at its centre.
#[must_use]
pub fn step_number(n: usize) -> StepNumber {
    StepNumber(n)
}

impl RenderOnce for StepNumber {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(SPACE_5))
            .rounded_full()
            .bg(p.surface_sunken)
            .border_1()
            .border_color(p.border)
            .text_size(px(FONT_SIZE_SM * REM))
            .text_color(p.text)
            .child(self.0.to_string())
    }
}
