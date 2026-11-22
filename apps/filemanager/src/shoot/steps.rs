//! Playing a scenario's [`Step`]s against a window. Everything here is
//! synchronous and works on a real window and on a test window alike: input
//! goes through `Window::dispatch_event`, and a selector resolves through the
//! tag registry ([`crate::app::probe`]). Waiting (`WaitIdle`, `Wait`) is the
//! caller's: it differs between the real run loop and a test context.

use std::path::PathBuf;

use gpui::{
    point, AnyWindowHandle, App, Entity, ExternalPaths, FileDropEvent, Keystroke, Modifiers,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Window,
};
use gpui_component::Root;

use super::Step;
use crate::app::probe;
use crate::app::shell::Shell;

/// The shell inside `handle`'s `Root`.
pub fn shell_of(handle: AnyWindowHandle, cx: &mut App) -> Result<Entity<Shell>, String> {
    let root = handle
        .downcast::<Root>()
        .ok_or("the window's root is not a Root")?;
    root.update(cx, |root, _, _| {
        root.view().clone().downcast::<Shell>().ok()
    })
    .map_err(|e| format!("window is gone: {e:#}"))?
    .ok_or_else(|| "the window's root view is not the Shell".to_string())
}

/// The centre of the element tagged `selector`, or an error naming it.
pub fn center_of(selector: &str) -> Result<Point<Pixels>, String> {
    probe::bounds(selector)
        .map(|b| b.center())
        .ok_or_else(|| format!("tag not found: {selector}"))
}

fn paths(list: &[&str]) -> ExternalPaths {
    ExternalPaths(list.iter().map(PathBuf::from).collect())
}

fn dispatch(window: &mut Window, event: impl gpui::InputEvent, cx: &mut App) {
    let _ = window.dispatch_event(event.to_platform_input(), cx);
}

fn inside(window: &Window) -> Point<Pixels> {
    let size = window.viewport_size();
    point(size.width / 2.0, size.height / 2.0)
}

/// Plays one step. `WaitIdle` and `Wait` do nothing here.
///
/// # Errors
/// A selector that does not resolve, a keystroke that does not parse, a
/// missing tool or a window that is gone, with the reason.
pub fn apply(step: &Step, handle: AnyWindowHandle, cx: &mut App) -> Result<(), String> {
    match step {
        Step::WaitIdle | Step::Wait(_) => Ok(()),
        Step::SelectTool(id) => {
            let shell = shell_of(handle, cx)?;
            on_window(handle, cx, |window, cx| {
                shell.update(cx, |shell, cx| shell.select(*id, window, cx));
                Ok(())
            })
        }
        Step::Click(_) | Step::Keys(_) | Step::Type(_) => input(step, handle, cx),
        Step::DragEnter(list) => on_window(handle, cx, |window, cx| {
            let position = inside(window);
            dispatch(
                window,
                FileDropEvent::Entered {
                    position,
                    paths: paths(list),
                },
                cx,
            );
            dispatch(window, FileDropEvent::Pending { position }, cx);
            Ok(())
        }),
        Step::Drop(list) => on_window(handle, cx, |window, cx| {
            let position = inside(window);
            dispatch(
                window,
                FileDropEvent::Entered {
                    position,
                    paths: paths(list),
                },
                cx,
            );
            dispatch(window, FileDropEvent::Pending { position }, cx);
            dispatch(window, FileDropEvent::Submit { position }, cx);
            Ok(())
        }),
        Step::Watcher(id) => {
            let shell = shell_of(handle, cx)?;
            on_window(handle, cx, |window, cx| {
                shell.update(cx, |shell, cx| {
                    let slot = shell
                        .tools()
                        .iter()
                        .find(|s| s.id == *id)
                        .ok_or_else(|| format!("no such tool: {}", id.id()))?;
                    slot.hooks.inject_file_changed(window, cx);
                    Ok(())
                })
            })
        }
        Step::Call(f) => on_window(handle, cx, |window, cx| {
            f(window, cx);
            Ok(())
        }),
    }
}

/// The pointer and keyboard steps.
fn input(step: &Step, handle: AnyWindowHandle, cx: &mut App) -> Result<(), String> {
    match step {
        Step::Click(selector) => {
            let at = center_of(selector)?;
            on_window(handle, cx, |window, cx| {
                dispatch(
                    window,
                    MouseMoveEvent {
                        position: at,
                        pressed_button: None,
                        modifiers: Modifiers::none(),
                    },
                    cx,
                );
                dispatch(
                    window,
                    MouseDownEvent {
                        button: MouseButton::Left,
                        position: at,
                        modifiers: Modifiers::none(),
                        click_count: 1,
                        first_mouse: false,
                    },
                    cx,
                );
                dispatch(
                    window,
                    MouseUpEvent {
                        button: MouseButton::Left,
                        position: at,
                        modifiers: Modifiers::none(),
                        click_count: 1,
                    },
                    cx,
                );
                Ok(())
            })
        }
        Step::Keys(keys) => {
            let parsed = keys
                .split_whitespace()
                .map(|k| Keystroke::parse(k).map_err(|e| format!("bad keystroke {k:?}: {e}")))
                .collect::<Result<Vec<_>, _>>()?;
            on_window(handle, cx, |window, cx| {
                for keystroke in parsed {
                    window.dispatch_keystroke(keystroke, cx);
                }
                Ok(())
            })
        }
        Step::Type(text) => on_window(handle, cx, |window, cx| {
            for c in text.chars() {
                let key = c.to_string();
                window.dispatch_keystroke(
                    Keystroke {
                        modifiers: Modifiers::none(),
                        key: key.to_lowercase(),
                        key_char: Some(key),
                    },
                    cx,
                );
            }
            Ok(())
        }),
        _ => Ok(()),
    }
}

fn on_window(
    handle: AnyWindowHandle,
    cx: &mut App,
    f: impl FnOnce(&mut Window, &mut App) -> Result<(), String>,
) -> Result<(), String> {
    handle
        .update(cx, |_, window, cx| f(window, cx))
        .map_err(|e| format!("window is gone: {e:#}"))?
}
