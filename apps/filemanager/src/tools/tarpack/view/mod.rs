//! Tar Packager view: the frame and everything above the entry table (the
//! loading skeleton, the restore failure, the empty state, the manifest header
//! with its Recent menu, the banners, the errors region and the drop result).
//! The entry table, the build result and the build bar fill the three slots
//! below it (`entry_table`, `build_result` with `report`, `build_bar` with
//! `progress`). `keys` binds the shortcuts.
//!
//! The root fills the tool area, pads itself (16 px by 24 px, plus the 15 px
//! scrollbar gutter on the right, as today) and holds a body
//! and the build bar. With a manifest the body does not scroll: the entry table
//! takes the room the rest leaves and scrolls its rows inside itself. Only if
//! the window is shorter than everything above the table plus three rows does
//! the body scroll, as a fallback. Without a manifest the body scrolls. The
//! build bar stays in view.

mod banners;
mod build_bar;
mod build_result;
mod drop_result;
mod empty;
mod entry_cells;
mod entry_table;
mod errors;
mod header;
mod keys;
mod progress;
mod report;
pub mod shortcuts;
#[cfg(test)]
mod tests;

use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    div, px, AnyElement, App, Context, Entity, FocusHandle, IntoElement, Render, Role,
    ScrollHandle, Subscription, Window,
};
use gpui_component::v_flex;

use self::build_bar::BarFocus;
use self::build_result::{CopyKind, ResultFocus, ResultUi};
use self::entry_table::EntryTable;
use super::model::{DropState, FocusHint, TarpackModel};
use super::text;
use crate::app::dev_mode::DevMode;
use crate::theme::palette;
use crate::theme::tokens::{SCROLLBAR_GUTTER, SPACE_2, SPACE_3, SPACE_4, SPACE_5};
use crate::widgets::{skeleton_bar, sync_confirm, ConfirmButton, ConfirmSpec, ConfirmVariant};

/// How long a Copy button says "Copied".
const COPIED_FOR: Duration = Duration::from_secs(2);

pub struct TarpackView {
    model: Entity<TarpackModel>,
    /// The view's root: the key context of the shortcuts needs focus inside
    /// it, so it takes focus when nothing has it.
    root_focus: FocusHandle,
    /// The errors report (focus hint `Report`).
    report_focus: FocusHandle,
    /// The Reload button (focus hint `Reload`).
    reload_focus: FocusHandle,
    /// The drop result: focus inside it is lost when it goes.
    result_focus: FocusHandle,
    /// The build result's container: the same for the build result.
    build_result_focus: FocusHandle,
    /// The build result's report region (a tab stop, so it scrolls by keys).
    build_report_focus: FocusHandle,
    /// The first tab stop after the drop result: the build bar's first
    /// control, which tracks it. The view moves focus there when the drop
    /// result goes.
    after_result_focus: FocusHandle,
    /// Create archive (focus hints `CreateArchive` and
    /// `CreateArchiveAfterDialog`).
    create_focus: FocusHandle,
    warnings_open: bool,
    paths_open: bool,
    /// The build result's Details, its report's warnings and the last copy.
    result_ui: ResultUi,
    /// Bumps with every copy, so an older "Copied" timer does not clear a
    /// newer one.
    copied_generation: u64,
    /// The overwrite confirmation last handed to the dialog.
    confirming: Option<String>,
    /// Whether the first frame has looked for focus yet.
    focus_taken: bool,
    /// Tells a held key from a press (see `keys`).
    latch: keys::SharedLatch,
    /// The entry table, made on the first frame that has a window.
    entry_table: Option<Entity<EntryTable>>,
    /// The body's scroll position (the fallback scroll, and the empty state).
    body_scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl TarpackView {
    pub fn new(model: Entity<TarpackModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        keys::ensure_bindings(cx);
        let latch = keys::SharedLatch::default();
        let released = latch.clone();
        let subscriptions = vec![
            cx.observe_in(&model, window, |this, _, window, cx| {
                this.model_changed(window, cx);
            }),
            cx.observe_global::<DevMode>(|_, cx| cx.notify()),
            // A key-up that went to another window (a dialog) never arrives.
            cx.observe_window_activation(window, move |_, _, _| {
                released.borrow_mut().release();
            }),
        ];
        Self {
            model,
            root_focus: cx.focus_handle(),
            report_focus: cx.focus_handle(),
            reload_focus: cx.focus_handle(),
            result_focus: cx.focus_handle(),
            build_result_focus: cx.focus_handle(),
            build_report_focus: cx.focus_handle(),
            after_result_focus: cx.focus_handle(),
            create_focus: cx.focus_handle(),
            warnings_open: false,
            paths_open: false,
            result_ui: ResultUi::default(),
            copied_generation: 0,
            confirming: None,
            focus_taken: false,
            latch,
            entry_table: None,
            body_scroll: ScrollHandle::new(),
            _subscriptions: subscriptions,
        }
    }

    /// The model changed: follow its overwrite confirmation, then redraw. The
    /// dialog is opened and closed here, never inside `render`.
    fn model_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let confirming = self
            .model
            .read(cx)
            .confirming_overwrite()
            .map(str::to_string);
        if confirming != self.confirming {
            let spec = confirming.as_deref().map(overwrite_spec);
            let on_confirm = self.model.downgrade();
            let on_cancel = self.model.downgrade();
            sync_confirm(
                window,
                cx,
                "tarpack-overwrite",
                spec,
                move |_: &mut Window, cx: &mut App| {
                    on_confirm.update(cx, TarpackModel::confirm_overwrite).ok();
                },
                move |_: &mut Window, cx: &mut App| {
                    on_cancel.update(cx, TarpackModel::cancel_overwrite).ok();
                },
            );
            self.confirming = confirming;
        }
        cx.notify();
    }

