//! Tar Packager: placeholder view; a UI task replaces it.

use gpui::{div, Context, IntoElement, ParentElement as _, Render, Window};

pub struct TarpackView;

impl Render for TarpackView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child("Tar Packager: not ported yet")
    }
}
