//! The banners under the header, in a fixed order: a command error, the config
//! failed closed, the config changed on disk, a read-only
//! database and the state warning.

use fm_dbmanager::config::ConfigStatus;
use fm_dbmanager::tool::DbmanagerSnapshot;
use gpui::prelude::*;
use gpui::{div, px, Entity, FocusHandle, IntoElement, SharedString};
use gpui_component::v_flex;

use super::DbmanagerView;
use crate::app::probe::Tag as _;
use crate::theme::tokens::SPACE_2;
use crate::tools::dbmanager::model::DbmanagerModel;
use crate::tools::dbmanager::text;
use crate::widgets::{button, Banner, Tone};

/// One banner, named for the order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Error,
    ConfigFailedClosed,
    ConfigChanged,
    ReadOnlyDatabase,
    StateWarning,
}

/// What the view remembers having dismissed.
#[derive(Clone, Copy, Debug, Default)]
pub struct Dismissed<'a> {
    /// The state warning's text, when dismissed.
    pub state_warning: Option<&'a str>,
}

/// The banners to show, in the fixed order.
#[must_use]
pub fn plan(
    has_error: bool,
    snapshot: Option<&DbmanagerSnapshot>,
    dismissed: Dismissed,
) -> Vec<Kind> {
    let mut kinds = Vec::new();
    if has_error {
        kinds.push(Kind::Error);
    }
    let Some(s) = snapshot else { return kinds };
    if s.config_fail_closed.is_some() || s.config.status == ConfigStatus::FailClosed {
        kinds.push(Kind::ConfigFailedClosed);
    }
    if s.config_changed {
        kinds.push(Kind::ConfigChanged);
    }
    if s.open && s.read_only_reason.is_some() {
        kinds.push(Kind::ReadOnlyDatabase);
    }
    if let Some(warning) = &s.state_warning {
        if dismissed.state_warning != Some(warning.as_str()) {
            kinds.push(Kind::StateWarning);
        }
    }
    kinds
}

/// The banners, 8 px apart; `None` when there is none.
pub fn banners(
    view: &Entity<DbmanagerView>,
    model: &Entity<DbmanagerModel>,
    state: &DbmanagerModel,
    dismissed: Dismissed,
    error_focus: &FocusHandle,
) -> Option<impl IntoElement> {
    let snapshot = state.snapshot();
    let kinds = plan(state.error().is_some(), snapshot, dismissed);
    if kinds.is_empty() {
        return None;
    }
    let mut column = v_flex().w_full().gap(px(SPACE_2));
    for kind in kinds {
        column = column.child(match kind {
            Kind::Error => error(model, state, error_focus).into_any_element(),
            Kind::ConfigFailedClosed => {
                let notice = snapshot.and_then(text::config_notice).unwrap_or_default();
                wrap(
                    "db-banner-config-closed",
                    Banner::new(Tone::Error, notice).id("db-config-closed-banner"),
                )
            }
            Kind::ConfigChanged => {
                let model = model.clone();
                wrap(
                    "db-banner-config-changed",
                    Banner::new(Tone::Warn, text::CONFIG_CHANGED)
                        .id("db-config-changed-banner")
                        .action(
                            button("db-config-changed-reload", text::RELOAD)
                                .debug_selector("db-config-changed-reload")
                                .on_click(move |_, _, cx| {
                                    model.update(cx, DbmanagerModel::reload);
                                }),
                        ),
                )
            }
            Kind::ReadOnlyDatabase => {
                let reason = snapshot
                    .and_then(|s| s.read_only_reason.as_deref())
                    .unwrap_or_default();
                wrap(
                    "db-banner-read-only",
                    Banner::new(Tone::Warn, text::database_read_only(reason))
                        .id("db-read-only-banner"),
                )
            }
            Kind::StateWarning => {
                let warning = snapshot
                    .and_then(|s| s.state_warning.clone())
                    .unwrap_or_default();
                let view = view.clone();
                let remembered = warning.clone();
                wrap(
                    "db-banner-state-warning",
                    Banner::new(Tone::Warn, warning)
                        .id("db-state-warning-banner")
                        .dismiss(move |_, cx| {
                            let remembered = remembered.clone();
                            view.update(cx, |v, cx| {
                                v.state_warning_dismissed = Some(remembered);
                                cx.notify();
                            });
                        }),
                )
            }
        });
    }
    Some(column)
}

fn wrap(name: &'static str, banner: Banner) -> gpui::AnyElement {
    div()
        .child(banner)
        .debug_selector(move || name.to_string())
        .into_any_element()
}

/// The command error: Details, dismissible, a new banner for each generation.
fn error(
    model: &Entity<DbmanagerModel>,
    state: &DbmanagerModel,
    focus: &FocusHandle,
) -> impl IntoElement {
    let Some(error) = state.error() else {
        return div().into_any_element();
    };
    let dismiss = model.clone();
    let detail = error.detail();
    let banner = Banner::new(Tone::Error, error.copy())
        .id(SharedString::from(format!(
            "db-error-{}",
            state.error_generation()
        )))
        .dismiss(move |_, cx| dismiss.update(cx, DbmanagerModel::dismiss_error));
    let banner = if detail.is_empty() {
        banner
    } else {
        banner.details(detail.to_string())
    };
    div()
        .id("db-error-focus")
        .track_focus(focus)
        .child(banner)
        .debug_selector(|| "db-error".to_string())
        .tag("db-error")
        .into_any_element()
}
