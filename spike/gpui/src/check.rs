//! B6 colour and geometry check, run on the B7 PNGs (`fm-spike --verify`).
//!
//! Colours: at 15 named points the rendered pixel must equal the token value
//! (after the stated opacity) within +-1 per channel. Geometry: the boxes the
//! screen recorded at paint time must be within 2 px of the reference image's,
//! measured by pixel inspection of `docs/manual/images/pipeline-chosen.png`.
//! Text rendering is not judged: text colours are the pixel farthest from the
//! background in a text region, so glyph shape and antialiasing do not matter.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use image::{Rgba, RgbaImage};

use crate::measure::Rect;
use crate::tokens::{Colors, Rgb, DARK, LIGHT};

const COLOUR_TOLERANCE: i32 = 1;
const BOX_TOLERANCE: f32 = 2.0;
pub const WIDTH: u32 = 1000;
pub const HEIGHT: u32 = 700;
/// The same, as logical pixels for a window.
pub const WIDTH_PX: f32 = 1000.0;
pub const HEIGHT_PX: f32 = 700.0;

/// Where a sample is taken, from the measured boxes (so a different text
/// width, which moves a button's right edge, cannot put a point on glyphs).
enum Probe {
    /// The pixel at (x, y).
    At(f32, f32),
    /// A 1 px line at a fractional position: the best of the rows (or
    /// columns) next to it; a one-pixel position error is the geometry
    /// check's business, not a colour error.
    Line(f32, f32),
    /// Text ink: the pixel of the region closest to the token colour. Glyph
    /// antialiasing and subpixel colour fringes never reach the exact token
    /// colour; a pixel at full coverage does.
    Ink(f32, f32, f32, f32),
}

struct Sample {
    name: &'static str,
    probe: Probe,
    expected: fn(&Colors) -> Rgb,
}

fn flat(name: &'static str, at: (f32, f32), expected: fn(&Colors) -> Rgb) -> Sample {
    Sample {
        name,
        probe: Probe::At(at.0, at.1),
        expected,
    }
}

fn line(name: &'static str, at: (f32, f32), expected: fn(&Colors) -> Rgb) -> Sample {
    Sample {
        name,
        probe: Probe::Line(at.0, at.1),
        expected,
    }
}

fn ink(name: &'static str, r: Rect, expected: fn(&Colors) -> Rgb) -> Sample {
    Sample {
        name,
        probe: Probe::Ink(r.x, r.y, r.x + r.w, r.y + r.h),
        expected,
    }
}

fn centre(r: Rect) -> (f32, f32) {
    (r.x + r.w / 2.0, r.y + r.h / 2.0)
}

/// The 15 sample points. `None` when a box they hang off was not recorded.
fn samples(boxes: &BTreeMap<String, Rect>) -> Option<Vec<Sample>> {
    let b = |n: &str| boxes.get(n).copied();
    let (side, title, input, primary, secondary) = (
        b("sidebar")?,
        b("title")?,
        b("input")?,
        b("primary")?,
        b("secondary")?,
    );
    let (step1, run2) = (b("step1")?, b("run2")?);
    let inset = |r: Rect| Rect::new(r.x + 8.0, r.y + 4.0, r.w - 16.0, r.h - 8.0);
    Some(vec![
        flat("window ground", (600.0, 650.0), |c| c.bg),
        flat("sidebar fill", (side.x + 100.0, 650.0), |c| {
            c.surface_sunken
        }),
        flat("sidebar border", (side.x + side.w - 0.5, 650.0), |c| {
            c.border
        }),
        flat("current nav fill", (160.0, 30.0), |c| c.surface),
        flat("accent dot", (24.0, 31.0), |c| c.accent),
        flat(
            "input fill",
            (input.x + input.w - 40.0, centre(input).1),
            |c| c.surface,
        ),
        line("input border", (centre(input).0, input.y + 0.5), |c| {
            c.border
        }),
        flat(
            "primary button fill",
            (primary.x + 5.0, centre(primary).1),
            |c| c.accent,
        ),
        ink("primary label", inset(primary), |c| c.accent_text),
        flat(
            "secondary button fill",
            (secondary.x + 5.0, centre(secondary).1),
            |c| c.surface,
        ),
        line(
            "secondary button border",
            (centre(secondary).0, secondary.y + 0.5),
            |c| c.border,
        ),
        flat(
            "disabled button (55%)",
            (run2.x + 5.0, centre(run2).1),
            Rgb::disabled_button,
        ),
        line("row rule", (500.0, step1.y + step1.h - 0.5), |c| c.border),
        flat("step number fill", (219.0, step1.y + 33.0), |c| {
            c.surface_sunken
        }),
        ink(
            "muted text",
            Rect::new(input.x, input.y + input.h + 8.0, 114.0, 22.0),
            |c| c.text_muted,
        ),
        ink("text", Rect::new(title.x, title.y, 84.0, title.h), |c| {
            c.text
        }),
    ])
}

