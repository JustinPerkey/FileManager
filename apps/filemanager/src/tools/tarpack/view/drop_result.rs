//! The drop result and the "matching" line shown while a drop is matched.

use gpui::prelude::*;
use gpui::{div, px, AnyElement, App, Entity, Font, Hsla, Role, StyledText, TextRun, Window};
use gpui_component::{h_flex, v_flex};

use crate::app::probe::Tag as _;
use crate::fonts::mono_family;
use crate::theme::palette;
use crate::theme::tokens::{FONT_SIZE_SM, RADIUS, REM, SPACE_1, SPACE_2, SPACE_3, SPACE_5};
use crate::tools::tarpack::model::{DropReport, TarpackModel};
use crate::tools::tarpack::text::{self, DropLine, FullPaths, ItemNote, LineKind, Tone};
use crate::widgets::text::{mono, muted};
use crate::widgets::{button, icon, Disclosure, IconName};

/// A line of text with some runs in the mono face.
fn rich(window: &Window, cx: &App, parts: &[(String, bool)], color: Hsla) -> StyledText {
    let style = window.text_style();
    let base = style.font();
    let mono_font = Font {
        family: mono_family(cx),
        ..base.clone()
    };
    let mut content = String::new();
    let mut runs = Vec::new();
    for (part, is_mono) in parts {
        if part.is_empty() {
            continue;
        }
        content.push_str(part);
        runs.push(TextRun {
            len: part.len(),
            font: if *is_mono {
                mono_font.clone()
            } else {
                base.clone()
            },
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        });
    }
    StyledText::new(content).with_runs(runs)
}

/// The pieces of a line's text: count, lead, then the items (target paths in
/// mono), each with its note, and "and N more".
fn line_parts(l: &DropLine) -> Vec<(String, bool)> {
    let mut parts = Vec::new();
    if let Some(count) = l.count {
        parts.push((format!("{count} "), false));
    }
    parts.push((l.lead.clone(), false));
    if !l.items.is_empty() {
        parts.push((" ".to_string(), false));
    }
    for (i, item) in l.items.iter().enumerate() {
        parts.push((item.label.clone(), item.mono));
        match &item.note {
            Some(ItemNote::Count(n)) => parts.push((format!(" ({n} files)"), false)),
            Some(ItemNote::Text(t)) => parts.push((format!(" ({t})"), false)),
            None => {}
        }
        if i + 1 < l.items.len() {
            parts.push((", ".to_string(), false));
        }
    }
    if l.more > 0 {
        parts.push((format!(", and {} more", l.more), false));
    }
    parts
}

fn line(l: &DropLine, window: &Window, cx: &App) -> impl IntoElement {
    let p = palette(cx);
    let (name, tone) = match l.tone {
        Tone::Ok => (IconName::CheckCircle, p.ok),
        Tone::Warn => (IconName::AlertTriangle, p.warn),
        Tone::Muted => (IconName::Info, p.text_muted),
    };
    let quiet = matches!(l.kind, LineKind::Hint | LineKind::Empty);
    let color = if quiet { p.text_muted } else { p.text };
    h_flex()
        .items_start()
        .gap(px(SPACE_2))
        .child(
            div()
                .flex_none()
                .pt(px(2.0))
                .child(icon(name).text_color(tone)),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(rich(window, cx, &line_parts(l), color)),
        )
}

