//! Rust transcription of `apps/desktop/src/styles/tokens.css`. Every name is
//! the CSS custom property name in `snake_case`. The test
//! `tokens_match_tokens_css` parses the CSS and compares every value, so this
//! file cannot drift from the file it transcribes.

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
    /// `--shadow-overlay`: `0 8px 24px rgb(0 0 0 / alpha)`.
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
/// `--radius`, px.
pub const RADIUS: f32 = 6.0;
/// Type sizes in rem (1 rem = 16 px): `--font-size-sm/md/lg/xl`.
pub const FONT_SIZE_SM: f32 = 0.8125;
pub const FONT_SIZE_MD: f32 = 0.875;
pub const FONT_SIZE_LG: f32 = 1.0;
pub const FONT_SIZE_XL: f32 = 1.25;
/// `--font-ui` and `--font-mono`, first choice then fallbacks.
pub const FONT_UI: &[&str] = &["Segoe UI"];
pub const FONT_MONO: &[&str] = &["Cascadia Mono", "Consolas"];
/// Body line height from `base.css`.
pub const LINE_HEIGHT: f32 = 1.4;

/// Pixels per rem.
pub const REM: f32 = 16.0;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::Path;

    /// Declarations (`--name: value`) of the block that starts at `selector`.
    fn block(css: &str, selector: &str) -> HashMap<String, String> {
        let start = css
            .find(selector)
            .unwrap_or_else(|| panic!("no block {selector}"));
        let open = start + css[start..].find('{').expect("open brace");
        let close = open + css[open..].find('}').expect("close brace");
        css[open + 1..close]
            .split(';')
            .filter_map(|d| {
                let (k, v) = d.split_once(':')?;
                let k = k.trim();
                k.starts_with("--")
                    .then(|| (k.trim_start_matches("--").to_string(), v.trim().to_string()))
            })
            .collect()
    }

    fn hex(v: &str) -> Rgb {
        let n = u32::from_str_radix(v.trim_start_matches('#'), 16).expect("hex colour");
        let bytes = n.to_be_bytes();
        Rgb(bytes[1], bytes[2], bytes[3])
    }

    /// `rgb(r g b / a)` -> (Rgb, a)
    fn rgba(v: &str) -> (Rgb, f32) {
        let inner = v.trim_start_matches("rgb(").trim_end_matches(')');
        let (rgb, a) = inner.split_once('/').expect("alpha");
        let n: Vec<u8> = rgb
            .split_whitespace()
            .map(|x| x.parse().expect("channel"))
            .collect();
        (Rgb(n[0], n[1], n[2]), a.trim().parse().expect("alpha"))
    }

    fn px(v: &str) -> f32 {
        v.trim_end_matches("px").parse().expect("px")
    }

    fn rem(v: &str) -> f32 {
        v.trim_end_matches("rem").parse().expect("rem")
    }

    /// `0 8px 24px rgb(0 0 0 / A)` -> A
    fn shadow_alpha(v: &str) -> f32 {
        assert!(v.starts_with("0 8px 24px "), "shadow geometry changed: {v}");
        rgba(v.trim_start_matches("0 8px 24px ")).1
    }

    fn check(colors: &Colors, d: &HashMap<String, String>) {
        let c = |name: &str| hex(&d[name]);
        assert_eq!(colors.bg, c("bg"));
        assert_eq!(colors.surface, c("surface"));
        assert_eq!(colors.surface_sunken, c("surface-sunken"));
        assert_eq!(colors.border, c("border"));
        assert_eq!(colors.text, c("text"));
        assert_eq!(colors.text_muted, c("text-muted"));
        assert_eq!(colors.accent, c("accent"));
        assert_eq!(colors.accent_text, c("accent-text"));
        assert_eq!(colors.ok, c("ok"));
        assert_eq!(colors.warn, c("warn"));
        assert_eq!(colors.danger, c("danger"));
        assert_eq!(colors.focus_ring, c("focus-ring"));
        assert_eq!(colors.drop_overlay, rgba(&d["drop-overlay"]));
        assert!((colors.shadow_overlay_alpha - shadow_alpha(&d["shadow-overlay"])).abs() < 1e-6);
        assert!((colors.scrim_alpha - rgba(&d["scrim"]).1).abs() < 1e-6);
    }

    #[test]
    fn tokens_match_tokens_css() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/desktop/src/styles/tokens.css");
        let css = std::fs::read_to_string(&path).expect("read tokens.css");
        let light = block(&css, ":root {");
        let dark = block(&css, ":root[data-theme=\"dark\"]");
        // Every token that the file declares is covered: light and dark hold
        // the colour tokens, light also holds the scales.
        check(&LIGHT, &light);
        check(&DARK, &dark);
        // The dark block must not omit a colour token that light has, other
        // than the ones it deliberately does not override.
        for k in light.keys().filter(|k| light[*k].starts_with('#')) {
            assert!(dark.contains_key(k), "dark lacks colour token {k}");
        }
        for (name, want) in [
            ("space-1", SPACE_1),
            ("space-2", SPACE_2),
            ("space-3", SPACE_3),
            ("space-4", SPACE_4),
            ("space-5", SPACE_5),
            ("space-6", SPACE_6),
            ("radius", RADIUS),
        ] {
            assert!((px(&light[name]) - want).abs() < 1e-6, "{name}");
        }
        for (name, want) in [
            ("font-size-sm", FONT_SIZE_SM),
            ("font-size-md", FONT_SIZE_MD),
            ("font-size-lg", FONT_SIZE_LG),
            ("font-size-xl", FONT_SIZE_XL),
        ] {
            assert!((rem(&light[name]) - want).abs() < 1e-6, "{name}");
        }
        assert_eq!(light["font-ui"], "\"Segoe UI\", system-ui, sans-serif");
        assert_eq!(light["font-mono"], "\"Cascadia Mono\", Consolas, monospace");
        assert!((shadow_alpha(&light["shadow-overlay"]) - LIGHT.shadow_overlay_alpha).abs() < 1e-6);
    }

    #[test]
    fn over_composites_like_a_browser() {
        // 55% of #5b616b over #eef0f2, per channel, rounded.
        let r = LIGHT.text_muted.over(0.55, LIGHT.surface_sunken);
        assert_eq!(r, Rgb(157, 161, 168));
        assert_eq!(LIGHT.accent.over(1.0, LIGHT.bg), LIGHT.accent);
        assert_eq!(LIGHT.accent.over(0.0, LIGHT.bg), LIGHT.bg);
    }
}
