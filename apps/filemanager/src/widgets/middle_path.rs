//! A path on one line with its middle cut out, so the drive and the file name
//! stay visible. GPUI's `text_ellipsis` only cuts the end.
//!
//! Paths arrive as display strings and stay strings here: nothing parses them
//! into a `PathBuf`.

use gpui::prelude::*;
use gpui::{div, px, App, Bounds, ElementId, Font, Pixels, SharedString, Window};
use gpui_component::ElementExt as _;

use super::button::tooltip_view;
use crate::fonts::mono_family;

/// The fewest characters that can show anything of a path.
const MIN_CHARS: usize = 5;
/// The least head kept ahead of the ellipsis: a drive (`C:\`).
const MIN_HEAD: usize = 3;
const ELLIPSIS: char = '\u{2026}';

/// Cuts `text` to `max_chars` characters, replacing the middle with `…`.
///
/// The start is kept, and the file name with its separator (`\gateway.exe`)
/// when the head and the ellipsis still fit with it; otherwise the start and
/// the end split the room evenly. Text that fits, and a `max_chars` under 5,
/// come back unchanged. It counts `char`s, so a name with U+FFFD or an
/// astral character is cut between characters, never inside one.
#[must_use]
pub fn middle_truncate(text: &str, max_chars: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    if n <= max_chars || max_chars < MIN_CHARS {
        return text.to_string();
    }
    // The separator and the name after the last one (the whole text, with no
    // separator at all).
    let name_start = chars
        .iter()
        .rposition(|c| matches!(c, '\\' | '/'))
        .unwrap_or(0);
    let name_len = n - name_start;
    let room = max_chars - 1;
    let tail = if name_len + MIN_HEAD <= room {
        name_len
    } else {
        room / 2
    };
    let head = room - tail;
    let mut out: String = chars[..head].iter().collect();
    out.push(ELLIPSIS);
    out.extend(&chars[n - tail..]);
    out
}

/// A middle-truncated path in mono, with the full path as its tooltip.
#[derive(IntoElement)]
pub struct MiddlePath {
    id: ElementId,
    display: SharedString,
}

impl MiddlePath {
    /// A path to show. It fills the width of its parent and recomputes how
    /// much fits when that width changes (one frame later).
    #[must_use]
    pub fn new(display: impl Into<SharedString>) -> Self {
        Self {
            id: "middle-path".into(),
            display: display.into(),
        }
    }

    /// The id its measured width is kept under. Paths drawn in the same parent
    /// (the rows of a table) need ids that differ.
    #[must_use]
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    /// The tooltip: the whole path.
    #[must_use]
    pub fn tooltip_text(&self) -> SharedString {
        self.display.clone()
    }

    /// What is drawn when `width` fits `advance` px per character.
    fn shown(&self, width: Option<Pixels>, advance: Pixels) -> SharedString {
        match width {
            Some(w) if advance > px(0.0) => {
                let fits = (w / advance).floor().max(0.0);
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "a width in characters, clamped at zero"
                )]
                let max = fits as usize;
                middle_truncate(&self.display, max).into()
            }
            _ => self.display.clone(),
        }
    }
}

impl RenderOnce for MiddlePath {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let family = mono_family(cx);
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let font = Font {
            family: family.clone(),
            ..style.font()
        };
        let font_id = window.text_system().resolve_font(&font);
        let advance = window
            .text_system()
            .em_advance(font_id, font_size)
            .unwrap_or(px(0.0));

        let width = window.use_keyed_state((self.id.clone(), "width"), cx, |_, _| None::<Pixels>);
        let shown = self.shown(*width.read(cx), advance);
        let tooltip = self.tooltip_text();

