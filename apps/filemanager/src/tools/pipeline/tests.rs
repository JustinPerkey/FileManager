//! The Pipeline model's tests: the copy, the folder, running, and the
//! fixtures. Model tests run against [`FixturePipeline`] and [`FakePlatform`];
//! one runs [`RealPipeline`] over a temporary directory.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use fm_pipeline::session::{Finished, PipelineError, PipelineErrorKind, PipelineSession, StepId};
use gpui::{AppContext as _, Context, Entity, TestAppContext};

use super::backend::{PipelineBackend, RealPipeline, StepJob};
use super::fixtures::{Behavior, FixturePipeline, PipelineFixture, StepGate};
use super::model::{clean_path, FocusHint, Pending, PipelineModel, StepStatus, ViewError};
use super::text::{self, STEP_IDS};
use crate::app::text::PICK_FAILED;
use crate::platform::fake::{FakePlatform, PlatformCall};
use crate::platform::{self, PickError, PickKind};

const DIR: &str = "C:\\Deliveries\\2026-10";

const ALL_KINDS: [PipelineErrorKind; 9] = [
    PipelineErrorKind::NoDirectory,
    PipelineErrorKind::NotAbsolute,
    PipelineErrorKind::NotADirectory,
    PipelineErrorKind::DirectoryUnreadable,
    PipelineErrorKind::DirectoryChanged,
    PipelineErrorKind::StepInProgress,
    PipelineErrorKind::PreviousStepNotPassed,
    PipelineErrorKind::StepNotImplemented,
    PipelineErrorKind::StepFailed,
];

// ---- helpers ----

fn start_boxed(
    cx: &mut TestAppContext,
    backend: Box<dyn PipelineBackend>,
) -> (Entity<PipelineModel>, FakePlatform) {
    // Step jobs run on real threads.
    cx.executor().allow_parking();
    let fake = FakePlatform::new();
    let installed = fake.clone();
    cx.update(|cx| platform::set_platform(cx, installed));
    let model = cx.new(|cx| PipelineModel::new(backend, cx));
    (model, fake)
}

fn start(
    cx: &mut TestAppContext,
    backend: FixturePipeline,
) -> (Entity<PipelineModel>, FakePlatform) {
    start_boxed(cx, Box::new(backend))
}

fn read<R>(
    cx: &mut TestAppContext,
    model: &Entity<PipelineModel>,
    f: impl FnOnce(&PipelineModel) -> R,
) -> R {
    model.read_with(cx, |m, _| f(m))
}

fn act<R>(
    cx: &mut TestAppContext,
    model: &Entity<PipelineModel>,
    f: impl FnOnce(&mut PipelineModel, &mut Context<PipelineModel>) -> R,
) -> R {
    model.update(cx, f)
}

/// Runs everything queued and waits for the real threads: until no step runs
/// and nothing is loading.
fn idle(cx: &mut TestAppContext, model: &Entity<PipelineModel>) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        cx.run_until_parked();
        if read(cx, model, |m| !m.busy() && !m.loading()) {
            cx.run_until_parked();
            return;
        }
        assert!(Instant::now() < deadline, "the model never went idle");
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn hint(cx: &mut TestAppContext, model: &Entity<PipelineModel>) -> Option<FocusHint> {
    act(cx, model, |m, _| m.take_focus_hint())
}

fn status(cx: &mut TestAppContext, model: &Entity<PipelineModel>, id: StepId) -> StepStatus {
    read(cx, model, |m| m.step_status(id))
}

fn folder(cx: &mut TestAppContext, model: &Entity<PipelineModel>) -> Option<String> {
    read(cx, model, |m| {
        m.session()
            .and_then(|s| s.directory.as_ref())
            .map(|d| d.path.clone())
    })
}

/// Requests `id` and confirms when the model asks; waits until idle.
fn run_step(cx: &mut TestAppContext, model: &Entity<PipelineModel>, id: StepId) {
    act(cx, model, |m, cx| m.request_step(id, cx));
    if read(cx, model, |m| m.pending().is_some()) {
        act(cx, model, PipelineModel::confirm);
    }
    idle(cx, model);
}

/// A fixture that panics inside the worker where told to.
struct Flaky {
    inner: FixturePipeline,
    panic_session: bool,
    panic_set_directory: bool,
    panic_finish: bool,
}

impl PipelineBackend for Flaky {
    fn session(&mut self) -> PipelineSession {
        assert!(!std::mem::take(&mut self.panic_session), "session exploded");
        self.inner.session()
    }

    fn set_directory(&mut self, path: Option<PathBuf>) -> Result<PipelineSession, PipelineError> {
        assert!(!self.panic_set_directory, "set_directory exploded");
        self.inner.set_directory(path)
    }

    fn begin_step(
        &mut self,
        step: StepId,
        expected_directory: &str,
    ) -> Result<Box<dyn StepJob>, PipelineError> {
        self.inner.begin_step(step, expected_directory)
    }

    fn finish_step(&mut self, step: StepId, finished: Finished) -> PipelineSession {
        let session = self.inner.finish_step(step, finished);
        assert!(!self.panic_finish, "finish_step exploded");
        session
    }
}

// ---- copy ----

