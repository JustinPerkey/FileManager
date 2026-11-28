//! The Tar Packager's keyboard shortcuts: Ctrl+O, F5 and Ctrl+R, Ctrl+E,
//! Ctrl+Enter, F8 and Escape, bound in the key context `Tarpack` (the view's
//! root) and mapped onto the model's [`Shortcut`].
//!
//! - Each handler asks `TarpackModel::shortcut` first and acts only if it says
//!   so; otherwise the key travels on (`cx.propagate()`), so a popover or a
//!   widget that wants the same key still gets it.
//! - **Auto-repeat.** The pinned GPUI hands an action no `is_held` flag (only
//!   the raw `KeyDownEvent` has one, and that reaches listeners after the
//!   action has run), so a [`RepeatLatch`] stands in: a shortcut is a repeat
//!   if the same one was seen last, nothing released a key since, and the last
//!   sighting is less than [`STALE`] ago. Key-up and modifier changes inside
//!   the view release it, so do the window losing or gaining activation, and
//!   the time bound covers a key-up that went to another window (an open
//!   dialog). The slowest keyboard repeat on Windows (a one second delay, then
//!   one repeat every 400 ms) always stays inside the bound.
//! - **Escape with the table focused.** `DataTable` binds Escape to its own
//!   "clear the selection" in its `DataTable` context, which sits below ours.
//!   A second Escape binding in `Tarpack > DataTable` (bound later, so it wins
//!   at the same depth) reaches our handler first; the handler lets the key
//!   through when there is no result to dismiss, and the table's own binding
//!   then clears its selection as before.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::prelude::*;
use gpui::{actions, App, Entity, KeyBinding, Window};

use crate::tools::tarpack::model::{Shortcut, TarpackModel};

/// The key context of the view's root.
pub const CONTEXT: &str = "Tarpack";

actions!(
    tarpack,
    [
        OpenManifest,
        ReloadManifest,
        EditManifest,
        BuildArchive,
        ShowErrorsReport,
        DismissResult
    ]
);

/// How long after the last sighting of a shortcut the same one still counts
/// as the key being held.
pub const STALE: Duration = Duration::from_millis(1100);

/// Remembers the last shortcut to tell a held key from a new press.
#[derive(Debug, Default)]
pub struct RepeatLatch {
    last: Option<(Shortcut, Instant)>,
}

impl RepeatLatch {
    /// Records a sighting of `s` at `now`; whether it is a fresh press (and
    /// so acts). An ignored repeat renews the latch, so a key held for any
    /// time stays ignored.
    pub fn admit(&mut self, s: Shortcut, now: Instant) -> bool {
        let repeat = self
            .last
            .is_some_and(|(last, at)| last == s && now.saturating_duration_since(at) < STALE);
        self.last = Some((s, now));
        !repeat
    }

    /// A key or modifier was released, or the window changed activation: the
    /// next sighting is a press.
    pub fn release(&mut self) {
        self.last = None;
    }
}

/// The latch the view shares with its handlers.
pub type SharedLatch = Rc<RefCell<RepeatLatch>>;

#[derive(Default)]
struct Bound;

impl gpui::Global for Bound {}

/// Binds the keys once per app.
pub fn ensure_bindings(cx: &mut App) {
    if cx.has_global::<Bound>() {
        return;
    }
    cx.set_global(Bound);
    let table = format!("{CONTEXT} > DataTable");
    cx.bind_keys([
        KeyBinding::new("ctrl-o", OpenManifest, Some(CONTEXT)),
        KeyBinding::new("f5", ReloadManifest, Some(CONTEXT)),
        KeyBinding::new("ctrl-r", ReloadManifest, Some(CONTEXT)),
        KeyBinding::new("ctrl-e", EditManifest, Some(CONTEXT)),
        KeyBinding::new("ctrl-enter", BuildArchive, Some(CONTEXT)),
        KeyBinding::new("f8", ShowErrorsReport, Some(CONTEXT)),
        KeyBinding::new("escape", DismissResult, Some(CONTEXT)),
        KeyBinding::new("escape", DismissResult, Some(table.as_str())),
    ]);
}

/// Runs `s` if the model allows it and the key is not a held one; otherwise
/// the key goes on.
fn act(
    model: &Entity<TarpackModel>,
    latch: &SharedLatch,
    s: Shortcut,
    window: &mut Window,
    cx: &mut App,
) {
    if !model.read(cx).shortcut(s, cx) {
        cx.propagate();
        return;
    }
    if !latch.borrow_mut().admit(s, Instant::now()) {
        return;
    }
    model.update(cx, |m, cx| m.run_shortcut(s, window, cx));
}

/// Wires the actions of the bindings onto `element`, the view's root.
pub fn attach<E: InteractiveElement>(
    element: E,
    model: &Entity<TarpackModel>,
    latch: &SharedLatch,
) -> E {
    let handler = |model: &Entity<TarpackModel>, latch: &SharedLatch, s: Shortcut| {
        let (model, latch) = (model.clone(), latch.clone());
        move |window: &mut Window, cx: &mut App| act(&model, &latch, s, window, cx)
    };
    let open = handler(model, latch, Shortcut::Open);
    let reload = handler(model, latch, Shortcut::Reload);
    let edit = handler(model, latch, Shortcut::Edit);
    let build = handler(model, latch, Shortcut::Build);
    let errors = handler(model, latch, Shortcut::ShowErrors);
    let escape = handler(model, latch, Shortcut::Escape);
    let release = latch.clone();
    let release_modifiers = latch.clone();
    element
        .on_action(move |_: &OpenManifest, window, cx| open(window, cx))
        .on_action(move |_: &ReloadManifest, window, cx| reload(window, cx))
        .on_action(move |_: &EditManifest, window, cx| edit(window, cx))
        .on_action(move |_: &BuildArchive, window, cx| build(window, cx))
        .on_action(move |_: &ShowErrorsReport, window, cx| errors(window, cx))
        .on_action(move |_: &DismissResult, window, cx| escape(window, cx))
        .on_key_up(move |_, _, _| release.borrow_mut().release())
        .on_modifiers_changed(move |_, _, _| release_modifiers.borrow_mut().release())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_repeat_is_the_same_shortcut_again_without_a_release() {
        let t0 = Instant::now();
        let mut latch = RepeatLatch::default();
        assert!(latch.admit(Shortcut::Build, t0), "the press acts");
        assert!(
            !latch.admit(Shortcut::Build, t0 + Duration::from_millis(500)),
            "the first repeat comes after the delay"
        );
        assert!(
            !latch.admit(Shortcut::Build, t0 + Duration::from_millis(900)),
            "an ignored repeat renews the latch"
        );
        assert!(
            !latch.admit(Shortcut::Build, t0 + Duration::from_millis(1800)),
            "held for any length of time"
        );
    }

    #[test]
    fn a_release_or_another_shortcut_or_time_makes_the_next_a_press() {
        let t0 = Instant::now();
        let mut latch = RepeatLatch::default();
        assert!(latch.admit(Shortcut::Escape, t0));
        latch.release();
        assert!(latch.admit(Shortcut::Escape, t0 + Duration::from_millis(10)));
        assert!(latch.admit(Shortcut::Reload, t0 + Duration::from_millis(20)));
        assert!(latch.admit(Shortcut::Escape, t0 + Duration::from_millis(30)));
        // A key-up that never arrived (a dialog had the focus): stale.
        assert!(latch.admit(Shortcut::Escape, t0 + Duration::from_millis(30) + STALE));
    }
}
