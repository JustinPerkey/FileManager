//! View tests of the Pipeline screen: wiring to the model, not pixels. Each
//! runs in a headless window under `Root` over a fixture backend and the fake
//! platform.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use fm_pipeline::session::{Finished, PipelineError, PipelineSession, StepId};
use gpui::{px, size, AppContext as _, Entity, Modifiers, TestAppContext, VisualTestContext};
use gpui_component::{Root, ThemeMode};

use super::PipelineView;
use crate::jobs::{WorkerGate, WorkerGateHandle};
use crate::platform::fake::{FakePlatform, PlatformCall};
use crate::platform::{self, PickError};
use crate::tools::pipeline::backend::{PipelineBackend, StepJob};
use crate::tools::pipeline::fixtures::{Behavior, FixturePipeline, StepGate};
use crate::tools::pipeline::model::{Pending, PipelineModel, StepStatus};
use crate::tools::pipeline::text;

struct Screen {
    model: Entity<PipelineModel>,
    view: Entity<PipelineView>,
    fake: FakePlatform,
}

fn open(
    cx: &mut TestAppContext,
    backend: impl PipelineBackend,
) -> (Screen, &mut VisualTestContext) {
    open_with(cx, backend, false).0
}

/// `gated`: the worker gate is closed, so the model is still loading; the
/// handle opens it.
fn open_with(
    cx: &mut TestAppContext,
    backend: impl PipelineBackend,
    gated: bool,
) -> ((Screen, &mut VisualTestContext), Option<WorkerGateHandle>) {
    // Step jobs run on real threads.
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
        let model = cx.new(|cx| PipelineModel::new(Box::new(backend), cx));
        let view = cx.new(|cx| PipelineView::new(model.clone(), window, cx));
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
        idle(vcx, &screen);
    }
    ((screen, vcx), handle)
}

/// Runs until nothing loads and no step runs.
fn idle(cx: &mut VisualTestContext, s: &Screen) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        cx.run_until_parked();
        if s.model.read_with(cx, |m, _| !m.busy() && !m.loading()) {
            cx.run_until_parked();
            return;
        }
        assert!(Instant::now() < deadline, "never idle");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Waits until `done` holds, running the executor.
fn until(cx: &mut VisualTestContext, s: &Screen, done: impl Fn(&PipelineModel) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        cx.run_until_parked();
        if s.model.read_with(cx, |m, _| done(m)) {
            return;
        }
        assert!(Instant::now() < deadline, "condition never held");
        std::thread::sleep(Duration::from_millis(2));
    }
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
    cx.run_until_parked();
}

fn field(s: &Screen, cx: &mut VisualTestContext) -> String {
    let input = s.view.read_with(cx, |v, _| v.input.clone());
    input.read_with(cx, |i, _| i.value().to_string())
}

fn focus_field(s: &Screen, cx: &mut VisualTestContext) {
    let input = s.view.read_with(cx, |v, _| v.input.clone());
    cx.update(|window, cx| input.update(cx, |i, cx| i.focus(window, cx)));
    cx.run_until_parked();
}

fn folder(s: &Screen, cx: &mut VisualTestContext) -> Option<String> {
    s.model.read_with(cx, |m, _| {
        m.session()
            .and_then(|x| x.directory.as_ref())
            .map(|d| d.path.clone())
    })
}

fn status(s: &Screen, cx: &mut VisualTestContext, id: StepId) -> StepStatus {
    s.model.read_with(cx, |m, _| m.step_status(id))
}

fn row_focused(s: &Screen, cx: &mut VisualTestContext, i: usize) -> bool {
    let handle = s.view.read_with(cx, |v, _| v.rows[i].clone());
    cx.update(|window, _| handle.is_focused(window))
}

fn next_focused(s: &Screen, cx: &mut VisualTestContext) -> bool {
    let handle = s.view.read_with(cx, |v, _| v.run_next.clone());
    cx.update(|window, _| handle.is_focused(window))
}

