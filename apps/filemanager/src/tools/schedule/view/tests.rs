//! View tests of the Schedule Creator screen: wiring to the model, not
//! pixels. Each runs in a headless window under `Root` over a fixture backend
//! and the fake platform.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use fm_schedule::session::{ApplySummary, ScheduleError, ScheduleSession};
use gpui::{px, size, AppContext as _, Entity, Modifiers, TestAppContext, VisualTestContext};
use gpui_component::{Root, ThemeMode};

use super::{confirm_spec, ScheduleView};
use crate::jobs::{WorkerGate, WorkerGateHandle};
use crate::platform::fake::{FakePlatform, PlatformCall};
use crate::platform::{self, PickError};
use crate::tools::schedule::backend::ScheduleBackend;
use crate::tools::schedule::fixtures::{FixtureSchedule, TEXT, XML};
use crate::tools::schedule::model::{ScheduleModel, Slot};
use crate::tools::schedule::text;

struct Screen {
    model: Entity<ScheduleModel>,
    view: Entity<ScheduleView>,
    fake: FakePlatform,
}

fn open(
    cx: &mut TestAppContext,
    backend: impl ScheduleBackend,
) -> (Screen, &mut VisualTestContext) {
    open_with(cx, backend, false).0
}

/// `gated`: the worker gate is closed, so the model is still loading; the
/// handle opens it.
fn open_with(
    cx: &mut TestAppContext,
    backend: impl ScheduleBackend,
    gated: bool,
) -> ((Screen, &mut VisualTestContext), Option<WorkerGateHandle>) {
    cx.executor().allow_parking();
    let fake = FakePlatform::new();
    let installed = fake.clone();
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::theme::apply(ThemeMode::Light, cx);
        platform::set_platform(cx, installed);
    });
    let handle = gated.then(|| cx.update(WorkerGate::install_closed));
    let made: std::rc::Rc<std::cell::RefCell<Option<_>>> = std::rc::Rc::default();
    let out = made.clone();
    let (_root, vcx) = cx.add_window_view(move |window, cx| {
        let model = cx.new(|cx| ScheduleModel::new(Box::new(backend), cx));
        let view = cx.new(|cx| ScheduleView::new(model.clone(), window, cx));
        *out.borrow_mut() = Some((model, view.clone()));
        Root::new(view, window, cx)
    });
    vcx.simulate_resize(size(px(1000.0), px(800.0)));
    // Focus-out and blur events only fire in an active window.
    vcx.update(|window, _| window.activate_window());
    vcx.run_until_parked();
    let (model, view) = made.borrow_mut().take().expect("view created");
    let screen = Screen { model, view, fake };
    if !gated {
        idle(vcx);
    }
    ((screen, vcx), handle)
}

