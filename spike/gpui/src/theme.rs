//! Maps the transcribed tokens onto gpui-component's theme slots.
//!
//! Slots gpui-component has no equivalent for are listed in `REPORT.md`
//! ("Gaps found"); the screens set those values on the element instead
//! (`Tokens::surface` on the input fill, for one).

use gpui::{px, App, Hsla, Rgba, SharedString};
use gpui_component::{Theme, ThemeMode};

use crate::tokens::{self, Colors, Rgb};

/// An opaque token colour as GPUI's colour type.
#[must_use]
pub fn hsla(c: Rgb) -> Hsla {
    Rgba {
        r: f32::from(c.0) / 255.0,
        g: f32::from(c.1) / 255.0,
        b: f32::from(c.2) / 255.0,
        a: 1.0,
    }
    .into()
}

/// A token colour with an alpha.
#[must_use]
pub fn hsla_a(c: Rgb, alpha: f32) -> Hsla {
    let mut h = hsla(c);
    h.a = alpha;
    h
}

/// `color-mix(in srgb, a pct%, b)`.
fn mix(a: Rgb, pct: f32, b: Rgb) -> Hsla {
    hsla(a.over(pct, b))
}

#[must_use]
pub fn colors_for(dark: bool) -> &'static Colors {
    if dark {
        &tokens::DARK
    } else {
        &tokens::LIGHT
    }
}

/// First installed family of `choices`, else `fallback`.
#[must_use]
pub fn pick_font(cx: &App, choices: &[&str], fallback: &str) -> SharedString {
    let installed = cx.text_system().all_font_names();
    choices
        .iter()
        .find(|c| installed.iter().any(|n| n == **c))
        .map_or_else(
            || SharedString::from(fallback.to_string()),
            |c| SharedString::from((*c).to_string()),
        )
}