#[test]
fn text_covers_every_error_kind_and_step() {
    let mut seen = Vec::new();
    for kind in ALL_KINDS {
        let message = text::error_message(kind);
        assert_ne!(message, "");
        assert!(!seen.contains(&message), "{kind:?} repeats a message");
        seen.push(message);
    }
    for id in STEP_IDS {
        let copy = text::step_copy(id);
        assert!(!copy.title.is_empty() && !copy.description.is_empty());
        assert_eq!(copy.confirm.is_some(), id != StepId::Verify, "{id:?}");
    }
    assert_eq!(text::UNEXPECTED, "Something went wrong. See Details.");
    assert_eq!(text::problems_label(1), "1 problem");
    assert_eq!(text::problems_label(0), "0 problems");
    assert_eq!(text::status_label(StepStatus::Problems(3)), "3 problems");
    assert_eq!(text::status_label(StepStatus::Waiting), "Waiting");
    assert_eq!(
        text::run_next_label(StepId::Verify),
        "Run next: Check required files"
    );
    assert_eq!(
        text::run_next_label(StepId::RunBatch),
        "Run next: Run batch files…"
    );
    assert_eq!(text::subfolders(1), "1 subfolder");
    assert_eq!(text::subfolders(3), "3 subfolders");
    assert_eq!(
        text::confirm_step_body(StepId::RunBatch, DIR).plain(),
        format!("The batch file in each subfolder will be run. Folder: {DIR}")
    );
    assert_eq!(
        text::confirm_all_body(DIR).plain(),
        format!(
            "Every step runs in order over {DIR}, stopping at the first that does not pass. \
             The batch file in each subfolder will be run. \
             The schedule step may update files in each subfolder. \
             An archive will be written for each subfolder."
        )
    );
}

/// The double-quoted literals of `src`, unescaped.
fn literals(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = src.chars();
    while let Some(c) = chars.next() {
        if c != '"' {
            continue;
        }
        let mut lit = String::new();
        while let Some(c) = chars.next() {
            match c {
                '"' => break,
                '\\' => lit.extend(chars.next()),
                c => lit.push(c),
            }
        }
        out.push(lit);
    }
    out
}

/// Every string of the TypeScript copy appears, as is, in `text.rs`, and
/// nothing else does. Goes once the Tauri app is deleted.
#[test]
fn text_matches_ts_sources() {
    let errors = include_str!("../../../../desktop/src/tools/pipeline/errorMessages.ts");
    let steps = include_str!("../../../../desktop/src/tools/pipeline/steps.ts");
    let messages = &errors[errors.find("const messages").expect("messages table")..];
    let messages = &messages[..messages.find("};").expect("end of table")];
    let unexpected_line = errors
        .lines()
        .find(|l| l.contains("\"Unexpected\") return"))
        .expect("the Unexpected line");
    let copy = &steps[steps.find("export const stepCopy").expect("stepCopy")..];

    let mut ts: Vec<String> = literals(messages);
    ts.extend(literals(unexpected_line).pop());
    ts.extend(literals(copy));
    ts.sort();
    ts.dedup();

    let mut ours: Vec<String> = ALL_KINDS
        .iter()
        .map(|k| text::error_message(*k).to_string())
        .collect();
    ours.push(text::UNEXPECTED.to_string());
    for id in STEP_IDS {
        let copy = text::step_copy(id);
        ours.push(copy.title.to_string());
        ours.push(copy.description.to_string());
        ours.extend(copy.confirm.map(str::to_string));
    }
    ours.sort();
    ours.dedup();
    assert_eq!(ours, ts);
}

#[test]
fn clean_path_trims_and_unquotes() {
    assert_eq!(
        clean_path("  \"C:\\a b\\jobs\"  "),
        Some(PathBuf::from("C:\\a b\\jobs"))
    );
    assert_eq!(clean_path("   "), None);
    assert_eq!(clean_path(""), None);
    assert_eq!(clean_path("\"\""), None);
    assert_eq!(clean_path("\" x \""), Some(PathBuf::from("x")));
    assert_eq!(clean_path("\""), Some(PathBuf::from("\"")));
    assert_eq!(clean_path("C:\\jobs"), Some(PathBuf::from("C:\\jobs")));
}

// ---- loading ----

#[gpui::test]
fn loads_session_on_start(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixturePipeline::chosen());
    assert!(read(cx, &model, |m| m.loading() && m.session().is_none()));
    idle(cx, &model);
    assert!(read(cx, &model, |m| !m.loading() && !m.load_failed()));
    assert_eq!(folder(cx, &model).as_deref(), Some(DIR));
    let ids = read(cx, &model, |m| {
        m.session()
            .map(|s| s.steps.iter().map(|v| v.id).collect::<Vec<_>>())
    });
    assert_eq!(ids, Some(STEP_IDS.to_vec()));
}

#[gpui::test]
fn load_failure_shows_retry_and_retry_loads(cx: &mut TestAppContext) {
    let flaky = Flaky {
        inner: FixturePipeline::chosen(),
        panic_session: true,
        panic_set_directory: false,
        panic_finish: false,
    };
    let (model, _) = start_boxed(cx, Box::new(flaky));
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.load_failed()
        && m.session().is_none()
        && !m.loading()));
    let error = read(cx, &model, |m| m.error().cloned());
    assert!(
        matches!(error, Some(ViewError::Unexpected { .. })),
        "{error:?}"
    );
    act(cx, &model, PipelineModel::retry_load);
    assert!(read(cx, &model, |m| !m.load_failed() && m.loading()));
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.session().is_some() && m.error().is_none()));
}

