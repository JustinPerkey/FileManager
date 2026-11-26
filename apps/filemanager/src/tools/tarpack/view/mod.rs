//! Tar Packager view: the frame and everything above the entry table (the
//! loading skeleton, the restore failure, the empty state, the manifest header
//! with its Recent menu, the banners, the errors region and the drop result).
//! The entry table, the build result and the build bar fill the three slots
//! below it.
//!
//! The root fills the tool area, pads itself (16 px by 24 px) and holds a body
//! and the build bar. With a manifest the body does not scroll: the entry table
//! takes the room the rest leaves and scrolls its rows inside itself. Only if
//! the window is shorter than everything above the table plus three rows does
//! the body scroll, as a fallback. Without a manifest the body scrolls. The
//! build bar stays in view.

mod banners;
mod drop_result;
mod empty;
mod entry_cells;
mod entry_table;
mod errors;
mod header;
pub mod shortcuts;
#[cfg(test)]
mod tests;

use gpui::prelude::*;
use gpui::{
    div, px, AnyElement, App, Context, Entity, FocusHandle, IntoElement, Render, Role,
    ScrollHandle, Window,
};
use gpui_component::v_flex;

use self::entry_table::EntryTable;
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
    /// The entry table, made on the first frame that has a window.
    entry_table: Option<Entity<EntryTable>>,
    /// The body's scroll position (the fallback scroll, and the empty state).
    body_scroll: ScrollHandle,
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
            entry_table: None,
            body_scroll: ScrollHandle::new(),
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

/// The entry table (U10), once a manifest is shown.
fn table_slot(table: Option<&Entity<EntryTable>>, shown: bool) -> Option<AnyElement> {
    table
        .filter(|_| shown)
        .map(|t| t.clone().into_any_element())
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

        if self.entry_table.is_none() {
            self.entry_table = Some(cx.new(|cx| EntryTable::new(model.clone(), window, cx)));
        }
        let table = table_slot(self.entry_table.as_ref(), table_shown);

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
                            .children(result_slot()),
                    ),
            )
            .children(bar_slot())
    }
}
