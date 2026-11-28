//! View tests of the Tar Packager's frame and everything above the entry
//! table, in a headless window under `Root`, over [`FixtureTarpack`] and
//! [`FakePlatform`].

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use fm_tarpack::session::{TarpackError, TarpackErrorKind};
use fm_tarpack::sources::{DropOutcome, Unmatched, UnmatchedReason};
use gpui::Focusable as _;
use gpui::{
    px, size, AppContext as _, Entity, Modifiers, Pixels, TestAppContext, VisualTestContext, Window,
};
use gpui_component::table::TableState;
use gpui_component::{Root, ThemeMode};

use super::entry_cells;
use super::entry_table::{self, EntryDelegate};
use super::shortcuts::SHORTCUT_ROWS;
use super::TarpackView;
use crate::app::dev_mode::DevMode;
use crate::jobs::WorkerGate;
use crate::platform;
use crate::platform::fake::{FakePlatform, PlatformCall};
use crate::theme::tokens::SCROLLBAR_GUTTER;
use crate::tools::tarpack::fixtures::{Call, FixtureTarpack, Gate, TarpackFixture};
use crate::tools::tarpack::model::{DropState, TarpackModel, SKELETON_DELAY};

struct Host {
    model: Entity<TarpackModel>,
    view: Entity<TarpackView>,
    backend: FixtureTarpack,
    platform: FakePlatform,
}

fn open(cx: &mut TestAppContext, backend: FixtureTarpack) -> (Host, &mut VisualTestContext) {
    cx.executor().allow_parking();
    let fake = FakePlatform::new();
    let installed = fake.clone();
    cx.update(|cx| {
        gpui_component::init(cx);
        DevMode::init(cx);
        crate::theme::apply(ThemeMode::Light, cx);
        platform::set_platform(cx, installed);
    });
    let mut parts = None;
    let boxed = backend.clone();
    let (_root, vcx) = cx.add_window_view(|window, cx| {
        let model = cx.new(|cx| TarpackModel::new(Box::new(boxed), cx));
        let view = cx.new(|cx| TarpackView::new(model.clone(), window, cx));
        parts = Some((model, view.clone()));
        Root::new(view, window, cx)
    });
    vcx.simulate_resize(size(px(1100.0), px(800.0)));
    let (model, view) = parts.expect("view created");
    (
        Host {
            model,
            view,
            backend,
            platform: fake,
        },
        vcx,
    )
}

/// Runs everything queued until nothing loads and no drop is pending.
fn idle(cx: &mut VisualTestContext, host: &Host) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        cx.run_until_parked();
        let done = host.model.read_with(cx, |m, _| {
            !m.loading() && !matches!(m.drop_state(), Some(DropState::Pending))
        });
        if done {
            cx.run_until_parked();
            return;
        }
        assert!(Instant::now() < deadline, "never went idle");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Draws until the table has measured itself and followed (it takes a frame
/// each: measure while painting, then lay the columns out again).
fn settle(vcx: &mut VisualTestContext) {
    for _ in 0..3 {
        vcx.update(|window, cx| window.draw(cx).clear(cx));
        vcx.run_until_parked();
    }
}

/// Runs everything queued until the platform has been asked `n` times.
fn wait_for_calls(cx: &mut VisualTestContext, host: &Host, n: usize) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while host.platform.calls().len() < n {
        cx.run_until_parked();
        assert!(Instant::now() < deadline, "the platform was never asked");
        std::thread::sleep(Duration::from_millis(2));
    }
    cx.run_until_parked();
}

fn open_fixture(
    cx: &mut TestAppContext,
    fixture: TarpackFixture,
) -> (Host, &mut VisualTestContext) {
    let (host, vcx) = open(cx, FixtureTarpack::from_fixture(fixture));
    idle(vcx, &host);
    settle(vcx);
    (host, vcx)
}

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

fn tab(cx: &mut VisualTestContext) {
    cx.update(Window::focus_next);
    cx.run_until_parked();
}

fn has(cx: &mut VisualTestContext, name: &'static str) -> bool {
    cx.debug_bounds(name).is_some()
}

fn bounds(cx: &mut VisualTestContext, name: &str) -> gpui::Bounds<Pixels> {
    let name: &'static str = Box::leak(name.to_string().into_boxed_str());
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("no element {name}"))
}

fn click(cx: &mut VisualTestContext, name: &str) {
    let at = bounds(cx, name).center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

fn act(
    cx: &mut VisualTestContext,
    host: &Host,
    f: impl FnOnce(&mut TarpackModel, &mut gpui::Context<TarpackModel>),
) {
    host.model.update(cx, f);
    cx.run_until_parked();
}

#[gpui::test]
fn renders_manifest_states(cx: &mut TestAppContext) {
    // Empty: the empty state, no header, no recent menu.
    let (_host, vcx) = open_fixture(cx, TarpackFixture::Empty);
    assert!(has(vcx, "tarpack-open"));
    assert!(has(vcx, "tarpack-create-example"));
    assert!(has(vcx, "tarpack-shortcuts"));
    assert!(!has(vcx, "tarpack-header"));
    assert!(!has(vcx, "tarpack-recent"), "nothing recent");
    assert!(!has(vcx, "tarpack-errors"));
}

#[gpui::test]
fn renders_loaded_manifest(cx: &mut TestAppContext) {
    let (_host, vcx) = open_fixture(cx, TarpackFixture::Loaded);
    for name in [
        "tarpack-header",
        "tarpack-open",
        "tarpack-recent",
        "tarpack-reload",
        "tarpack-shortcuts",
    ] {
        assert!(has(vcx, name), "{name}");
    }
    for name in [
        "tarpack-create-example",
        "tarpack-edit",
        "tarpack-errors",
        "tarpack-changed",
        "tarpack-error",
        "tarpack-drop-result",
    ] {
        assert!(!has(vcx, name), "{name} is not shown");
    }
    // The shortcuts button is last in the header.
    let header = bounds(vcx, "tarpack-header");
    let shortcuts = bounds(vcx, "tarpack-shortcuts");
    assert!(shortcuts.right() <= header.right());
    assert!(bounds(vcx, "tarpack-reload").right() <= shortcuts.left());
}

#[gpui::test]
fn renders_errors_withheld_and_changed(cx: &mut TestAppContext) {
    // Errors: the region (open at first), the warnings.
    let (_host, vcx) = open_fixture(cx, TarpackFixture::Errors);
    assert!(has(vcx, "tarpack-errors"));
    assert!(has(vcx, "tarpack-errors-toggle"));
    assert!(has(vcx, "tarpack-warnings-toggle"));
    assert!(has(vcx, "tarpack-header"));

    // Withheld: the same region.
    let (_host, vcx) = open_fixture(cx, TarpackFixture::Withheld);
    assert!(has(vcx, "tarpack-errors"));
    assert!(!has(vcx, "tarpack-warnings-toggle"));

    // Changed on disk: the banner with its Reload action.
    let (host, vcx) = open_fixture(cx, TarpackFixture::Ready);
    assert!(!has(vcx, "tarpack-changed"));
    act(vcx, &host, TarpackModel::file_changed);
    assert!(has(vcx, "tarpack-changed"));
    assert!(has(vcx, "tarpack-changed-reload"));
}

#[gpui::test]
fn renders_error_banner_and_dismisses(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Loaded);
    host.backend
        .fail_next(TarpackError::new(TarpackErrorKind::Io, "disk on fire"));
    click(vcx, "tarpack-reload");
    idle(vcx, &host);
    assert!(has(vcx, "tarpack-error"), "banner");
    click(vcx, "banner-dismiss");
    assert!(!has(vcx, "tarpack-error"));
}

#[gpui::test]
fn skeleton_only_when_model_says(cx: &mut TestAppContext) {
    let gate = cx.update(WorkerGate::install_closed);
    let (host, vcx) = open(cx, FixtureTarpack::loaded());
    vcx.run_until_parked();
    assert!(
        host.model.read_with(vcx, |m, _| m.loading()),
        "gate closed: loading"
    );
    // Within the delay nothing shows yet, not even the skeleton.
    assert!(!host.model.read_with(vcx, |m, _| m.show_skeleton()));
    assert!(!has(vcx, "tarpack-skeleton"));

    vcx.executor().advance_clock(SKELETON_DELAY);
    vcx.run_until_parked();
    assert!(host.model.read_with(vcx, |m, _| m.loading()));
    assert!(host.model.read_with(vcx, |m, _| m.show_skeleton()));
    assert!(has(vcx, "tarpack-skeleton"), "skeleton after the delay");
    assert!(!has(vcx, "tarpack-open"));
    assert!(!has(vcx, "tarpack-shortcuts"));

    gate.open();
    idle(vcx, &host);
    assert!(!host.model.read_with(vcx, |m, _| m.loading()));
    assert!(!has(vcx, "tarpack-skeleton"), "gone once loaded");
    assert!(has(vcx, "tarpack-header"));
}

