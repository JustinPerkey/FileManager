//! The session's shortcuts. The actions belong to the key context
//! `Dbmanager`, which the view's root sets, so they work wherever focus is
//! inside the view (inputs included: they are chords or function keys).

use gpui::{actions, App, KeyBinding};

pub const CONTEXT: &str = "Dbmanager";

actions!(
    dbmanager,
    [OpenDatabase, ReloadDatabase, ShowConfigProblems]
);

#[derive(Default)]
struct Bound;

impl gpui::Global for Bound {}

/// Binds the keys once per app.
pub fn ensure_bindings(cx: &mut App) {
    if cx.has_global::<Bound>() {
        return;
    }
    cx.set_global(Bound);
    cx.bind_keys([
        KeyBinding::new("ctrl-o", OpenDatabase, Some(CONTEXT)),
        KeyBinding::new("f5", ReloadDatabase, Some(CONTEXT)),
        KeyBinding::new("ctrl-r", ReloadDatabase, Some(CONTEXT)),
        KeyBinding::new("f8", ShowConfigProblems, Some(CONTEXT)),
    ]);
}
