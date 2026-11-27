//! The entry table: today's ruled sheet on gpui-component's virtual
//! `DataTable`, one fixed row height, scrolling inside itself.
//!
//! `EntryTable` is the view the frame hosts. It owns the table state; the
//! delegate reads the entries from the model on demand, so 10,000 rows cost
//! only the rows on screen.
//!
//! Keys (the table is one Tab stop): Up, Down, Home, End and the page keys are
//! the `DataTable`'s own; Enter and Delete are ours (Browse and Clear for the
//! selected row); Left and Right scroll sideways. While a build runs the
//! selection keys are unbound (`building` in the key context) and the buttons
//! are disabled; the model refuses Browse and Clear as well.

use fm_tarpack::session::SessionEntry;
use gpui::prelude::*;
use gpui::{
    actions, div, point, px, relative, AnyElement, App, Context, Edges, Entity, FocusHandle,
    Focusable, FontWeight, KeyBinding, KeyContext, NoAction, Pixels, Render, Stateful, Window,
};
use gpui_component::table::{Column, DataTable, TableDelegate, TableState};
use gpui_component::{v_flex, ElementExt as _, FocusableExt as _, Sizable as _, Size};

use super::entry_cells::{self, Actions, HEADINGS};
use crate::app::probe::Tag as _;
use crate::theme::tokens::{FONT_SIZE_MD, FONT_SIZE_SM, LINE_HEIGHT, REM, SPACE_1, SPACE_2};
use crate::theme::{palette, INTRO_MAX_WIDTH};
use crate::tools::tarpack::model::TarpackModel;
use crate::widgets::tabular;

/// Column widths: Status, Mode, Owner and Actions are fixed.
pub const STATUS_WIDTH: f32 = 116.0;
pub const MODE_WIDTH: f32 = 132.0;
pub const OWNER_WIDTH: f32 = 80.0;
pub const ACTIONS_WIDTH: f32 = 176.0;
/// File, Windows location and Linux target share what is left equally while
/// the table is at least this wide; below it the table scrolls sideways.
pub const MIN_WIDTH: f32 = 1024.0;
/// One row: two lines of 14 px at 1.4, 16 px of padding and the 1 px rule.
pub const ROW_HEIGHT: f32 = 57.0;
/// The header: 13 px at 1.4, 16 px of padding and the 1 px rule below.
pub const HEADER_HEIGHT: f32 = 35.0;
/// The table never gets shorter than this many rows (unless it has fewer).
const MIN_ROWS: usize = 3;
/// How far one Left or Right press scrolls.
const SCROLL_STEP: f32 = 96.0;

const CONTEXT: &str = "TarpackTable";
const BUILDING: &str = "building";

actions!(
    tarpack_table,
    [BrowseRow, ClearRow, ScrollLeft, ScrollRight]
);

#[derive(Default)]
struct Bound;

impl gpui::Global for Bound {}

/// Binds the table's keys once per app.
fn ensure_bindings(cx: &mut App) {
    if cx.has_global::<Bound>() {
        return;
    }
    cx.set_global(Bound);
    // The `DataTable`'s own context sits below ours, so a binding that must
    // beat it names it as the child. The later binding wins at equal depth.
    let table = format!("{CONTEXT} > DataTable");
    let building = format!("({CONTEXT} && {BUILDING}) > DataTable");
    let mut keys = vec![
        KeyBinding::new("enter", BrowseRow, Some(CONTEXT)),
        KeyBinding::new("delete", ClearRow, Some(CONTEXT)),
        KeyBinding::new("left", ScrollLeft, Some(table.as_str())),
        KeyBinding::new("right", ScrollRight, Some(table.as_str())),
    ];
    for key in ["up", "down", "home", "end", "pageup", "pagedown"] {
        keys.push(KeyBinding::new(key, NoAction, Some(building.as_str())));
    }
    cx.bind_keys(keys);
}