const ALL_TAGS: [&str; 12] = [
    "pipeline-folder-input",
    "pipeline-browse",
    "pipeline-run-next",
    "pipeline-run-all",
    "pipeline-step-1",
    "pipeline-step-2",
    "pipeline-step-3",
    "pipeline-step-4",
    "pipeline-run-1",
    "pipeline-run-2",
    "pipeline-run-3",
    "pipeline-run-4",
];

fn assert_screen(cx: &mut VisualTestContext, what: &str) {
    for name in &ALL_TAGS {
        assert!(has(cx, name), "{what}: no {name}");
    }
}

#[gpui::test]
fn renders_each_fixture_state(cx: &mut TestAppContext) {
    let (s, cx) = open(cx, FixturePipeline::empty());
    assert_screen(cx, "empty");
    assert!(!has(cx, "pipeline-folder-note"), "no folder, no note");
    assert!(!has(cx, "pipeline-error"));
    assert_eq!(
        s.model.read_with(cx, |m, _| m.run_reason()),
        Some(text::REASON_NO_FOLDER)
    );

    let (s, cx) = open(cx, FixturePipeline::chosen());
    assert_screen(cx, "chosen");
    assert!(has(cx, "pipeline-folder-note"), "the subfolder count");
    assert!(!has(cx, "pipeline-field-error"));
    assert_eq!(field(&s, cx), "C:\\Deliveries\\2026-10");

    let (s, cx) = open(cx, FixturePipeline::bad());
    assert_screen(cx, "bad");
    assert!(has(cx, "pipeline-field-error"), "the folder's error line");
    assert!(!has(cx, "pipeline-folder-note"));
    assert_eq!(
        field(&s, cx),
        "C:\\Deliveries\\2026-13",
        "the typed path stays"
    );

    let (s, cx) = open(cx, FixturePipeline::ran());
    assert_screen(cx, "ran");
    assert_eq!(status(&s, cx, StepId::Verify), StepStatus::NotImplemented);

    let (s, cx) = open(cx, FixturePipeline::demo());
    assert_screen(cx, "demo");
    assert_eq!(status(&s, cx, StepId::Verify), StepStatus::Passed);
    assert_eq!(status(&s, cx, StepId::RunBatch), StepStatus::Problems(2));
    assert_eq!(text::status_label(StepStatus::Problems(2)), "2 problems");
    assert!(has(cx, "pipeline-issues-2"), "the problems list");
    assert!(!has(cx, "pipeline-issues-1"));
}

#[gpui::test]
fn slow_fixture_shows_a_step_running(cx: &mut TestAppContext) {
    let gate = StepGate::new();
    let backend = FixturePipeline::chosen().with_gate(&gate, Duration::from_secs(20));
    let (s, cx) = open(cx, backend);
    click(cx, "pipeline-run-next");
    until(cx, &s, |m| m.running() == Some(StepId::Verify));
    assert_screen(cx, "running");
    assert_eq!(status(&s, cx, StepId::Verify), StepStatus::Running);
    assert!(s.model.read_with(cx, |m, _| m.via_next().is_some()));
    // The folder field and Run next are inert; a second click runs nothing.
    click(cx, "pipeline-run-all");
    assert!(s.model.read_with(cx, |m, _| m.pending().is_none()));
    gate.open();
    idle(cx, &s);
}

#[gpui::test]
fn step_list_shows_lock_lines_and_note(cx: &mut TestAppContext) {
    let (_s, cx) = open(cx, FixturePipeline::chosen());
    assert!(
        !has(cx, "pipeline-lock-1"),
        "the first step is never locked"
    );
    for n in ["pipeline-lock-2", "pipeline-lock-3", "pipeline-lock-4"] {
        assert!(has(cx, n), "{n}");
    }
    assert_eq!(
        text::runs_after(StepId::Verify),
        "Runs after Check required files passes."
    );
    assert!(has(cx, "pipeline-folder-note"));
    // Once the first step passes, the second unlocks.
    let (_s, cx) = open(cx, FixturePipeline::demo());
    assert!(!has(cx, "pipeline-lock-2"));
    assert!(has(cx, "pipeline-lock-3"));
}

