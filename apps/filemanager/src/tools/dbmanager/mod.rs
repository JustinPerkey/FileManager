//! Database Manager: the model (read side), its backend, its copy and (for
//! tests and the harness) its fixtures. The view is the ui-implementer's;
//! `view/` holds a plain stand-in until it replaces it.

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

pub use self::model::DbmanagerModel;
use crate::app::drop::DropTarget;
use crate::tools::{Backend, BackendSet, ToolHooks, ToolId, ToolSlot};

use self::backend::{DbmanagerBackend, RealDbmanager};

struct Hooks {
    model: Entity<DbmanagerModel>,
}

impl ToolHooks for Hooks {
    fn activated(&self, _window: &mut Window, cx: &mut App) {
        self.model.update(cx, DbmanagerModel::activated);
    }

    fn drop_target(&self, cx: &App) -> DropTarget {
        match self.model.read(cx).drop_availability() {
            Ok(()) => DropTarget::Enabled {
                label: text::DROP_LABEL.into(),
            },
            Err(reason) => DropTarget::Disabled {
                reason: reason.into(),
            },
        }
    }

    fn dropped(&self, paths: Vec<PathBuf>, _window: &mut Window, cx: &mut App) {
        self.model.update(cx, |m, cx| m.dropped(paths, cx));
    }
}

fn backend_for(cx: &App) -> Box<dyn DbmanagerBackend> {
    match BackendSet::get(cx).dbmanager {
        Backend::Real => Box::new(RealDbmanager::new()),
        #[cfg(any(test, feature = "shoot"))]
        Backend::Fixture(crate::tools::FixtureId::Dbmanager(fixture)) => {
            Box::new(fixtures::FixtureDbmanager::from_fixture(fixture))
        }
        #[cfg(any(test, feature = "shoot"))]
        Backend::Fixture(other) => panic!("not a Database Manager fixture: {other:?}"),
    }
}

pub(super) fn slot(_window: &mut Window, cx: &mut App) -> ToolSlot {
    let backend = backend_for(cx);
    let model = cx.new(|cx| DbmanagerModel::new(backend, cx));
    ToolSlot {
        id: ToolId::Dbmanager,
        label: "Database Manager",
        view: cx
            .new(|cx| view::DbmanagerView::new(model.clone(), cx))
            .into(),
        hooks: Box::new(Hooks { model }),
    }
}
