//! The select: a secondary-button-looking trigger that opens a list, over
//! gpui-component's `Select`.
//!
//! gpui-component's select keeps its state in an entity; this widget keeps
//! that entity in the window's keyed state (by `id`), so a view just describes
//! the select each frame: its options, the selected value, and what to do on a
//! pick. Values are strings: the id of the option, not its title.
//!
//! Keyboard (from gpui-component): Enter, Space or Down opens, Up and Down
//! move, Enter picks, Escape closes.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{div, px, App, ElementId, Entity, SharedString, Subscription, Window};
use gpui_component::searchable_list::SearchableListItem;
use gpui_component::select::{Select, SelectEvent, SelectState};
use gpui_component::IndexPath;

use super::button::tooltip_view;
use super::icon::{icon, IconName};
use crate::theme::tokens::{RADIUS, SPACE_1, SPACE_2};
use crate::theme::{palette, DISABLED_OPACITY};

/// The smallest a select gets.
const MIN_HEIGHT: f32 = 24.0;

/// One option: what the list shows and the value a pick reports.
#[derive(Clone, Debug, PartialEq)]
struct Opt {
    title: SharedString,
    value: SharedString,
}

impl SearchableListItem for Opt {
    type Value = SharedString;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &SharedString {
        &self.value
    }
}

type PickHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// What lives between frames: gpui-component's state, the latest handler, and
/// what the view last asked to have selected.
struct Held {
    state: Entity<SelectState<Vec<Opt>>>,
    on_pick: Rc<RefCell<Option<PickHandler>>>,
    options: Vec<Opt>,
    applied: Option<SharedString>,
    _pick: Subscription,
}

/// A select. Build it with [`select`].
#[derive(IntoElement)]
pub struct FmSelect {
    id: ElementId,
    label: SharedString,
    options: Vec<Opt>,
    selected: Option<SharedString>,
    disabled: bool,
    width: Option<gpui::Pixels>,
    on_pick: Option<PickHandler>,
}

/// A select named `label`. The label is only the placeholder (the text shown
/// while nothing is selected), the tooltip and the accessible name. A keyboard
/// user never sees a tooltip, so every caller must also render a visible label
/// beside the select, as a form row does. It offers `options`, each `(title, value)`.
/// `id` must be unique among the selects of one view.
#[must_use]
pub fn select(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    options: Vec<(SharedString, SharedString)>,
    selected: Option<SharedString>,
) -> FmSelect {
    FmSelect {
        id: id.into(),
        label: label.into(),
        options: options
            .into_iter()
            .map(|(title, value)| Opt { title, value })
            .collect(),
        selected,
        disabled: false,
        width: None,
        on_pick: None,
    }
}

impl FmSelect {
    /// Disabled: muted on `surface_sunken`, and it does not open.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// A fixed width; by default the select takes its parent's.
    #[must_use]
    pub fn width(mut self, width: gpui::Pixels) -> Self {
        self.width = Some(width);
        self
    }

    /// Called with the value of the option the user picked.
    #[must_use]
    pub fn on_pick(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_pick = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for FmSelect {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = *palette(cx);
        let options = self.options;
        let initial = self
            .selected
            .as_ref()
            .and_then(|v| options.iter().position(|o| &o.value == v));

        let held = window.use_keyed_state((self.id.clone(), "state"), cx, {
            let options = options.clone();
            let selected = self.selected.clone();
            move |window, cx| {
                let state = cx.new(|cx| {
                    SelectState::new(
                        options.clone(),
                        initial.map(|row| IndexPath::default().row(row)),
                        window,
                        cx,
                    )
                });
                let on_pick: Rc<RefCell<Option<PickHandler>>> = Rc::default();
                let pick = cx.subscribe_in(&state, window, {
                    let on_pick = on_pick.clone();
                    move |held: &mut Held, _state, event: &SelectEvent<Vec<Opt>>, window, cx| {
                        let SelectEvent::Confirm(Some(value)) = event else {
                            return;
                        };
                        held.applied = Some(value.clone());
                        let handler = on_pick.borrow().clone();
                        if let Some(handler) = handler {
                            handler(value, window, cx);
                        }
                    }
                });
                Held {
                    state,
                    on_pick,
                    options,
                    applied: selected,
                    _pick: pick,
                }
            }
        });

        // Bring the state in line with what the view describes this frame.
        let state = held.update(cx, |held, cx| {
            held.on_pick.borrow_mut().clone_from(&self.on_pick);
            if held.options != options {
                let state = held.state.clone();
                let items = options.clone();
                state.update(cx, |s, cx| s.set_items(items, window, cx));
                held.options.clone_from(&options);
            }
            if held.applied != self.selected {
                let value = self.selected.clone();
                held.state.update(cx, |s, cx| match &value {
                    Some(v) => s.set_selected_value(v, window, cx),
                    None => s.set_selected_index(None, window, cx),
                });
                held.applied = value;
            }
            held.state.clone()
        });

        let (fill, text, opacity) = if self.disabled {
            (p.surface_sunken, p.text_muted, DISABLED_OPACITY)
        } else {
            (p.surface, p.text, 1.0)
        };
        let chevron = icon(IconName::ChevronDown).text_color(p.text_muted);
        let label = self.label;
        div()
            .id((self.id, "tip"))
            .min_w_0()
            .when_some(self.width, Styled::w)
            .tooltip(tooltip_view(label.clone()))
            .child(
                Select::new(&state)
                    .placeholder(label.clone())
                    .accessibility_label(label)
                    .icon(chevron)
                    .disabled(self.disabled)
                    .bg(fill)
                    .text_color(text)
                    .border_color(p.border)
                    .rounded(px(RADIUS))
                    .h_auto()
                    .min_h(px(MIN_HEIGHT))
                    .px(px(SPACE_2))
                    .py(px(SPACE_1))
                    .opacity(opacity),
            )
    }
}