#[gpui::test]
fn failed_step_shows_details(cx: &mut TestAppContext) {
    let backend =
        FixturePipeline::chosen().with_behavior(StepId::Verify, Behavior::Fail("disk gone".into()));
    let (s, cx) = open(cx, backend);
    click(cx, "pipeline-run-1");
    idle(cx, &s);
    assert_eq!(status(&s, cx, StepId::Verify), StepStatus::Failed);
    assert!(has(cx, "pipeline-details-1"));
    assert!(!s.view.read_with(cx, |v, _| v.details[0]));
    click(cx, "pipeline-details-1");
    assert!(s.view.read_with(cx, |v, _| v.details[0]));
}

#[gpui::test]
fn enter_commits_typed_folder(cx: &mut TestAppContext) {
    let (s, cx) = open(cx, FixturePipeline::empty());
    focus_field(&s, cx);
    cx.simulate_input("C:\\Typed\\Jobs");
    assert_eq!(folder(&s, cx), None, "nothing is sent while typing");
    press(cx, "enter");
    idle(cx, &s);
    assert_eq!(folder(&s, cx).as_deref(), Some("C:\\Typed\\Jobs"));
    assert!(has(cx, "pipeline-folder-note"));
}

#[gpui::test]
fn blur_commits_and_escape_reverts(cx: &mut TestAppContext) {
    let (s, cx) = open(cx, FixturePipeline::chosen());
    focus_field(&s, cx);
    cx.update(|window, cx| {
        let input = s.view.read(cx).input.clone();
        input.update(cx, |i, cx| i.set_value("C:\\Other", window, cx));
    });
    press(cx, "escape");
    assert_eq!(
        field(&s, cx),
        "C:\\Deliveries\\2026-10",
        "Escape drops the draft"
    );
    assert_eq!(folder(&s, cx).as_deref(), Some("C:\\Deliveries\\2026-10"));

    focus_field(&s, cx);
    cx.simulate_input("2");
    assert_eq!(
        field(&s, cx),
        "C:\\Deliveries\\2026-102",
        "typing is a draft"
    );
    assert_eq!(folder(&s, cx).as_deref(), Some("C:\\Deliveries\\2026-10"));
    // Leaving the field commits.
    press(cx, "tab");
    idle(cx, &s);
    assert_eq!(folder(&s, cx).as_deref(), Some("C:\\Deliveries\\2026-102"));
    assert_eq!(field(&s, cx), "C:\\Deliveries\\2026-102");
}

#[gpui::test]
fn browse_button_calls_platform_pick(cx: &mut TestAppContext) {
    let (s, cx) = open(cx, FixturePipeline::empty());
    s.fake.answer_pick(Ok(Some(PathBuf::from("C:\\Picked"))));
    click(cx, "pipeline-browse");
    idle(cx, &s);
    let picks = s
        .fake
        .calls()
        .into_iter()
        .filter(|c| matches!(c, PlatformCall::Pick(_)))
        .count();
    assert_eq!(picks, 1);
    assert_eq!(folder(&s, cx).as_deref(), Some("C:\\Picked"));
    assert_eq!(field(&s, cx), "C:\\Picked", "the field follows the session");
}

