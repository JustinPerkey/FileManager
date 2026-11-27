//! The Pipeline screen: renders [`PipelineModel`] and calls its methods.
//! Behaviour, copy and predicates live in the model and `text.rs`; this view
//! owns layout, focus and the folder field's draft.
//!
//! - `folder.rs`: the folder field, Browse and the note or error under them.
//! - `run_bar.rs`: Run next, Run all and the reason.
//! - `steps.rs`: the four steps and their results.
//!
//! Focus moves in two cases, both applied at the start of the next render so
//! the buttons they name are drawn in their new state: when a step starts
//! running (its row, because the Run button it came from is now inert), and
//! to the model's hint at the end of a run (a step's row, or Run next).
//! Step rows are focusable but not tab stops.
//!
//! The confirmation dialog is synced from the model observer, never from
//! `render`.

mod folder;
mod run_bar;
mod steps;
#[cfg(test)]
mod tests;

use fm_pipeline::session::StepId;
use gpui::prelude::*;
use gpui::{div, px, App, Context, Entity, FocusHandle, SharedString, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::v_flex;

use super::model::{FocusHint, Pending, PipelineModel};
use super::text;
use crate::app::probe::Tag as _;
use crate::theme::palette;
use crate::theme::tokens::{SPACE_4, SPACE_5};
use crate::widgets::{
    h1, intro, sync_confirm, Banner, ConfirmButton, ConfirmSpec, ConfirmVariant, Tone,
};

/// Where the next render puts focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Step(StepId),
    RunNext,
}

/// The index of `id` in run order.
fn index(id: StepId) -> usize {
    text::STEP_IDS
        .iter()
        .position(|s| *s == id)
        .expect("every step is in STEP_IDS")
}

pub struct PipelineView {
    model: Entity<PipelineModel>,
    /// The folder field. Its text is a draft until committed.
    input: Entity<InputState>,
    /// The session's folder text last put into the field; the draft differs
    /// from it while the user has typed something uncommitted.
    shown: String,
    /// One focus handle per step row, in run order.
    rows: [FocusHandle; 4],
    run_next: FocusHandle,
    /// The step running when the model last changed.
    running: Option<StepId>,
    focus: Option<Target>,
    /// Whether each step's "Details" is open.
    details: [bool; 4],
    _subscriptions: Vec<Subscription>,
}

impl PipelineView {
    pub fn new(model: Entity<PipelineModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(text::FOLDER_PLACEHOLDER));
        let field = gpui::Focusable::focus_handle(input.read(cx), cx);
        let subscriptions = vec![
            // Leaving the field commits (`InputEvent::Blur` fires too, but
            // only focus-out is delivered consistently).
            cx.on_focus_out(&field, window, |this, _, _, cx| {
                this.commit(cx);
            }),
            cx.observe_in(&model, window, |this, _, window, cx| {
                this.model_changed(window, cx);
            }),
            cx.subscribe_in(&input, window, |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.commit(cx);
                }
            }),
        ];
        let mut view = Self {
            model,
            input,
            shown: String::new(),
            rows: std::array::from_fn(|_| cx.focus_handle()),
            run_next: cx.focus_handle(),
            running: None,
            focus: None,
            details: [false; 4],
            _subscriptions: subscriptions,
        };
        view.model_changed(window, cx);
        view
    }

    /// The model changed: follow its folder, its confirmation and its focus
    /// wishes.
    fn model_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (folder, running, pending) = {
            let model = self.model.read(cx);
            let folder = model
                .session()
                .and_then(|s| s.directory.as_ref())
                .map(|d| d.path.clone())
                .unwrap_or_default();
            (folder, model.running(), confirm_spec(model))
        };
        if folder != self.shown {
            self.set_field(&folder, window, cx);
            self.shown = folder;
        }
        if running != self.running {
            self.running = running;
            if let Some(id) = running {
                self.focus = Some(Target::Step(id));
            }
        }
        if let Some(hint) = self.model.update(cx, |m, _| m.take_focus_hint()) {
            self.focus = Some(match hint {
                FocusHint::Step(id) => Target::Step(id),
                FocusHint::RunNext => Target::RunNext,
            });
        }
        let on_confirm = self.model.downgrade();
        let on_cancel = self.model.downgrade();
        sync_confirm(
            window,
            cx,
            "pipeline-confirm",
            pending,
            move |_: &mut Window, cx: &mut App| {
                on_confirm.update(cx, PipelineModel::confirm).ok();
            },
            move |_: &mut Window, cx: &mut App| {
                on_cancel.update(cx, PipelineModel::cancel).ok();
            },
        );
        cx.notify();
    }

    fn set_field(&self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| {
            input.set_value(text.to_string(), window, cx);
        });
    }

    /// Enter or leaving the field: sends the draft to the model when it
    /// differs from the folder shown.
    fn commit(&mut self, cx: &mut Context<Self>) {
        let typed = self.input.read(cx).value().to_string();
        if typed != self.shown {
            self.model
                .update(cx, |model, cx| model.set_directory_text(&typed, cx));
        }
    }

    /// Escape in the field: drops the draft.
    fn revert(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let typed = self.input.read(cx).value();
        if typed.as_ref() == self.shown.as_str() {
            cx.propagate();
        } else {
            let shown = self.shown.clone();
            self.set_field(&shown, window, cx);
        }
    }

    fn apply_focus(&mut self, window: &mut Window, cx: &mut App) {
        let Some(target) = self.focus.take() else {
            return;
        };
        match target {
            Target::Step(id) => self.rows[index(id)].focus(window, cx),
            Target::RunNext => self.run_next.focus(window, cx),
        }
    }
}