impl Rgb {
    /// The disabled button: `--surface-sunken` at 55% over the page ground.
    fn disabled_button(c: &Colors) -> Rgb {
        c.surface_sunken.over(0.55, c.bg)
    }
}

fn px(img: &RgbaImage, x: u32, y: u32) -> Rgb {
    let Rgba(ch) = *img.get_pixel(x.min(img.width() - 1), y.min(img.height() - 1));
    Rgb(ch[0], ch[1], ch[2])
}

fn dist(a: Rgb, b: Rgb) -> i32 {
    (i32::from(a.0) - i32::from(b.0))
        .abs()
        .max((i32::from(a.1) - i32::from(b.1)).abs())
        .max((i32::from(a.2) - i32::from(b.2)).abs())
}

fn hex(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2)
}

/// Pixel index containing the coordinate, never negative.
fn at(v: f32) -> u32 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to the image"
    )]
    let n = v.max(0.0).floor() as u32;
    n
}

/// Outcome of one named check.
pub struct Line {
    pub ok: bool,
    pub text: String,
}

/// The result of checking one image.
pub struct Report {
    pub lines: Vec<Line>,
}

impl Report {
    #[must_use]
    pub fn failures(&self) -> usize {
        self.lines.iter().filter(|l| !l.ok).count()
    }

    #[must_use]
    pub fn text(&self) -> String {
        let mut s = String::new();
        for l in &self.lines {
            let _ = writeln!(s, "{}", l.text);
        }
        s
    }
}

/// Colour samples of `img` against `colors`, at points taken from `boxes`.
#[must_use]
pub fn check_colours(img: &RgbaImage, colors: &Colors, boxes: &BTreeMap<String, Rect>) -> Report {
    let mut lines = Vec::new();
    if img.width() != WIDTH || img.height() != HEIGHT {
        lines.push(Line {
            ok: false,
            text: format!(
                "FAIL image size {}x{}, expected {WIDTH}x{HEIGHT} (display scale is not 1?)",
                img.width(),
                img.height()
            ),
        });
        return Report { lines };
    }
    let Some(all) = samples(boxes) else {
        lines.push(Line {
            ok: false,
            text: "FAIL a box the sample points hang off was not recorded".into(),
        });
        return Report { lines };
    };
    for s in all {
        let want = (s.expected)(colors);
        let (got, place) = match s.probe {
            Probe::At(x, y) => (px(img, at(x), at(y)), format!("({},{})", at(x), at(y))),
            Probe::Line(x, y) => {
                let (px_x, row) = (at(x), at(y));
                let best = [row.saturating_sub(1), row, row + 1]
                    .into_iter()
                    .map(|r| px(img, px_x, r))
                    .min_by_key(|p| dist(*p, want))
                    .unwrap_or(want);
                (
                    best,
                    format!(
                        "({px_x},{}..{}) line, best of 3 rows",
                        row.saturating_sub(1),
                        row + 1
                    ),
                )
            }
            Probe::Ink(x0, y0, x1, y1) => {
                let mut best = (px(img, at(x0), at(y0)), at(x0), at(y0));
                for y in at(y0)..at(y1) {
                    for x in at(x0)..at(x1) {
                        let p = px(img, x, y);
                        if dist(p, want) < dist(best.0, want) {
                            best = (p, x, y);
                        }
                    }
                }
                (
                    best.0,
                    format!("({},{}) closest ink pixel of region", best.1, best.2),
                )
            }
        };
        let ok = dist(got, want) <= COLOUR_TOLERANCE;
        lines.push(Line {
            ok,
            text: format!(
                "{} colour {:<24} at {place}: want {} got {}",
                if ok { "PASS" } else { "FAIL" },
                s.name,
                hex(want),
                hex(got)
            ),
        });
    }
    Report { lines }
}

