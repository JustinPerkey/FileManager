//! Schedule Creator: the model, its backend, its copy and (for tests and the
//! harness) its fixtures. The view is the ui-implementer's; `view/` holds a
//! plain stand-in until it replaces it.

pub mod backend;
#[cfg(any(test, feature = "shoot"))]
pub mod fixtures;
pub mod model;
pub mod text;
pub mod view;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

use gpui::{App, AppContext as _, Entity, Window};

pub use self::model::ScheduleModel;
use crate::app::drop::DropTarget;
use crate::tools::{Backend, BackendSet, ToolHooks, ToolId, ToolSlot};

use self::backend::{RealSchedule, ScheduleBackend};

struct Hooks {
    model: Entity<ScheduleModel>,
}

impl ToolHooks for Hooks {
    fn activated(&self, _window: &mut Window, cx: &mut App) {
        self.model.update(cx, ScheduleModel::activated);
    }

    fn drop_target(&self, cx: &App) -> DropTarget {
        let model = self.model.read(cx);
        if model.drop_ready() {
            DropTarget::Enabled {
                label: text::DROP_LABEL.into(),
            }
        } else {
            let reason = if model.applying() {
                text::REASON_APPLYING
            } else if model.pending().is_some() {
                text::REASON_CONFIRMING
            } else {
                text::REASON_NOT_READY
            };
            DropTarget::Disabled {
                reason: reason.into(),
            }
        }
    }

    fn dropped(&self, paths: Vec<PathBuf>, _window: &mut Window, cx: &mut App) {
        self.model.update(cx, |m, cx| m.set_dropped(paths, cx));
    }
}

fn backend_for(cx: &App) -> Box<dyn ScheduleBackend> {
    match BackendSet::get(cx).schedule {
        Backend::Real => Box::new(RealSchedule::new()),
        #[cfg(any(test, feature = "shoot"))]
        Backend::Fixture(crate::tools::FixtureId::Schedule(fixture)) => {
            Box::new(fixtures::FixtureSchedule::from_fixture(fixture))
        }
        #[cfg(any(test, feature = "shoot"))]
        Backend::Fixture(other) => panic!("not a Schedule fixture: {other:?}"),
    }
}

pub(super) fn slot(_window: &mut Window, cx: &mut App) -> ToolSlot {
    let backend = backend_for(cx);
    let model = cx.new(|cx| ScheduleModel::new(backend, cx));
    ToolSlot {
        id: ToolId::Schedule,
        label: "Schedule Creator",
        view: cx
            .new(|cx| view::ScheduleView::new(model.clone(), cx))
            .into(),
        hooks: Box::new(Hooks { model }),
    }
}
