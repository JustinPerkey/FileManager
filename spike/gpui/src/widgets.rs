//! B1-B5: gpui-component widgets that `FileManager`'s controls map onto, on one
//! demo screen, each with a headless test of the behaviour the plan names.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    div, px, AppContext as _, ClickEvent, Context, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::checkbox::Checkbox;
use gpui_component::dialog::DialogFooter;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::popover::Popover;
use gpui_component::table::{Column, DataTable, TableDelegate, TableState};
use gpui_component::{
    h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, Sizable as _, WindowExt as _,
};

use crate::assets::icon_path;
use crate::tokens::REM;

gpui::actions!(fm_spike, [ToggleDevMode]);

/// Key bindings of the demo: the developer-mode chord (B8).
pub fn init(cx: &mut gpui::App) {
    cx.bind_keys([gpui::KeyBinding::new(
        "ctrl-alt-shift-d",
        ToggleDevMode,
        Some("WidgetsDemo"),
    )]);
}

/// B2: unchecked, checked, not set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriState {
    Unchecked,
    Checked,
    NotSet,
}

impl TriState {
    /// The click / Space order of the Database Manager's nullable boolean.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Unchecked => Self::Checked,
            Self::Checked => Self::NotSet,
            Self::NotSet => Self::Unchecked,
        }
    }
}

/// B3: the delegate of a 10,000-row Tar-Packager-like table.
pub struct Rows {
    pub count: usize,
}

const STATUSES: [(&str, &str); 3] = [
    ("check-circle", "Ready"),
    ("alert-triangle", "Missing"),
    ("x-circle", "Failed"),
];

impl Rows {
    #[must_use]
    pub fn status(row: usize) -> (&'static str, &'static str) {
        STATUSES[row % 3]
    }
}

/// Keeps the first and last parts of a path: the drive and the file name
/// survive, the middle becomes `...`. GPUI's `text_ellipsis` cuts only the end.
#[must_use]
pub fn middle_truncate(text: &str, max_chars: usize) -> String {
    let n = text.chars().count();
    if n <= max_chars || max_chars < 5 {
        return text.to_string();
    }
    // Keep the whole file name when there is room for the head and the dots.
    let name_len = text
        .rsplit(['\\', '/'])
        .next()
        .map_or(0, |name| name.chars().count());
    let tail = if name_len + 1 + 3 + 3 <= max_chars {
        name_len
    } else {
        (max_chars - 3) / 2
    };
    let head = max_chars - 3 - tail;
    let start: String = text.chars().take(head).collect();
    let end: String = text.chars().skip(n - tail).collect();
    format!("{start}...{end}")
}

impl TableDelegate for Rows {
    fn columns_count(&self, _: &gpui::App) -> usize {
        6
    }

    fn rows_count(&self, _: &gpui::App) -> usize {
        self.count
    }

    fn column(&self, col_ix: usize, _: &gpui::App) -> Column {
        // Fixed widths in rem as in `tarpack.css`: Status 7.25, Mode 8.25, Owner 5.
        let (key, name, rem) = [
            ("status", "Status", 7.25),
            ("file", "File", 14.0),
            ("windows", "Windows location", 18.0),
            ("linux", "Linux target", 18.0),
            ("mode", "Mode", 8.25),
            ("owner", "Owner", 5.0),
        ][col_ix];
        Column::new(key, name)
            .width(px(rem * REM))
            .resizable(false)
            .movable(false)
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let th = cx.theme();
        let mono = th.mono_font_family.clone();
        match col_ix {
            0 => {
                let (icon, word) = Self::status(row_ix);
                let tone = [th.success, th.warning, th.danger][row_ix % 3];
                h_flex()
                    .gap(px(4.0))
                    .items_center()
                    .text_color(tone)
                    .child(
                        Icon::empty()
                            .path(icon_path(icon))
                            .with_size(px(14.0))
                            .text_color(tone),
                    )
                    .child(word)
                    .into_any_element()
            }
            1 => div()
                .child(format!("file-{row_ix:05}.bin"))
                .into_any_element(),
            2 => div()
                .font_family(mono)
                .child(middle_truncate(
                    &format!(
                        r"C:\Deliveries\2026-10\batch-{row_ix:05}\payload\file-{row_ix:05}.bin"
                    ),
                    40,
                ))
                .into_any_element(),
            3 => div()
                .font_family(mono)
                .child(format!("/srv/gateway/in/file-{row_ix:05}.bin"))
                .into_any_element(),
            4 => div().font_family(mono).child("0644").into_any_element(),
            _ => div().child("root").into_any_element(),
        }
    }
}

