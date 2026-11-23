//! View tests of the controls: keyboard behaviour and labels, in a real
//! (headless) window under `Root`, as the app draws them.
//!
//! [`Gallery`] draws one of each widget (also what a screenshot of the set
//! would show); the behaviour tests use [`Probe`], a view of one widget with
//! the state the test reads.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{
    div, px, size, AnyElement, App, Context, Entity, Modifiers, Pixels, Point, SharedString,
    TestAppContext, VisualTestContext, Window,
};
use gpui_component::input::{InputState, TextareaState};
use gpui_component::{h_flex, v_flex, Root, ThemeMode};

use super::*;
use crate::app::probe::Tag as _;
use crate::assets::ICON_NAMES;

// MARK: harness

type Builder<S> = Box<dyn Fn(&S, &mut Window, &mut Context<Probe<S>>) -> AnyElement>;

/// A view of whatever `build` draws from `state`.
struct Probe<S: 'static> {
    state: S,
    build: Builder<S>,
}

impl<S: 'static> Render for Probe<S> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // A row, so each widget keeps its own width: a click at the centre of
        // its bounds lands on it.
        h_flex()
            .p(px(16.0))
            .items_start()
            .child((self.build)(&self.state, window, cx))
    }
}

/// Opens a window with `build`'s widget in it. Nothing has focus.
fn open<S: 'static>(
    cx: &mut TestAppContext,
    make: impl FnOnce(&mut Window, &mut Context<Probe<S>>) -> S + 'static,
    build: impl Fn(&S, &mut Window, &mut Context<Probe<S>>) -> AnyElement + 'static,
) -> (Entity<Probe<S>>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::theme::apply(ThemeMode::Light, cx);
    });
    let slot: Rc<RefCell<Option<Entity<Probe<S>>>>> = Rc::default();
    let slot_in = slot.clone();
    let (_root, vcx) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| Probe {
            state: make(window, cx),
            build: Box::new(build),
        });
        *slot_in.borrow_mut() = Some(view.clone());
        Root::new(view, window, cx)
    });
    vcx.simulate_resize(size(px(800.0), px(600.0)));
    vcx.run_until_parked();
    let view = slot.borrow_mut().take().expect("probe created");
    (view, vcx)
}

/// A key press as a platform delivers it: key down, then key up. Buttons and
/// checkboxes activate from the pair (a lone key down does nothing).
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
    cx.run_until_parked();
}

/// Focus lands on the first tab stop, as the first Tab press does in a window
/// where nothing is focused.
fn first_tab_stop(cx: &mut VisualTestContext) {
    cx.update(Window::focus_next);
    cx.run_until_parked();
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> gpui::Bounds<Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("no element with debug selector {selector}"))
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let at = bounds(cx, selector).center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

fn counter() -> (
    Rc<Cell<usize>>,
    impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) {
    let count = Rc::new(Cell::new(0));
    let handler = {
        let count = count.clone();
        move |_: &gpui::ClickEvent, _: &mut Window, _: &mut App| count.set(count.get() + 1)
    };
    (count, handler)
}

// MARK: buttons

#[gpui::test]
fn button_click_and_keyboard_activate(cx: &mut TestAppContext) {
    let count = Rc::new(Cell::new(0));
    let (_view, cx) = open(cx, |_, _| (), {
        let count = count.clone();
        move |(), _, _| {
            let count = count.clone();
            button("run", "Run")
                .debug_selector("run")
                .on_click(move |_, _, _| count.set(count.get() + 1))
                .into_any_element()
        }
    });

    click(cx, "run");
    assert_eq!(count.get(), 1, "a click activates");

    first_tab_stop(cx);
    press(cx, "enter");
    assert_eq!(count.get(), 2, "Tab, then Enter (down and up) activates");

    press(cx, "space");
    assert_eq!(count.get(), 3, "Space (down and up) activates");
}

#[gpui::test]
fn disabled_button_ignores_activation(cx: &mut TestAppContext) {
    let (count, handler) = counter();
    let (_view, cx) = open(cx, |_, _| (), {
        let handler = Rc::new(handler);
        move |(), _, _| {
            let handler = handler.clone();
            button("save", "Save")
                .primary()
                .disabled(true)
                .debug_selector("save")
                .on_click(move |e, w, c| handler(e, w, c))
                .into_any_element()
        }
    });

    click(cx, "save");
    first_tab_stop(cx);
    press(cx, "enter");
    press(cx, "space");
    cx.simulate_keystrokes("tab");
    press(cx, "enter");
    assert_eq!(count.get(), 0, "no click, Enter or Space activates it");
}

