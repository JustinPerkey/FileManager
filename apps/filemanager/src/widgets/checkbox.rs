//! The checkbox, with the Database Manager's third state ("Not set").

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{div, px, App, ElementId, SharedString, Window};
use gpui_component::checkbox::Checkbox;
use gpui_component::{h_flex, Disableable as _};

use super::text::muted;
use crate::theme::tokens::SPACE_2;

/// A nullable boolean's three states.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckState {
    Unchecked,
    Checked,
    /// No value: the column is NULL, or the setting is absent.
    NotSet,
}

impl CheckState {
    /// The click and Space order: unchecked, checked, not set, unchecked.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Unchecked => Self::Checked,
            Self::Checked => Self::NotSet,
            Self::NotSet => Self::Unchecked,
        }
    }
}

type ChangeHandler = Rc<dyn Fn(CheckState, &mut Window, &mut App)>;

/// A labelled checkbox. Build it with [`checkbox`].
#[derive(IntoElement)]
pub struct FmCheckbox {
    id: ElementId,
    label: SharedString,
    state: CheckState,
    disabled: bool,
    on_change: Option<ChangeHandler>,
}

/// A checkbox with its visible label after the box. The view owns `state`: it
/// changes only when the view writes the value `on_change` hands it and
/// re-renders.
#[must_use]
pub fn checkbox(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    state: CheckState,
) -> FmCheckbox {
    FmCheckbox {
        id: id.into(),
        label: label.into(),
        state,
        disabled: false,
        on_change: None,
    }
}

impl FmCheckbox {
    /// Disabled: the box fades and ignores the pointer and the keyboard.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Called with the state a click or Space asks for ([`CheckState::next`]).
    #[must_use]
    pub fn on_change(
        mut self,
        handler: impl Fn(CheckState, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for FmCheckbox {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let state = self.state;
        let on_change = self.on_change;
        h_flex()
            .gap(px(SPACE_2))
            .items_center()
            .child(
                Checkbox::new(self.id)
                    .label(self.label)
                    .checked(state == CheckState::Checked)
                    .indeterminate(state == CheckState::NotSet)
                    .disabled(self.disabled)
                    .when_some(on_change, |c, handler| {
                        c.on_change(move |_, window, cx| handler(state.next(), window, cx))
                    }),
            )
            .when(state == CheckState::NotSet, |row| row.child(not_set()))
    }
}

/// The word that goes with the dash, so the state is not colour or shape alone.
fn not_set() -> impl IntoElement {
    div()
        .debug_selector(|| "not-set".into())
        .child(muted("Not set"))
}