#[gpui::test]
fn header_actions_call_model(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Loaded);
    click(vcx, "tarpack-open");
    idle(vcx, &host);
    assert!(
        host.platform
            .calls()
            .iter()
            .any(|c| matches!(c, PlatformCall::Pick(_))),
        "Open asks the platform for a file"
    );

    click(vcx, "tarpack-reload");
    idle(vcx, &host);
    assert!(host.backend.calls().contains(&Call::Reload));

    // The Recent menu by keys: Tab to it, Down opens, Enter picks the first.
    tab(vcx);
    tab(vcx);
    press(vcx, "down");
    assert!(has(vcx, "tarpack-recent-item-0"), "the list is open");
    press(vcx, "enter");
    idle(vcx, &host);
    assert!(host.backend.calls().contains(&Call::OpenRecent(0)));
}

#[gpui::test]
fn empty_state_buttons_call_model(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Empty);
    click(vcx, "tarpack-create-example");
    wait_for_calls(vcx, &host, 1);
    assert!(host
        .platform
        .calls()
        .iter()
        .any(|c| matches!(c, PlatformCall::PickSave(_))));
    // Tab reaches the buttons: the shortcuts help comes first, then Open
    // manifest.
    tab(vcx);
    tab(vcx);
    press(vcx, "enter");
    wait_for_calls(vcx, &host, 2);
    assert!(host
        .platform
        .calls()
        .iter()
        .any(|c| matches!(c, PlatformCall::Pick(_))));
}

#[gpui::test]
fn edit_button_follows_dev_mode(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Loaded);
    assert!(!has(vcx, "tarpack-edit"));
    vcx.update(|_, cx| DevMode::toggle(cx));
    vcx.run_until_parked();
    assert!(has(vcx, "tarpack-edit"), "on in developer mode");
    click(vcx, "tarpack-edit");
    idle(vcx, &host);
    assert!(host
        .platform
        .calls()
        .iter()
        .any(|c| matches!(c, PlatformCall::OpenWithSystem(_))));
    vcx.update(|_, cx| DevMode::toggle(cx));
    vcx.run_until_parked();
    assert!(!has(vcx, "tarpack-edit"));
}

#[gpui::test]
fn changed_banner_reload(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Ready);
    act(vcx, &host, TarpackModel::file_changed);
    assert!(has(vcx, "tarpack-changed"));
    click(vcx, "tarpack-changed-reload");
    idle(vcx, &host);
    assert!(host.backend.calls().contains(&Call::Reload));
    assert!(!has(vcx, "tarpack-changed"), "a reload clears it");
}

#[gpui::test]
fn errors_toggle_by_keyboard(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Errors);
    let expanded =
        |vcx: &mut VisualTestContext| host.model.read_with(vcx, |m, _| m.errors_expanded());
    assert!(expanded(vcx), "starts open");
    assert!(has(vcx, "tarpack-errors-report"));

    // Open, Recent, Reload and the shortcuts button come first.
    for _ in 0..5 {
        tab(vcx);
    }
    press(vcx, "enter");
    assert!(!expanded(vcx), "Enter hides");
    assert!(!has(vcx, "tarpack-errors-report"));
    press(vcx, "space");
    assert!(expanded(vcx), "Space shows");
    assert!(has(vcx, "tarpack-errors-report"));
}

/// Tabs until focus is inside the drop result.
fn tab_into_result(vcx: &mut VisualTestContext, host: &Host) {
    let mut guard = 0;
    while !vcx.update(|window, cx| host.view.read(cx).result_focus.contains_focused(window, cx)) {
        tab(vcx);
        guard += 1;
        assert!(guard < 12, "the result holds a tab stop");
    }
}

fn drop_outcome() -> DropOutcome {
    DropOutcome {
        matched: vec![],
        unmatched: vec![Unmatched {
            path: "C:\\drops\\README.md".into(),
            reason: UnmatchedReason::NoEntry,
        }],
        ambiguous: vec![],
    }
}

#[gpui::test]
fn drop_result_lines_and_dismiss(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Loaded);
    assert!(!has(vcx, "tarpack-drop-result"));
    host.backend.script_drop(drop_outcome());
    act(vcx, &host, |m, cx| {
        m.dropped(vec!["C:\\drops\\README.md".into()], cx);
    });
    idle(vcx, &host);
    assert!(has(vcx, "tarpack-drop-result"));
    assert!(has(vcx, "tarpack-drop-paths"), "a line has full paths");
    let lines = host.model.read_with(vcx, |m, _| match m.drop_state() {
        Some(DropState::Result(r)) => r.lines.len(),
        other => panic!("no result: {other:?}"),
    });
    assert_eq!(lines, 1);

    // The disclosure and Dismiss are tab stops: Dismiss by key.
    tab_into_result(vcx, &host);
    tab(vcx);
    press(vcx, "enter");
    assert!(!has(vcx, "tarpack-drop-result"), "dismissed");
    assert!(host.model.read_with(vcx, |m, _| m.drop_state().is_none()));
}

#[gpui::test]
fn focus_follows_hint(cx: &mut TestAppContext) {
    // Report: show_errors opens the report and focuses it (Tab leaves it).
    let (host, vcx) = open_fixture(cx, TarpackFixture::Errors);
    act(vcx, &host, |m, cx| m.set_errors_expanded(false, cx));
    assert!(!has(vcx, "tarpack-errors-report"));
    act(vcx, &host, TarpackModel::show_errors);
    assert!(has(vcx, "tarpack-errors-report"));
    let focused = host.view.read_with(vcx, |v, _| v.report_focus.clone());
    assert!(
        vcx.update(|window, _| focused.is_focused(window)),
        "the report has focus"
    );

    // Reload: focus inside a drop result moves to the Reload button when a
    // reload removes the result.
    let (host, vcx) = open_fixture(cx, TarpackFixture::Loaded);
    host.backend.script_drop(drop_outcome());
    act(vcx, &host, |m, cx| {
        m.dropped(vec!["C:\\drops\\README.md".into()], cx);
    });
    idle(vcx, &host);
    click(vcx, "tarpack-drop-dismiss");
    host.backend.script_drop(drop_outcome());
    act(vcx, &host, |m, cx| {
        m.dropped(vec!["C:\\drops\\README.md".into()], cx);
    });
    idle(vcx, &host);
    // Put focus inside the result: Tab from the last header stop reaches the
    // disclosure.
    let reload = host.view.read_with(vcx, |v, _| v.reload_focus.clone());
    let mut guard = 0;
    while !vcx.update(|window, cx| host.view.read(cx).result_focus.contains_focused(window, cx)) {
        tab(vcx);
        guard += 1;
        assert!(guard < 12, "the result holds a tab stop");
    }
    act(vcx, &host, TarpackModel::reload);
    idle(vcx, &host);
    assert!(!has(vcx, "tarpack-drop-result"));
    assert!(
        vcx.update(|window, _| reload.is_focused(window)),
        "Reload has focus"
    );
}

#[gpui::test]
fn focus_stays_when_not_lost(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Loaded);
    host.backend.script_drop(drop_outcome());
    act(vcx, &host, |m, cx| {
        m.dropped(vec!["C:\\drops\\README.md".into()], cx);
    });
    idle(vcx, &host);
    // Focus is on Open, outside the result: a Reload hint moves nothing.
    tab(vcx);
    act(vcx, &host, TarpackModel::reload);
    idle(vcx, &host);
    let reload = host.view.read_with(vcx, |v, _| v.reload_focus.clone());
    assert!(!vcx.update(|window, _| reload.is_focused(window)));
}

#[gpui::test]
fn shortcuts_keys_do_not_overflow_into_actions(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Loaded);
    vcx.update(|_, cx| DevMode::toggle(cx));
    vcx.run_until_parked();
    click(vcx, "tarpack-shortcuts");
    let rows = SHORTCUT_ROWS.len();
    assert!(has(vcx, "tarpack-shortcuts-popover"));
    for i in 0..rows {
        let key = bounds(vcx, &format!("tarpack-shortcuts-last-key-{i}"));
        let action = bounds(vcx, &format!("tarpack-shortcuts-action-{i}"));
        assert!(
            key.right() <= action.left(),
            "row {i}: keys end at {:?}, the action starts at {:?}",
            key.right(),
            action.left()
        );
    }
    let _ = host;
}

