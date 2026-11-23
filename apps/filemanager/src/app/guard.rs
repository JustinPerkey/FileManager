//! The close guard: the window does not close while a tool has unsaved work.
//! Only the tools know what is unsaved; the shell asks them all.

use gpui::{App, SharedString};

use crate::tools::{ToolId, ToolSlot};

/// What is holding the window open, one item per unsaved piece of work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloseRequest {
    pub items: Vec<(ToolId, SharedString)>,
}

/// Asks every tool for its unsaved work, in registry order.
pub(crate) fn collect(tools: &[ToolSlot], cx: &App) -> Vec<(ToolId, SharedString)> {
    tools
        .iter()
        .flat_map(|slot| {
            slot.hooks
                .unsaved_work(cx)
                .into_iter()
                .map(|item| (slot.id, item))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use gpui::{Modifiers, TestAppContext};

    use crate::app::shell::test_support::{open, probes};
    use crate::app::shell::Shell;
    use crate::tools::ToolId;

    #[gpui::test]
    fn close_without_unsaved_work_closes(cx: &mut TestAppContext) {
        let (_states, build) = probes();
        let (shell, cx) = open(cx, build);
        assert!(cx.simulate_close());
        assert!(shell.read_with(cx, |s, _| s.close_request().is_none()));
    }

    #[gpui::test]
    fn close_with_unsaved_work_is_held_and_requested(cx: &mut TestAppContext) {
        let (states, build) = probes();
        let (shell, cx) = open(cx, build);
        states[1].borrow_mut().unsaved = vec!["Draft of orders".into()];
        assert!(!cx.simulate_close(), "held");
        let request = shell.read_with(cx, |s, _| s.close_request().cloned());
        assert_eq!(
            request.map(|r| r.items),
            Some(vec![(ToolId::Tarpack, "Draft of orders".into())])
        );
    }

    #[gpui::test]
    fn cancel_keeps_window_open(cx: &mut TestAppContext) {
        let (states, build) = probes();
        let (shell, cx) = open(cx, build);
        states[0].borrow_mut().unsaved = vec!["x".into()];
        assert!(!cx.simulate_close());
        cx.run_until_parked();
        let at = cx
            .debug_bounds("dialog-cancel")
            .expect("the prompt shows Keep editing")
            .center();
        cx.simulate_click(at, Modifiers::none());
        assert!(shell.read_with(cx, |s, _| s.close_request().is_none()));
        assert_eq!(cx.windows().len(), 1);
    }

    #[gpui::test]
    fn confirm_closes_window(cx: &mut TestAppContext) {
        let (states, build) = probes();
        let (shell, cx) = open(cx, build);
        states[0].borrow_mut().unsaved = vec!["x".into()];
        assert!(!cx.simulate_close());
        cx.update(|window, cx| shell.update(cx, |s, cx| s.confirm_close(window, cx)));
        cx.run_until_parked();
        assert_eq!(cx.windows().len(), 0);
    }

    #[gpui::test]
    fn request_close_matches_the_hook(cx: &mut TestAppContext) {
        // Clean: closes at once.
        let (_states, build) = probes();
        let (shell, vcx) = open(cx, build);
        vcx.update(|window, cx| shell.update(cx, |s, cx| s.request_close(window, cx)));
        vcx.run_until_parked();
        assert_eq!(vcx.windows().len(), 0, "clean: closed, like the hook");
        // Dirty: held, with the same request the hook sets.
        let (states, build) = probes();
        let (shell, vcx) = open(cx, build);
        states[2].borrow_mut().unsaved = vec!["y".into()];
        vcx.update(|window, cx| shell.update(cx, |s, cx| s.request_close(window, cx)));
        vcx.run_until_parked();
        assert_eq!(vcx.windows().len(), 1, "dirty: held, like the hook");
        let via_request = shell.read_with(vcx, |s, _| s.close_request().cloned());
        shell.update(vcx, Shell::cancel_close);
        assert!(!vcx.simulate_close());
        let via_hook = shell.read_with(vcx, |s, _| s.close_request().cloned());
        assert_eq!(via_request, via_hook);
        assert!(via_hook.is_some());
    }

    #[gpui::test]
    fn second_close_while_requested_does_not_duplicate(cx: &mut TestAppContext) {
        let (states, build) = probes();
        let (shell, cx) = open(cx, build);
        states[0].borrow_mut().unsaved = vec!["x".into()];
        assert!(!cx.simulate_close());
        assert!(!cx.simulate_close());
        let request = shell.read_with(cx, |s, _| s.close_request().cloned());
        assert_eq!(request.expect("held").items.len(), 1);
    }
}
