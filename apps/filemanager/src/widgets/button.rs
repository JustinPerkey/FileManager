//! The button: `.btn` of `controls.css` over gpui-component's `Button`.
//!
//! gpui-component supplies focus, Tab order, Enter and Space activation, the
//! focus ring and the tooltip; this module supplies the look. The variant is
//! gpui-component's `Custom` one so the hover and pressed fills are ours, and
//! the rest-state fill, border and text colour are set on the element, which
//! gpui-component replays inside its own hover and disabled states.

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{px, AnyView, App, ClickEvent, ElementId, FocusHandle, Hsla, SharedString, Window};
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
use gpui_component::spinner::Spinner;
use gpui_component::tooltip::Tooltip;
use gpui_component::{h_flex, Disableable as _, Sizable as _, Size};

use super::icon::{icon, IconName};
use crate::theme::tokens::{FONT_SIZE_MD, RADIUS, REM, SPACE_1, SPACE_3};
use crate::theme::{palette, Palette, DISABLED_OPACITY, QUIET_DISABLED_OPACITY};

/// Height of a button: padding 4 + a 1.4 line of 14 px + the 1 px borders.
const HEIGHT: f32 = 30.0;
/// The smallest a button gets (an icon-only one).
const MIN_SIZE: f32 = 24.0;

/// How a button is dressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Variant {
    /// `surface` fill, `border` outline.
    #[default]
    Secondary,
    /// The one accent-filled action of a view.
    Primary,
    /// Transparent until hovered.
    Quiet,
    /// `danger` label and outline on `surface`; never filled.
    Danger,
}

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// A button. Build it with [`button`].
#[derive(IntoElement)]
#[expect(clippy::struct_excessive_bools, reason = "independent builder flags")]
pub struct FmButton {
    id: ElementId,
    label: SharedString,
    label_visible: bool,
    variant: Variant,
    icon: Option<IconName>,
    icon_flip: bool,
    trailing: Option<IconName>,
    disabled: bool,
    busy: bool,
    tooltip: Option<SharedString>,
    shortcut: Option<SharedString>,
    on_click: Option<ClickHandler>,
    focus: Option<FocusHandle>,
    debug_selector: Option<SharedString>,
}

/// A secondary button. `label` is the visible text, and the accessible name
/// when [`FmButton::icon_only`] hides it.
#[must_use]
pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> FmButton {
    FmButton {
        id: id.into(),
        label: label.into(),
        label_visible: true,
        variant: Variant::Secondary,
        icon: None,
        icon_flip: false,
        trailing: None,
        disabled: false,
        busy: false,
        tooltip: None,
        shortcut: None,
        on_click: None,
        focus: None,
        debug_selector: None,
    }
}

impl FmButton {
    /// The accent-filled button.
    #[must_use]
    pub fn primary(mut self) -> Self {
        self.variant = Variant::Primary;
        self
    }

    /// The default look, spelled out.
    #[must_use]
    pub fn secondary(mut self) -> Self {
        self.variant = Variant::Secondary;
        self
    }

    /// Transparent until hovered.
    #[must_use]
    pub fn quiet(mut self) -> Self {
        self.variant = Variant::Quiet;
        self
    }

    /// A destructive action: `danger` text and outline, no fill.
    #[must_use]
    pub fn danger(mut self) -> Self {
        self.variant = Variant::Danger;
        self
    }

    /// An icon before the label, 4 px apart.
    #[must_use]
    pub fn icon(mut self, name: IconName) -> Self {
        self.icon = Some(name);
        self
    }

    /// Rotates the icon 180 degrees (a chevron that points the other way).
    #[must_use]
    pub fn icon_flip(mut self, flip: bool) -> Self {
        self.icon_flip = flip;
        self
    }

    /// An icon after the label (a menu button's chevron), 4 px apart.
    #[must_use]
    pub fn trailing_icon(mut self, name: IconName) -> Self {
        self.trailing = Some(name);
        self
    }