/// The widths of the seven columns for a table `width` wide.
#[must_use]
pub fn column_widths(width: f32) -> [f32; 7] {
    let fixed = STATUS_WIDTH + MODE_WIDTH + OWNER_WIDTH + ACTIONS_WIDTH;
    let share = (width.max(MIN_WIDTH) - fixed) / 3.0;
    [
        STATUS_WIDTH,
        share,
        share,
        share,
        MODE_WIDTH,
        OWNER_WIDTH,
        ACTIONS_WIDTH,
    ]
}

/// What the table's natural height is for `rows` rows: the rule above, the
/// header and the rows.
#[expect(
    clippy::cast_precision_loss,
    reason = "a row count far below f32's exact integers"
)]
fn natural_height(rows: usize) -> f32 {
    1.0 + HEADER_HEIGHT + rows as f32 * ROW_HEIGHT
}

/// The least height the table takes: three rows, or all of them if fewer.
fn min_height(rows: usize) -> f32 {
    natural_height(rows.min(MIN_ROWS))
}

const KEYS: [&str; 7] = [
    "status", "file", "windows", "target", "mode", "owner", "actions",
];

/// Reads its rows from the model.
pub struct EntryDelegate {
    model: Entity<TarpackModel>,
    widths: [f32; 7],
    /// The table's own focus handle (Clear leaves focus there).
    focus: Option<FocusHandle>,
}

impl EntryDelegate {
    fn new(model: Entity<TarpackModel>) -> Self {
        Self {
            model,
            widths: column_widths(MIN_WIDTH),
            focus: None,
        }
    }

    /// The column widths now in use.
    #[cfg(test)]
    pub(super) fn widths(&self) -> [f32; 7] {
        self.widths
    }

    /// Sets the table width the columns follow; whether they changed.
    fn set_width(&mut self, width: f32) -> bool {
        let widths = column_widths(width);
        let changed = widths
            .iter()
            .zip(&self.widths)
            .any(|(a, b)| (a - b).abs() > 0.01);
        self.widths = widths;
        changed
    }
}

/// The entries the model shows, none while no manifest is loaded.
fn entries(model: &TarpackModel) -> &[SessionEntry] {
    model
        .session()
        .and_then(|s| s.manifest.as_ref())
        .map_or(&[], |m| m.entries.as_slice())
}

impl TableDelegate for EntryDelegate {
    fn columns_count(&self, _: &App) -> usize {
        KEYS.len()
    }

    fn rows_count(&self, cx: &App) -> usize {
        entries(self.model.read(cx)).len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        Column::new(KEYS[col_ix], HEADINGS[col_ix])
            .width(px(self.widths[col_ix]))
            .paddings(Edges::all(px(SPACE_2)))
            .resizable(false)
            .movable(false)
    }

    fn render_th(
        &mut self,
        col_ix: usize,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        div()
            .w_full()
            .text_size(px(FONT_SIZE_SM * REM))
            .line_height(relative(LINE_HEIGHT))
            .font_weight(FontWeight::SEMIBOLD)
            .child(HEADINGS[col_ix])
    }

    fn render_tr(
        &mut self,
        row_ix: usize,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) -> Stateful<gpui::Div> {
        div()
            .id(("row", row_ix))
            .debug_selector(move || format!("tarpack-row-{row_ix}"))
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let model = self.model.clone();
        let state = model.read(cx);
        let Some(entry) = entries(state).get(row_ix) else {
            return div().into_any_element();
        };
        match col_ix {
            0 => entry_cells::status_cell(entry),
            1 => entry_cells::file_cell(entry, row_ix, cx),
            2 => entry_cells::windows_cell(entry, row_ix, cx),
            3 => {
                let inner = px(self.widths[3] - 2.0 * SPACE_2);
                entry_cells::target_cell(entry, row_ix, inner, window, cx)
            }
            4 => entry_cells::mode_cell(entry, cx),
            5 => entry_cells::owner_cell(entry),
            _ => match &self.focus {
                Some(focus) => entry_cells::actions_cell(
                    entry,
                    row_ix,
                    &Actions {
                        model: &model,
                        focus,
                        building: state.building(),
                    },
                ),
                None => div().into_any_element(),
            },
        }
    }
}