#[gpui::test]
fn recent_menu_hidden_when_empty(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::loaded();
    backend.set_recent(&[]);
    let (host, vcx) = open(cx, backend);
    idle(vcx, &host);
    assert!(has(vcx, "tarpack-header"));
    assert!(!has(vcx, "tarpack-recent"));
}

#[gpui::test]
fn drop_pending_line_after_the_delay(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::loaded();
    let gate = Gate::new();
    backend.hold_drops(&gate);
    let (host, vcx) = open(cx, backend);
    idle(vcx, &host);
    act(vcx, &host, |m, cx| {
        m.dropped(vec!["C:\\x\\gateway".into()], cx);
    });
    assert!(!has(vcx, "tarpack-drop-result"), "not before 150 ms");
    vcx.executor().advance_clock(Duration::from_millis(150));
    vcx.run_until_parked();
    assert!(has(vcx, "tarpack-drop-result"), "the matching line");
    assert!(!has(vcx, "tarpack-drop-dismiss"), "no button on it");
    gate.open();
    idle(vcx, &host);
    assert!(has(vcx, "tarpack-drop-dismiss"), "the result replaces it");
}

// ---- the entry table (U10) ----

/// A manifest of `n` entries, none assigned.
fn big_manifest(n: usize) -> String {
    let mut text = String::from("version = 1\nname = \"Big\"\n");
    for i in 0..n {
        let _ = writeln!(
            text,
            "[[file]]\nid = \"f{i}\"\nsource = \"file-{i}.bin\"\ndir = \"/srv/gateway/in\""
        );
    }
    text
}

/// A host showing `n` unassigned entries.
fn open_big(cx: &mut TestAppContext, n: usize) -> (Host, &mut VisualTestContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Loaded);
    host.backend.set_text(&big_manifest(n));
    act(vcx, &host, TarpackModel::reload);
    idle(vcx, &host);
    settle(vcx);
    (host, vcx)
}

fn table_state(vcx: &mut VisualTestContext, host: &Host) -> Entity<TableState<EntryDelegate>> {
    host.view.read_with(vcx, |v, cx| {
        v.entry_table
            .as_ref()
            .expect("the table was made")
            .read(cx)
            .table()
            .clone()
    })
}

fn focus_table(vcx: &mut VisualTestContext, host: &Host) {
    let table = table_state(vcx, host);
    vcx.update(|window, cx| {
        let handle = table.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
    vcx.run_until_parked();
}

fn selected_row(vcx: &mut VisualTestContext, host: &Host) -> Option<usize> {
    let table = table_state(vcx, host);
    table.read_with(vcx, |t, _| t.selected_row())
}

fn resize(vcx: &mut VisualTestContext, width: f32, height: f32) {
    vcx.simulate_resize(size(px(width), px(height)));
    settle(vcx);
}

#[gpui::test]
fn table_renders_status_cells(cx: &mut TestAppContext) {
    // Missing: five rows, the second chosen file is gone.
    let (host, vcx) = open_fixture(cx, TarpackFixture::Missing);
    for row in 0..5 {
        assert!(has(vcx, format!("tarpack-row-{row}").leak()), "row {row}");
        assert!(has(vcx, format!("tarpack-browse-{row}").leak()));
        // Every entry is assigned here, so every row can be cleared.
        assert!(has(vcx, format!("tarpack-clear-{row}").leak()));
    }
    assert!(!has(vcx, "tarpack-row-5"));
    assert!(has(vcx, "tarpack-table"));
    assert!(!has(vcx, "tarpack-no-entries"));
    let statuses: Vec<_> = host.model.read_with(vcx, |m, _| {
        m.session()
            .and_then(|s| s.manifest.as_ref())
            .expect("manifest")
            .entries
            .iter()
            .map(|e| entry_cells::status_parts(e.status).2)
            .collect()
    });
    assert_eq!(statuses.iter().filter(|w| **w == "Missing").count(), 1);
    assert_eq!(statuses.iter().filter(|w| **w == "Ready").count(), 4);

    // Partial: Clear only where a file is assigned.
    let (_host, vcx) = open_fixture(cx, TarpackFixture::Partial);
    assert!(has(vcx, "tarpack-clear-0"));
    assert!(
        !has(vcx, "tarpack-clear-3"),
        "an unassigned row has no Clear"
    );
    assert!(has(vcx, "tarpack-browse-3"));
}

#[gpui::test]
fn crlf_marker_shown_for_converting_entries(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Ready);
    let converting: Vec<bool> = host.model.read_with(vcx, |m, _| {
        m.session()
            .and_then(|s| s.manifest.as_ref())
            .expect("manifest")
            .entries
            .iter()
            .map(|e| e.view.normalize_eol)
            .collect()
    });
    assert_eq!(converting.iter().filter(|c| **c).count(), 1, "one converts");
    for (row, converts) in converting.iter().enumerate() {
        assert_eq!(
            has(vcx, format!("tarpack-eol-{row}").leak()),
            *converts,
            "row {row}"
        );
    }
}

#[gpui::test]
fn down_moves_focus_and_scrolls(cx: &mut TestAppContext) {
    let (host, vcx) = open_big(cx, 10_000);
    focus_table(vcx, &host);
    assert_eq!(selected_row(vcx, &host), None);
    for _ in 0..60 {
        press(vcx, "down");
    }
    assert_eq!(selected_row(vcx, &host), Some(59));
    assert!(has(vcx, "tarpack-row-59"), "row 59 is drawn");
    assert!(!has(vcx, "tarpack-row-0"), "row 0 scrolled away");
    assert!(
        !has(vcx, "tarpack-row-200"),
        "rows far below are not drawn: the list is virtual"
    );
    // The selected row is inside the table.
    let row = bounds(vcx, "tarpack-row-59");
    let table = bounds(vcx, "tarpack-table");
    assert!(row.top() >= table.top() && row.bottom() <= table.bottom() + px(1.0));
    // Up does not wrap past the first row.
    for _ in 0..70 {
        press(vcx, "up");
    }
    assert_eq!(selected_row(vcx, &host), Some(0));
    press(vcx, "up");
    assert_eq!(selected_row(vcx, &host), Some(0), "no wrapping");
}

#[gpui::test]
fn enter_browses_focused_row(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Ready);
    focus_table(vcx, &host);
    press(vcx, "enter");
    assert!(
        host.platform.calls().is_empty(),
        "no row, nothing to browse"
    );
    press(vcx, "down");
    press(vcx, "down");
    press(vcx, "enter");
    wait_for_calls(vcx, &host, 1);
    assert!(matches!(host.platform.calls()[0], PlatformCall::Pick(_)));
    idle(vcx, &host);
    // Enter on the table did one browse, not two.
    assert_eq!(host.platform.calls().len(), 1);

    // The Browse button of a row does the same by click.
    click(vcx, "tarpack-browse-3");
    wait_for_calls(vcx, &host, 2);
}

#[gpui::test]
fn delete_clears_only_assigned_row(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Partial);
    let cleared = |host: &Host| -> Vec<String> {
        host.backend
            .calls()
            .into_iter()
            .filter_map(|c| match c {
                Call::Clear(id) => Some(id),
                _ => None,
            })
            .collect()
    };
    focus_table(vcx, &host);
    // Row 3 is unassigned: Delete does nothing.
    for _ in 0..4 {
        press(vcx, "down");
    }
    assert_eq!(selected_row(vcx, &host), Some(3));
    press(vcx, "delete");
    idle(vcx, &host);
    assert_eq!(cleared(&host), Vec::<String>::new());
    // Row 0 is assigned.
    for _ in 0..3 {
        press(vcx, "up");
    }
    press(vcx, "delete");
    idle(vcx, &host);
    assert_eq!(cleared(&host), vec!["gateway-bin".to_string()]);
    assert!(!has(vcx, "tarpack-clear-0"), "the row no longer has a file");
    // Clear by the button leaves focus on the table.
    click(vcx, "tarpack-clear-1");
    idle(vcx, &host);
    assert_eq!(cleared(&host).len(), 2);
    let table = table_state(vcx, &host);
    let focused = vcx.update(|window, cx| table.read(cx).focus_handle(cx).is_focused(window));
    assert!(focused, "focus did not fall to the window");
}

/// Starts a build and runs until the model reports it building.
fn start_build(vcx: &mut VisualTestContext, host: &Host) {
    act(vcx, host, TarpackModel::build);
    let deadline = Instant::now() + Duration::from_secs(20);
    while !host.model.read_with(vcx, |m, _| m.building()) {
        vcx.run_until_parked();
        assert!(Instant::now() < deadline, "the build never started");
        std::thread::sleep(Duration::from_millis(2));
    }
    vcx.run_until_parked();
}

