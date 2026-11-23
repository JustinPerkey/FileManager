use std::collections::HashMap;
use std::path::PathBuf;

use gpui::{Hsla, TestAppContext, VisualTestContext, WindowAppearance};
use gpui_component::{ActiveTheme as _, ThemeMode};

use super::tokens::{self, Colors, Rgb, DARK, LIGHT};
use super::{apply, follow, hsla, hsla_a, observer_registrations, palette, set_override};

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// A text file of the repository with `\r\n` read as `\n`: a Windows checkout
/// may convert line endings, and the parsers below split on `\n`.
fn read_text(path: &std::path::Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
        .replace("\r\n", "\n")
}

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

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-6
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
    let (overlay, alpha) = rgba(&d["drop-overlay"]);
    assert_eq!(colors.drop_overlay.0, overlay);
    assert!(close(colors.drop_overlay.1, alpha));
    assert!(close(
        colors.shadow_overlay_alpha,
        shadow_alpha(&d["shadow-overlay"])
    ));
    assert!(close(colors.scrim_alpha, rgba(&d["scrim"]).1));
}

#[test]
fn tokens_match_tokens_css() {
    let path = manifest().join("../desktop/src/styles/tokens.css");
    if !path.exists() {
        // Cutover removed the Tauri app: tokens.rs is the only home.
        assert!(
            !manifest().join("../desktop").exists(),
            "apps/desktop exists but {} does not",
            path.display()
        );
        return;
    }
    let css = read_text(&path);
    let light = block(&css, ":root {");
    let dark = block(&css, ":root[data-theme=\"dark\"]");
    check(&LIGHT, &light);
    check(&DARK, &dark);
    // The `prefers-color-scheme: dark` block carries the same values.
    let media = block(&css, ":root:not([data-theme=\"light\"])");
    assert_eq!(
        media, dark,
        "media dark block differs from [data-theme=dark]"
    );
    // The dark block overrides every colour token the light block has.
    for k in light
        .keys()
        .filter(|k| light[*k].starts_with('#') || light[*k].starts_with("rgb("))
    {
        assert!(dark.contains_key(k), "dark lacks colour token {k}");
    }
    for (name, want) in [
        ("space-1", tokens::SPACE_1),
        ("space-2", tokens::SPACE_2),
        ("space-3", tokens::SPACE_3),
        ("space-4", tokens::SPACE_4),
        ("space-5", tokens::SPACE_5),
        ("space-6", tokens::SPACE_6),
        ("radius", tokens::RADIUS),
    ] {
        assert!(close(px(&light[name]), want), "{name}");
    }
    for (name, want) in [
        ("font-size-sm", tokens::FONT_SIZE_SM),
        ("font-size-md", tokens::FONT_SIZE_MD),
        ("font-size-lg", tokens::FONT_SIZE_LG),
        ("font-size-xl", tokens::FONT_SIZE_XL),
    ] {
        assert!(close(rem(&light[name]), want), "{name}");
    }
    assert_eq!(light["font-ui"], "\"Segoe UI\", system-ui, sans-serif");
    assert_eq!(tokens::FONT_UI, "Segoe UI");
    assert!(light["font-mono"].starts_with("\"Cascadia Mono\""));
    assert_eq!(tokens::FONT_MONO[0], "Cascadia Mono");
}

/// A `key: "value"` line of the front matter's section, by section name.
fn front_matter_section(md: &str, section: &str) -> HashMap<String, String> {
    let body = md.strip_prefix("---\n").expect("front matter");
    let body = &body[..body.find("\n---").expect("front matter end")];
    let mut out = HashMap::new();
    let mut inside = false;
    for line in body.lines() {
        if !line.starts_with(' ') {
            inside = line.trim_end() == format!("{section}:");
            continue;
        }
        if inside {
            if let Some((k, v)) = line.trim().split_once(':') {
                out.insert(
                    k.trim().trim_matches('"').to_string(),
                    v.trim().trim_matches('"').to_string(),
                );
            }
        }
    }
    out
}

#[test]
fn design_md_matches_tokens() {
    let md = read_text(&manifest().join("../../DESIGN.md"));
    let colors = front_matter_section(&md, "colors");
    for (suffix, set) in [("", &LIGHT), ("-dark", &DARK)] {
        let c = |name: &str| hex(&colors[&format!("{name}{suffix}")]);
        assert_eq!(set.bg, c("bg"), "bg{suffix}");
        assert_eq!(set.surface, c("surface"), "surface{suffix}");
        assert_eq!(set.surface_sunken, c("surface-sunken"), "sunken{suffix}");
        assert_eq!(set.border, c("border"), "border{suffix}");
        assert_eq!(set.text, c("text"), "text{suffix}");
        assert_eq!(set.text_muted, c("text-muted"), "muted{suffix}");
        assert_eq!(set.accent, c("accent"), "accent{suffix}");
        assert_eq!(set.accent_text, c("accent-text"), "accent-text{suffix}");
        assert_eq!(set.ok, c("ok"), "ok{suffix}");
        assert_eq!(set.warn, c("warn"), "warn{suffix}");
        assert_eq!(set.danger, c("danger"), "danger{suffix}");
        assert_eq!(set.focus_ring, c("focus-ring"), "focus-ring{suffix}");
    }
    let rounded = front_matter_section(&md, "rounded");
    assert!(close(px(&rounded["md"]), tokens::RADIUS));
    let spacing = front_matter_section(&md, "spacing");
    for (key, want) in [
        ("1", tokens::SPACE_1),
        ("2", tokens::SPACE_2),
        ("3", tokens::SPACE_3),
        ("4", tokens::SPACE_4),
        ("5", tokens::SPACE_5),
        ("6", tokens::SPACE_6),
    ] {
        assert!(close(px(&spacing[key]), want), "spacing {key}");
    }
}