// ---- the folder ----

#[gpui::test]
fn typed_folder_sets_directory(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixturePipeline::empty());
    idle(cx, &model);
    act(cx, &model, |m, cx| {
        m.set_directory_text(" \"C:\\Builds\\gateway\" ", cx);
    });
    idle(cx, &model);
    assert_eq!(folder(cx, &model).as_deref(), Some("C:\\Builds\\gateway"));
    assert!(read(cx, &model, |m| m.can_run() && m.run_reason().is_none()));
    assert!(read(cx, &model, |m| m.announcement().is_none()
        && m.error().is_none()));
    let count = read(cx, &model, |m| {
        m.session()
            .and_then(|s| s.directory.as_ref())
            .map(|d| d.subdirectories)
    });
    assert_eq!(count, Some(12));

    act(cx, &model, |m, cx| m.set_directory_text("   ", cx));
    idle(cx, &model);
    assert_eq!(folder(cx, &model), None);
    assert_eq!(
        read(cx, &model, super::model::PipelineModel::run_reason),
        Some(text::REASON_NO_FOLDER)
    );

    act(cx, &model, |m, cx| m.set_directory_text("C:\\x", cx));
    idle(cx, &model);
    act(cx, &model, PipelineModel::clear_directory);
    idle(cx, &model);
    assert_eq!(folder(cx, &model), None);
}

#[gpui::test]
fn bad_folder_shows_slot_error_and_announces(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixturePipeline::empty());
    idle(cx, &model);
    act(cx, &model, |m, cx| m.set_directory_text("jobs", cx));
    idle(cx, &model);
    let kind = read(cx, &model, |m| {
        m.session()
            .and_then(|s| s.directory.as_ref())
            .and_then(|d| d.error.as_ref())
            .map(|e| e.kind)
    });
    assert_eq!(kind, Some(PipelineErrorKind::NotAbsolute));
    let said = read(cx, &model, |m| m.announcement().map(str::to_string));
    assert_eq!(
        said.as_deref(),
        Some(text::error_message(PipelineErrorKind::NotAbsolute))
    );
    assert!(
        read(cx, &model, |m| m.error().is_none()),
        "a slot error is not a banner"
    );
    assert!(read(cx, &model, |m| !m.can_run()));
    assert_eq!(
        read(cx, &model, super::model::PipelineModel::run_reason),
        Some(text::REASON_BAD_FOLDER)
    );
}

#[gpui::test]
fn only_newest_folder_change_applies(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixturePipeline::empty());
    idle(cx, &model);
    // The older change would announce its error if it were applied.
    act(cx, &model, |m, cx| {
        m.set_directory_text("relative", cx);
        m.set_directory_text("C:\\good", cx);
    });
    idle(cx, &model);
    assert_eq!(folder(cx, &model).as_deref(), Some("C:\\good"));
    assert!(read(cx, &model, |m| m.announcement().is_none() && m.can_run()));
}

#[gpui::test]
fn browse_uses_folder_spec_and_cancel_changes_nothing(cx: &mut TestAppContext) {
    let (model, fake) = start(cx, FixturePipeline::empty());
    idle(cx, &model);
    fake.answer_pick(Ok(Some(PathBuf::from("C:\\Picked"))));
    act(cx, &model, PipelineModel::browse);
    idle(cx, &model);
    assert_eq!(folder(cx, &model).as_deref(), Some("C:\\Picked"));

    // Nothing scripted: the dialog is cancelled.
    act(cx, &model, PipelineModel::browse);
    idle(cx, &model);
    assert_eq!(folder(cx, &model).as_deref(), Some("C:\\Picked"));
    assert!(read(cx, &model, |m| m.error().is_none()));

    let picks: Vec<_> = fake
        .calls()
        .into_iter()
        .map(|c| match c {
            PlatformCall::Pick(spec) => spec,
            other => panic!("unexpected call {other:?}"),
        })
        .collect();
    assert_eq!(picks.len(), 2);
    assert!(picks.iter().all(|s| s.kind == PickKind::Folder));
    assert_eq!(picks[0].start_dir, None);
    assert_eq!(picks[1].start_dir.as_deref(), Some(Path::new("C:\\Picked")));
}

#[gpui::test]
fn browse_error_shows_dialog_failure_copy(cx: &mut TestAppContext) {
    let (model, fake) = start(cx, FixturePipeline::empty());
    idle(cx, &model);
    fake.answer_pick(Err(PickError::Failed {
        detail: "no shell".into(),
    }));
    act(cx, &model, PipelineModel::browse);
    idle(cx, &model);
    let error = read(cx, &model, |m| m.error().cloned()).expect("a banner");
    assert_eq!(error.copy(), PICK_FAILED);
    assert_eq!(error.detail(), "no shell");
    assert_eq!(folder(cx, &model), None);
}