        div()
            .id(self.id)
            .w_full()
            .min_w_0()
            .overflow_hidden()
            .whitespace_nowrap()
            .font_family(family)
            .on_prepaint(move |bounds: Bounds<Pixels>, _window, cx| {
                let changed = width
                    .read(cx)
                    .is_none_or(|old| (old - bounds.size.width).abs() > px(0.5));
                if changed {
                    width.update(cx, |w, cx| {
                        *w = Some(bounds.size.width);
                        cx.notify();
                    });
                }
            })
            .tooltip(tooltip_view(tooltip))
            .child(shown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn len(s: &str) -> usize {
        s.chars().count()
    }

    #[test]
    fn middle_truncate_cases() {
        // Drive and file name kept: the cut is in the middle.
        let p = r"C:\Users\me\build\out\gateway.exe";
        let t = middle_truncate(p, 24);
        assert_eq!(len(&t), 24);
        assert!(t.starts_with(r"C:\"), "{t}");
        assert!(t.ends_with(r"\gateway.exe"), "{t}");
        assert!(t.contains('\u{2026}'), "{t}");
        assert_eq!(t, "C:\\Users\\me\u{2026}\\gateway.exe");

        // Text that fits is unchanged, and so is anything under 5 characters.
        assert_eq!(middle_truncate("C:\\a.txt", 40), "C:\\a.txt");
        assert_eq!(middle_truncate(p, len(p)), p);
        assert_eq!(middle_truncate(p, 4), p);
        assert_eq!(middle_truncate(p, 0), p);

        // A file name too long to keep whole: the start and the end of the
        // path share the room evenly.
        let long = format!("C:\\d\\{}", "n".repeat(100));
        let t = middle_truncate(&long, 20);
        assert_eq!(len(&t), 20);
        assert!(t.starts_with("C:\\d\\"), "{t}");
        assert!(t.ends_with("nnnnnnnnn"), "{t}");

        // A 200-character path keeps the drive and the name.
        let deep = format!("C:\\{}\\file.txt", "d".repeat(190));
        let t = middle_truncate(&deep, 30);
        assert_eq!(len(&t), 30);
        assert!(t.starts_with("C:\\d"), "{t}");
        assert!(t.ends_with("\\file.txt"), "{t}");

        // A UNC prefix is plain head text.
        let unc = r"\\srv\share\deep\er\f.bin";
        let t = middle_truncate(unc, 18);
        assert_eq!(len(&t), 18);
        assert!(t.starts_with(r"\\srv\"), "{t}");
        assert!(t.ends_with(r"\f.bin"), "{t}");

        // No separator: no name to keep, an even split.
        let t = middle_truncate("gateway-release-candidate.tar.zst", 15);
        assert_eq!(len(&t), 15);
        assert!(t.starts_with("gateway"), "{t}");
        assert!(t.ends_with("zst"), "{t}");

        // U+FFFD is an ordinary character and survives.
        let lossy = "C:\\d\u{FFFD}\\a-longer-folder\\bad\u{FFFD}.txt";
        let t = middle_truncate(lossy, 20);
        assert_eq!(len(&t), 20);
        assert!(t.ends_with("\\bad\u{FFFD}.txt"), "{t}");
        assert!(t.starts_with("C:\\d\u{FFFD}"), "{t}");

        // Astral characters count as one and are never split.
        let astral = format!("C:\\d\\{}", "\u{1F600}".repeat(40));
        let t = middle_truncate(&astral, 20);
        assert_eq!(len(&t), 20);
        assert!(t.ends_with('\u{1F600}'), "{t}");
    }

    #[test]
    fn narrower_than_a_character_shows_the_whole_path() {
        let m = MiddlePath::new(r"C:\a\b\c.txt");
        // Not measured yet: everything, clipped by the element.
        assert_eq!(&*m.shown(None, px(8.0)), r"C:\a\b\c.txt");
        // A width of five characters at 8 px.
        assert_eq!(len(&m.shown(Some(px(40.0)), px(8.0))), 5);
        // No usable font advance: everything.
        assert_eq!(&*m.shown(Some(px(40.0)), px(0.0)), r"C:\a\b\c.txt");
        // Wide enough: unchanged.
        assert_eq!(&*m.shown(Some(px(400.0)), px(8.0)), r"C:\a\b\c.txt");
    }
}