/// WCAG relative luminance.
fn luminance(c: Rgb) -> f64 {
    let lin = |v: u8| {
        let s = f64::from(v) / 255.0;
        if s <= 0.039_28 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.0) + 0.7152 * lin(c.1) + 0.0722 * lin(c.2)
}

fn tokens_selection() -> f32 {
    super::SELECTION_ALPHA
}

fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

#[test]
fn contrast_pairs_meet_4_5() {
    for (name, t) in [("light", &LIGHT), ("dark", &DARK)] {
        let mut pairs = Vec::new();
        for fg in [("text", t.text), ("text-muted", t.text_muted)] {
            for bg in [
                ("bg", t.bg),
                ("surface", t.surface),
                ("surface-sunken", t.surface_sunken),
            ] {
                pairs.push((fg, bg));
            }
        }
        pairs.push((("accent-text", t.accent_text), ("accent", t.accent)));
        for fg in [("ok", t.ok), ("warn", t.warn), ("danger", t.danger)] {
            pairs.push((fg, ("surface", t.surface)));
        }
        pairs.push((("danger", t.danger), ("surface-sunken", t.surface_sunken)));
        // Derived fills, composited as `apply` and the widgets do.
        let on_fill = ("accent-text", t.accent_text);
        for (fname, fill) in [("ok", t.ok), ("warn", t.warn), ("danger", t.danger)] {
            pairs.push((on_fill, (fname, fill)));
        }
        pairs.push((on_fill, ("primary-hover", t.accent.over(0.88, t.text))));
        pairs.push((on_fill, ("danger-hover", t.danger.over(0.88, t.text))));
        let selection = |under: Rgb| t.accent.over(tokens_selection(), under);
        pairs.push((
            ("text", t.text),
            ("selection over surface", selection(t.surface)),
        ));
        pairs.push((("text", t.text), ("selection over bg", selection(t.bg))));
        for ((fname, fg), (bname, bg)) in pairs {
            let ratio = contrast(fg, bg);
            assert!(ratio >= 4.5, "{name}: {fname} on {bname} is {ratio:.2}");
        }
    }
}

#[test]
fn over_composites_like_a_browser() {
    // 55% of #5b616b over #eef0f2, per channel, rounded.
    let r = LIGHT.text_muted.over(0.55, LIGHT.surface_sunken);
    assert_eq!(r, Rgb(157, 161, 168));
    assert_eq!(LIGHT.accent.over(1.0, LIGHT.bg), LIGHT.accent);
    assert_eq!(LIGHT.accent.over(0.0, LIGHT.bg), LIGHT.bg);
}

fn setup(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
}

