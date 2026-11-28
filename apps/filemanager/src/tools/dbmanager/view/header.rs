//! The session header: the database (or the tool's name), the config line and
//! the actions.

use fm_dbmanager::config::ConfigStatus;
use fm_dbmanager::tool::{DbFileSlot, DbmanagerSnapshot};
use gpui::prelude::*;
use gpui::{div, px, Entity, IntoElement, SharedString};
use gpui_component::{h_flex, v_flex};

use super::shortcuts;
use crate::app::probe::Tag as _;
use crate::theme::tokens::{SPACE_1, SPACE_2, SPACE_4};
use crate::tools::dbmanager::model::DbmanagerModel;
use crate::tools::dbmanager::text;
use crate::widgets::button::tooltip_view;
use crate::widgets::text::{h1, mono_small_muted, small, small_muted};
use crate::widgets::{button, tag, IconName, MenuButton, MenuItem, ShortcutsHelp};

/// The least width the title gets before the actions wrap under it (16rem).
const TITLE_BASIS: f32 = 256.0;

/// The Recent menu: file name as the label, folder as the detail, the whole
/// path as the tooltip. Renders nothing when `recent` is empty.
pub fn recent_menu(model: &Entity<DbmanagerModel>, recent: &[String]) -> MenuButton {
    let items = recent
        .iter()
        .map(|path| MenuItem {
            label: text::file_name(path).to_owned().into(),
            detail: folder(path).map(Into::into),
            tooltip: Some(path.clone().into()),
            disabled: false,
        })
        .collect();
    let model = model.clone();
    MenuButton::new("db-recent", text::RECENT)
        .items(items)
        .on_select(move |index, _window, cx| {
            model.update(cx, |m, cx| m.open_recent(index, cx));
        })
}

/// The folder of a display path, `None` when it has none.
pub fn folder(path: &str) -> Option<String> {
    let name = text::file_name(path);
    let folder = path
        .strip_suffix(name)
        .map(|f| f.trim_end_matches(['\\', '/']))?;
    (!folder.is_empty()).then(|| folder.to_owned())
}

/// The config line's words, per status.
#[cfg(test)]
#[must_use]
pub fn config_line_text(s: &DbmanagerSnapshot) -> String {
    if s.config.status == ConfigStatus::Missing {
        return text::CONFIG_MISSING.to_owned();
    }
    format!(
        "{} {}",
        text::CONFIG_LABEL,
        s.config
            .display
            .as_deref()
            .unwrap_or(text::CONFIG_NO_LOCATION)
    )
}

/// The config line: "Config: <path>" with the path in mono, or the missing
/// notice.
pub fn config_line(s: &DbmanagerSnapshot) -> impl IntoElement {
    let row = h_flex().flex_wrap().gap_x(px(SPACE_1)).items_baseline();
    let row = if s.config.status == ConfigStatus::Missing {
        row.child(small(text::CONFIG_MISSING))
    } else {
        row.child(small(text::CONFIG_LABEL)).child(mono_small_muted(
            s.config
                .display
                .clone()
                .unwrap_or_else(|| text::CONFIG_NO_LOCATION.to_owned()),
        ))
    };
    row.debug_selector(|| "db-config-line".to_string())
        .tag("db-config-line")
}

/// What the header shows.
pub struct HeaderProps<'a> {
    pub snapshot: Option<&'a DbmanagerSnapshot>,
    /// The open database; `None` when nothing is open.
    pub open: Option<&'a DbFileSlot>,
}

/// The header: the title block at the left, the actions at the right.
pub fn header(model: &Entity<DbmanagerModel>, props: &HeaderProps) -> impl IntoElement {
    let recent: &[String] = props.snapshot.map_or(&[], |s| s.recent.as_slice());
    let is_open = props.open.is_some();

    let open_button = {
        let model = model.clone();
        button("db-open", text::OPEN)
            .shortcut("Ctrl+O")
            .debug_selector("db-open")
            .on_click(move |_, _, cx| model.update(cx, DbmanagerModel::open))
            .tag("db-open")
    };
    let reload = is_open.then(|| {
        let model = model.clone();
        button("db-reload", text::RELOAD)
            .shortcut("F5 or Ctrl+R")
            .debug_selector("db-reload")
            .on_click(move |_, _, cx| model.update(cx, DbmanagerModel::reload))
            .tag("db-reload")
    });
    let close = is_open.then(|| {
        let model = model.clone();
        button("db-close", text::CLOSE)
            .debug_selector("db-close")
            .on_click(move |_, _, cx| model.update(cx, DbmanagerModel::close_database))
            .tag("db-close")
    });
    let help = ShortcutsHelp::new("db-shortcuts", shortcuts::rows());

    let mut title = v_flex()
        .flex_grow(1.0)
        .flex_shrink(1.0)
        .flex_basis(px(TITLE_BASIS))
        .min_w_0()
        .gap(px(SPACE_1));
    match (props.open, props.snapshot) {
        (Some(slot), Some(s)) => {
            title = title
                .child(h1(text::file_name(&slot.display).to_owned()))
                .child(mono_small_muted(slot.display.clone()))
                .child(config_line(s));
            let mut notes = h_flex().flex_wrap().items_center().gap(px(SPACE_2));
            let mut any = false;
            if s.read_only_reason.is_some() {
                any = true;
                notes = notes.child(tag(text::READ_ONLY, Some(IconName::Lock)));
            }
            if let Some(path) = &s.backup_path {
                any = true;
                let full = SharedString::from(path.display().to_string());
                let name = path
                    .file_name()
                    .map_or_else(|| full.to_string(), |n| n.to_string_lossy().into_owned());
                notes = notes.child(
                    div()
                        .id("db-backup-line")
                        .tooltip(tooltip_view(full))
                        .child(small_muted(text::backup_line(&name)))
                        .debug_selector(|| "db-backup-line".to_string()),
                );
            }
            if any {
                title = title.child(notes);
            }
        }
        _ => title = title.child(h1(text::TITLE)),
    }

    h_flex()
        .flex_wrap()
        .items_start()
        .justify_between()
        .gap_x(px(SPACE_4))
        .gap_y(px(SPACE_2))
        .child(title)
        .child(
            h_flex()
                .flex_wrap()
                .gap(px(SPACE_2))
                .child(open_button)
                .child(recent_menu(model, recent))
                .children(reload)
                .children(close)
                .child(help),
        )
        .debug_selector(|| "db-header".to_string())
        .tag("db-header")
}
