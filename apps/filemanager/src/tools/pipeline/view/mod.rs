//! Pipeline: placeholder view; a UI task replaces it. It shows the model's
//! folder and step statuses as plain text.

use gpui::{div, Context, Entity, IntoElement, ParentElement as _, Render, Window};

use super::model::PipelineModel;
use super::text;

pub struct PipelineView {
    model: Entity<PipelineModel>,
}

impl PipelineView {
    pub fn new(model: Entity<PipelineModel>, cx: &mut Context<Self>) -> Self {
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        Self { model }
    }
}

impl Render for PipelineView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model.read(cx);
        let mut root = div().child(text::HEADER);
        if let Some(session) = model.session() {
            let folder = session
                .directory
                .as_ref()
                .map_or("no folder".to_string(), |d| d.path.clone());
            root = root.child(folder);
            for view in &session.steps {
                let status = text::status_label(model.step_status(view.id));
                root = root.child(format!("{}: {status}", text::step_copy(view.id).title));
            }
        } else if model.loading() {
            root = root.child(text::LOADING);
        }
        root
    }
}
