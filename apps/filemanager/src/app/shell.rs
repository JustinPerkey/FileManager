//! The window's root view: window-level behaviour only. It holds the current
//! tool, the tool slots from the registry and the chrome view, routes
//! Explorer drops to the current tool, holds the window closed while a tool
//! has unsaved work, and keeps the notice the chrome shows. It renders the
//! chrome as its only child.

use std::path::PathBuf;
use std::time::Instant;

use gpui::{
    div, AnyView, App, AppContext as _, Context, DragMoveEvent, Entity, ExternalPaths,
    FileDropEvent, Global, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    Styled as _, Task, Window,
};

use crate::app::drop::DropTarget;
use crate::app::guard::{self, CloseRequest};
use crate::app::text;
use crate::app::view;
use crate::platform::path_loss::Coalescer;
use crate::tools::{self, ToolId, ToolSlot};

/// The channel the path-loss logger signals on, as a GPUI global. `run`
/// sets it; the shell drains it. Tests set their own.
pub struct PathLossFeed(pub async_channel::Receiver<()>);

impl Global for PathLossFeed {}

/// A message the shell keeps until it is dismissed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellNotice {
    /// A dropped item's name is not valid Unicode, so the OS drop skipped it.
    DroppedNamesUnreadable,
}

impl ShellNotice {
    /// The user-facing copy.
    #[must_use]
    pub fn message(self) -> &'static str {
        match self {
            ShellNotice::DroppedNamesUnreadable => text::DROPPED_NAMES_UNREADABLE,
        }
    }
}

pub struct Shell {
    current: ToolId,
    tools: Vec<ToolSlot>,
    chrome: Option<AnyView>,
    drag_over: Option<DropTarget>,
    file_drag: bool,
    coalescer: Coalescer,
    notice: Option<ShellNotice>,
    close_request: Option<CloseRequest>,
    _path_loss: Option<Task<()>>,
}

impl Shell {
    /// Builds the registry's tools and the chrome, and returns the shell.
    pub fn open(window: &mut Window, cx: &mut App) -> Entity<Self> {
        let tools = tools::build(window, cx);
        Self::with_tools(tools, window, cx)
    }

