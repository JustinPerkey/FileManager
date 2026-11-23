//! The steps: one row each, ruled above and below, with the number, title,
//! description, status and Run button, and under it the step's result.

use fm_pipeline::session::{PipelineErrorKind, PipelineSession, StepView};
use gpui::prelude::*;
use gpui::{div, px, Context, ElementId, Role, SharedString};
use gpui_component::{h_flex, v_flex};

use super::{index, PipelineView};
use crate::app::probe::Tag as _;
use crate::theme::tokens::{RADIUS, SPACE_1, SPACE_2, SPACE_3, SPACE_4, SPACE_5};
use crate::theme::Palette;
use crate::tools::pipeline::model::{PipelineModel, StepStatus};
use crate::tools::pipeline::text;
use crate::widgets::{
    body, button, h2, mono, mono_small_muted, muted, small_muted, status, step_number, Disclosure,
    IconName, StatusTone,
};

/// The least width of the status column (9rem).
const STATUS_WIDTH: f32 = 144.0;
/// The least width of the title and description before the status wraps (16rem).
const TEXT_BASIS: f32 = 256.0;
/// Width of the focus ring around a row.
const RING: f32 = 2.0;

/// The tone and icon of a status.
fn tone(status: StepStatus) -> (StatusTone, IconName) {
    match status {
        StepStatus::Passed => (StatusTone::Ok, IconName::CheckCircle),
        StepStatus::Problems(_) => (StatusTone::Warn, IconName::AlertTriangle),
        StepStatus::Failed | StepStatus::NotImplemented => (StatusTone::Danger, IconName::XCircle),
        StepStatus::Running | StepStatus::Waiting | StepStatus::NotRun => {
            (StatusTone::Muted, IconName::Circle)
        }
    }
}

pub(super) fn steps(
    view: &PipelineView,
    model: &PipelineModel,
    session: &PipelineSession,
    p: &Palette,
    cx: &Context<PipelineView>,
) -> impl IntoElement {
    let rows: Vec<_> = session
        .steps
        .iter()
        .enumerate()
        .map(|(i, step)| row(view, model, session, i, step, p, cx))
        .collect();
    v_flex()
        .id("pipeline-steps")
        .role(Role::List)
        .aria_label(text::STEPS_LABEL)
        .border_t_1()
        .border_color(p.border)
        .children(rows)
}

fn row(
    view: &PipelineView,
    model: &PipelineModel,
    session: &PipelineSession,
    i: usize,
    step: &StepView,
    p: &Palette,
    cx: &Context<PipelineView>,
) -> impl IntoElement {
    let copy = text::step_copy(step.id);
    let n = i + 1;
    let locked = !step.unlocked;
    let status_now = model.step_status(step.id);
    let running = model.running() == Some(step.id);
    let (status_tone, status_icon) = tone(status_now);
    let lock = locked
        .then(|| i.checked_sub(1).map(|before| session.steps[before].id))
        .flatten()
        .map(|before| {
            div()
                .mt(px(SPACE_1))
                .child(small_muted(text::runs_after(before)))
                .debug_selector(move || format!("pipeline-lock-{n}"))
        });
    let run_label = if running {
        text::RUNNING
    } else if copy.confirm.is_some() {
        text::RUN_ASKS
    } else {
        text::RUN
    };
    let on_run = view.model.clone();
    let id = step.id;
    let run = button(("pipeline-run", n), run_label)
        .tooltip(text::run_aria(id))
        .busy(running)
        .disabled(locked || !model.can_run())
        .debug_selector(format!("pipeline-run-{n}"))
        .on_click(move |_, _, cx| {
            on_run.update(cx, |m, cx| m.request_step(id, cx));
        })
        .tag(format!("pipeline-run-{n}"));

    let head = h_flex()
        .flex_wrap()
        .items_center()
        .gap_x(px(SPACE_3))
        .gap_y(px(SPACE_2))
        .child(step_number(n))
        .child(
            v_flex()
                .flex_grow(1.0)
                .flex_shrink(1.0)
                .flex_basis(px(TEXT_BASIS))
                .min_w_0()
                .child(h2(copy.title))
                .child(muted(copy.description))
                .children(lock),
        )
        .child(div().flex_none().min_w(px(STATUS_WIDTH)).child(status(
            status_tone,
            status_icon,
            text::status_label(status_now),
        )))
        .child(run);

    let result = result(view, step, i, p, cx);
    let name: SharedString = format!("pipeline-step-{n}").into();
    let focus_name = name.clone();
    div()
        .border_b_1()
        .border_color(p.border)
        .child(
            v_flex()
                .id(ElementId::from(focus_name))
                .role(Role::ListItem)
                .aria_label(copy.title)
                .track_focus(&view.rows[index(step.id)])
                // The ring is a transparent border until focused, inset so a
                // row's content does not move.
                .mx(px(-RING))
                .px(px(RING))
                .py(px(SPACE_3 - RING))
                .gap(px(SPACE_2))
                .border_2()
                .border_color(gpui::transparent_black())
                .rounded(px(RADIUS))
                .focus(|style| style.border_color(p.focus_ring))
                .child(head)
                .children(result),
        )
        .debug_selector({
            let name = name.to_string();
            move || name
        })
        .tag(name)
}

