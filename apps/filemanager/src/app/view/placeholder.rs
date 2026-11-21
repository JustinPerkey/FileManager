//! A deliberately plain stand-in for the chrome: one button per tool and the
//! current tool's view. No styling beyond defaults.

use gpui::{
    div, AnyView, App, AppContext as _, ClickEvent, Context, Entity, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Window,
};
use gpui_component::button::Button;
use gpui_component::v_flex;

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
            div().debug_selector(move || selector.clone()).child(
                Button::new(id.id()).label(slot.label).on_click({
                    let shell = self.shell.clone();
                    move |_: &ClickEvent, window, cx| {
                        shell.update(cx, |shell, cx| shell.select(id, window, cx));
                    }
                }),
            )
        });
        v_flex()
            .children(nav)
            .child(self.shell.read(cx).tool_view(current))
    }
}
