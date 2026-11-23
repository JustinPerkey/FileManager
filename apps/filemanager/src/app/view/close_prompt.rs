//! The "close with unsaved changes" prompt, shown through the shared confirm
//! dialog while the Shell holds a close request. Keep editing is focused;
//! Escape also keeps editing.

use gpui::{App, Entity, SharedString, Window};

use crate::app::guard::CloseRequest;
use crate::app::shell::Shell;
use crate::widgets::{sync_confirm, ConfirmButton, ConfirmSpec, ConfirmVariant};

/// The dialog's title.
pub const TITLE: &str = "Close FileManager with unsaved changes?";
/// The cancel button.
pub const KEEP_EDITING: &str = "Keep editing";
/// The confirming button.
pub const CLOSE_WITHOUT_SAVING: &str = "Close without saving";

/// What the dialog shows for `request`: one body line per unsaved item.
#[must_use]
pub fn spec(request: &CloseRequest) -> ConfirmSpec {
    let body = request
        .items
        .iter()
        .map(|(_, item)| item.as_ref())
        .collect::<Vec<_>>()
        .join("\n");
    ConfirmSpec {
        title: TITLE.into(),
        body: SharedString::from(body),
        extra: None,
        cancel_label: KEEP_EDITING.into(),
        confirm: Some(ConfirmButton {
            label: CLOSE_WITHOUT_SAVING.into(),
            variant: ConfirmVariant::Danger,
            icon: None,
        }),
        busy: false,
        wide: false,
    }
}

/// Opens, updates or closes the prompt to match the shell. Call it when the
/// shell changes, never from a render.
pub fn sync(shell: &Entity<Shell>, window: &mut Window, cx: &mut App) {
    let spec = shell.read(cx).close_request().map(spec);
    let on_confirm = shell.downgrade();
    let on_cancel = shell.downgrade();
    sync_confirm(
        window,
        cx,
        "close-prompt",
        spec,
        move |window: &mut Window, cx: &mut App| {
            on_confirm
                .update(cx, |shell, cx| shell.confirm_close(window, cx))
                .ok();
        },
        move |_window: &mut Window, cx: &mut App| {
            on_cancel.update(cx, Shell::cancel_close).ok();
        },
    );
}
