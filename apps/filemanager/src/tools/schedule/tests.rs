//! The Schedule Creator model's tests: the copy, the files, browsing, drops,
//! applying and the fixtures. Model tests run against [`FixtureSchedule`] and
//! [`FakePlatform`]; one runs [`RealSchedule`] over a temporary directory.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use fm_schedule::session::{
    ApplySummary, FileSlot, Hooks as CoreHooks, ScheduleError, ScheduleErrorKind, ScheduleSession,
};
use fm_schedule::{MergeError, MergeOutcome, ParseError, Schedule};
use gpui::{AppContext as _, Context, Entity, TestAppContext};

use super::backend::{RealSchedule, ScheduleBackend};
use super::fixtures::{FixtureSchedule, ScheduleFixture, TEXT, XML};
use super::model::{FocusHint, ScheduleModel, Slot, ViewError};
use super::text;
use super::Hooks;
use crate::app::drop::DropTarget;
use crate::app::text::PICK_FAILED;
use crate::platform::fake::{FakePlatform, PlatformCall};
use crate::platform::{self, PickError, PickKind};
use crate::tools::ToolHooks as _;

const ALL_KINDS: [ScheduleErrorKind; 13] = [
    ScheduleErrorKind::NoText,
    ScheduleErrorKind::NoXml,
    ScheduleErrorKind::NotAFile,
    ScheduleErrorKind::NotAbsolute,
    ScheduleErrorKind::FilesChanged,
    ScheduleErrorKind::TextUnreadable,
    ScheduleErrorKind::XmlUnreadable,
    ScheduleErrorKind::DropAmbiguous,
    ScheduleErrorKind::ParseFailed,
    ScheduleErrorKind::ParseNotImplemented,
    ScheduleErrorKind::MergeFailed,
    ScheduleErrorKind::MergeNotImplemented,
    ScheduleErrorKind::Io,
];

fn err(kind: ScheduleErrorKind) -> ScheduleError {
    ScheduleError::new(kind, "detail")
}

fn summary(added: u32) -> ApplySummary {
    ApplySummary {
        xml_path: XML.to_string(),
        backup_path: format!("{XML}.bak"),
        added,
    }
}

// ---- helpers ----

fn start_boxed(
    cx: &mut TestAppContext,
    backend: Box<dyn ScheduleBackend>,
) -> (Entity<ScheduleModel>, FakePlatform) {
    cx.executor().allow_parking();
    let fake = FakePlatform::new();
    let installed = fake.clone();
    cx.update(|cx| platform::set_platform(cx, installed));
    let model = cx.new(|cx| ScheduleModel::new(backend, cx));
    (model, fake)
}

fn start(
    cx: &mut TestAppContext,
    backend: FixtureSchedule,
) -> (Entity<ScheduleModel>, FakePlatform) {
    start_boxed(cx, Box::new(backend))
}

fn read<R>(
    cx: &mut TestAppContext,
    model: &Entity<ScheduleModel>,
    f: impl FnOnce(&ScheduleModel) -> R,
) -> R {
    model.read_with(cx, |m, _| f(m))
}

fn act<R>(
    cx: &mut TestAppContext,
    model: &Entity<ScheduleModel>,
    f: impl FnOnce(&mut ScheduleModel, &mut Context<ScheduleModel>) -> R,
) -> R {
    model.update(cx, f)
}

/// Runs everything queued until nothing loads or applies.
fn idle(cx: &mut TestAppContext, model: &Entity<ScheduleModel>) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        cx.run_until_parked();
        if read(cx, model, |m| !m.loading() && !m.applying()) {
            cx.run_until_parked();
            return;
        }
        assert!(Instant::now() < deadline, "the model never went idle");
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn path_of(cx: &mut TestAppContext, model: &Entity<ScheduleModel>, slot: Slot) -> Option<String> {
    read(cx, model, |m| {
        let s = m.session()?;
        match slot {
            Slot::Text => s.text.as_ref(),
            Slot::Xml => s.xml.as_ref(),
        }
        .map(|f| f.path.clone())
    })
}

fn typed(cx: &mut TestAppContext, model: &Entity<ScheduleModel>, slot: Slot, s: &str) {
    act(cx, model, |m, cx| m.set_typed(slot, s, cx));
    idle(cx, model);
}

fn hint(cx: &mut TestAppContext, model: &Entity<ScheduleModel>) -> Option<FocusHint> {
    act(cx, model, |m, _| m.take_focus_hint())
}

fn apply_confirmed(cx: &mut TestAppContext, model: &Entity<ScheduleModel>) {
    act(cx, model, ScheduleModel::request_apply);
    act(cx, model, ScheduleModel::confirm);
    idle(cx, model);
}

type Calls = Arc<Mutex<Vec<(String, String)>>>;

/// Records what `apply` was called with, counts `session` calls, and panics
/// where told to (each flag once).
struct Spy {
    inner: FixtureSchedule,
    applied: Calls,
    sessions: Arc<AtomicUsize>,
    panic_session: bool,
    panic_set_xml: bool,
    panic_apply: bool,
}

