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
//! Limit: releasing only `d` while still holding the modifiers does not clear
//! the latch, so re-pressing `d` then needs the modifiers released first; and a
//! window that stops producing frames while latched clears it on its next frame.

use gpui::{App, Global, Keystroke, Modifiers, Window};

/// The developer-mode flag, a global. Views and models watch it with
/// `cx.observe_global::<DevMode>`.
pub struct DevMode(bool);

impl Global for DevMode {}

/// Set while a toggled chord is still held; see the module doc.
struct ChordLatch(bool);

impl Global for ChordLatch {}

impl DevMode {
    /// Installs the global, off, and the keystroke interceptor. Call once at
    /// startup.
    pub fn init(cx: &mut App) {
        cx.set_global(DevMode(false));
        cx.set_global(ChordLatch(false));
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
            return false;
        }
        let latched = cx.try_global::<ChordLatch>().is_some_and(|l| l.0);
        if !latched {
            cx.set_global(ChordLatch(true));
            Self::toggle(cx);
            watch_release(window);
        }
        true
    }
}

fn chord_modifiers_down(m: Modifiers) -> bool {
    m.control && m.alt && m.shift
}

/// Clears the latch on the first frame where the chord modifiers are not all
/// down; until then checks again every frame.
fn watch_release(window: &Window) {
    window.on_next_frame(|window, cx| {
        if chord_modifiers_down(window.modifiers()) {
            watch_release(window);
        } else {
            cx.set_global(ChordLatch(false));
        }
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
