//! Database Manager view: the frame and everything about the session (the
//! header with the database and config status, the banners, the config
//! problems, the empty state and the first shortcuts). The panes below the
//! session (the table list, the sheet) fill the `workspace_slot` later.
//!
//! The root fills the tool area, pads itself (16 px by 24 px) and scrolls its
//! own body. Behaviour, drafts and most copy are the model's.

mod banners;
mod config_problems;
mod empty;
mod header;
pub mod keys;
pub mod shortcuts;
#[cfg(test)]
mod tests;

use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    div, px, AnyElement, Context, Entity, FocusHandle, IntoElement, KeyUpEvent, Render, Role,
    ScrollHandle, Window,
};
use gpui_component::v_flex;

use self::banners::Dismissed;
use self::keys::{OpenDatabase, ReloadDatabase, ShowConfigProblems, CONTEXT};
use super::model::{DbmanagerModel, FocusHint};
use super::text;
use crate::theme::palette;
use crate::theme::tokens::{SPACE_2, SPACE_3, SPACE_4, SPACE_5};
use crate::widgets::skeleton_bar;

/// How long loading lasts before the skeleton shows.
pub const SKELETON_DELAY: Duration = Duration::from_millis(150);

/// The loading skeleton's state.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Skeleton {
    /// Nothing is loading with nothing to show.
    Idle,
    /// Loading; the timer of this generation is running.
    Armed(u64),
    /// Loading has lasted [`SKELETON_DELAY`]: the bars show.
    Shown,
}

pub struct DbmanagerView {
    model: Entity<DbmanagerModel>,
    /// The root: shortcuts work while focus is anywhere inside it.
    focus: FocusHandle,
    /// The error banner (focus hint `ErrorBanner`).
    error_focus: FocusHandle,
    /// The config problems' toggle (F8).
    problems_focus: FocusHandle,
    problems_open: bool,
    /// The "no config file" notice was dismissed (for the app run).
    no_config_dismissed: bool,
    /// The text of the state warning that was dismissed.
    state_warning_dismissed: Option<String>,
    /// A shortcut fired and its key has not come up yet: a held key does not
    /// repeat.
    latched: bool,
    skeleton: Skeleton,
    skeleton_generation: u64,
    body_scroll: ScrollHandle,
}

impl DbmanagerView {
    pub fn new(model: Entity<DbmanagerModel>, cx: &mut Context<Self>) -> Self {
        keys::ensure_bindings(cx);
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        Self {
            model,
            focus: cx.focus_handle(),
            error_focus: cx.focus_handle(),
            problems_focus: cx.focus_handle(),
            problems_open: false,
            no_config_dismissed: false,
            state_warning_dismissed: None,
            latched: false,
            skeleton: Skeleton::Idle,
            skeleton_generation: 0,
            body_scroll: ScrollHandle::new(),
        }
    }

    /// Moves focus as the model hints. This view owns the error banner's; the
    /// panes below take theirs when they exist.
    fn follow_focus_hint(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(hint) = self.model.update(cx, |m, _| m.take_focus_hint()) else {
            return;
        };
        if hint == FocusHint::ErrorBanner {
            self.error_focus.focus(window, cx);
        }
    }

