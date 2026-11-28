//! The build report inside the build result: the entries left out, the
//! manifest errors and the warnings, under a rule, scrolling inside itself at
//! 40% of the view's height.

use fm_tarpack::archive::BuildSummary;
use gpui::prelude::*;
use gpui::{div, px, transparent_black, AnyElement, App, FocusHandle, Pixels, Role, Window};
use gpui_component::v_flex;

use super::errors::{diagnostic_list, failure, group_heading};
use crate::theme::palette;
use crate::theme::tokens::{RADIUS, SPACE_1, SPACE_2};
use crate::tools::tarpack::text;
use crate::widgets::Disclosure;

/// Share of the view's height at which the report scrolls.
const HEIGHT_SHARE: f32 = 0.4;

/// A group's title: 14 px, 600.
fn title(text: String) -> gpui::Div {
    group_heading("").child(text)
}

/// The report of `summary`; `None` when it has nothing to report. The region
/// is a tab stop, which is what makes it scrollable by keys.
pub fn report(
    summary: &BuildSummary,
    warnings_open: bool,
    on_warnings_toggle: impl Fn(bool, &mut Window, &mut App) + 'static,
    focus: &FocusHandle,
    window: &Window,
    cx: &App,
) -> Option<AnyElement> {
    if summary.left_out.is_empty()
        && summary.manifest_errors.is_empty()
        && summary.warnings.is_empty()
    {
        return None;
    }
    let p = palette(cx);
    let max_height: Pixels = window.viewport_size().height * HEIGHT_SHARE;
    let mut groups: Vec<AnyElement> = Vec::new();
    if !summary.left_out.is_empty() {
        groups.push(
            v_flex()
                .debug_selector(|| "tarpack-report-left-out".to_string())
                .child(title(text::report_left_out_heading(summary.left_out.len())).mb(px(SPACE_1)))
                .children(summary.left_out.iter().map(|f| failure(f, cx)))
                .into_any_element(),
        );
    }
    if !summary.manifest_errors.is_empty() {
        groups.push(
            v_flex()
                .debug_selector(|| "tarpack-report-errors".to_string())
                .child(
                    title(text::report_errors_heading(summary.manifest_errors.len()))
                        .mb(px(SPACE_1)),
                )
                .child(diagnostic_list(
                    &summary.manifest_errors,
                    text::MANIFEST_ERRORS_LABEL,
                    cx,
                ))
                .into_any_element(),
        );
    }
    if !summary.warnings.is_empty() {
        groups.push(
            Disclosure::new(
                "tarpack-build-warnings",
                text::report_warnings_heading(summary.warnings.len()),
            )
            .debug_selector("tarpack-build-warnings-toggle")
            .open(warnings_open)
            .on_toggle(on_warnings_toggle)
            .child(div().px(px(SPACE_2)).child(diagnostic_list(
                &summary.warnings,
                text::WARNINGS_LABEL,
                cx,
            )))
            .into_any_element(),
        );
    }
    Some(
        div()
            .mt(px(SPACE_1))
            .pt(px(SPACE_2))
            .border_t_1()
            .border_color(p.border)
            .child(
                v_flex()
                    .id("tarpack-build-report")
                    .debug_selector(|| "tarpack-build-report".to_string())
                    .role(Role::Region)
                    .aria_label(text::REPORT_LABEL)
                    .track_focus(focus)
                    .tab_stop(true)
                    .gap(px(SPACE_2))
                    .max_h(max_height)
                    .overflow_y_scroll()
                    .border_1()
                    .border_color(transparent_black())
                    .rounded(px(RADIUS))
                    .focus_visible(|s| s.border_color(p.focus_ring))
                    .children(groups),
            )
            .into_any_element(),
    )
}