impl Spy {
    fn new(inner: FixtureSchedule) -> (Self, Calls) {
        let applied = Calls::default();
        let spy = Spy {
            inner,
            applied: applied.clone(),
            sessions: Arc::default(),
            panic_session: false,
            panic_set_xml: false,
            panic_apply: false,
        };
        (spy, applied)
    }
}

impl ScheduleBackend for Spy {
    fn session(&mut self) -> ScheduleSession {
        self.sessions.fetch_add(1, Ordering::SeqCst);
        assert!(!std::mem::take(&mut self.panic_session), "session exploded");
        self.inner.session()
    }
    fn set_text(&mut self, path: Option<PathBuf>) -> ScheduleSession {
        self.inner.set_text(path)
    }
    fn set_xml(&mut self, path: Option<PathBuf>) -> ScheduleSession {
        // Changes the state, then fails: the model cannot know what holds.
        let session = self.inner.set_xml(path);
        assert!(!std::mem::take(&mut self.panic_set_xml), "set_xml exploded");
        session
    }
    fn set_dropped(&mut self, paths: Vec<PathBuf>) -> Result<ScheduleSession, ScheduleError> {
        self.inner.set_dropped(paths)
    }
    fn apply(&mut self, t: &str, x: &str) -> Result<ApplySummary, ScheduleError> {
        self.applied
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((t.to_string(), x.to_string()));
        assert!(!std::mem::take(&mut self.panic_apply), "apply exploded");
        self.inner.apply(t, x)
    }
}

// ---- copy ----

#[test]
fn text_covers_every_error_kind() {
    let mut seen = Vec::new();
    for kind in ALL_KINDS {
        let message = text::error_message(&err(kind));
        assert_ne!(message, "");
        assert!(!seen.contains(&message), "{kind:?} repeats a message");
        seen.push(message);
    }
    let parse = |line| ScheduleError {
        kind: ScheduleErrorKind::ParseFailed,
        message: "missing time".to_string(),
        line,
    };
    assert_eq!(
        text::error_message(&parse(Some(4))),
        "Line 4 of the schedule file: missing time. Nothing was changed."
    );
    assert_eq!(
        text::error_message(&parse(None)),
        "The schedule file has a problem: missing time. Nothing was changed."
    );
    let mut stop = parse(Some(2));
    stop.message = "bad!".to_string();
    assert_eq!(
        text::error_message(&stop),
        "Line 2 of the schedule file: bad! Nothing was changed."
    );
    assert_eq!(text::file_name("C:\\work\\week.txt"), "week.txt");
    assert_eq!(text::file_name("/a/b.xml"), "b.xml");
    assert_eq!(text::file_name("C:\\"), "C:\\");
    assert_eq!(text::file_name("plain"), "plain");
    assert_eq!(
        text::result_sentence(&summary(1)),
        "Added 1 entry to schedule.xml."
    );
    assert_eq!(
        text::result_sentence(&summary(2)),
        "Added 2 entries to schedule.xml."
    );
    assert_eq!(text::confirm_title(XML), "Update schedule.xml?");
    assert_eq!(
        text::confirm_body(TEXT, XML).plain(),
        format!(
            "The schedule in {TEXT} will be read and added to {XML}. \
             A backup copy of the XML file is saved next to it first."
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

/// Every plain string of the TypeScript copy appears, as is, in `text.rs`.
/// (`ParseFailed`'s template literals are checked by value in
/// `text_covers_every_error_kind`.) Goes once the Tauri app is deleted.
#[test]
fn text_matches_ts_sources() {
    let errors = include_str!("../../../../desktop/src/tools/schedule/errorMessages.ts");
    let view = include_str!("../../../../desktop/src/tools/schedule/ScheduleView.tsx");
    let table = &errors[errors.find("const messages").expect("messages table")..];
    let table = &table[..table.find("\n};").expect("end of table")];
    let unexpected = errors
        .lines()
        .find(|l| l.contains("\"Unexpected\") return"))
        .expect("the Unexpected line");

    let mut ts = literals(table);
    ts.extend(literals(unexpected).pop());
    ts.sort();
    ts.dedup();

    let mut ours: Vec<String> = ALL_KINDS
        .iter()
        .filter(|k| **k != ScheduleErrorKind::ParseFailed)
        .map(|k| text::error_message(&err(*k)))
        .collect();
    ours.push(text::UNEXPECTED.to_string());
    ours.sort();
    ours.dedup();
    assert_eq!(ours, ts);

    // The view's own copy; long strings wrap across lines in JSX, so compare
    // without the wrapping.
    let view = view.split_whitespace().collect::<Vec<_>>().join(" ");
    for (needle, ours) in [
        ("Choose a schedule file.", text::REASON_NO_TEXT),
        ("Choose the XML file to update.", text::REASON_NO_XML),
        ("Fix the file paths above first.", text::REASON_FIX_PATHS),
        ("The XML file is being updated", text::REASON_APPLYING),
        ("Finish or cancel the update first", text::REASON_CONFIRMING),
        ("Not ready yet", text::REASON_NOT_READY),
        (text::DROP_LABEL, text::DROP_LABEL),
        (text::TEXT_LABEL, text::TEXT_LABEL),
        (text::XML_LABEL, text::XML_LABEL),
        (text::TEXT_PLACEHOLDER, text::TEXT_PLACEHOLDER),
        (text::XML_PLACEHOLDER, text::XML_PLACEHOLDER),
        (text::TEXT_BROWSE_LABEL, text::TEXT_BROWSE_LABEL),
        (text::XML_BROWSE_LABEL, text::XML_BROWSE_LABEL),
        (text::ADD, text::ADD),
        (text::ADDING, text::ADDING),
        (text::CONFIRM_LABEL, text::CONFIRM_LABEL),
        (text::TRY_AGAIN, text::TRY_AGAIN),
        (text::BROWSE, text::BROWSE),
        (text::DUPLICATE_HEADLINE, text::DUPLICATE_HEADLINE),
        (text::DUPLICATE_REST, text::DUPLICATE_REST),
        (text::TITLE, text::TITLE),
    ] {
        assert!(view.contains(needle), "{ours:?} is not in the TS view");
    }
    let flat = &view;
    for (needle, ours) in [
        (">Loading<", text::LOADING),
        (">Files<", text::FILES_LEGEND),
        ("<summary>Details</summary>", text::DETAILS),
        (text::BACKUP_PREFIX.trim_end(), text::BACKUP_PREFIX),
        (
            "`Update ${fileName(confirmed[1])}?`",
            "the confirmation title",
        ),
    ] {
        assert!(flat.contains(needle), "{ours:?} is not in the TS view");
    }
    assert_eq!(text::confirm_title("C:\\a\\b.xml"), "Update b.xml?");
    let body = text::confirm_body("T", "X");
    assert_eq!(
        (body.text_path.as_str(), body.xml_path.as_str()),
        ("T", "X")
    );
    for piece in [body.before, body.between, body.after] {
        let piece = piece.trim().trim_start_matches('.').trim();
        assert!(flat.contains(piece), "{piece:?} is not in the TS view");
    }
    assert!(flat.contains(text::INTRO), "intro");
    assert!(flat.contains("A backup copy of the XML file is saved next to it first."));
    assert!(flat.contains("The schedule in"));
    assert!(flat.contains("will be read and added to"));
}

// ---- loading ----

#[gpui::test]
fn loads_session_and_retries_after_failure(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixtureSchedule::ready());
    assert!(read(cx, &model, |m| m.loading() && m.session().is_none()));
    idle(cx, &model);
    assert!(read(cx, &model, |m| !m.loading() && !m.load_failed()));
    assert_eq!(path_of(cx, &model, Slot::Text).as_deref(), Some(TEXT));

    let (mut spy, _) = Spy::new(FixtureSchedule::empty());
    spy.panic_session = true;
    let (model, _) = start_boxed(cx, Box::new(spy));
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.load_failed()
        && m.session().is_none()
        && !m.loading()));
    let error = read(cx, &model, |m| m.error().cloned());
    assert!(
        matches!(error, Some(ViewError::Unexpected { .. })),
        "{error:?}"
    );
    act(cx, &model, ScheduleModel::retry_load);
    assert!(read(cx, &model, |m| !m.load_failed() && m.loading()));
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.session().is_some() && m.error().is_none()));
}

