//! The window's root view: window-level behaviour only. It holds the current
//! tool, the tool slots from the registry and the chrome view, routes
//! Explorer drops to the current tool, holds the window closed while a tool
//! has unsaved work, and keeps the notice the chrome shows. It renders the
//! chrome as its only child.

use std::path::PathBuf;

use gpui::{
    canvas, div, AnyView, App, AppContext as _, Context, DispatchPhase, DragMoveEvent, Entity,
    ExternalPaths, FileDropEvent, Global, InteractiveElement as _, IntoElement, MouseUpEvent,
    ParentElement as _, Render, Styled as _, Task, Window,
};

use gpui_component::WindowExt as _;

use crate::app::drop::DropTarget;
use crate::app::guard::{self, CloseRequest};
use crate::app::probe;
use crate::app::text;
use crate::app::view;
use crate::platform::path_loss::{Coalescer, Signal};
use crate::tools::{self, ToolId, ToolSlot};

/// The channel the path-loss logger signals on, as a GPUI global. `run`
/// sets it; the shell drains it. Tests set their own.
pub struct PathLossFeed(pub async_channel::Receiver<Signal>);

impl Global for PathLossFeed {}

/// A message the shell keeps until it is dismissed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellNotice {
    /// A dropped item's name is not valid Unicode, so the OS drop skipped it.
    DroppedNamesUnreadable,
    /// GPUI could not read a dropped item's name at all, so it skipped it.
    DroppedItemsUnreadable,
}

impl ShellNotice {
    /// The user-facing copy.
    #[must_use]
    pub fn message(self) -> &'static str {
        match self {
            ShellNotice::DroppedNamesUnreadable => text::DROPPED_NAMES_UNREADABLE,
            ShellNotice::DroppedItemsUnreadable => text::DROPPED_ITEMS_UNREADABLE,
        }
    }
}

