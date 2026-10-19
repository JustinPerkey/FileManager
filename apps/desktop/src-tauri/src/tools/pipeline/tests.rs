use std::fs;
use std::path::Path;

use fm_pipeline::{Issue, Step, StepError, StepFn, StepInput, StepOutcome};
use tempfile::TempDir;

use super::core::{Core, Dir};
use super::types::{PipelineErrorKind as K, StepId, StepIssue, StepReport};

fn dir_with(subdirs: &[&str]) -> TempDir {
    let d = tempfile::tempdir().expect("tempdir");
    for s in subdirs {
        fs::create_dir(d.path().join(s)).unwrap();
    }
    fs::write(d.path().join("loose.txt"), "x").unwrap();
    d
}

/// Passes, with one line per subdirectory seen.
#[expect(clippy::unnecessary_wraps, reason = "must match fm_pipeline::StepFn")]
fn pass(input: StepInput<'_>) -> Result<StepOutcome, StepError> {
    Ok(StepOutcome {
        summary: format!("{} folders checked", input.subdirectories.len()),
        issues: Vec::new(),
    })
}

/// Reports an issue for every subdirectory.
#[expect(clippy::unnecessary_wraps, reason = "must match fm_pipeline::StepFn")]
fn flag_all(input: StepInput<'_>) -> Result<StepOutcome, StepError> {
    Ok(StepOutcome {
        summary: "checked".into(),
        issues: input
            .subdirectories
            .iter()
            .map(|p| Issue {
                subdirectory: Some(p.clone()),
                message: "missing run.bat".into(),
            })
            .collect(),
    })
}

fn broken(_: StepInput<'_>) -> Result<StepOutcome, StepError> {
    Err(StepError::Failed("disk went away".into()))
}

fn fakes(step: Step) -> StepFn {
    match step {
        Step::Verify => pass,
        Step::RunBatch => flag_all,
        Step::Schedule => broken,
        Step::Tarpack => fm_pipeline::tarpack::tarpack,
    }
}

/// Every step passes.
fn all_pass(_: Step) -> StepFn {
    pass
}

/// Verify and `RunBatch` pass; Schedule stops part way.
fn schedule_breaks(step: Step) -> StepFn {
    match step {
        Step::Schedule => broken,
        _ => pass,
    }
}