#[gpui::test]
fn keys_inert_while_building(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Building);
    focus_table(vcx, &host);
    press(vcx, "down");
    assert_eq!(selected_row(vcx, &host), Some(0));
    let table = table_state(vcx, &host);
    let tab_stop =
        |vcx: &mut VisualTestContext| vcx.update(|_, cx| table.read(cx).focus_handle(cx).tab_stop);
    assert!(tab_stop(vcx), "the table is a Tab stop");

    start_build(vcx, &host);
    let picks = host.platform.calls().len();
    // The build job records its own calls (`BuildRun`) on its worker thread,
    // at a time the test does not control, so count only the calls the keys
    // could cause: a Clear or an Assign.
    let edits = |host: &Host| {
        host.backend
            .calls()
            .iter()
            .filter(|c| matches!(c, Call::Clear(_) | Call::Assign(..)))
            .count()
    };
    let before = edits(&host);
    for key in ["down", "down", "end", "enter", "delete"] {
        press(vcx, key);
    }
    assert_eq!(selected_row(vcx, &host), Some(0), "the selection stays");
    assert_eq!(host.platform.calls().len(), picks, "no Browse");
    assert_eq!(edits(&host), before, "no Clear");
    assert!(!tab_stop(vcx), "the table leaves the Tab order");

    host.backend.release_build();
    idle(vcx, &host);
    let deadline = Instant::now() + Duration::from_secs(20);
    while host.model.read_with(vcx, |m, _| m.building()) {
        vcx.run_until_parked();
        assert!(Instant::now() < deadline, "the build never finished");
        std::thread::sleep(Duration::from_millis(2));
    }
    vcx.run_until_parked();
    assert!(tab_stop(vcx), "and comes back");
    press(vcx, "down");
    assert_eq!(selected_row(vcx, &host), Some(1), "the keys work again");
}

/// Whether focus is on the table or inside it.
fn table_has_focus(vcx: &mut VisualTestContext, host: &Host) -> bool {
    let table = table_state(vcx, host);
    vcx.update(|window, cx| {
        let handle = table.read(cx).focus_handle(cx);
        handle.contains_focused(window, cx)
    })
}

#[gpui::test]
fn tab_skips_the_table_while_building(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Building);
    // Control: with no build, Tab reaches the table.
    vcx.update(Window::blur);
    let mut reached = false;
    for _ in 0..40 {
        tab(vcx);
        reached |= table_has_focus(vcx, &host);
    }
    assert!(reached, "Tab reaches the table when nothing builds");

    start_build(vcx, &host);
    vcx.update(Window::blur);
    for step in 0..40 {
        tab(vcx);
        assert!(
            !table_has_focus(vcx, &host),
            "Tab {step} landed in the table while building"
        );
    }
    // Shift+Tab does not land there either.
    for step in 0..40 {
        vcx.update(Window::focus_prev);
        vcx.run_until_parked();
        assert!(
            !table_has_focus(vcx, &host),
            "Shift+Tab {step} landed in the table while building"
        );
    }
    host.backend.release_build();
    idle(vcx, &host);
}

#[gpui::test]
fn narrow_window_scrolls_sideways(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Ready);
    resize(vcx, 1000.0, 700.0);
    let table = table_state(vcx, &host);
    let (max, bounds_w) = table.read_with(vcx, |t, _| {
        (
            t.horizontal_scroll_handle.max_offset().x,
            t.horizontal_scroll_handle.bounds().size.width,
        )
    });
    assert!(
        max > px(0.0),
        "the content is wider than the table: {max:?}"
    );
    assert!(bounds_w < px(1024.0));
    // No row stacks: every row has the one height.
    for row in 0..5 {
        let b = bounds(vcx, &format!("tarpack-row-{row}"));
        assert_eq!(b.size.height, px(entry_table::ROW_HEIGHT), "row {row}");
    }
    // Right scrolls the columns sideways; Left brings them back.
    focus_table(vcx, &host);
    press(vcx, "right");
    let x = table.read_with(vcx, |t, _| t.horizontal_scroll_handle.offset().x);
    assert!(x < px(0.0), "scrolled: {x:?}");
    assert_eq!(
        selected_row(vcx, &host),
        None,
        "Right does not turn into a column selection"
    );
    press(vcx, "left");
    let x = table.read_with(vcx, |t, _| t.horizontal_scroll_handle.offset().x);
    assert_eq!(x, px(0.0));
}

#[gpui::test]
fn column_widths_follow_width(cx: &mut TestAppContext) {
    // The pure rule.
    let wide = entry_table::column_widths(1366.0);
    assert_eq!(wide[0], 116.0);
    assert_eq!(wide[4], 132.0);
    assert_eq!(wide[5], 80.0);
    assert_eq!(wide[6], 176.0);
    assert!((wide.iter().sum::<f32>() - 1366.0).abs() < 0.01);
    assert_eq!(wide[1], wide[2]);
    assert_eq!(wide[2], wide[3]);
    // Below 1024 the shares stop shrinking.
    let narrow = entry_table::column_widths(800.0);
    assert!((narrow.iter().sum::<f32>() - 1024.0).abs() < 0.01);
    assert!((narrow[1] - 520.0 / 3.0).abs() < 0.01);

    // In the window: the table's width, less the padding, is shared out.
    let (host, vcx) = open_fixture(cx, TarpackFixture::Ready);
    let table = table_state(vcx, &host);
    let widths = |vcx: &mut VisualTestContext| table.read_with(vcx, |t, _| t.delegate().widths());
    resize(vcx, 1400.0, 800.0);
    let a = widths(vcx);
    assert!(
        (a.iter().sum::<f32>() - (1400.0 - 48.0 - SCROLLBAR_GUTTER)).abs() < 1.0,
        "{a:?}"
    );
    resize(vcx, 1200.0, 800.0);
    let b = widths(vcx);
    assert!(b[1] < a[1], "{a:?} then {b:?}");
    assert!(
        (b.iter().sum::<f32>() - (1200.0 - 48.0 - SCROLLBAR_GUTTER)).abs() < 1.0,
        "{b:?}"
    );
    assert_eq!(b[0], 116.0);
}

#[gpui::test]
fn column_x_positions_match_today_with_sidebar(cx: &mut TestAppContext) {
    use crate::app::shell::test_support::open as open_shell;
    use crate::tools::{self, Backend, BackendSet, FixtureId};

    // The whole shell at 1366x800, the sidebar and the `ready` fixture.
    let view = std::rc::Rc::new(std::cell::RefCell::new(None));
    let grab = view.clone();
    let build = move |window: &mut Window, cx: &mut gpui::App| {
        cx.set_global(BackendSet {
            tarpack: Backend::Fixture(FixtureId::Tarpack(TarpackFixture::Ready)),
            ..BackendSet::fixtures()
        });
        let slots = tools::build(window, cx);
        let ours = slots
            .iter()
            .find(|s| s.id == tools::ToolId::Tarpack)
            .and_then(|s| s.view.clone().downcast::<TarpackView>().ok());
        *grab.borrow_mut() = ours;
        slots
    };
    let (_shell, vcx) = open_shell(cx, build);
    vcx.simulate_resize(size(px(1366.0), px(800.0)));
    click(vcx, "tool-nav-tarpack");
    let deadline = Instant::now() + Duration::from_secs(20);
    while !has(vcx, "tarpack-table") {
        vcx.run_until_parked();
        settle(vcx);
        assert!(Instant::now() < deadline, "the table never appeared");
        std::thread::sleep(Duration::from_millis(2));
    }
    settle(vcx);

    // Each column's left edge: the table's left edge plus the widths before it.
    // The widths are what the columns were laid out at, so they must also fill
    // the table, or the edges are not where the cells are.
    let table = bounds(vcx, "tarpack-table");
    let row = bounds(vcx, "tarpack-row-0");
    assert!(
        (row.left() - table.left()).abs() < px(0.5),
        "rows start at the table"
    );
    let state = view
        .borrow()
        .as_ref()
        .expect("the Tar Packager view")
        .read_with(vcx, |v, cx| {
            v.entry_table
                .as_ref()
                .expect("the table was made")
                .read(cx)
                .table()
                .clone()
        });
    let widths = state.read_with(vcx, |t, _| t.delegate().widths());
    assert!(
        (widths.iter().sum::<f32>() - f32::from(table.size.width)).abs() < 1.0,
        "{widths:?} fill {:?}",
        table.size.width
    );
    let mut left = f32::from(table.left());
    let mut edges = Vec::new();
    for w in widths {
        edges.push(left);
        left += w;
    }
    for (got, want) in edges
        .iter()
        .zip([216.0, 332.0, 534.0, 737.0, 939.0, 1071.0, 1151.0])
    {
        assert!(
            (got - want).abs() <= 2.0,
            "column at {got}, wanted {want}: {edges:?}"
        );
    }
}

