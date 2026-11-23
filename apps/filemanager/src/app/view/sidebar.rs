//! The tool sidebar: one nav item per tool, the current one marked by fill,
//! border, weight and a dot, so the state never rests on one cue alone.
//!
//! Only the current item is a tab stop (Tab reaches it, then the tool view);
//! Up and Down move focus between items and wrap, Home and End jump, and
//! Enter or Space (the button's own activation) select.

use gpui::prelude::*;
use gpui::{
    div, px, relative, transparent_black, App, Context, Entity, FocusHandle, FontWeight,
    SharedString, Subscription, Window,
};
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
use gpui_component::v_flex;

use crate::app::probe::Tag as _;
use crate::app::shell::Shell;
use crate::theme::tokens::{FONT_SIZE_MD, RADIUS, REM, SPACE_1, SPACE_2, SPACE_3};
use crate::theme::{palette, Palette};
use crate::tools::ToolId;

gpui::actions!(fm_nav, [Previous, Next, First, Last]);

/// The key context of the nav; the arrows are bound here.
const CONTEXT: &str = "ToolNav";
/// 12rem.
const WIDTH: f32 = 192.0;
/// The share of the window the sidebar never exceeds.
const MAX_SHARE: f32 = 0.4;
/// Vertical padding of the sidebar.
const PAD_Y: f32 = SPACE_3;
/// The dot before the current item's label.
const DOT: f32 = 6.0;
/// Where the dot sits so it centres on the first text line: half of the
/// 1.4 line height of 14 px text, less half the dot.
const DOT_TOP: f32 = FONT_SIZE_MD * REM * 1.4 / 2.0 - DOT / 2.0;

#[derive(Default)]
struct Bound;

impl gpui::Global for Bound {}

/// Binds the nav's keys once per app.
fn ensure_bindings(cx: &mut App) {
    if cx.has_global::<Bound>() {
        return;
    }
    cx.set_global(Bound);
    cx.bind_keys([
        gpui::KeyBinding::new("up", Previous, Some(CONTEXT)),
        gpui::KeyBinding::new("down", Next, Some(CONTEXT)),
        gpui::KeyBinding::new("home", First, Some(CONTEXT)),
        gpui::KeyBinding::new("end", Last, Some(CONTEXT)),
    ]);
}

/// Where a nav key sends focus, from the focused item `from` of `len`.
#[must_use]
pub(crate) fn target(key: Key, from: Option<usize>, len: usize) -> Option<usize> {
    let last = len.checked_sub(1)?;
    Some(match key {
        Key::First => 0,
        Key::Last => last,
        Key::Next => from.map_or(0, |i| if i >= last { 0 } else { i + 1 }),
        Key::Previous => from.map_or(last, |i| if i == 0 { last } else { i - 1 }),
    })
}

/// A nav key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Key {
    Previous,
    Next,
    First,
    Last,
}

/// The sidebar view.
pub struct Sidebar {
    shell: Entity<Shell>,
    handles: Vec<FocusHandle>,
    _shell: Subscription,
}

impl Sidebar {
    pub fn new(shell: Entity<Shell>, cx: &mut Context<Self>) -> Self {
        ensure_bindings(cx);
        let handles = shell
            .read(cx)
            .tools()
            .iter()
            .map(|_| cx.focus_handle())
            .collect();
        let subscription = cx.observe(&shell, |_, _, cx| cx.notify());
        Self {
            shell,
            handles,
            _shell: subscription,
        }
    }

    fn go(&mut self, key: Key, window: &mut Window, cx: &mut Context<Self>) {
        let from = self.handles.iter().position(|h| h.is_focused(window));
        if let Some(to) = target(key, from, self.handles.len()) {
            self.handles[to].focus(window, cx);
        }
    }
}

