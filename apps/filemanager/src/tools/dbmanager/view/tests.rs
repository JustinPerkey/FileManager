//! View tests of the Database Manager's frame and session, in a headless
//! window under `Root`, over a [`FixtureEnv`] (a real core over a temp-dir
//! database) and [`FakePlatform`].

use std::path::PathBuf;
use std::time::{Duration, Instant};

use fm_dbmanager::config::{ConfigDiagnostic, ConfigStatus, DiagnosticSeverity};
use fm_dbmanager::tool::DbmanagerSnapshot;
use gpui::{
    px, size, AppContext as _, Entity, Modifiers, Pixels, TestAppContext, VisualTestContext,
};
use gpui_component::{Root, ThemeMode};

use super::banners::{plan, Dismissed, Kind};
use super::header::config_line_text;
use super::{copy, shortcuts, DbmanagerView};
use crate::platform;
use crate::platform::fake::{FakePlatform, PlatformCall};
use crate::tools::dbmanager::fixtures::{DbmanagerFixture, FixtureEnv};
use crate::tools::dbmanager::model::DbmanagerModel;
use crate::tools::dbmanager::text;

struct Host {
    model: Entity<DbmanagerModel>,
    view: Entity<DbmanagerView>,
    platform: FakePlatform,
    /// Keeps the fixture's temp dir alive.
    _env: FixtureEnv,
}

fn open(cx: &mut TestAppContext, fixture: DbmanagerFixture) -> (Host, &mut VisualTestContext) {
    cx.executor().allow_parking();
    let env = FixtureEnv::new(fixture);
    let fake = FakePlatform::new();
    let installed = fake.clone();
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::theme::apply(ThemeMode::Light, cx);
        platform::set_platform(cx, installed);
    });
    let backend = env.backend();
    let mut parts = None;
    let (_root, vcx) = cx.add_window_view(|window, cx| {
        let model = cx.new(|cx| DbmanagerModel::new(Box::new(backend), cx));
        let view = cx.new(|cx| DbmanagerView::new(model.clone(), cx));
        parts = Some((model, view.clone()));
        Root::new(view, window, cx)
    });
    vcx.simulate_resize(size(px(1100.0), px(800.0)));
    let (model, view) = parts.expect("view created");
    (
        Host {
            model,
            view,
            platform: fake,
            _env: env,
        },
        vcx,
    )
}

/// Runs everything queued until nothing loads, then draws.
fn idle(cx: &mut VisualTestContext, host: &Host) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        cx.run_until_parked();
        if host.model.read_with(cx, |m, _| !m.loading() && !m.busy()) {
            cx.run_until_parked();
            break;
        }
        assert!(Instant::now() < deadline, "never went idle");
        std::thread::sleep(Duration::from_millis(2));
    }
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn started(cx: &mut TestAppContext, fixture: DbmanagerFixture) -> (Host, &mut VisualTestContext) {
    let (host, vcx) = open(cx, fixture);
    idle(vcx, &host);
    (host, vcx)
}

fn has(cx: &mut VisualTestContext, name: &'static str) -> bool {
    cx.debug_bounds(name).is_some()
}

fn bounds(cx: &mut VisualTestContext, name: &'static str) -> gpui::Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("no element {name}"))
}

fn click(cx: &mut VisualTestContext, name: &'static str) {
    let at = bounds(cx, name).center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
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

fn key_down_only(cx: &mut VisualTestContext, key: &str) {
    let keystroke = cx
        .update(|_, cx| gpui::Keystroke::parse_with_os(key, cx.platform_info().os))
        .expect("keystroke");
    cx.simulate_event(gpui::KeyDownEvent {
        keystroke,
        is_held: false,
        prefer_character_input: false,
    });
    cx.run_until_parked();
}

/// Puts focus on the view's root, as a click on its background does.
fn focus_root(cx: &mut VisualTestContext, host: &Host) {
    let view = host.view.clone();
    cx.update(|window, cx| view.read(cx).focus.clone().focus(window, cx));
    cx.run_until_parked();
}

fn snapshot(cx: &mut VisualTestContext, host: &Host) -> DbmanagerSnapshot {
    host.model
        .read_with(cx, |m, _| m.snapshot().cloned())
        .expect("a snapshot")
}

fn picks(host: &Host) -> usize {
    host.platform
        .calls()
        .iter()
        .filter(|c| matches!(c, PlatformCall::Pick(_)))
        .count()
}

fn wait_for_picks(cx: &mut VisualTestContext, host: &Host, n: usize) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while picks(host) < n {
        cx.run_until_parked();
        assert!(Instant::now() < deadline, "the platform was never asked");
        std::thread::sleep(Duration::from_millis(2));
    }
    cx.run_until_parked();
}

