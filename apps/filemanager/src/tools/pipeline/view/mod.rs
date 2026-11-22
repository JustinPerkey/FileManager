//! Pipeline: placeholder view; a UI task replaces it.

use gpui::{div, Context, IntoElement, ParentElement as _, Render, Window};

pub struct PipelineView;

impl Render for PipelineView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child("Pipeline: not ported yet")
    }
}
