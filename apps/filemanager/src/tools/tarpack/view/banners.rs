//! The banners above the manifest: a command error, "changed on disk" and the
//! state warning.

use gpui::prelude::*;
use gpui::{div, px, Entity, IntoElement, SharedString};
use gpui_component::v_flex;

use crate::app::probe::Tag as _;
use crate::theme::tokens::SPACE_2;
use crate::tools::tarpack::model::TarpackModel;
use crate::tools::tarpack::text;
use crate::widgets::{button, Banner, Tone};

/// The banners the model asks for, in order, 8 px apart; `None` when there is
/// none. The changed-on-disk banner belongs to a loaded manifest.
pub fn banners(
    model: &Entity<TarpackModel>,
    state: &TarpackModel,
    has_manifest: bool,
) -> Option<impl IntoElement> {
    let mut column = v_flex().gap(px(SPACE_2));
    let mut any = false;

    if let Some(error) = state.error() {
        any = true;
        let dismiss = model.clone();
        let detail = error.detail();
        let banner = Banner::new(Tone::Error, error.copy())
            // A new generation is a new banner, with its Details closed.
            .id(SharedString::from(format!(
                "tarpack-error-{}",
                state.error_generation()
            )))
            .dismiss(move |_, cx| dismiss.update(cx, TarpackModel::dismiss_error));
        let banner = if detail.is_empty() {
            banner
        } else {
            banner.details(detail.to_string())
        };
        column = column.child(
            div()
                .child(banner)
                .debug_selector(|| "tarpack-error".to_string())
                .tag("tarpack-error"),
        );
    }

    if has_manifest && state.changed_on_disk() {
        any = true;
        let reload = model.clone();
        column = column.child(
            div()
                .child(
                    Banner::new(Tone::Warn, text::CHANGED_ON_DISK)
                        .id("tarpack-changed-banner")
                        .action(
                            button("tarpack-changed-reload", text::CHANGED_ACTION)
                                .debug_selector("tarpack-changed-reload")
                                .on_click(move |_, _, cx| {
                                    reload.update(cx, TarpackModel::reload);
                                }),
                        ),
                )
                .debug_selector(|| "tarpack-changed".to_string())
                .tag("tarpack-changed"),
        );
    }

    if let Some(warning) = state.state_warning() {
        any = true;
        let dismiss = model.clone();
        column = column.child(
            div().child(
                Banner::new(Tone::Info, warning.to_string())
                    .id("tarpack-state-warning")
                    .dismiss(move |_, cx| {
                        dismiss.update(cx, TarpackModel::dismiss_state_warning);
                    }),
            ),
        );
    }

    any.then_some(column)
}