#[gpui::test]
fn busy_button_is_inert_and_keeps_label(cx: &mut TestAppContext) {
    let (count, handler) = counter();
    let handler = Rc::new(handler);
    let busy = button("build", "Build").primary().busy(true);
    assert_eq!(busy.visible_label().map(AsRef::as_ref), Some("Build"));
    assert!(busy.is_busy());

    let (_view, cx) = open(
        cx,
        |_, _| (),
        move |(), _, _| {
            let handler = handler.clone();
            button("build", "Build")
                .primary()
                .busy(true)
                .debug_selector("build")
                .on_click(move |e, w, c| handler(e, w, c))
                .into_any_element()
        },
    );
    click(cx, "build");
    first_tab_stop(cx);
    press(cx, "enter");
    press(cx, "space");
    assert_eq!(count.get(), 0, "a busy button does nothing when activated");
}

#[test]
fn icon_only_button_has_tooltip() {
    // The type is the guarantee: `icon_only` takes the tooltip as an argument,
    // and the label it replaces stays as the accessible name.
    let b = button("dismiss", "Dismiss")
        .quiet()
        .icon_only(IconName::X, "Dismiss");
    assert_eq!(b.visible_label(), None);
    assert_eq!(b.tooltip_text().as_deref(), Some("Dismiss"));
    let with_key = b.shortcut("Esc");
    assert_eq!(
        with_key.tooltip_text().as_deref(),
        Some("Dismiss. Shortcut: Esc")
    );
}

#[test]
fn icon_names_are_the_shipped_icons() {
    let names: Vec<&str> = IconName::ALL.iter().map(|n| n.as_str()).collect();
    assert_eq!(names, ICON_NAMES);
}

// MARK: checkbox

fn checkbox_probe(cx: &mut TestAppContext) -> (Rc<Cell<CheckState>>, &mut VisualTestContext) {
    let state = Rc::new(Cell::new(CheckState::Unchecked));
    let (_view, cx) = open(cx, |_, _| (), {
        let state = state.clone();
        move |(), _, cx| {
            let this = cx.entity();
            let shared = state.clone();
            div()
                .debug_selector(|| "nullable".into())
                .child(checkbox("nullable", "Nullable", state.get()).on_change(
                    move |next, _, cx| {
                        shared.set(next);
                        this.update(cx, |_, cx| cx.notify());
                    },
                ))
                .into_any_element()
        }
    });
    (state, cx)
}