    /// A shell over the given tools; the first one is current. Registers the
    /// window's close guard (once per window) and starts listening for
    /// path-loss signals if a [`PathLossFeed`] is set.
    ///
    /// # Panics
    /// If `tools` is empty.
    pub fn with_tools(tools: Vec<ToolSlot>, window: &mut Window, cx: &mut App) -> Entity<Self> {
        let current = tools.first().expect("at least one tool").id;
        let feed = cx.try_global::<PathLossFeed>().map(|f| f.0.clone());
        let shell = cx.new(|cx| Shell {
            current,
            tools,
            chrome: None,
            drag_over: None,
            file_drag: false,
            coalescer: Coalescer::default(),
            notice: None,
            close_request: None,
            _path_loss: feed.map(|rx: async_channel::Receiver<()>| {
                cx.spawn(async move |this: gpui::WeakEntity<Shell>, cx| {
                    while rx.recv().await.is_ok() {
                        let alive = this.update(cx, Shell::path_loss_signal);
                        if alive.is_err() {
                            break;
                        }
                    }
                })
            }),
        });
        let weak = shell.downgrade();
        window.on_window_should_close(cx, move |_window, cx| {
            weak.update(cx, Shell::should_close).unwrap_or(true)
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
        self.refresh_drag_over(cx);
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

    /// `Some` while a file drag is over the window and the current tool has
    /// a drop target: the chrome shows its label, or its reason when it is
    /// disabled.
    #[must_use]
    pub fn drag_over(&self) -> Option<&DropTarget> {
        self.drag_over.as_ref()
    }

    /// The notice the chrome shows, until [`Shell::dismiss_notice`].
    #[must_use]
    pub fn notice(&self) -> Option<ShellNotice> {
        self.notice
    }

    /// Clears the notice.
    pub fn dismiss_notice(&mut self, cx: &mut Context<Self>) {
        if self.notice.take().is_some() {
            cx.notify();
        }
    }

    /// Set while the window is held open because a tool has unsaved work.
    /// The chrome shows the prompt and calls [`Shell::confirm_close`] or
    /// [`Shell::cancel_close`].
    #[must_use]
    pub fn close_request(&self) -> Option<&CloseRequest> {
        self.close_request.as_ref()
    }

    /// Asks to close the window by the same path as the title-bar X: closes
    /// at once if no tool has unsaved work, otherwise sets the close request.
    pub fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.should_close(cx) {
            window.remove_window();
        }
    }

    /// Closes the window, which quits the app, discarding the unsaved work
    /// the request listed.
    pub fn confirm_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_request = None;
        cx.notify();
        window.remove_window();
    }

    /// Clears the close request; the window stays open.
    pub fn cancel_close(&mut self, cx: &mut Context<Self>) {
        if self.close_request.take().is_some() {
            cx.notify();
        }
    }

    /// The should-close decision shared by the window hook and
    /// [`Shell::request_close`]: true if the window may close now.
    fn should_close(&mut self, cx: &mut Context<Self>) -> bool {
        if self.close_request.is_some() {
            return false;
        }
        let items = guard::collect(&self.tools, cx);
        if items.is_empty() {
            return true;
        }
        self.close_request = Some(CloseRequest { items });
        cx.notify();
        false
    }

    fn current_target(&self, cx: &App) -> DropTarget {
        self.tools
            .iter()
            .find(|s| s.id == self.current)
            .map_or(DropTarget::None, |s| s.hooks.drop_target(cx))
    }

    fn refresh_drag_over(&mut self, cx: &mut Context<Self>) {
        let next = if self.file_drag {
            match self.current_target(cx) {
                DropTarget::None => None,
                target => Some(target),
            }
        } else {
            None
        };
        if next != self.drag_over {
            self.drag_over = next;
            cx.notify();
        }
    }

    fn drag_entered(&mut self, cx: &mut Context<Self>) {
        if !self.file_drag {
            self.file_drag = true;
            self.coalescer.drag_started();
        }
        self.refresh_drag_over(cx);
    }

    fn drag_left(&mut self, cx: &mut Context<Self>) {
        if self.file_drag {
            self.file_drag = false;
            self.coalescer.drag_ended(Instant::now());
        }
        self.refresh_drag_over(cx);
    }

    fn drop_paths(&mut self, paths: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        self.drag_left(cx);
        if paths.is_empty() {
            return;
        }
        if !matches!(self.current_target(cx), DropTarget::Enabled { .. }) {
            return;
        }
        if let Some(slot) = self.tools.iter().find(|s| s.id == self.current) {
            slot.hooks.dropped(paths, window, cx);
        }
    }

    fn path_loss_signal(&mut self, cx: &mut Context<Self>) {
        if self.coalescer.signal(Instant::now()) {
            self.notice = Some(ShellNotice::DroppedNamesUnreadable);
            cx.notify();
        }
    }
}

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = div()
            .key_context("Shell")
            .size_full()
            .on_drag_move(
                cx.listener(|this, _: &DragMoveEvent<ExternalPaths>, _window, cx| {
                    this.drag_entered(cx);
                }),
            )
            .on_file_drop_exit(cx.listener(|this, _: &FileDropEvent, _window, cx| {
                this.drag_left(cx);
            }))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                this.drop_paths(paths.paths().to_vec(), window, cx);
            }));
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

    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::app::dev_mode::DevMode;
    use crate::tools::ToolHooks;

    /// What a [`Probe`] tool reports and records.
    #[derive(Default)]
    pub struct ProbeState {
        pub target: DropTarget,
        pub unsaved: Vec<gpui::SharedString>,
        pub dropped: Vec<Vec<PathBuf>>,
    }

    /// Shared handle to a probe's state.
    pub type SharedProbe = Rc<RefCell<ProbeState>>;

    /// A tool whose hooks read and write a shared [`ProbeState`].
    pub struct Probe(pub Rc<RefCell<ProbeState>>);

    impl ToolHooks for Probe {
        fn drop_target(&self, _cx: &App) -> DropTarget {
            self.0.borrow().target.clone()
        }

        fn dropped(&self, paths: Vec<PathBuf>, _window: &mut Window, _cx: &mut App) {
            self.0.borrow_mut().dropped.push(paths);
        }

        fn unsaved_work(&self, _cx: &App) -> Vec<gpui::SharedString> {
            self.0.borrow().unsaved.clone()
        }
    }

    /// The registry's tools with a [`Probe`] on each; the states come back in
    /// registry order (Pipeline, Tarpack, Schedule).
    pub fn probes() -> (
        Vec<SharedProbe>,
        impl FnOnce(&mut Window, &mut App) -> Vec<ToolSlot>,
    ) {
        let states: Vec<_> = (0..3).map(|_| Rc::default()).collect();
        let given = states.clone();
        let build = move |window: &mut Window, cx: &mut App| {
            tools::build(window, cx)
                .into_iter()
                .zip(given)
                .map(|(slot, state)| ToolSlot {
                    hooks: Box::new(Probe(state)),
                    ..slot
                })
                .collect()
        };
        (states, build)
    }

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

    fn enabled() -> DropTarget {
        DropTarget::Enabled {
            label: "Drop here".into(),
        }
    }

    fn external(paths: &[&str]) -> ExternalPaths {
        ExternalPaths(paths.iter().map(PathBuf::from).collect())
    }

    fn inside() -> gpui::Point<gpui::Pixels> {
        gpui::point(gpui::px(300.0), gpui::px(300.0))
    }

    fn drop_on(cx: &mut gpui::VisualTestContext, paths: ExternalPaths) {
        cx.simulate_event(FileDropEvent::Entered {
            position: inside(),
            paths,
        });
        cx.simulate_event(FileDropEvent::Pending { position: inside() });
        cx.simulate_event(FileDropEvent::Submit { position: inside() });
    }

    #[gpui::test]
    fn drop_reaches_current_tool_when_enabled(cx: &mut TestAppContext) {
        let (states, build) = test_support::probes();
        let (_shell, cx) = open(cx, build);
        states[0].borrow_mut().target = enabled();
        drop_on(cx, external(&["C:\\drop\\a.txt", "C:\\drop\\folder"]));
        assert_eq!(
            states[0].borrow().dropped,
            [vec![
                PathBuf::from("C:\\drop\\a.txt"),
                PathBuf::from("C:\\drop\\folder")
            ]]
        );
    }

    #[gpui::test]
    fn drop_ignored_when_disabled(cx: &mut TestAppContext) {
        let (states, build) = test_support::probes();
        let (shell, cx) = open(cx, build);
        let disabled = DropTarget::Disabled {
            reason: "Busy".into(),
        };
        states[0].borrow_mut().target = disabled.clone();
        cx.simulate_event(FileDropEvent::Entered {
            position: inside(),
            paths: external(&["C:\\drop\\a.txt"]),
        });
        assert_eq!(
            shell.read_with(cx, |s, _| s.drag_over().cloned()),
            Some(disabled)
        );
        cx.simulate_event(FileDropEvent::Submit { position: inside() });
        assert_eq!(states[0].borrow().dropped.len(), 0);
        assert_eq!(shell.read_with(cx, |s, _| s.drag_over().cloned()), None);
    }

    #[gpui::test]
    fn empty_drop_ignored(cx: &mut TestAppContext) {
        let (states, build) = test_support::probes();
        let (_shell, cx) = open(cx, build);
        states[0].borrow_mut().target = enabled();
        drop_on(cx, external(&[]));
        assert_eq!(states[0].borrow().dropped.len(), 0);
    }

    #[gpui::test]
    fn drag_over_tracks_enter_and_exit(cx: &mut TestAppContext) {
        let (states, build) = test_support::probes();
        let (shell, cx) = open(cx, build);
        let over =
            |cx: &mut gpui::VisualTestContext| shell.read_with(cx, |s, _| s.drag_over().cloned());
        assert_eq!(over(cx), None);
        cx.simulate_event(FileDropEvent::Entered {
            position: inside(),
            paths: external(&["C:\\drop\\a.txt"]),
        });
        assert_eq!(over(cx), None, "no target: no overlay");
        cx.simulate_event(FileDropEvent::Exited);
        states[0].borrow_mut().target = enabled();
        cx.simulate_event(FileDropEvent::Entered {
            position: inside(),
            paths: external(&["C:\\drop\\a.txt"]),
        });
        assert_eq!(over(cx), Some(enabled()));
        cx.simulate_event(FileDropEvent::Exited);
        assert_eq!(over(cx), None, "leave hides the overlay");
    }

    #[gpui::test]
    fn drop_goes_to_the_current_tool_only(cx: &mut TestAppContext) {
        let (states, build) = test_support::probes();
        let (shell, cx) = open(cx, build);
        for state in &states {
            state.borrow_mut().target = enabled();
        }
        cx.update(|window, cx| {
            shell.update(cx, |s, cx| s.select(ToolId::Tarpack, window, cx));
        });
        drop_on(cx, external(&["C:\\drop\\a.txt"]));
        assert_eq!(states[0].borrow().dropped.len(), 0);
        assert_eq!(states[1].borrow().dropped.len(), 1);
        assert_eq!(states[2].borrow().dropped.len(), 0);
    }

    /// A name with a lone surrogate must survive routing.
    #[cfg(windows)]
    #[gpui::test]
    fn dropped_non_unicode_path_survives_routing(cx: &mut TestAppContext) {
        use std::os::windows::ffi::OsStringExt as _;
        let name = std::ffi::OsString::from_wide(&[0x61, 0xD800, 0x62]);
        let path = PathBuf::from("C:\\drop").join(name);
        assert!(path.to_str().is_none());
        let (states, build) = test_support::probes();
        let (_shell, cx) = open(cx, build);
        states[0].borrow_mut().target = enabled();
        drop_on(cx, ExternalPaths([path.clone()].into_iter().collect()));
        assert_eq!(states[0].borrow().dropped, [vec![path]]);
    }

    #[gpui::test]
    fn path_loss_shows_one_notice_per_drag(cx: &mut TestAppContext) {
        let (tx, rx) = async_channel::unbounded();
        cx.update(|cx| cx.set_global(PathLossFeed(rx)));
        let (shell, cx) = open(cx, tools::build);
        let notice = |cx: &mut gpui::VisualTestContext| shell.read_with(cx, |s, _| s.notice());
        assert_eq!(notice(cx), None);
        cx.simulate_event(FileDropEvent::Entered {
            position: inside(),
            paths: external(&["C:\\drop\\a.txt"]),
        });
        for _ in 0..3 {
            tx.try_send(()).expect("open channel");
        }
        cx.run_until_parked();
        assert_eq!(notice(cx), Some(ShellNotice::DroppedNamesUnreadable));
        shell.update(cx, Shell::dismiss_notice);
        assert_eq!(notice(cx), None);
        // Later signals of the same drag do not bring it back.
        tx.try_send(()).expect("open channel");
        cx.run_until_parked();
        assert_eq!(notice(cx), None, "one notice per drag");
        cx.simulate_event(FileDropEvent::Submit { position: inside() });
        // The next drag raises it again.
        cx.simulate_event(FileDropEvent::Entered {
            position: inside(),
            paths: external(&["C:\\drop\\b.txt"]),
        });
        tx.try_send(()).expect("open channel");
        cx.run_until_parked();
        assert_eq!(notice(cx), Some(ShellNotice::DroppedNamesUnreadable));
    }
}
