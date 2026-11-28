//! The build bar: the last item of the view's column, full width, always in
//! view. Settings at the left (the output, the format and its notice), the
//! action at the right (why not, or the progress, then Show errors and Create
//! archive). The two groups wrap under each other when the window is narrow.

use fm_tarpack::session::TarpackSession;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, App, Entity, FocusHandle, SharedString};
use gpui_component::h_flex;

use super::progress;
use crate::app::probe::Tag as _;
use crate::theme::palette;
use crate::theme::tokens::{
    FONT_SIZE_SM, REM, SCROLLBAR_GUTTER, SPACE_1, SPACE_2, SPACE_3, SPACE_4, SPACE_5,
};
use crate::tools::tarpack::model::TarpackModel;
use crate::tools::tarpack::text;
use crate::widgets::{button, icon, select, IconName, MiddlePath};

/// What the groups ask for before they wrap (24rem, 16rem, 22 characters of
/// the 13 px mono face, 12rem).
const GROUP_BASIS: f32 = 24.0 * REM;
const OUTPUT_BASIS: f32 = 16.0 * REM;
const PATH_BASIS: f32 = 22.0 * 0.6 * FONT_SIZE_SM * REM;
const STATUS_BASIS: f32 = 12.0 * REM;
/// The format select: "zstd (.tar.zst)" and a chevron.
const FORMAT_WIDTH: f32 = 168.0;

/// The focus handles the view wants tracked on the bar's controls.
pub struct BarFocus<'a> {
    /// The first control after the drop result (focus hint `AfterResult`).
    pub first: &'a FocusHandle,
    /// Create archive (focus hints `CreateArchive*`).
    pub create: &'a FocusHandle,
}

/// The output's path, or "not chosen".
fn output_path(session: &TarpackSession, cx: &App) -> AnyElement {
    let p = palette(cx);
    let shown = div().text_size(px(FONT_SIZE_SM * REM));
    match &session.output_path {
        Some(path) => shown
            .child(MiddlePath::new(path.clone()).id("tarpack-output-path"))
            .into_any_element(),
        None => shown
            .text_color(p.text_muted)
            .child(text::OUTPUT_NOT_CHOSEN)
            .into_any_element(),
    }
}

/// "Output:", the path and Choose….
fn output_group(
    model: &Entity<TarpackModel>,
    state: &TarpackModel,
    session: &TarpackSession,
    choose_focus: Option<&FocusHandle>,
    cx: &App,
) -> impl IntoElement {
    let p = palette(cx);
    let choose = {
        let model = model.clone();
        let b = button("tarpack-choose-output", text::CHOOSE_OUTPUT)
            .icon(IconName::Folder)
            .disabled(session.manifest.is_none() || state.building())
            .debug_selector("tarpack-choose-output")
            .on_click(move |_, _, cx| model.update(cx, TarpackModel::choose_output));
        match choose_focus {
            Some(handle) => b.track_focus(handle),
            None => b,
        }
        .tag("tarpack-choose-output")
    };
    h_flex()
        .flex_wrap()
        .items_center()
        .gap(px(SPACE_2))
        .flex_grow(1.0)
        .flex_shrink(1.0)
        .flex_basis(px(OUTPUT_BASIS))
        .min_w_0()
        .child(
            div()
                .flex_none()
                .text_size(px(FONT_SIZE_SM * REM))
                .text_color(p.text_muted)
                .child(text::OUTPUT_LABEL),
        )
        .child(
            div()
                .flex_grow(1.0)
                .flex_shrink(1.0)
                .flex_basis(px(PATH_BASIS))
                .min_w_0()
                .child(output_path(session, cx)),
        )
        .child(choose)
}

/// "Format", the select and the notice.
fn format_group(
    model: &Entity<TarpackModel>,
    state: &TarpackModel,
    session: &TarpackSession,
    cx: &App,
) -> impl IntoElement {
    let p = palette(cx);
    let small = px(FONT_SIZE_SM * REM);
    let options = session
        .formats
        .iter()
        .map(|f| {
            (
                SharedString::from(text::format_option_label(f)),
                SharedString::from(text::format_name(f.format)),
            )
        })
        .collect();
    let formats = session.formats.clone();
    let format = {
        let model = model.clone();
        select(
            "tarpack-format-select",
            text::FORMAT_LABEL,
            options,
            Some(text::format_name(session.format).into()),
        )
        .width(px(FORMAT_WIDTH))
        .disabled(session.manifest.is_none() || state.building())
        .on_pick(move |value, _, cx| {
            if let Some(option) = formats
                .iter()
                .find(|f| text::format_name(f.format) == value.as_ref())
            {
                let format = option.format;
                model.update(cx, |m, cx| m.set_format(format, cx));
            }
        })
    };
    h_flex()
        .flex_wrap()
        .items_center()
        .gap(px(SPACE_2))
        .child(
            div()
                .text_size(small)
                .text_color(p.text_muted)
                .child(text::FORMAT_LABEL),
        )
        .child(
            div()
                .debug_selector(|| "tarpack-format".to_string())
                .child(format)
                .tag("tarpack-format"),
        )
        .children(state.format_notice().map(|notice| {
            div()
                .id("tarpack-format-notice")
                .text_size(small)
                .child(notice.to_string())
        }))
}