fn close(cx: &mut VisualTestContext, host: &Host) {
    host.model.update(cx, DbmanagerModel::close_database);
    idle(cx, host);
}

#[gpui::test]
fn renders_nothing_open_with_recent(cx: &mut TestAppContext) {
    let (host, vcx) = started(cx, DbmanagerFixture::Loaded);
    close(vcx, &host);
    assert!(!snapshot(vcx, &host).open);
    assert!(has(vcx, "db-empty"));
    assert!(has(vcx, "db-empty-open"));
    assert!(has(vcx, "db-empty-recent-0"), "the recent database");
    assert!(has(vcx, "db-recent"), "the Recent menu");
    assert!(has(vcx, "db-open"));
    assert!(has(vcx, "db-shortcuts"));
    assert!(has(vcx, "db-config-line"));
    assert!(!has(vcx, "db-reload"));
    assert!(!has(vcx, "db-close"));
    assert!(!has(vcx, "db-workspace"));
}

#[gpui::test]
fn renders_open_database_header(cx: &mut TestAppContext) {
    let (host, vcx) = started(cx, DbmanagerFixture::Loaded);
    assert!(snapshot(vcx, &host).open);
    for name in [
        "db-header",
        "db-open",
        "db-recent",
        "db-reload",
        "db-close",
        "db-shortcuts",
        "db-config-line",
        "db-workspace",
    ] {
        assert!(has(vcx, name), "{name}");
    }
    assert!(!has(vcx, "db-empty"));
    assert!(!has(vcx, "db-config-problems"));
    assert!(!has(vcx, "db-error"));
    // Close returns to the empty state.
    click(vcx, "db-close");
    idle(vcx, &host);
    assert!(has(vcx, "db-empty"));
}

#[gpui::test]
fn config_line_per_status(cx: &mut TestAppContext) {
    for (fixture, status) in [
        (DbmanagerFixture::Loaded, ConfigStatus::Loaded),
        (DbmanagerFixture::ConfigMissing, ConfigStatus::Missing),
        (DbmanagerFixture::ConfigFailClosed, ConfigStatus::FailClosed),
    ] {
        let (host, vcx) = started(cx, fixture);
        let s = snapshot(vcx, &host);
        assert_eq!(s.config.status, status);
        assert!(has(vcx, "db-config-line"), "{status:?}");
        let line = config_line_text(&s);
        if status == ConfigStatus::Missing {
            assert_eq!(line, copy::CONFIG_MISSING_LINE);
        } else {
            let path = s.config.display.clone().expect("a config path");
            assert_eq!(line, format!("Config: {path}"));
        }
        assert_eq!(
            has(vcx, "db-banner-no-config"),
            status == ConfigStatus::Missing
        );
        assert_eq!(
            has(vcx, "db-banner-config-closed"),
            status == ConfigStatus::FailClosed
        );
    }
}

fn full_snapshot(cx: &mut TestAppContext) -> DbmanagerSnapshot {
    let (host, vcx) = started(cx, DbmanagerFixture::ConfigMissing);
    let mut s = snapshot(vcx, &host);
    s.config_fail_closed = Some("it is broken".into());
    s.config_changed = true;
    s.read_only_reason = Some("the file is read-only".into());
    s.state_warning = Some("A warning".into());
    s
}