/// The view the frame hosts: the summary line and the table, or the empty text.
pub struct EntryTable {
    model: Entity<TarpackModel>,
    table: Entity<TableState<EntryDelegate>>,
    /// The width the table was last laid out at.
    width: Option<Pixels>,
}

impl EntryTable {
    pub fn new(model: Entity<TarpackModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let delegate = EntryDelegate::new(model.clone());
        let table = cx.new(|cx| {
            TableState::new(delegate, window, cx)
                .col_resizable(false)
                .col_movable(false)
                .sortable(false)
                .col_selectable(false)
                .loop_selection(false)
        });
        let focus = table.read(cx).focus_handle(cx);
        table.update(cx, |t, _| t.delegate_mut().focus = Some(focus));
        Self {
            model,
            table,
            width: None,
        }
    }

    /// The table state, for the tests.
    #[cfg(test)]
    pub(super) fn table(&self) -> &Entity<TableState<EntryDelegate>> {
        &self.table
    }

    /// The selected row's entry id and whether it has a file.
    fn selected(&self, cx: &App) -> Option<(String, bool)> {
        let row = self.table.read(cx).selected_row()?;
        let entry = entries(self.model.read(cx)).get(row)?;
        Some((entry.view.id.clone(), entry.assigned.is_some()))
    }

    fn browse_selected(&mut self, cx: &mut Context<Self>) {
        let Some((id, _)) = self.selected(cx) else {
            return;
        };
        self.model.update(cx, |m, cx| m.browse(&id, cx));
    }

    fn clear_selected(&mut self, cx: &mut Context<Self>) {
        let Some((id, assigned)) = self.selected(cx) else {
            return;
        };
        if assigned {
            self.model.update(cx, |m, cx| m.clear(&id, cx));
        }
    }

    fn scroll_sideways(&mut self, direction: f32, cx: &mut Context<Self>) {
        let handle = self.table.read(cx).horizontal_scroll_handle.clone();
        let offset = handle.offset();
        let max = handle.max_offset().x;
        let x = (offset.x - px(direction * SCROLL_STEP)).clamp(-max, px(0.0));
        handle.set_offset(point(x, offset.y));
        self.table.update(cx, |_, cx| cx.notify());
    }

    /// The text standing in for the table.
    fn empty(withheld: bool, failed: usize, cx: &App) -> AnyElement {
        let p = palette(cx);
        let (lead, next) = entry_cells::empty_text(withheld, failed);
        v_flex()
            .id("tarpack-no-entries")
            .debug_selector(|| "tarpack-no-entries".to_string())
            .w_full()
            .max_w(px(INTRO_MAX_WIDTH))
            .gap(px(SPACE_1))
            .text_size(px(FONT_SIZE_MD * REM))
            .line_height(relative(LINE_HEIGHT))
            .child(div().child(lead))
            .child(div().text_color(p.text_muted).child(next))
            .into_any_element()
    }
}

impl Render for EntryTable {
    #[expect(clippy::too_many_lines, reason = "one builder chain")]
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        ensure_bindings(cx);
        let p = *palette(cx);
        let state = self.model.read(cx);
        let building = state.building();
        let Some(manifest) = state.session().and_then(|s| s.manifest.as_ref()) else {
            return div().into_any_element();
        };
        let rows = manifest.entries.len();
        let failed = manifest.failed_entries.len();
        if rows == 0 {
            return Self::empty(manifest.entries_withheld, failed, cx);
        }
        let ready = manifest
            .entries
            .iter()
            .filter(|e| e.status == fm_tarpack::sources::EntryStatus::Ready)
            .count();
        let eol = manifest
            .entries
            .iter()
            .filter(|e| e.view.normalize_eol)
            .count();
        let summary = entry_cells::summary(rows, ready, failed, eol);

