//! The visual shell: the tool sidebar and tool area side by side, with the
//! notice, the drop overlay and the close prompt on top. [`chrome`] builds it
//! for a [`Shell`], which stays the only owner of the state they show.

mod close_prompt;
mod drop_overlay;
mod notice;
mod sidebar;
#[cfg(test)]
mod tests;

use gpui::prelude::*;
use gpui::{AnyView, App, Context, Entity, Subscription, Window};
use gpui_component::{h_flex, v_flex};

use crate::app::shell::Shell;
use crate::theme::palette;

/// Builds the chrome view for `shell`.
pub fn chrome(shell: Entity<Shell>, window: &mut Window, cx: &mut App) -> AnyView {
    crate::theme::init(window, cx);
    let sidebar = cx.new(|cx| sidebar::Sidebar::new(shell.clone(), cx));
    cx.new(|cx| Chrome::new(shell, sidebar, window, cx)).into()
}

struct Chrome {
    shell: Entity<Shell>,
    sidebar: Entity<sidebar::Sidebar>,
    _shell: Subscription,
}

impl Chrome {
    fn new(
        shell: Entity<Shell>,
        sidebar: Entity<sidebar::Sidebar>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.observe_in(&shell, window, |_, shell, window, cx| {
            close_prompt::sync(&shell, window, cx);
            cx.notify();
        });
        Self {
            shell,
            sidebar,
            _shell: subscription,
        }
    }
}

impl Render for Chrome {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *palette(cx);
        let shell = self.shell.read(cx);
        let notice = shell.notice().map(|n| notice::notice(n, &self.shell));
        let overlay = shell
            .drag_over()
            .and_then(|target| drop_overlay::drop_overlay(target, &p));
        let area = v_flex().flex_1().min_w_0().h_full().children(notice).child(
            v_flex()
                .flex_1()
                .min_h_0()
                .child(shell.tool_view(shell.current())),
        );
        h_flex()
            .relative()
            .size_full()
            .bg(p.bg)
            .text_color(p.text)
            .child(self.sidebar.clone())
            .child(area)
            .children(overlay)
    }
}
