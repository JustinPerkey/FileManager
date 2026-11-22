//! Schedule Creator: placeholder view; a UI task replaces it.

use gpui::{div, Context, Entity, IntoElement, ParentElement as _, Render, Window};

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
        let files = model.session().map_or_else(String::new, |s| {
            let path = |f: &Option<fm_schedule::session::FileSlot>| {
                f.as_ref().map_or("none", |f| f.path.as_str()).to_string()
            };
            format!(
                "Schedule file: {}. XML file: {}.",
                path(&s.text),
                path(&s.xml)
            )
        });
        div().child(format!("{} {files}", super::text::TITLE))
    }
}
