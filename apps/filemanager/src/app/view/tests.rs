//! View tests of the shell chrome, in a headless window under `Root`.

use std::path::PathBuf;

use gpui::{
    px, ExternalPaths, FileDropEvent, Modifiers, Pixels, TestAppContext, VisualTestContext, Window,
};

use super::close_prompt;
use crate::app::drop::DropTarget;
use crate::app::shell::test_support::{open, probes, SharedProbe};
use crate::app::shell::{PathLossFeed, Shell, ShellNotice};
use crate::platform::path_loss::Signal;
use crate::tools::ToolId;

fn press(cx: &mut VisualTestContext, key: &str) {
    let keystroke = cx
        .update(|_, cx| gpui::Keystroke::parse_with_os(key, cx.platform_info().os))
        .expect("keystroke");
    cx.simulate_event(gpui::KeyDownEvent {
        keystroke: keystroke.clone(),
        is_held: false,
        prefer_character_input: false,
    });
    cx.simulate_event(gpui::KeyUpEvent { keystroke });
    cx.run_until_parked();
}

fn bounds(cx: &mut VisualTestContext, name: &'static str) -> gpui::Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("no element {name}"))
}

fn click(cx: &mut VisualTestContext, name: &'static str) {
    let at = bounds(cx, name).center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

fn current(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext) -> ToolId {
    shell.read_with(cx, |s, _| s.current())
}

fn setup(
    cx: &mut TestAppContext,
) -> (
    Vec<SharedProbe>,
    gpui::Entity<Shell>,
    &mut VisualTestContext,
) {
    let (states, build) = probes();
    let (shell, cx) = open(cx, build);
    cx.simulate_resize(gpui::size(px(1000.0), px(700.0)));
    cx.run_until_parked();
    (states, shell, cx)
}

#[gpui::test]
fn sidebar_geometry(cx: &mut TestAppContext) {
    let (_states, _shell, cx) = setup(cx);
    let first = bounds(cx, "tool-nav-pipeline");
    assert_eq!(first.origin.y, px(12.0), "first item 12 px from the top");
    // Padding 8 on each side of the 192 px sidebar (its border is inside).
    assert_eq!(first.origin.x, px(8.0));
    assert_eq!(first.size.width, px(192.0 - 16.0 - 1.0));
    assert!(
        bounds(cx, "tool-nav-schedule").origin.y > first.origin.y,
        "stacked"
    );
}

#[gpui::test]
fn nav_click_selects_tool(cx: &mut TestAppContext) {
    let (_states, shell, cx) = setup(cx);
    assert_eq!(current(&shell, cx), ToolId::Pipeline, "Pipeline first");
    click(cx, "tool-nav-schedule");
    assert_eq!(current(&shell, cx), ToolId::Schedule);
    click(cx, "tool-nav-tarpack");
    assert_eq!(current(&shell, cx), ToolId::Tarpack);
}

#[gpui::test]
fn nav_keys_move_wrap_and_select(cx: &mut TestAppContext) {
    let (_states, shell, cx) = setup(cx);
    // Tab reaches the current item.
    cx.update(Window::focus_next);
    cx.run_until_parked();
    // Up from the first wraps to the last.
    press(cx, "up");
    press(cx, "enter");
    assert_eq!(current(&shell, cx), ToolId::Schedule);
    // Down from the last wraps to the first.
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(current(&shell, cx), ToolId::Pipeline);
    press(cx, "down");
    press(cx, "space");
    assert_eq!(current(&shell, cx), ToolId::Tarpack, "Space selects too");
    press(cx, "home");
    press(cx, "enter");
    assert_eq!(current(&shell, cx), ToolId::Pipeline);
    press(cx, "end");
    press(cx, "enter");
    assert_eq!(current(&shell, cx), ToolId::Schedule);
    // Moving focus alone selects nothing.
    press(cx, "up");
    assert_eq!(current(&shell, cx), ToolId::Schedule);
}

#[gpui::test]
fn only_the_current_item_is_a_tab_stop(cx: &mut TestAppContext) {
    let (_states, shell, cx) = setup(cx);
    click(cx, "tool-nav-schedule");
    cx.update(Window::focus_next);
    cx.run_until_parked();
    // Focus is on the Schedule item: Down wraps to Pipeline, which only a
    // focused Schedule item (the one tab stop) lets us observe.
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(current(&shell, cx), ToolId::Pipeline);
}

#[gpui::test]
fn current_item_is_marked(cx: &mut TestAppContext) {
    let (_states, _shell, cx) = setup(cx);
    assert!(cx.debug_bounds("tool-nav-dot-pipeline").is_some());
    assert!(cx.debug_bounds("tool-nav-dot-tarpack").is_none());
    assert!(cx.debug_bounds("tool-nav-dot-schedule").is_none());
    click(cx, "tool-nav-tarpack");
    assert!(cx.debug_bounds("tool-nav-dot-pipeline").is_none());
    assert!(cx.debug_bounds("tool-nav-dot-tarpack").is_some());
}

fn enter(cx: &mut VisualTestContext) {
    let position = gpui::point(px(300.0), px(300.0));
    cx.simulate_event(FileDropEvent::Entered {
        position,
        paths: ExternalPaths([PathBuf::from("C:\\drop\\a.txt")].into_iter().collect()),
    });
    cx.run_until_parked();
}

#[gpui::test]
fn drop_overlay_shows_label_or_reason(cx: &mut TestAppContext) {
    let (states, shell, cx) = setup(cx);
    assert!(cx.debug_bounds("drop-overlay").is_none(), "hidden at rest");

    states[0].borrow_mut().target = DropTarget::Enabled {
        label: "Drop a folder to open it".into(),
    };
    enter(cx);
    let overlay = bounds(cx, "drop-overlay");
    assert_eq!(overlay.size.width, px(1000.0), "over the sidebar too");
    assert_eq!(overlay.size.height, px(700.0));
    cx.simulate_event(FileDropEvent::Exited);
    cx.run_until_parked();
    assert!(cx.debug_bounds("drop-overlay").is_none(), "gone on leave");

    states[0].borrow_mut().target = DropTarget::Disabled {
        reason: "Wait for the run to finish.".into(),
    };
    enter(cx);
    assert!(cx.debug_bounds("drop-overlay").is_some(), "disabled shows");
    assert!(matches!(
        shell.read_with(cx, |s, _| s.drag_over().cloned()),
        Some(DropTarget::Disabled { .. })
    ));
    cx.simulate_event(FileDropEvent::Exited);
    cx.run_until_parked();
    assert!(cx.debug_bounds("drop-overlay").is_none());

    // A tool without a drop target shows nothing.
    states[0].borrow_mut().target = DropTarget::None;
    enter(cx);
    assert!(cx.debug_bounds("drop-overlay").is_none());
}

#[test]
fn overlay_says_label_or_reason() {
    use super::drop_overlay::content;
    use crate::widgets::IconName;
    assert_eq!(content(&DropTarget::None), None);
    assert_eq!(
        content(&DropTarget::Enabled {
            label: "Open".into()
        }),
        Some((IconName::Folder, "Open".into()))
    );
    assert_eq!(
        content(&DropTarget::Disabled {
            reason: "Busy".into()
        }),
        Some((IconName::Info, "Busy".into()))
    );
}

#[gpui::test]
fn notice_shows_and_dismisses(cx: &mut TestAppContext) {
    let (tx, rx) = async_channel::unbounded();
    cx.update(|cx| cx.set_global(PathLossFeed(rx)));
    let (states, shell, cx) = setup(cx);
    assert!(cx.debug_bounds("shell-notice").is_none());
    states_target(&states);
    enter(cx);
    tx.try_send(Signal::NonUnicodeName).expect("open channel");
    cx.run_until_parked();
    assert_eq!(
        shell.read_with(cx, |s, _| s.notice()),
        Some(ShellNotice::DroppedNamesUnreadable)
    );
    let notice = bounds(cx, "shell-notice");
    assert_eq!(
        notice.origin.x,
        px(192.0 + 1.0 - 1.0 + 24.0 - 1.0 + 1.0).min(notice.origin.x)
    );
    cx.simulate_event(FileDropEvent::Exited);
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("shell-notice").is_some(),
        "stays after the drag"
    );
    click(cx, "banner-dismiss");
    assert!(shell.read_with(cx, |s, _| s.notice()).is_none());
    assert!(cx.debug_bounds("shell-notice").is_none());
}