    /// Shows only the icon. The tooltip is the only visible name, so it is
    /// required here: there is no way to build an unlabelled icon button.
    #[must_use]
    pub fn icon_only(mut self, name: IconName, tooltip: impl Into<SharedString>) -> Self {
        self.icon = Some(name);
        self.label_visible = false;
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Disabled: muted, faded, and it ignores the pointer and the keyboard.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Busy: work is running. Inert, full strength, the label unchanged, with
    /// a spinner in place of the icon.
    #[must_use]
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    /// The tooltip (the shortcut, when set, is appended to it).
    #[must_use]
    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    /// Names the keyboard shortcut in the tooltip: "Shortcut: Ctrl+O".
    #[must_use]
    pub fn shortcut(mut self, keys: impl Into<SharedString>) -> Self {
        self.shortcut = Some(keys.into());
        self
    }

    /// Runs on a click, or on Enter or Space while the button has focus.
    #[must_use]
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// Tracks `handle` as the button's focus handle, so a view can move focus
    /// to it (a dialog starts on Cancel).
    #[must_use]
    pub fn track_focus(mut self, handle: &FocusHandle) -> Self {
        self.focus = Some(handle.clone());
        self
    }

    /// Names the element for `VisualTestContext::debug_bounds` (debug builds
    /// only; the screenshot harness uses `.tag(..)` instead).
    #[must_use]
    pub fn debug_selector(mut self, name: impl Into<SharedString>) -> Self {
        self.debug_selector = Some(name.into());
        self
    }

    /// The label drawn on the button; `None` for an icon-only button.
    #[must_use]
    pub fn visible_label(&self) -> Option<&SharedString> {
        self.label_visible.then_some(&self.label)
    }

    /// The tooltip text, shortcut included.
    #[must_use]
    pub fn tooltip_text(&self) -> Option<SharedString> {
        match (&self.tooltip, &self.shortcut) {
            (Some(tip), Some(keys)) => Some(format!("{tip}. Shortcut: {keys}").into()),
            (Some(tip), None) => Some(tip.clone()),
            (None, Some(keys)) => Some(format!("Shortcut: {keys}").into()),
            (None, None) => None,
        }
    }

    /// Whether the button is busy.
    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.busy
    }
}

/// The builder of a tooltip over `text`, shared by every widget that has one.
pub(crate) fn tooltip_view(text: SharedString) -> impl Fn(&mut Window, &mut App) -> AnyView {
    move |window, cx| Tooltip::new(text.clone()).build(window, cx)
}

/// What a button wears: `(fill, text, border, hover and pressed fill, opacity)`.
///
/// Busy beats disabled. A busy button is working, not unavailable: the
/// disabled colours at full strength (the spinner and the full opacity are
/// what tell it from a disabled one). Quiet keeps no fill and no border.
#[must_use]
pub(crate) fn button_colors(
    variant: Variant,
    disabled: bool,
    busy: bool,
    p: &Palette,
) -> (Hsla, Hsla, Hsla, Hsla, f32) {
    let transparent = gpui::transparent_black();
    let quiet = variant == Variant::Quiet;
    let (fill, text, border, hover) = match variant {
        Variant::Secondary => (p.surface, p.text, p.border, p.surface_sunken),
        Variant::Primary => (p.accent, p.accent_text, p.accent, p.primary_hover),
        Variant::Quiet => (transparent, p.text, transparent, p.surface_sunken),
        Variant::Danger => (p.surface, p.danger, p.danger, p.surface_sunken),
    };
    if busy {
        if quiet {
            (transparent, p.text_muted, transparent, hover, 1.0)
        } else {
            (p.surface_sunken, p.text_muted, p.border, hover, 1.0)
        }
    } else if disabled && quiet {
        (
            transparent,
            p.text_muted,
            transparent,
            hover,
            QUIET_DISABLED_OPACITY,
        )
    } else if disabled {
        (
            p.surface_sunken,
            p.text_muted,
            p.border,
            hover,
            DISABLED_OPACITY,
        )
    } else {
        (fill, text, border, hover, 1.0)
    }
}

impl RenderOnce for FmButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.into_button(cx)
    }
}