// ---- the files ----

#[gpui::test]
fn typed_paths_set_slots(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixtureSchedule::empty());
    idle(cx, &model);
    typed(cx, &model, Slot::Text, &format!(" \"{TEXT}\" "));
    assert_eq!(path_of(cx, &model, Slot::Text).as_deref(), Some(TEXT));
    assert_eq!(
        read(cx, &model, ScheduleModel::apply_reason),
        Some(text::REASON_NO_XML)
    );
    typed(cx, &model, Slot::Xml, XML);
    assert_eq!(path_of(cx, &model, Slot::Xml).as_deref(), Some(XML));
    assert!(read(cx, &model, |m| m.can_apply() && m.announcement().is_none()));

    // Unchanged: nothing is sent.
    let before = read(cx, &model, |m| m.session().cloned());
    typed(cx, &model, Slot::Xml, XML);
    assert_eq!(read(cx, &model, |m| m.session().cloned()), before);

    // Empty clears.
    typed(cx, &model, Slot::Text, "   ");
    assert_eq!(path_of(cx, &model, Slot::Text), None);
    assert!(read(cx, &model, |m| !m.can_apply()));
}

#[gpui::test]
fn typed_path_error_is_announced(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixtureSchedule::empty());
    idle(cx, &model);
    typed(cx, &model, Slot::Text, "week.txt");
    let said = text::error_message(&err(ScheduleErrorKind::NotAbsolute));
    assert_eq!(
        read(cx, &model, |m| m.announcement().map(String::from)),
        Some(said)
    );
    // The slot error is not a banner.
    assert!(read(cx, &model, |m| m.error().is_none()));
    let first = read(cx, &model, ScheduleModel::announcement_generation);
    typed(cx, &model, Slot::Text, "month.txt");
    assert!(read(cx, &model, ScheduleModel::announcement_generation) > first);
}