#[gpui::test]
fn run_next_click_and_keyboard_run_step(cx: &mut TestAppContext) {
    let (s, cx) = open(cx, FixturePipeline::chosen().all_pass());
    click(cx, "pipeline-run-next");
    idle(cx, &s);
    assert_eq!(status(&s, cx, StepId::Verify), StepStatus::Passed);
    // Focus came back to Run next; Enter on it asks for the next step, which
    // confirms first.
    assert!(next_focused(&s, cx), "focus returns to Run next");
    press(cx, "enter");
    let pending = s.model.read_with(cx, |m, _| m.pending().map(|(p, _)| *p));
    assert_eq!(
        pending,
        Some(Pending::Step {
            id: StepId::RunBatch,
            via_next: true
        })
    );
    press(cx, "escape");
    // A step's own button runs that step on a click.
    click(cx, "pipeline-run-2");
    assert!(s.model.read_with(cx, |m, _| m.pending().is_some()));
}

#[gpui::test]
fn confirming_step_opens_dialog_and_cancel_runs_nothing(cx: &mut TestAppContext) {
    let (s, cx) = open(cx, FixturePipeline::chosen().all_pass());
    click(cx, "pipeline-run-1");
    idle(cx, &s);
    click(cx, "pipeline-run-2");
    assert!(has(cx, "dialog-cancel"), "the confirmation is open");
    assert!(has(cx, "dialog-confirm"));
    // Focus starts on Cancel: Enter cancels.
    press(cx, "enter");
    assert!(s.model.read_with(cx, |m, _| m.pending().is_none()));
    assert!(!has(cx, "dialog-cancel"), "closed");
    assert_ne!(status(&s, cx, StepId::RunBatch), StepStatus::Passed);

    click(cx, "pipeline-run-2");
    press(cx, "escape");
    assert!(!has(cx, "dialog-cancel"));
    assert_ne!(status(&s, cx, StepId::RunBatch), StepStatus::Passed);

    click(cx, "pipeline-run-2");
    click(cx, "dialog-confirm");
    idle(cx, &s);
    assert_eq!(status(&s, cx, StepId::RunBatch), StepStatus::Passed);
    assert!(!has(cx, "dialog-confirm"));
}

#[gpui::test]
fn run_buttons_disabled_with_reason(cx: &mut TestAppContext) {
    let (s, cx) = open(cx, FixturePipeline::empty());
    click(cx, "pipeline-run-next");
    click(cx, "pipeline-run-all");
    click(cx, "pipeline-run-1");
    assert!(s
        .model
        .read_with(cx, |m, _| m.pending().is_none() && !m.busy()));
    assert_eq!(status(&s, cx, StepId::Verify), StepStatus::NotRun);

    let (s, cx) = open(cx, FixturePipeline::bad());
    click(cx, "pipeline-run-1");
    assert!(s.model.read_with(cx, |m, _| !m.busy()));
    assert_eq!(
        s.model.read_with(cx, |m, _| m.run_reason()),
        Some(text::REASON_BAD_FOLDER)
    );

    // A locked step does nothing even with a folder.
    let (s, cx) = open(cx, FixturePipeline::chosen());
    click(cx, "pipeline-run-3");
    assert!(s
        .model
        .read_with(cx, |m, _| m.pending().is_none() && !m.busy()));
}

#[gpui::test]
fn error_banner_reshows_on_new_generation(cx: &mut TestAppContext) {
    let (s, cx) = open(cx, FixturePipeline::empty());
    assert!(!has(cx, "pipeline-error"));
    let fail = |s: &Screen| {
        s.fake.answer_pick(Err(PickError::Failed {
            detail: "no shell".into(),
        }));
    };
    fail(&s);
    click(cx, "pipeline-browse");
    idle(cx, &s);
    assert!(has(cx, "pipeline-error"));
    let first = s.model.read_with(cx, |m, _| m.error_generation());
    click(cx, "banner-dismiss");
    assert!(!has(cx, "pipeline-error"), "dismissed");

    fail(&s);
    click(cx, "pipeline-browse");
    idle(cx, &s);
    assert!(has(cx, "pipeline-error"), "the same error shows again");
    assert!(s.model.read_with(cx, |m, _| m.error_generation()) > first);
}

/// Panics on its first `session` call, so the first load fails.
struct FailsFirstLoad {
    inner: FixturePipeline,
    failed: Arc<AtomicBool>,
}

