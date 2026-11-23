//! Tar Packager view: the frame and everything above the entry table (the
//! loading skeleton, the restore failure, the empty state, the manifest header
//! with its Recent menu, the banners, the errors region and the drop result).
//! The entry table, the build result and the build bar fill the three slots
//! below it.
//!
//! The root fills the tool area, pads itself (16 px by 24 px) and scrolls its
//! own body; the slot under the body (the build bar) stays in view.

mod banners;
mod drop_result;
mod empty;
mod errors;
mod header;
pub mod shortcuts;
#[cfg(test)]
mod tests;

use gpui::prelude::*;
use gpui::{
    div, px, AnyElement, App, Context, Entity, FocusHandle, IntoElement, Render, Role, Window,
};
use gpui_component::v_flex;

use super::model::{DropState, FocusHint, TarpackModel};
use super::text;
use crate::app::dev_mode::DevMode;
use crate::theme::palette;
use crate::theme::tokens::{SPACE_2, SPACE_3, SPACE_4, SPACE_5};
use crate::widgets::skeleton_bar;

pub struct TarpackView {
    model: Entity<TarpackModel>,
    /// The errors report (focus hint `Report`).
    report_focus: FocusHandle,
    /// The Reload button (focus hint `Reload`).
    reload_focus: FocusHandle,
    /// The drop result: focus inside it is lost when it goes.
    result_focus: FocusHandle,
    /// The first tab stop after the drop result: the build bar's first
    /// control. The bar (U11) tracks it; the view moves focus there when the
    /// drop result goes.
    after_result_focus: FocusHandle,
    warnings_open: bool,
    paths_open: bool,
}

impl TarpackView {
    pub fn new(model: Entity<TarpackModel>, cx: &mut Context<Self>) -> Self {
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        Self {
            model,
            report_focus: cx.focus_handle(),
            reload_focus: cx.focus_handle(),
            result_focus: cx.focus_handle(),
            after_result_focus: cx.focus_handle(),
            warnings_open: false,
            paths_open: false,
        }
    }

    /// The focus handle of the first control after the drop result, for the
    /// build bar to track.
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
        let in_result = self.result_focus.contains_focused(window, cx);
        match hint {
            FocusHint::Report => self.report_focus.focus(window, cx),
            FocusHint::Reload if in_result => self.reload_focus.focus(window, cx),
            FocusHint::AfterResult if in_result => self.after_result_focus.focus(window, cx),
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

/// The entry table (U10).
fn table_slot() -> Option<AnyElement> {
    None
}

/// The build result (U11).
fn result_slot() -> Option<AnyElement> {
    None
}

/// The build bar (U11).
fn bar_slot() -> Option<AnyElement> {
    None
}

impl Render for TarpackView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.follow_focus_hint(window, cx);
        let p = *palette(cx);
        let dev = DevMode::is_on(cx);
        let entity = cx.entity();
        let model = self.model.clone();
        let state = self.model.read(cx);
        if !matches!(state.drop_state(), Some(DropState::Result(_))) {
            self.paths_open = false;
        }

        let loading = state.loading();
        let manifest = state.session().and_then(|s| s.manifest.as_ref());
        let recent = state.recent();

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

        v_flex()
            .size_full()
            .px(px(SPACE_5))
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
                    .child(
                        v_flex()
                            .gap(px(SPACE_3))
                            .child(top)
                            .children(table_slot())
                            .children(result_slot()),
                    ),
            )
            .children(bar_slot())
    }
}
