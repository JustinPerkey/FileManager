//! The embedded fonts. Cascadia Mono (SIL Open Font License 1.1) ships inside
//! the exe so text renders the same on every machine. Segoe UI is a Windows 11
//! system font and is not embedded. See `assets/fonts/README.md` for where the
//! files came from.

use std::borrow::Cow;

use gpui::App;

/// The family name both embedded files register under.
pub const MONO_FAMILY: &str = "Cascadia Mono";

/// The licence text of the embedded fonts; it travels with the exe.
pub const CASCADIA_LICENSE: &str = include_str!("../assets/fonts/OFL.txt");

const REGULAR: &[u8] = include_bytes!("../assets/fonts/CascadiaMono-Regular.ttf");
const BOLD: &[u8] = include_bytes!("../assets/fonts/CascadiaMono-Bold.ttf");

/// Registers the embedded fonts with GPUI's text system.
pub fn register(cx: &App) {
    if let Err(e) = cx.text_system().add_fonts(font_data()) {
        // Text still renders, in the fallback font; there is no logger yet.
        eprintln!("could not register the embedded fonts: {e:#}");
    }
}

fn font_data() -> Vec<Cow<'static, [u8]>> {
    vec![Cow::Borrowed(REGULAR), Cow::Borrowed(BOLD)]
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::sync::{Arc, Mutex};

    use gpui::{
        Bounds, DevicePixels, Font, FontId, FontMetrics, FontRun, GlyphId, LineLayout,
        NoopTextSystem, Pixels, PlatformTextSystem, RenderGlyphParams, Result, Size,
        TestAppContext, TestDispatcher, TextRenderingMode,
    };

    use super::*;

    /// The test platform's text system registers nothing, so this one wraps
    /// it and records what `add_fonts` was given.
    #[derive(Default)]
    struct Recording {
        fonts: Mutex<Vec<Vec<u8>>>,
    }

    impl PlatformTextSystem for Recording {
        fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
            let mut all = self.fonts.lock().unwrap();
            all.extend(fonts.into_iter().map(Cow::into_owned));
            Ok(())
        }
        fn all_font_names(&self) -> Vec<String> {
            Vec::new()
        }
        fn font_id(&self, d: &Font) -> Result<FontId> {
            NoopTextSystem.font_id(d)
        }
        fn font_metrics(&self, id: FontId) -> FontMetrics {
            NoopTextSystem.font_metrics(id)
        }
        fn typographic_bounds(&self, id: FontId, g: GlyphId) -> Result<Bounds<f32>> {
            NoopTextSystem.typographic_bounds(id, g)
        }
        fn advance(&self, id: FontId, g: GlyphId) -> Result<Size<f32>> {
            NoopTextSystem.advance(id, g)
        }
        fn glyph_for_char(&self, id: FontId, ch: char) -> Option<GlyphId> {
            NoopTextSystem.glyph_for_char(id, ch)
        }
        fn glyph_raster_bounds(&self, p: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
            NoopTextSystem.glyph_raster_bounds(p)
        }
        fn rasterize_glyph(
            &self,
            p: &RenderGlyphParams,
            b: Bounds<DevicePixels>,
        ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
            NoopTextSystem.rasterize_glyph(p, b)
        }
        fn layout_line(&self, text: &str, size: Pixels, runs: &[FontRun]) -> LineLayout {
            NoopTextSystem.layout_line(text, size, runs)
        }
        fn recommended_rendering_mode(&self, id: FontId, size: Pixels) -> TextRenderingMode {
            NoopTextSystem.recommended_rendering_mode(id, size)
        }
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    #[test]
    fn cascadia_mono_is_registered() {
        let recording = Arc::new(Recording::default());
        let cx =
            TestAppContext::build_with_text_system(TestDispatcher::new(0), None, recording.clone());
        cx.update(|cx| register(cx));
        let registered = recording.fonts.lock().unwrap().clone();
        assert_eq!(registered, vec![REGULAR.to_vec(), BOLD.to_vec()]);
        // The test platform cannot list names, so check the family is in each
        // file's `name` table (UTF-16BE on Windows platforms, ASCII on Mac).
        let utf16: Vec<u8> = MONO_FAMILY
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect();
        for (i, data) in registered.iter().enumerate() {
            assert!(
                contains(data, &utf16) || contains(data, MONO_FAMILY.as_bytes()),
                "font {i} is not {MONO_FAMILY}"
            );
        }
    }

    #[test]
    fn license_is_embedded() {
        assert!(CASCADIA_LICENSE.contains("SIL OPEN FONT LICENSE"));
    }
}