// ---- status and reasons ----

#[gpui::test]
fn step_status_covers_every_case(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixturePipeline::demo());
    idle(cx, &model);
    assert_eq!(status(cx, &model, StepId::Verify), StepStatus::Passed);
    assert_eq!(
        status(cx, &model, StepId::RunBatch),
        StepStatus::Problems(2)
    );
    assert_eq!(status(cx, &model, StepId::Schedule), StepStatus::Waiting);
    assert_eq!(status(cx, &model, StepId::Tarpack), StepStatus::Waiting);

    let (model, _) = start(cx, FixturePipeline::chosen());
    idle(cx, &model);
    assert_eq!(status(cx, &model, StepId::Verify), StepStatus::NotRun);
    assert_eq!(status(cx, &model, StepId::RunBatch), StepStatus::Waiting);

    let (model, _) = start(cx, FixturePipeline::ran());
    idle(cx, &model);
    assert_eq!(
        status(cx, &model, StepId::Verify),
        StepStatus::NotImplemented
    );

    let failing = FixturePipeline::chosen()
        .with_behavior(StepId::Verify, Behavior::Fail("disk went away".into()));
    let (model, _) = start(cx, failing);
    idle(cx, &model);
    run_step(cx, &model, StepId::Verify);
    assert_eq!(status(cx, &model, StepId::Verify), StepStatus::Failed);

    let gate = StepGate::new();
    let held = FixturePipeline::chosen().with_gate(&gate, Duration::from_secs(20));
    let (model, _) = start(cx, held);
    idle(cx, &model);
    act(cx, &model, |m, cx| m.request_step(StepId::Verify, cx));
    cx.run_until_parked();
    assert_eq!(status(cx, &model, StepId::Verify), StepStatus::Running);
    assert_eq!(status(cx, &model, StepId::RunBatch), StepStatus::Waiting);
    gate.open();
    idle(cx, &model);

    let labels = [
        (StepStatus::Running, "Running…"),
        (StepStatus::NotImplemented, "Not implemented"),
        (StepStatus::Failed, "Failed"),
        (StepStatus::Passed, "Passed"),
        (StepStatus::Problems(1), "1 problem"),
        (StepStatus::Problems(2), "2 problems"),
        (StepStatus::Waiting, "Waiting"),
        (StepStatus::NotRun, "Not run"),
    ];
    for (status, label) in labels {
        assert_eq!(text::status_label(status), label);
    }
}

#[gpui::test]
fn run_reason_and_can_run(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixturePipeline::empty());
    idle(cx, &model);
    assert_eq!(
        read(cx, &model, super::model::PipelineModel::run_reason),
        Some("Choose a folder.")
    );
    assert!(read(cx, &model, |m| !m.can_run()));
    act(cx, &model, super::model::PipelineModel::request_all);
    assert!(
        read(cx, &model, |m| m.pending().is_none()),
        "refused silently"
    );

    let (model, _) = start(cx, FixturePipeline::bad());
    idle(cx, &model);
    assert_eq!(
        read(cx, &model, super::model::PipelineModel::run_reason),
        Some("Fix the folder path above first.")
    );
    act(cx, &model, |m, cx| m.request_step(StepId::Verify, cx));
    idle(cx, &model);
    assert_eq!(status(cx, &model, StepId::Verify), StepStatus::NotRun);
    assert!(read(cx, &model, |m| m.error().is_none()));

    let gate = StepGate::new();
    let held = FixturePipeline::chosen().with_gate(&gate, Duration::from_secs(20));
    let (model, _) = start(cx, held);
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.can_run() && m.run_reason().is_none()));
    act(cx, &model, |m, cx| m.request_step(StepId::Verify, cx));
    cx.run_until_parked();
    assert_eq!(
        read(cx, &model, super::model::PipelineModel::run_reason),
        Some("A step is running.")
    );
    assert!(read(cx, &model, |m| !m.can_run() && m.busy()));
    act(cx, &model, |m, cx| {
        m.request_all(cx);
        m.set_directory_text("C:\\other", cx);
        m.browse(cx);
    });
    assert!(read(cx, &model, |m| m.pending().is_none()));
    gate.open();
    idle(cx, &model);
    assert_eq!(
        folder(cx, &model).as_deref(),
        Some(DIR),
        "the folder did not change"
    );
    assert!(read(cx, &model, |m| m.error().is_none()));
}

// ---- running ----

#[gpui::test]
fn step_without_confirm_runs_immediately(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixturePipeline::chosen().all_pass());
    idle(cx, &model);
    act(cx, &model, |m, cx| m.request_step(StepId::Verify, cx));
    assert!(read(cx, &model, |m| m.pending().is_none()));
    idle(cx, &model);
    assert_eq!(status(cx, &model, StepId::Verify), StepStatus::Passed);
    let said = read(cx, &model, |m| m.announcement().map(str::to_string));
    assert_eq!(said.as_deref(), Some("Check required files: Passed."));
    assert_eq!(hint(cx, &model), Some(FocusHint::Step(StepId::Verify)));
    assert_eq!(
        read(cx, &model, super::model::PipelineModel::next_step),
        Some(StepId::RunBatch)
    );
}