#[gpui::test]
fn only_newest_command_applies(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixtureSchedule::empty());
    idle(cx, &model);
    // Two commands in a row: the first (a bad path, which would announce) is
    // superseded before it answers.
    act(cx, &model, |m, cx| m.set_typed(Slot::Text, "week.txt", cx));
    act(cx, &model, |m, cx| m.set_typed(Slot::Xml, XML, cx));
    idle(cx, &model);
    assert_eq!(read(cx, &model, ScheduleModel::announcement_generation), 0);
    assert_eq!(path_of(cx, &model, Slot::Xml).as_deref(), Some(XML));
}

#[gpui::test]
fn browse_sets_slot_and_cancel_does_nothing(cx: &mut TestAppContext) {
    let (model, fake) = start(cx, FixtureSchedule::empty());
    idle(cx, &model);
    fake.answer_pick(Ok(Some(PathBuf::from(TEXT))));
    act(cx, &model, |m, cx| m.browse(Slot::Text, cx));
    idle(cx, &model);
    assert_eq!(path_of(cx, &model, Slot::Text).as_deref(), Some(TEXT));
    // A browse does not announce.
    assert!(read(cx, &model, |m| m.announcement().is_none()));

    fake.answer_pick(Ok(None));
    act(cx, &model, |m, cx| m.browse(Slot::Xml, cx));
    idle(cx, &model);
    assert_eq!(path_of(cx, &model, Slot::Xml), None);
    assert!(read(cx, &model, |m| m.error().is_none()));
}

#[gpui::test]
fn browse_passes_todays_filters(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("week.txt");
    fs::write(&file, "x").expect("write");
    let (model, fake) = start_boxed(cx, Box::new(RealSchedule::new()));
    idle(cx, &model);
    act(cx, &model, |m, cx| m.browse(Slot::Text, cx));
    act(cx, &model, |m, cx| m.browse(Slot::Xml, cx));
    idle(cx, &model);
    // With a file chosen, the dialog starts in its folder.
    typed(
        cx,
        &model,
        Slot::Text,
        file.to_str().expect("utf-8 temp path"),
    );
    act(cx, &model, |m, cx| m.browse(Slot::Text, cx));
    idle(cx, &model);

    let specs: Vec<_> = fake
        .calls()
        .into_iter()
        .map(|c| match c {
            PlatformCall::Pick(spec) => spec,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(specs.len(), 3);
    let filters = |i: usize| -> Vec<(String, Vec<String>)> {
        specs[i]
            .filters
            .iter()
            .map(|f| (f.name.clone(), f.patterns.clone()))
            .collect()
    };
    let all = ("All files".to_string(), vec!["*.*".to_string()]);
    assert_eq!(
        filters(0),
        [("Text".to_string(), vec!["*.txt".to_string()]), all.clone()]
    );
    assert_eq!(
        filters(1),
        [("XML".to_string(), vec!["*.xml".to_string()]), all]
    );
    assert!(specs.iter().all(|s| s.kind == PickKind::File));
    assert_eq!(specs[0].start_dir, None);
    assert_eq!(specs[2].start_dir.as_deref(), Some(dir.path()));
}

#[gpui::test]
fn browse_error_shows_dialog_failure_copy(cx: &mut TestAppContext) {
    let (model, fake) = start(cx, FixtureSchedule::empty());
    idle(cx, &model);
    fake.answer_pick(Err(PickError::Failed {
        detail: "com error".to_string(),
    }));
    act(cx, &model, |m, cx| m.browse(Slot::Xml, cx));
    idle(cx, &model);
    let error = read(cx, &model, |m| m.error().cloned()).expect("an error");
    assert_eq!(error.copy(), PICK_FAILED);
    assert_eq!(error.detail(), "com error");
    assert_eq!(read(cx, &model, ScheduleModel::error_generation), 1);
    act(cx, &model, ScheduleModel::dismiss_error);
    assert!(read(cx, &model, |m| m.error().is_none()));
}

// ---- applying ----

#[gpui::test]
fn apply_reason_and_can_apply(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixtureSchedule::empty());
    assert_eq!(
        read(cx, &model, ScheduleModel::apply_reason),
        None,
        "loading"
    );
    assert!(read(cx, &model, |m| !m.can_apply()));
    idle(cx, &model);
    assert_eq!(
        read(cx, &model, ScheduleModel::apply_reason),
        Some("Choose a schedule file.")
    );
    typed(cx, &model, Slot::Text, TEXT);
    assert_eq!(
        read(cx, &model, ScheduleModel::apply_reason),
        Some("Choose the XML file to update.")
    );
    typed(cx, &model, Slot::Xml, "rel.xml");
    assert_eq!(
        read(cx, &model, ScheduleModel::apply_reason),
        Some("Fix the file paths above first.")
    );
    assert!(read(cx, &model, |m| !m.can_apply()));
    // A refused request opens nothing.
    act(cx, &model, ScheduleModel::request_apply);
    assert!(read(cx, &model, |m| m.pending().is_none()));
    typed(cx, &model, Slot::Xml, XML);
    assert_eq!(read(cx, &model, ScheduleModel::apply_reason), None);
    assert!(read(cx, &model, ScheduleModel::can_apply));
}

#[gpui::test]
fn apply_confirms_with_shown_paths_and_applies_exactly_those(cx: &mut TestAppContext) {
    let (spy, applied) = Spy::new(FixtureSchedule::ready().with_outcome(Ok(summary(2))));
    let (model, _) = start_boxed(cx, Box::new(spy));
    idle(cx, &model);
    act(cx, &model, ScheduleModel::request_apply);
    assert_eq!(
        read(cx, &model, |m| m
            .pending()
            .map(|(t, x)| (t.to_string(), x.to_string()))),
        Some((TEXT.to_string(), XML.to_string()))
    );
    assert!(applied.lock().unwrap().is_empty(), "nothing before confirm");
    // The files are fixed while the confirmation is open.
    typed(cx, &model, Slot::Xml, "C:\\Other\\other.xml");
    assert_eq!(path_of(cx, &model, Slot::Xml).as_deref(), Some(XML));
    assert!(read(cx, &model, |m| !m.can_apply()));

    act(cx, &model, ScheduleModel::confirm);
    assert!(read(cx, &model, |m| m.applying() && m.pending().is_none()));
    idle(cx, &model);
    assert_eq!(
        *applied.lock().unwrap(),
        [(TEXT.to_string(), XML.to_string())]
    );
    assert_eq!(hint(cx, &model), Some(FocusHint::Outcome));
    assert_eq!(hint(cx, &model), None, "cleared on read");
}

#[gpui::test]
fn cancel_applies_nothing(cx: &mut TestAppContext) {
    let (spy, applied) = Spy::new(FixtureSchedule::ready());
    let (model, _) = start_boxed(cx, Box::new(spy));
    idle(cx, &model);
    act(cx, &model, ScheduleModel::request_apply);
    act(cx, &model, ScheduleModel::cancel);
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.pending().is_none() && !m.applying()));
    assert!(applied.lock().unwrap().is_empty());
    // Confirm with nothing open does nothing either.
    act(cx, &model, ScheduleModel::confirm);
    idle(cx, &model);
    assert!(applied.lock().unwrap().is_empty());
}

