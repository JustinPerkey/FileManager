//! The keyboard-shortcuts help: a quiet "Keyboard shortcuts" button opening a
//! non-modal popover with a table of the keys.
//!
//! The popover is gpui-component's `Popover`, which owns Escape (it closes and
//! focus returns to the button), the click outside and the anchoring. Each tool
//! supplies its own rows. [`ShortcutsHelp::on_open_change`] tells the view when
//! the popover is open, so a tool's Escape key does not also dismiss its result.

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{div, px, Anchor, App, ElementId, Role, SharedString, Window};
use gpui_component::popover::Popover;
use gpui_component::{v_flex, ThemeStyled as _};

use super::button::button;
use super::icon::IconName;
use super::kbd::{alternatives, Alternatives};
use super::text::{h2, small, small_muted};
use crate::app::probe::Tag as _;
use crate::theme::palette;
use crate::theme::tokens::{RADIUS, SPACE_1, SPACE_2, SPACE_3, SPACE_4};

/// The widest the popover gets.
const MAX_WIDTH: f32 = 576.0;
/// Share of the window height at which the table scrolls.
const MAX_HEIGHT_SHARE: f32 = 0.7;

/// One line of the table.
#[derive(Clone, Debug)]
pub struct ShortcutRow {
    /// Alternative chords, each a list of key names: `[["Ctrl", "O"]]`.
    pub keys: Vec<Vec<&'static str>>,
    pub action: SharedString,
    /// Where it applies ("In the table", "Always").
    pub when: SharedString,
}

type OpenChange = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// The help button and its popover. Build it with [`ShortcutsHelp::new`].
#[derive(IntoElement)]
pub struct ShortcutsHelp {
    id: ElementId,
    rows: Vec<ShortcutRow>,
    on_open_change: Option<OpenChange>,
}

impl ShortcutsHelp {
    /// The help for `rows`. `id` is the tag of the button; `<id>-popover` is
    /// the tag of the open popover.
    #[must_use]
    pub fn new(id: impl Into<ElementId>, rows: Vec<ShortcutRow>) -> Self {
        Self {
            id: id.into(),
            rows,
            on_open_change: None,
        }
    }

    /// Runs with `true` when the popover opens and `false` when it closes, by
    /// Escape, a click outside or the button.
    #[must_use]
    pub fn on_open_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Rc::new(handler));
        self
    }
}

fn chords(keys: &[Vec<&'static str>]) -> Alternatives {
    let refs: Vec<&[&str]> = keys.iter().map(Vec::as_slice).collect();
    alternatives(&refs)
}

impl RenderOnce for ShortcutsHelp {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = self.id;
        let rows = Rc::new(self.rows);
        let on_open_change = self.on_open_change;
        let max_height = window.viewport_size().height * MAX_HEIGHT_SHARE;
        let trigger = button(id.clone(), "Keyboard shortcuts")
            .quiet()
            .icon(IconName::Keyboard)
            .into_button(cx);

        let popover = Popover::new(SharedString::from(format!("{id}-popover-state")))
            .appearance(false)
            .anchor(Anchor::TopRight)
            .trigger(trigger)
            .when_some(on_open_change, |p, handler| {
                p.on_open_change(move |open, window, cx| handler(*open, window, cx))
            })
            .content({
                let id = id.clone();
                move |_, _, cx| {
                    let p = *palette(cx);
                    // One grid, cells row by row: each column is as wide as
                    // its widest cell. Keys never wrap; the action wraps.
                    let cells = rows.iter().enumerate().flat_map(|(i, row)| {
                        let (keys_id, action_id) =
                            (format!("{id}-keys-{i}"), format!("{id}-action-{i}"));
                        [
                            div()
                                .flex_none()
                                .whitespace_nowrap()
                                .py(px(SPACE_1))
                                .child(
                                    chords(&row.keys)
                                        .last_key_selector(format!("{id}-last-key-{i}")),
                                )
                                .debug_selector(move || keys_id.clone())
                                .into_any_element(),
                            div()
                                .py(px(SPACE_1))
                                .child(small(row.action.clone()))
                                .debug_selector(move || action_id.clone())
                                .into_any_element(),
                            div()
                                .py(px(SPACE_1))
                                .child(small_muted(row.when.clone()))
                                .into_any_element(),
                        ]
                    });
                    v_flex()
                        .popover_style(cx)
                        .bg(p.surface)
                        .border_1()
                        .border_color(p.border)
                        .rounded(px(RADIUS))
                        .py(px(SPACE_3))
                        .px(px(SPACE_4))
                        .max_w(px(MAX_WIDTH))
                        .max_h(max_height)
                        .id(SharedString::from(format!("{id}-panel")))
                        .role(Role::Dialog)
                        .aria_label("Keyboard shortcuts")
                        .child(h2("Keyboard shortcuts").mb(px(SPACE_2)))
                        .child(
                            div()
                                .id(SharedString::from(format!("{id}-rows")))
                                .grid()
                                .grid_cols_max_content(3)
                                .gap_x(px(SPACE_3))
                                .overflow_y_scroll()
                                .children(cells),
                        )
                        .debug_selector({
                            let name = format!("{id}-popover");
                            move || name.clone()
                        })
                        .tag(format!("{id}-popover"))
                }
            });

        div()
            .child(popover)
            .debug_selector({
                let name = id.to_string();
                move || name.clone()
            })
            .tag(id.to_string())
    }
}
