//! The Schedule Creator screen: renders [`ScheduleModel`] and calls its
//! methods. Behaviour, copy and predicates live in the model and `text.rs`;
//! this view owns layout, focus and the two path fields' drafts.
//!
//! - `steps.rs`: the two steps (label, field, Browse, the slot's error).
//! - `apply.rs`: the Add row and the result banner.
//!
//! Focus moves only to the model's hint after an apply, applied at the start
//! of the next render so the target is drawn in its new state. The
//! confirmation dialog is synced from the model observer, never from
//! `render`.

mod apply;
mod steps;
#[cfg(test)]
mod tests;

use gpui::prelude::*;
use gpui::{div, px, App, Context, Entity, FocusHandle, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::v_flex;

use super::model::{FocusHint, ScheduleModel, Slot};
use super::text;
use crate::app::probe::Tag as _;
use crate::theme::tokens::{SPACE_1, SPACE_4, SPACE_5};
use crate::widgets::{
    button, h1, intro, sync_confirm, Banner, ConfirmButton, ConfirmSpec, ConfirmVariant, Tone,
};

/// The confirmation's cancel button. (`text.rs` has no constant for it.)
const CANCEL: &str = "Cancel";

const SLOTS: [Slot; 2] = [Slot::Text, Slot::Xml];

fn index(slot: Slot) -> usize {
    match slot {
        Slot::Text => 0,
        Slot::Xml => 1,
    }
}

/// One path field. Its text is a draft until committed.
struct Field {
    input: Entity<InputState>,
    /// The session's path last put into the field.
    shown: String,
}

pub struct ScheduleView {
    model: Entity<ScheduleModel>,
    fields: [Field; 2],
    /// The result or error banner.
    outcome: FocusHandle,
    add: FocusHandle,
    focus: Option<FocusHint>,
    _subscriptions: Vec<Subscription>,
}

impl ScheduleView {
    pub fn new(model: Entity<ScheduleModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let placeholders = [text::TEXT_PLACEHOLDER, text::XML_PLACEHOLDER];
        let inputs: [Entity<InputState>; 2] = std::array::from_fn(|i| {
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholders[i]))
        });
        let mut subscriptions = vec![cx.observe_in(&model, window, |this, _, window, cx| {
            this.model_changed(window, cx);
        })];
        for (slot, input) in SLOTS.into_iter().zip(&inputs) {
            let field = gpui::Focusable::focus_handle(input.read(cx), cx);
            // Leaving a field commits.
            subscriptions.push(cx.on_focus_out(&field, window, move |this, _, _, cx| {
                this.commit(slot, cx);
            }));
            subscriptions.push(cx.subscribe_in(
                input,
                window,
                move |this, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        this.commit(slot, cx);
                    }
                },
            ));
        }
        let mut view = Self {
            model,
            fields: inputs.map(|input| Field {
                input,
                shown: String::new(),
            }),
            outcome: cx.focus_handle(),
            add: cx.focus_handle(),
            focus: None,
            _subscriptions: subscriptions,
        };
        view.model_changed(window, cx);
        view
    }

    /// The model changed: follow its paths, its confirmation and its focus
    /// wish.
    fn model_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for slot in SLOTS {
            let path = self
                .model
                .read(cx)
                .file_path(slot)
                .unwrap_or_default()
                .to_string();
            let field = &mut self.fields[index(slot)];
            if path != field.shown {
                let input = field.input.clone();
                input.update(cx, |input, cx| input.set_value(path.clone(), window, cx));
                self.fields[index(slot)].shown = path;
            }
        }
        if let Some(hint) = self.model.update(cx, |m, _| m.take_focus_hint()) {
            self.focus = Some(hint);
        }
        let spec = confirm_spec(self.model.read(cx));
        let on_confirm = self.model.downgrade();
        let on_cancel = self.model.downgrade();
        sync_confirm(
            window,
            cx,
            "schedule-confirm",
            spec,
            move |_: &mut Window, cx: &mut App| {
                on_confirm.update(cx, ScheduleModel::confirm).ok();
            },
            move |_: &mut Window, cx: &mut App| {
                on_cancel.update(cx, ScheduleModel::cancel).ok();
            },
        );
        cx.notify();
    }

    fn set_field(&self, slot: Slot, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.fields[index(slot)].input.update(cx, |input, cx| {
            input.set_value(text.to_string(), window, cx);
        });
    }

    /// Enter or leaving the field: sends the draft to the model when it
    /// differs from the path shown.
    fn commit(&mut self, slot: Slot, cx: &mut Context<Self>) {
        let field = &self.fields[index(slot)];
        let typed = field.input.read(cx).value().to_string();
        if typed != field.shown {
            self.model.update(cx, |m, cx| m.set_typed(slot, &typed, cx));
        }
    }

    /// Escape in a field: drops the draft.
    fn revert(&mut self, slot: Slot, window: &mut Window, cx: &mut Context<Self>) {
        let field = &self.fields[index(slot)];
        if field.input.read(cx).value().as_ref() == field.shown.as_str() {
            cx.propagate();
        } else {
            let shown = field.shown.clone();
            self.set_field(slot, &shown, window, cx);
        }
    }

    fn apply_focus(&mut self, window: &mut Window, cx: &mut App) {
        match self.focus.take() {
            Some(FocusHint::Outcome) => self.outcome.focus(window, cx),
            Some(FocusHint::AddButton) => self.add.focus(window, cx),
            None => {}
        }
    }
}