#[gpui::test]
fn result_sentence_after_apply(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixtureSchedule::ready().with_outcome(Ok(summary(2))));
    idle(cx, &model);
    apply_confirmed(cx, &model);
    let result = read(cx, &model, |m| m.result().cloned()).expect("a result");
    assert_eq!(
        text::result_sentence(&result),
        "Added 2 entries to schedule.xml."
    );
    assert_eq!(result.backup_path, format!("{XML}.bak"));
    assert_eq!(
        read(cx, &model, |m| m.announcement().map(String::from)).as_deref(),
        Some("Added 2 entries to schedule.xml.")
    );
    assert!(read(cx, &model, |m| m.error().is_none()));
}

#[gpui::test]
fn same_pair_twice_warns_duplicate(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixtureSchedule::ready().with_outcome(Ok(summary(1))));
    idle(cx, &model);
    assert!(read(cx, &model, |m| !m.already_applied()));
    apply_confirmed(cx, &model);
    assert!(read(cx, &model, ScheduleModel::already_applied));
    act(cx, &model, ScheduleModel::request_apply);
    assert!(
        read(cx, &model, ScheduleModel::already_applied),
        "warns in the confirmation"
    );
    act(cx, &model, ScheduleModel::cancel);
    // Another pair is not a duplicate.
    typed(cx, &model, Slot::Xml, "C:\\Config\\other.xml");
    assert!(read(cx, &model, |m| !m.already_applied()));
    typed(cx, &model, Slot::Xml, XML);
    assert!(read(cx, &model, ScheduleModel::already_applied));
}

#[gpui::test]
fn apply_error_is_banner_and_session_kept(cx: &mut TestAppContext) {
    for kind in [
        ScheduleErrorKind::ParseNotImplemented,
        ScheduleErrorKind::MergeNotImplemented,
        ScheduleErrorKind::FilesChanged,
        ScheduleErrorKind::Io,
    ] {
        let backend = FixtureSchedule::ready().with_outcome(Err(err(kind)));
        let (model, _) = start(cx, backend);
        idle(cx, &model);
        apply_confirmed(cx, &model);
        let error = read(cx, &model, |m| m.error().cloned()).expect("a banner");
        assert_eq!(error, ViewError::Schedule(err(kind)));
        assert_eq!(error.copy(), text::error_message(&err(kind)));
        assert!(read(cx, &model, |m| m.result().is_none() && !m.already_applied()));
        assert_eq!(path_of(cx, &model, Slot::Text).as_deref(), Some(TEXT));
        assert_eq!(hint(cx, &model), Some(FocusHint::Outcome));
        // Nothing is announced: the banner is an alert.
        assert!(read(cx, &model, |m| m.announcement().is_none()));
    }
    // The default fixture ends like the real parse stub, and says nothing changed.
    let (model, _) = start(cx, FixtureSchedule::ready());
    idle(cx, &model);
    apply_confirmed(cx, &model);
    let copy = read(cx, &model, |m| m.error().map(ViewError::copy)).expect("banner");
    assert!(copy.contains("Nothing was changed."), "{copy}");
    assert_eq!(
        copy,
        "Reading schedule files is not implemented yet. Nothing was changed."
    );
}