/// The report or the error of a step that has run, indented under the number.
fn result(
    view: &PipelineView,
    step: &StepView,
    i: usize,
    p: &Palette,
    cx: &Context<PipelineView>,
) -> Option<impl IntoElement> {
    let indent = SPACE_5 + SPACE_3;
    if let Some(report) = &step.report {
        let issues = (!report.issues.is_empty()).then(|| {
            let items: Vec<_> = report
                .issues
                .iter()
                .enumerate()
                .map(|(j, issue)| {
                    h_flex()
                        .id(("pipeline-issue", i * 1000 + j))
                        .role(Role::ListItem)
                        .items_start()
                        .gap(px(SPACE_1))
                        .child(body("•"))
                        .child(
                            h_flex()
                                .flex_wrap()
                                .min_w_0()
                                .children(
                                    issue
                                        .subdirectory
                                        .as_ref()
                                        .map(|name| mono(format!("{name}: "))),
                                )
                                .child(body(issue.message.clone())),
                        )
                })
                .collect();
            v_flex()
                .id(("pipeline-issues", i))
                .debug_selector(move || format!("pipeline-issues-{}", i + 1))
                .role(Role::List)
                .aria_label(text::problems_aria(step.id))
                .pl(px(SPACE_4))
                .children(items)
        });
        return Some(
            v_flex()
                .gap(px(SPACE_1))
                .pl(px(indent))
                .children((!report.summary.is_empty()).then(|| body(report.summary.clone())))
                .children(issues)
                .into_any_element(),
        );
    }
    let error = step.error.as_ref()?;
    let not_implemented = error.kind == PipelineErrorKind::StepNotImplemented;
    let message = if not_implemented {
        muted(text::error_message(error.kind))
    } else {
        body(text::error_message(error.kind)).text_color(p.danger)
    };
    let details = (error.kind == PipelineErrorKind::StepFailed).then(|| {
        let this = cx.entity();
        Disclosure::new(("pipeline-details", i), text::DETAILS)
            .debug_selector(format!("pipeline-details-{}", i + 1))
            .open(view.details[i])
            .on_toggle(move |open, _, cx| {
                this.update(cx, |view, cx| {
                    view.details[i] = open;
                    cx.notify();
                });
            })
            .child(mono_small_muted(error.message.clone()))
    });
    Some(
        v_flex()
            .gap(px(SPACE_1))
            .pl(px(indent))
            .child(message)
            .children(details)
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_status_has_a_tone_and_icon() {
        assert_eq!(tone(StepStatus::Passed).1, IconName::CheckCircle);
        assert_eq!(tone(StepStatus::Problems(2)).0, StatusTone::Warn);
        assert_eq!(tone(StepStatus::Failed).0, StatusTone::Danger);
        assert_eq!(tone(StepStatus::NotImplemented).1, IconName::XCircle);
        for s in [StepStatus::Running, StepStatus::Waiting, StepStatus::NotRun] {
            assert_eq!(tone(s), (StatusTone::Muted, IconName::Circle));
        }
    }
}
