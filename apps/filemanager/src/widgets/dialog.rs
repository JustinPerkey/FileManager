//! The confirmation dialog: the only modal in the app, driven by model state.
//!
//! A view calls [`sync_confirm`] whenever its model changes. `Some(spec)`
//! opens the dialog (focus on Cancel), or updates it live if it is already
//! open; `None` closes it. The dialog never closes itself: Cancel and Escape
//! call `on_cancel`, and the model answers by passing `None`.
//!
//! gpui-component's `Dialog` supplies the scrim, the focus trap (Tab cycles
//! inside), Escape and the focus return to the opener; this module supplies the
//! look of `.confirm-dialog` in `controls.css`. The shadow is gpui-component's
//! (the `--shadow-overlay` token is not applied), and a mouse press does not
//! focus a button, so after a dialog opened with the mouse focus may not return
//! to the opener.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{div, px, AnyElement, App, FocusHandle, Global, SharedString, Window, WindowId};
use gpui_component::dialog::{Dialog, DialogFooter};
use gpui_component::WindowExt as _;

use super::button::button;
use super::icon::IconName;
use super::text::{body, h2};
use crate::app::probe::Tag as _;
use crate::theme::palette;
use crate::theme::tokens::{RADIUS, SPACE_4, SPACE_5};

/// Width of the dialog.
const WIDTH: f32 = 448.0;
/// Width of a wide dialog.
const WIDE: f32 = 640.0;
/// Space kept clear between the dialog and the window's edges, each side.
const EDGE: f32 = 24.0;

/// How the confirming button is dressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmVariant {
    /// The accent-filled button.
    Primary,
    /// `danger` label and outline, for an action that destroys something.
    Danger,
}

/// The confirming button.
#[derive(Clone, Debug)]
pub struct ConfirmButton {
    /// Its visible label: the action, not "OK".
    pub label: SharedString,
    pub variant: ConfirmVariant,
    pub icon: Option<IconName>,
}

/// Builds the extra region's content each time the dialog draws.
pub type ExtraBuilder = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// What the dialog shows. The view rebuilds it from its model on every change.
#[derive(Clone)]
pub struct ConfirmSpec {
    pub title: SharedString,
    /// The short description.
    pub body: SharedString,
    /// A scrolling region below the body (the files an action touches).
    pub extra: Option<ExtraBuilder>,
    /// "Cancel" by default; "Keep editing", "Close".
    pub cancel_label: SharedString,
    /// `None`: the single-button form, only the cancel button.
    pub confirm: Option<ConfirmButton>,
    /// The confirmed work is running: the confirm button shows busy, Cancel is
    /// disabled and Escape does nothing.
    pub busy: bool,
    /// 640 px instead of 448 px.
    pub wide: bool,
}

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// One open dialog: what it shows now, and who to tell.
struct Live {
    spec: RefCell<ConfirmSpec>,
    on_confirm: RefCell<Handler>,
    on_cancel: RefCell<Handler>,
    cancel: FocusHandle,
    confirm: FocusHandle,
    open: Cell<bool>,
    /// Order of opening in the app, so a close targets the newest.
    seq: Cell<u64>,
}