/// What the confirmation shows for the model's pending request.
fn confirm_spec(model: &PipelineModel) -> Option<ConfirmSpec> {
    let (pending, folder) = model.pending()?;
    let (title, body) = match pending {
        Pending::Step { id, .. } => (
            text::confirm_title(*id),
            text::confirm_step_body(*id, folder),
        ),
        Pending::All => (text::ALL_TITLE.to_string(), text::confirm_all_body(folder)),
    };
    Some(ConfirmSpec {
        title: title.into(),
        body: SharedString::from(body.plain()),
        extra: None,
        cancel_label: text::CANCEL.into(),
        confirm: Some(ConfirmButton {
            label: text::CONFIRM_RUN.into(),
            variant: ConfirmVariant::Primary,
            icon: None,
        }),
        busy: false,
        wide: false,
    })
}

impl Render for PipelineView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.apply_focus(window, cx);
        let p = *palette(cx);
        let model = self.model.read(cx);

        let mut page = v_flex()
            .w_full()
            .px(px(SPACE_5))
            .py(px(SPACE_4))
            .gap(px(SPACE_4))
            .debug_selector(|| "pipeline-page".into())
            .child(
                v_flex()
                    .gap(px(crate::theme::tokens::SPACE_1))
                    .child(h1(text::HEADER))
                    .child(intro(text::INTRO))
                    .debug_selector(|| "pipeline-header".into()),
            );
        if model.load_failed() {
            page = page.children(model.error().map(|error| {
                let this = self.model.clone();
                div()
                    .child(
                        Banner::new(Tone::Error, error.copy())
                            .id(("pipeline-load-error", model.error_generation()))
                            .details(error.detail().to_string())
                            .action(
                                crate::widgets::button("pipeline-retry", text::TRY_AGAIN)
                                    .debug_selector("pipeline-retry")
                                    .on_click(move |_, _, cx| {
                                        this.update(cx, PipelineModel::retry_load);
                                    }),
                            ),
                    )
                    .debug_selector(|| "pipeline-error".into())
                    .tag("pipeline-error")
            }));
        } else if let Some(session) = model.session() {
            page = page.child(folder::folder(self, model, &p, cx));
            page = page.children(model.error().map(|error| {
                let this = self.model.clone();
                div()
                    .child(
                        Banner::new(Tone::Error, error.copy())
                            .id(("pipeline-error", model.error_generation()))
                            .details(error.detail().to_string())
                            .dismiss(move |_, cx| {
                                this.update(cx, PipelineModel::dismiss_error);
                            }),
                    )
                    .debug_selector(|| "pipeline-error".into())
                    .tag("pipeline-error")
            }));
            page = page
                .child(run_bar::run_bar(self, model, &p, cx))
                .child(steps::steps(self, model, session, &p, cx));
        }

        div().id("pipeline").size_full().child(
            div()
                .id("pipeline-scroll")
                .size_full()
                .overflow_y_scroll()
                .child(page),
        )
    }
}