impl PipelineBackend for FailsFirstLoad {
    fn session(&mut self) -> PipelineSession {
        assert!(self.failed.swap(true, Ordering::SeqCst), "first load fails");
        self.inner.session()
    }
    fn set_directory(&mut self, path: Option<PathBuf>) -> Result<PipelineSession, PipelineError> {
        self.inner.set_directory(path)
    }
    fn begin_step(
        &mut self,
        step: StepId,
        expected: &str,
    ) -> Result<Box<dyn StepJob>, PipelineError> {
        self.inner.begin_step(step, expected)
    }
    fn finish_step(&mut self, step: StepId, finished: Finished) -> PipelineSession {
        self.inner.finish_step(step, finished)
    }
}

#[gpui::test]
fn retry_after_load_failure(cx: &mut TestAppContext) {
    let backend = FailsFirstLoad {
        inner: FixturePipeline::chosen(),
        failed: Arc::default(),
    };
    let (s, cx) = open(cx, backend);
    assert!(s.model.read_with(cx, |m, _| m.load_failed()));
    assert!(has(cx, "pipeline-error"));
    assert!(has(cx, "pipeline-retry"));
    assert!(!has(cx, "pipeline-folder-input"), "nothing else renders");
    assert!(!has(cx, "pipeline-run-next"));
    click(cx, "pipeline-retry");
    idle(cx, &s);
    assert!(!has(cx, "pipeline-retry"));
    assert!(has(cx, "pipeline-folder-input"));
    assert_eq!(folder(&s, cx).as_deref(), Some("C:\\Deliveries\\2026-10"));
}

#[gpui::test]
fn focus_follows_hint_after_run_next(cx: &mut TestAppContext) {
    let (s, cx) = open(cx, FixturePipeline::chosen().all_pass());
    click(cx, "pipeline-run-next");
    idle(cx, &s);
    assert!(next_focused(&s, cx), "another step can run: Run next");

    // A step's own button ends on that step's row.
    let (s, cx) = open(cx, FixturePipeline::chosen().all_pass());
    click(cx, "pipeline-run-1");
    idle(cx, &s);
    assert!(row_focused(&s, cx, 0), "focus returns to the step it ran");
}

#[gpui::test]
fn focus_moves_to_step_when_it_starts_running(cx: &mut TestAppContext) {
    let gate = StepGate::new();
    let backend = FixturePipeline::chosen().with_gate(&gate, Duration::from_secs(20));
    let (s, cx) = open(cx, backend);
    click(cx, "pipeline-run-1");
    until(cx, &s, |m| m.running() == Some(StepId::Verify));
    assert!(row_focused(&s, cx, 0), "the running step's row has focus");
    assert!(!next_focused(&s, cx));
    gate.open();
    idle(cx, &s);
}

#[gpui::test]
fn loading_renders_header_only(cx: &mut TestAppContext) {
    let body = [
        "pipeline-folder-input",
        "pipeline-browse",
        "pipeline-run-next",
        "pipeline-run-all",
        "pipeline-step-1",
    ];
    let ((s, cx), gate) = open_with(cx, FixturePipeline::chosen(), true);
    let gate = gate.expect("gate installed");
    cx.run_until_parked();
    assert!(
        s.model.read_with(cx, |m, _| m.loading()),
        "gate closed: loading"
    );
    assert!(has(cx, "pipeline-header"), "header while loading");
    crate::app::tests::assert_header_only(cx, "pipeline-page", "pipeline-header");
    for name in body {
        assert!(!has(cx, name), "{name} drawn while loading");
    }

    gate.open();
    idle(cx, &s);
    assert!(!s.model.read_with(cx, |m, _| m.loading()), "loaded");
    assert!(has(cx, "pipeline-header"));
    for name in body {
        assert!(has(cx, name), "{name} missing once loaded");
    }
}