/// The settings group: the output and the format.
fn settings(
    model: &Entity<TarpackModel>,
    state: &TarpackModel,
    session: &TarpackSession,
    choose_focus: Option<&FocusHandle>,
    cx: &App,
) -> impl IntoElement {
    h_flex()
        .flex_wrap()
        .items_center()
        .gap_x(px(SPACE_4))
        .gap_y(px(SPACE_2))
        .flex_grow(1.0)
        .flex_shrink(1.0)
        .flex_basis(px(GROUP_BASIS))
        .min_w_0()
        .child(output_group(model, state, session, choose_focus, cx))
        .child(format_group(model, state, session, cx))
}

/// Why Create archive is off, and the note about what a build leaves out.
fn status_text(state: &TarpackModel, cx: &App) -> impl IntoElement {
    let p = palette(cx);
    let small = px(FONT_SIZE_SM * REM);
    let reason = state.build_reason();
    let note = state.left_out_note();
    div()
        .id("tarpack-build-status")
        .flex_grow(1.0)
        .flex_shrink(1.0)
        .flex_basis(px(STATUS_BASIS))
        .min_w_0()
        .text_right()
        .text_size(small)
        .children(reason.map(|reason| {
            div()
                .id("tarpack-build-reason")
                .debug_selector(|| "tarpack-build-reason".to_string())
                .text_color(p.text_muted)
                .child(reason)
        }))
        .children(note.map(|note| {
            div()
                .id("tarpack-build-note")
                .debug_selector(|| "tarpack-build-note".to_string())
                .child(
                    h_flex()
                        .justify_end()
                        .items_start()
                        .gap(px(SPACE_1))
                        .child(
                            div()
                                .flex_none()
                                .pt(px(2.0))
                                .child(icon(IconName::AlertTriangle).text_color(p.warn)),
                        )
                        .child(div().min_w_0().child(note)),
                )
        }))
}

/// The action group: the status or the progress, Show errors, Create archive.
fn action(
    model: &Entity<TarpackModel>,
    state: &TarpackModel,
    session: &TarpackSession,
    show_errors_focus: Option<&FocusHandle>,
    create: &FocusHandle,
    cx: &App,
) -> impl IntoElement {
    let building = state.building();
    let errors = session
        .manifest
        .as_ref()
        .is_some_and(|m| m.error_count > 0)
        .then(|| {
            let model = model.clone();
            let b = button("tarpack-show-errors", text::SHOW_ERRORS)
                .shortcut("F8")
                .disabled(building)
                .debug_selector("tarpack-show-errors")
                .on_click(move |_, _, cx| model.update(cx, TarpackModel::show_errors));
            match show_errors_focus {
                Some(handle) => b.track_focus(handle),
                None => b,
            }
            .tag("tarpack-show-errors")
        });
    let create_button = {
        let model = model.clone();
        button("tarpack-build", text::CREATE_ARCHIVE)
            .primary()
            .busy(building)
            .disabled(!state.can_build())
            .shortcut("Ctrl+Enter")
            .track_focus(create)
            .debug_selector("tarpack-build")
            .on_click(move |_, _, cx| model.update(cx, TarpackModel::build))
            .tag("tarpack-build")
    };
    h_flex()
        .flex_wrap()
        .items_center()
        .justify_end()
        .gap_x(px(SPACE_3))
        .gap_y(px(SPACE_2))
        .flex_grow(1.0)
        .flex_shrink(1.0)
        .flex_basis(px(GROUP_BASIS))
        .min_w_0()
        .child(if building {
            progress::progress(state, cx).into_any_element()
        } else {
            status_text(state, cx).into_any_element()
        })
        .children(errors)
        .child(create_button)
}

/// The bar. `None` until a session is loaded. It cancels the view's side
/// padding (and the scrollbar gutter's room) and its bottom padding, so its
/// rule runs edge to edge and its contents stay where the page's are.
pub fn build_bar(
    model: &Entity<TarpackModel>,
    state: &TarpackModel,
    focus: &BarFocus,
    cx: &App,
) -> Option<AnyElement> {
    let session = state.session()?;
    let p = palette(cx);
    let building = state.building();
    let choose_enabled = session.manifest.is_some() && !building;
    let errors_enabled = session.manifest.as_ref().is_some_and(|m| m.error_count > 0) && !building;
    // The first control that can take focus holds the `AfterResult` handle.
    let choose_focus = choose_enabled.then_some(focus.first);
    let show_errors_focus = (!choose_enabled && errors_enabled).then_some(focus.first);
    Some(
        h_flex()
            .id("tarpack-build-bar")
            .flex_none()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_x(px(SPACE_4))
            .gap_y(px(SPACE_2))
            .ml(px(-SPACE_5))
            .mr(px(-(SPACE_5 + SCROLLBAR_GUTTER)))
            .mb(px(-SPACE_4))
            .pl(px(SPACE_5))
            .pr(px(SPACE_5 + SCROLLBAR_GUTTER))
            .py(px(SPACE_3))
            .bg(p.surface)
            .border_t_1()
            .border_color(p.border)
            .child(settings(model, state, session, choose_focus, cx))
            .child(action(
                model,
                state,
                session,
                show_errors_focus,
                focus.create,
                cx,
            ))
            .debug_selector(|| "tarpack-build-bar".to_string())
            .tag("tarpack-build-bar")
            .into_any_element(),
    )
}
