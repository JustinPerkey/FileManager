//! The build result, between the table and the build bar: what the last build
//! came to. A success (clean or partial) lists what it holds; a failure says
//! why, with the technical Details closed. Copy buttons say "Copied" for a
//! moment; the view owns that, and whether Details and the warnings are open.

use fm_tarpack::archive::BuildSummary;
use fm_tarpack::session::{SessionEntry, TarpackError};
use gpui::prelude::*;
use gpui::{div, px, AnyElement, App, Context, Entity, FocusHandle, FontWeight, Role, Window};
use gpui_component::{h_flex, v_flex};

use super::report::report;
use super::TarpackView;
use crate::app::probe::Tag as _;
use crate::theme::palette;
use crate::theme::tokens::{
    FONT_SIZE_LG, FONT_SIZE_MD, FONT_SIZE_SM, RADIUS, REM, SPACE_1, SPACE_2, SPACE_3, SPACE_4,
};
use crate::tools::tarpack::model::{BuildOutcome, TarpackModel};
use crate::tools::tarpack::text;
use crate::widgets::text::mono;
use crate::widgets::{button, icon, tabular, Disclosure, IconName};

/// Which copy button was last used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyKind {
    Sha,
    Command,
    Report,
}

/// What the view remembers for the result.
#[derive(Clone, Copy, Debug, Default)]
pub struct ResultUi {
    pub copied: Option<CopyKind>,
    pub details_open: bool,
    pub warnings_open: bool,
}

/// The focus handles the result uses.
pub struct ResultFocus<'a> {
    /// The result's container: focus inside it is lost when it goes.
    pub result: &'a FocusHandle,
    /// The report's scroll region.
    pub report: &'a FocusHandle,
}

/// Room for "Copied" in the 13 px face.
const COPIED_WIDTH: f32 = 6.0 * 0.6 * FONT_SIZE_SM * REM;
/// What a code value asks for before it wraps (16rem).
const CODE_BASIS: f32 = 16.0 * REM;

/// What the three actions of the view's result do.
type Copier = fn(&mut TarpackModel, &mut Context<TarpackModel>);

struct Ctx<'a> {
    view: &'a Entity<TarpackView>,
    model: &'a Entity<TarpackModel>,
    ui: ResultUi,
}

/// A quiet "Copy" button and the "Copied" note after it.
fn copy_button(
    ctx: &Ctx,
    id: &'static str,
    name: &'static str,
    visible: &'static str,
    kind: CopyKind,
    copier: Copier,
    cx: &App,
) -> impl IntoElement {
    let p = palette(cx);
    let (model, view) = (ctx.model.clone(), ctx.view.clone());
    let shows_copied = ctx.ui.copied == Some(kind);
    h_flex()
        .items_center()
        .gap(px(SPACE_1))
        .child(
            button(id, visible)
                .quiet()
                .icon(IconName::Copy)
                .accessible_name(name)
                .debug_selector(id)
                .on_click(move |_, _, cx| {
                    model.update(cx, copier);
                    view.update(cx, |v, cx| v.show_copied(kind, cx));
                })
                .tag(id),
        )
        .child(
            div()
                .min_w(px(COPIED_WIDTH))
                .text_size(px(FONT_SIZE_SM * REM))
                .text_color(p.text_muted)
                .child(if shows_copied { text::COPIED } else { "" }),
        )
}

/// A small label above a block: 13 px, 600, muted.
fn block_label(label: &'static str, cx: &App) -> impl IntoElement {
    div()
        .text_size(px(FONT_SIZE_SM * REM))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(palette(cx).text_muted)
        .child(label)
}

/// The head: the tone's icon, the heading and the Dismiss button.
fn head(ctx: &Ctx, tone: (IconName, gpui::Hsla), heading: String) -> impl IntoElement {
    let model = ctx.model.clone();
    h_flex()
        .items_start()
        .gap(px(SPACE_2))
        .child(
            div()
                .flex_none()
                .pt(px(2.0))
                .child(icon(tone.0).text_color(tone.1)),
        )
        .child(
            div()
                .id("tarpack-build-result-heading")
                .role(Role::Heading)
                .aria_level(2)
                .flex_1()
                .min_w_0()
                .text_size(px(FONT_SIZE_LG * REM))
                .font_weight(FontWeight::SEMIBOLD)
                .child(heading),
        )
        .child(
            div().flex_none().child(
                button("tarpack-result-dismiss", text::RESULT_DISMISS)
                    .quiet()
                    .icon_only(IconName::X, text::RESULT_DISMISS)
                    .debug_selector("tarpack-result-dismiss")
                    .on_click(move |_, _, cx| model.update(cx, TarpackModel::dismiss_result))
                    .tag("tarpack-result-dismiss"),
            ),
        )
}