fn display(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Runs `step` the way the command does, without the worker thread.
fn run(c: &mut Core, step: Step, root: &Path) {
    let job = c.begin(step, &display(root)).expect("begin");
    let finished = job.run();
    c.finish(step, finished);
}

#[test]
fn empty_session_lists_every_step_in_order() {
    let s = Core::default().snapshot();
    assert_eq!(s.directory, None);
    assert!(!s.can_run);
    assert_eq!(s.running, None);
    let ids: Vec<StepId> = s.steps.iter().map(|v| v.id).collect();
    assert_eq!(
        ids,
        [
            StepId::Verify,
            StepId::RunBatch,
            StepId::Schedule,
            StepId::Tarpack
        ]
    );
    assert!(s
        .steps
        .iter()
        .all(|v| v.report.is_none() && v.error.is_none()));
}

#[test]
fn set_directory_counts_subdirectories() {
    let d = dir_with(&["a", "b"]);
    let mut c = Core::default();
    c.set_directory(Some(Dir::check(d.path()))).unwrap();
    let s = c.snapshot();
    let slot = s.directory.unwrap();
    assert_eq!(slot.path, display(d.path()));
    assert_eq!(slot.error, None);
    assert_eq!(slot.subdirectories, 2);
    assert!(s.can_run);
}

#[test]
fn bad_directories_are_kept_with_their_error() {
    let d = dir_with(&[]);
    let mut c = Core::default();
    for (path, kind) in [
        (Path::new("relative").to_path_buf(), K::NotAbsolute),
        (d.path().join("loose.txt"), K::NotADirectory),
        (d.path().join("missing"), K::NotADirectory),
    ] {
        c.set_directory(Some(Dir::check(&path))).unwrap();
        let s = c.snapshot();
        let slot = s.directory.unwrap();
        assert_eq!(slot.path, display(&path));
        assert_eq!(slot.error.map(|e| e.kind), Some(kind));
        assert!(!s.can_run);
    }
    c.set_directory(None).unwrap();
    assert_eq!(c.snapshot().directory, None);
}

#[test]
fn begin_needs_a_usable_matching_directory() {
    let d = dir_with(&[]);
    let mut c = Core::with_hooks(fakes);
    let err = |r: Result<_, super::types::PipelineError>| r.err().map(|e| e.kind);
    assert_eq!(err(c.begin(Step::Verify, "")), Some(K::NoDirectory));
    c.set_directory(Some(Dir::check(&d.path().join("loose.txt"))))
        .unwrap();
    assert_eq!(err(c.begin(Step::Verify, "")), Some(K::NotADirectory));
    c.set_directory(Some(Dir::check(d.path()))).unwrap();
    assert_eq!(
        err(c.begin(Step::Verify, "C:\\elsewhere")),
        Some(K::DirectoryChanged)
    );
    assert_eq!(c.snapshot().running, None);
}

#[test]
fn passing_step_is_recorded() {
    let d = dir_with(&["a", "b", "c"]);
    let mut c = Core::with_hooks(fakes);
    c.set_directory(Some(Dir::check(d.path()))).unwrap();
    run(&mut c, Step::Verify, d.path());
    let s = c.snapshot();
    assert_eq!(
        s.steps[0].report,
        Some(StepReport {
            summary: "3 folders checked".into(),
            issues: vec![],
            passed: true,
        })
    );
    assert_eq!(s.steps[0].error, None);
    assert!(s.can_run);
}

#[test]
fn issues_name_the_subdirectory() {
    let d = dir_with(&["one", "two"]);
    let mut c = Core::with_hooks(fakes);
    c.set_directory(Some(Dir::check(d.path()))).unwrap();
    run(&mut c, Step::Verify, d.path());
    run(&mut c, Step::RunBatch, d.path());
    let report = c.snapshot().steps[1].report.clone().unwrap();
    assert!(!report.passed);
    assert_eq!(
        report.issues,
        ["one", "two"].map(|n| StepIssue {
            subdirectory: Some(n.into()),
            message: "missing run.bat".into(),
        })
    );
}

#[test]
fn step_errors_are_recorded_not_returned() {
    let d = dir_with(&["a"]);
    let mut c = Core::with_hooks(schedule_breaks);
    c.set_directory(Some(Dir::check(d.path()))).unwrap();
    run(&mut c, Step::Verify, d.path());
    run(&mut c, Step::RunBatch, d.path());
    run(&mut c, Step::Schedule, d.path());
    let s = c.snapshot();
    let error = s.steps[2].error.as_ref().unwrap();
    assert_eq!(error.kind, K::StepFailed);
    assert_eq!(error.message, "disk went away");
    assert!(s.steps[2].report.is_none());
}

#[test]
fn one_step_at_a_time_and_no_directory_change_while_running() {
    let d = dir_with(&[]);
    let mut c = Core::with_hooks(fakes);
    c.set_directory(Some(Dir::check(d.path()))).unwrap();
    let job = c.begin(Step::Verify, &display(d.path())).unwrap();
    let s = c.snapshot();
    assert_eq!(s.running, Some(StepId::Verify));
    assert!(!s.can_run);
    assert_eq!(
        c.begin(Step::RunBatch, &display(d.path()))
            .err()
            .map(|e| e.kind),
        Some(K::StepInProgress)
    );
    assert_eq!(
        c.set_directory(None).err().map(|e| e.kind),
        Some(K::StepInProgress)
    );
    c.finish(Step::Verify, job.run());
    assert_eq!(c.snapshot().running, None);
}

#[test]
fn subdirectories_are_listed_when_the_step_runs() {
    let d = dir_with(&["a"]);
    let mut c = Core::with_hooks(fakes);
    c.set_directory(Some(Dir::check(d.path()))).unwrap();
    fs::create_dir(d.path().join("b")).unwrap();
    run(&mut c, Step::Verify, d.path());
    let s = c.snapshot();
    assert_eq!(
        s.steps[0].report.as_ref().unwrap().summary,
        "2 folders checked"
    );
    assert_eq!(s.directory.unwrap().subdirectories, 2);
}

#[test]
fn changing_the_directory_forgets_results() {
    let d = dir_with(&["a"]);
    let mut c = Core::with_hooks(fakes);
    c.set_directory(Some(Dir::check(d.path()))).unwrap();
    run(&mut c, Step::Verify, d.path());
    c.set_directory(Some(Dir::check(d.path()))).unwrap();
    assert!(c.snapshot().steps[0].report.is_none());
}

#[test]
fn default_hooks_are_the_fm_pipeline_stubs() {
    let d = dir_with(&["a"]);
    let mut c = Core::default();
    c.set_directory(Some(Dir::check(d.path()))).unwrap();
    run(&mut c, Step::Verify, d.path());
    let s = c.snapshot();
    assert_eq!(
        s.steps[0].error.as_ref().map(|e| e.kind),
        Some(K::StepNotImplemented)
    );
    assert_eq!(s.next_step, Some(StepId::Verify));
    // The later steps are locked behind Verify; check their hooks directly.
    let subdirectories = [d.path().join("a")];
    let input = StepInput {
        root: d.path(),
        subdirectories: &subdirectories,
    };
    for step in Step::ALL {
        assert_eq!((Step::hook(step))(input), Err(StepError::NotImplemented));
    }
}

#[test]
fn steps_unlock_in_order() {
    let d = dir_with(&["a"]);
    let mut c = Core::with_hooks(all_pass);
    c.set_directory(Some(Dir::check(d.path()))).unwrap();
    let unlocked =
        |c: &Core| -> Vec<bool> { c.snapshot().steps.iter().map(|v| v.unlocked).collect() };
    assert_eq!(unlocked(&c), [true, false, false, false]);
    assert_eq!(c.snapshot().next_step, Some(StepId::Verify));
    assert_eq!(
        c.begin(Step::Schedule, &display(d.path()))
            .err()
            .map(|e| e.kind),
        Some(K::PreviousStepNotPassed)
    );
    assert_eq!(c.snapshot().running, None);

    run(&mut c, Step::Verify, d.path());
    assert_eq!(unlocked(&c), [true, true, false, false]);
    assert_eq!(c.snapshot().next_step, Some(StepId::RunBatch));
    for step in [Step::RunBatch, Step::Schedule, Step::Tarpack] {
        run(&mut c, step, d.path());
    }
    assert_eq!(unlocked(&c), [true; 4]);
    assert_eq!(c.snapshot().next_step, None);
}

#[test]
fn a_step_with_problems_keeps_the_next_locked() {
    let d = dir_with(&["a"]);
    let mut c = Core::with_hooks(fakes);
    c.set_directory(Some(Dir::check(d.path()))).unwrap();
    run(&mut c, Step::Verify, d.path());
    run(&mut c, Step::RunBatch, d.path());
    let s = c.snapshot();
    assert_eq!(s.next_step, Some(StepId::RunBatch));
    assert!(s.steps[1].unlocked);
    assert!(!s.steps[2].unlocked);

    // A refused start changes nothing.
    assert_eq!(
        c.begin(Step::Schedule, &display(d.path()))
            .err()
            .map(|e| e.kind),
        Some(K::PreviousStepNotPassed)
    );
    assert_eq!(c.snapshot(), s);
}

#[test]
fn running_a_step_again_forgets_the_later_results() {
    let d = dir_with(&["a"]);
    let mut c = Core::with_hooks(all_pass);
    c.set_directory(Some(Dir::check(d.path()))).unwrap();
    for step in Step::ALL {
        run(&mut c, step, d.path());
    }
    let job = c.begin(Step::RunBatch, &display(d.path())).unwrap();
    let s = c.snapshot();
    assert!(s.steps[0].report.is_some());
    assert!(s.steps[1..].iter().all(|v| v.report.is_none()));
    c.finish(Step::RunBatch, job.run());
    assert_eq!(c.snapshot().next_step, Some(StepId::Schedule));
}

#[test]
fn a_folder_that_goes_away_during_a_step_blocks_the_next_run() {
    let d = dir_with(&["a"]);
    let root = d.path().join("jobs");
    fs::create_dir(&root).unwrap();
    let mut c = Core::with_hooks(fakes);
    c.set_directory(Some(Dir::check(&root))).unwrap();
    let job = c.begin(Step::Verify, &display(&root)).unwrap();
    fs::remove_dir(&root).unwrap();
    c.finish(Step::Verify, job.run());
    let s = c.snapshot();
    assert_eq!(
        s.directory.unwrap().error.map(|e| e.kind),
        Some(K::NotADirectory)
    );
    assert!(!s.can_run);
}