/// B1-B5 on one screen.
pub struct WidgetsDemo {
    pub plain: Entity<InputState>,
    pub readonly: Entity<InputState>,
    pub invalid: Entity<InputState>,
    pub submitted: Vec<String>,
    pub tri: TriState,
    pub table: Entity<TableState<Rows>>,
    pub busy: Rc<Cell<bool>>,
    pub danger: Rc<Cell<bool>>,
    pub confirmed: Rc<Cell<usize>>,
    pub opener: FocusHandle,
    pub cancel: FocusHandle,
    pub recent_picked: Rc<Cell<Option<usize>>>,
    pub dev_mode: bool,
    _subs: Vec<Subscription>,
}

impl WidgetsDemo {
    pub fn new(rows: usize, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let plain = cx.new(|cx| InputState::new(window, cx).placeholder("C:\\path\\to\\jobs"));
        let readonly = cx.new(|cx| {
            let mut s = InputState::new(window, cx);
            s.set_value("read only, still selectable", window, cx);
            s.set_readonly(true, cx);
            s
        });
        let invalid = cx.new(|cx| {
            let mut s = InputState::new(window, cx);
            s.set_value(r"C:\does\not\exist", window, cx);
            s
        });
        let table = cx.new(|cx| {
            TableState::new(Rows { count: rows }, window, cx)
                .col_resizable(false)
                .col_movable(false)
                .sortable(false)
        });
        let sub = cx.subscribe_in(
            &plain,
            window,
            |this, state, ev: &InputEvent, _window, cx| {
                if let InputEvent::PressEnter { .. } = ev {
                    this.submitted.push(state.read(cx).value().to_string());
                    cx.notify();
                }
            },
        );
        Self {
            plain,
            readonly,
            invalid,
            submitted: Vec::new(),
            tri: TriState::Unchecked,
            table,
            busy: Rc::new(Cell::new(false)),
            danger: Rc::new(Cell::new(false)),
            confirmed: Rc::new(Cell::new(0)),
            opener: cx.focus_handle(),
            cancel: cx.focus_handle(),
            recent_picked: Rc::new(Cell::new(None)),
            dev_mode: false,
            _subs: vec![sub],
        }
    }

