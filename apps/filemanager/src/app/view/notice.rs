//! The shell's notice: a warn banner at the top of the tool area, inset like
//! a tool view, until dismissed.

use gpui::prelude::*;
use gpui::{div, px, App, Entity};

use crate::app::probe::Tag as _;
use crate::app::shell::{Shell, ShellNotice};
use crate::theme::tokens::{SPACE_4, SPACE_5};
use crate::widgets::{Banner, Tone};

/// The notice banner of `notice`.
pub fn notice(notice: ShellNotice, shell: &Entity<Shell>) -> impl IntoElement {
    let shell = shell.clone();
    div()
        .flex_none()
        .px(px(SPACE_5))
        .pt(px(SPACE_4))
        .child(
            Banner::new(Tone::Warn, notice.message())
                .id("shell-notice-banner")
                .dismiss(move |_window: &mut gpui::Window, cx: &mut App| {
                    shell.update(cx, Shell::dismiss_notice);
                }),
        )
        .debug_selector(|| "shell-notice".to_string())
        .tag("shell-notice")
}
