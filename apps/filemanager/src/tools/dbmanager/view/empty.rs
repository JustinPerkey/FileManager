//! The empty state: nothing is open.

use fm_dbmanager::tool::{DbFileSlot, DbmanagerSnapshot};
use gpui::prelude::*;
use gpui::{px, Entity};
use gpui_component::{h_flex, v_flex};

use super::header::{config_line, folder};
use crate::app::probe::Tag as _;
use crate::theme::tokens::{SPACE_1, SPACE_2, SPACE_3};
use crate::theme::INTRO_MAX_WIDTH;
use crate::tools::dbmanager::model::DbmanagerModel;
use crate::tools::dbmanager::text;
use crate::widgets::text::{body, mono_small_muted, muted, small_muted};
use crate::widgets::{button, Banner, IconName, Tone};

/// The sentence, the way to open, the recent databases, the drop hint and the
/// config line. `failed` is the database that could not be opened, with its
/// error.
pub fn empty_state(
    model: &Entity<DbmanagerModel>,
    snapshot: Option<&DbmanagerSnapshot>,
    failed: Option<&DbFileSlot>,
) -> impl IntoElement {
    let recent: &[String] = snapshot.map_or(&[], |s| s.recent.as_slice());
    let open = {
        let model = model.clone();
        button("db-empty-open", text::OPEN)
            .primary()
            .shortcut("Ctrl+O")
            .debug_selector("db-empty-open")
            .on_click(move |_, _, cx| model.update(cx, DbmanagerModel::open))
            .tag("db-empty-open")
    };
    // The recent databases as quiet buttons: the file name, its folder beside.
    let recents = (!recent.is_empty()).then(|| {
        v_flex()
            .items_start()
            .gap(px(SPACE_1))
            .child(small_muted(text::RECENT))
            .children(recent.iter().enumerate().map(|(i, path)| {
                let model = model.clone();
                let name = text::file_name(path).to_owned();
                let folder = folder(path);
                let accessible = match &folder {
                    Some(folder) => format!("{name}, {folder}"),
                    None => name.clone(),
                };
                h_flex()
                    .flex_wrap()
                    .items_center()
                    .gap_x(px(SPACE_2))
                    .child(
                        button(("db-empty-recent", i), name)
                            .quiet()
                            .icon(IconName::Folder)
                            .accessible_name(accessible)
                            .tooltip(path.clone())
                            .debug_selector(format!("db-empty-recent-{i}"))
                            .on_click(move |_, _, cx| {
                                model.update(cx, |m, cx| m.open_recent(i, cx));
                            }),
                    )
                    .children(folder.map(mono_small_muted))
            }))
    });
    let failure = failed.and_then(|slot| {
        let error = slot.error.as_ref()?;
        let config_location = snapshot.and_then(|s| s.config.display.as_deref());
        Some(
            v_flex()
                .w_full()
                .gap(px(SPACE_2))
                .child(
                    Banner::new(Tone::Error, text::error_message(error, config_location))
                        .id("db-open-failed")
                        .details(error.message.clone()),
                )
                .child(mono_small_muted(slot.display.clone()))
                .debug_selector(|| "db-open-failed".to_string()),
        )
    });
    v_flex()
        .items_start()
        .max_w(px(INTRO_MAX_WIDTH))
        .gap(px(SPACE_3))
        .children(failure)
        .child(body(text::EMPTY_LEAD))
        .child(h_flex().flex_wrap().gap(px(SPACE_2)).child(open))
        .children(recents)
        .child(muted(text::DROP_HINT))
        .children(snapshot.map(config_line))
        .debug_selector(|| "db-empty".to_string())
        .tag("db-empty")
}