#[gpui::test]
fn step_with_confirm_waits_and_uses_the_confirmed_folder(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixturePipeline::chosen().all_pass());
    idle(cx, &model);
    run_step(cx, &model, StepId::Verify);
    act(cx, &model, |m, cx| m.request_step(StepId::RunBatch, cx));
    idle(cx, &model);
    let pending = read(cx, &model, |m| {
        m.pending().map(|(p, d)| (*p, d.to_string()))
    });
    assert_eq!(
        pending,
        Some((
            Pending::Step {
                id: StepId::RunBatch,
                via_next: false
            },
            DIR.to_string()
        ))
    );
    assert_eq!(
        status(cx, &model, StepId::RunBatch),
        StepStatus::NotRun,
        "nothing ran yet"
    );
    act(cx, &model, PipelineModel::confirm);
    assert!(read(cx, &model, |m| m.pending().is_none()));
    idle(cx, &model);
    assert_eq!(status(cx, &model, StepId::RunBatch), StepStatus::Passed);
}

#[gpui::test]
fn cancel_runs_nothing(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixturePipeline::chosen().all_pass());
    idle(cx, &model);
    run_step(cx, &model, StepId::Verify);
    act(cx, &model, |m, cx| m.request_step(StepId::RunBatch, cx));
    assert!(read(cx, &model, |m| m.pending().is_some()));
    act(cx, &model, PipelineModel::cancel);
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.pending().is_none() && !m.busy()));
    assert_eq!(status(cx, &model, StepId::RunBatch), StepStatus::NotRun);
    act(cx, &model, PipelineModel::confirm);
    idle(cx, &model);
    assert_eq!(
        status(cx, &model, StepId::RunBatch),
        StepStatus::NotRun,
        "confirm after cancel does nothing"
    );
}

#[gpui::test]
fn run_all_confirms_once_and_stops_at_first_failure(cx: &mut TestAppContext) {
    let backend = FixturePipeline::chosen()
        .all_pass()
        .with_behavior(StepId::RunBatch, Behavior::NotImplemented);
    let (model, _) = start(cx, backend);
    idle(cx, &model);
    act(cx, &model, super::model::PipelineModel::request_all);
    assert!(read(
        cx,
        &model,
        |m| matches!(m.pending(), Some((Pending::All, d)) if d == DIR)
    ));
    assert!(read(cx, &model, |m| !m.running_all()));
    act(cx, &model, PipelineModel::confirm);
    assert!(read(cx, &model, |m| m.running_all() && m.busy()));
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.pending().is_none() && !m.running_all()));
    assert_eq!(status(cx, &model, StepId::Verify), StepStatus::Passed);
    assert_eq!(
        status(cx, &model, StepId::RunBatch),
        StepStatus::NotImplemented
    );
    assert_eq!(status(cx, &model, StepId::Schedule), StepStatus::Waiting);
    let said = read(cx, &model, |m| m.announcement().map(str::to_string));
    assert_eq!(said.as_deref(), Some("Stopped at Run batch files."));
    assert_eq!(hint(cx, &model), Some(FocusHint::Step(StepId::RunBatch)));
}

#[gpui::test]
fn run_all_all_passed_announces(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixturePipeline::chosen().all_pass());
    idle(cx, &model);
    act(cx, &model, super::model::PipelineModel::request_all);
    act(cx, &model, PipelineModel::confirm);
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.all_passed() && m.next_step().is_none()));
    let said = read(cx, &model, |m| m.announcement().map(str::to_string));
    assert_eq!(said.as_deref(), Some("All steps passed."));
    assert_eq!(hint(cx, &model), Some(FocusHint::Step(StepId::Tarpack)));
    assert!(read(cx, &model, |m| m.can_run() && m.run_reason().is_none()));
}

#[gpui::test]
fn run_next_busy_only_for_its_step(cx: &mut TestAppContext) {
    let gate = StepGate::new();
    let held = FixturePipeline::chosen()
        .all_pass()
        .with_gate(&gate, Duration::from_secs(20));
    let (model, _) = start(cx, held);
    idle(cx, &model);
    act(cx, &model, |m, cx| m.request_step(StepId::Verify, cx));
    cx.run_until_parked();
    assert!(read(cx, &model, |m| m.running() == Some(StepId::Verify)
        && m.via_next().is_none()));
    gate.open();
    idle(cx, &model);

    let gate = StepGate::new();
    let held = FixturePipeline::chosen()
        .all_pass()
        .with_gate(&gate, Duration::from_secs(20));
    let (model, _) = start(cx, held);
    idle(cx, &model);
    act(cx, &model, PipelineModel::request_next);
    cx.run_until_parked();
    assert!(read(cx, &model, |m| m.running() == Some(StepId::Verify)
        && m.via_next() == Some(StepId::Verify)));
    gate.open();
    idle(cx, &model);
    assert_eq!(
        read(cx, &model, super::model::PipelineModel::via_next),
        None
    );
}

