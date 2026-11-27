//! Rust transcription of `apps/desktop/src/styles/tokens.css`. Every name is
//! the CSS custom property name in `snake_case`. The test
//! `tokens_match_tokens_css` parses the CSS and compares every value, so this
//! file cannot drift from the file it transcribes (while that file exists).
//! This module and `mod.rs` are the only places in `src/` with colour literals.

/// An opaque colour, 8 bits per channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// Composite `self` at `alpha` over `under`, rounded to 8 bits (what a
    /// browser or GPUI shows for a translucent fill).
    #[must_use]
    pub fn over(self, alpha: f32, under: Rgb) -> Rgb {
        let mix = |a: u8, b: u8| -> u8 {
            let v = f32::from(a) * alpha + f32::from(b) * (1.0 - alpha);
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "rounded value is within 0..=255"
            )]
            let r = v.round() as u8;
            r
        };
        Rgb(
            mix(self.0, under.0),
            mix(self.1, under.1),
            mix(self.2, under.2),
        )
    }
}

/// One colour scheme: every colour token of `tokens.css`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Colors {
    pub bg: Rgb,
    pub surface: Rgb,
    pub surface_sunken: Rgb,
    pub border: Rgb,
    pub text: Rgb,
    pub text_muted: Rgb,
    pub accent: Rgb,
    pub accent_text: Rgb,
    pub ok: Rgb,
    pub warn: Rgb,
    pub danger: Rgb,
    pub focus_ring: Rgb,
    /// `--drop-overlay`: colour and alpha.
    pub drop_overlay: (Rgb, f32),
    /// `--shadow-overlay`: `0 8px 24px rgb(0 0 0 / alpha)`. **Not applied**:
    /// gpui-component's dialog and popover shadow is fixed and the human
    /// accepted that. Kept so the test can hold it to `tokens.css`.
    pub shadow_overlay_alpha: f32,
    /// `--scrim`: `rgb(0 0 0 / alpha)`.
    pub scrim_alpha: f32,
}

pub const LIGHT: Colors = Colors {
    bg: Rgb(0xf7, 0xf7, 0xf8),
    surface: Rgb(0xff, 0xff, 0xff),
    surface_sunken: Rgb(0xee, 0xf0, 0xf2),
    border: Rgb(0xd9, 0xdc, 0xe1),
    text: Rgb(0x1b, 0x1d, 0x21),
    text_muted: Rgb(0x5b, 0x61, 0x6b),
    accent: Rgb(0x24, 0x57, 0xc5),
    accent_text: Rgb(0xff, 0xff, 0xff),
    ok: Rgb(0x1d, 0x7a, 0x46),
    warn: Rgb(0x8a, 0x5a, 0x00),
    danger: Rgb(0xb3, 0x26, 0x1e),
    focus_ring: Rgb(0x24, 0x57, 0xc5),
    drop_overlay: (Rgb(36, 87, 197), 0.08),
    shadow_overlay_alpha: 0.18,
    scrim_alpha: 0.4,
};

pub const DARK: Colors = Colors {
    bg: Rgb(0x16, 0x17, 0x1a),
    surface: Rgb(0x1e, 0x20, 0x24),
    surface_sunken: Rgb(0x12, 0x13, 0x16),
    border: Rgb(0x33, 0x36, 0x3d),
    text: Rgb(0xe8, 0xe9, 0xec),
    text_muted: Rgb(0xa3, 0xa8, 0xb1),
    accent: Rgb(0x7a, 0xa2, 0xff),
    accent_text: Rgb(0x0d, 0x15, 0x30),
    ok: Rgb(0x5f, 0xd0, 0x8f),
    warn: Rgb(0xf2, 0xc1, 0x4e),
    danger: Rgb(0xff, 0x8a, 0x80),
    focus_ring: Rgb(0x7a, 0xa2, 0xff),
    drop_overlay: (Rgb(122, 162, 255), 0.12),
    shadow_overlay_alpha: 0.5,
    scrim_alpha: 0.6,
};

/// Spacing scale, `--space-1..6`, in px.
pub const SPACE_1: f32 = 4.0;
pub const SPACE_2: f32 = 8.0;
pub const SPACE_3: f32 = 12.0;
pub const SPACE_4: f32 = 16.0;
pub const SPACE_5: f32 = 24.0;
pub const SPACE_6: f32 = 32.0;
/// The width today's `scrollbar-gutter: stable` reserves on the right of a
/// scrolling view (about 15 px). GPUI's scrollbars overlay the content, so a
/// view that had the gutter pads its right edge by this much to keep its
/// columns where they were.
pub const SCROLLBAR_GUTTER: f32 = 15.0;
/// `--radius`, px.
pub const RADIUS: f32 = 6.0;
/// Type sizes in rem (1 rem = 16 px): `--font-size-sm/md/lg/xl`.
pub const FONT_SIZE_SM: f32 = 0.8125;
pub const FONT_SIZE_MD: f32 = 0.875;
pub const FONT_SIZE_LG: f32 = 1.0;
pub const FONT_SIZE_XL: f32 = 1.25;
/// `--font-ui`: the first family (system fonts only; nothing is embedded).
pub const FONT_UI: &str = crate::fonts::UI_FAMILY;
/// `--font-mono`: Cascadia Mono, else Consolas. The installed one is
/// `crate::fonts::mono_family(cx)`; there is no other picker.
pub const FONT_MONO: [&str; 2] = crate::fonts::MONO_FAMILIES;
/// Body line height from `base.css`.
pub const LINE_HEIGHT: f32 = 1.4;

/// Pixels per rem.
pub const REM: f32 = 16.0;
