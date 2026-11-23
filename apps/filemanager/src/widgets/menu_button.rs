//! The menu button: a secondary button with a trailing chevron that opens a
//! list of choices under it (the Recent files list).
//!
//! The list is a gpui-component `Popover` around a list of our own, so Home and
//! End jump, Tab closes, disabled rows are skipped and a row can carry a detail
//! line. The popover owns the open state, Escape, the click outside and the
//! focus return to the trigger; this module owns the keys inside the list and
//! the look. The shadow is gpui-component's popover shadow.

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{
    div, px, transparent_black, AnyElement, App, ElementId, Role, ScrollHandle, SharedString,
    Window,
};
use gpui_component::popover::{Popover, PopoverState};
use gpui_component::{v_flex, ThemeStyled as _};

use super::button::{button, tooltip_view};
use super::icon::IconName;
use super::text::{small, small_muted};
use crate::app::probe::Tag as _;
use crate::theme::tokens::{RADIUS, SPACE_1, SPACE_3};
use crate::theme::{palette, DISABLED_OPACITY};

gpui::actions!(fm_menu, [CloseMenu]);

/// The key context of the list; `tab` is bound to [`CloseMenu`] here, deeper
/// than the root's Tab binding, so Tab closes the list instead of moving on.
const CONTEXT: &str = "FmMenu";

#[derive(Default)]
struct Bound;

impl gpui::Global for Bound {}

/// Binds the list's keys once per app.
fn ensure_bindings(cx: &mut App) {
    if cx.has_global::<Bound>() {
        return;
    }
    cx.set_global(Bound);
    cx.bind_keys([
        gpui::KeyBinding::new("tab", CloseMenu, Some(CONTEXT)),
        gpui::KeyBinding::new("shift-tab", CloseMenu, Some(CONTEXT)),
    ]);
}

/// The narrowest the list gets.
const MIN_WIDTH: f32 = 256.0;
/// The widest the list gets.
const MAX_WIDTH: f32 = 512.0;
/// Share of the window height at which the list scrolls.
const MAX_HEIGHT_SHARE: f32 = 0.6;

/// One choice in the list.
#[derive(Clone, Debug, Default)]
pub struct MenuItem {
    pub label: SharedString,
    /// A second line under the label, muted (a folder, a date).
    pub detail: Option<SharedString>,
    pub tooltip: Option<SharedString>,
    /// Shown but skipped by the keys and ignoring clicks.
    pub disabled: bool,
}

type SelectHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// A menu button. Build it with [`MenuButton::new`].
#[derive(IntoElement)]
pub struct MenuButton {
    id: ElementId,
    label: SharedString,
    items: Vec<MenuItem>,
    on_select: Option<SelectHandler>,
    disabled: bool,
    shortcut: Option<SharedString>,
}

impl MenuButton {
    /// A menu button named `label`. `id` is also the tag and debug selector of
    /// the trigger; row `n` is `<id>-item-<n>`.
    #[must_use]
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            items: Vec::new(),
            on_select: None,
            disabled: false,
            shortcut: None,
        }
    }

    /// The choices. With none, the button is not rendered at all.
    #[must_use]
    pub fn items(mut self, items: Vec<MenuItem>) -> Self {
        self.items = items;
        self
    }

    /// Runs with the index of the chosen item, after the list has closed.
    #[must_use]
    pub fn on_select(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Disabled: the list cannot be opened.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Names a keyboard shortcut in the trigger's tooltip.
    #[must_use]
    pub fn shortcut(mut self, keys: impl Into<SharedString>) -> Self {
        self.shortcut = Some(keys.into());
        self
    }
}

/// The first row the keys can reach.
fn first_enabled(disabled: &[bool]) -> Option<usize> {
    disabled.iter().position(|d| !d)
}

/// The last row the keys can reach.
fn last_enabled(disabled: &[bool]) -> Option<usize> {
    disabled.iter().rposition(|d| !d)
}

/// The enabled row after (`forward`) or before `from`, wrapping round the
/// ends and skipping disabled rows. From no selection, forward is the first
/// enabled row and backward the last.
fn step(disabled: &[bool], from: Option<usize>, forward: bool) -> Option<usize> {
    let n = disabled.len();
    if n == 0 {
        return None;
    }
    let start = match (from, forward) {
        (Some(i), true) => i + 1,
        (Some(i), false) => i + n - 1,
        (None, true) => 0,
        (None, false) => n - 1,
    };
    (0..n)
        .map(|k| {
            if forward {
                (start + k) % n
            } else {
                (start + n - k) % n
            }
        })
        .find(|&i| !disabled[i])
}