/// Reference boxes by pixel inspection of `pipeline-chosen.png` (x, y, w, h);
/// `true` marks a value that depends on text width, which may differ.
pub const REFERENCE_BOXES: &[(&str, [(f32, bool); 4])] = &[
    (
        "sidebar",
        [(0.0, false), (0.0, false), (192.0, false), (700.0, false)],
    ),
    (
        "title",
        [(216.0, false), (16.0, false), (760.0, false), (28.0, false)],
    ),
    (
        "input",
        [(216.0, false), (134.0, false), (661.0, true), (32.0, false)],
    ),
    (
        "browse",
        [(889.0, true), (135.0, false), (87.0, true), (29.0, false)],
    ),
    (
        "primary",
        [(216.0, false), (209.0, false), (225.0, true), (30.0, false)],
    ),
    (
        "secondary",
        [(453.0, true), (209.0, false), (82.0, true), (30.0, false)],
    ),
    (
        "run1",
        [(924.0, true), (274.0, false), (52.0, true), (30.0, false)],
    ),
    (
        "run2",
        [(912.0, true), (352.0, true), (64.0, true), (30.0, false)],
    ),
    (
        "step1",
        [(216.0, false), (256.0, false), (760.0, false), (67.0, true)],
    ),
    (
        "step2",
        [(216.0, false), (323.0, true), (760.0, false), (89.0, true)],
    ),
    (
        "step3",
        [(216.0, false), (412.0, true), (760.0, false), (89.0, true)],
    ),
    (
        "step4",
        [(216.0, false), (501.0, true), (760.0, false), (89.0, true)],
    ),
];

/// Parses the `name x y w h` lines `--shoot` writes next to the PNG.
#[must_use]
pub fn parse_boxes(text: &str) -> BTreeMap<String, Rect> {
    text.lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let name = it.next()?.to_string();
            let v: Vec<f32> = it.filter_map(|n| n.parse().ok()).collect();
            (v.len() == 4).then(|| (name, Rect::new(v[0], v[1], v[2], v[3])))
        })
        .collect()
}

/// Geometry of the recorded boxes against the reference image's.
#[must_use]
pub fn check_boxes(boxes: &BTreeMap<String, Rect>) -> Report {
    let mut lines = Vec::new();
    for (name, want) in REFERENCE_BOXES {
        let Some(got) = boxes.get(*name) else {
            lines.push(Line {
                ok: false,
                text: format!("FAIL box {name}: not recorded"),
            });
            continue;
        };
        let g = [got.x, got.y, got.w, got.h];
        for (i, what) in ["x", "y", "w", "h"].into_iter().enumerate() {
            let (r, text_dependent) = want[i];
            let within = (g[i] - r).abs() <= BOX_TOLERANCE;
            // A miss on a value that follows the text's width (or wrapping) is
            // the font's, which the human's similarity rule exempts: listed as
            // TEXT, not counted as a failure.
            let (ok, tag) = match (within, text_dependent) {
                (true, _) => (true, "PASS"),
                (false, true) => (true, "TEXT"),
                (false, false) => (false, "FAIL"),
            };
            lines.push(Line {
                ok,
                text: format!(
                    "{tag} box {name}.{what}: reference {r:.1} got {:.1} delta {:+.1}",
                    g[i],
                    g[i] - r
                ),
            });
        }
    }
    Report { lines }
}

/// reference | light | dark, 10 px apart on a grey ground.
#[must_use]
pub fn side_by_side(images: &[&RgbaImage]) -> RgbaImage {
    let gap = 10;
    let w: u32 = images.iter().map(|i| i.width()).sum::<u32>()
        + gap * (u32::try_from(images.len()).unwrap_or(1) - 1);
    let h = images.iter().map(|i| i.height()).max().unwrap_or(0);
    let mut out = RgbaImage::from_pixel(w, h, Rgba([128, 128, 128, 255]));
    let mut x = 0;
    for img in images {
        image::imageops::replace(&mut out, *img, i64::from(x), 0);
        x += img.width() + gap;
    }
    out
}