    /// Starts the skeleton's timer while something loads with nothing to show,
    /// and forgets it when that ends.
    fn track_skeleton(&mut self, waiting: bool, cx: &mut Context<Self>) {
        if !waiting {
            self.skeleton = Skeleton::Idle;
            return;
        }
        if self.skeleton != Skeleton::Idle {
            return;
        }
        self.skeleton_generation += 1;
        let generation = self.skeleton_generation;
        self.skeleton = Skeleton::Armed(generation);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SKELETON_DELAY).await;
            this.update(cx, |v, cx| {
                if v.skeleton == Skeleton::Armed(generation) {
                    v.skeleton = Skeleton::Shown;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Takes the latch for a shortcut; `false` for a repeat of a held key.
    fn take_latch(&mut self) -> bool {
        !std::mem::replace(&mut self.latched, true)
    }

    fn open_shortcut(&mut self, cx: &mut Context<Self>) {
        if self.take_latch() {
            self.model.update(cx, DbmanagerModel::open);
        }
    }

    fn reload_shortcut(&mut self, cx: &mut Context<Self>) {
        let open = self
            .model
            .read(cx)
            .snapshot()
            .is_some_and(|s| s.open && s.database.is_some());
        if open && self.take_latch() {
            self.model.update(cx, DbmanagerModel::reload);
        }
    }

    fn problems_shortcut(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let any = self
            .model
            .read(cx)
            .snapshot()
            .is_some_and(|s| !s.diagnostics.is_empty());
        if any && self.take_latch() {
            self.problems_open = true;
            self.problems_focus.focus(window, cx);
            cx.notify();
        }
    }
}

/// The skeleton: two bars while loading.
fn skeleton() -> impl IntoElement {
    v_flex()
        .id("db-skeleton")
        .debug_selector(|| "db-skeleton".to_string())
        .w_full()
        .gap(px(SPACE_2))
        .role(Role::Status)
        .aria_label(text::LOADING)
        .child(skeleton_bar(0.6, true))
        .child(skeleton_bar(0.4, false))
}

/// The slot the table list and the sheet fill.
fn workspace_slot() -> impl IntoElement {
    div()
        .id("db-workspace")
        .flex_1()
        .debug_selector(|| "db-workspace".to_string())
}

impl Render for DbmanagerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focus.contains_focused(window, cx) {
            self.latched = false;
        }
        self.follow_focus_hint(window, cx);
        let p = *palette(cx);
        let entity = cx.entity();
        let model = self.model.clone();
        let state = self.model.read(cx);
        let snapshot = state.snapshot();

        let open_slot = snapshot
            .filter(|s| s.open)
            .and_then(|s| s.database.as_ref());
        // The last database failed to open: nothing is open.
        let failed = snapshot
            .filter(|s| !s.open)
            .and_then(|s| s.database.as_ref())
            .filter(|d| d.error.is_some());
        let waiting = state.loading() && open_slot.is_none();
        // A first load that failed has no snapshot and no end: show the
        // empty state with the error rather than a skeleton forever.
        let stuck = snapshot.is_none() && state.error().is_some();
        let skeleton_wanted = waiting && !stuck;
        let diagnostics_open = self.problems_open;
        let dismissed = Dismissed {
            no_config: self.no_config_dismissed,
            state_warning: self.state_warning_dismissed.as_deref(),
        };

        let mut body = v_flex().w_full().gap(px(SPACE_3));
        body = body.child(header::header(
            &model,
            &header::HeaderProps {
                snapshot,
                open: open_slot,
            },
        ));
        body = body.children(banners::banners(
            &entity,
            &model,
            state,
            dismissed,
            &self.error_focus,
        ));
        if let Some(s) = snapshot {
            body = body.children(config_problems::config_problems(
                &entity,
                s,
                diagnostics_open,
                &self.problems_focus,
                cx,
            ));
        }
        let workspace: Option<AnyElement> = if open_slot.is_some() {
            Some(workspace_slot().into_any_element())
        } else if skeleton_wanted {
            (self.skeleton == Skeleton::Shown).then(|| skeleton().into_any_element())
        } else {
            Some(empty::empty_state(&model, snapshot, failed).into_any_element())
        };

        self.track_skeleton(skeleton_wanted, cx);

        v_flex()
            .id("db-root")
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &OpenDatabase, _, cx| this.open_shortcut(cx)))
            .on_action(cx.listener(|this, _: &ReloadDatabase, _, cx| this.reload_shortcut(cx)))
            .on_action(cx.listener(|this, _: &ShowConfigProblems, window, cx| {
                this.problems_shortcut(window, cx);
            }))
            .on_key_up(cx.listener(|this, _: &KeyUpEvent, _, _| this.latched = false))
            .size_full()
            .px(px(SPACE_5))
            .py(px(SPACE_4))
            .bg(p.bg)
            .text_color(p.text)
            .child(
                div()
                    .id("db-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.body_scroll)
                    .child(
                        v_flex()
                            .min_h_full()
                            .gap(px(SPACE_3))
                            .child(body.flex_none())
                            .children(workspace),
                    ),
            )
    }
}