/// "Run next" until nothing is left, confirming when asked; the hint after
/// each run.
fn walk_with_next(
    cx: &mut TestAppContext,
    model: &Entity<PipelineModel>,
) -> Vec<Option<FocusHint>> {
    let mut hints = Vec::new();
    while read(cx, model, |m| m.next_step().is_some()) {
        act(cx, model, PipelineModel::request_next);
        if read(cx, model, |m| m.pending().is_some()) {
            act(cx, model, PipelineModel::confirm);
        }
        idle(cx, model);
        hints.push(hint(cx, model));
    }
    hints
}

#[gpui::test]
fn focus_hint_after_run_next(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixturePipeline::chosen().all_pass());
    idle(cx, &model);
    let hints = walk_with_next(cx, &model);
    assert_eq!(
        hints,
        [
            Some(FocusHint::RunNext),
            Some(FocusHint::RunNext),
            Some(FocusHint::RunNext),
            Some(FocusHint::Step(StepId::Tarpack)),
        ]
    );
    // After a run from a step's own button, focus goes to that step.
    let (model, _) = start(cx, FixturePipeline::chosen().all_pass());
    idle(cx, &model);
    run_step(cx, &model, StepId::Verify);
    assert_eq!(hint(cx, &model), Some(FocusHint::Step(StepId::Verify)));
}

#[gpui::test]
fn focus_hint_is_cleared_on_read(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixturePipeline::chosen().all_pass());
    idle(cx, &model);
    act(cx, &model, PipelineModel::request_next);
    idle(cx, &model);
    assert_eq!(hint(cx, &model), Some(FocusHint::RunNext));
    assert_eq!(hint(cx, &model), None);
    act(cx, &model, PipelineModel::request_next);
    act(cx, &model, PipelineModel::confirm);
    idle(cx, &model);
    assert_eq!(
        hint(cx, &model),
        Some(FocusHint::RunNext),
        "a second run, a second hint"
    );
    // An unread hint does not survive the next request.
    act(cx, &model, PipelineModel::request_next);
    act(cx, &model, super::model::PipelineModel::request_all);
    assert_eq!(hint(cx, &model), None);
}

#[gpui::test]
fn repeated_result_bumps_announcement_generation(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixturePipeline::chosen().all_pass());
    idle(cx, &model);
    run_step(cx, &model, StepId::Verify);
    let first = read(cx, &model, PipelineModel::announcement_generation);
    let said = read(cx, &model, |m| m.announcement().map(str::to_string));
    run_step(cx, &model, StepId::Verify);
    let again = read(cx, &model, |m| m.announcement().map(str::to_string));
    assert_eq!(said, again, "the same sentence");
    assert!(read(cx, &model, PipelineModel::announcement_generation) > first);
}

#[gpui::test]
fn locked_step_is_refused_without_the_view(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixturePipeline::chosen().all_pass());
    idle(cx, &model);
    for id in [StepId::RunBatch, StepId::Schedule, StepId::Tarpack] {
        act(cx, &model, |m, cx| m.request_step(id, cx));
        assert!(read(cx, &model, |m| m.pending().is_none() && !m.busy()));
        idle(cx, &model);
        assert!(read(cx, &model, |m| m.error().is_none()));
        assert_eq!(status(cx, &model, id), StepStatus::Waiting);
    }
}

#[gpui::test]
fn second_request_in_the_same_frame_does_not_start_a_second_run(cx: &mut TestAppContext) {
    let gate = StepGate::new();
    let held = FixturePipeline::chosen()
        .all_pass()
        .with_gate(&gate, Duration::from_secs(20));
    let (model, _) = start(cx, held);
    idle(cx, &model);
    // No park between the calls.
    act(cx, &model, |m, cx| {
        m.request_step(StepId::Verify, cx);
        assert!(m.busy(), "busy at once");
        m.request_step(StepId::Verify, cx);
        m.request_next(cx);
    });
    for _ in 0..3 {
        cx.run_until_parked();
        assert!(read(cx, &model, |m| m.busy() && m.error().is_none()));
        assert_eq!(
            read(cx, &model, PipelineModel::running),
            Some(StepId::Verify)
        );
    }
    gate.open();
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.error().is_none()));
    assert_eq!(status(cx, &model, StepId::Verify), StepStatus::Passed);
    assert_eq!(
        read(cx, &model, PipelineModel::next_step),
        Some(StepId::RunBatch)
    );
}

#[gpui::test]
fn unexpected_failure_in_a_run_refreshes_the_session(cx: &mut TestAppContext) {
    let flaky = Flaky {
        inner: FixturePipeline::chosen(),
        panic_session: false,
        panic_set_directory: false,
        panic_finish: true,
    };
    let (model, _) = start_boxed(cx, Box::new(flaky));
    idle(cx, &model);
    run_step(cx, &model, StepId::Verify);
    assert!(read(cx, &model, |m| matches!(
        m.error(),
        Some(ViewError::Unexpected { .. })
    )));
    // The session is the backend's, fetched again: the step is not running.
    assert!(read(cx, &model, |m| !m.busy()
        && m.session().is_some_and(|s| s.running.is_none())));
}

// ---- errors ----

