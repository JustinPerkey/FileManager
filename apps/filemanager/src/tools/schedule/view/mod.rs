//! Schedule Creator: placeholder view; a UI task replaces it.

use gpui::{div, Context, IntoElement, ParentElement as _, Render, Window};

pub struct ScheduleView;

impl Render for ScheduleView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child("Schedule Creator: not ported yet")
    }
}
