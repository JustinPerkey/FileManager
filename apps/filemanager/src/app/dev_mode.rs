//! Developer mode: app-wide, off at every launch, toggled by
//! Ctrl+Alt+Shift+D. The chord is deliberately absent from any shortcuts list.
//!
//! The chord is caught by an app-level keystroke interceptor
//! (`App::intercept_keystrokes`, installed once in [`DevMode::init`]). GPUI
//! delivers element key listeners only along the focused path, so a listener on
//! the Shell would miss the chord when nothing is focused, when the focused
//! element has stopped rendering, or when focus is in a dialog or sheet (the
//! `Root` renders those as siblings of the Shell). The interceptor sees every
//! key-down before the focus path and consumes only the chord.
//!
//! Held chord: the pinned GPUI (`gpui-pre` 0.3.12) does not pass `is_held` to
//! interceptors (`KeystrokeEvent` has no such field), so a **latch** is used:
//! the chord toggles only while the latch is clear, then sets it. Key-up and
//! modifier-change listeners only run along the focused path, so the latch is
//! released by polling `Window::modifiers()` (which GPUI updates for every
//! modifiers-changed event whatever has focus) once per frame while the latch
//! is set, and clearing it as soon as Ctrl, Alt or Shift is no longer down.
//! A non-chord key-down resets a latch whose modifiers were never seen down,
//! and the poll gives up after one second of never seeing them down (a time
//! bound, so it does not depend on the display's frame rate). Each latch has a
//! generation, so a stale poll chain stops instead of running beside a new one.
//! Limit: releasing only `d` while still holding the modifiers does not clear
//! the latch, so re-pressing `d` then needs the modifiers released first; and a
//! window that stops producing frames while latched clears it on its next frame.
//! The latch only clears after a frame has seen the modifiers down, so a
//! key-down that arrives before its modifiers-changed event does not clear it.
//! The latch is global but polled on one window, which is fine while the app
//! has a single window. An `AltGr` layout that turns the chord into a character
//! sends `WM_CHAR` to a focused input regardless of the interceptor.

use std::time::{Duration, Instant};

use gpui::{App, Global, Keystroke, Modifiers, Window};

/// The developer-mode flag, a global. Views and models watch it with
/// `cx.observe_global::<DevMode>`.
pub struct DevMode(bool);

impl Global for DevMode {}

/// `held` is set while a toggled chord is still held; `seen_down` records that
/// the per-frame poll has seen the chord modifiers down since then. See the
/// module doc.
struct ChordLatch {
    held: bool,
    seen_down: bool,
    /// When the latch was set; the poll gives up `UNSEEN_TIMEOUT` after it if
    /// it never saw the modifiers down.
    since: Instant,
    /// Identifies the poll chain that owns the latch. Every new latch and
    /// every reset moves it on, so an older chain stops at its next frame and
    /// two chains never run at once.
    generation: u64,
}

/// How long the poll waits to see the chord modifiers down before giving up
/// and clearing the latch. A time bound, not a frame count: at 240 Hz 60
/// frames is 250 ms, shorter than a key-repeat delay.
const UNSEEN_TIMEOUT: Duration = Duration::from_secs(1);

impl Global for ChordLatch {}

impl ChordLatch {
    fn idle(generation: u64) -> Self {
        Self {
            held: false,
            seen_down: false,
            since: Instant::now(),
            generation,
        }
    }

    /// Releases the latch and retires any poll chain still pending.
    fn reset(cx: &mut App) {
        let generation = cx.try_global::<ChordLatch>().map_or(0, |l| l.generation);
        cx.set_global(ChordLatch::idle(generation + 1));
    }
}

impl DevMode {
    /// Installs the global, off, and the keystroke interceptor. Call once at
    /// startup.
    pub fn init(cx: &mut App) {
        cx.set_global(DevMode(false));
        cx.set_global(ChordLatch::idle(0));
        cx.intercept_keystrokes(|event, window, cx| {
            if Self::handle_keystroke(&event.keystroke, window, cx) {
                cx.stop_propagation();
            }
        })
        .detach();
    }

    /// Whether developer mode is on.
    #[must_use]
    pub fn is_on(cx: &App) -> bool {
        cx.try_global::<DevMode>().is_some_and(|d| d.0)
    }

    /// Flips developer mode and notifies global observers.
    pub fn toggle(cx: &mut App) {
        let on = Self::is_on(cx);
        cx.set_global(DevMode(!on));
    }

    /// The single place that decides "toggle or not". Toggles on a fresh
    /// Ctrl+Alt+Shift+D and returns true; returns true without toggling for the
    /// repeat of a held chord; returns false for every other keystroke.
    pub fn handle_keystroke(keystroke: &Keystroke, window: &mut Window, cx: &mut App) -> bool {
        if !is_chord(keystroke) {
            // A latch whose modifiers were never seen down is stale.
            if cx
                .try_global::<ChordLatch>()
                .is_some_and(|l| l.held && !l.seen_down)
            {
                ChordLatch::reset(cx);
            }
            return false;
        }
        let latched = cx.try_global::<ChordLatch>().is_some_and(|l| l.held);
        if !latched {
            let generation = cx.try_global::<ChordLatch>().map_or(0, |l| l.generation) + 1;
            cx.set_global(ChordLatch {
                held: true,
                seen_down: chord_modifiers_down(window.modifiers()),
                since: Instant::now(),
                generation,
            });
            Self::toggle(cx);
            watch_release(window, generation);
        }
        true
    }
}