/// One row per slot `apply` sets: the slot's name, its value, the token it
/// must hold (computed here from `tokens.rs`, not from the `Palette`).
#[gpui::test]
fn apply_sets_every_slot(cx: &mut TestAppContext) {
    setup(cx);
    for (mode, t) in [(ThemeMode::Light, &LIGHT), (ThemeMode::Dark, &DARK)] {
        cx.update(|cx| apply(mode, cx));
        let name = if mode.is_dark() { "dark" } else { "light" };
        let hover = |c: Rgb| hsla(c.over(0.88, t.text));
        cx.read(|cx| {
            let th = cx.theme();
            let mut rows: Vec<(&str, Hsla, Hsla)> = Vec::new();
            macro_rules! slots {
                ($want:expr => $($slot:ident),+ $(,)?) => {
                    $(rows.push((stringify!($slot), th.$slot, $want));)+
                };
            }
            slots!(hsla(t.bg) => background, list_head);
            slots!(hsla(t.text) => foreground, caret, popover_foreground, accent_foreground,
                button_foreground, secondary_foreground, sidebar_foreground,
                sidebar_accent_foreground);
            slots!(hsla(t.border) => border, input, sidebar_border, table_row_border,
                window_border, title_bar_border, scrollbar_thumb);
            slots!(hsla(t.surface_sunken) => muted, accent, sidebar, button_hover,
                button_active, secondary_hover, secondary_active, list_hover, list_active,
                table_hover, table_active, scrollbar, title_bar);
            slots!(hsla(t.text_muted) => muted_foreground, table_head_foreground,
                scrollbar_thumb_hover);
            slots!(hsla(t.surface) => popover, button, secondary, sidebar_accent,
                list_even, table, table_even, table_head);
            rows.push(("colors.list", th.colors.list, hsla(t.surface)));
            slots!(hsla(t.accent) => primary, button_primary, sidebar_primary, link, info,
                progress_bar, drag_border, list_active_border, table_active_border);
            slots!(hsla(t.focus_ring) => ring);
            slots!(hsla_a(t.accent, 0.3) => selection);
            slots!(hover(t.accent) => primary_hover, primary_active, button_primary_hover,
                button_primary_active, link_hover, link_active);
            slots!(hsla(t.accent_text) => primary_foreground, button_primary_foreground,
                sidebar_primary_foreground, info_foreground, success_foreground,
                warning_foreground, danger_foreground, button_danger_foreground);
            slots!(hsla(t.danger) => danger, button_danger);
            slots!(hover(t.danger) => danger_hover, danger_active, button_danger_hover,
                button_danger_active);
            slots!(hsla(t.ok) => success);
            slots!(hsla(t.warn) => warning);
            slots!(hsla_a(Rgb(0, 0, 0), t.scrim_alpha) => overlay);
            slots!(hsla_a(t.drop_overlay.0, t.drop_overlay.1) => drop_target);
            for (slot, got, want) in rows {
                assert_eq!(got, want, "{name}: {slot}");
            }
            assert_eq!(th.is_dark(), mode.is_dark());
            assert_eq!(th.font_size, gpui::px(tokens::REM));
            assert_eq!(th.mono_font_size, gpui::px(14.0));
            assert_eq!(th.radius, gpui::px(tokens::RADIUS));
            assert_eq!(th.radius_lg, gpui::px(tokens::RADIUS));
            assert!(!th.shadow);
            assert_eq!(th.font_family.as_ref(), tokens::FONT_UI);
            assert_eq!(palette(cx).bg, hsla(t.bg));
        });
    }
}

fn is_dark(cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| cx.theme().is_dark())
}

#[gpui::test]
fn override_wins_over_appearance(cx: &mut TestAppContext) {
    setup(cx);
    let window = cx.add_window(|_, _| gpui::Empty);
    let vcx = &mut VisualTestContext::from_window(window.into(), cx);
    vcx.update(super::init);
    assert!(!is_dark(vcx));
    vcx.update(|_, cx| set_override(Some(ThemeMode::Dark), cx));
    assert!(is_dark(vcx));
    vcx.update(|_, cx| set_override(None, cx));
    assert!(!is_dark(vcx));
    // An override also beats a later appearance change.
    vcx.update(|_, cx| set_override(Some(ThemeMode::Light), cx));
    vcx.update(|_, cx| follow(WindowAppearance::Dark, cx));
    assert!(!is_dark(vcx));
    vcx.update(|_, cx| set_override(None, cx));
    assert!(is_dark(vcx));
}

#[gpui::test]
fn appearance_change_reapplies(cx: &mut TestAppContext) {
    setup(cx);
    let window = cx.add_window(|_, _| gpui::Empty);
    let vcx = &mut VisualTestContext::from_window(window.into(), cx);
    vcx.update(super::init);
    assert!(!is_dark(vcx));
    vcx.update(|_, cx| follow(WindowAppearance::Dark, cx));
    assert!(is_dark(vcx));
    assert_eq!(vcx.update(|_, cx| cx.theme().background), hsla(DARK.bg));
    vcx.update(|_, cx| follow(WindowAppearance::VibrantLight, cx));
    assert!(!is_dark(vcx));
    assert_eq!(vcx.update(|_, cx| cx.theme().background), hsla(LIGHT.bg));
}

#[gpui::test]
fn init_twice_registers_one_observer(cx: &mut TestAppContext) {
    setup(cx);
    let window = cx.add_window(|_, _| gpui::Empty);
    let vcx = &mut VisualTestContext::from_window(window.into(), cx);
    vcx.update(super::init);
    vcx.update(super::init);
    assert_eq!(vcx.update(|_, cx| observer_registrations(cx)), 1);
    assert!(!is_dark(vcx));
}

#[test]
fn crlf_checkouts_parse_the_same() {
    // A Windows runner may check the files out with CRLF line endings.
    let dir = tempfile::tempdir().expect("temp dir");
    let md = read_text(&manifest().join("../../DESIGN.md"));
    let path = dir.path().join("DESIGN.md");
    std::fs::write(&path, md.replace('\n', "\r\n")).expect("write");
    assert!(std::fs::read_to_string(&path)
        .unwrap()
        .starts_with("---\r\n"));
    let read = read_text(&path);
    assert_eq!(read, md);
    assert_eq!(
        front_matter_section(&read, "colors"),
        front_matter_section(&md, "colors")
    );
}