impl FmButton {
    /// The dressed gpui-component button, for widgets that attach their own
    /// behaviour to it (a menu button's popover).
    pub(crate) fn into_button(self, cx: &App) -> Button {
        let p = *palette(cx);
        let inert = self.disabled || self.busy;
        let (fill, text, border, hover, opacity) =
            button_colors(self.variant, self.disabled, self.busy, &p);
        let hover = if inert { fill } else { hover };

        let variant = ButtonCustomVariant::new(cx)
            .color(border)
            .foreground(text)
            .hover(hover)
            .active(hover);

        let tooltip = self.tooltip_text();
        let leading = if self.busy {
            Some(
                Spinner::new()
                    .with_size(Size::Size(px(FONT_SIZE_MD * REM)))
                    .color(text)
                    .into_any_element(),
            )
        } else {
            self.icon.map(|name| {
                let icon = icon(name).text_color(text);
                if self.icon_flip {
                    icon.rotate(gpui::radians(std::f32::consts::PI))
                } else {
                    icon
                }
                .into_any_element()
            })
        };
        let content = h_flex()
            .gap(px(SPACE_1))
            .items_center()
            .when_some(leading, ParentElement::child)
            .when(self.label_visible, |row| {
                row.child(gpui::div().whitespace_nowrap().child(self.label.clone()))
            })
            .when_some(self.trailing, |row, name| {
                row.child(icon(name).text_color(text))
            });

        let on_click = self.on_click.filter(|_| !inert);
        let selector = self.debug_selector;
        Button::new(self.id)
            .custom(variant)
            .accessibility_label(self.label)
            .h(px(HEIGHT))
            .min_w(px(MIN_SIZE))
            .min_h(px(MIN_SIZE))
            .px(px(SPACE_3))
            .py(px(SPACE_1))
            .border_1()
            .border_color(border)
            .bg(fill)
            .text_color(text)
            .opacity(opacity)
            .rounded(px(RADIUS))
            .disabled(self.disabled && !self.busy)
            .child(content)
            .when_some(self.focus, |b, handle| b.track_focus(&handle))
            .when_some(tooltip, Button::tooltip)
            .when_some(on_click, |b, handler| {
                b.on_click(move |event, window, cx| handler(event, window, cx))
            })
            .when_some(selector, |b, name| {
                b.debug_selector(move || name.to_string())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_is_appended_to_the_tooltip() {
        let b = button("b", "Open")
            .tooltip("Open a file")
            .shortcut("Ctrl+O");
        assert_eq!(
            b.tooltip_text().as_deref(),
            Some("Open a file. Shortcut: Ctrl+O")
        );
        let b = button("b", "Open").shortcut("Ctrl+O");
        assert_eq!(b.tooltip_text().as_deref(), Some("Shortcut: Ctrl+O"));
        assert_eq!(button("b", "Open").tooltip_text(), None);
    }

    #[test]
    fn icon_only_hides_the_label_and_requires_a_tooltip() {
        let b = button("b", "Dismiss").icon_only(IconName::X, "Dismiss");
        assert_eq!(b.visible_label(), None);
        assert_eq!(b.tooltip_text().as_deref(), Some("Dismiss"));
        assert_eq!(
            button("b", "Open").visible_label().map(AsRef::as_ref),
            Some("Open")
        );
    }

    #[test]
    fn busy_button_colours() {
        use gpui_component::ThemeMode;
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            let p = Palette::new(mode);
            let clear = gpui::transparent_black();
            for variant in [
                Variant::Secondary,
                Variant::Primary,
                Variant::Quiet,
                Variant::Danger,
            ] {
                let quiet = variant == Variant::Quiet;
                let (fill, text, border) = if quiet {
                    (clear, p.text_muted, clear)
                } else {
                    (p.surface_sunken, p.text_muted, p.border)
                };
                // Busy: the disabled colours at full strength, even if also disabled.
                for disabled in [false, true] {
                    let got = button_colors(variant, disabled, true, &p);
                    assert_eq!(
                        (got.0, got.1, got.2, got.4),
                        (fill, text, border, 1.0),
                        "busy {variant:?} disabled={disabled}"
                    );
                }
                // Disabled: unchanged, faded.
                let got = button_colors(variant, true, false, &p);
                let opacity = if quiet {
                    QUIET_DISABLED_OPACITY
                } else {
                    DISABLED_OPACITY
                };
                assert_eq!(
                    (got.0, got.1, got.2, got.4),
                    (fill, text, border, opacity),
                    "disabled {variant:?}"
                );
            }
            // Enabled primary keeps the accent fill.
            let got = button_colors(Variant::Primary, false, false, &p);
            assert_eq!((got.0, got.1, got.4), (p.accent, p.accent_text, 1.0));
        }
    }
}