    /// B4: opens the confirmation. Focus starts on Cancel, Escape and Cancel
    /// close it, and while `busy` both are inert and the dialog stays open.
    pub fn open_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (busy, danger, confirmed) = (
            self.busy.clone(),
            self.danger.clone(),
            self.confirmed.clone(),
        );
        let cancel = self.cancel.clone();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let (busy2, danger2, confirmed2, cancel2) = (
                busy.clone(),
                danger.clone(),
                confirmed.clone(),
                cancel.clone(),
            );
            dialog
                .title("Run all steps?")
                .w(px(440.0))
                // Escape: off while the confirm is running.
                .keyboard(!busy.get())
                .overlay_closable(false)
                .content(|content, _, cx| {
                    content
                        .child(div().text_color(cx.theme().muted_foreground).child(
                            "Every step runs in order, stopping at the first that does not pass.",
                        ))
                        .child(
                            div()
                                .id("dialog-scroll")
                                .h(px(96.0))
                                .overflow_y_scroll()
                                .children(
                                    (0..24).map(|i| div().child(format!("subfolder-{i:02}"))),
                                ),
                        )
                })
                .footer(
                    DialogFooter::new()
                        .child(
                            div().debug_selector(|| "dialog-cancel".into()).child(
                                Button::new("dialog-cancel")
                                    .label("Cancel")
                                    .track_focus(&cancel2)
                                    .disabled(busy.get())
                                    .on_click({
                                        let busy = busy.clone();
                                        move |_, window, cx| {
                                            if !busy.get() {
                                                window.close_dialog(cx);
                                            }
                                        }
                                    }),
                            ),
                        )
                        .child({
                            let b = Button::new("dialog-confirm")
                                .label(if busy.get() { "Running…" } else { "Run" })
                                .loading(busy.get())
                                .on_click(move |_, _window, _cx| {
                                    // The confirmed work starts; the dialog stays
                                    // open until it finishes.
                                    busy2.set(true);
                                    confirmed2.set(confirmed2.get() + 1);
                                });
                            div().debug_selector(|| "dialog-confirm".into()).child(
                                if danger2.get() {
                                    b.danger()
                                } else {
                                    b.primary()
                                },
                            )
                        }),
                )
        });
        let cancel = self.cancel.clone();
        window.defer(cx, move |window, cx| cancel.focus(window, cx));
    }

    /// The confirmed work is done: the dialog may close.
    pub fn finish_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.busy.set(false);
        window.close_dialog(cx);
    }

    fn section(title: &'static str, body: impl IntoElement) -> impl IntoElement {
        v_flex()
            .gap(px(4.0))
            .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(title))
            .child(body)
    }
}

impl WidgetsDemo {
    fn inputs(&self, cx: &Context<Self>) -> impl IntoElement {
        let th = cx.theme();
        let mono = th.mono_font_family.clone();
        let (danger, border, surface) = (th.danger, th.border, th.popover);
        let field = |state: &Entity<InputState>| {
            Input::new(state)
                .bg(surface)
                .border_color(border)
                .font_family(mono.clone())
        };
        Self::section(
            "B1 text input",
            v_flex()
                .gap(px(4.0))
                .w(px(420.0))
                .child(
                    div()
                        .debug_selector(|| "plain".into())
                        .child(field(&self.plain)),
                )
                .child(field(&self.readonly).readonly(true))
                .child(field(&self.invalid).border_color(danger)),
        )
    }

    fn tri_state(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tri = self.tri;
        Self::section(
            "B2 checkbox with not set",
            h_flex()
                .gap(px(8.0))
                .items_center()
                .child(
                    div().debug_selector(|| "tri".into()).child(
                        Checkbox::new("tri")
                            .checked(tri == TriState::Checked)
                            .indeterminate(tri == TriState::NotSet)
                            .accessibility_label("Nullable flag")
                            .on_change(cx.listener(|this, _checked, _window, cx| {
                                this.tri = this.tri.next();
                                cx.notify();
                            })),
                    ),
                )
                .child(match tri {
                    TriState::NotSet => "Not set",
                    TriState::Checked => "Yes",
                    TriState::Unchecked => "No",
                }),
        )
    }

    fn recent_menu(&self) -> impl IntoElement {
        let picked = self.recent_picked.clone();
        div().debug_selector(|| "recent".into()).child(
            Button::new("recent")
                .label("Recent")
                .icon(Icon::empty().path(icon_path("chevron-down")))
                .dropdown_menu(move |menu, _, _| {
                    let mut menu = menu;
                    for (i, (name, when)) in [
                        ("jobs.toml", "C:\\Deliveries\\2026-10"),
                        ("old.toml", "C:\\Archive"),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let picked = picked.clone();
                        menu = menu.item(
                            PopupMenuItem::element(move |_, cx| {
                                v_flex().child(name).child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(cx.theme().muted_foreground)
                                        .child(when),
                                )
                            })
                            .on_click(move |_, _, _| picked.set(Some(i))),
                        );
                    }
                    menu
                }),
        )
    }

    fn overlays(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let confirm_variant = self.danger.clone();
        Self::section(
            "B4 / B5 dialog, menu, popover",
            h_flex()
                .gap(px(8.0))
                .child(
                    div().debug_selector(|| "open-dialog".into()).child(
                        Button::new("open-dialog")
                            .label("Run all\u{2026}")
                            .track_focus(&self.opener)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.open_confirm(window, cx);
                            })),
                    ),
                )
                .child(
                    div().debug_selector(|| "danger-toggle".into()).child(
                        Button::new("danger-toggle")
                            .label("Danger variant")
                            .on_click(move |_, _, _| {
                                confirm_variant.set(!confirm_variant.get());
                            }),
                    ),
                )
                .child(self.recent_menu())
                .child(
                    div().debug_selector(|| "help".into()).child(
                        Popover::new("help")
                            .trigger(Button::new("help-trigger").label("Shortcuts").small())
                            .content(|_, _, cx| {
                                div()
                                    .debug_selector(|| "help-content".into())
                                    .bg(cx.theme().popover)
                                    .child("Ctrl+Alt+Shift+D toggles developer mode")
                            }),
                    ),
                ),
        )
    }
}