#[gpui::test]
fn session_command_clears_result_and_error(cx: &mut TestAppContext) {
    let (model, fake) = start(cx, FixtureSchedule::ready().with_outcome(Ok(summary(1))));
    idle(cx, &model);
    apply_confirmed(cx, &model);
    assert!(read(cx, &model, |m| m.result().is_some()));
    typed(cx, &model, Slot::Text, "C:\\Schedules\\week-45.txt");
    assert!(read(cx, &model, |m| m.result().is_none()));

    fake.answer_pick(Err(PickError::Failed {
        detail: "x".to_string(),
    }));
    act(cx, &model, |m, cx| m.browse(Slot::Xml, cx));
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.error().is_some()));
    typed(cx, &model, Slot::Text, TEXT);
    assert!(read(cx, &model, |m| m.error().is_none()));
}

// ---- drops ----

#[gpui::test]
fn drop_target_enabled_and_disabled_reasons(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixtureSchedule::ready().with_outcome(Ok(summary(1))));
    let hooks = Hooks {
        model: model.clone(),
    };
    let target = |cx: &mut TestAppContext| cx.read(|cx| hooks.drop_target(cx));
    let disabled = |reason: &str| DropTarget::Disabled {
        reason: reason.to_string().into(),
    };
    assert_eq!(target(cx), disabled("Not ready yet"));
    idle(cx, &model);
    assert_eq!(
        target(cx),
        DropTarget::Enabled {
            label: "Drop a schedule file, an XML file, or both".into()
        }
    );
    act(cx, &model, ScheduleModel::request_apply);
    assert_eq!(target(cx), disabled("Finish or cancel the update first"));
    act(cx, &model, ScheduleModel::confirm);
    assert_eq!(target(cx), disabled("The XML file is being updated"));
    idle(cx, &model);
    assert!(matches!(target(cx), DropTarget::Enabled { .. }));

    // A drop while confirming is ignored.
    act(cx, &model, ScheduleModel::request_apply);
    act(cx, &model, |m, cx| {
        m.set_dropped(vec![PathBuf::from("C:\\x\\other.xml")], cx);
    });
    idle(cx, &model);
    assert_eq!(path_of(cx, &model, Slot::Xml).as_deref(), Some(XML));
}

#[gpui::test]
fn drop_sets_slots_and_announces(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixtureSchedule::empty());
    idle(cx, &model);
    act(cx, &model, |m, cx| {
        m.set_dropped(vec![PathBuf::from(XML), PathBuf::from(TEXT)], cx);
    });
    idle(cx, &model);
    assert_eq!(path_of(cx, &model, Slot::Xml).as_deref(), Some(XML));
    assert_eq!(path_of(cx, &model, Slot::Text).as_deref(), Some(TEXT));
    assert_eq!(
        read(cx, &model, |m| m.announcement().map(String::from)).as_deref(),
        Some("Schedule file: week-44.txt. XML file: schedule.xml.")
    );
    // A problem is named.
    act(cx, &model, |m, cx| {
        m.set_dropped(vec![PathBuf::from("rel.txt")], cx);
    });
    idle(cx, &model);
    assert_eq!(
        read(cx, &model, |m| m.announcement().map(String::from)).as_deref(),
        Some("Schedule file: rel.txt (has a problem). XML file: schedule.xml.")
    );
}

#[gpui::test]
fn ambiguous_drop_is_error(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixtureSchedule::ready());
    idle(cx, &model);
    act(cx, &model, |m, cx| {
        m.set_dropped(
            vec![PathBuf::from("C:\\a.xml"), PathBuf::from("C:\\b.xml")],
            cx,
        );
    });
    idle(cx, &model);
    let error = read(cx, &model, |m| m.error().cloned()).expect("an error");
    let ViewError::Schedule(e) = &error else {
        panic!("{error:?}")
    };
    assert_eq!(e.kind, ScheduleErrorKind::DropAmbiguous);
    assert!(error.copy().starts_with("Drop one schedule file"));
    assert_eq!(
        path_of(cx, &model, Slot::Xml).as_deref(),
        Some(XML),
        "unchanged"
    );
}

// ---- activation ----

#[gpui::test]
fn activated_clears_transient_state(cx: &mut TestAppContext) {
    let (model, fake) = start(cx, FixtureSchedule::ready().with_outcome(Ok(summary(1))));
    idle(cx, &model);
    apply_confirmed(cx, &model);
    // The apply left a focus hint, which is unread.
    fake.answer_pick(Err(PickError::Failed {
        detail: "x".to_string(),
    }));
    act(cx, &model, |m, cx| m.browse(Slot::Xml, cx));
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.result().is_some()
        && m.error().is_some()
        && m.announcement().is_some()
        && m.already_applied()));
    act(cx, &model, ScheduleModel::activated);
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.result().is_none()
        && m.error().is_none()
        && m.announcement().is_none()
        && !m.already_applied()));
    assert_eq!(hint(cx, &model), None, "the hint is forgotten");
    // The session is kept.
    assert_eq!(path_of(cx, &model, Slot::Text).as_deref(), Some(TEXT));

    // An open confirmation closes.
    act(cx, &model, ScheduleModel::request_apply);
    assert!(read(cx, &model, |m| m.pending().is_some()));
    act(cx, &model, ScheduleModel::activated);
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.pending().is_none()));
}