#[gpui::test]
fn banners_in_fixed_order(cx: &mut TestAppContext) {
    let s = full_snapshot(cx);
    assert_eq!(
        plan(true, Some(&s), Dismissed::default()),
        vec![
            Kind::Error,
            Kind::ConfigFailedClosed,
            Kind::NoConfig,
            Kind::ConfigChanged,
            Kind::ReadOnlyDatabase,
            Kind::StateWarning,
        ]
    );
    // Dismissals remove only their own banner.
    let dismissed = Dismissed {
        no_config: true,
        state_warning: Some("A warning"),
    };
    assert_eq!(
        plan(false, Some(&s), dismissed),
        vec![
            Kind::ConfigFailedClosed,
            Kind::ConfigChanged,
            Kind::ReadOnlyDatabase
        ]
    );
    // A different warning is a new banner.
    let other = Dismissed {
        no_config: true,
        state_warning: Some("Another"),
    };
    assert!(plan(false, Some(&s), other).contains(&Kind::StateWarning));
    // Rendered: a command error sits above the fail-closed banner.
    let (host, vcx) = started(cx, DbmanagerFixture::ConfigFailClosed);
    host.model.update(vcx, |m, cx| {
        m.open_path(PathBuf::from("/nowhere/at/all.db"), cx);
    });
    idle(vcx, &host);
    let error = bounds(vcx, "db-error");
    let closed = bounds(vcx, "db-banner-config-closed");
    assert!(error.bottom() <= closed.top(), "{error:?} {closed:?}");
}

#[gpui::test]
fn no_config_banner_dismisses_for_the_run(cx: &mut TestAppContext) {
    let (host, vcx) = started(cx, DbmanagerFixture::ConfigMissing);
    assert!(has(vcx, "db-banner-no-config"));
    click(vcx, "banner-dismiss");
    idle(vcx, &host);
    assert!(!has(vcx, "db-banner-no-config"));
    // A reload keeps it gone.
    host.model.update(vcx, DbmanagerModel::reload);
    idle(vcx, &host);
    assert!(!has(vcx, "db-banner-no-config"));
    assert!(has(vcx, "db-config-line"), "the line stays");
}

#[gpui::test]
fn config_problems_expand_with_f8(cx: &mut TestAppContext) {
    let (host, vcx) = started(cx, DbmanagerFixture::Diagnostics);
    let s = snapshot(vcx, &host);
    assert_eq!((s.error_count, s.warning_count), (1, 1));
    assert!(has(vcx, "db-config-problems"));
    assert!(!has(vcx, "db-config-problem-0"), "closed at first");
    focus_root(vcx, &host);
    press(vcx, "f8");
    vcx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(has(vcx, "db-config-problem-0"));
    assert!(has(vcx, "db-config-problem-1"));
    let focus = host.view.read_with(vcx, |v, _| v.problems_focus.clone());
    assert!(vcx.update(|window, _| focus.is_focused(window)));
    // The toggle closes it again.
    click(vcx, "db-config-problems-toggle");
    assert!(!has(vcx, "db-config-problem-0"));
    // A config without problems has no panel, and F8 does nothing.
    let (host, vcx) = started(cx, DbmanagerFixture::Loaded);
    focus_root(vcx, &host);
    press(vcx, "f8");
    assert!(!has(vcx, "db-config-problems"));
}

#[test]
fn problems_copy() {
    assert_eq!(
        copy::problems_summary(2, 1),
        "Config: 2 errors, 1 warning. Entries with errors are read-only."
    );
    assert_eq!(copy::problems_summary(0, 2), "Config: 2 warnings.");
    let d = |line, col, table: Option<&str>, column: Option<&str>| ConfigDiagnostic {
        severity: DiagnosticSeverity::Error,
        message: String::new(),
        line,
        col,
        table: table.map(Into::into),
        column: column.map(Into::into),
    };
    assert_eq!(
        copy::position(&d(Some(12), Some(3), None, None)),
        "Line 12, column 3"
    );
    assert_eq!(
        copy::position(&d(None, None, Some("book"), Some("isbn"))),
        "Table book, field isbn"
    );
    assert_eq!(copy::position(&d(None, None, None, None)), "");
}

