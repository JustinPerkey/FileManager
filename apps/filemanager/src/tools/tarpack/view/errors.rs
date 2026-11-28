//! The manifest errors region, the warnings disclosure and the diagnostic
//! lists they share.

use fm_tarpack::manifest::{Diagnostic, EntryFailure};
use fm_tarpack::session::SessionManifest;
use gpui::prelude::*;
use gpui::{
    div, px, transparent_black, AnyElement, App, Entity, FocusHandle, Pixels, Role, Window,
};
use gpui_component::{h_flex, v_flex};

use crate::app::probe::Tag as _;
use crate::fonts::mono_family;
use crate::theme::palette;
use crate::theme::tokens::{FONT_SIZE_MD, FONT_SIZE_SM, RADIUS, REM, SPACE_1, SPACE_2, SPACE_3};
use crate::tools::tarpack::model::TarpackModel;
use crate::tools::tarpack::text;
use crate::widgets::text::{body, h2, mono};
use crate::widgets::{button, icon, Disclosure, IconName};

/// The least width the heading gets before the toggle wraps under it (16rem).
const TEXT_BASIS: f32 = 256.0;
/// Share of the view's height at which the report scrolls.
const REPORT_HEIGHT_SHARE: f32 = 0.4;
/// The height of a button (`controls.css`).
const CONTROL_HEIGHT: f32 = 30.0;
/// The position column: 5 characters of the mono face, as no-break spaces.
const POSITION_WIDTH: usize = 5;

/// One diagnostic: its position (mono, muted) and its message.
fn diagnostic_row(d: &Diagnostic, cx: &App) -> impl IntoElement {
    let p = palette(cx);
    let position = format!(
        "{:\u{a0}<width$}",
        text::diagnostic_position(d),
        width = POSITION_WIDTH
    );
    h_flex()
        .items_start()
        .gap(px(SPACE_2))
        .py(px(2.0))
        .text_size(px(FONT_SIZE_MD * REM))
        .child(
            div()
                .flex_none()
                .whitespace_nowrap()
                .font_family(mono_family(cx))
                .text_color(p.text_muted)
                .child(position),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_color(p.text)
                .child(d.message.clone()),
        )
}

/// A list of diagnostics, labelled for assistive technology.
pub(super) fn diagnostic_list(
    diagnostics: &[Diagnostic],
    label: impl Into<gpui::SharedString>,
    cx: &App,
) -> impl IntoElement {
    let label = label.into();
    v_flex()
        .id(label.clone())
        .role(Role::List)
        .aria_label(label)
        .children(diagnostics.iter().map(|d| diagnostic_row(d, cx)))
}

/// A group heading: 14 px, 600.
pub(super) fn group_heading(label: &'static str) -> gpui::Div {
    div()
        .text_size(px(FONT_SIZE_MD * REM))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .child(label)
}

/// One `[[file]]` table with errors: its name, where it is, its diagnostics.
pub(super) fn failure(f: &EntryFailure, cx: &App) -> impl IntoElement {
    let p = palette(cx);
    let name: AnyElement = match &f.id {
        Some(id) => mono(id.clone())
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .into_any_element(),
        None => group_heading("")
            .child(format!("Entry #{}", f.index))
            .into_any_element(),
    };
    let mut where_line = h_flex()
        .flex_wrap()
        .gap_x(px(SPACE_1))
        .text_size(px(FONT_SIZE_SM * REM))
        .text_color(p.text_muted);
    if let Some(source) = &f.source {
        where_line = where_line
            .child(text::FAILURE_SOURCE)
            .child(div().font_family(mono_family(cx)).child(source.clone()))
            .child("\u{b7}");
    }
    where_line = where_line
        .child(
            div()
                .font_family(mono_family(cx))
                .child(text::FAILURE_TABLE),
        )
        .child(format!("{} {}", text::FAILURE_ON_LINE, f.line));
    v_flex()
        .mb(px(SPACE_3))
        .child(name)
        .child(div().mb(px(SPACE_1)).child(where_line))
        .child(diagnostic_list(
            &f.errors,
            text::failure_errors_label(f),
            cx,
        ))
}

/// The report: whole-manifest errors, then the files with errors.
fn report(manifest: &SessionManifest, cx: &App) -> impl IntoElement {
    v_flex()
        .when(!manifest.errors.is_empty(), |col| {
            col.child(
                v_flex()
                    .child(group_heading(text::WHOLE_MANIFEST).mb(px(SPACE_1)))
                    .child(diagnostic_list(
                        &manifest.errors,
                        text::WHOLE_MANIFEST_ERRORS,
                        cx,
                    )),
            )
        })
        .when(!manifest.failed_entries.is_empty(), |col| {
            col.child(
                v_flex()
                    .when(!manifest.errors.is_empty(), |s| s.mt(px(SPACE_2)))
                    .child(group_heading(text::FILES_WITH_ERRORS).mb(px(SPACE_1)))
                    .children(manifest.failed_entries.iter().map(|f| failure(f, cx))),
            )
        })
}

