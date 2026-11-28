//! The build's progress, in the build bar's action group: the phase label, a
//! 6 px track with an accent fill of the done fraction, and the percent.
//! There is no "progress unavailable" state: the model has none.

use gpui::prelude::*;
use gpui::{div, px, relative, App, Role};
use gpui_component::{h_flex, v_flex};

use crate::app::probe::Tag as _;
use crate::fonts::mono_family;
use crate::theme::palette;
use crate::theme::tokens::{FONT_SIZE_SM, RADIUS, REM, SPACE_1, SPACE_2};
use crate::tools::tarpack::model::TarpackModel;
use crate::tools::tarpack::text;
use crate::widgets::tabular;

/// The track's height.
const TRACK_HEIGHT: f32 = 6.0;
/// Room for "100%" in the 13 px face, so the bar does not move as the number
/// grows (`4ch`).
const PERCENT_WIDTH: f32 = 4.0 * 0.6 * FONT_SIZE_SM * REM;

/// The label above the bar: "Step 1 of 2 · Writing" with the file's name in
/// mono after it.
fn label(state: &TarpackModel, cx: &App) -> impl IntoElement {
    let full = state.progress_label();
    let file = state
        .progress_file()
        .and_then(|file| full.strip_suffix(file).map(|head| (head.to_string(), file)));
    let row = h_flex()
        .flex_wrap()
        .gap_x(px(SPACE_1))
        .min_w_0()
        .text_size(px(FONT_SIZE_SM * REM));
    match file {
        Some((head, file)) => row.child(div().child(head.trim_end().to_string())).child(
            div()
                .min_w_0()
                .font_family(mono_family(cx))
                .child(file.to_string()),
        ),
        None => row.child(div().child(full)),
    }
}

/// The progress of the running build.
pub fn progress(state: &TarpackModel, cx: &App) -> impl IntoElement {
    let p = palette(cx);
    let event = state.progress();
    let fraction = text::progress_fraction(event);
    let percent = text::progress_percent(event);
    v_flex()
        .id("tarpack-progress")
        .debug_selector(|| "tarpack-progress".to_string())
        .flex_grow(1.0)
        .flex_shrink(1.0)
        .flex_basis(px(12.0 * REM))
        .min_w_0()
        .gap(px(SPACE_1))
        .child(label(state, cx))
        .child(
            h_flex()
                .items_center()
                .gap(px(SPACE_2))
                .child(
                    div()
                        .id("tarpack-progress-track")
                        .debug_selector(|| "tarpack-progress-track".to_string())
                        .role(Role::ProgressIndicator)
                        .aria_label(text::BUILDING_ARCHIVE)
                        .aria_min_numeric_value(0.0)
                        .aria_max_numeric_value(100.0)
                        .aria_numeric_value(f64::from(percent))
                        .aria_value(state.progress_value_text())
                        .flex_1()
                        .h(px(TRACK_HEIGHT))
                        .overflow_hidden()
                        .bg(p.surface_sunken)
                        .rounded(px(RADIUS))
                        .child(
                            div()
                                .h_full()
                                .w(relative(fraction))
                                .bg(p.accent)
                                .rounded(px(RADIUS))
                                .debug_selector(|| "tarpack-progress-fill".to_string()),
                        ),
                )
                .child(tabular(
                    div()
                        .flex_none()
                        .min_w(px(PERCENT_WIDTH))
                        .text_right()
                        .text_size(px(FONT_SIZE_SM * REM))
                        .child(format!("{percent}%")),
                )),
        )
        .tag("tarpack-progress")
}