#[gpui::test]
fn activated_during_apply_keeps_the_apply_error(cx: &mut TestAppContext) {
    for panic_apply in [false, true] {
        let (mut spy, _) = Spy::new(FixtureSchedule::ready());
        spy.panic_apply = panic_apply;
        let (model, _) = start_boxed(cx, Box::new(spy));
        idle(cx, &model);
        act(cx, &model, ScheduleModel::request_apply);
        act(cx, &model, ScheduleModel::confirm);
        // The refresh queues behind the apply and lands after it.
        act(cx, &model, ScheduleModel::activated);
        idle(cx, &model);
        let error = read(cx, &model, |m| m.error().cloned()).expect("the apply's error");
        assert_eq!(
            matches!(error, ViewError::Unexpected { .. }),
            panic_apply,
            "{error:?}"
        );
        assert_eq!(hint(cx, &model), Some(FocusHint::Outcome));
    }
}

// ---- the real backend ----

#[expect(clippy::unnecessary_wraps, reason = "the hook's signature")]
fn fake_parse(text: &str) -> Result<Schedule, ParseError> {
    Ok(Schedule {
        columns: vec!["Item".into()],
        rows: text
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| vec![l.to_owned()])
            .collect(),
    })
}

fn fake_merge(schedule: &Schedule, xml: &str) -> Result<MergeOutcome, MergeError> {
    let at = xml
        .rfind("</schedule>")
        .ok_or_else(|| MergeError::InvalidXml("no </schedule>".into()))?;
    let items: String = schedule
        .rows
        .iter()
        .flat_map(|r| ["<item>", r[0].as_str(), "</item>"])
        .collect();
    Ok(MergeOutcome {
        xml: format!("{}{items}{}", &xml[..at], &xml[at..]),
        added: u32::try_from(schedule.rows.len()).expect("small"),
    })
}

#[gpui::test]
fn real_backend_apply_writes_backup_in_temp_dir(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let text_file = dir.path().join("week.txt");
    let xml_file = dir.path().join("schedule.xml");
    fs::write(&text_file, "a\nb\n").expect("write");
    fs::write(&xml_file, "<schedule></schedule>").expect("write");
    let backend = RealSchedule::with_hooks(CoreHooks {
        parse: fake_parse,
        merge: fake_merge,
    });
    let (model, _) = start_boxed(cx, Box::new(backend));
    idle(cx, &model);
    typed(cx, &model, Slot::Text, text_file.to_str().expect("utf-8"));
    typed(cx, &model, Slot::Xml, xml_file.to_str().expect("utf-8"));
    assert!(read(cx, &model, ScheduleModel::can_apply));
    apply_confirmed(cx, &model);

    let result = read(cx, &model, |m| m.result().cloned()).expect("applied");
    assert_eq!(result.added, 2);
    assert_eq!(
        fs::read_to_string(&xml_file).expect("read"),
        "<schedule><item>a</item><item>b</item></schedule>"
    );
    let backup = PathBuf::from(&result.backup_path);
    assert_eq!(backup.parent(), Some(dir.path()));
    assert_eq!(
        fs::read_to_string(backup).expect("backup"),
        "<schedule></schedule>"
    );
    assert!(read(cx, &model, |m| m.error().is_none()));
}

#[gpui::test]
fn real_stub_hooks_change_nothing(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let text_file = dir.path().join("week.txt");
    let xml_file = dir.path().join("schedule.xml");
    fs::write(&text_file, "a").expect("write");
    fs::write(&xml_file, "<schedule/>").expect("write");
    let (model, _) = start_boxed(cx, Box::new(RealSchedule::new()));
    idle(cx, &model);
    typed(cx, &model, Slot::Text, text_file.to_str().expect("utf-8"));
    typed(cx, &model, Slot::Xml, xml_file.to_str().expect("utf-8"));
    apply_confirmed(cx, &model);
    let copy = read(cx, &model, |m| m.error().map(ViewError::copy)).expect("banner");
    assert_eq!(
        copy,
        "Reading schedule files is not implemented yet. Nothing was changed."
    );
    assert_eq!(fs::read_to_string(&xml_file).expect("read"), "<schedule/>");
    assert_eq!(
        fs::read_dir(dir.path()).expect("dir").count(),
        2,
        "no backup"
    );
}

#[gpui::test]
fn dismiss_result_hides_it_and_keeps_the_applied_pair(cx: &mut TestAppContext) {
    let (model, _) = start(cx, FixtureSchedule::ready().with_outcome(Ok(summary(1))));
    idle(cx, &model);
    act(cx, &model, ScheduleModel::dismiss_result);
    apply_confirmed(cx, &model);
    assert!(read(cx, &model, |m| m.result().is_some()));
    act(cx, &model, ScheduleModel::dismiss_result);
    assert!(read(cx, &model, |m| m.result().is_none() && m.already_applied()));
}