/// Switches the global gpui-component theme to the light or dark token set.
pub fn apply(dark: bool, cx: &mut App) {
    let t = *colors_for(dark);
    let ui = pick_font(cx, tokens::FONT_UI, ".SystemUIFont");
    let mono = pick_font(cx, tokens::FONT_MONO, "monospace");
    // A mode change loads that mode's registered theme and replaces the
    // colours, so the colours are edited in a second `update`.
    Theme::change(
        if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        None,
        cx,
    );
    Theme::update(cx, |th| {
        th.font_family = ui;
        th.mono_font_family = mono;
        th.font_size = px(tokens::FONT_SIZE_MD * tokens::REM);
        th.mono_font_size = px(tokens::FONT_SIZE_MD * tokens::REM);
        th.radius = px(tokens::RADIUS);
        th.radius_lg = px(tokens::RADIUS);
        th.shadow = false;

        th.background = hsla(t.bg);
        th.foreground = hsla(t.text);
        th.border = hsla(t.border);
        th.input = hsla(t.border);
        th.muted = hsla(t.surface_sunken);
        th.muted_foreground = hsla(t.text_muted);
        th.accent = hsla(t.surface_sunken);
        th.accent_foreground = hsla(t.text);
        th.ring = hsla(t.focus_ring);
        th.selection = hsla(t.accent);
        th.caret = hsla(t.text);
        th.link = hsla(t.accent);
        th.link_hover = mix(t.accent, 0.88, t.text);
        th.link_active = mix(t.accent, 0.88, t.text);
        th.overlay = hsla_a(Rgb(0, 0, 0), t.scrim_alpha);
        th.drop_target = hsla_a(t.drop_overlay.0, t.drop_overlay.1);

        th.popover = hsla(t.surface);
        th.popover_foreground = hsla(t.text);

        // Buttons: default (our "secondary"), primary, danger.
        th.button = hsla(t.surface);
        th.button_hover = hsla(t.surface_sunken);
        th.button_active = hsla(t.surface_sunken);
        th.button_foreground = hsla(t.text);
        th.secondary = hsla(t.surface);
        th.secondary_hover = hsla(t.surface_sunken);
        th.secondary_active = hsla(t.surface_sunken);
        th.secondary_foreground = hsla(t.text);
        th.primary = hsla(t.accent);
        th.primary_hover = mix(t.accent, 0.88, t.text);
        th.primary_active = mix(t.accent, 0.88, t.text);
        th.primary_foreground = hsla(t.accent_text);
        th.button_primary = hsla(t.accent);
        th.button_primary_hover = mix(t.accent, 0.88, t.text);
        th.button_primary_active = mix(t.accent, 0.88, t.text);
        th.button_primary_foreground = hsla(t.accent_text);
        th.danger = hsla(t.danger);
        th.danger_hover = mix(t.danger, 0.88, t.text);
        th.danger_active = mix(t.danger, 0.88, t.text);
        // tokens.css has no "text on danger"; accent-text is the one
        // text-on-fill token (white in light, dark navy in dark).
        th.danger_foreground = hsla(t.accent_text);
        th.button_danger = hsla(t.danger);
        th.button_danger_hover = mix(t.danger, 0.88, t.text);
        th.button_danger_active = mix(t.danger, 0.88, t.text);
        th.button_danger_foreground = hsla(t.accent_text);
        th.success = hsla(t.ok);
        th.success_foreground = hsla(t.accent_text);
        th.warning = hsla(t.warn);
        th.warning_foreground = hsla(t.accent_text);
        th.info = hsla(t.accent);
        th.info_foreground = hsla(t.accent_text);

        // Sidebar (nav).
        th.sidebar = hsla(t.surface_sunken);
        th.sidebar_border = hsla(t.border);
        th.sidebar_foreground = hsla(t.text);
        th.sidebar_accent = hsla(t.surface);
        th.sidebar_accent_foreground = hsla(t.text);
        th.sidebar_primary = hsla(t.accent);
        th.sidebar_primary_foreground = hsla(t.accent_text);

        // Lists, tables, menus.
        th.colors.list = hsla(t.surface);
        th.list_even = hsla(t.surface);
        th.list_head = hsla(t.bg);
        th.list_hover = hsla(t.surface_sunken);
        th.list_active = hsla(t.surface_sunken);
        th.list_active_border = hsla(t.accent);
        th.table = hsla(t.surface);
        th.table_even = hsla(t.surface);
        th.table_head = hsla(t.surface);
        th.table_head_foreground = hsla(t.text_muted);
        th.table_hover = hsla(t.surface_sunken);
        th.table_active = hsla(t.surface_sunken);
        th.table_active_border = hsla(t.accent);
        th.table_row_border = hsla(t.border);

        // Scrollbars and progress.
        th.scrollbar = hsla(t.surface_sunken);
        th.scrollbar_thumb = hsla(t.border);
        th.scrollbar_thumb_hover = hsla(t.text_muted);
        th.progress_bar = hsla(t.accent);
        th.drag_border = hsla(t.accent);
        th.window_border = hsla(t.border);
        th.title_bar = hsla(t.surface_sunken);
        th.title_bar_border = hsla(t.border);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use gpui_component::ActiveTheme as _;

    #[gpui::test]
    fn apply_sets_slots_from_tokens(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        cx.update(|cx| apply(false, cx));
        let (bg, sidebar, primary) = cx.read(|cx| {
            (
                cx.theme().background,
                cx.theme().sidebar,
                cx.theme().primary,
            )
        });
        assert_eq!(bg, hsla(tokens::LIGHT.bg));
        assert_eq!(sidebar, hsla(tokens::LIGHT.surface_sunken));
        assert_eq!(primary, hsla(tokens::LIGHT.accent));
        cx.update(|cx| apply(true, cx));
        let (bg, dark) = cx.read(|cx| (cx.theme().background, cx.theme().is_dark()));
        assert!(dark);
        assert_eq!(bg, hsla(tokens::DARK.bg));
    }
}