pub struct Shell {
    current: ToolId,
    tools: Vec<ToolSlot>,
    chrome: Option<AnyView>,
    drag_over: Option<DropTarget>,
    file_drag: bool,
    /// The paths of the file drag in progress, kept for the drop.
    drag_paths: Vec<PathBuf>,
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
            drag_paths: Vec::new(),
            coalescer: Coalescer::default(),
            notice: None,
            close_request: None,
            _path_loss: feed.map(|rx: async_channel::Receiver<Signal>| {
                cx.spawn(async move |this: gpui::WeakEntity<Shell>, cx| {
                    while let Ok(signal) = rx.recv().await {
                        let alive = this.update(cx, |shell, cx| shell.path_loss_signal(signal, cx));
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
        self.refresh_drag_over(window, cx);
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

    /// What the current tool's target is right now. While a modal layer
    /// (dialog or sheet) is open a drop is refused, so whatever the tool
    /// reports reads as disabled with the reason.
    fn current_target(&self, window: &mut Window, cx: &mut App) -> DropTarget {
        if window.has_active_dialog(cx) || window.has_active_sheet(cx) {
            return DropTarget::Disabled {
                reason: text::DROP_BLOCKED_BY_DIALOG.into(),
            };
        }
        self.tools
            .iter()
            .find(|s| s.id == self.current)
            .map_or(DropTarget::None, |s| s.hooks.drop_target(cx))
    }

    fn refresh_drag_over(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let next = if self.file_drag {
            match self.current_target(window, cx) {
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

    fn drag_entered(&mut self, paths: &ExternalPaths, window: &mut Window, cx: &mut Context<Self>) {
        if !self.file_drag {
            self.file_drag = true;
            self.drag_paths = paths.paths().to_vec();
            self.coalescer.drag_started();
        }
        self.refresh_drag_over(window, cx);
    }

    /// The drag ended (left the window, or was dropped, accepted or not).
    fn drag_left(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.file_drag {
            self.file_drag = false;
            self.coalescer.drag_ended(cx.background_executor().now());
        }
        self.drag_paths.clear();
        self.refresh_drag_over(window, cx);
    }

    /// The drop: delivers the drag's paths to the current tool if it accepts
    /// them. Called once per drag, from the window-level mouse-up.
    fn drop_paths(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = std::mem::take(&mut self.drag_paths);
        let target = self.current_target(window, cx);
        self.drag_left(window, cx);
        if paths.is_empty() || !matches!(target, DropTarget::Enabled { .. }) {
            return;
        }
        if let Some(slot) = self.tools.iter().find(|s| s.id == self.current) {
            slot.hooks.dropped(paths, window, cx);
        }
    }

    fn path_loss_signal(&mut self, signal: Signal, cx: &mut Context<Self>) {
        let now = cx.background_executor().now();
        if let Some(kind) = self.coalescer.signal(signal, now) {
            self.notice = Some(match kind {
                Signal::NonUnicodeName => ShellNotice::DroppedNamesUnreadable,
                Signal::UnreadableName => ShellNotice::DroppedItemsUnreadable,
            });
            cx.notify();
        }
    }
}

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The end of a drag is tracked by window-level capture listeners, not
        // element listeners: an element only hears events while its hitbox is
        // hovered, and a dialog, sheet or popover above the Shell takes that
        // away. The drag-move listener only starts the tracking; the drop is delivered from the
        // capture-phase mouse-up, exactly once, wherever the pointer is.
        let weak = cx.entity().downgrade();
        let tracker = canvas(
            |_, _, _| probe::begin_frame(),
            move |_, (), window, _cx| {
                let on_exit = weak.clone();
                window.on_mouse_event(move |event: &FileDropEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture
                        && matches!(event, FileDropEvent::Exited | FileDropEvent::Ended)
                    {
                        on_exit
                            .update(cx, |shell, cx| shell.drag_left(window, cx))
                            .ok();
                    }
                });
                let on_up = weak.clone();
                window.on_mouse_event(move |_: &MouseUpEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture {
                        on_up
                            .update(cx, |shell, cx| {
                                if shell.file_drag {
                                    shell.drop_paths(window, cx);
                                }
                            })
                            .ok();
                    }
                });
            },
        )
        .absolute()
        .size_full();
        let root = div()
            .key_context("Shell")
            .size_full()
            .child(tracker)
            .on_drag_move(
                cx.listener(|this, event: &DragMoveEvent<ExternalPaths>, window, cx| {
                    let paths = event.drag(cx).clone();
                    this.drag_entered(&paths, window, cx);
                }),
            );
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

    use crate::platform::path_loss::Signal;

    const NON_UNICODE: Signal = Signal::NonUnicodeName;
    const UNREADABLE: Signal = Signal::UnreadableName;

    fn feed(cx: &mut TestAppContext) -> async_channel::Sender<Signal> {
        let (tx, rx) = async_channel::unbounded();
        cx.update(|cx| cx.set_global(PathLossFeed(rx)));
        tx
    }

    fn notice(shell: &Entity<Shell>, cx: &mut gpui::VisualTestContext) -> Option<ShellNotice> {
        shell.read_with(cx, |s, _| s.notice())
    }

    fn enter(cx: &mut gpui::VisualTestContext) {
        cx.simulate_event(FileDropEvent::Entered {
            position: inside(),
            paths: external(&["C:\\drop\\a.txt"]),
        });
    }

    fn send(tx: &async_channel::Sender<Signal>, signal: Signal, cx: &mut gpui::VisualTestContext) {
        tx.try_send(signal).expect("open channel");
        cx.run_until_parked();
    }

    #[gpui::test]
    fn path_loss_outside_a_drag_is_ignored(cx: &mut TestAppContext) {
        let tx = feed(cx);
        let (shell, cx) = open(cx, tools::build);
        send(&tx, NON_UNICODE, cx);
        send(&tx, UNREADABLE, cx);
        assert_eq!(notice(&shell, cx), None, "a paste logs the same text");
    }

    #[gpui::test]
    fn shell_starts_a_probe_frame_on_every_draw(cx: &mut TestAppContext) {
        let tx = feed(cx);
        let (shell, cx) = open(cx, tools::build);
        enter(cx);
        tx.try_send(NON_UNICODE).expect("open channel");
        cx.run_until_parked();
        assert!(
            probe::bounds("shell-notice").is_some(),
            "the notice is drawn"
        );
        // Hide the tagged element and redraw. No manual `begin_frame`: only the
        // Shell's own call can make the stale entry stop resolving.
        shell.update(cx, Shell::dismiss_notice);
        cx.run_until_parked();
        assert_eq!(probe::bounds("shell-notice"), None);
    }

    #[gpui::test]
    fn path_loss_during_drag_shows_one_notice(cx: &mut TestAppContext) {
        let tx = feed(cx);
        let (shell, cx) = open(cx, tools::build);
        enter(cx);
        for _ in 0..3 {
            tx.try_send(NON_UNICODE).expect("open channel");
        }
        cx.run_until_parked();
        assert_eq!(
            notice(&shell, cx),
            Some(ShellNotice::DroppedNamesUnreadable)
        );
        shell.update(cx, Shell::dismiss_notice);
        send(&tx, NON_UNICODE, cx);
        assert_eq!(notice(&shell, cx), None, "one notice per drag");
        cx.simulate_event(FileDropEvent::Submit { position: inside() });
        enter(cx);
        send(&tx, NON_UNICODE, cx);
        assert_eq!(
            notice(&shell, cx),
            Some(ShellNotice::DroppedNamesUnreadable),
            "the next drag raises it again"
        );
    }

    #[gpui::test]
    fn path_loss_within_1s_after_drag_counts(cx: &mut TestAppContext) {
        let tx = feed(cx);
        let (shell, cx) = open(cx, tools::build);
        enter(cx);
        cx.simulate_event(FileDropEvent::Submit { position: inside() });
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(900));
        send(&tx, NON_UNICODE, cx);
        assert_eq!(
            notice(&shell, cx),
            Some(ShellNotice::DroppedNamesUnreadable)
        );
    }

    #[gpui::test]
    fn path_loss_after_1s_is_ignored(cx: &mut TestAppContext) {
        let tx = feed(cx);
        let (shell, cx) = open(cx, tools::build);
        enter(cx);
        cx.simulate_event(FileDropEvent::Exited);
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(1100));
        send(&tx, NON_UNICODE, cx);
        assert_eq!(notice(&shell, cx), None);
    }

    #[gpui::test]
    fn unreadable_name_shows_its_own_notice(cx: &mut TestAppContext) {
        let tx = feed(cx);
        let (shell, cx) = open(cx, tools::build);
        enter(cx);
        send(&tx, UNREADABLE, cx);
        let shown = notice(&shell, cx);
        assert_eq!(shown, Some(ShellNotice::DroppedItemsUnreadable));
        assert_eq!(
            shown.map(ShellNotice::message),
            Some("Some dropped items could not be read. Drop them again, or use Browse.")
        );
    }

    #[gpui::test]
    fn non_unicode_notice_wins_when_both_arrive(cx: &mut TestAppContext) {
        let tx = feed(cx);
        let (shell, cx) = open(cx, tools::build);
        enter(cx);
        send(&tx, UNREADABLE, cx);
        send(&tx, NON_UNICODE, cx);
        assert_eq!(
            notice(&shell, cx),
            Some(ShellNotice::DroppedNamesUnreadable)
        );
        let tx2 = tx.clone();
        shell.update(cx, Shell::dismiss_notice);
        send(&tx2, UNREADABLE, cx);
        assert_eq!(notice(&shell, cx), None, "the other kind does not return");
    }

    #[gpui::test]
    fn drop_refused_while_dialog_open(cx: &mut TestAppContext) {
        let (states, build) = test_support::probes();
        let (shell, cx) = open(cx, build);
        states[0].borrow_mut().target = enabled();
        open_dialog(cx);
        enter(cx);
        assert_eq!(
            shell.read_with(cx, |s, _| s.drag_over().cloned()),
            Some(DropTarget::Disabled {
                reason: "Close the open dialog first.".into()
            })
        );
        // Whatever the tool reports.
        states[0].borrow_mut().target = DropTarget::None;
        cx.simulate_event(FileDropEvent::Pending { position: inside() });
        assert_eq!(
            shell.read_with(cx, |s, _| s.drag_over().cloned()),
            Some(DropTarget::Disabled {
                reason: "Close the open dialog first.".into()
            })
        );
        cx.simulate_event(FileDropEvent::Submit { position: inside() });
        assert_eq!(states[0].borrow().dropped.len(), 0);
        assert_eq!(shell.read_with(cx, |s, _| s.drag_over().cloned()), None);
    }

    fn open_dialog(cx: &mut gpui::VisualTestContext) {
        cx.update(|window, cx| {
            window.open_dialog(cx, |dialog, _, _| dialog.title("A dialog"));
        });
        cx.run_until_parked();
    }

    /// With a dialog open: the overlay says refused, the end of the drag is
    /// seen anyway, nothing is dropped, and the next path-loss notice is not
    /// suppressed.
    #[gpui::test]
    fn dialog_open_drag_exit_clears_and_resets(cx: &mut TestAppContext) {
        let tx = feed(cx);
        let (states, build) = test_support::probes();
        let (shell, cx) = open(cx, build);
        states[0].borrow_mut().target = enabled();
        open_dialog(cx);
        cx.simulate_event(FileDropEvent::Entered {
            position: inside(),
            paths: external(&["C:\\drop\\a.txt"]),
        });
        assert!(matches!(
            shell.read_with(cx, |s, _| s.drag_over().cloned()),
            Some(DropTarget::Disabled { .. })
        ));
        tx.try_send(NON_UNICODE).expect("open channel");
        cx.run_until_parked();
        assert!(shell.read_with(cx, |s, _| s.notice()).is_some());
        shell.update(cx, Shell::dismiss_notice);
        cx.simulate_event(FileDropEvent::Exited);
        assert_eq!(shell.read_with(cx, |s, _| s.drag_over().cloned()), None);
        // A new drag: the notice comes back.
        cx.simulate_event(FileDropEvent::Entered {
            position: inside(),
            paths: external(&["C:\\drop\\b.txt"]),
        });
        tx.try_send(NON_UNICODE).expect("open channel");
        cx.run_until_parked();
        assert!(shell.read_with(cx, |s, _| s.notice()).is_some());
        assert_eq!(states[0].borrow().dropped.len(), 0);
    }

    #[gpui::test]
    fn dialog_open_drop_is_refused_and_ends_the_drag(cx: &mut TestAppContext) {
        let tx = feed(cx);
        let (states, build) = test_support::probes();
        let (shell, cx) = open(cx, build);
        states[0].borrow_mut().target = enabled();
        open_dialog(cx);
        drop_on(cx, external(&["C:\\drop\\a.txt"]));
        assert_eq!(shell.read_with(cx, |s, _| s.drag_over().cloned()), None);
        assert_eq!(states[0].borrow().dropped.len(), 0);
        // The drag is over: a path-loss signal after it is a new episode only
        // once the coalescing second has passed, but a new drag resets it.
        cx.simulate_event(FileDropEvent::Entered {
            position: inside(),
            paths: external(&["C:\\drop\\b.txt"]),
        });
        tx.try_send(NON_UNICODE).expect("open channel");
        cx.run_until_parked();
        assert!(shell.read_with(cx, |s, _| s.notice()).is_some());
    }

    fn drop_at(cx: &mut gpui::VisualTestContext, at: gpui::Point<gpui::Pixels>) {
        cx.simulate_event(FileDropEvent::Entered {
            position: at,
            paths: external(&["C:\\drop\\a.txt"]),
        });
        cx.simulate_event(FileDropEvent::Pending { position: at });
        cx.simulate_event(FileDropEvent::Submit { position: at });
    }

    #[gpui::test]
    fn an_unoccluded_drop_is_delivered_exactly_once(cx: &mut TestAppContext) {
        let (states, build) = test_support::probes();
        let (_shell, cx) = open(cx, build);
        states[0].borrow_mut().target = enabled();
        drop_at(cx, inside());
        assert_eq!(states[0].borrow().dropped.len(), 1);
        drop_at(cx, inside());
        assert_eq!(states[0].borrow().dropped.len(), 2);
    }

    /// A notification occludes the Shell where it sits; a drop there is
    /// still delivered, once.
    #[gpui::test]
    fn a_drop_over_a_notification_is_delivered_exactly_once(cx: &mut TestAppContext) {
        use gpui_component::notification::Notification;
        let (states, build) = test_support::probes();
        let (_shell, cx) = open(cx, build);
        states[0].borrow_mut().target = enabled();
        cx.update(|window, cx| {
            window.push_notification(Notification::new().message("A toast"), cx);
        });
        cx.run_until_parked();
        let size = cx.update(|window, _| window.viewport_size());
        assert_eq!(
            (size.width, size.height),
            (gpui::px(1920.0), gpui::px(1080.0)),
            "the toast points below assume the default test window"
        );
        // The toast blocks the Shell's hitbox at x 1525..1900, y 0..5.
        for (n, (x, y)) in [(1525.0, 5.0), (1600.0, 3.0), (1700.0, 3.0)]
            .into_iter()
            .enumerate()
        {
            drop_at(cx, gpui::point(gpui::px(x), gpui::px(y)));
            assert_eq!(
                states[0].borrow().dropped.len(),
                n + 1,
                "drop at ({x}, {y})"
            );
        }
    }
}
