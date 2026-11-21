//! Developer mode: app-wide, off at every launch, toggled by
//! Ctrl+Alt+Shift+D. The chord is deliberately absent from any shortcuts list.
//!
//! It is handled by an `on_key_down` listener on the Shell rather than by a
//! `gpui::actions!` binding: an action handler cannot tell an auto-repeated
//! key from a fresh press, and a held chord must not toggle again
//! (`KeyDownEvent::is_held`). GPUI delivers element key listeners only along
//! the focused path, so the Shell tracks a focus handle of its own and takes
//! focus when the window opens: the chord then works whichever tool element
//! (or none) has focus.

use gpui::{App, Global, KeyDownEvent};

/// The developer-mode flag, a global. Views and models watch it with
/// `cx.observe_global::<DevMode>`.
pub struct DevMode(bool);

impl Global for DevMode {}

impl DevMode {
    /// Installs the global, off. Call once at startup.
    pub fn init(cx: &mut App) {
        cx.set_global(DevMode(false));
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

    /// Handles a key press: toggles on a fresh Ctrl+Alt+Shift+D and returns
    /// true; ignores everything else, and the auto-repeat of a held key.
    pub fn handle_key(event: &KeyDownEvent, cx: &mut App) -> bool {
        if !is_chord(event) {
            return false;
        }
        if !event.is_held {
            Self::toggle(cx);
        }
        true
    }
}

fn is_chord(event: &KeyDownEvent) -> bool {
    let m = event.keystroke.modifiers;
    m.control
        && m.alt
        && m.shift
        && !m.platform
        && !m.function
        && event.keystroke.key.eq_ignore_ascii_case("d")
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui::{Focusable as _, Keystroke, TestAppContext, VisualTestContext};

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

    fn key_down(cx: &mut VisualTestContext, chord: &'static str, is_held: bool) {
        cx.simulate_event(KeyDownEvent {
            keystroke: Keystroke::parse(chord).unwrap(),
            is_held,
            prefer_character_input: false,
        });
    }

    #[gpui::test]
    fn off_at_launch(cx: &mut TestAppContext) {
        let cx = open_shell(cx);
        assert!(!on(cx));
    }

    #[gpui::test]
    fn chord_toggles(cx: &mut TestAppContext) {
        let cx = open_shell(cx);
        cx.simulate_keystrokes(CHORD);
        assert!(on(cx));
        cx.simulate_keystrokes(CHORD);
        assert!(!on(cx));
        // Other chords do nothing.
        cx.simulate_keystrokes("ctrl-alt-d");
        cx.simulate_keystrokes("ctrl-shift-d");
        assert!(!on(cx));
    }

    #[gpui::test]
    fn held_chord_does_not_toggle_again(cx: &mut TestAppContext) {
        let cx = open_shell(cx);
        key_down(cx, CHORD, false);
        assert!(on(cx));
        key_down(cx, CHORD, true);
        key_down(cx, CHORD, true);
        assert!(on(cx), "auto-repeat does not toggle");
    }

    #[gpui::test]
    fn chord_works_without_focus(cx: &mut TestAppContext) {
        let (shell, cx) = open(cx, tools::build);
        // Only the Shell's own handle is focused: no tool element has focus.
        let (shell_focus, focused) =
            cx.update(|window, cx| (shell.focus_handle(cx), window.focused(cx)));
        assert_eq!(
            focused,
            Some(shell_focus),
            "only the Shell itself has focus"
        );
        cx.simulate_keystrokes(CHORD);
        assert!(on(cx));
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
        cx.simulate_keystrokes(CHORD);
        cx.simulate_keystrokes(CHORD);
        assert_eq!(seen.get(), 2);
    }
}