        // The columns follow the width the table was laid out at.
        let width = self.width.map_or(MIN_WIDTH, f32::from);
        let focus = self.table.read(cx).focus_handle(cx);
        // While a build runs the table leaves the Tab order.
        focus.clone().tab_stop(!building);
        self.table.update(cx, |t, cx| {
            if t.delegate_mut().set_width(width) {
                t.refresh(cx);
            }
            // A reload can leave the selection past the last row.
            if t.selected_row().is_some_and(|row| row >= rows) {
                t.clear_selection(cx);
            }
        });

        let ring = focus.is_focused(window) && window.last_input_was_keyboard();
        let me = cx.entity();
        let mut context = KeyContext::new_with_defaults(cx.platform_info().os);
        context.add(CONTEXT);
        if building {
            context.add(BUILDING);
        }

        let table = DataTable::new(&self.table)
            .bordered(false)
            .focus_ring(false)
            .header_row_height(px(HEADER_HEIGHT))
            .with_size(Size::Size(px(ROW_HEIGHT)))
            .accessibility_label(entry_cells::TABLE_LABEL);

        let region_min = FONT_SIZE_MD * REM * LINE_HEIGHT + SPACE_2 + min_height(rows);
        v_flex()
            .w_full()
            // The region takes what the frame leaves and never less than the
            // summary and three rows: below that the frame's body scrolls.
            .flex_1()
            .min_h(px(region_min))
            .gap(px(SPACE_2))
            .child(tabular(
                div()
                    .text_size(px(FONT_SIZE_MD * REM))
                    .line_height(relative(LINE_HEIGHT))
                    .child(summary),
            ))
            .child(
                // The sheet: as tall as its rows, or as the region if shorter,
                // scrolling its rows inside itself.
                div()
                    .relative()
                    .w_full()
                    // Basis 0 so the row count never sizes the frame; it
                    // grows to the region, but no taller than its rows.
                    .flex_basis(px(0.0))
                    .flex_grow(1.0)
                    .flex_shrink(1.0)
                    .min_h(px(min_height(rows)))
                    .max_h(px(natural_height(rows)))
                    // Clips the table's rows, so they never count toward the
                    // frame body's scroll extent.
                    .overflow_hidden()
                    .border_t_1()
                    .border_color(p.border)
                    .bg(p.surface)
                    .key_context(context)
                    .on_action(cx.listener(|this, _: &BrowseRow, _, cx| this.browse_selected(cx)))
                    .on_action(cx.listener(|this, _: &ClearRow, _, cx| this.clear_selected(cx)))
                    .on_action(cx.listener(|this, _: &ScrollLeft, _, cx| {
                        this.scroll_sideways(-1.0, cx);
                    }))
                    .on_action(cx.listener(|this, _: &ScrollRight, _, cx| {
                        this.scroll_sideways(1.0, cx);
                    }))
                    .on_prepaint(move |bounds, _, cx| {
                        me.update(cx, |this, cx| {
                            let changed = this
                                .width
                                .is_none_or(|old| (old - bounds.size.width).abs() > px(0.5));
                            if changed {
                                this.width = Some(bounds.size.width);
                                cx.notify();
                            }
                        });
                    })
                    .child(table)
                    .when(ring, |sheet| {
                        // The focus ring, inside the sheet's edge.
                        sheet.child(
                            div()
                                .absolute()
                                .inset_0()
                                .border_2()
                                .border_color(p.focus_ring)
                                .debug_selector(|| "tarpack-table-focus".to_string()),
                        )
                    })
                    .debug_selector(|| "tarpack-table".to_string())
                    .tag("tarpack-table"),
            )
            .into_any_element()
    }
}