#[gpui::test]
fn table_scrolls_inside_itself_when_short(cx: &mut TestAppContext) {
    let (host, vcx) = open_big(cx, 300);
    resize(vcx, 1100.0, 560.0);
    let body = host.view.read_with(vcx, |v, _| v.body_scroll.clone());
    focus_table(vcx, &host);
    for _ in 0..30 {
        press(vcx, "down");
    }
    assert_eq!(selected_row(vcx, &host), Some(29));
    assert_eq!(body.offset().y, px(0.0), "the body did not scroll");
    assert!(
        body.max_offset().y <= px(0.5),
        "nothing to scroll in the body"
    );
    settle(vcx);
    assert!(
        !has(vcx, "tarpack-row-0"),
        "row 0 scrolled out of the table"
    );
    assert!(has(vcx, "tarpack-row-29"), "the selected row is drawn");
    // The table is at least three rows tall.
    let b = bounds(vcx, "tarpack-table");
    assert!(b.size.height >= px(entry_table::HEADER_HEIGHT + 3.0 * entry_table::ROW_HEIGHT));

    // Shorter than everything above plus three rows: the body scrolls.
    resize(vcx, 1100.0, 260.0);
    assert!(body.max_offset().y > px(0.0), "the fallback scroll");
}

#[gpui::test]
fn build_bar_stays_visible_with_many_rows(cx: &mut TestAppContext) {
    let (host, vcx) = open_big(cx, 10_000);
    resize(vcx, 1366.0, 800.0);
    let b = bounds(vcx, "tarpack-table");
    // The table ends inside the frame's bottom padding, so what is below it
    // (the result and the build bar) is in view.
    assert!(
        b.bottom() <= px(800.0 - 16.0),
        "the table ends at {:?}",
        b.bottom()
    );
    assert!(b.size.height > px(200.0));
    let body = host.view.read_with(vcx, |v, _| v.body_scroll.clone());
    assert_eq!(body.offset().y, px(0.0));
}

#[test]
fn file_name_is_what_follows_the_last_separator() {
    use crate::tools::tarpack::text::file_name;
    // `pathParts.test.ts`: fileName.
    assert_eq!(file_name(r"C:\out\gateway.tar.zst"), "gateway.tar.zst");
    assert_eq!(file_name("C:/out/\u{1F600}.tar"), "\u{1F600}.tar");
    assert_eq!(file_name("plain"), "plain");
    assert_eq!(file_name("/opt/gateway/bin/gateway"), "gateway");
    assert_eq!(file_name("C:\\d\\bad\u{FFFD}.txt"), "bad\u{FFFD}.txt");
}

#[test]
fn display_folder_shows_a_lossy_folder_as_is() {
    use crate::tools::tarpack::text::display_folder;
    // `pathParts.test.ts`: displayFolder (the assignedFolder half is
    // `browse_starts_in_a_folder_that_is_not_valid_unicode`).
    assert_eq!(display_folder(r"C:\out\a.tar").as_deref(), Some(r"C:\out"));
    assert_eq!(display_folder(r"C:\a.tar").as_deref(), Some(r"C:\"));
    assert_eq!(
        display_folder("C:\\o\u{FFFD}t\\a.tar").as_deref(),
        Some("C:\\o\u{FFFD}t")
    );
    assert_eq!(display_folder("a.tar"), None);
}

// ---- the build side (U11) ----

/// Runs everything queued until `done` holds of the model.
fn wait_until(
    cx: &mut VisualTestContext,
    host: &Host,
    what: &str,
    done: impl Fn(&TarpackModel) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        cx.run_until_parked();
        if host.model.read_with(cx, |m, _| done(m)) {
            settle(cx);
            return;
        }
        assert!(Instant::now() < deadline, "never: {what}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Waits for a started build to end in a result or the confirmation.
fn built(cx: &mut VisualTestContext, host: &Host) {
    wait_until(cx, host, "the build ends", |m| {
        !m.building() && (m.result().is_some() || m.confirming_overwrite().is_some())
    });
}

fn build_calls(host: &Host) -> usize {
    host.backend
        .calls()
        .iter()
        .filter(|c| matches!(c, Call::BuildRun { .. }))
        .count()
}

fn is_focused(vcx: &mut VisualTestContext, handle: &gpui::FocusHandle) -> bool {
    vcx.update(|window, _| handle.is_focused(window))
}

fn key_down(cx: &mut VisualTestContext, key: &str, is_held: bool) {
    let keystroke = cx
        .update(|_, cx| gpui::Keystroke::parse_with_os(key, cx.platform_info().os))
        .expect("keystroke");
    cx.simulate_event(gpui::KeyDownEvent {
        keystroke,
        is_held,
        prefer_character_input: false,
    });
    cx.run_until_parked();
}

fn key_up(cx: &mut VisualTestContext, key: &str) {
    let keystroke = cx
        .update(|_, cx| gpui::Keystroke::parse_with_os(key, cx.platform_info().os))
        .expect("keystroke");
    cx.simulate_event(gpui::KeyUpEvent { keystroke });
    cx.run_until_parked();
}

#[gpui::test]
fn bar_shows_reason_or_note(cx: &mut TestAppContext) {
    // Nothing assigned and no output: the reason, not the note.
    let (host, vcx) = open_fixture(cx, TarpackFixture::Loaded);
    assert!(has(vcx, "tarpack-build-bar"));
    for name in ["tarpack-choose-output", "tarpack-format", "tarpack-build"] {
        assert!(has(vcx, name), "{name}");
    }
    assert!(has(vcx, "tarpack-build-reason"));
    assert!(!has(vcx, "tarpack-build-note"));
    assert!(!has(vcx, "tarpack-show-errors"));
    assert!(!has(vcx, "tarpack-progress"));
    assert!(host.model.read_with(vcx, |m, _| m.build_reason().is_some()));
    // The disabled button builds nothing.
    click(vcx, "tarpack-build");
    assert_eq!(build_calls(&host), 0);

    // Everything ready: neither.
    let (_host, vcx) = open_fixture(cx, TarpackFixture::Ready);
    assert!(!has(vcx, "tarpack-build-reason"));
    assert!(!has(vcx, "tarpack-build-note"));

    // Errors: the note says what a build leaves out, and Show errors is there.
    let (host, vcx) = open_fixture(cx, TarpackFixture::Errors);
    assert!(has(vcx, "tarpack-build-note"));
    assert!(!has(vcx, "tarpack-build-reason"));
    assert!(host
        .model
        .read_with(vcx, |m, _| m.left_out_note().is_some()));
    act(vcx, &host, |m, cx| m.set_errors_expanded(false, cx));
    click(vcx, "tarpack-show-errors");
    assert!(has(vcx, "tarpack-errors-report"), "Show errors opens it");

    // No manifest: the bar is there, its output controls are not usable.
    let (host, vcx) = open_fixture(cx, TarpackFixture::Empty);
    assert!(has(vcx, "tarpack-build-bar"));
    click(vcx, "tarpack-choose-output");
    assert_eq!(host.platform.calls(), Vec::<PlatformCall>::new());
}

#[gpui::test]
fn bar_stays_under_the_table_and_result(cx: &mut TestAppContext) {
    let (_host, vcx) = open_fixture(cx, TarpackFixture::Result);
    click(vcx, "tarpack-build");
    let bar = bounds(vcx, "tarpack-build-bar");
    // Edge to edge, at the bottom of the window.
    assert_eq!(bar.left(), px(0.0));
    assert_eq!(bar.right(), px(1100.0));
    assert_eq!(bar.bottom(), px(800.0));
    assert!(bar.top() >= bounds(vcx, "tarpack-table").bottom());
}

#[gpui::test]
fn choose_output_calls_model(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Ready);
    click(vcx, "tarpack-choose-output");
    wait_for_calls(vcx, &host, 1);
    assert!(matches!(
        host.platform.calls()[0],
        PlatformCall::PickSave(_)
    ));
}

#[gpui::test]
fn format_select_by_keyboard(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Ready);
    let before = host
        .model
        .read_with(vcx, |m, _| m.session().expect("session").format);
    // Focus is on Choose; the next tab stop is the format select.
    let first = host
        .view
        .read_with(vcx, |v, _| v.after_result_focus.clone());
    vcx.update(|window, cx| window.focus(&first, cx));
    tab(vcx);
    press(vcx, "enter");
    press(vcx, "down");
    press(vcx, "enter");
    idle(vcx, &host);
    let set: Vec<_> = host
        .backend
        .calls()
        .into_iter()
        .filter_map(|c| match c {
            Call::SetFormat(f) => Some(f),
            _ => None,
        })
        .collect();
    assert_eq!(set.len(), 1, "one format picked");
    assert_ne!(set[0], before);
    let after = host
        .model
        .read_with(vcx, |m, _| m.session().expect("session").format);
    assert_eq!(after, set[0]);
}

#[gpui::test]
fn build_click_and_ctrl_enter_build(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Result);
    click(vcx, "tarpack-build");
    built(vcx, &host);
    assert_eq!(build_calls(&host), 1);
    assert!(has(vcx, "tarpack-build-result"));
    click(vcx, "tarpack-result-dismiss");
    assert!(!has(vcx, "tarpack-build-result"));
    press(vcx, "ctrl-enter");
    built(vcx, &host);
    assert_eq!(build_calls(&host), 2, "Ctrl+Enter builds");
    assert!(has(vcx, "tarpack-build-result"));
}

#[gpui::test]
fn progress_renders_fraction(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Building);
    click(vcx, "tarpack-build");
    wait_until(vcx, &host, "progress", |m| m.progress().is_some());
    assert!(has(vcx, "tarpack-progress"));
    assert!(
        !has(vcx, "tarpack-build-reason"),
        "no reason while building"
    );
    let fraction = host.model.read_with(vcx, |m, _| {
        crate::tools::tarpack::text::progress_fraction(m.progress())
    });
    assert!(fraction > 0.0 && fraction < 1.0, "midway: {fraction}");
    let track = bounds(vcx, "tarpack-progress-track");
    let fill = bounds(vcx, "tarpack-progress-fill");
    let drawn = f32::from(fill.size.width) / f32::from(track.size.width);
    assert!((drawn - fraction).abs() < 0.02, "{drawn} vs {fraction}");
    // The settings are off while it runs.
    let calls = host.platform.calls().len();
    click(vcx, "tarpack-choose-output");
    assert_eq!(host.platform.calls().len(), calls);
    host.backend.release_build();
    built(vcx, &host);
    assert!(!has(vcx, "tarpack-progress"));
    assert!(has(vcx, "tarpack-build-result"));
}

