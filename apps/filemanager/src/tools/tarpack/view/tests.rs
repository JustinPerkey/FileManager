//! View tests of the Tar Packager's frame and everything above the entry
//! table, in a headless window under `Root`, over [`FixtureTarpack`] and
//! [`FakePlatform`].

use std::time::{Duration, Instant};

use fm_tarpack::session::{TarpackError, TarpackErrorKind};
use fm_tarpack::sources::{DropOutcome, Unmatched, UnmatchedReason};
use gpui::{
    px, size, AppContext as _, Entity, Modifiers, Pixels, TestAppContext, VisualTestContext, Window,
};
use gpui_component::{Root, ThemeMode};

use super::shortcuts::SHORTCUT_ROWS;
use super::TarpackView;
use crate::app::dev_mode::DevMode;
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
    let (host, vcx) = open(cx, FixtureTarpack::loaded());
    let check = |vcx: &mut VisualTestContext, host: &Host| {
        let shown = host.model.read_with(vcx, |m, _| m.show_skeleton());
        assert_eq!(has(vcx, "tarpack-skeleton"), shown);
        // While loading, nothing else of the view shows.
        if host.model.read_with(vcx, |m, _| m.loading()) {
            assert!(!has(vcx, "tarpack-open"));
            assert!(!has(vcx, "tarpack-shortcuts"));
        }
    };
    vcx.run_until_parked();
    check(vcx, &host);
    vcx.executor().advance_clock(SKELETON_DELAY);
    vcx.run_until_parked();
    check(vcx, &host);
    idle(vcx, &host);
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