fn chord_modifiers_down(m: Modifiers) -> bool {
    m.control && m.alt && m.shift
}

/// Clears the latch on the first frame where the chord modifiers are not all
/// down, but only after a frame has seen them down (a key-down can arrive
/// before its modifiers-changed event). Until then it checks every frame, for
/// at most `UNSEEN_TIMEOUT`. It stops when the latch is released or a newer
/// chain (`generation`) owns it.
fn watch_release(window: &Window, generation: u64) {
    window.on_next_frame(move |window, cx| {
        let Some(latch) = cx.try_global::<ChordLatch>() else {
            return;
        };
        if !latch.held || latch.generation != generation {
            return;
        }
        let (seen_down, since) = (latch.seen_down, latch.since);
        if chord_modifiers_down(window.modifiers()) {
            if !seen_down {
                cx.global_mut::<ChordLatch>().seen_down = true;
            }
        } else if seen_down || since.elapsed() >= UNSEEN_TIMEOUT {
            ChordLatch::reset(cx);
            return;
        }
        watch_release(window, generation);
    });
}

fn is_chord(keystroke: &Keystroke) -> bool {
    let m = keystroke.modifiers;
    m.control
        && m.alt
        && m.shift
        && !m.platform
        && !m.function
        && keystroke.key.eq_ignore_ascii_case("d")
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui::{
        AppContext as _, FocusHandle, Focusable as _, KeyDownEvent, ModifiersChangedEvent,
        ParentElement as _, TestAppContext, VisualTestContext,
    };
    use gpui_component::input::{Input, InputState};
    use gpui_component::WindowExt as _;

    use super::*;
    use crate::app::shell::test_support::open;
    use crate::tools;

    const CHORD: &str = "ctrl-alt-shift-d";

    fn open_shell(cx: &mut TestAppContext) -> &mut VisualTestContext {
        open(cx, tools::build).1
    }

    fn on(cx: &mut VisualTestContext) -> bool {
        cx.update(|_, cx| DevMode::is_on(cx))
    }

    fn modifiers_changed(cx: &mut VisualTestContext, modifiers: Modifiers) {
        cx.simulate_event(ModifiersChangedEvent {
            modifiers,
            capslock: gpui::Capslock::default(),
        });
    }

    fn all_down() -> Modifiers {
        Modifiers {
            control: true,
            alt: true,
            shift: true,
            ..Modifiers::default()
        }
    }

    /// The modifiers go down and the chord key goes down; nothing is released.
    fn press(cx: &mut VisualTestContext) {
        modifiers_changed(cx, all_down());
        key_down(cx, false);
    }

    fn key_down(cx: &mut VisualTestContext, is_held: bool) {
        cx.simulate_event(KeyDownEvent {
            keystroke: Keystroke::parse(CHORD).unwrap(),
            is_held,
            prefer_character_input: false,
        });
    }

    /// The modifiers come up, and the next frame is delivered.
    fn release(cx: &mut VisualTestContext) {
        modifiers_changed(cx, Modifiers::default());
        cx.update(|window, cx| {
            window.simulate_next_frame(cx);
        });
    }

    fn tap(cx: &mut VisualTestContext) {
        press(cx);
        release(cx);
    }

    #[gpui::test]
    fn off_at_launch(cx: &mut TestAppContext) {
        let cx = open_shell(cx);
        assert!(!on(cx));
    }

    #[gpui::test]
    fn chord_toggles(cx: &mut TestAppContext) {
        let cx = open_shell(cx);
        tap(cx);
        assert!(on(cx));
        tap(cx);
        assert!(!on(cx));
        // Other chords do nothing.
        cx.simulate_keystrokes("ctrl-alt-d");
        cx.simulate_keystrokes("ctrl-shift-d");
        cx.simulate_keystrokes("ctrl-alt-shift-x");
        cx.simulate_keystrokes("ctrl-alt-shift-cmd-d");
        assert!(!on(cx));
    }

    #[gpui::test]
    fn held_chord_toggles_once(cx: &mut TestAppContext) {
        let cx = open_shell(cx);
        press(cx);
        assert!(on(cx));
        key_down(cx, true);
        key_down(cx, true);
        key_down(cx, false);
        assert!(on(cx), "a held chord toggles once");
        release(cx);
        press(cx);
        assert!(!on(cx), "after the release a new chord toggles again");
    }

    #[gpui::test]
    fn latch_survives_a_key_down_before_its_modifiers(cx: &mut TestAppContext) {
        let cx = open_shell(cx);
        // The key-down arrives before any modifiers-changed event.
        key_down(cx, false);
        assert!(on(cx));
        cx.update(|window, cx| {
            window.simulate_next_frame(cx);
        });
        key_down(cx, true);
        assert!(
            on(cx),
            "the latch was not cleared by a frame with no modifiers seen"
        );
        // The modifiers arrive, a frame sees them, then they are released.
        modifiers_changed(cx, all_down());
        cx.update(|window, cx| {
            window.simulate_next_frame(cx);
        });
        release(cx);
        key_down(cx, false);
        assert!(!on(cx));
    }

    fn frame(cx: &mut VisualTestContext) {
        cx.update(|window, cx| {
            window.simulate_next_frame(cx);
        });
    }

    #[gpui::test]
    fn stale_latch_is_reset_by_a_non_chord_key(cx: &mut TestAppContext) {
        let cx = open_shell(cx);
        // Key-down with the modifiers never seen down, then a frame.
        key_down(cx, false);
        assert!(on(cx));
        frame(cx);
        modifiers_changed(cx, Modifiers::default());
        cx.simulate_keystrokes("x");
        key_down(cx, false);
        assert!(!on(cx), "the next chord toggles");
    }

    #[gpui::test]
    fn unseen_latch_clears_after_the_time_bound(cx: &mut TestAppContext) {
        let cx = open_shell(cx);
        key_down(cx, false);
        assert!(on(cx));
        // Many frames inside the bound do not clear it, however fast they come.
        for _ in 0..1000 {
            frame(cx);
        }
        key_down(cx, true);
        assert!(on(cx), "still latched inside the time bound");
        // Pretend the latch is older than the bound.
        cx.update(|_, cx| {
            cx.global_mut::<ChordLatch>().since = Instant::now()
                .checked_sub(UNSEEN_TIMEOUT * 2)
                .expect("clock far enough from its origin");
        });
        frame(cx);
        // The poll has stopped: no callbacks remain.
        let ran = cx.update(gpui::Window::simulate_next_frame);
        assert_eq!(ran, 0);
        key_down(cx, false);
        assert!(!on(cx), "latch cleared at the bound");
    }

    #[gpui::test]
    fn a_stale_poll_chain_stops_when_a_new_latch_starts(cx: &mut TestAppContext) {
        let cx = open_shell(cx);
        key_down(cx, false); // chain A
        cx.simulate_keystrokes("x"); // resets the stale latch
        key_down(cx, false); // chain B, while A's callback is still pending
        let both = cx.update(gpui::Window::simulate_next_frame);
        assert_eq!(both, 2, "both callbacks were pending");
        let after = cx.update(gpui::Window::simulate_next_frame);
        assert_eq!(after, 1, "only the newest chain reschedules");
    }

    #[gpui::test]
    fn chord_works_without_focus(cx: &mut TestAppContext) {
        let cx = open_shell(cx);
        cx.update(gpui::Window::blur);
        cx.run_until_parked();
        assert!(cx.update(|window, cx| window.focused(cx)).is_none());
        tap(cx);
        assert!(on(cx));
    }

    #[gpui::test]
    fn chord_works_after_focus_is_lost(cx: &mut TestAppContext) {
        let cx = open_shell(cx);
        // Focus an element that is not (or no longer) in the rendered tree.
        let orphan: FocusHandle = cx.update(|_, cx| cx.focus_handle());
        cx.update(|window, cx| orphan.focus(window, cx));
        cx.run_until_parked();
        tap(cx);
        assert!(on(cx));
    }

    #[gpui::test]
    fn chord_works_with_focus_in_a_dialog(cx: &mut TestAppContext) {
        let cx = open_shell(cx);
        let input = cx.update(|window, cx| cx.new(|cx| InputState::new(window, cx)));
        let input_in = input.clone();
        let first = Cell::new(true);
        cx.update(|window, cx| {
            window.open_dialog(cx, move |dialog, window, cx| {
                if first.replace(false) {
                    input_in.read(cx).focus_handle(cx).focus(window, cx);
                }
                dialog.title("Test").child(Input::new(&input_in))
            });
        });
        cx.run_until_parked();
        let focused_in_input =
            cx.update(|window, cx| input.read(cx).focus_handle(cx).is_focused(window));
        assert!(focused_in_input, "focus is inside the dialog's input");
        cx.simulate_keystrokes("a");
        press(cx);
        assert!(on(cx));
        let text = cx.update(|_, cx| input.read(cx).value());
        assert_eq!(text.as_ref(), "a", "the input received no d");
    }

    #[gpui::test]
    fn observers_are_notified(cx: &mut TestAppContext) {
        let cx = open_shell(cx);
        let seen = Rc::new(Cell::new(0));
        let seen_in = seen.clone();
        cx.update(|_, cx| {
            cx.observe_global::<DevMode>(move |_| seen_in.set(seen_in.get() + 1))
                .detach();
        });
        tap(cx);
        tap(cx);
        assert_eq!(seen.get(), 2);
    }
}
