//! Pipeline: the model, its backend, its copy and (for tests and the harness)
//! its fixtures. The view is the ui-implementer's; `view/` holds a plain
//! stand-in until it replaces it.

pub mod backend;
#[cfg(any(test, feature = "shoot"))]
pub mod fixtures;
pub mod model;
pub mod text;
pub mod view;

#[cfg(test)]
mod tests;

use gpui::{App, AppContext as _, Entity, Window};

pub use self::model::PipelineModel;
use crate::tools::{Backend, BackendSet, ToolHooks, ToolId, ToolSlot};

use self::backend::{PipelineBackend, RealPipeline};

struct Hooks {
    model: Entity<PipelineModel>,
}

impl ToolHooks for Hooks {
    fn activated(&self, _window: &mut Window, cx: &mut App) {
        self.model.update(cx, PipelineModel::activated);
    }
}

fn backend_for(cx: &App) -> Box<dyn PipelineBackend> {
    match BackendSet::get(cx).pipeline {
        Backend::Real => Box::new(RealPipeline::new()),
        #[cfg(any(test, feature = "shoot"))]
        Backend::Fixture(crate::tools::FixtureId::Pipeline(fixture)) => {
            Box::new(fixtures::FixturePipeline::from_fixture(fixture))
        }
        #[cfg(any(test, feature = "shoot"))]
        Backend::Fixture(other) => panic!("not a Pipeline fixture: {other:?}"),
    }
}

pub(super) fn slot(window: &mut Window, cx: &mut App) -> ToolSlot {
    let backend = backend_for(cx);
    let model = cx.new(|cx| PipelineModel::new(backend, cx));
    ToolSlot {
        id: ToolId::Pipeline,
        label: "Pipeline",
        view: cx
            .new(|cx| view::PipelineView::new(model.clone(), window, cx))
            .into(),
        hooks: Box::new(Hooks { model }),
    }
}