/// The errors region: a danger-outlined panel with the count, what it means,
/// and the Show or Hide toggle; expanded, the report under a rule. `None`
/// when the manifest has no errors.
pub fn errors_region(
    model: &Entity<TarpackModel>,
    manifest: &SessionManifest,
    expanded: bool,
    report_focus: &FocusHandle,
    window: &Window,
    cx: &App,
) -> Option<AnyElement> {
    if manifest.error_count == 0 {
        return None;
    }
    let p = palette(cx);
    let toggle = {
        let model = model.clone();
        button("tarpack-errors-toggle", text::errors_toggle(expanded))
            .icon(IconName::ChevronDown)
            .icon_flip(expanded)
            .debug_selector("tarpack-errors-toggle")
            .on_click(move |_, _, cx| {
                model.update(cx, |m, cx| m.set_errors_expanded(!expanded, cx));
            })
            .tag("tarpack-errors-toggle")
    };
    let max_height: Pixels = window.viewport_size().height * REPORT_HEIGHT_SHARE;
    let notice = h_flex()
        .flex_wrap()
        .items_center()
        .gap_x(px(SPACE_3))
        .gap_y(px(SPACE_2))
        .child(icon(IconName::XCircle).text_color(p.danger))
        .child(
            v_flex()
                .flex_grow(1.0)
                .flex_shrink(1.0)
                .flex_basis(px(TEXT_BASIS))
                .min_w_0()
                .gap(px(SPACE_1))
                .child(h2(text::errors_heading(manifest.error_count)))
                .child(body(text::consequence(manifest))),
        )
        .child(toggle);
    let region = v_flex()
        .bg(p.surface)
        .border_1()
        .border_color(p.danger)
        .rounded(px(RADIUS))
        .px(px(SPACE_3))
        .py(px(SPACE_2))
        .child(notice)
        .when(expanded, |region| {
            region
                .child(div().mt(px(SPACE_2)).h(px(1.0)).bg(p.border))
                .child(
                    // The report may scroll, so it is a tab stop: that is what
                    // makes it scrollable by keys.
                    div()
                        .id("tarpack-errors-report")
                        .debug_selector(|| "tarpack-errors-report".to_string())
                        .role(Role::Region)
                        .aria_label(text::errors_heading(manifest.error_count))
                        .track_focus(report_focus)
                        .tab_stop(true)
                        .mt(px(SPACE_2))
                        .max_h(max_height)
                        .overflow_y_scroll()
                        .border_1()
                        .border_color(transparent_black())
                        .rounded(px(RADIUS))
                        .focus_visible(|s| s.border_color(p.focus_ring))
                        .child(report(manifest, cx)),
                )
        })
        .debug_selector(|| "tarpack-errors".to_string())
        .tag("tarpack-errors");
    Some(region.into_any_element())
}

/// The warnings: a bordered disclosure with the warn icon before its summary.
/// `None` when the manifest has none.
pub fn warnings(
    manifest: &SessionManifest,
    open: bool,
    on_toggle: impl Fn(bool, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Option<AnyElement> {
    if manifest.warnings.is_empty() {
        return None;
    }
    let p = palette(cx);
    Some(
        v_flex()
            .bg(p.surface)
            .border_1()
            .border_color(p.border)
            .rounded(px(RADIUS))
            .px(px(SPACE_3))
            .py(px(SPACE_2))
            .child(
                h_flex()
                    .items_start()
                    .gap(px(SPACE_1))
                    .child(
                        // As tall as the toggle, so the icon sits on its line.
                        h_flex()
                            .h(px(CONTROL_HEIGHT))
                            .items_center()
                            .child(icon(IconName::AlertTriangle).text_color(p.warn)),
                    )
                    .child(
                        Disclosure::new(
                            "tarpack-warnings",
                            text::warnings_summary(manifest.warnings.len()),
                        )
                        .debug_selector("tarpack-warnings-toggle")
                        .open(open)
                        .on_toggle(on_toggle)
                        .child(div().px(px(SPACE_2)).child(
                            diagnostic_list(&manifest.warnings, text::WARNINGS_LABEL, cx),
                        )),
                    ),
            )
            .into_any_element(),
    )
}