#[gpui::test]
fn begin_error_is_a_banner_and_step_failure_is_not(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixturePipeline::chosen().all_pass());
    idle(cx, &model);
    run_step(cx, &model, StepId::Verify);
    // The confirmation names a folder; the step runs on exactly that one, so a
    // folder changed in the meantime refuses it.
    act(cx, &model, |m, cx| m.request_step(StepId::RunBatch, cx));
    assert!(read(cx, &model, |m| m.pending().is_some()));
    act(cx, &model, |m, cx| m.set_directory_text("C:\\Other", cx));
    idle(cx, &model);
    act(cx, &model, PipelineModel::confirm);
    idle(cx, &model);
    let error = read(cx, &model, |m| m.error().cloned()).expect("a banner");
    assert_eq!(
        error.copy(),
        text::error_message(PipelineErrorKind::DirectoryChanged)
    );
    assert!(
        matches!(&error, ViewError::Pipeline(e) if e.kind == PipelineErrorKind::DirectoryChanged)
    );
    assert!(read(cx, &model, |m| !m.busy()));
    assert_eq!(status(cx, &model, StepId::RunBatch), StepStatus::Waiting);

    // A step that fails is recorded in the session, not a banner.
    let failing = FixturePipeline::chosen()
        .with_behavior(StepId::Verify, Behavior::Fail("disk went away".into()));
    let (model, _) = start(cx, failing);
    idle(cx, &model);
    run_step(cx, &model, StepId::Verify);
    assert!(read(cx, &model, |m| m.error().is_none()));
    assert_eq!(status(cx, &model, StepId::Verify), StepStatus::Failed);
    let detail = read(cx, &model, |m| {
        m.session()
            .and_then(|s| s.steps[0].error.as_ref())
            .map(|e| (e.kind, e.message.clone()))
    });
    assert_eq!(
        detail,
        Some((PipelineErrorKind::StepFailed, "disk went away".to_string()))
    );

    let (model, _) = start(cx, FixturePipeline::chosen());
    idle(cx, &model);
    run_step(cx, &model, StepId::Verify);
    assert!(read(cx, &model, |m| m.error().is_none()));
    assert_eq!(
        status(cx, &model, StepId::Verify),
        StepStatus::NotImplemented
    );
}

#[gpui::test]
fn repeated_error_bumps_generation(cx: &mut TestAppContext) {
    let (model, fake) = start(cx, FixturePipeline::empty());
    idle(cx, &model);
    assert_eq!(
        read(cx, &model, super::model::PipelineModel::error_generation),
        0
    );
    for expected in 1..=2 {
        fake.answer_pick(Err(PickError::Failed {
            detail: "same".into(),
        }));
        act(cx, &model, PipelineModel::browse);
        idle(cx, &model);
        assert_eq!(
            read(cx, &model, super::model::PipelineModel::error_generation),
            expected
        );
        assert!(read(cx, &model, |m| m.error().is_some()));
    }
}

#[gpui::test]
fn dismiss_error_clears_it(cx: &mut TestAppContext) {
    let (model, fake) = start(cx, FixturePipeline::empty());
    idle(cx, &model);
    fake.answer_pick(Err(PickError::Failed { detail: "x".into() }));
    act(cx, &model, PipelineModel::browse);
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.error().is_some()));
    act(cx, &model, PipelineModel::dismiss_error);
    assert!(read(cx, &model, |m| m.error().is_none()));
}

#[gpui::test]
fn panicked_step_job_is_recorded_as_failed_without_a_banner(cx: &mut TestAppContext) {
    let backend = FixturePipeline::chosen().with_behavior(StepId::Verify, Behavior::Panic);
    let (model, _) = start(cx, backend);
    idle(cx, &model);
    run_step(cx, &model, StepId::Verify);
    assert!(read(cx, &model, |m| !m.busy() && m.error().is_none()));
    assert_eq!(status(cx, &model, StepId::Verify), StepStatus::Failed);
    let error = read(cx, &model, |m| {
        m.session().and_then(|s| s.steps[0].error.clone())
    })
    .expect("recorded");
    assert_eq!(error.kind, PipelineErrorKind::StepFailed);
    assert!(
        error.message.starts_with("the step stopped unexpectedly: "),
        "{}",
        error.message
    );
    // The session is usable again.
    assert!(read(cx, &model, PipelineModel::can_run));
}

#[gpui::test]
fn panicking_backend_call_is_unexpected(cx: &mut TestAppContext) {
    let flaky = Flaky {
        inner: FixturePipeline::empty(),
        panic_session: false,
        panic_set_directory: true,
        panic_finish: false,
    };
    let (model, _) = start_boxed(cx, Box::new(flaky));
    idle(cx, &model);
    act(cx, &model, |m, cx| m.set_directory_text("C:\\x", cx));
    idle(cx, &model);
    let error = read(cx, &model, |m| m.error().cloned()).expect("a banner");
    assert!(matches!(&error, ViewError::Unexpected { message } if message.contains("exploded")));
    assert_eq!(error.copy(), text::UNEXPECTED);
}

// ---- activation ----