#[gpui::test]
fn unexpected_command_failure_refetches_the_session(cx: &mut TestAppContext) {
    let (mut spy, _) = Spy::new(FixtureSchedule::empty());
    spy.panic_set_xml = true;
    let sessions = spy.sessions.clone();
    let (model, _) = start_boxed(cx, Box::new(spy));
    idle(cx, &model);
    assert_eq!(sessions.load(Ordering::SeqCst), 1);
    typed(cx, &model, Slot::Xml, XML);
    let error = read(cx, &model, |m| m.error().cloned()).expect("a banner");
    assert_eq!(error.copy(), text::UNEXPECTED);
    assert!(error.detail().contains("set_xml exploded"));
    // The backend did take the file; the refetch shows it, and the banner stays.
    assert_eq!(sessions.load(Ordering::SeqCst), 2);
    assert_eq!(path_of(cx, &model, Slot::Xml).as_deref(), Some(XML));
    assert!(read(cx, &model, |m| m.error().is_some()));
}

#[gpui::test]
fn unexpected_apply_failure_refetches_the_session(cx: &mut TestAppContext) {
    let (mut spy, _) = Spy::new(FixtureSchedule::ready());
    spy.panic_apply = true;
    let sessions = spy.sessions.clone();
    let (model, _) = start_boxed(cx, Box::new(spy));
    idle(cx, &model);
    apply_confirmed(cx, &model);
    let error = read(cx, &model, |m| m.error().cloned()).expect("a banner");
    assert_eq!(error.copy(), text::UNEXPECTED);
    assert!(error.detail().contains("apply exploded"));
    assert_eq!(sessions.load(Ordering::SeqCst), 2);
    assert!(read(cx, &model, |m| m.result().is_none() && !m.applying()));
}

#[gpui::test]
fn files_changed_on_apply_refetches_the_session(cx: &mut TestAppContext) {
    let (spy, _) =
        Spy::new(FixtureSchedule::ready().with_outcome(Err(err(ScheduleErrorKind::FilesChanged))));
    let sessions = spy.sessions.clone();
    let (model, _) = start_boxed(cx, Box::new(spy));
    idle(cx, &model);
    apply_confirmed(cx, &model);
    assert_eq!(sessions.load(Ordering::SeqCst), 2);
}

// ---- fixtures ----

#[gpui::test]
fn fixtures_build_each_screenshot_state(cx: &mut TestAppContext) {
    let slot_ok = |s: Option<FileSlot>| s.is_some_and(|f| f.error.is_none());
    for (fixture, text_ok, can) in [
        (ScheduleFixture::Empty, false, false),
        (ScheduleFixture::Ready, true, true),
        (ScheduleFixture::Bad, false, false),
    ] {
        let (model, _) = start(cx, FixtureSchedule::from_fixture(fixture));
        idle(cx, &model);
        let s = read(cx, &model, |m| m.session().cloned()).expect("session");
        assert_eq!(slot_ok(s.text.clone()), text_ok, "{fixture:?}");
        assert_eq!(s.can_apply, can, "{fixture:?}");
        // Every fixture's apply ends like the real parse stub.
        let mut backend = FixtureSchedule::from_fixture(fixture);
        let expected = s.text.as_ref().zip(s.xml.as_ref());
        let (t, x) = expected.map_or((TEXT, XML), |(t, x)| (t.path.as_str(), x.path.as_str()));
        if can {
            let kind = backend.apply(t, x).expect_err("a stub").kind;
            assert_eq!(kind, ScheduleErrorKind::ParseNotImplemented, "{fixture:?}");
        }
    }
    let default_kind = |mut b: FixtureSchedule| {
        // Both slots set, so the outcome is what answers.
        b.set_text(Some(PathBuf::from(TEXT)));
        b.set_xml(Some(PathBuf::from(XML)));
        b.apply(TEXT, XML).expect_err("a stub").kind
    };
    for b in [
        FixtureSchedule::empty(),
        FixtureSchedule::ready(),
        FixtureSchedule::bad(),
    ] {
        assert_eq!(default_kind(b), ScheduleErrorKind::ParseNotImplemented);
    }
    // `bad`: the error names the problem.
    let (model, _) = start(cx, FixtureSchedule::bad());
    idle(cx, &model);
    let kind = read(cx, &model, |m| {
        m.session()
            .and_then(|s| s.text.as_ref())
            .and_then(|f| f.error.as_ref())
            .map(|e| e.kind)
    });
    assert_eq!(kind, Some(ScheduleErrorKind::NotAbsolute));
    assert_eq!(
        read(cx, &model, ScheduleModel::apply_reason),
        Some(text::REASON_FIX_PATHS)
    );

    // `confirm` is reached from `ready`, and Update ends in the parse stub's error.
    let (model, _) = start(cx, FixtureSchedule::ready());
    idle(cx, &model);
    act(cx, &model, ScheduleModel::request_apply);
    assert!(read(cx, &model, |m| m.pending().is_some()));
    act(cx, &model, ScheduleModel::confirm);
    idle(cx, &model);
    let kind = read(cx, &model, |m| match m.error() {
        Some(ViewError::Schedule(e)) => Some(e.kind),
        _ => None,
    });
    assert_eq!(kind, Some(ScheduleErrorKind::ParseNotImplemented));
}
