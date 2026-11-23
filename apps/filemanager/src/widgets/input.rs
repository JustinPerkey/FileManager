//! Text fields: `.input` of `controls.css` over gpui-component's `Input` (one
//! line) and `Textarea` (several).
//!
//! The field's state (`InputState`, `TextareaState`) belongs to the view: it
//! holds the text, the caret and the focus. This module only dresses it.
//! The fill and border are set on the element, not taken from the theme slot.

use gpui::prelude::*;
use gpui::{px, App, Entity, SharedString, Window};
use gpui_component::input::{Input, InputState, Textarea, TextareaState};

use crate::fonts::mono_family;
use crate::theme::tokens::{FONT_SIZE_MD, LINE_HEIGHT, RADIUS, REM, SPACE_1, SPACE_2};
use crate::theme::{palette, INPUT_DISABLED_OPACITY};

/// The shortest a field gets.
const MIN_HEIGHT: f32 = 32.0;

enum Source {
    Line(Entity<InputState>),
    Area(Entity<TextareaState>),
}

/// A text field. Build it with [`text_input`] or [`text_area`].
#[derive(IntoElement)]
#[expect(clippy::struct_excessive_bools, reason = "independent builder flags")]
pub struct FmInput {
    source: Source,
    mono: bool,
    invalid: bool,
    read_only: bool,
    disabled: bool,
    rows: Option<usize>,
    label: Option<SharedString>,
}

/// A one-line field over `state`.
#[must_use]
pub fn text_input(state: &Entity<InputState>) -> FmInput {
    FmInput::new(Source::Line(state.clone()))
}

/// A multi-line field over `state`; set its height with [`FmInput::multiline`].
#[must_use]
pub fn text_area(state: &Entity<TextareaState>) -> FmInput {
    FmInput::new(Source::Area(state.clone()))
}

impl FmInput {
    fn new(source: Source) -> Self {
        Self {
            source,
            mono: false,
            invalid: false,
            read_only: false,
            disabled: false,
            rows: None,
            label: None,
        }
    }

    /// Text in the mono family (paths, identifiers).
    #[must_use]
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }

    /// Draws the border in `danger` while the content is not accepted.
    #[must_use]
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Shows the text on `surface_sunken` without a border. It keeps full
    /// strength, stays focusable and its text stays selectable and copyable.
    #[must_use]
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Faded, and ignores the pointer and the keyboard.
    ///
    /// gpui-component may apply its own disabled styling on top of our opacity
    /// 0.6, so the input could fade twice. Left as is for now: the first
    /// screenshot that shows a disabled input checks it. If it fades twice,
    /// drop our 0.6 and keep gpui-component's, if its text still meets 4.5:1.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// How many lines tall a [`text_area`] is. A one-line field ignores it.
    #[must_use]
    pub fn multiline(mut self, rows: usize) -> Self {
        self.rows = Some(rows);
        self
    }

    /// The accessible name, for a field whose label is not its placeholder.
    #[must_use]
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl RenderOnce for FmInput {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = *palette(cx);
        let transparent = gpui::transparent_black();
        let fill = if self.read_only {
            p.surface_sunken
        } else {
            p.surface
        };
        let border = if self.invalid {
            p.danger
        } else if self.read_only {
            transparent
        } else {
            p.border
        };
        let font = self.mono.then(|| mono_family(cx));
        let line = FONT_SIZE_MD * REM * LINE_HEIGHT;
        let area_height = self.rows.map(|rows| {
            px(f32::from(u16::try_from(rows).unwrap_or(u16::MAX)) * line + 2.0 * SPACE_1 + 2.0)
        });
        let (disabled, read_only, label) = (self.disabled, self.read_only, self.label);
        let opacity = if disabled {
            INPUT_DISABLED_OPACITY
        } else {
            1.0
        };

        match self.source {
            Source::Line(state) => Input::new(&state)
                .bg(fill)
                .border_color(border)
                .rounded(px(RADIUS))
                .min_h(px(MIN_HEIGHT))
                .px(px(SPACE_2))
                .py(px(SPACE_1))
                .opacity(opacity)
                .when_some(font, Styled::font_family)
                .disabled(disabled)
                .readonly(read_only)
                .when_some(label, Input::aria_label)
                .into_any_element(),
            Source::Area(state) => Textarea::new(&state)
                .bg(fill)
                .border_color(border)
                .rounded(px(RADIUS))
                .min_h(px(MIN_HEIGHT))
                .px(px(SPACE_2))
                .py(px(SPACE_1))
                .opacity(opacity)
                .when_some(font, Styled::font_family)
                .when_some(area_height, Textarea::h)
                .disabled(disabled)
                .readonly(read_only)
                .when_some(label, Textarea::aria_label)
                .into_any_element(),
        }
    }
}
