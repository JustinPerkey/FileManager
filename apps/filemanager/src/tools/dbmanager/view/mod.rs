//! Database Manager: placeholder view; a UI task replaces it. Plain text only:
//! the database, the place, and the labels of the current list.

use gpui::{div, Context, Entity, IntoElement, ParentElement as _, Render, Window};

use super::model::{DbmanagerModel, Place};
use super::text;

pub struct DbmanagerView {
    model: Entity<DbmanagerModel>,
}

impl DbmanagerView {
    pub fn new(model: Entity<DbmanagerModel>, cx: &mut Context<Self>) -> Self {
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        Self { model }
    }
}

impl Render for DbmanagerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model.read(cx);
        let database = model
            .snapshot()
            .and_then(|s| s.database.as_ref())
            .map_or(text::NO_DATABASE_OPEN, |d| d.display.as_str());
        let place = match model.place() {
            Place::Tables => "Tables".to_owned(),
            Place::Table { table } => format!("Table {table}"),
            Place::Object { table, label, .. } => format!("{table}: {label}"),
            Place::New { table } => format!("New {table}"),
        };
        let labels = match model.place() {
            Place::Table { table } => model
                .list_state(table)
                .and_then(|l| l.page.as_ref())
                .map(|p| {
                    p.items
                        .iter()
                        .map(|i| i.label.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default(),
            _ => String::new(),
        };
        div()
            .child(format!("{} {database}", text::TITLE))
            .child(place)
            .child(labels)
    }
}