/// Runs the executor until the worker threads have answered.
fn idle(cx: &mut VisualTestContext) {
    for _ in 0..50 {
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
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

fn has(cx: &mut VisualTestContext, name: &'static str) -> bool {
    cx.debug_bounds(name).is_some()
}

fn click(cx: &mut VisualTestContext, name: &'static str) {
    let at = cx
        .debug_bounds(name)
        .unwrap_or_else(|| panic!("no element {name}"))
        .center();
    cx.simulate_click(at, Modifiers::none());
    idle(cx);
}

fn field(s: &Screen, cx: &mut VisualTestContext, slot: Slot) -> String {
    let input = s
        .view
        .read_with(cx, |v, _| v.fields[super::index(slot)].input.clone());
    input.read_with(cx, |i, _| i.value().to_string())
}

fn focus_field(s: &Screen, cx: &mut VisualTestContext, slot: Slot) {
    let input = s
        .view
        .read_with(cx, |v, _| v.fields[super::index(slot)].input.clone());
    cx.update(|window, cx| input.update(cx, |i, cx| i.focus(window, cx)));
    cx.run_until_parked();
}

fn set_draft(s: &Screen, cx: &mut VisualTestContext, slot: Slot, value: &str) {
    let input = s
        .view
        .read_with(cx, |v, _| v.fields[super::index(slot)].input.clone());
    cx.update(|window, cx| input.update(cx, |i, cx| i.set_value(value.to_string(), window, cx)));
}

fn path(s: &Screen, cx: &mut VisualTestContext, slot: Slot) -> Option<String> {
    s.model
        .read_with(cx, |m, _| m.file_path(slot).map(str::to_string))
}

fn focused(s: &Screen, cx: &mut VisualTestContext, outcome: bool) -> bool {
    let handle = s.view.read_with(cx, |v, _| {
        if outcome {
            v.outcome.clone()
        } else {
            v.add.clone()
        }
    });
    cx.update(|window, _| handle.is_focused(window))
}

fn summary() -> ApplySummary {
    ApplySummary {
        xml_path: XML.to_string(),
        backup_path: "C:\\Config\\schedule.xml.bak".to_string(),
        added: 3,
    }
}

fn ready_ok() -> FixtureSchedule {
    FixtureSchedule::ready().with_outcome(Ok(summary()))
}

const ALL_TAGS: [&str; 5] = [
    "schedule-text-input",
    "schedule-text-browse",
    "schedule-xml-input",
    "schedule-xml-browse",
    "schedule-apply",
];

fn assert_screen(cx: &mut VisualTestContext, what: &str) {
    for name in &ALL_TAGS {
        assert!(has(cx, name), "{what}: no {name}");
    }
}

#[gpui::test]
fn renders_each_fixture_state(cx: &mut TestAppContext) {
    let (s, cx) = open(cx, FixtureSchedule::empty());
    assert_screen(cx, "empty");
    assert!(!has(cx, "schedule-error") && !has(cx, "schedule-result"));
    assert!(has(cx, "schedule-reason"));
    assert_eq!(field(&s, cx, Slot::Text), "");

    let (s, cx) = open(cx, FixtureSchedule::ready());
    assert_screen(cx, "ready");
    assert!(!has(cx, "schedule-reason"));
    assert!(!has(cx, "schedule-text-error"));
    assert_eq!(field(&s, cx, Slot::Text), TEXT);
    assert_eq!(field(&s, cx, Slot::Xml), XML);

    let (s, cx) = open(cx, FixtureSchedule::bad());
    assert_screen(cx, "bad");
    assert!(has(cx, "schedule-text-error"), "the slot's error line");
    assert!(!has(cx, "schedule-xml-error"));
    assert!(has(cx, "schedule-reason"));
    assert_eq!(
        field(&s, cx, Slot::Text),
        "schedules\\week-44.txt",
        "the typed path stays"
    );
}

#[gpui::test]
fn enter_and_blur_commit_typed_paths(cx: &mut TestAppContext) {
    let (s, cx) = open(cx, FixtureSchedule::empty());
    focus_field(&s, cx, Slot::Text);
    cx.simulate_input("C:\\Typed\\a.txt");
    assert_eq!(
        path(&s, cx, Slot::Text),
        None,
        "nothing is sent while typing"
    );
    press(cx, "enter");
    idle(cx);
    assert_eq!(
        path(&s, cx, Slot::Text).as_deref(),
        Some("C:\\Typed\\a.txt")
    );

    focus_field(&s, cx, Slot::Xml);
    cx.simulate_input("C:\\Typed\\b.xml");
    assert_eq!(path(&s, cx, Slot::Xml), None);
    // Leaving the field commits.
    press(cx, "tab");
    idle(cx);
    assert_eq!(path(&s, cx, Slot::Xml).as_deref(), Some("C:\\Typed\\b.xml"));
    assert!(s.model.read_with(cx, |m, _| m.can_apply()));
}

#[gpui::test]
fn escape_reverts_typing(cx: &mut TestAppContext) {
    let (s, cx) = open(cx, FixtureSchedule::ready());
    focus_field(&s, cx, Slot::Xml);
    set_draft(&s, cx, Slot::Xml, "C:\\Other.xml");
    press(cx, "escape");
    assert_eq!(field(&s, cx, Slot::Xml), XML, "Escape drops the draft");
    assert_eq!(path(&s, cx, Slot::Xml).as_deref(), Some(XML));
    assert_eq!(path(&s, cx, Slot::Text).as_deref(), Some(TEXT));
}

#[gpui::test]
fn browse_buttons_pick_their_slot(cx: &mut TestAppContext) {
    let (s, cx) = open(cx, FixtureSchedule::empty());
    s.fake
        .answer_pick(Ok(Some(PathBuf::from("C:\\Picked\\s.txt"))));
    click(cx, "schedule-text-browse");
    s.fake
        .answer_pick(Ok(Some(PathBuf::from("C:\\Picked\\s.xml"))));
    click(cx, "schedule-xml-browse");
    let picks = s
        .fake
        .calls()
        .into_iter()
        .filter(|c| matches!(c, PlatformCall::Pick(_)))
        .count();
    assert_eq!(picks, 2);
    assert_eq!(
        path(&s, cx, Slot::Text).as_deref(),
        Some("C:\\Picked\\s.txt")
    );
    assert_eq!(
        path(&s, cx, Slot::Xml).as_deref(),
        Some("C:\\Picked\\s.xml")
    );
    assert_eq!(field(&s, cx, Slot::Xml), "C:\\Picked\\s.xml");

    // A failing dialog is the error banner, which dismisses.
    s.fake.answer_pick(Err(PickError::Failed {
        detail: "no shell".into(),
    }));
    click(cx, "schedule-text-browse");
    assert!(has(cx, "schedule-error"));
    click(cx, "banner-dismiss");
    assert!(!has(cx, "schedule-error"));
}

#[gpui::test]
fn apply_opens_confirmation_and_update_confirms(cx: &mut TestAppContext) {
    let (s, cx) = open(cx, ready_ok());
    click(cx, "schedule-apply");
    assert!(s.model.read_with(cx, |m, _| m.pending().is_some()));
    assert!(has(cx, "dialog-cancel") && has(cx, "dialog-confirm"));
    click(cx, "dialog-confirm");
    assert!(!has(cx, "dialog-confirm"), "closed");
    assert!(s.model.read_with(cx, |m, _| m.result().is_some()));
    assert!(has(cx, "schedule-result"));
}

#[gpui::test]
fn cancel_applies_nothing(cx: &mut TestAppContext) {
    let (s, cx) = open(cx, ready_ok());
    click(cx, "schedule-apply");
    // Focus starts on Cancel: Enter cancels.
    press(cx, "enter");
    assert!(s.model.read_with(cx, |m, _| m.pending().is_none()));
    assert!(!has(cx, "dialog-cancel"));
    assert!(s.model.read_with(cx, |m, _| m.result().is_none()));

    click(cx, "schedule-apply");
    press(cx, "escape");
    assert!(!has(cx, "dialog-cancel"));
    assert!(s.model.read_with(cx, |m, _| m.result().is_none()));
    assert!(!has(cx, "schedule-result"));
}

#[gpui::test]
fn result_banner_with_backup_line_dismisses(cx: &mut TestAppContext) {
    let (s, cx) = open(cx, ready_ok());
    click(cx, "schedule-apply");
    click(cx, "dialog-confirm");
    assert!(has(cx, "schedule-result"));
    assert!(!has(cx, "schedule-error"));
    let result = s.model.read_with(cx, |m, _| m.result().cloned());
    assert_eq!(
        result.map(|r| r.backup_path).as_deref(),
        Some("C:\\Config\\schedule.xml.bak")
    );
    click(cx, "banner-dismiss");
    assert!(s.model.read_with(cx, |m, _| m.result().is_none()));
    assert!(!has(cx, "schedule-result"));
}

#[gpui::test]
fn duplicate_warning_shows_in_confirmation_only(cx: &mut TestAppContext) {
    let (s, cx) = open(cx, ready_ok());
    let body = |s: &Screen, cx: &mut VisualTestContext| {
        s.model
            .read_with(cx, |m, _| confirm_spec(m).map(|c| c.body.to_string()))
    };
    click(cx, "schedule-apply");
    let first = body(&s, cx).expect("confirming");
    assert!(!first.contains(text::DUPLICATE_HEADLINE), "first time");
    click(cx, "dialog-confirm");
    // Right after the apply the pair counts as applied, but beside the
    // result there is no duplicate banner.
    assert!(s.model.read_with(cx, |m, _| m.already_applied()));
    assert!(has(cx, "schedule-result"));
    assert!(body(&s, cx).is_none());

    click(cx, "schedule-apply");
    let again = body(&s, cx).expect("confirming again");
    assert!(again.contains(text::DUPLICATE_HEADLINE));
    assert!(again.contains(text::DUPLICATE_REST));
    assert!(again.contains(TEXT) && again.contains(XML));
}

#[gpui::test]
fn apply_disabled_with_reason(cx: &mut TestAppContext) {
    let (s, cx) = open(cx, FixtureSchedule::empty());
    click(cx, "schedule-apply");
    assert!(s.model.read_with(cx, |m, _| m.pending().is_none()));
    assert_eq!(
        s.model.read_with(cx, |m, _| m.apply_reason()),
        Some(text::REASON_NO_TEXT)
    );

    let (s, cx) = open(cx, FixtureSchedule::bad());
    click(cx, "schedule-apply");
    assert!(s.model.read_with(cx, |m, _| m.pending().is_none()));
    assert_eq!(
        s.model.read_with(cx, |m, _| m.apply_reason()),
        Some(text::REASON_FIX_PATHS)
    );
}

#[gpui::test]
fn focus_follows_hint_after_apply(cx: &mut TestAppContext) {
    let (s, cx) = open(cx, ready_ok());
    click(cx, "schedule-apply");
    click(cx, "dialog-confirm");
    assert!(focused(&s, cx, true), "the result banner has focus");

    // The default fixture ends not implemented: the error banner has focus.
    let (s, cx) = open(cx, FixtureSchedule::ready());
    click(cx, "schedule-apply");
    click(cx, "dialog-confirm");
    assert!(has(cx, "schedule-error"));
    assert!(!has(cx, "schedule-result"));
    assert!(focused(&s, cx, true), "the error banner has focus");
}

/// Panics on its first `session` call, so the first load fails.
struct FailsFirstLoad {
    inner: FixtureSchedule,
    failed: Arc<AtomicBool>,
}

impl ScheduleBackend for FailsFirstLoad {
    fn session(&mut self) -> ScheduleSession {
        assert!(self.failed.swap(true, Ordering::SeqCst), "first load fails");
        self.inner.session()
    }
    fn set_text(&mut self, path: Option<PathBuf>) -> ScheduleSession {
        self.inner.set_text(path)
    }
    fn set_xml(&mut self, path: Option<PathBuf>) -> ScheduleSession {
        self.inner.set_xml(path)
    }
    fn set_dropped(&mut self, paths: Vec<PathBuf>) -> Result<ScheduleSession, ScheduleError> {
        self.inner.set_dropped(paths)
    }
    fn apply(&mut self, text: &str, xml: &str) -> Result<ApplySummary, ScheduleError> {
        self.inner.apply(text, xml)
    }
}

#[gpui::test]
fn retry_after_load_failure(cx: &mut TestAppContext) {
    let backend = FailsFirstLoad {
        inner: FixtureSchedule::ready(),
        failed: Arc::default(),
    };
    let (s, cx) = open(cx, backend);
    assert!(s.model.read_with(cx, |m, _| m.load_failed()));
    assert!(has(cx, "schedule-error"));
    assert!(has(cx, "schedule-retry"));
    assert!(!has(cx, "schedule-text-input"), "nothing else renders");
    click(cx, "schedule-retry");
    assert!(!has(cx, "schedule-retry"));
    assert!(has(cx, "schedule-text-input"));
    assert_eq!(field(&s, cx, Slot::Text), TEXT);
}

#[gpui::test]
fn loading_renders_header_only(cx: &mut TestAppContext) {
    let body = [
        "schedule-text-input",
        "schedule-xml-input",
        "schedule-apply",
    ];
    let ((s, cx), gate) = open_with(cx, FixtureSchedule::ready(), true);
    let gate = gate.expect("gate installed");
    cx.run_until_parked();
    assert!(
        s.model.read_with(cx, |m, _| m.loading()),
        "gate closed: loading"
    );
    assert!(has(cx, "schedule-header"), "header while loading");
    for name in body {
        assert!(!has(cx, name), "{name} drawn while loading");
    }

    gate.open();
    idle(cx);
    assert!(!s.model.read_with(cx, |m, _| m.loading()), "loaded");
    assert!(has(cx, "schedule-header"));
    for name in body {
        assert!(has(cx, name), "{name} missing once loaded");
    }
}