/// `fm-spike --diff <a.png> <b.png> <out.txt>`: how many pixels differ and by
/// how much; the process exits 0 when the images are identical.
#[must_use]
pub fn diff(a: &Path, b: &Path, out: &Path) -> i32 {
    let text = match (image::open(a), image::open(b)) {
        (Ok(a), Ok(b)) => {
            let (a, b) = (a.to_rgba8(), b.to_rgba8());
            if a.dimensions() == b.dimensions() {
                let mut count = 0_usize;
                let mut max = 0_u8;
                let mut first = None;
                for (x, y, pa) in a.enumerate_pixels() {
                    let pb = b.get_pixel(x, y);
                    if pa != pb {
                        count += 1;
                        first.get_or_insert((x, y));
                        for i in 0..4 {
                            max = max.max(pa.0[i].abs_diff(pb.0[i]));
                        }
                    }
                }
                format!(
                    "{count} differing pixel(s), max channel difference {max}, first at {first:?}\n"
                )
            } else {
                format!(
                    "size differs: {:?} vs {:?}\n",
                    a.dimensions(),
                    b.dimensions()
                )
            }
        }
        (Err(e), _) | (_, Err(e)) => format!("cannot read: {e}\n"),
    };
    let identical = text.starts_with("0 differing");
    let _ = std::fs::write(out, text);
    i32::from(!identical)
}