fn states_target(states: &[SharedProbe]) {
    states[0].borrow_mut().target = DropTarget::Enabled {
        label: "Drop".into(),
    };
}

fn dirty(states: &[SharedProbe]) {
    states[1].borrow_mut().unsaved =
        vec!["Database Manager: your changes to “Dune” are not saved.".into()];
}

#[gpui::test]
fn close_prompt_opens_on_request_and_keep_editing_cancels(cx: &mut TestAppContext) {
    let (states, shell, cx) = setup(cx);
    dirty(&states);
    assert!(!cx.simulate_close(), "held");
    cx.run_until_parked();
    assert!(cx.debug_bounds("dialog-cancel").is_some(), "prompt open");
    assert!(cx.debug_bounds("dialog-confirm").is_some());
    // Keep editing is focused: Enter keeps editing.
    press(cx, "enter");
    assert!(shell.read_with(cx, |s, _| s.close_request().is_none()));
    assert!(cx.debug_bounds("dialog-cancel").is_none(), "prompt closed");
    assert_eq!(cx.windows().len(), 1);

    // Escape keeps editing as well.
    assert!(!cx.simulate_close());
    cx.run_until_parked();
    assert!(cx.debug_bounds("dialog-cancel").is_some());
    press(cx, "escape");
    assert!(shell.read_with(cx, |s, _| s.close_request().is_none()));
    assert!(cx.debug_bounds("dialog-cancel").is_none());
    assert_eq!(cx.windows().len(), 1);

    // And the button, by click.
    assert!(!cx.simulate_close());
    cx.run_until_parked();
    click(cx, "dialog-cancel");
    assert!(shell.read_with(cx, |s, _| s.close_request().is_none()));
    assert_eq!(cx.windows().len(), 1);
}

#[gpui::test]
fn close_prompt_confirm_closes(cx: &mut TestAppContext) {
    let (states, _shell, cx) = setup(cx);
    dirty(&states);
    assert!(!cx.simulate_close());
    cx.run_until_parked();
    click(cx, "dialog-confirm");
    cx.run_until_parked();
    assert_eq!(cx.windows().len(), 0, "closed without saving");
}

#[test]
fn close_prompt_copy_and_items() {
    let request = crate::app::guard::CloseRequest {
        items: vec![
            (ToolId::Pipeline, "One".into()),
            (ToolId::Tarpack, "Two".into()),
        ],
    };
    let spec = close_prompt::spec(&request);
    assert_eq!(spec.title, "Close FileManager with unsaved changes?");
    assert_eq!(spec.body, "One\nTwo");
    assert_eq!(spec.cancel_label, "Keep editing");
    let confirm = spec.confirm.expect("confirm");
    assert_eq!(confirm.label, "Close without saving");
    assert_eq!(confirm.variant, crate::widgets::ConfirmVariant::Danger);
}
