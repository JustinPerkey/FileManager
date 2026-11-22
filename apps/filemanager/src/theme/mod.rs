//! The visual system: the tokens of `styles/tokens.css` ([`tokens`]) mapped
//! onto gpui-component's theme ([`apply`]), following the system's light or
//! dark appearance live ([`init`]), with an override for the screenshot
//! harness ([`set_override`]). Widgets read colours through [`palette`] and
//! never spell a colour themselves.

pub mod tokens;

#[cfg(test)]
mod tests;

use gpui::{px, App, Global, Hsla, Rgba, SharedString, Window, WindowAppearance};
use gpui_component::{Theme, ThemeColor, ThemeMode};

use self::tokens::{Colors, Rgb};

/// `65ch` at 14 px: the widest an intro paragraph runs, in px.
pub const INTRO_MAX_WIDTH: f32 = 546.0;
/// Opacity of a disabled control.
pub const DISABLED_OPACITY: f32 = 0.55;
/// Opacity of a disabled quiet (borderless) control.
pub const QUIET_DISABLED_OPACITY: f32 = 0.45;
/// Opacity of a disabled input.
pub const INPUT_DISABLED_OPACITY: f32 = 0.6;

/// Black, the base of the scrim.
const BLACK: Rgb = Rgb(0, 0, 0);
/// How much of the accent (or danger) a hovered or pressed fill keeps; the
/// rest is the text colour (`color-mix(in srgb, accent 88%, text)`).
const HOVER_MIX: f32 = 0.88;

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

/// The token set for `mode`.
#[must_use]
pub fn colors_for(mode: ThemeMode) -> &'static Colors {
    if mode.is_dark() {
        &tokens::DARK
    } else {
        &tokens::LIGHT
    }
}

/// The current token colours as GPUI colours, plus the derived hover fills.
/// Read with [`palette`]; replaced by [`apply`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub dark: bool,
    pub bg: Hsla,
    pub surface: Hsla,
    pub surface_sunken: Hsla,
    pub border: Hsla,
    pub text: Hsla,
    pub text_muted: Hsla,
    pub accent: Hsla,
    pub accent_text: Hsla,
    pub ok: Hsla,
    pub warn: Hsla,
    pub danger: Hsla,
    pub focus_ring: Hsla,
    /// `--drop-overlay`, alpha included.
    pub drop_overlay: Hsla,
    /// `--scrim`, alpha included.
    pub scrim: Hsla,
    /// Accent at 88% over text: hover and pressed primary fill.
    pub primary_hover: Hsla,
    /// Danger at 88% over text: hover and pressed danger fill.
    pub danger_hover: Hsla,
}

impl Palette {
    /// The palette of `mode`.
    #[must_use]
    pub fn new(mode: ThemeMode) -> Self {
        let t = colors_for(mode);
        Self {
            dark: mode.is_dark(),
            bg: hsla(t.bg),
            surface: hsla(t.surface),
            surface_sunken: hsla(t.surface_sunken),
            border: hsla(t.border),
            text: hsla(t.text),
            text_muted: hsla(t.text_muted),
            accent: hsla(t.accent),
            accent_text: hsla(t.accent_text),
            ok: hsla(t.ok),
            warn: hsla(t.warn),
            danger: hsla(t.danger),
            focus_ring: hsla(t.focus_ring),
            drop_overlay: hsla_a(t.drop_overlay.0, t.drop_overlay.1),
            scrim: hsla_a(BLACK, t.scrim_alpha),
            primary_hover: hsla(t.accent.over(HOVER_MIX, t.text)),
            danger_hover: hsla(t.danger.over(HOVER_MIX, t.text)),
        }
    }
}

impl Global for Palette {}

/// The palette of the applied theme. Light until [`apply`] has run.
#[must_use]
pub fn palette(cx: &App) -> &Palette {
    cx.try_global::<Palette>().unwrap_or(&LIGHT_PALETTE)
}

static LIGHT_PALETTE: std::sync::LazyLock<Palette> =
    std::sync::LazyLock::new(|| Palette::new(ThemeMode::Light));

/// Switches the global gpui-component theme to `mode`'s tokens and stores the
/// matching [`Palette`].
pub fn apply(mode: ThemeMode, cx: &mut App) {
    let p = Palette::new(mode);
    let ui = SharedString::from(tokens::FONT_UI);
    let mono = crate::fonts::mono_family(cx);
    // A mode change loads that mode's registered theme and replaces the
    // colours, so the colours are edited in a second `update`.
    Theme::change(mode, None, cx);
    Theme::update(cx, |th| {
        th.font_family = ui;
        th.mono_font_family = mono;
        // gpui-component's `Root` makes this the window's rem size, so it
        // stays 16 px (1rem as in tokens.css).
        th.font_size = px(tokens::REM);
        th.mono_font_size = px(14.0);
        th.radius = px(tokens::RADIUS);
        th.radius_lg = px(tokens::RADIUS);
        // `shadow_overlay_alpha` is not applied: the dialog and popover
        // shadow is fixed in gpui-component.
        th.shadow = false;

        set_neutral_slots(&mut th.colors, &p);
        set_accent_slots(&mut th.colors, &p);
    });
    cx.set_global(p);
}