/// The "Full paths" content: every path behind the lines, in groups.
fn full_paths(lines: &[DropLine], cx: &App) -> Vec<AnyElement> {
    let p = palette(cx);
    lines
        .iter()
        .filter_map(|l| l.full.as_ref())
        .map(|full| match full {
            FullPaths::Paths { lead, paths } => v_flex()
                .child(
                    div()
                        .mt(px(SPACE_2))
                        .text_color(p.text_muted)
                        .child(lead.clone()),
                )
                .child(
                    v_flex().mt(px(SPACE_1)).pl(px(SPACE_5)).children(
                        paths
                            .iter()
                            .map(|path| mono(path.clone()).text_size(px(FONT_SIZE_SM * REM))),
                    ),
                )
                .into_any_element(),
            FullPaths::Entries { lead, entries } => v_flex()
                .child(
                    div()
                        .mt(px(SPACE_2))
                        .text_color(p.text_muted)
                        .child(lead.clone()),
                )
                .child(
                    v_flex()
                        .mt(px(SPACE_1))
                        .pl(px(SPACE_5))
                        .children(entries.iter().map(|e| {
                            v_flex()
                                .child(if e.mono {
                                    mono(e.label.clone())
                                        .text_size(px(FONT_SIZE_SM * REM))
                                        .into_any_element()
                                } else {
                                    div()
                                        .text_size(px(FONT_SIZE_SM * REM))
                                        .child(e.label.clone())
                                        .into_any_element()
                                })
                                .child(
                                    v_flex().pl(px(SPACE_5)).children(e.candidates.iter().map(
                                        |c| mono(c.clone()).text_size(px(FONT_SIZE_SM * REM)),
                                    )),
                                )
                        })),
                )
                .into_any_element(),
        })
        .collect()
}

/// The result of the last drop. `paths_open` is the view's state of the
/// "Full paths" disclosure.
pub fn drop_result(
    model: &Entity<TarpackModel>,
    report: &DropReport,
    paths_open: bool,
    on_paths_toggle: impl Fn(bool, &mut Window, &mut App) + 'static,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    let p = palette(cx);
    let groups = full_paths(&report.lines, cx);
    let dismiss = {
        let model = model.clone();
        button("tarpack-drop-dismiss", text::DISMISS_DROP_RESULT)
            .quiet()
            .icon_only(IconName::X, "Dismiss")
            .debug_selector("tarpack-drop-dismiss")
            .on_click(move |_, _, cx| model.update(cx, TarpackModel::dismiss_drop_result))
            .tag("tarpack-drop-dismiss")
    };
    h_flex()
        .items_start()
        .gap(px(SPACE_3))
        .px(px(SPACE_3))
        .py(px(SPACE_2))
        .bg(p.surface)
        .border_1()
        .border_color(p.border)
        .rounded(px(RADIUS))
        .id("tarpack-drop-result-box")
        .role(Role::Region)
        .aria_label(text::DROP_RESULT_LABEL)
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .child(
                    v_flex()
                        .gap(px(SPACE_1))
                        .children(report.lines.iter().map(|l| line(l, window, cx))),
                )
                .when(!groups.is_empty(), |col| {
                    col.child(
                        div()
                            .mt(px(SPACE_1))
                            .text_size(px(FONT_SIZE_SM * REM))
                            .child(
                                Disclosure::new("tarpack-drop-paths", text::FULL_PATHS)
                                    .debug_selector("tarpack-drop-paths")
                                    .open(paths_open)
                                    .on_toggle(on_paths_toggle)
                                    .child(v_flex().px(px(SPACE_2)).children(groups)),
                            ),
                    )
                }),
        )
        .child(div().flex_none().child(dismiss))
        .debug_selector(|| "tarpack-drop-result".to_string())
        .tag("tarpack-drop-result")
}

/// The line shown while a drop is being matched.
pub fn drop_pending(cx: &App) -> impl IntoElement {
    let p = palette(cx);
    h_flex()
        .items_start()
        .gap(px(SPACE_2))
        .px(px(SPACE_3))
        .py(px(SPACE_2))
        .bg(p.surface)
        .border_1()
        .border_color(p.border)
        .rounded(px(RADIUS))
        .child(
            div()
                .flex_none()
                .pt(px(2.0))
                .child(icon(IconName::Info).text_color(p.text_muted)),
        )
        .child(muted(text::MATCHING))
        .debug_selector(|| "tarpack-drop-result".to_string())
        .tag("tarpack-drop-result")
}