#[gpui::test]
fn checkbox_cycles_not_set(cx: &mut TestAppContext) {
    let (state, cx) = checkbox_probe(cx);
    // The box is at the left edge of the control.
    let b = bounds(cx, "nullable");
    let on_box = Point::new(b.origin.x + px(8.0), b.center().y);

    for expected in [
        CheckState::Checked,
        CheckState::NotSet,
        CheckState::Unchecked,
    ] {
        cx.simulate_click(on_box, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(state.get(), expected, "click");
    }

    first_tab_stop(cx);
    for expected in [
        CheckState::Checked,
        CheckState::NotSet,
        CheckState::Unchecked,
    ] {
        press(cx, "space");
        assert_eq!(state.get(), expected, "Space");
    }
    assert_eq!(
        CheckState::Unchecked.next().next().next(),
        CheckState::Unchecked
    );
}

#[gpui::test]
fn not_set_shows_the_words(cx: &mut TestAppContext) {
    let (state, cx) = checkbox_probe(cx);
    assert!(
        cx.debug_bounds("not-set").is_none(),
        "unchecked has no extra word"
    );
    first_tab_stop(cx);
    press(cx, "space");
    assert!(
        cx.debug_bounds("not-set").is_none(),
        "checked has no extra word"
    );
    press(cx, "space");
    assert_eq!(state.get(), CheckState::NotSet);
    assert!(
        cx.debug_bounds("not-set").is_some(),
        "not set says so in words"
    );
    press(cx, "space");
    assert!(cx.debug_bounds("not-set").is_none());
}

// MARK: inputs

#[gpui::test]
fn input_typing_and_read_only_selects(cx: &mut TestAppContext) {
    let (view, cx) = open(
        cx,
        |window, cx| {
            let editable = cx.new(|cx| InputState::new(window, cx).placeholder("C:\\jobs"));
            let fixed = cx.new(|cx| {
                let mut s = InputState::new(window, cx);
                s.set_value("read only, still selectable", window, cx);
                s
            });
            (editable, fixed)
        },
        |(editable, fixed), _, _| {
            v_flex()
                .w(px(300.0))
                .gap(px(8.0))
                .child(text_input(editable).mono())
                .child(text_input(fixed).read_only(true))
                .into_any_element()
        },
    );
    let (editable, fixed) = view.read_with(cx, |p, _| p.state.clone());

    cx.update(|window, cx| editable.update(cx, |s, cx| s.focus(window, cx)));
    cx.simulate_input("abc");
    assert_eq!(
        editable.read_with(cx, |s, _| s.value()),
        "abc",
        "typing reaches the state"
    );

    cx.update(|window, cx| fixed.update(cx, |s, cx| s.focus(window, cx)));
    cx.simulate_input("zzz");
    assert_eq!(
        fixed.read_with(cx, |s, _| s.value()),
        "read only, still selectable",
        "a read-only input ignores typing"
    );
    cx.simulate_keystrokes("ctrl-a");
    assert_eq!(
        fixed.read_with(cx, |s, _| s.selected_value()),
        "read only, still selectable",
        "but its text can be selected"
    );
}

#[gpui::test]
fn text_area_takes_lines(cx: &mut TestAppContext) {
    let (view, cx) = open(
        cx,
        |window, cx| cx.new(|cx| TextareaState::new(window, cx)),
        |area, _, _| {
            div()
                .w(px(300.0))
                .child(text_area(area).multiline(4).mono().invalid(true))
                .into_any_element()
        },
    );
    let area = view.read_with(cx, |p, _| p.state.clone());
    cx.update(|window, cx| area.update(cx, |s, cx| s.focus(window, cx)));
    cx.simulate_input("one");
    cx.simulate_keystrokes("enter");
    cx.simulate_input("two");
    assert_eq!(area.read_with(cx, |s, _| s.value()), "one\ntwo");
}

// MARK: banner and disclosure

#[gpui::test]
fn banner_dismiss_and_action(cx: &mut TestAppContext) {
    let (acted, on_action) = counter();
    let (dismissed, _) = counter();
    let on_action = Rc::new(on_action);
    let (_view, cx) = open(cx, |_, _| (), {
        let dismissed = dismissed.clone();
        move |(), _, _| {
            let on_action = on_action.clone();
            let dismissed = dismissed.clone();
            Banner::new(Tone::Warn, "The manifest changed on disk.")
                .action(
                    button("reload", "Reload")
                        .debug_selector("banner-action")
                        .on_click(move |e, w, c| on_action(e, w, c)),
                )
                .dismiss(move |_, _| dismissed.set(dismissed.get() + 1))
                .into_any_element()
        }
    });
    click(cx, "banner-action");
    assert_eq!((acted.get(), dismissed.get()), (1, 0));
    click(cx, "banner-dismiss");
    assert_eq!((acted.get(), dismissed.get()), (1, 1));

    // Both are reachable and work from the keyboard: Reload, then Dismiss.
    first_tab_stop(cx);
    press(cx, "enter");
    assert_eq!((acted.get(), dismissed.get()), (2, 1), "Reload by Enter");
    cx.simulate_keystrokes("tab");
    press(cx, "enter");
    assert_eq!((acted.get(), dismissed.get()), (2, 2), "Dismiss by Enter");
}

#[gpui::test]
fn banner_details_toggle(cx: &mut TestAppContext) {
    let (_view, cx) = open(
        cx,
        |_, _| (),
        |(), _, _| {
            Banner::new(Tone::Error, "Could not save.")
                .id("save-error")
                .details("os error 5: access denied")
                .into_any_element()
        },
    );
    assert!(
        cx.debug_bounds("banner-details-text").is_none(),
        "closed at first"
    );
    click(cx, "banner-details-toggle");
    assert!(
        cx.debug_bounds("banner-details-text").is_some(),
        "a click opens it"
    );
    click(cx, "banner-details-toggle");
    assert!(
        cx.debug_bounds("banner-details-text").is_none(),
        "and closes it"
    );
}

#[gpui::test]
fn disclosure_toggles_by_keyboard(cx: &mut TestAppContext) {
    let open_now = Rc::new(Cell::new(false));
    let (_view, cx) = open(cx, |_, _| (), {
        let open_now = open_now.clone();
        move |(), _, cx| {
            let this = cx.entity();
            let shared = open_now.clone();
            Disclosure::new("more", "More")
                .open(open_now.get())
                .on_toggle(move |next, _, cx| {
                    shared.set(next);
                    this.update(cx, |_, cx| cx.notify());
                })
                .child(div().debug_selector(|| "more-body".into()).child("Body"))
                .into_any_element()
        }
    });
    assert!(cx.debug_bounds("more-body").is_none());
    first_tab_stop(cx);
    press(cx, "enter");
    assert!(open_now.get(), "Enter opens");
    assert!(cx.debug_bounds("more-body").is_some(), "the content shows");
    press(cx, "space");
    assert!(!open_now.get(), "Space closes");
    assert!(cx.debug_bounds("more-body").is_none());
}

// MARK: select

fn options() -> Vec<(SharedString, SharedString)> {
    ["Gzip", "Xz", "Zstd"]
        .iter()
        .map(|t| (SharedString::from(*t), SharedString::from(t.to_lowercase())))
        .collect()
}

#[gpui::test]
fn select_opens_and_picks_by_keyboard(cx: &mut TestAppContext) {
    let picked: Rc<RefCell<Vec<SharedString>>> = Rc::default();
    let (_view, cx) = open(cx, |_, _| (), {
        let picked = picked.clone();
        move |(), _, _| {
            let picked = picked.clone();
            select("format", "Archive format", options(), None)
                .width(px(200.0))
                .on_pick(move |value, _, _| picked.borrow_mut().push(value.clone()))
                .into_any_element()
        }
    });
    first_tab_stop(cx);
    let keys = |cx: &mut VisualTestContext, keys: &str| {
        cx.simulate_keystrokes(keys);
        cx.run_until_parked();
    };

    // Enter opens the list, nothing is chosen yet; Down moves to the first
    // option, then the second; Enter picks it.
    keys(cx, "enter");
    assert!(picked.borrow().is_empty(), "opening picks nothing");
    keys(cx, "down");
    keys(cx, "down");
    keys(cx, "enter");
    assert_eq!(
        *picked.borrow(),
        ["xz"],
        "Down, Down, Enter picks the second"
    );

    // Escape closes an open list without a pick.
    keys(cx, "enter");
    keys(cx, "escape");
    assert_eq!(picked.borrow().len(), 1, "Escape picks nothing");
    // Closed: Enter now opens the list again (an open list would pick).
    keys(cx, "enter");
    assert_eq!(picked.borrow().len(), 1, "Escape closed it");
    keys(cx, "escape");

    // Each of Down, Space and Enter opens a closed select: after it, moving
    // and Enter pick an option. (If the key had not opened the list, the
    // moving key would have, and Enter would have found nothing to pick.)
    for key in ["down", "space", "enter"] {
        let before = picked.borrow().len();
        keys(cx, key);
        keys(cx, "up");
        keys(cx, "enter");
        assert_eq!(
            picked.borrow().len(),
            before + 1,
            "{key} opens, Up moves, Enter picks"
        );
    }
}

// MARK: middle path

#[gpui::test]
fn middle_path_tooltip_is_full_path(cx: &mut TestAppContext) {
    let path =
        r"C:\Deliveries\2026-10\batch-00001\payload\a-rather-long-folder-name\file-00001.bin";
    assert_eq!(
        MiddlePath::new(path).tooltip_text(),
        path,
        "the tooltip is the whole path"
    );

    // Drawn in a narrow box the path stays inside it, however long it is.
    let (_view, cx) = open(
        cx,
        |_, _| (),
        move |(), _, _| {
            div()
                .w(px(180.0))
                .debug_selector(|| "narrow".into())
                .child(MiddlePath::new(path))
                .into_any_element()
        },
    );
    cx.run_until_parked();
    let b = bounds(cx, "narrow");
    assert!(b.size.width <= px(180.0), "{:?}", b.size);
}

// MARK: gallery

/// One of each widget, as a screenshot of the set would show.
struct Gallery {
    check: CheckState,
    editable: Entity<InputState>,
    fixed: Entity<InputState>,
    format: Option<SharedString>,
    more: bool,
}

impl Render for Gallery {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let row = || h_flex().gap(px(8.0)).items_center().flex_wrap();
        v_flex()
            .w(px(720.0))
            .p(px(16.0))
            .gap(px(12.0))
            .child(h1("Widgets"))
            .child(h2("Controls"))
            .child(intro("One of each control the tool views are built from."))
            .child(
                row()
                    .child(button("g-secondary", "Secondary"))
                    .child(button("g-primary", "Primary").primary().tag("g-primary"))
                    .child(button("g-quiet", "Quiet").quiet())
                    .child(button("g-danger", "Delete").danger())
                    .child(button("g-icon", "Browse").icon(IconName::Folder))
                    .child(
                        button("g-icon-only", "Copy")
                            .quiet()
                            .icon_only(IconName::Copy, "Copy path"),
                    )
                    .child(button("g-disabled", "Disabled").disabled(true))
                    .child(button("g-busy", "Building").primary().busy(true)),
            )
            .child(
                row()
                    .child(checkbox("g-check", "Nullable", self.check).on_change({
                        let this = this.clone();
                        move |next, _, cx| {
                            this.update(cx, |g, cx| {
                                g.check = next;
                                cx.notify();
                            });
                        }
                    }))
                    .child(
                        select("g-select", "Archive format", options(), self.format.clone())
                            .width(px(160.0))
                            .on_pick({
                                let this = this.clone();
                                move |v, _, cx| {
                                    let v = v.clone();
                                    this.update(cx, |g, cx| {
                                        g.format = Some(v);
                                        cx.notify();
                                    });
                                }
                            }),
                    ),
            )
            .child(div().w(px(320.0)).child(text_input(&self.editable).mono()))
            .child(
                div()
                    .w(px(320.0))
                    .child(text_input(&self.fixed).read_only(true)),
            )
            .child(
                Banner::new(Tone::Info, "Three files were added.")
                    .id("g-banner")
                    .action(button("g-banner-action", "Undo"))
                    .dismiss(|_, _| {})
                    .details("added: a.bin, b.bin, c.bin"),
            )
            .child(
                Disclosure::new("g-more", "More options")
                    .open(self.more)
                    .on_toggle({
                        let this = this.clone();
                        move |next, _, cx| {
                            this.update(cx, |g, cx| {
                                g.more = next;
                                cx.notify();
                            });
                        }
                    })
                    .child(small_muted("Nothing else to set.")),
            )
            .child(
                row()
                    .child(tag("tar.zst", Some(IconName::Info)))
                    .child(status(StatusTone::Ok, IconName::CheckCircle, "Ready"))
                    .child(status(StatusTone::Warn, IconName::AlertTriangle, "Missing"))
                    .child(status(StatusTone::Danger, IconName::XCircle, "Failed"))
                    .child(status(StatusTone::Muted, IconName::Circle, "Not run"))
                    .child(step_number(1))
                    .child(chord(&["Ctrl", "O"]))
                    .child(alternatives(&[&["Ctrl", "Enter"], &["F8"]])),
            )
            .child(div().w(px(240.0)).child(MiddlePath::new(
                r"C:\Deliveries\2026-10\batch-00001\payload\file-00001.bin",
            )))
            .child(div().w(px(240.0)).child(skeleton_bar(0.6, false)))
            .child(div().w(px(240.0)).child(skeleton_bar(1.0, true)))
            .child(tabular(body("1,234,567 bytes")))
            .child(muted("Muted body"))
            .child(small("Small"))
            .child(mono("mono-text"))
            .child(mono_small_muted("mono small muted"))
    }
}