/// A line of mono entry names under a lede.
fn name_list(names: Vec<String>, label: &'static str) -> impl IntoElement {
    v_flex()
        .id(label)
        .role(Role::List)
        .aria_label(label)
        .pl(px(SPACE_4))
        .children(names.into_iter().map(|name| {
            h_flex()
                .gap(px(SPACE_2))
                .child("\u{2022}")
                .child(mono(name))
        }))
}

/// A value in mono next to its Copy button, wrapping under it when narrow.
fn code_row(code: AnyElement, copy: impl IntoElement) -> impl IntoElement {
    h_flex()
        .flex_wrap()
        .items_start()
        .gap(px(SPACE_2))
        .child(
            div()
                .flex_grow(1.0)
                .flex_shrink(1.0)
                .flex_basis(px(CODE_BASIS))
                .min_w_0()
                .child(code),
        )
        .child(copy)
}

/// The lines of the entries whose line endings were converted.
fn normalized(summary: &BuildSummary, entries: &[SessionEntry], cx: &App) -> Option<AnyElement> {
    if summary.normalized_entries.is_empty() {
        return None;
    }
    let small = px(FONT_SIZE_SM * REM);
    Some(
        v_flex()
            .gap(px(SPACE_1))
            .child(block_label(text::NORMALIZED_LABEL, cx))
            .child(
                v_flex()
                    .id("tarpack-normalized")
                    .role(Role::List)
                    .aria_label(text::NORMALIZED_LABEL)
                    .pl(px(SPACE_4))
                    .text_size(small)
                    .children(summary.normalized_entries.iter().map(|n| {
                        let source = text::source_of(&n.id, entries);
                        let line = text::normalized_line(source, n.crlf_replaced);
                        let rest = line.strip_prefix(source).unwrap_or(&line).to_string();
                        h_flex()
                            .flex_wrap()
                            .child("\u{2022}\u{a0}")
                            .child(mono(source.to_string()).text_size(small))
                            .child(rest)
                    })),
            )
            .into_any_element(),
    )
}

/// A build that wrote an archive.
#[expect(clippy::too_many_lines, reason = "one column of the result's parts")]
fn success(
    ctx: &Ctx,
    state: &TarpackModel,
    summary: &BuildSummary,
    entries: &[SessionEntry],
    focus: &ResultFocus,
    window: &Window,
    cx: &App,
) -> Vec<AnyElement> {
    let p = palette(cx);
    let small = px(FONT_SIZE_SM * REM);
    let mut parts: Vec<AnyElement> = Vec::new();
    let sources = |ids: &[String]| -> Vec<String> {
        ids.iter()
            .map(|id| text::source_of(id, entries).to_string())
            .collect()
    };

    if let Some(lede) = text::not_loaded_lede(summary) {
        parts.push(
            v_flex()
                .child(div().child(lede))
                .child(name_list(sources(&summary.not_loaded), "No file chosen"))
                .into_any_element(),
        );
    }
    if let Some(lede) = text::left_out_lede(summary) {
        parts.push(div().child(lede).into_any_element());
    }
    parts.push(
        div()
            .debug_selector(|| "tarpack-result-path".to_string())
            .child(mono(summary.path.clone()).text_size(small))
            .into_any_element(),
    );
    parts.push(
        tabular(
            div()
                .text_size(px(FONT_SIZE_MD * REM))
                .child(text::result_facts(summary)),
        )
        .into_any_element(),
    );
    parts.push(
        v_flex()
            .gap(px(SPACE_1))
            .child(block_label(text::SHA256_LABEL, cx))
            .child(code_row(
                div()
                    .debug_selector(|| "tarpack-result-sha".to_string())
                    .child(mono(summary.sha256_hex.clone()).text_size(small))
                    .into_any_element(),
                copy_button(
                    ctx,
                    "tarpack-copy-sha",
                    text::COPY_SHA256,
                    text::COPY,
                    CopyKind::Sha,
                    TarpackModel::copy_sha256,
                    cx,
                ),
            ))
            .into_any_element(),
    );
    if let Some(command) = state.extraction_command(cx) {
        parts.push(
            v_flex()
                .gap(px(SPACE_1))
                .child(block_label(text::EXTRACT_LABEL, cx))
                .child(code_row(
                    div()
                        .p(px(SPACE_2))
                        .bg(p.surface_sunken)
                        .rounded(px(RADIUS))
                        .debug_selector(|| "tarpack-result-command".to_string())
                        .child(mono(command.to_string()).text_size(small))
                        .into_any_element(),
                    copy_button(
                        ctx,
                        "tarpack-copy-command",
                        text::COPY_EXTRACTION_COMMAND,
                        text::COPY,
                        CopyKind::Command,
                        TarpackModel::copy_extraction_command,
                        cx,
                    ),
                ))
                .child(
                    div()
                        .text_size(small)
                        .text_color(p.text_muted)
                        .child(text::EXTRACT_HINT),
                )
                .into_any_element(),
        );
    }
    parts.extend(normalized(summary, entries, cx));
    let view = ctx.view.clone();
    parts.extend(report(
        summary,
        ctx.ui.warnings_open,
        move |open, _, cx| view.update(cx, |v, cx| v.set_build_warnings_open(open, cx)),
        focus.report,
        window,
        cx,
    ));
    let reveal = {
        let model = ctx.model.clone();
        button("tarpack-reveal", text::SHOW_IN_FOLDER)
            .icon(IconName::Folder)
            .debug_selector("tarpack-reveal")
            .on_click(move |_, _, cx| model.update(cx, TarpackModel::reveal_output))
            .tag("tarpack-reveal")
    };
    parts.push(
        h_flex()
            .flex_wrap()
            .items_center()
            .gap(px(SPACE_2))
            .child(reveal)
            .when(!text::result_is_clean(summary), |row| {
                row.child(copy_button(
                    ctx,
                    "tarpack-copy-report",
                    text::COPY_REPORT,
                    text::COPY_REPORT,
                    CopyKind::Report,
                    TarpackModel::copy_report,
                    cx,
                ))
            })
            .into_any_element(),
    );
    parts
}

