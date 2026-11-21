//! The window's root view: window-level behaviour only. It holds the current
//! tool, the tool slots from the registry and the chrome view, and renders the
//! chrome as its only child.

use gpui::{
    div, AnyView, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Styled as _, Window,
};

use crate::app::view;
use crate::tools::{self, ToolId, ToolSlot};

pub struct Shell {
    current: ToolId,
    tools: Vec<ToolSlot>,
    chrome: Option<AnyView>,
}

impl Shell {
    /// Builds the registry's tools and the chrome, and returns the shell.
    pub fn open(window: &mut Window, cx: &mut App) -> Entity<Self> {
        let tools = tools::build(window, cx);
        Self::with_tools(tools, window, cx)
    }

    /// A shell over the given tools; the first one is current.
    ///
    /// # Panics
    /// If `tools` is empty.
    pub fn with_tools(tools: Vec<ToolSlot>, window: &mut Window, cx: &mut App) -> Entity<Self> {
        let current = tools.first().expect("at least one tool").id;
        let shell = cx.new(|_| Shell {
            current,
            tools,
            chrome: None,
        });
        let chrome = view::chrome(shell.clone(), window, cx);
        shell.update(cx, |shell, _cx| {
            shell.chrome = Some(chrome);
        });
        shell
    }

    /// The current tool.
    #[must_use]
    pub fn current(&self) -> ToolId {
        self.current
    }

    /// Makes `id` the current tool and calls its `activated` hook.
    pub fn select(&mut self, id: ToolId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(slot) = self.tools.iter().find(|s| s.id == id) else {
            return;
        };
        self.current = id;
        slot.hooks.activated(window, cx);
        cx.notify();
    }

    /// The tool slots, in registry order.
    #[must_use]
    pub fn tools(&self) -> &[ToolSlot] {
        &self.tools
    }

    /// The tool's view, for the chrome to place.
    ///
    /// # Panics
    /// If `id` is not in the registry.
    #[must_use]
    pub fn tool_view(&self, id: ToolId) -> AnyView {
        self.tools
            .iter()
            .find(|s| s.id == id)
            .expect("every ToolId is in the registry")
            .view
            .clone()
    }
}

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let root = div().key_context("Shell").size_full();
        match &self.chrome {
            Some(chrome) => root.child(chrome.clone()),
            None => root,
        }
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use gpui::{TestAppContext, VisualTestContext};
    use gpui_component::Root;

    use super::*;
    use crate::app::dev_mode::DevMode;

    /// The shell inside a `Root`, in a fresh window; nothing is focused.
    pub fn open(
        cx: &mut TestAppContext,
        tools: impl FnOnce(&mut Window, &mut App) -> Vec<ToolSlot>,
    ) -> (Entity<Shell>, &mut VisualTestContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            DevMode::init(cx);
        });
        let mut shell = None;
        let window = cx.add_window(|window, cx| {
            let slots = tools(window, cx);
            let entity = Shell::with_tools(slots, window, cx);
            shell = Some(entity.clone());
            Root::new(entity, window, cx)
        });
        let vcx = VisualTestContext::from_window(window.into(), cx).into_mut();
        vcx.run_until_parked();
        (shell.expect("shell created"), vcx)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{Modifiers, TestAppContext};

    use super::test_support::open;
    use super::*;
    use crate::tools::ToolHooks;

    struct Counting {
        id: ToolId,
        log: Rc<RefCell<Vec<ToolId>>>,
    }

    impl ToolHooks for Counting {
        fn activated(&self, _window: &mut Window, _cx: &mut App) {
            self.log.borrow_mut().push(self.id);
        }
    }

    fn counting_tools(
        log: &Rc<RefCell<Vec<ToolId>>>,
    ) -> impl FnOnce(&mut Window, &mut App) -> Vec<ToolSlot> {
        let log = log.clone();
        move |window, cx| {
            tools::build(window, cx)
                .into_iter()
                .map(|slot| ToolSlot {
                    hooks: Box::new(Counting {
                        id: slot.id,
                        log: log.clone(),
                    }),
                    ..slot
                })
                .collect()
        }
    }

    #[gpui::test]
    fn select_changes_current_and_activates(cx: &mut TestAppContext) {
        let log = Rc::new(RefCell::new(Vec::new()));
        let (shell, cx) = open(cx, counting_tools(&log));
        assert_eq!(shell.read_with(cx, |s, _| s.current()), ToolId::Pipeline);
        assert!(log.borrow().is_empty());
        cx.update(|window, cx| {
            shell.update(cx, |s, cx| s.select(ToolId::Tarpack, window, cx));
        });
        assert_eq!(shell.read_with(cx, |s, _| s.current()), ToolId::Tarpack);
        assert_eq!(
            *log.borrow(),
            [ToolId::Tarpack],
            "activated once, on the new tool"
        );
    }

    #[gpui::test]
    fn placeholder_nav_selects_by_click(cx: &mut TestAppContext) {
        let (shell, cx) = open(cx, tools::build);
        let at = cx
            .debug_bounds("tool-nav-schedule")
            .expect("the placeholder nav has a schedule button")
            .center();
        cx.simulate_click(at, Modifiers::none());
        assert_eq!(shell.read_with(cx, |s, _| s.current()), ToolId::Schedule);
    }
}