fn open_gallery(cx: &mut TestAppContext) -> (Entity<Gallery>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::theme::apply(ThemeMode::Light, cx);
    });
    let slot: Rc<RefCell<Option<Entity<Gallery>>>> = Rc::default();
    let slot_in = slot.clone();
    let (_root, vcx) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| Gallery {
            check: CheckState::Unchecked,
            editable: cx.new(|cx| InputState::new(window, cx).placeholder("C:\\jobs")),
            fixed: cx.new(|cx| {
                let mut s = InputState::new(window, cx);
                s.set_value("read only", window, cx);
                s
            }),
            format: None,
            more: false,
        });
        *slot_in.borrow_mut() = Some(view.clone());
        Root::new(view, window, cx)
    });
    vcx.simulate_resize(size(px(800.0), px(900.0)));
    vcx.run_until_parked();
    let view = slot.borrow_mut().take().expect("gallery created");
    (view, vcx)
}

#[gpui::test]
fn gallery_draws_and_every_control_is_a_tab_stop(cx: &mut TestAppContext) {
    let (_view, cx) = open_gallery(cx);

    // Tab walks every control once and comes back to the start. The stops:
    // secondary, primary, quiet, danger, icon, icon-only, busy (focusable: it
    // is working, not unavailable), checkbox, select, editable input, read-only
    // input (focusable and selectable), banner action, details, dismiss, the
    // disclosure. The disabled button is skipped.
    let mut seen = Vec::new();
    cx.update(Window::focus_next);
    for _ in 0..40 {
        let id = cx.update(|window, cx| window.focused(cx).map(|h| format!("{h:?}")));
        let id = id.expect("something has focus");
        if seen.contains(&id) {
            break;
        }
        seen.push(id);
        cx.simulate_keystrokes("tab");
    }
    assert_eq!(seen.len(), 15, "tab stops: {seen:?}");
}
