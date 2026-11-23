//! The title bar (no manifest), the manifest header and the Recent menu.

use fm_tarpack::session::SessionManifest;
use gpui::prelude::*;
use gpui::{div, px, App, Entity, FocusHandle, Window};
use gpui_component::{h_flex, v_flex};

use super::shortcuts;
use crate::app::probe::Tag as _;
use crate::theme::palette;
use crate::theme::tokens::{FONT_SIZE_SM, REM, SPACE_1, SPACE_2, SPACE_4};
use crate::tools::tarpack::model::TarpackModel;
use crate::tools::tarpack::text;
use crate::widgets::text::h1;
use crate::widgets::{button, MenuButton, MenuItem, MiddlePath, ShortcutsHelp};

/// The least width the title gets before the actions wrap under it (16rem).
const TITLE_BASIS: f32 = 256.0;

/// The shortcuts help of the Tar Packager. Its popover tells the model when it
/// is open, so Escape belongs to it.
pub fn shortcuts_help(model: &Entity<TarpackModel>, dev: bool) -> ShortcutsHelp {
    let model = model.clone();
    ShortcutsHelp::new("tarpack-shortcuts", shortcuts::rows(dev)).on_open_change(
        move |open, _window: &mut Window, cx: &mut App| {
            model.update(cx, |m, _| m.set_shortcuts_open(open));
        },
    )
}

/// The Recent menu: file name as the label, folder as the detail, the whole
/// path as the tooltip. Renders nothing when `recent` is empty.
pub fn recent_menu(model: &Entity<TarpackModel>, recent: &[String]) -> MenuButton {
    let items = recent
        .iter()
        .map(|path| MenuItem {
            label: text::file_name(path).into(),
            detail: text::display_folder(path).map(Into::into),
            tooltip: Some(path.clone().into()),
            disabled: false,
        })
        .collect();
    let model = model.clone();
    MenuButton::new("tarpack-recent", text::RECENT)
        .items(items)
        .on_select(move |index, _window, cx| {
            model.update(cx, |m, cx| m.open_recent(index, cx));
        })
}

/// The title bar of the view without a manifest.
pub fn titlebar(model: &Entity<TarpackModel>, dev: bool) -> impl IntoElement {
    h_flex()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_x(px(SPACE_4))
        .gap_y(px(SPACE_2))
        .child(h1(text::TITLE))
        .child(shortcuts_help(model, dev))
}

/// What the header needs from the view.
pub struct HeaderProps<'a> {
    pub manifest: &'a SessionManifest,
    pub recent: &'a [String],
    pub can_edit: bool,
    pub dev: bool,
    /// The Reload button's focus handle (focus hint `Reload`).
    pub reload_focus: &'a FocusHandle,
}

/// The manifest header: name and path at the left, the actions at the right.
pub fn header(model: &Entity<TarpackModel>, props: &HeaderProps, cx: &App) -> impl IntoElement {
    let p = palette(cx);
    let open = {
        let model = model.clone();
        button("tarpack-open", text::OPEN)
            .shortcut("Ctrl+O")
            .debug_selector("tarpack-open")
            .on_click(move |_, _, cx| model.update(cx, TarpackModel::open))
            .tag("tarpack-open")
    };
    let reload = {
        let model = model.clone();
        button("tarpack-reload", text::RELOAD)
            .shortcut("F5 or Ctrl+R")
            .track_focus(props.reload_focus)
            .debug_selector("tarpack-reload")
            .on_click(move |_, _, cx| model.update(cx, TarpackModel::reload))
            .tag("tarpack-reload")
    };
    let edit = props.can_edit.then(|| {
        let model = model.clone();
        button("tarpack-edit", text::EDIT_IN_EDITOR)
            .shortcut("Ctrl+E")
            .debug_selector("tarpack-edit")
            .on_click(move |_, _, cx| model.update(cx, TarpackModel::edit))
            .tag("tarpack-edit")
    });

    h_flex()
        .flex_wrap()
        .items_start()
        .justify_between()
        .gap_x(px(SPACE_4))
        .gap_y(px(SPACE_2))
        .child(
            v_flex()
                .flex_grow(1.0)
                .flex_shrink(1.0)
                .flex_basis(px(TITLE_BASIS))
                .min_w_0()
                .child(h1(props.manifest.name.clone()))
                .child(
                    div()
                        .mt(px(SPACE_1))
                        .text_size(px(FONT_SIZE_SM * REM))
                        .text_color(p.text_muted)
                        .child(MiddlePath::new(props.manifest.path.clone()).id("tarpack-path")),
                ),
        )
        .child(
            h_flex()
                .flex_wrap()
                .gap(px(SPACE_2))
                .child(open)
                .child(recent_menu(model, props.recent))
                .child(reload)
                .children(edit)
                .child(shortcuts_help(model, props.dev)),
        )
        .debug_selector(|| "tarpack-header".to_string())
        .tag("tarpack-header")
}
