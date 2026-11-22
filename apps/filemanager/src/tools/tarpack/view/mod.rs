//! Tar Packager: placeholder view; a UI task replaces it.

use gpui::{div, Context, Entity, IntoElement, ParentElement as _, Render, Window};

use super::TarpackModel;

pub struct TarpackView {
    model: Entity<TarpackModel>,
}

impl TarpackView {
    pub fn new(model: Entity<TarpackModel>, cx: &mut Context<Self>) -> Self {
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        Self { model }
    }
}

impl Render for TarpackView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model.read(cx);
        let manifest = model
            .session()
            .and_then(|s| s.manifest.as_ref())
            .map_or(String::new(), |m| format!("Manifest: {}.", m.path));
        div().child(format!("{} {manifest}", super::text::TITLE))
    }
}
