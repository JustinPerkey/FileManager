//! A skeleton bar: the placeholder of a line while its content loads.

use gpui::prelude::*;
use gpui::{div, px, relative, App, Window};

use crate::theme::palette;
use crate::theme::tokens::{RADIUS, SPACE_4, SPACE_5};

/// A skeleton bar. Build it with [`skeleton_bar`].
#[derive(IntoElement)]
pub struct SkeletonBar {
    fraction: f32,
    wide: bool,
}

/// A `surface_sunken` bar, 16 px tall (24 px when `wide`), `fraction` of its
/// parent's width. The parent must have a definite width.
#[must_use]
pub fn skeleton_bar(fraction: f32, wide: bool) -> SkeletonBar {
    SkeletonBar { fraction, wide }
}

impl RenderOnce for SkeletonBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .w(relative(self.fraction.clamp(0.0, 1.0)))
            .h(px(if self.wide { SPACE_5 } else { SPACE_4 }))
            .rounded(px(RADIUS))
            .bg(palette(cx).surface_sunken)
    }
}