#[gpui::test]
fn activated_clears_transient_state_and_keeps_running_step(cx: &mut TestAppContext) {
    let (model, fake) = start(cx, FixturePipeline::chosen().all_pass());
    idle(cx, &model);
    run_step(cx, &model, StepId::Verify);
    fake.answer_pick(Err(PickError::Failed { detail: "x".into() }));
    act(cx, &model, PipelineModel::browse);
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.error().is_some()
        && m.announcement().is_some()));
    act(cx, &model, PipelineModel::activated);
    assert!(read(cx, &model, |m| m.error().is_none()
        && m.announcement().is_none()));
    assert_eq!(hint(cx, &model), None, "the unread hint is gone");
    idle(cx, &model);
    assert!(
        read(cx, &model, |m| m.session().is_some()),
        "the session is refreshed"
    );

    act(cx, &model, |m, cx| m.request_step(StepId::RunBatch, cx));
    assert!(read(cx, &model, |m| m.pending().is_some()));
    act(cx, &model, PipelineModel::activated);
    assert!(read(cx, &model, |m| m.pending().is_none()));
    idle(cx, &model);

    // A step running when the tool is left lands its result here.
    let gate = StepGate::new();
    let held = FixturePipeline::chosen()
        .all_pass()
        .with_gate(&gate, Duration::from_secs(20));
    let (model, _) = start(cx, held);
    idle(cx, &model);
    act(cx, &model, |m, cx| m.request_step(StepId::Verify, cx));
    cx.run_until_parked();
    act(cx, &model, PipelineModel::activated);
    cx.run_until_parked();
    assert_eq!(
        read(cx, &model, super::model::PipelineModel::running),
        Some(StepId::Verify)
    );
    assert!(read(cx, &model, |m| m.busy()
        && m.session().is_some_and(|s| s.running.is_some())));
    gate.open();
    idle(cx, &model);
    assert_eq!(status(cx, &model, StepId::Verify), StepStatus::Passed);
    let said = read(cx, &model, |m| m.announcement().map(str::to_string));
    assert_eq!(said.as_deref(), Some("Check required files: Passed."));
}

// ---- the real backend ----

#[gpui::test]
fn real_backend_runs_verify_in_temp_dir(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir(dir.path().join("a")).expect("subfolder");
    std::fs::create_dir(dir.path().join("b")).expect("subfolder");
    std::fs::write(dir.path().join("file.txt"), "x").expect("file");
    let (model, _) = start_boxed(cx, Box::new(RealPipeline::new()));
    idle(cx, &model);
    let typed = dir.path().to_str().expect("utf-8 temp path").to_string();
    act(cx, &model, |m, cx| m.set_directory_text(&typed, cx));
    idle(cx, &model);
    let count = read(cx, &model, |m| {
        m.session()
            .and_then(|s| s.directory.as_ref())
            .map(|d| d.subdirectories)
    });
    assert_eq!(count, Some(2));
    run_step(cx, &model, StepId::Verify);
    assert!(read(cx, &model, |m| m.error().is_none()));
    assert_eq!(
        status(cx, &model, StepId::Verify),
        StepStatus::NotImplemented,
        "the stub hook"
    );
    let said = read(cx, &model, |m| m.announcement().map(str::to_string));
    assert_eq!(
        said.as_deref(),
        Some("Check required files: Not implemented.")
    );

    act(cx, &model, |m, cx| {
        m.set_directory_text("relative\\jobs", cx);
    });
    idle(cx, &model);
    let kind = read(cx, &model, |m| {
        m.session()
            .and_then(|s| s.directory.as_ref())
            .and_then(|d| d.error.as_ref())
            .map(|e| e.kind)
    });
    assert_eq!(kind, Some(PipelineErrorKind::NotAbsolute));
}

// ---- fixtures ----

#[gpui::test]
fn fixtures_build_each_screenshot_state(cx: &mut TestAppContext) {
    for fixture in [
        PipelineFixture::Empty,
        PipelineFixture::Chosen,
        PipelineFixture::Bad,
        PipelineFixture::Ran,
        PipelineFixture::Demo,
        PipelineFixture::Slow,
    ] {
        let (model, _) = start(cx, FixturePipeline::from_fixture(fixture));
        idle(cx, &model);
        let statuses: Vec<_> = STEP_IDS.iter().map(|id| status(cx, &model, *id)).collect();
        let slot = read(cx, &model, |m| {
            m.session().and_then(|s| s.directory.clone())
        });
        let can_run = read(cx, &model, PipelineModel::can_run);
        match fixture {
            PipelineFixture::Empty => {
                assert!(slot.is_none() && !can_run, "{fixture:?}");
            }
            PipelineFixture::Chosen | PipelineFixture::Slow => {
                let slot = slot.expect("a folder");
                assert_eq!((slot.path.as_str(), slot.subdirectories), (DIR, 12));
                assert!(slot.error.is_none() && can_run);
                assert_eq!(
                    statuses,
                    [
                        StepStatus::NotRun,
                        StepStatus::Waiting,
                        StepStatus::Waiting,
                        StepStatus::Waiting
                    ]
                );
            }
            PipelineFixture::Bad => {
                let error = slot.expect("a folder").error.expect("an error");
                assert_eq!(error.kind, PipelineErrorKind::NotADirectory);
                assert!(!can_run);
            }
            PipelineFixture::Ran => {
                assert_eq!(statuses[0], StepStatus::NotImplemented);
            }
            PipelineFixture::Demo => {
                assert_eq!(statuses[..2], [StepStatus::Passed, StepStatus::Problems(2)]);
            }
        }
    }
}
