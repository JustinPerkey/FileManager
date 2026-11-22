//! The tool registry: the single list of tools. Adding a tool is a module, an
//! entry in [`build`], and a [`ToolId`] variant. Tools never import each other.

pub mod pipeline;
pub mod schedule;
pub mod tarpack;

use std::path::PathBuf;

use gpui::{AnyView, App, Global, SharedString, Window};

use crate::app::drop::DropTarget;

/// Which tool. The order of the variants is not the display order; [`build`]
/// is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolId {
    Pipeline,
    Tarpack,
    Schedule,
}

impl ToolId {
    /// The stable id string (as in the Tauri app's registry).
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            ToolId::Pipeline => "pipeline",
            ToolId::Tarpack => "tarpack",
            ToolId::Schedule => "schedule",
        }
    }
}

/// Which backend one tool runs against.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Backend {
    /// The real backend (the domain crate, the real file system).
    #[default]
    Real,
    /// A fixture backend; the tool's milestone defines what each id means. Only
    /// in tests and the screenshot harness.
    #[cfg(any(test, feature = "shoot"))]
    Fixture(FixtureId),
}

/// Names one of a tool's fixture backends; each tool's milestone adds a
/// variant.
#[cfg(any(test, feature = "shoot"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixtureId {
    Pipeline(pipeline::fixtures::PipelineFixture),
}

/// The backend choice for every tool, installed as a global by
/// [`crate::run_with`]. A tool's `slot` reads it with [`BackendSet::get`]. Each
/// tool milestone adds its fixtures behind
/// `#[cfg(any(test, feature = "shoot"))]`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BackendSet {
    pub pipeline: Backend,
    pub tarpack: Backend,
    pub schedule: Backend,
}

impl Global for BackendSet {}

impl BackendSet {
    /// The set installed on `cx`; all real if none was.
    #[must_use]
    pub fn get(cx: &App) -> Self {
        cx.try_global::<Self>().copied().unwrap_or_default()
    }
}

/// What the shell asks of a tool. Every method has a default so placeholder
/// tools stay small.
pub trait ToolHooks {
    /// The tool became the current one.
    fn activated(&self, _window: &mut Window, _cx: &mut App) {}

    /// Whether the tool takes an Explorer drop now. The default is no target.
    fn drop_target(&self, _cx: &App) -> DropTarget {
        DropTarget::None
    }

    /// Files dropped on the window while this tool is current and its target
    /// is enabled. Never called with an empty list.
    fn dropped(&self, _paths: Vec<PathBuf>, _window: &mut Window, _cx: &mut App) {}

    /// One short description per piece of unsaved work. The window does not
    /// close without confirmation while any tool returns some.
    fn unsaved_work(&self, _cx: &App) -> Vec<SharedString> {
        Vec::new()
    }

    /// Screenshot harness: injects a "file changed on disk" signal into the
    /// tool's model, as its file watcher would. Does nothing by default; the
    /// tools that watch a file implement it.
    #[cfg(any(test, feature = "shoot"))]
    fn inject_file_changed(&self, _window: &mut Window, _cx: &mut App) {}
}

/// A tool as the shell holds it.
pub struct ToolSlot {
    pub id: ToolId,
    pub label: &'static str,
    pub view: AnyView,
    pub hooks: Box<dyn ToolHooks>,
}

/// Builds every tool, in display order.
pub fn build(window: &mut Window, cx: &mut App) -> Vec<ToolSlot> {
    vec![
        pipeline::slot(window, cx),
        tarpack::slot(window, cx),
        schedule::slot(window, cx),
    ]
}

#[cfg(test)]
mod tests {
    use gpui::TestAppContext;

    use super::*;

    #[gpui::test]
    fn registry_order_and_labels(cx: &mut TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| gpui::Empty);
        let slots = cx.update(build);
        let got: Vec<_> = slots.iter().map(|s| (s.id.id(), s.label)).collect();
        assert_eq!(
            got,
            [
                ("pipeline", "Pipeline"),
                ("tarpack", "Tar Packager"),
                ("schedule", "Schedule Creator"),
            ]
        );
        let mut ids: Vec<_> = got.iter().map(|(id, _)| *id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 3, "ids are unique");
    }
}
