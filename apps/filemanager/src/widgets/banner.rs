//! The banner: one line of news, in the colour of its tone, with an optional
//! action, details and a dismiss button. No tinted fill and no thick stripe:
//! a `surface` panel with a 1 px outline.

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{div, px, App, ElementId, SharedString, Window};
use gpui_component::{h_flex, v_flex};

use super::button::{button, FmButton};
use super::disclosure::Disclosure;
use super::icon::{icon, IconName};
use super::text::mono_small_muted;
use crate::theme::palette;
use crate::theme::tokens::{RADIUS, SPACE_1, SPACE_2, SPACE_3};

/// The least width a message gets before the actions wrap below it (16rem).
const BODY_BASIS: f32 = 256.0;

/// What a banner is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// Accent outline, `info` icon.
    Info,
    /// `warn` outline, `alert-triangle` icon.
    Warn,
    /// `danger` outline, `x-circle` icon.
    Error,
}

impl Tone {
    /// The icon of the tone.
    #[must_use]
    pub fn icon(self) -> IconName {
        match self {
            Self::Info => IconName::Info,
            Self::Warn => IconName::AlertTriangle,
            Self::Error => IconName::XCircle,
        }
    }
}

type DismissHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A banner.
#[derive(IntoElement)]
pub struct Banner {
    id: ElementId,
    tone: Tone,
    message: SharedString,
    action: Option<FmButton>,
    dismiss: Option<DismissHandler>,
    details: Option<SharedString>,
}

impl Banner {
    /// A banner with `message`. Give each banner of a view its own
    /// [`Banner::id`] when it can show details.
    #[must_use]
    pub fn new(tone: Tone, message: impl Into<SharedString>) -> Self {
        Self {
            id: "banner".into(),
            tone,
            message: message.into(),
            action: None,
            dismiss: None,
            details: None,
        }
    }

    /// The id the banner's buttons and its details state are keyed by.
    #[must_use]
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    /// A button at the right. Give it an id that differs from the dismiss one.
    #[must_use]
    pub fn action(mut self, action: FmButton) -> Self {
        self.action = Some(action);
        self
    }

    /// A quiet "x" button (tooltip "Dismiss") at the right.
    #[must_use]
    pub fn dismiss(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.dismiss = Some(Rc::new(handler));
        self
    }

    /// Technical text under the message, behind a "Details" disclosure.
    #[must_use]
    pub fn details(mut self, text: impl Into<SharedString>) -> Self {
        self.details = Some(text.into());
        self
    }
}

impl RenderOnce for Banner {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = *palette(cx);
        let tone = match self.tone {
            Tone::Info => p.accent,
            Tone::Warn => p.warn,
            Tone::Error => p.danger,
        };
        let open = window.use_keyed_state((self.id.clone(), "details"), cx, |_, _| false);
        let is_open = *open.read(cx);

        let details = self.details.map(|text| {
            Disclosure::new((self.id.clone(), "details-toggle"), "Details")
                .debug_selector("banner-details-toggle")
                .open(is_open)
                .on_toggle(move |next, _window, cx| {
                    open.update(cx, |state, cx| {
                        *state = next;
                        cx.notify();
                    });
                })
                .child(
                    div()
                        .debug_selector(|| "banner-details-text".into())
                        .px(px(SPACE_3))
                        .child(mono_small_muted(text)),
                )
        });
        let has_actions = self.action.is_some() || self.dismiss.is_some();
        let dismiss_id = (self.id.clone(), "dismiss");

        h_flex()
            .flex_wrap()
            .items_center()
            .gap_x(px(SPACE_3))
            .gap_y(px(SPACE_2))
            .px(px(SPACE_3))
            .py(px(SPACE_2))
            .bg(p.surface)
            .border_1()
            .border_color(tone)
            .rounded(px(RADIUS))
            .child(icon(self.tone.icon()).text_color(tone))
            .child(
                v_flex()
                    .flex_grow(1.0)
                    .flex_shrink(1.0)
                    .flex_basis(px(BODY_BASIS))
                    .min_w_0()
                    .gap(px(SPACE_1))
                    .text_color(p.text)
                    .child(div().child(self.message))
                    .children(details),
            )
            .when(has_actions, |row| {
                row.child(
                    h_flex()
                        .ml_auto()
                        .gap(px(SPACE_1))
                        .children(self.action)
                        .when_some(self.dismiss, |actions, handler| {
                            actions.child(
                                button(dismiss_id, "Dismiss")
                                    .quiet()
                                    .icon_only(IconName::X, "Dismiss")
                                    .debug_selector("banner-dismiss")
                                    .on_click(move |_, window, cx| handler(window, cx)),
                            )
                        }),
                )
            })
    }
}