/// What the confirmation shows for the model's pending request.
fn confirm_spec(model: &ScheduleModel) -> Option<ConfirmSpec> {
    let (text_path, xml_path) = model.pending()?;
    let mut body = text::confirm_body(text_path, xml_path).plain();
    if model.already_applied() {
        body = format!(
            "{body} {} {}",
            text::DUPLICATE_HEADLINE,
            text::DUPLICATE_REST
        );
    }
    Some(ConfirmSpec {
        title: text::confirm_title(xml_path).into(),
        body: body.into(),
        extra: None,
        cancel_label: CANCEL.into(),
        confirm: Some(ConfirmButton {
            label: text::CONFIRM_LABEL.into(),
            variant: ConfirmVariant::Primary,
            icon: None,
        }),
        busy: false,
        wide: false,
    })
}

impl Render for ScheduleView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.apply_focus(window, cx);
        let p = *crate::theme::palette(cx);
        let model = self.model.read(cx);

        let mut page = v_flex()
            .w_full()
            .px(px(SPACE_5))
            .py(px(SPACE_4))
            .gap(px(SPACE_4))
            .debug_selector(|| "schedule-page".into())
            .child(
                v_flex()
                    .gap(px(SPACE_1))
                    .child(h1(text::TITLE))
                    .child(intro(text::INTRO))
                    .debug_selector(|| "schedule-header".into()),
            );
        if model.load_failed() {
            page = page.children(model.error().map(|error| {
                let this = self.model.clone();
                div()
                    .child(
                        Banner::new(Tone::Error, error.copy())
                            .id(("schedule-load-error", model.error_generation()))
                            .details(error.detail().to_string())
                            .action(
                                button("schedule-retry", text::TRY_AGAIN)
                                    .debug_selector("schedule-retry")
                                    .on_click(move |_, _, cx| {
                                        this.update(cx, ScheduleModel::retry_load);
                                    }),
                            ),
                    )
                    .debug_selector(|| "schedule-error".into())
                    .tag("schedule-error")
            }));
        } else if model.session().is_some() {
            page = page.children(self.error_banner(model));
            page = page
                .child(steps::steps(self, model, &p, cx))
                .child(apply::apply_row(self, model, &p))
                .children(apply::result(self, model));
        }

        div().id("schedule").size_full().child(
            div()
                .id("schedule-scroll")
                .size_full()
                .overflow_y_scroll()
                .child(page),
        )
    }
}

impl ScheduleView {
    fn error_banner(&self, model: &ScheduleModel) -> Option<impl IntoElement> {
        // The result banner holds the outcome focus when there is one.
        let focus = model.result().is_none();
        model.error().map(|error| {
            let this = self.model.clone();
            div()
                .id("schedule-error-focus")
                .when(focus, |d| d.track_focus(&self.outcome))
                .child(
                    Banner::new(Tone::Error, error.copy())
                        .id(("schedule-error", model.error_generation()))
                        .details(error.detail().to_string())
                        .dismiss(move |_, cx| {
                            this.update(cx, ScheduleModel::dismiss_error);
                        }),
                )
                .debug_selector(|| "schedule-error".into())
                .tag("schedule-error")
        })
    }
}