#[gpui::test]
fn overwrite_dialog_replace_and_cancel(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Replace);
    let create = host.view.read_with(vcx, |v, _| v.create_focus.clone());
    click(vcx, "tarpack-build");
    built(vcx, &host);
    assert!(has(vcx, "dialog-cancel"), "the confirmation is open");
    assert!(has(vcx, "dialog-confirm"));
    assert!(host
        .model
        .read_with(vcx, |m, _| m.confirming_overwrite().is_some()));
    // The dialog starts on Cancel: Enter cancels, and focus lands on Create.
    press(vcx, "enter");
    assert!(!has(vcx, "dialog-cancel"));
    assert!(host
        .model
        .read_with(vcx, |m, _| m.confirming_overwrite().is_none()));
    assert_eq!(build_calls(&host), 1, "nothing was replaced");
    assert!(is_focused(vcx, &create), "Create archive has focus again");

    // Escape cancels too.
    click(vcx, "tarpack-build");
    built(vcx, &host);
    press(vcx, "escape");
    assert!(!has(vcx, "dialog-cancel"));
    assert_eq!(build_calls(&host), 2);

    // Replace builds again, with overwrite.
    click(vcx, "tarpack-build");
    built(vcx, &host);
    click(vcx, "dialog-confirm");
    built(vcx, &host);
    assert!(!has(vcx, "dialog-confirm"));
    assert_eq!(
        host.backend
            .calls()
            .into_iter()
            .filter(|c| matches!(c, Call::BuildRun { overwrite: true }))
            .count(),
        1
    );
    assert!(has(vcx, "tarpack-build-result"));
}

#[gpui::test]
fn result_success_partial_failure_render(cx: &mut TestAppContext) {
    // Success: path, SHA, Show in folder; clean, so no Copy report.
    let (host, vcx) = open_fixture(cx, TarpackFixture::Result);
    click(vcx, "tarpack-build");
    built(vcx, &host);
    for name in [
        "tarpack-build-result",
        "tarpack-result-path",
        "tarpack-result-sha",
        "tarpack-copy-sha",
        "tarpack-reveal",
        "tarpack-result-dismiss",
    ] {
        assert!(has(vcx, name), "{name}");
    }
    assert!(!has(vcx, "tarpack-copy-report"));
    assert!(!has(vcx, "tarpack-build-report"));
    // It sits between the table and the bar.
    let result = bounds(vcx, "tarpack-build-result");
    assert!(result.top() >= bounds(vcx, "tarpack-table").bottom());
    assert!(result.bottom() <= bounds(vcx, "tarpack-build-bar").top());
    click(vcx, "tarpack-reveal");
    wait_for_calls(vcx, &host, 1);
    assert!(matches!(host.platform.calls()[0], PlatformCall::Reveal(_)));

    // Partial: Copy report appears.
    let (host, vcx) = open_fixture(cx, TarpackFixture::ResultPartial);
    click(vcx, "tarpack-build");
    built(vcx, &host);
    assert!(has(vcx, "tarpack-copy-report"));

    // Entries with errors: the report, and its warnings open on a click.
    let (host, vcx) = open_fixture(cx, TarpackFixture::Errors);
    resize(vcx, 1100.0, 1600.0);
    click(vcx, "tarpack-build");
    built(vcx, &host);
    assert!(has(vcx, "tarpack-build-report"));
    assert!(has(vcx, "tarpack-copy-report"));
    assert!(has(vcx, "tarpack-build-warnings-toggle"));
    assert!(!host.view.read_with(vcx, |v, _| v.result_ui.warnings_open));
    click(vcx, "tarpack-build-warnings-toggle");
    assert!(host.view.read_with(vcx, |v, _| v.result_ui.warnings_open));
    // The report scrolls inside itself, at most 40% of the window.
    let report = bounds(vcx, "tarpack-build-report");
    assert!(report.size.height <= px(1600.0 * 0.4 + 1.0));

    // A failure: no archive, no Reveal; the message sits in Details.
    let (host, vcx) = open_fixture(cx, TarpackFixture::Ready);
    host.backend
        .fail_build(TarpackError::new(TarpackErrorKind::Io, "the disk is full"));
    click(vcx, "tarpack-build");
    built(vcx, &host);
    assert!(has(vcx, "tarpack-build-result"));
    assert!(!has(vcx, "tarpack-reveal"));
    assert!(!has(vcx, "tarpack-copy-sha"));
    assert!(!host.view.read_with(vcx, |v, _| v.result_ui.details_open));
    click(vcx, "tarpack-build-details-toggle");
    assert!(host.view.read_with(vcx, |v, _| v.result_ui.details_open));
    click(vcx, "tarpack-result-dismiss");
    assert!(!has(vcx, "tarpack-build-result"));
    assert!(
        !host.view.read_with(vcx, |v, _| v.result_ui.details_open),
        "a new result starts closed"
    );

    // A panicked build: the unexpected result, Details too.
    let (host, vcx) = open_fixture(cx, TarpackFixture::Ready);
    host.backend.panic_in_build();
    click(vcx, "tarpack-build");
    built(vcx, &host);
    assert!(matches!(
        host.model.read_with(vcx, |m, _| m.result().cloned()),
        Some(crate::tools::tarpack::model::BuildOutcome::Unexpected { .. })
    ));
    assert!(has(vcx, "tarpack-build-details-toggle"));
    assert!(!has(vcx, "tarpack-reveal"));
}

#[gpui::test]
fn copy_buttons_copy_and_show_copied(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::ResultPartial);
    click(vcx, "tarpack-build");
    built(vcx, &host);
    let Some(crate::tools::tarpack::model::BuildOutcome::Ok(summary)) =
        host.model.read_with(vcx, |m, _| m.result().cloned())
    else {
        panic!("a result");
    };
    let copied = |vcx: &mut VisualTestContext, host: &Host| {
        host.view.read_with(vcx, |v, _| v.result_ui.copied)
    };
    assert_eq!(copied(vcx, &host), None);
    click(vcx, "tarpack-copy-sha");
    assert_eq!(
        host.platform.calls(),
        [PlatformCall::CopyText(summary.sha256_hex.clone())]
    );
    assert_eq!(copied(vcx, &host), Some(super::build_result::CopyKind::Sha));
    click(vcx, "tarpack-copy-report");
    assert_eq!(
        host.platform.calls()[1],
        PlatformCall::CopyText(crate::tools::tarpack::text::report_text(&summary))
    );
    assert_eq!(
        copied(vcx, &host),
        Some(super::build_result::CopyKind::Report),
        "the note moves to the button just used"
    );
    // "Copied" goes after a moment.
    vcx.executor().advance_clock(Duration::from_secs(3));
    vcx.run_until_parked();
    assert_eq!(copied(vcx, &host), None);
}