/// A build that wrote nothing: the sentence is the heading, the message the
/// closed Details.
fn failure(ctx: &Ctx, message: String) -> Vec<AnyElement> {
    let view = ctx.view.clone();
    vec![Disclosure::new("tarpack-build-details", text::DETAILS)
        .debug_selector("tarpack-build-details-toggle")
        .open(ctx.ui.details_open)
        .on_toggle(move |open, _, cx| view.update(cx, |v, cx| v.set_build_details_open(open, cx)))
        .child(div().px(px(SPACE_2)).child(mono(message)))
        .into_any_element()]
}

/// The tone of a result's icon.
fn tone(outcome: &BuildOutcome, cx: &App) -> (IconName, gpui::Hsla) {
    let p = palette(cx);
    match outcome {
        BuildOutcome::Ok(s) if text::result_is_clean(s) => (IconName::CheckCircle, p.ok),
        BuildOutcome::Ok(_) => (IconName::AlertTriangle, p.warn),
        BuildOutcome::Err(_) | BuildOutcome::Unexpected { .. } => (IconName::XCircle, p.danger),
    }
}

/// The heading: the summary's, the error's sentence, or the unexpected one.
fn heading(outcome: &BuildOutcome, entries: &[SessionEntry]) -> String {
    match outcome {
        BuildOutcome::Ok(s) => text::result_heading(s),
        BuildOutcome::Err(e) => text::error_copy(e, entries),
        BuildOutcome::Unexpected { .. } => text::UNEXPECTED.to_string(),
    }
}

/// The technical message of a failure.
fn message(error: &TarpackError) -> String {
    error.message.clone()
}

/// The result box, or `None` when the last build left none. The outer element
/// holds the focus handle, so focus inside it can be told from focus
/// elsewhere.
pub fn build_result(
    view: &Entity<TarpackView>,
    model: &Entity<TarpackModel>,
    state: &TarpackModel,
    ui: ResultUi,
    focus: &ResultFocus,
    window: &Window,
    cx: &App,
) -> Option<AnyElement> {
    let outcome = state.result()?;
    let entries = state
        .session()
        .and_then(|s| s.manifest.as_ref())
        .map_or(&[][..], |m| &m.entries[..]);
    let ctx = Ctx { view, model, ui };
    let p = palette(cx);
    let heading = heading(outcome, entries);
    let body = match outcome {
        BuildOutcome::Ok(summary) => success(&ctx, state, summary, entries, focus, window, cx),
        BuildOutcome::Err(e) => failure(&ctx, message(e)),
        BuildOutcome::Unexpected { detail } => failure(&ctx, detail.clone()),
    };
    Some(
        div()
            .id("tarpack-build-result-focus")
            .track_focus(focus.result)
            .child(
                v_flex()
                    .id("tarpack-build-result-box")
                    .role(Role::Region)
                    .aria_label(heading.clone())
                    .gap(px(SPACE_2))
                    .p(px(SPACE_3))
                    .bg(p.surface)
                    .border_1()
                    .border_color(p.border)
                    .rounded(px(RADIUS))
                    .child(head(&ctx, tone(outcome, cx), heading))
                    .children(body)
                    .debug_selector(|| "tarpack-build-result".to_string())
                    .tag("tarpack-build-result"),
            )
            .into_any_element(),
    )
}