/// `fm-spike --verify <dir> <reference.png>`: checks `<dir>/pipeline-chosen.png`
/// (light) and `-dark.png`, writes `<dir>/check-result.txt` and
/// `<dir>/side-by-side.png`. Returns the process exit code.
#[must_use]
pub fn run(dir: &Path, reference: &Path) -> i32 {
    let mut out = String::new();
    let mut failures = 0;
    let mut shots: Vec<RgbaImage> = Vec::new();
    match image::open(reference) {
        Ok(r) => shots.push(r.to_rgba8()),
        Err(e) => {
            let _ = writeln!(
                out,
                "FAIL cannot read reference {}: {e}",
                reference.display()
            );
            failures += 1;
        }
    }
    for (file, colors, label) in [
        ("pipeline-chosen.png", &LIGHT, "light"),
        ("pipeline-chosen-dark.png", &DARK, "dark"),
    ] {
        let _ = writeln!(out, "== {label}: {file} ==");
        match image::open(dir.join(file)) {
            Ok(img) => {
                let img = img.to_rgba8();
                let boxes_file = dir.join(file.replace(".png", ".boxes.txt"));
                match std::fs::read_to_string(&boxes_file) {
                    Ok(t) => {
                        let boxes = parse_boxes(&t);
                        let rep = check_colours(&img, colors, &boxes);
                        failures += rep.failures();
                        out.push_str(&rep.text());
                        let rep = check_boxes(&boxes);
                        failures += rep.failures();
                        out.push_str(&rep.text());
                    }
                    Err(e) => {
                        failures += 1;
                        let _ = writeln!(out, "FAIL no boxes file {}: {e}", boxes_file.display());
                    }
                }
                shots.push(img);
            }
            Err(e) => {
                failures += 1;
                let _ = writeln!(out, "FAIL cannot read {file}: {e}");
            }
        }
    }
    if shots.len() == 3 {
        let refs: Vec<&RgbaImage> = shots.iter().collect();
        if let Err(e) = side_by_side(&refs).save(dir.join("side-by-side.png")) {
            let _ = writeln!(out, "FAIL side-by-side: {e}");
            failures += 1;
        }
    }
    let _ = writeln!(out, "{failures} failing check(s)");
    let _ = std::fs::write(dir.join("check-result.txt"), &out);
    i32::from(failures > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_boxes() -> BTreeMap<String, Rect> {
        REFERENCE_BOXES
            .iter()
            .map(|(n, v)| ((*n).to_string(), Rect::new(v[0].0, v[1].0, v[2].0, v[3].0)))
            .collect()
    }

    fn reference() -> RgbaImage {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/manual/images/pipeline-chosen.png");
        image::open(path).expect("reference image").to_rgba8()
    }

    /// The sample points are right: the reference image itself (Chromium,
    /// light) passes every colour check, so a miss on a GPUI render is the
    /// render's, not the checker's.
    #[test]
    fn colour_check_passes_on_the_reference_image() {
        let rep = check_colours(&reference(), &LIGHT, &reference_boxes());
        assert_eq!(rep.failures(), 0, "{}", rep.text());
        assert!(rep.lines.len() >= 12);
    }

    #[test]
    fn colour_check_fails_on_a_wrong_colour() {
        let mut img = reference();
        img.put_pixel(100, 650, Rgba([0xee, 0xf0, 0xf5, 255]));
        let rep = check_colours(&img, &LIGHT, &reference_boxes());
        assert_eq!(rep.failures(), 1, "{}", rep.text());
    }

    #[test]
    fn dark_tokens_are_not_the_light_image() {
        assert!(check_colours(&reference(), &DARK, &reference_boxes()).failures() > 8);
    }

    /// The hard-coded reference boxes are what the reference image shows.
    #[test]
    fn reference_boxes_match_the_reference_image() {
        let img = reference();
        let border = LIGHT.border;
        // Sidebar edge and the rules between steps.
        assert_eq!(px(&img, 191, 650), border);
        for y in [255, 322, 411, 500, 589] {
            assert_eq!(px(&img, 500, y), border, "rule at y={y}");
        }
        // Input: left, right, top, bottom border lines.
        assert_eq!(px(&img, 216, 150), border);
        assert_eq!(px(&img, 876, 150), border);
        assert_eq!(px(&img, 500, 134), border);
        assert_eq!(px(&img, 500, 165), border);
        // Primary: first and last fill column and row.
        assert_eq!(px(&img, 216, 224), LIGHT.accent);
        assert_eq!(px(&img, 440, 224), LIGHT.accent);
        assert_eq!(px(&img, 300, 209), LIGHT.accent);
        assert_eq!(px(&img, 300, 238), LIGHT.accent);
        // Secondary and Browse edges.
        assert_eq!(px(&img, 453, 224), border);
        assert_eq!(px(&img, 534, 224), border);
        assert_eq!(px(&img, 889, 150), border);
        assert_eq!(px(&img, 975, 150), border);
    }

    #[test]
    fn boxes_within_tolerance_pass_and_others_fail() {
        let mut boxes = reference_boxes();
        assert_eq!(check_boxes(&boxes).failures(), 0);
        boxes.get_mut("title").expect("title").w += 1.5;
        assert_eq!(check_boxes(&boxes).failures(), 0);
        boxes.get_mut("title").expect("title").w += 1.0;
        assert_eq!(check_boxes(&boxes).failures(), 1);
        // A text-width miss is listed but is not a failure.
        boxes.get_mut("title").expect("title").w = 760.0;
        boxes.get_mut("primary").expect("primary").w -= 40.0;
        let rep = check_boxes(&boxes);
        assert_eq!(rep.failures(), 0);
        assert!(rep.text().contains("TEXT box primary.w"));
    }

    #[test]
    fn diff_counts_differing_pixels() {
        let dir = tempfile::tempdir().expect("temp dir");
        let a = RgbaImage::from_pixel(4, 4, Rgba([10, 20, 30, 255]));
        let mut b = a.clone();
        a.save(dir.path().join("a.png")).expect("save a");
        b.save(dir.path().join("b.png")).expect("save b");
        let out = dir.path().join("out.txt");
        assert_eq!(
            diff(&dir.path().join("a.png"), &dir.path().join("b.png"), &out),
            0
        );
        b.put_pixel(1, 2, Rgba([13, 20, 30, 255]));
        b.save(dir.path().join("b.png")).expect("save b");
        assert_eq!(
            diff(&dir.path().join("a.png"), &dir.path().join("b.png"), &out),
            1
        );
        let text = std::fs::read_to_string(&out).expect("result");
        assert!(
            text.starts_with("1 differing pixel(s), max channel difference 3"),
            "{text}"
        );
    }

    #[test]
    fn side_by_side_is_three_images_wide() {
        let a = RgbaImage::new(1000, 700);
        let s = side_by_side(&[&a, &a, &a]);
        assert_eq!((s.width(), s.height()), (3020, 700));
    }
}
