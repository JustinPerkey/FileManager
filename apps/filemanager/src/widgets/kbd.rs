//! Keyboard keys in running text: `Ctrl` `+` `O`.

use gpui::prelude::*;
use gpui::{div, px, App, SharedString, Window};
use gpui_component::h_flex;

use crate::fonts::mono_family;
use crate::theme::palette;
use crate::theme::tokens::{FONT_SIZE_SM, RADIUS, REM, SPACE_1};

/// One key. Build it with [`kbd`].
#[derive(IntoElement)]
pub struct Kbd(SharedString);

/// One key cap: 13 px mono on `surface_sunken` with a 1 px `border`.
#[must_use]
pub fn kbd(key: impl Into<SharedString>) -> Kbd {
    Kbd(key.into())
}

impl RenderOnce for Kbd {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        div()
            .flex_none()
            .px(px(SPACE_1))
            .font_family(mono_family(cx))
            .text_size(px(FONT_SIZE_SM * REM))
            .text_color(p.text)
            .bg(p.surface_sunken)
            .border_1()
            .border_color(p.border)
            .rounded(px(RADIUS))
            .whitespace_nowrap()
            .child(self.0)
    }
}

/// A chord. Build it with [`chord`].
#[derive(IntoElement)]
pub struct Chord(Vec<SharedString>);

/// Keys pressed together: the caps joined with "+".
#[must_use]
pub fn chord(keys: &[&str]) -> Chord {
    Chord(
        keys.iter()
            .map(|k| SharedString::from((*k).to_string()))
            .collect(),
    )
}

impl RenderOnce for Chord {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let muted = palette(cx).text_muted;
        let mut row = h_flex().gap(px(SPACE_1)).items_center().flex_none();
        for (i, key) in self.0.into_iter().enumerate() {
            if i > 0 {
                row = row.child(div().text_color(muted).child("+"));
            }
            row = row.child(Kbd(key));
        }
        row
    }
}

/// Alternative chords. Build it with [`alternatives`].
#[derive(IntoElement)]
pub struct Alternatives(Vec<Chord>);

/// Chords that do the same thing, joined with "or" in `text_muted`.
#[must_use]
pub fn alternatives(chords: &[&[&str]]) -> Alternatives {
    Alternatives(chords.iter().map(|keys| chord(keys)).collect())
}

impl RenderOnce for Alternatives {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let muted = palette(cx).text_muted;
        let mut row = h_flex().gap(px(SPACE_1)).items_center().flex_none();
        for (i, chord) in self.0.into_iter().enumerate() {
            if i > 0 {
                row = row.child(div().text_color(muted).child("or"));
            }
            row = row.child(chord);
        }
        row
    }
}