    /// A copy button was used: it says "Copied" for a moment.
    fn show_copied(&mut self, kind: CopyKind, cx: &mut Context<Self>) {
        self.result_ui.copied = Some(kind);
        self.copied_generation += 1;
        let generation = self.copied_generation;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(COPIED_FOR).await;
            this.update(cx, |v, cx| {
                if v.copied_generation == generation {
                    v.result_ui.copied = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn set_build_details_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.result_ui.details_open = open;
        cx.notify();
    }

    fn set_build_warnings_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.result_ui.warnings_open = open;
        cx.notify();
    }

    /// The focus handle of the first control after the drop result: the build
    /// bar's first control tracks it.
    #[must_use]
    pub fn after_result_focus(&self) -> &FocusHandle {
        &self.after_result_focus
    }

    /// Moves focus as the model hints. Focus moves only where it would be
    /// lost: when it sits inside the drop result that is going away.
    fn follow_focus_hint(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(hint) = self.model.update(cx, |m, _| m.take_focus_hint()) else {
            return;
        };
        // Either result: a reload removes both, and a build result can hold
        // focus too.
        let in_result = self.result_focus.contains_focused(window, cx)
            || self.build_result_focus.contains_focused(window, cx);
        match hint {
            FocusHint::Report => self.report_focus.focus(window, cx),
            FocusHint::Reload if in_result => self.reload_focus.focus(window, cx),
            FocusHint::AfterResult if in_result => self.after_result_focus.focus(window, cx),
            // Focus went with the result, sat on the button while it was
            // disabled for the build, or is on nothing (the view's root):
            // only then does it move.
            FocusHint::CreateArchive
                if in_result
                    || self.create_focus.is_focused(window)
                    || self.root_focus.is_focused(window)
                    || window.focused(cx).is_none() =>
            {
                self.create_focus.focus(window, cx);
            }
            FocusHint::CreateArchiveAfterDialog => self.create_focus.focus(window, cx),
            _ => {}
        }
    }

    /// The drop's pending line or its result.
    fn drop_part(
        &self,
        entity: &Entity<Self>,
        state: &TarpackModel,
        window: &Window,
        cx: &App,
    ) -> Option<AnyElement> {
        let model = self.model.clone();
        match state.drop_state() {
            Some(DropState::Result(report)) => {
                let view = entity.clone();
                Some(
                    div()
                        .id("tarpack-drop-result-focus")
                        .track_focus(&self.result_focus)
                        .child(drop_result::drop_result(
                            &model,
                            report,
                            self.paths_open,
                            move |open, _, cx| {
                                view.update(cx, |v, cx| {
                                    v.paths_open = open;
                                    cx.notify();
                                });
                            },
                            window,
                            cx,
                        ))
                        .into_any_element(),
                )
            }
            Some(DropState::Pending) if state.drop_pending_shown() => {
                Some(drop_result::drop_pending(cx).into_any_element())
            }
            _ => None,
        }
    }
}

/// The entry table (U10), once a manifest is shown.
fn table_slot(table: Option<&Entity<EntryTable>>, shown: bool) -> Option<AnyElement> {
    table
        .filter(|_| shown)
        .map(|t| t.clone().into_any_element())
}

/// What the overwrite confirmation shows for the output it names.
fn overwrite_spec(output: &str) -> ConfirmSpec {
    ConfirmSpec {
        title: text::overwrite_title(output).into(),
        body: text::overwrite_body(output).into(),
        extra: None,
        cancel_label: text::OVERWRITE_CANCEL.into(),
        confirm: Some(ConfirmButton {
            label: text::OVERWRITE_CONFIRM.into(),
            variant: ConfirmVariant::Primary,
            icon: None,
        }),
        busy: false,
        wide: false,
    }
}

impl Render for TarpackView {
    #[expect(clippy::too_many_lines, reason = "one builder chain")]
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.follow_focus_hint(window, cx);
        let p = *palette(cx);
        let dev = DevMode::is_on(cx);
        let entity = cx.entity();
        if !self.focus_taken {
            // Nothing is focused in a fresh window: the shortcuts need the
            // view's key context on the focus path.
            self.focus_taken = true;
            if window.focused(cx).is_none() {
                self.root_focus.focus(window, cx);
            }
        }
        let model = self.model.clone();
        if self.entry_table.is_none() {
            self.entry_table = Some(cx.new(|cx| EntryTable::new(model.clone(), window, cx)));
        }
        let state = self.model.read(cx);
        if !matches!(state.drop_state(), Some(DropState::Result(_))) {
            self.paths_open = false;
        }
        if state.result().is_none() {
            self.result_ui = ResultUi::default();
        }

        let loading = state.loading();
        let manifest = state.session().and_then(|s| s.manifest.as_ref());
        let recent = state.recent();
        let table_shown = !loading && manifest.is_some();

        let mut top = v_flex().w_full().gap(px(SPACE_3));
        if loading {
            if state.show_skeleton() {
                top = top.child(
                    v_flex()
                        .id("tarpack-skeleton")
                        .debug_selector(|| "tarpack-skeleton".to_string())
                        .w_full()
                        .gap(px(SPACE_2))
                        .role(Role::Status)
                        .aria_label(text::LOADING)
                        .child(skeleton_bar(0.6, true))
                        .child(skeleton_bar(0.4, false)),
                );
            }
        } else {
            top = match manifest {
                Some(manifest) => top.child(header::header(
                    &model,
                    &header::HeaderProps {
                        manifest,
                        recent,
                        can_edit: state.can_edit(cx),
                        dev,
                        reload_focus: &self.reload_focus,
                    },
                    cx,
                )),
                None => top.child(header::titlebar(&model, dev)),
            };
            top = top.children(banners::banners(&model, state, manifest.is_some()));
            match manifest {
                None => top = top.child(empty::empty_state(&model, recent)),
                Some(manifest) => {
                    top = top.children(errors::errors_region(
                        &model,
                        manifest,
                        state.errors_expanded(),
                        &self.report_focus,
                        window,
                        cx,
                    ));
                    let view = entity.clone();
                    top = top.children(errors::warnings(
                        manifest,
                        self.warnings_open,
                        move |open, _, cx| {
                            view.update(cx, |v, cx| {
                                v.warnings_open = open;
                                cx.notify();
                            });
                        },
                        cx,
                    ));
                    top = top.children(self.drop_part(&entity, state, window, cx));
                }
            }
        }

        let table = table_slot(self.entry_table.as_ref(), table_shown);
        let result = build_result::build_result(
            &entity,
            &model,
            state,
            self.result_ui,
            &ResultFocus {
                result: &self.build_result_focus,
                report: &self.build_report_focus,
            },
            window,
            cx,
        );
        let bar = if loading {
            None
        } else {
            build_bar::build_bar(
                &model,
                state,
                &BarFocus {
                    first: &self.after_result_focus,
                    create: &self.create_focus,
                },
                cx,
            )
        };

        let root = v_flex()
            .id("tarpack-view")
            .key_context(keys::CONTEXT)
            .track_focus(&self.root_focus)
            .tab_stop(false)
            .size_full()
            .pl(px(SPACE_5))
            // Today's scrollbar gutter, which GPUI's overlay scrollbars lack.
            .pr(px(SPACE_5 + SCROLLBAR_GUTTER))
            .py(px(SPACE_4))
            .gap(px(SPACE_3))
            .bg(p.bg)
            .text_color(p.text)
            .child(
                div()
                    .id("tarpack-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.body_scroll)
                    .child(
                        // At least as tall as the body. The table's region (the
                        // one flexible child) starts at its three-row minimum
                        // and grows into what the rest leaves; when the rest
                        // needs more than the body has, this column is taller
                        // than the body and the body scrolls.
                        v_flex()
                            .min_h_full()
                            .gap(px(SPACE_3))
                            .child(top.flex_none())
                            .children(table)
                            .children(result),
                    ),
            )
            .children(bar);
        keys::attach(root, &model, &self.latch)
    }
}
