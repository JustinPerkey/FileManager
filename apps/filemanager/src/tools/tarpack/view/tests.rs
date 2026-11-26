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
        let view = cx.new(|cx| TarpackView::new(model.clone(), cx));
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

    act(vcx, &host, TarpackModel::build);
    let deadline = Instant::now() + Duration::from_secs(20);
    while !host.model.read_with(vcx, |m, _| m.building()) {
        vcx.run_until_parked();
        assert!(Instant::now() < deadline, "the build never started");
        std::thread::sleep(Duration::from_millis(2));
    }
    vcx.run_until_parked();
    let picks = host.platform.calls().len();
    let before = host.backend.calls().len();
    for key in ["down", "down", "end", "enter", "delete"] {
        press(vcx, key);
    }
    assert_eq!(selected_row(vcx, &host), Some(0), "the selection stays");
    assert_eq!(host.platform.calls().len(), picks, "no Browse");
    assert_eq!(host.backend.calls().len(), before, "no Clear");
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
        (a.iter().sum::<f32>() - (1400.0 - 48.0)).abs() < 1.0,
        "{a:?}"
    );
    resize(vcx, 1200.0, 800.0);
    let b = widths(vcx);
    assert!(b[1] < a[1], "{a:?} then {b:?}");
    assert!(
        (b.iter().sum::<f32>() - (1200.0 - 48.0)).abs() < 1.0,
        "{b:?}"
    );
    assert_eq!(b[0], 116.0);
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