#[gpui::test]
fn extraction_command_follows_dev_mode(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Result);
    click(vcx, "tarpack-build");
    built(vcx, &host);
    assert!(!has(vcx, "tarpack-copy-command"));
    assert!(!has(vcx, "tarpack-result-command"));
    vcx.update(|_, cx| DevMode::toggle(cx));
    vcx.run_until_parked();
    settle(vcx);
    assert!(has(vcx, "tarpack-result-command"), "on in developer mode");
    click(vcx, "tarpack-copy-command");
    let command = host
        .model
        .read_with(vcx, |m, cx| m.extraction_command(cx).map(String::from))
        .expect("a command");
    assert_eq!(host.platform.calls(), [PlatformCall::CopyText(command)]);
    vcx.update(|_, cx| DevMode::toggle(cx));
    vcx.run_until_parked();
    settle(vcx);
    assert!(!has(vcx, "tarpack-result-command"));
    assert!(!has(vcx, "tarpack-copy-command"));
}

#[gpui::test]
fn shortcuts_respect_gating(cx: &mut TestAppContext) {
    // No manifest: only Open acts.
    let (host, vcx) = open_fixture(cx, TarpackFixture::Empty);
    for key in ["f5", "ctrl-r", "ctrl-e", "ctrl-enter", "f8", "escape"] {
        press(vcx, key);
    }
    assert!(!host.backend.calls().contains(&Call::Reload));
    assert_eq!(build_calls(&host), 0);
    assert_eq!(host.platform.calls(), Vec::<PlatformCall>::new());
    press(vcx, "ctrl-o");
    wait_for_calls(vcx, &host, 1);
    assert!(matches!(host.platform.calls()[0], PlatformCall::Pick(_)));

    // A manifest with errors: F5 and Ctrl+R reload, F8 opens the report,
    // Ctrl+E waits for developer mode, Ctrl+Enter builds.
    let (host, vcx) = open_fixture(cx, TarpackFixture::Errors);
    let reloads = |host: &Host| {
        host.backend
            .calls()
            .iter()
            .filter(|c| **c == Call::Reload)
            .count()
    };
    press(vcx, "f5");
    idle(vcx, &host);
    press(vcx, "ctrl-r");
    idle(vcx, &host);
    assert_eq!(reloads(&host), 2);
    act(vcx, &host, |m, cx| m.set_errors_expanded(false, cx));
    press(vcx, "f8");
    assert!(has(vcx, "tarpack-errors-report"));
    press(vcx, "ctrl-e");
    assert!(host.platform.calls().is_empty(), "Edit is for developers");
    vcx.update(|_, cx| DevMode::toggle(cx));
    vcx.run_until_parked();
    press(vcx, "ctrl-e");
    wait_for_calls(vcx, &host, 1);
    assert!(matches!(
        host.platform.calls()[0],
        PlatformCall::OpenWithSystem(_)
    ));
    vcx.update(|_, cx| DevMode::toggle(cx));
    press(vcx, "ctrl-enter");
    built(vcx, &host);
    assert_eq!(build_calls(&host), 1);

    // While a build runs, nothing acts.
    let (host, vcx) = open_fixture(cx, TarpackFixture::Building);
    press(vcx, "ctrl-enter");
    wait_until(vcx, &host, "progress", |m| m.progress().is_some());
    let calls = host.backend.calls().len();
    let picks = host.platform.calls().len();
    for key in ["f5", "ctrl-r", "ctrl-o", "f8", "ctrl-enter", "escape"] {
        press(vcx, key);
    }
    assert_eq!(host.backend.calls().len(), calls);
    assert_eq!(host.platform.calls().len(), picks);
    host.backend.release_build();
    built(vcx, &host);
}

#[gpui::test]
fn held_key_acts_once(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Ready);
    let reloads = |host: &Host| {
        host.backend
            .calls()
            .iter()
            .filter(|c| **c == Call::Reload)
            .count()
    };
    key_down(vcx, "f5", false);
    idle(vcx, &host);
    for _ in 0..4 {
        key_down(vcx, "f5", true);
        idle(vcx, &host);
    }
    assert_eq!(reloads(&host), 1, "the repeats did nothing");
    // Released, then pressed again: acts.
    key_up(vcx, "f5");
    key_down(vcx, "f5", false);
    idle(vcx, &host);
    assert_eq!(reloads(&host), 2);

    // A held Escape dismisses one result, not the next.
    let (host, vcx) = open_fixture(cx, TarpackFixture::Loaded);
    host.backend.script_drop(drop_outcome());
    act(vcx, &host, |m, cx| {
        m.dropped(vec!["C:\\drops\\README.md".into()], cx);
    });
    idle(vcx, &host);
    assert!(has(vcx, "tarpack-drop-result"));
    key_down(vcx, "escape", false);
    assert!(!has(vcx, "tarpack-drop-result"));
    key_down(vcx, "escape", true);
    assert!(!host.model.read_with(vcx, |m, cx| m
        .shortcut(crate::tools::tarpack::model::Shortcut::Escape, cx)));
}

#[gpui::test]
fn held_escape_does_not_dismiss_the_second_result(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Result);
    host.backend.script_drop(drop_outcome());
    click(vcx, "tarpack-build");
    built(vcx, &host);
    act(vcx, &host, |m, cx| {
        m.dropped(vec!["C:\\drops\\README.md".into()], cx);
    });
    idle(vcx, &host);
    assert!(has(vcx, "tarpack-drop-result"));
    assert!(has(vcx, "tarpack-build-result"));
    key_down(vcx, "escape", false);
    assert!(!has(vcx, "tarpack-drop-result"), "the drop result first");
    assert!(has(vcx, "tarpack-build-result"));
    key_down(vcx, "escape", true);
    assert!(has(vcx, "tarpack-build-result"), "the repeat does nothing");
    key_up(vcx, "escape");
    key_down(vcx, "escape", false);
    assert!(
        !has(vcx, "tarpack-build-result"),
        "a new press dismisses it"
    );
}

#[gpui::test]
fn escape_dismisses_result_with_table_focused(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Result);
    click(vcx, "tarpack-build");
    built(vcx, &host);
    focus_table(vcx, &host);
    press(vcx, "down");
    assert_eq!(selected_row(vcx, &host), Some(0));
    // The table has focus and a selection: Escape still dismisses the result
    // and leaves the selection alone.
    press(vcx, "escape");
    assert!(!has(vcx, "tarpack-build-result"));
    assert_eq!(selected_row(vcx, &host), Some(0));
    // With nothing to dismiss, Escape reaches the table and clears it.
    press(vcx, "escape");
    assert_eq!(selected_row(vcx, &host), None);
}

#[gpui::test]
fn escape_ignored_while_help_open(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Result);
    click(vcx, "tarpack-build");
    built(vcx, &host);
    click(vcx, "tarpack-shortcuts");
    assert!(has(vcx, "tarpack-shortcuts-popover"));
    assert!(!host.model.read_with(vcx, |m, cx| m
        .shortcut(crate::tools::tarpack::model::Shortcut::Escape, cx)));
    press(vcx, "escape");
    assert!(!has(vcx, "tarpack-shortcuts-popover"), "Escape closed it");
    assert!(has(vcx, "tarpack-build-result"), "and only it");
    press(vcx, "escape");
    assert!(!has(vcx, "tarpack-build-result"), "the next one dismisses");
}

#[gpui::test]
fn help_lists_rows_without_dev_chord(cx: &mut TestAppContext) {
    let (_host, vcx) = open_fixture(cx, TarpackFixture::Loaded);
    let listed = |vcx: &mut VisualTestContext| {
        click(vcx, "tarpack-shortcuts");
        let mut n = 0;
        while has(vcx, format!("tarpack-shortcuts-action-{n}").leak()) {
            n += 1;
        }
        press(vcx, "escape");
        n
    };
    let plain = SHORTCUT_ROWS.iter().filter(|r| !r.dev_only).count();
    assert_eq!(listed(vcx), plain, "Ctrl+E is not listed");
    vcx.update(|_, cx| DevMode::toggle(cx));
    vcx.run_until_parked();
    assert_eq!(listed(vcx), SHORTCUT_ROWS.len(), "listed in developer mode");
    for row in SHORTCUT_ROWS {
        for chord in row.keys {
            assert!(
                !chord.contains(&"Alt") && !chord.contains(&"Shift"),
                "the developer-mode chord is never listed"
            );
        }
    }
}