/// One nav item.
fn item(
    id: ToolId,
    label: &'static str,
    is_current: bool,
    handle: &FocusHandle,
    shell: &Entity<Shell>,
    p: &Palette,
    cx: &App,
) -> impl IntoElement {
    let selector: SharedString = format!("tool-nav-{}", id.id()).into();
    let variant = ButtonCustomVariant::new(cx)
        .color(transparent_black())
        .foreground(p.text)
        .hover(p.surface)
        .active(p.surface);
    let text = div()
        .flex_1()
        .min_w_0()
        .text_left()
        .text_size(px(FONT_SIZE_MD * REM))
        .line_height(relative(1.4))
        .whitespace_normal()
        .font_weight(if is_current {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        })
        .child(label);
    let dot = is_current.then(|| {
        div()
            .debug_selector({
                let name = format!("tool-nav-dot-{}", id.id());
                move || name
            })
            .flex_none()
            .size(px(DOT))
            .mt(px(DOT_TOP))
            .mr(px(SPACE_2))
            .rounded_full()
            .bg(p.accent)
    });
    let shell = shell.clone();
    Button::new(selector.clone())
        .custom(variant)
        .accessibility_label(label)
        .toggled(is_current)
        .track_focus(handle)
        .tab_stop(is_current)
        .w_full()
        .h_auto()
        .px(px(SPACE_3))
        .py(px(SPACE_2))
        .border_1()
        .border_color(if is_current {
            p.border
        } else {
            transparent_black()
        })
        .bg(if is_current {
            p.surface
        } else {
            transparent_black()
        })
        .text_color(p.text)
        .rounded(px(RADIUS))
        .child(
            div()
                .flex()
                .items_start()
                .w_full()
                .children(dot)
                .child(text),
        )
        .on_click(move |_, window, cx| {
            shell.update(cx, |shell, cx| shell.select(id, window, cx));
        })
        .debug_selector({
            let name = selector.clone();
            move || name.to_string()
        })
        .tag(selector)
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *palette(cx);
        let shell = self.shell.read(cx);
        let current = shell.current();
        let items: Vec<_> = shell
            .tools()
            .iter()
            .zip(&self.handles)
            .map(|(slot, handle)| {
                item(
                    slot.id,
                    slot.label,
                    slot.id == current,
                    handle,
                    &self.shell,
                    &p,
                    cx,
                )
            })
            .collect();

        div()
            .id("sidebar")
            .flex_none()
            .w(px(WIDTH))
            .max_w(relative(MAX_SHARE))
            .h_full()
            .overflow_y_scroll()
            .bg(p.surface_sunken)
            .border_r_1()
            .border_color(p.border)
            .px(px(SPACE_2))
            .py(px(PAD_Y))
            .child(
                v_flex()
                    .key_context(CONTEXT)
                    .on_action(cx.listener(|this, _: &Previous, window, cx| {
                        this.go(Key::Previous, window, cx);
                    }))
                    .on_action(cx.listener(|this, _: &Next, window, cx| {
                        this.go(Key::Next, window, cx);
                    }))
                    .on_action(cx.listener(|this, _: &First, window, cx| {
                        this.go(Key::First, window, cx);
                    }))
                    .on_action(cx.listener(|this, _: &Last, window, cx| {
                        this.go(Key::Last, window, cx);
                    }))
                    .gap(px(SPACE_1))
                    .children(items),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_wrap_and_jump() {
        assert_eq!(target(Key::Next, Some(2), 3), Some(0));
        assert_eq!(target(Key::Next, Some(0), 3), Some(1));
        assert_eq!(target(Key::Previous, Some(0), 3), Some(2));
        assert_eq!(target(Key::Previous, Some(2), 3), Some(1));
        assert_eq!(target(Key::First, Some(2), 3), Some(0));
        assert_eq!(target(Key::Last, Some(0), 3), Some(2));
        assert_eq!(target(Key::Next, None, 3), Some(0));
        assert_eq!(target(Key::Previous, None, 3), Some(2));
        assert_eq!(target(Key::Next, None, 0), None);
    }
}
