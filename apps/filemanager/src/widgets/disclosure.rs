//! A disclosure: a quiet button with a chevron that shows or hides content.

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{px, AnyElement, App, ElementId, SharedString, Window};
use gpui_component::v_flex;

use super::button::{self, button};
use super::icon::IconName;
use crate::theme::tokens::SPACE_1;

type ToggleHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// A disclosure. The view owns `open`: the toggle asks for the opposite
/// through [`Disclosure::on_toggle`] and the view re-renders with it.
#[derive(IntoElement)]
pub struct Disclosure {
    id: ElementId,
    summary: SharedString,
    open: bool,
    on_toggle: Option<ToggleHandler>,
    debug_selector: Option<SharedString>,
    content: Vec<AnyElement>,
}

impl Disclosure {
    /// A closed disclosure. `summary` is the text of the toggle.
    #[must_use]
    pub fn new(id: impl Into<ElementId>, summary: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            summary: summary.into(),
            open: false,
            on_toggle: None,
            debug_selector: None,
            content: Vec::new(),
        }
    }

    /// Whether the content shows. The chevron points up while it does.
    #[must_use]
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Called with the state the toggle asks for, on a click, Enter or Space.
    #[must_use]
    pub fn on_toggle(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }
}

impl Disclosure {
    /// Names the toggle for `VisualTestContext::debug_bounds` (debug builds
    /// only).
    #[must_use]
    pub fn debug_selector(mut self, name: impl Into<SharedString>) -> Self {
        self.debug_selector = Some(name.into());
        self
    }
}

impl ParentElement for Disclosure {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.content.extend(elements);
    }
}

impl RenderOnce for Disclosure {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let open = self.open;
        let toggle = button(self.id, self.summary)
            .quiet()
            .icon(IconName::ChevronDown)
            .icon_flip(open)
            .when_some(self.debug_selector, button::FmButton::debug_selector)
            .when_some(self.on_toggle, |b, handler| {
                b.on_click(move |_, window, cx| handler(!open, window, cx))
            });
        v_flex()
            .items_start()
            .gap(px(SPACE_1))
            .child(toggle)
            .when(open, |col| col.children(self.content))
    }
}