#[gpui::test]
fn after_result_focus_lands_on_build_bar_first_control(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Loaded);
    host.backend.script_drop(drop_outcome());
    act(vcx, &host, |m, cx| {
        m.dropped(vec!["C:\\drops\\README.md".into()], cx);
    });
    idle(vcx, &host);
    let mut guard = 0;
    while !vcx.update(|window, cx| host.view.read(cx).result_focus.contains_focused(window, cx)) {
        tab(vcx);
        guard += 1;
        assert!(guard < 12, "the result holds a tab stop");
    }
    click(vcx, "tarpack-drop-dismiss");
    assert!(!has(vcx, "tarpack-drop-result"));
    let first = host
        .view
        .read_with(vcx, |v, _| v.after_result_focus.clone());
    assert!(is_focused(vcx, &first), "the bar's first control has focus");
    // It is Choose: Enter asks for the output.
    press(vcx, "enter");
    wait_for_calls(vcx, &host, 1);
    assert!(matches!(
        host.platform.calls()[0],
        PlatformCall::PickSave(_)
    ));
}

#[gpui::test]
fn reload_from_inside_build_result_focuses_reload(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Result);
    click(vcx, "tarpack-build");
    built(vcx, &host);
    let mut guard = 0;
    while !vcx.update(|window, cx| {
        host.view
            .read(cx)
            .build_result_focus
            .contains_focused(window, cx)
    }) {
        tab(vcx);
        guard += 1;
        assert!(guard < 30, "the build result holds a tab stop");
    }
    act(vcx, &host, TarpackModel::reload);
    idle(vcx, &host);
    assert!(!has(vcx, "tarpack-build-result"));
    let reload = host.view.read_with(vcx, |v, _| v.reload_focus.clone());
    assert!(is_focused(vcx, &reload), "Reload has focus");
}

#[gpui::test]
fn create_archive_hints_focus_the_button(cx: &mut TestAppContext) {
    let (host, vcx) = open_fixture(cx, TarpackFixture::Result);
    let create = host.view.read_with(vcx, |v, _| v.create_focus.clone());
    // A click on Create archive: focus is on it while it is disabled for the
    // build, and is on it again when the result shows.
    click(vcx, "tarpack-build");
    built(vcx, &host);
    assert!(is_focused(vcx, &create));
    // Dismiss, with focus on the Dismiss button: focus goes to Create.
    let mut guard = 0;
    while !vcx.update(|window, cx| {
        host.view
            .read(cx)
            .build_result_focus
            .contains_focused(window, cx)
    }) {
        tab(vcx);
        guard += 1;
        assert!(guard < 30, "the build result holds a tab stop");
    }
    assert!(!is_focused(vcx, &create));
    act(vcx, &host, TarpackModel::dismiss_result);
    assert!(is_focused(vcx, &create), "Create archive has focus again");
    // Focus elsewhere stays there when the result is dismissed.
    click(vcx, "tarpack-build");
    built(vcx, &host);
    let reload = host.view.read_with(vcx, |v, _| v.reload_focus.clone());
    vcx.update(|window, cx| window.focus(&reload, cx));
    act(vcx, &host, TarpackModel::dismiss_result);
    assert!(is_focused(vcx, &reload), "focus was not lost, so it stays");

    // After the confirmation closes, Create archive takes focus whatever had it.
    let (host, vcx) = open_fixture(cx, TarpackFixture::Replace);
    let create = host.view.read_with(vcx, |v, _| v.create_focus.clone());
    click(vcx, "tarpack-build");
    built(vcx, &host);
    click(vcx, "dialog-cancel");
    assert!(is_focused(vcx, &create));
}

/// A view that draws one report.
struct ReportHost {
    summary: fm_tarpack::archive::BuildSummary,
    focus: gpui::FocusHandle,
}

impl gpui::Render for ReportHost {
    fn render(
        &mut self,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        use gpui::prelude::*;
        gpui::div().size_full().children(super::report::report(
            &self.summary,
            false,
            |_, _, _| {},
            &self.focus,
            window,
            cx,
        ))
    }
}

fn summary_with(
    left_out: Vec<fm_tarpack::manifest::EntryFailure>,
    errors: Vec<fm_tarpack::manifest::Diagnostic>,
    warnings: Vec<fm_tarpack::manifest::Diagnostic>,
) -> fm_tarpack::archive::BuildSummary {
    fm_tarpack::archive::BuildSummary {
        path: "C:\\out\\gateway.tar.zst".to_string(),
        format: fm_tarpack::format::ArchiveFormat::TarZst,
        entries: 3,
        files: 2,
        dirs: 1,
        bytes: 2048,
        uncompressed_bytes: 4096,
        sha256_hex: "ab".repeat(32),
        extract_command: String::new(),
        normalized_entries: Vec::new(),
        built_ids: vec!["a".to_string()],
        left_out,
        not_loaded: Vec::new(),
        manifest_errors: errors,
        warnings,
        error_count: 1,
    }
}

fn show_report(
    cx: &mut TestAppContext,
    summary: fm_tarpack::archive::BuildSummary,
) -> &mut VisualTestContext {
    let (_root, vcx) = cx.add_window_view(|_, cx| ReportHost {
        summary,
        focus: cx.focus_handle(),
    });
    vcx.simulate_resize(size(px(900.0), px(600.0)));
    vcx.update(|window, cx| window.draw(cx).clear(cx));
    vcx
}

#[gpui::test]
fn report_sections_only_when_non_empty(cx: &mut TestAppContext) {
    use fm_tarpack::manifest::{Diagnostic, EntryFailure, Severity};
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::theme::apply(ThemeMode::Light, cx);
    });
    let diag = |line: u32, message: &str| Diagnostic {
        severity: Severity::Error,
        line,
        col: 1,
        entry_id: None,
        message: message.to_string(),
    };
    let failure = |index: u32, id: Option<&str>| EntryFailure {
        index,
        id: id.map(str::to_string),
        source: Some("a.bin".to_string()),
        line: 4,
        errors: vec![diag(5, "missing `dir`"), diag(6, "bad mode")],
    };
    // Nothing to report: nothing drawn.
    let vcx = show_report(cx, summary_with(vec![], vec![], vec![]));
    assert!(!has(vcx, "tarpack-build-report"));
    // Left out (one with an id, one without) and manifest errors, no warnings.
    let vcx = show_report(
        cx,
        summary_with(
            vec![failure(1, Some("a")), failure(2, None)],
            vec![diag(1, "unknown key")],
            vec![],
        ),
    );
    assert!(has(vcx, "tarpack-build-report"));
    assert!(has(vcx, "tarpack-report-left-out"));
    assert!(has(vcx, "tarpack-report-errors"));
    assert!(!has(vcx, "tarpack-build-warnings-toggle"));
    // Warnings only: the collapsed disclosure alone.
    let vcx = show_report(
        cx,
        summary_with(vec![], vec![], vec![diag(3, "shared source")]),
    );
    assert!(has(vcx, "tarpack-build-report"));
    assert!(has(vcx, "tarpack-build-warnings-toggle"));
    assert!(!has(vcx, "tarpack-report-left-out"));
    assert!(!has(vcx, "tarpack-report-errors"));
}

#[gpui::test]
fn bar_tab_order_follows_the_buttons(cx: &mut TestAppContext) {
    // Choose, Format, Show errors, Create archive.
    let (host, vcx) = open_fixture(cx, TarpackFixture::Errors);
    act(vcx, &host, |m, cx| m.set_errors_expanded(false, cx));
    let create = host.view.read_with(vcx, |v, _| v.create_focus.clone());
    let choose = host
        .view
        .read_with(vcx, |v, _| v.after_result_focus.clone());
    vcx.update(|window, cx| window.focus(&choose, cx));
    tab(vcx);
    assert!(
        !is_focused(vcx, &choose) && !is_focused(vcx, &create),
        "the format select"
    );
    tab(vcx);
    assert!(!is_focused(vcx, &create), "Show errors");
    tab(vcx);
    assert!(is_focused(vcx, &create), "Create archive");
    // Show errors is between: Enter two stops back opens the report.
    vcx.update(|window, cx| window.focus(&choose, cx));
    tab(vcx);
    tab(vcx);
    press(vcx, "enter");
    assert!(has(vcx, "tarpack-errors-report"), "that was Show errors");
}
