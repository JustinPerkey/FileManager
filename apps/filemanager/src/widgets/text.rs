//! Text roles: today's `base.css` type scale. Each function returns a [`Typo`]
//! (an element that is also `Styled`, so a view can add width or margins).
//! Colour is the palette's `text` or `text_muted`; sizes are the token sizes.

use std::sync::Arc;

use gpui::prelude::*;
use gpui::{
    div, px, relative, App, FontFeatures, FontWeight, SharedString, StyleRefinement, Window,
};
use gpui_component::StyledExt as _;

use crate::fonts::mono_family;
use crate::theme::tokens::{
    FONT_SIZE_LG, FONT_SIZE_MD, FONT_SIZE_SM, FONT_SIZE_XL, LINE_HEIGHT, REM,
};
use crate::theme::{palette, INTRO_MAX_WIDTH};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Role {
    H1,
    H2,
    Body,
    Muted,
    Small,
    SmallMuted,
    Mono,
    MonoSmallMuted,
    Intro,
}

/// One run of text in a role. Build it with [`h1`], [`body`] and friends.
#[derive(IntoElement)]
pub struct Typo {
    text: SharedString,
    role: Role,
    style: StyleRefinement,
}

impl Typo {
    fn new(role: Role, text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            role,
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for Typo {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Typo {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        let (size, weight, muted, mono) = match self.role {
            Role::H1 => (FONT_SIZE_XL, FontWeight::BOLD, false, false),
            Role::H2 => (FONT_SIZE_LG, FontWeight::SEMIBOLD, false, false),
            Role::Body => (FONT_SIZE_MD, FontWeight::NORMAL, false, false),
            Role::Muted | Role::Intro => (FONT_SIZE_MD, FontWeight::NORMAL, true, false),
            Role::Small => (FONT_SIZE_SM, FontWeight::NORMAL, false, false),
            Role::SmallMuted => (FONT_SIZE_SM, FontWeight::NORMAL, true, false),
            Role::Mono => (FONT_SIZE_MD, FontWeight::NORMAL, false, true),
            Role::MonoSmallMuted => (FONT_SIZE_SM, FontWeight::NORMAL, true, true),
        };
        div()
            .text_size(px(size * REM))
            .line_height(relative(LINE_HEIGHT))
            .font_weight(weight)
            .text_color(if muted { p.text_muted } else { p.text })
            .when(mono, |d| d.font_family(mono_family(cx)))
            .when(self.role == Role::Intro, |d| d.max_w(px(INTRO_MAX_WIDTH)))
            .refine_style(&self.style)
            .child(self.text)
    }
}

/// The view title: 20 px, bold.
#[must_use]
pub fn h1(text: impl Into<SharedString>) -> Typo {
    Typo::new(Role::H1, text)
}

/// A section heading: 16 px, semibold.
#[must_use]
pub fn h2(text: impl Into<SharedString>) -> Typo {
    Typo::new(Role::H2, text)
}

/// Body text: 14 px.
#[must_use]
pub fn body(text: impl Into<SharedString>) -> Typo {
    Typo::new(Role::Body, text)
}

/// Body text in the muted colour.
#[must_use]
pub fn muted(text: impl Into<SharedString>) -> Typo {
    Typo::new(Role::Muted, text)
}

/// 13 px text.
#[must_use]
pub fn small(text: impl Into<SharedString>) -> Typo {
    Typo::new(Role::Small, text)
}

/// 13 px muted text.
#[must_use]
pub fn small_muted(text: impl Into<SharedString>) -> Typo {
    Typo::new(Role::SmallMuted, text)
}

/// Body-size text in the mono family (paths, identifiers, file contents).
#[must_use]
pub fn mono(text: impl Into<SharedString>) -> Typo {
    Typo::new(Role::Mono, text)
}

/// 13 px muted mono text.
#[must_use]
pub fn mono_small_muted(text: impl Into<SharedString>) -> Typo {
    Typo::new(Role::MonoSmallMuted, text)
}

/// A muted introduction paragraph, at most [`INTRO_MAX_WIDTH`] wide.
#[must_use]
pub fn intro(text: impl Into<SharedString>) -> Typo {
    Typo::new(Role::Intro, text)
}

/// Turns on tabular figures (the `tnum` OpenType feature) for the element and
/// its text, so counts and sizes line up in a column. GPUI exposes font
/// features on any `Styled` element, so this applies it for real; a font
/// without the feature ignores it.
#[must_use]
pub fn tabular<E: Styled>(el: E) -> E {
    el.font_features(FontFeatures(Arc::new(vec![("tnum".to_string(), 1)])))
}
