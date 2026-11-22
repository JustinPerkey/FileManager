//! Schedule Creator: placeholder view; a UI task replaces it.

use gpui::{div, Context, Entity, IntoElement, ParentElement as _, Render, Window};

use super::model::Slot;
use super::ScheduleModel;

pub struct ScheduleView {
    model: Entity<ScheduleModel>,
}

impl ScheduleView {
    pub fn new(model: Entity<ScheduleModel>, cx: &mut Context<Self>) -> Self {
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        Self { model }
    }
}

impl Render for ScheduleView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model.read(cx);
        let path = |slot| model.file_path(slot).unwrap_or("none");
        let files = if model.session().is_some() {
            format!(
                "Schedule file: {}. XML file: {}.",
                path(Slot::Text),
                path(Slot::Xml)
            )
        } else {
            String::new()
        };
        div().child(format!("{} {files}", super::text::TITLE))
    }
}