impl Render for WidgetsDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (background, foreground) = (cx.theme().background, cx.theme().foreground);
        v_flex()
            .key_context("WidgetsDemo")
            .on_action(cx.listener(|this, _: &ToggleDevMode, _, cx| {
                this.dev_mode = !this.dev_mode;
                cx.notify();
            }))
            .size_full()
            .gap(px(12.0))
            .p(px(16.0))
            .bg(background)
            .text_color(foreground)
            .child(self.inputs(cx))
            .child(self.tri_state(cx))
            .child(Self::section(
                "B3 table, 10,000 rows",
                div()
                    .h(px(240.0))
                    .child(DataTable::new(&self.table).accessibility_label("Entries")),
            ))
            .child(self.overlays(cx))
            .child(div().child(SharedString::from(format!(
                "submitted: {:?}; dev mode: {}",
                self.submitted, self.dev_mode
            ))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    use gpui::{size, ClipboardItem, Focusable as _, Modifiers, TestAppContext, VisualTestContext};
    use gpui_component::Root;

    fn open(cx: &mut TestAppContext, rows: usize) -> (Entity<WidgetsDemo>, &mut VisualTestContext) {
        cx.update(gpui_component::init);
        cx.update(init);
        cx.update(|cx| crate::theme::apply(false, cx));
        let slot: Rc<RefCell<Option<Entity<WidgetsDemo>>>> = Rc::default();
        let slot_in = slot.clone();
        let (_root, cx) = cx.add_window_view(move |window, cx| {
            let view = cx.new(|cx| WidgetsDemo::new(rows, window, cx));
            *slot_in.borrow_mut() = Some(view.clone());
            Root::new(view, window, cx)
        });
        cx.simulate_resize(size(px(1000.0), px(900.0)));
        cx.run_until_parked();
        let view = slot.borrow_mut().take().expect("view");
        (view, cx)
    }

    /// A key press as a platform delivers it: key down, then key up. Buttons
    /// and checkboxes activate from the pair (a lone key down does nothing).
    fn press(cx: &mut VisualTestContext, key: &str) {
        let keystroke = cx
            .update(|_, cx| gpui::Keystroke::parse_with_os(key, cx.platform_info().os))
            .expect("keystroke");
        cx.simulate_event(gpui::KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        cx.simulate_event(gpui::KeyUpEvent { keystroke });
    }

    fn center(cx: &mut VisualTestContext, selector: &'static str) -> gpui::Point<gpui::Pixels> {
        cx.debug_bounds(selector)
            .unwrap_or_else(|| panic!("no element with debug selector {selector}"))
            .center()
    }

    #[gpui::test]
    fn input_paste_and_submit(cx: &mut TestAppContext) {
        let (view, cx) = open(cx, 10);
        let (plain, readonly) = view.read_with(cx, |v, _| (v.plain.clone(), v.readonly.clone()));
        cx.update(|window, cx| plain.update(cx, |s, cx| s.focus(window, cx)));
        cx.simulate_input("abc");
        assert_eq!(plain.read_with(cx, |s, _| s.value()), "abc");
        // Selection and caret movement.
        cx.simulate_keystrokes("ctrl-a");
        assert_eq!(plain.read_with(cx, |s, _| s.selected_value()), "abc");
        cx.simulate_keystrokes("home");
        assert_eq!(plain.read_with(cx, |s, _| s.cursor()), 0);
        cx.simulate_keystrokes("right right");
        assert_eq!(plain.read_with(cx, |s, _| s.cursor()), 2);
        cx.simulate_keystrokes("end");
        // Paste (Ctrl+V).
        cx.write_to_clipboard(ClipboardItem::new_string("XYZ".into()));
        cx.simulate_keystrokes("ctrl-v");
        assert_eq!(plain.read_with(cx, |s, _| s.value()), "abcXYZ");
        // Enter reaches the model.
        cx.simulate_keystrokes("enter");
        assert_eq!(
            view.read_with(cx, |v, _| v.submitted.clone()),
            vec!["abcXYZ".to_string()]
        );
        // Read-only: selectable, not editable.
        cx.update(|window, cx| readonly.update(cx, |s, cx| s.focus(window, cx)));
        cx.simulate_input("zzz");
        assert_eq!(
            readonly.read_with(cx, |s, _| s.value()),
            "read only, still selectable"
        );
        cx.simulate_keystrokes("ctrl-a");
        assert_eq!(
            readonly.read_with(cx, |s, _| s.selected_value()),
            "read only, still selectable"
        );
    }

    #[gpui::test]
    fn tri_state_checkbox_cycles(cx: &mut TestAppContext) {
        let (view, cx) = open(cx, 10);
        assert_eq!(view.read_with(cx, |v, _| v.tri), TriState::Unchecked);
        let at = center(cx, "tri");
        cx.simulate_click(at, Modifiers::none());
        assert_eq!(view.read_with(cx, |v, _| v.tri), TriState::Checked);
        cx.simulate_click(at, Modifiers::none());
        assert_eq!(view.read_with(cx, |v, _| v.tri), TriState::NotSet);
        assert_eq!(
            TriState::Unchecked.next().next().next(),
            TriState::Unchecked
        );
    }

    /// B2 / B8: the box is reachable with Tab and toggled by Space.
    #[gpui::test]
    fn tri_state_checkbox_is_keyboard_operable(cx: &mut TestAppContext) {
        let (view, cx) = open(cx, 10);
        // Nothing has focus in a fresh window; the first Tab press lands on
        // the first tab stop (focus_next), then Tab keystrokes move on.
        cx.update(gpui::Window::focus_next);
        let mut tabs = 1;
        let mut seen = Vec::new();
        while view.read_with(cx, |v, _| v.tri) == TriState::Unchecked {
            assert!(
                tabs < 40,
                "Tab never reached the checkbox; focus order {seen:?}"
            );
            cx.simulate_keystrokes("tab");
            seen.push(cx.update(|w, cx| w.focused(cx).map(|h| format!("{h:?}"))));
            tabs += 1;
            press(cx, "space");
        }
        eprintln!("TAB-STOPS-TO-CHECKBOX {tabs}");
        assert_eq!(view.read_with(cx, |v, _| v.tri), TriState::Checked);
        press(cx, "space");
        assert_eq!(view.read_with(cx, |v, _| v.tri), TriState::NotSet);
        press(cx, "space");
        assert_eq!(view.read_with(cx, |v, _| v.tri), TriState::Unchecked);
    }

    #[gpui::test]
    fn table_keyboard_focus_moves_and_scrolls(cx: &mut TestAppContext) {
        let (view, cx) = open(cx, 10_000);
        let table = view.read_with(cx, |v, _| v.table.clone());
        assert_eq!(table.read_with(cx, |t, _| t.delegate().count), 10_000);
        cx.update(|window, cx| {
            let handle = table.read(cx).focus_handle(cx);
            window.focus(&handle, cx);
        });
        for _ in 0..60 {
            cx.simulate_keystrokes("down");
        }
        let (selected, visible) = table.read_with(cx, |t, _| {
            (t.selected_row(), t.visible_range().rows().clone())
        });
        let selected = selected.expect("a row is selected");
        assert!(
            selected >= 59,
            "60 presses of Down moved the selection to {selected}"
        );
        assert!(
            visible.contains(&selected),
            "selected row {selected} is scrolled into view, visible {visible:?}"
        );
        cx.simulate_keystrokes("up");
        assert_eq!(
            table.read_with(cx, |t, _| t.selected_row()),
            Some(selected - 1)
        );
    }

    /// Report-only timing of 10,000 rows: per scroll step, the time to update
    /// and lay out and prepare one frame headlessly (no GPU).
    #[gpui::test]
    fn table_scroll_steps_are_cheap(cx: &mut TestAppContext) {
        let (view, cx) = open(cx, 10_000);
        let table = view.read_with(cx, |v, _| v.table.clone());
        let mut times = Vec::new();
        for row in (0..10_000).step_by(100) {
            let t = std::time::Instant::now();
            table.update(cx, |t, cx| t.scroll_to_row(row, cx));
            cx.run_until_parked();
            times.push(t.elapsed());
        }
        times.sort();
        let (median, max) = (times[times.len() / 2], times[times.len() - 1]);
        eprintln!("TABLE-TIMING 100 scroll steps over 10,000 rows: median {median:?}, max {max:?}");
        assert!(
            max < std::time::Duration::from_secs(2),
            "a scroll step took {max:?}"
        );
    }

    #[test]
    fn middle_truncation_keeps_drive_and_file_name() {
        let p = r"C:\Deliveries\2026-10\batch-00001\payload\file-00001.bin";
        let t = middle_truncate(p, 30);
        assert_eq!(t.chars().count(), 30);
        assert!(t.starts_with("C:\\Deli"), "{t}");
        assert!(t.ends_with("file-00001.bin"), "{t}");
        assert_eq!(middle_truncate("short", 30), "short");
    }

    #[gpui::test]
    fn dialog_focus_starts_on_cancel_and_escape_cancels(cx: &mut TestAppContext) {
        let (view, cx) = open(cx, 10);
        let (opener, cancel) = view.read_with(cx, |v, _| (v.opener.clone(), v.cancel.clone()));
        // Opened from the keyboard: the opener has focus (a mouse press on a
        // gpui-component Button does not focus it, so nothing would return).
        cx.update(|window, cx| opener.focus(window, cx));
        press(cx, "enter");
        cx.run_until_parked();
        assert!(
            cx.update(gpui_component::WindowExt::has_active_dialog),
            "dialog opened"
        );
        assert!(
            cx.update(|window, _| cancel.is_focused(window)),
            "focus starts on Cancel"
        );
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(
            !cx.update(gpui_component::WindowExt::has_active_dialog),
            "Escape cancelled"
        );
        assert!(
            cx.update(|window, _| opener.is_focused(window)),
            "focus returned to the opener"
        );
    }

    #[gpui::test]
    fn dialog_busy_is_inert(cx: &mut TestAppContext) {
        let (view, cx) = open(cx, 10);
        let at = center(cx, "open-dialog");
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
        // Confirm: the work starts, the dialog stays open.
        let confirm = center(cx, "dialog-confirm");
        cx.simulate_click(confirm, Modifiers::none());
        cx.run_until_parked();
        assert!(view.read_with(cx, |v, _| v.busy.get()));
        assert_eq!(view.read_with(cx, |v, _| v.confirmed.get()), 1);
        assert!(
            cx.update(gpui_component::WindowExt::has_active_dialog),
            "still open while busy"
        );
        // Escape and Cancel do nothing.
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(
            cx.update(gpui_component::WindowExt::has_active_dialog),
            "Escape is inert"
        );
        let cancel = center(cx, "dialog-cancel");
        cx.simulate_click(cancel, Modifiers::none());
        cx.run_until_parked();
        assert!(
            cx.update(gpui_component::WindowExt::has_active_dialog),
            "Cancel is inert"
        );
        // The work finishes: the dialog closes.
        view.update_in(cx, WidgetsDemo::finish_confirm);
        cx.run_until_parked();
        assert!(!cx.update(gpui_component::WindowExt::has_active_dialog));
    }

    #[gpui::test]
    fn menu_keys_wrap_and_escape_refocuses(cx: &mut TestAppContext) {
        let (view, cx) = open(cx, 10);
        let at = center(cx, "recent");
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
        // Down, Down (past the last item: wraps to the first), Enter picks it.
        cx.simulate_keystrokes("down down down enter");
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |v, _| v.recent_picked.get()),
            Some(0),
            "Down, Down, Down over two items wraps to the first"
        );
        // From the keyboard: Tab to the Recent button, Enter opens the menu.
        view.read_with(cx, |v, _| v.recent_picked.set(None));
        cx.update(gpui::Window::focus_next);
        let mut stops = 1;
        loop {
            assert!(stops < 14, "Tab never reached the Recent button");
            press(cx, "enter");
            cx.run_until_parked();
            if cx.update(gpui_component::WindowExt::has_active_dialog) {
                cx.simulate_keystrokes("escape");
                cx.run_until_parked();
            }
            // Is a menu open? Down then Enter picks the first item if so.
            cx.simulate_keystrokes("down enter");
            cx.run_until_parked();
            if view.read_with(cx, |v, _| v.recent_picked.get()).is_some() {
                break;
            }
            cx.simulate_keystrokes("escape");
            cx.simulate_keystrokes("tab");
            stops += 1;
        }
        eprintln!("TAB-STOPS-TO-RECENT-BUTTON {stops}");
        // Picking an item closed the menu; was focus returned to the trigger?
        // Enter on it opens the menu again, Down + Enter picks the first item.
        view.read_with(cx, |v, _| v.recent_picked.set(None));
        press(cx, "enter");
        cx.run_until_parked();
        cx.simulate_keystrokes("down enter");
        cx.run_until_parked();
        eprintln!(
            "MENU-REOPEN-BY-ENTER-AFTER-PICK {:?}",
            view.read_with(cx, |v, _| v.recent_picked.get())
        );
        // Escape closes the menu and returns focus to the trigger.
        view.read_with(cx, |v, _| v.recent_picked.set(None));
        press(cx, "enter");
        cx.run_until_parked();
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        cx.simulate_keystrokes("down enter");
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |v, _| v.recent_picked.get()),
            None,
            "Escape closed the menu"
        );
        press(cx, "enter");
        cx.run_until_parked();
        cx.simulate_keystrokes("down enter");
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |v, _| v.recent_picked.get()),
            Some(0),
            "after Escape the trigger has focus again: Enter reopens the menu"
        );
    }

    #[gpui::test]
    fn popover_opens_under_its_button_and_escape_closes_it(cx: &mut TestAppContext) {
        let (_view, cx) = open(cx, 10);
        assert!(cx.debug_bounds("help-content").is_none(), "closed at first");
        let at = center(cx, "help");
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
        let content = cx.debug_bounds("help-content").expect("popover opened");
        let trigger = cx.debug_bounds("help").expect("trigger");
        assert!(
            content.origin.y >= trigger.origin.y,
            "anchored under the button"
        );
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("help-content").is_none(),
            "Escape closed the popover"
        );
    }

    #[gpui::test]
    fn developer_mode_chord_toggles_a_flag(cx: &mut TestAppContext) {
        let (view, cx) = open(cx, 10);
        let plain = view.read_with(cx, |v, _| v.plain.clone());
        cx.update(|window, cx| plain.update(cx, |s, cx| s.focus(window, cx)));
        assert!(!view.read_with(cx, |v, _| v.dev_mode));
        cx.simulate_keystrokes("ctrl-alt-shift-d");
        assert!(view.read_with(cx, |v, _| v.dev_mode));
        cx.simulate_keystrokes("ctrl-alt-shift-d");
        assert!(!view.read_with(cx, |v, _| v.dev_mode));
    }
}