#[derive(Default)]
struct Registry(HashMap<(WindowId, &'static str), Rc<Live>>);

impl Global for Registry {}

/// Shows, updates or closes the confirmation dialog `key` of this window.
///
/// Call it from a view's model observer (or any update), never from inside a
/// render. `on_confirm` and `on_cancel` replace the previous ones on every
/// call, so they may capture the latest state. Escape calls `on_cancel` unless
/// the spec is busy; the scrim does nothing. `None` closes the newest dialog,
/// so with several confirmations open it closes only if this key's is the
/// newest one opened here (a dialog opened by other code on top is not seen).
pub fn sync_confirm(
    window: &mut Window,
    cx: &mut App,
    key: &'static str,
    spec: Option<ConfirmSpec>,
    on_confirm: impl Fn(&mut Window, &mut App) + 'static,
    on_cancel: impl Fn(&mut Window, &mut App) + 'static,
) {
    let id = (window.window_handle().window_id(), key);
    let Some(spec) = spec else {
        let registry = &mut cx.default_global::<Registry>().0;
        let live = registry.remove(&id);
        // `close_dialog` closes the newest dialog, so close only when ours is
        // the newest this module opened in the window. A dialog opened by other
        // code on top of ours cannot be told apart: that one would close.
        let newest = live.as_ref().is_some_and(|l| {
            registry
                .iter()
                .all(|((w, _), o)| *w != id.0 || !o.open.get() || o.seq.get() < l.seq.get())
        });
        if newest && live.is_some_and(|l| l.open.get()) && window.has_active_dialog(cx) {
            window.close_dialog(cx);
        }
        return;
    };

    let live = if let Some(live) = cx.default_global::<Registry>().0.get(&id) {
        live.clone()
    } else {
        let live = Rc::new(Live {
            spec: RefCell::new(spec.clone()),
            on_confirm: RefCell::new(Rc::new(|_, _| {})),
            on_cancel: RefCell::new(Rc::new(|_, _| {})),
            cancel: cx.focus_handle(),
            confirm: cx.focus_handle(),
            open: Cell::new(false),
            seq: Cell::new(0),
        });
        cx.default_global::<Registry>().0.insert(id, live.clone());
        live
    };
    *live.spec.borrow_mut() = spec;
    *live.on_confirm.borrow_mut() = Rc::new(on_confirm);
    *live.on_cancel.borrow_mut() = Rc::new(on_cancel);

    if live.open.get() && window.has_active_dialog(cx) {
        // The builder runs on every draw and reads the spec above.
        window.refresh();
        return;
    }
    live.open.set(true);
    live.seq.set(next_seq(cx));
    let cancel = live.cancel.clone();
    window.open_dialog(cx, move |dialog, window, cx| {
        build(dialog, &live, window, cx)
    });
    window.defer(cx, move |window, cx| cancel.focus(window, cx));
}

fn next_seq(cx: &mut App) -> u64 {
    #[derive(Default)]
    struct Counter(u64);
    impl Global for Counter {}
    let c = cx.default_global::<Counter>();
    c.0 += 1;
    c.0
}

fn build(dialog: Dialog, live: &Rc<Live>, window: &mut Window, cx: &mut App) -> Dialog {
    let spec = live.spec.borrow().clone();
    let p = *palette(cx);
    let viewport = window.viewport_size();
    let width = if spec.wide { WIDE } else { WIDTH };
    let width = width.min((f32::from(viewport.width) - 2.0 * EDGE).max(0.0));
    let extra_max = viewport.height * 0.5;
    let busy = spec.busy;

    let cancel = {
        let live = live.clone();
        button("dialog-cancel", spec.cancel_label.clone())
            .track_focus(&live.cancel.clone())
            .disabled(busy)
            .debug_selector("dialog-cancel")
            .on_click(move |_, window, cx| {
                let handler = live.on_cancel.borrow().clone();
                handler(window, cx);
            })
            .tag("dialog-cancel")
    };
    let confirm = spec.confirm.as_ref().map(|c| {
        let live = live.clone();
        let b = button("dialog-confirm", c.label.clone());
        let b = match c.variant {
            ConfirmVariant::Primary => b.primary(),
            ConfirmVariant::Danger => b.danger(),
        };
        let b = match c.icon {
            Some(name) => b.icon(name),
            None => b,
        };
        b.busy(busy)
            .track_focus(&live.confirm.clone())
            .debug_selector("dialog-confirm")
            .on_click(move |_, window, cx| {
                let handler = live.on_confirm.borrow().clone();
                handler(window, cx);
            })
            .tag("dialog-confirm")
    });

    let extra = spec.extra.clone();
    let text = spec.body.clone();
    let on_cancel = live.on_cancel.clone();
    let mut dialog = dialog
        .w(px(width))
        .bg(p.surface)
        .border_color(p.border)
        .rounded(px(RADIUS))
        .p(px(SPACE_5))
        .gap(px(SPACE_4))
        .title(h2(spec.title.clone()).tag("dialog-title"))
        .close_button(false)
        .overlay_closable(false)
        .keyboard(!busy)
        // Escape. The model closes the dialog; the dialog never does.
        .on_cancel(move |_, window, cx| {
            let handler = on_cancel.borrow().clone();
            handler(window, cx);
            false
        })
        // Enter: gpui-component routes it to the dialog, not the focused
        // button. Do what the focused button would do, so Enter on Cancel
        // cancels and Enter on the confirming button confirms; with focus
        // anywhere else it does nothing (never a confirm by accident).
        .on_ok({
            let live = live.clone();
            move |_, window, cx| {
                let handler = if live.cancel.is_focused(window) {
                    Some(live.on_cancel.borrow().clone())
                } else if live.confirm.is_focused(window) {
                    Some(live.on_confirm.borrow().clone())
                } else {
                    None
                };
                if let Some(handler) = handler.filter(|_| !live.spec.borrow().busy) {
                    handler(window, cx);
                }
                false
            }
        })
        .content(move |content, window, cx| {
            let region = extra.as_ref().map(|build| {
                div()
                    .id("dialog-extra")
                    .mt(px(SPACE_4))
                    .max_h(extra_max)
                    .overflow_y_scroll()
                    .child(build(window, cx))
            });
            content
                .gap(px(0.0))
                .child(body(text.clone()))
                .children(region)
        });
    dialog = dialog.footer(DialogFooter::new().child(cancel).children(confirm));
    dialog
}