/// What the list remembers between draws.
struct MenuState {
    open: bool,
    selected: Option<usize>,
    list: gpui::FocusHandle,
    scroll: ScrollHandle,
}

impl RenderOnce for MenuButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if self.items.is_empty() {
            return div().into_any_element();
        }
        ensure_bindings(cx);
        let id = self.id.clone();
        let state = window.use_keyed_state(
            SharedString::from(format!("{id}-menu-state")),
            cx,
            |_, cx| MenuState {
                open: false,
                selected: None,
                list: cx.focus_handle(),
                scroll: ScrollHandle::new(),
            },
        );
        let disabled: Rc<Vec<bool>> = Rc::new(self.items.iter().map(|i| i.disabled).collect());
        let (open, list, scroll) = {
            let s = state.read(cx);
            (s.open, s.list.clone(), s.scroll.clone())
        };
        let max_height = window.viewport_size().height * MAX_HEIGHT_SHARE;

        let mut trigger = button(id.clone(), self.label.clone())
            .trailing_icon(IconName::ChevronDown)
            .disabled(self.disabled);
        if let Some(keys) = self.shortcut {
            trigger = trigger.shortcut(keys);
        }
        let trigger = trigger.into_button(cx);

        let items = Rc::new(self.items);
        let on_select = self.on_select;
        let popover = Popover::new(SharedString::from(format!("{id}-popover")))
            .appearance(false)
            .open(open && !self.disabled)
            .track_focus(&list)
            .trigger(trigger)
            .on_open_change({
                let state = state.clone();
                let disabled = disabled.clone();
                move |open, _, cx| {
                    state.update(cx, |s, cx| {
                        s.open = *open;
                        if *open {
                            s.selected = first_enabled(&disabled);
                        }
                        cx.notify();
                    });
                }
            })
            .content({
                let ctx = Rc::new(Panel {
                    state: state.clone(),
                    disabled: disabled.clone(),
                    items: items.clone(),
                    on_select: on_select.clone(),
                    id: id.clone(),
                    label: self.label.clone(),
                    list: list.clone(),
                    scroll: scroll.clone(),
                    max_height,
                });
                move |_, _, cx| ctx.build(cx)
            });

        div()
            .on_key_down({
                let state = state.clone();
                let disabled = disabled.clone();
                let blocked = self.disabled;
                move |event, window, cx| {
                    if blocked || state.read(cx).open || event.keystroke.key != "down" {
                        return;
                    }
                    state.update(cx, |s, cx| {
                        s.open = true;
                        s.selected = first_enabled(&disabled);
                        cx.notify();
                    });
                    window.prevent_default();
                    cx.stop_propagation();
                }
            })
            .child(popover)
            .debug_selector({
                let name = id.to_string();
                move || name.clone()
            })
            .tag(id.to_string())
            .into_any_element()
    }
}

/// Everything the open list draws from.
struct Panel {
    state: gpui::Entity<MenuState>,
    disabled: Rc<Vec<bool>>,
    items: Rc<Vec<MenuItem>>,
    on_select: Option<SelectHandler>,
    id: ElementId,
    label: SharedString,
    list: gpui::FocusHandle,
    scroll: ScrollHandle,
    max_height: gpui::Pixels,
}

impl Panel {
    /// The rows, each wired to pick and to follow the pointer.
    fn rows(
        &self,
        popover: &gpui::Entity<PopoverState>,
        selected: Option<usize>,
        p: &crate::theme::Palette,
    ) -> Vec<AnyElement> {
        let Self {
            state,
            items,
            on_select,
            id,
            ..
        } = self;
        items
            .iter()
            .enumerate()
            .map(|(ix, item)| {
                let pick = {
                    let popover = popover.clone();
                    let on_select = on_select.clone();
                    move |window: &mut Window, cx: &mut App| {
                        popover.update(cx, |s, cx| s.dismiss(window, cx));
                        if let Some(on_select) = &on_select {
                            on_select(ix, window, cx);
                        }
                    }
                };
                let hover = {
                    let state = state.clone();
                    move |cx: &mut App| {
                        state.update(cx, |s, cx| {
                            if s.selected != Some(ix) {
                                s.selected = Some(ix);
                                cx.notify();
                            }
                        });
                    }
                };
                row(id, ix, item, selected == Some(ix), p, hover, pick)
            })
            .collect()
    }