/// Text, border and surface slots of the mapping table.
fn set_neutral_slots(c: &mut ThemeColor, p: &Palette) {
    // text
    for slot in [
        &mut c.foreground,
        &mut c.caret,
        &mut c.popover_foreground,
        &mut c.accent_foreground,
        &mut c.button_foreground,
        &mut c.secondary_foreground,
        &mut c.sidebar_foreground,
        &mut c.sidebar_accent_foreground,
    ] {
        *slot = p.text;
    }
    // border
    for slot in [
        &mut c.border,
        &mut c.input,
        &mut c.sidebar_border,
        &mut c.table_row_border,
        &mut c.window_border,
        &mut c.title_bar_border,
        &mut c.scrollbar_thumb,
    ] {
        *slot = p.border;
    }
    // surface-sunken
    for slot in [
        &mut c.muted,
        &mut c.accent,
        &mut c.sidebar,
        &mut c.button_hover,
        &mut c.button_active,
        &mut c.secondary_hover,
        &mut c.secondary_active,
        &mut c.list_hover,
        &mut c.list_active,
        &mut c.table_hover,
        &mut c.table_active,
        &mut c.scrollbar,
        &mut c.title_bar,
    ] {
        *slot = p.surface_sunken;
    }
    // text-muted
    for slot in [
        &mut c.muted_foreground,
        &mut c.table_head_foreground,
        &mut c.scrollbar_thumb_hover,
    ] {
        *slot = p.text_muted;
    }
    // surface
    for slot in [
        &mut c.popover,
        &mut c.button,
        &mut c.secondary,
        &mut c.sidebar_accent,
        &mut c.list,
        &mut c.list_even,
        &mut c.table,
        &mut c.table_even,
        &mut c.table_head,
    ] {
        *slot = p.surface;
    }
    c.list_head = p.bg;
    c.background = p.bg;
}

/// Accent, status and overlay slots of the mapping table.
fn set_accent_slots(c: &mut ThemeColor, p: &Palette) {
    // accent
    for slot in [
        &mut c.primary,
        &mut c.button_primary,
        &mut c.sidebar_primary,
        &mut c.selection,
        &mut c.link,
        &mut c.info,
        &mut c.progress_bar,
        &mut c.drag_border,
        &mut c.list_active_border,
        &mut c.table_active_border,
    ] {
        *slot = p.accent;
    }
    c.ring = p.focus_ring;
    for slot in [
        &mut c.primary_hover,
        &mut c.primary_active,
        &mut c.button_primary_hover,
        &mut c.button_primary_active,
        &mut c.link_hover,
        &mut c.link_active,
    ] {
        *slot = p.primary_hover;
    }
    // text on a fill
    for slot in [
        &mut c.primary_foreground,
        &mut c.button_primary_foreground,
        &mut c.sidebar_primary_foreground,
        &mut c.info_foreground,
        &mut c.success_foreground,
        &mut c.warning_foreground,
        &mut c.danger_foreground,
        &mut c.button_danger_foreground,
    ] {
        *slot = p.accent_text;
    }
    // danger
    c.danger = p.danger;
    c.button_danger = p.danger;
    for slot in [
        &mut c.danger_hover,
        &mut c.danger_active,
        &mut c.button_danger_hover,
        &mut c.button_danger_active,
    ] {
        *slot = p.danger_hover;
    }
    c.success = p.ok;
    c.warning = p.warn;
    c.overlay = p.scrim;
    c.drop_target = p.drop_overlay;
}

/// What the theme follows: the window's appearance, unless overridden.
#[derive(Default)]
struct Appearance {
    system: Option<ThemeMode>,
    forced: Option<ThemeMode>,
}

impl Global for Appearance {}

/// Applies the mode the state calls for (override, else system, else light).
fn reapply(cx: &mut App) {
    let mode = {
        let state = cx.default_global::<Appearance>();
        state.forced.or(state.system).unwrap_or(ThemeMode::Light)
    };
    apply(mode, cx);
}

/// Applies the theme for `window`'s appearance and follows the system's light
/// or dark setting from now on, refreshing the window on each change.
pub fn init(window: &mut Window, cx: &mut App) {
    follow(window.appearance(), cx);
    window
        .observe_window_appearance(|window, cx| {
            follow(window.appearance(), cx);
            window.refresh();
        })
        .detach();
}

/// Records the system's appearance and applies the theme it (or an override)
/// calls for.
fn follow(appearance: WindowAppearance, cx: &mut App) {
    cx.default_global::<Appearance>().system = Some(appearance.into());
    reapply(cx);
}

/// Forces the theme to `mode` whatever the system says (the screenshot
/// harness's `--dark`); `None` goes back to following the system.
pub fn set_override(mode: Option<ThemeMode>, cx: &mut App) {
    cx.default_global::<Appearance>().forced = mode;
    reapply(cx);
}
