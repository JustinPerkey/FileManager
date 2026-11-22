//! A deliberately plain stand-in for the chrome: one button per tool, the
//! current tool's view, and the drag overlay, notice and close request as
//! plain text. No styling beyond defaults.

use gpui::{
    div, AnyView, App, AppContext as _, ClickEvent, Context, Entity, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Window,
};
use gpui_component::button::Button;
use gpui_component::v_flex;

use crate::app::drop::DropTarget;
use crate::app::probe::Tag as _;
use crate::app::shell::Shell;

pub fn chrome(shell: Entity<Shell>, _window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_| Placeholder { shell }).into()
}

struct Placeholder {
    shell: Entity<Shell>,
}

impl Render for Placeholder {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let shell = self.shell.read(cx);
        let current = shell.current();
        let nav = shell.tools().iter().map(|slot| {
            let id = slot.id;
            let selector = format!("tool-nav-{}", id.id());
            div()
                .debug_selector({
                    let selector = selector.clone();
                    move || selector
                })
                .child(Button::new(id.id()).label(slot.label).on_click({
                    let shell = self.shell.clone();
                    move |_: &ClickEvent, window, cx| {
                        shell.update(cx, |shell, cx| shell.select(id, window, cx));
                    }
                }))
                .tag(selector)
        });
        let overlay = match shell.drag_over() {
            Some(DropTarget::Enabled { label }) => Some(label.clone()),
            Some(DropTarget::Disabled { reason }) => Some(reason.clone()),
            Some(DropTarget::None) | None => None,
        };
        let notice = shell.notice().map(|notice| {
            div()
                .child(notice.message())
                .child(Button::new("dismiss-notice").label("Dismiss").on_click({
                    let shell = self.shell.clone();
                    move |_: &ClickEvent, _window, cx| {
                        shell.update(cx, Shell::dismiss_notice);
                    }
                }))
                .tag("shell-notice")
        });
        let close = shell.close_request().map(|request| {
            let items = request
                .items
                .iter()
                .map(|(_, item)| div().child(item.clone()));
            v_flex()
                .child("Close with unsaved work?")
                .children(items)
                .child(div().debug_selector(|| "keep-editing".to_string()).child(
                    Button::new("keep-editing").label("Keep editing").on_click({
                        let shell = self.shell.clone();
                        move |_: &ClickEvent, _window, cx| {
                            shell.update(cx, Shell::cancel_close);
                        }
                    }),
                ))
                .child(div().debug_selector(|| "close-anyway".to_string()).child(
                    Button::new("close-anyway").label("Close").on_click({
                        let shell = self.shell.clone();
                        move |_: &ClickEvent, window, cx| {
                            shell.update(cx, |shell, cx| shell.confirm_close(window, cx));
                        }
                    }),
                ))
        });
        v_flex()
            .children(nav)
            .children(overlay.map(|text| div().child(text)))
            .children(notice)
            .children(close)
            .child(self.shell.read(cx).tool_view(current))
    }
}