#[gpui::test]
fn open_and_recent_call_model(cx: &mut TestAppContext) {
    let (host, vcx) = started(cx, DbmanagerFixture::Loaded);
    click(vcx, "db-open");
    wait_for_picks(vcx, &host, 1);
    // Ctrl+O from inside the view.
    focus_root(vcx, &host);
    press(vcx, "ctrl-o");
    wait_for_picks(vcx, &host, 2);
    // A recent database, by the menu.
    close(vcx, &host);
    assert!(!snapshot(vcx, &host).open);
    click(vcx, "db-recent");
    vcx.update(|window, cx| window.draw(cx).clear(cx));
    click(vcx, "db-recent-item-0");
    idle(vcx, &host);
    assert!(snapshot(vcx, &host).open, "the recent database opened");
    // And by the empty state's button.
    close(vcx, &host);
    click(vcx, "db-empty-recent-0");
    idle(vcx, &host);
    assert!(snapshot(vcx, &host).open);
}

#[gpui::test]
fn held_key_does_not_repeat(cx: &mut TestAppContext) {
    let (host, vcx) = started(cx, DbmanagerFixture::Loaded);
    focus_root(vcx, &host);
    key_down_only(vcx, "ctrl-o");
    key_down_only(vcx, "ctrl-o");
    key_down_only(vcx, "ctrl-o");
    wait_for_picks(vcx, &host, 1);
    assert_eq!(picks(&host), 1);
    // The key comes up; the next press counts.
    let keystroke = vcx
        .update(|_, cx| gpui::Keystroke::parse_with_os("ctrl-o", cx.platform_info().os))
        .expect("keystroke");
    vcx.simulate_event(gpui::KeyUpEvent { keystroke });
    press(vcx, "ctrl-o");
    wait_for_picks(vcx, &host, 2);
}

#[gpui::test]
fn reload_shortcut_needs_database(cx: &mut TestAppContext) {
    let (host, vcx) = started(cx, DbmanagerFixture::Loaded);
    let before = host
        .model
        .read_with(vcx, |m, _| m.announcement_generation());
    focus_root(vcx, &host);
    press(vcx, "f5");
    idle(vcx, &host);
    let after = host
        .model
        .read_with(vcx, |m, _| m.announcement_generation());
    assert!(after > before, "F5 reloads");
    press(vcx, "ctrl-r");
    idle(vcx, &host);
    let again = host
        .model
        .read_with(vcx, |m, _| m.announcement_generation());
    assert!(again > after, "Ctrl+R reloads");
    close(vcx, &host);
    let closed = host
        .model
        .read_with(vcx, |m, _| m.announcement_generation());
    press(vcx, "f5");
    press(vcx, "ctrl-r");
    idle(vcx, &host);
    let still = host
        .model
        .read_with(vcx, |m, _| m.announcement_generation());
    assert_eq!(still, closed, "nothing open: no reload");
}

#[gpui::test]
fn failed_restore_shows_empty_state_with_error(cx: &mut TestAppContext) {
    let (host, vcx) = started(cx, DbmanagerFixture::StateWarning);
    let s = snapshot(vcx, &host);
    assert!(!s.open);
    let slot = s
        .database
        .as_ref()
        .expect("the slot of the failed database");
    assert!(slot.error.is_some(), "the restore failed");
    assert!(has(vcx, "db-empty"));
    assert!(has(vcx, "db-open-failed"));
    assert!(!has(vcx, "db-reload"));
    assert!(!has(vcx, "db-workspace"));
    assert!(has(vcx, "db-banner-state-warning"));
}

#[test]
fn shortcut_rows_are_the_sessions() {
    let rows = shortcuts::rows();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[1].keys, vec![vec!["F5"], vec!["Ctrl", "R"]]);
    assert_eq!(rows[2].when.as_ref(), "The config has problems");
    assert_eq!(text::OPEN, "Open database\u{2026}");
}