    /// The list, drawn inside the popover.
    fn build(&self, cx: &mut gpui::Context<PopoverState>) -> AnyElement {
        let Self {
            state,
            disabled,
            on_select,
            items: _,
            id,
            label,
            list,
            scroll,
            max_height,
        } = self;
        let (max_height, on_select) = (*max_height, on_select.clone());
        let popover = cx.entity();
        let p = *palette(cx);
        let selected = state.read(cx).selected;
        let rows = self.rows(&popover, selected, &p);

        let key_state = state.clone();
        let key_disabled = disabled.clone();
        let key_popover = popover.clone();
        let key_select = on_select.clone();
        let move_to = move |cx: &mut App, to: Option<usize>| {
            key_state.update(cx, |s, cx| {
                if to.is_some() {
                    s.selected = to;
                }
                if let Some(ix) = to {
                    s.scroll.scroll_to_item(ix);
                }
                cx.notify();
            });
        };
        v_flex()
            .popover_style(cx)
            .bg(p.surface)
            .border_1()
            .border_color(p.border)
            .rounded(px(RADIUS))
            .p(px(SPACE_1))
            .min_w(px(MIN_WIDTH))
            .max_w(px(MAX_WIDTH))
            .id(SharedString::from(format!("{id}-menu")))
            .role(Role::Menu)
            .aria_label(label.clone())
            .child(
                v_flex()
                    .id(SharedString::from(format!("{id}-list")))
                    .track_focus(list)
                    .key_context(CONTEXT)
                    .on_action({
                        let popover = popover.clone();
                        move |_: &CloseMenu, window, cx| {
                            popover.update(cx, |s, cx| s.dismiss(window, cx));
                        }
                    })
                    .track_scroll(scroll)
                    .max_h(max_height)
                    .overflow_y_scroll()
                    .on_key_down({
                        let state = state.clone();
                        move |event, window, cx| {
                            let current = state.read(cx).selected;
                            let key = event.keystroke.key.as_str();
                            let to = match key {
                                "down" => Some(step(&key_disabled, current, true)),
                                "up" => Some(step(&key_disabled, current, false)),
                                "home" => Some(first_enabled(&key_disabled)),
                                "end" => Some(last_enabled(&key_disabled)),
                                _ => None,
                            };
                            if let Some(to) = to {
                                move_to(cx, to);
                            } else if matches!(key, "enter" | "space") {
                                if let Some(ix) = current {
                                    key_popover.update(cx, |s, cx| s.dismiss(window, cx));
                                    if let Some(select) = &key_select {
                                        select(ix, window, cx);
                                    }
                                }
                            } else {
                                return;
                            }
                            window.prevent_default();
                            cx.stop_propagation();
                        }
                    })
                    .children(rows),
            )
            .into_any_element()
    }
}

/// One row of the list.
fn row(
    id: &ElementId,
    ix: usize,
    item: &MenuItem,
    selected: bool,
    p: &crate::theme::Palette,
    hover_select: impl Fn(&mut App) + 'static,
    pick: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let name = SharedString::from(format!("{id}-item-{ix}"));
    let ring = if selected {
        p.focus_ring
    } else {
        transparent_black()
    };
    let el = v_flex()
        .id(name.clone())
        .role(Role::MenuItem)
        .aria_label(item.label.clone())
        .px(px(SPACE_3))
        .py(px(SPACE_1))
        .border_2()
        .border_color(ring)
        .rounded(px(RADIUS))
        .when(selected, |d| d.bg(p.surface_sunken))
        .child(small(item.label.clone()))
        .children(item.detail.clone().map(small_muted))
        .when_some(item.tooltip.clone(), |d, t| d.tooltip(tooltip_view(t)));
    let el = if item.disabled {
        el.opacity(DISABLED_OPACITY).cursor_default()
    } else {
        el.cursor_pointer()
            .on_mouse_move(move |_, _, cx| hover_select(cx))
            .on_click(move |_, window, cx| pick(window, cx))
    };
    let selector = name.to_string();
    el.debug_selector(move || selector.clone())
        .into_any_element()
}
