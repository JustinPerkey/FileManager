//! The Tar Packager model's tests: the copy, loading and restoring, opening,
//! reloading, the watcher, the manifest's errors, assignment by browse, clear
//! and drop, the output and format, building with progress, the confirmed
//! overwrite, the result, reveal and copy, the shortcuts, and the fixtures.
//! Model tests run
//! against [`FixtureTarpack`] and [`FakePlatform`]; those that need the disk
//! run [`RealTarpack`] over a temporary directory.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use fm_core::AppDirs;
use fm_tarpack::archive::{BuildPhase, BuildSummary, Progress};
use fm_tarpack::format::ArchiveFormat;
use fm_tarpack::manifest::{Diagnostic, EntryFailure, EntryView, Severity};
use fm_tarpack::session::{SessionEntry, TarpackError, TarpackErrorKind};
use fm_tarpack::sources::{
    Ambiguity, DropOutcome, EntryStatus, Unmatched, UnmatchedReason as Reason,
};
use gpui::{AppContext as _, Context, Entity, TestAppContext};

use super::backend::{RealTarpack, TarpackBackend};
use super::fixtures::{
    gateway_with_errors, gateway_withheld, Call, Command, FixtureTarpack, Gate, TarpackFixture,
    GATEWAY, MANIFEST_PATH, OUTPUT_PATH,
};
use super::model::{
    default_start_dir, BuildOutcome, DropState, FocusHint, Origin, Shortcut, TarpackModel,
    ViewError, LOOKUP_THREADS, SKELETON_DELAY,
};
use super::text::{self, DropBlocker, DropLine};
use super::Hooks;
use crate::app::dev_mode::DevMode;
use crate::app::drop::DropTarget;
use crate::app::text::{DROP_BLOCKED_BY_DIALOG, PICK_FAILED};
use crate::platform::fake::{FakePlatform, PlatformCall};
use crate::platform::{self, FileFilter, OpenError, OpenSpec, PickError, PickKind, SaveSpec};
use crate::tools::ToolHooks as _;

const ALL_KINDS: [TarpackErrorKind; 17] = [
    TarpackErrorKind::NoManifest,
    TarpackErrorKind::ManifestUnreadable,
    TarpackErrorKind::NoEntries,
    TarpackErrorKind::ManifestChangedOnDisk,
    TarpackErrorKind::UnknownEntry,
    TarpackErrorKind::NotAFile,
    TarpackErrorKind::NoOutput,
    TarpackErrorKind::EntriesNotReady,
    TarpackErrorKind::OutputExists,
    TarpackErrorKind::PathExists,
    TarpackErrorKind::SourceMissing,
    TarpackErrorKind::SourceUnreadable,
    TarpackErrorKind::SourceChanged,
    TarpackErrorKind::VerifyFailed,
    TarpackErrorKind::BuildInProgress,
    TarpackErrorKind::OpenerFailed,
    TarpackErrorKind::Io,
];

/// The kinds whose copy starts with the entry's file name.
const ENTRY_KINDS: [TarpackErrorKind; 5] = [
    TarpackErrorKind::NotAFile,
    TarpackErrorKind::EntriesNotReady,
    TarpackErrorKind::SourceMissing,
    TarpackErrorKind::SourceUnreadable,
    TarpackErrorKind::SourceChanged,
];

const ERROR_MESSAGES_TS: &str =
    include_str!("../../../../desktop/src/tools/tarpack/errorMessages.ts");
const DROP_RESULT_TSX: &str = include_str!("../../../../desktop/src/tools/tarpack/DropResult.tsx");
const MANIFEST_ERRORS_TSX: &str =
    include_str!("../../../../desktop/src/tools/tarpack/ManifestErrors.tsx");
const MANIFEST_HEADER_TSX: &str =
    include_str!("../../../../desktop/src/tools/tarpack/ManifestHeader.tsx");
const TARPACK_VIEW_TSX: &str =
    include_str!("../../../../desktop/src/tools/tarpack/TarpackView.tsx");

fn err(kind: TarpackErrorKind) -> TarpackError {
    TarpackError::new(kind, "detail")
}

fn entry_error(kind: TarpackErrorKind, id: &str) -> TarpackError {
    TarpackError::for_entry(kind, "detail", id)
}

fn entry(id: &str) -> SessionEntry {
    SessionEntry {
        view: EntryView {
            id: id.to_string(),
            source: format!("{id}.bin"),
            target_path: format!("/opt/{id}"),
            mode: "0755".to_string(),
            mode_text: "rwxr-xr-x".to_string(),
            owner: "root:root".to_string(),
            uid: 0,
            gid: 0,
            normalize_eol: false,
        },
        assigned: None,
        status: EntryStatus::Unassigned,
    }
}

// ---- helpers ----

fn start_boxed(
    cx: &mut TestAppContext,
    backend: Box<dyn TarpackBackend>,
) -> (Entity<TarpackModel>, FakePlatform) {
    cx.executor().allow_parking();
    let fake = FakePlatform::new();
    let installed = fake.clone();
    cx.update(|cx| platform::set_platform(cx, installed));
    let model = cx.new(|cx| TarpackModel::new(backend, cx));
    (model, fake)
}

fn start(
    cx: &mut TestAppContext,
    backend: &FixtureTarpack,
) -> (Entity<TarpackModel>, FakePlatform) {
    start_boxed(cx, Box::new(backend.clone()))
}

fn read<R>(
    cx: &mut TestAppContext,
    model: &Entity<TarpackModel>,
    f: impl FnOnce(&TarpackModel) -> R,
) -> R {
    model.read_with(cx, |m, _| f(m))
}

fn act<R>(
    cx: &mut TestAppContext,
    model: &Entity<TarpackModel>,
    f: impl FnOnce(&mut TarpackModel, &mut Context<TarpackModel>) -> R,
) -> R {
    model.update(cx, f)
}

/// Runs everything queued, polling until `done`.
fn wait_for(cx: &mut TestAppContext, mut done: impl FnMut(&mut TestAppContext) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        cx.run_until_parked();
        if done(cx) {
            cx.run_until_parked();
            return;
        }
        assert!(Instant::now() < deadline, "the condition never held");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Runs everything queued until the platform has been asked `n` times. A
/// dialog that starts in the default folder asks only after a lookup on a
/// thread of its own.
fn wait_for_calls(cx: &mut TestAppContext, fake: &FakePlatform, n: usize) {
    wait_for(cx, |_| fake.calls().len() >= n);
}

/// Runs everything queued until nothing loads and no drop is pending.
fn idle(cx: &mut TestAppContext, model: &Entity<TarpackModel>) {
    wait_for(cx, |cx| {
        read(cx, model, |m| {
            !m.loading() && !matches!(m.drop_state(), Some(DropState::Pending))
        })
    });
}

fn error_of(cx: &mut TestAppContext, model: &Entity<TarpackModel>) -> Option<ViewError> {
    read(cx, model, |m| m.error().cloned())
}

fn say(cx: &mut TestAppContext, model: &Entity<TarpackModel>) -> Option<String> {
    read(cx, model, |m| m.announcement().map(String::from))
}

fn generation(cx: &mut TestAppContext, model: &Entity<TarpackModel>) -> u64 {
    read(cx, model, TarpackModel::announcement_generation)
}

fn hint(cx: &mut TestAppContext, model: &Entity<TarpackModel>) -> Option<FocusHint> {
    act(cx, model, |m, _| m.take_focus_hint())
}

fn manifest_path(cx: &mut TestAppContext, model: &Entity<TarpackModel>) -> Option<String> {
    read(cx, model, |m| {
        m.session()?.manifest.as_ref().map(|m| m.path.clone())
    })
}

fn entries(cx: &mut TestAppContext, model: &Entity<TarpackModel>) -> Vec<SessionEntry> {
    read(cx, model, |m| {
        m.session()
            .and_then(|s| s.manifest.as_ref())
            .map(|m| m.entries.clone())
            .unwrap_or_default()
    })
}

fn status_of(
    cx: &mut TestAppContext,
    model: &Entity<TarpackModel>,
    id: &str,
) -> Option<(Option<String>, EntryStatus)> {
    entries(cx, model)
        .into_iter()
        .find(|e| e.view.id == id)
        .map(|e| (e.assigned, e.status))
}

fn drop_lines_of(cx: &mut TestAppContext, model: &Entity<TarpackModel>) -> Option<Vec<DropLine>> {
    read(cx, model, |m| match m.drop_state() {
        Some(DropState::Result(r)) => Some(r.lines.clone()),
        _ => None,
    })
}

fn dropped(cx: &mut TestAppContext, model: &Entity<TarpackModel>, paths: &[&str]) {
    let paths = paths.iter().map(PathBuf::from).collect();
    act(cx, model, |m, cx| m.dropped(paths, cx));
}

#[expect(clippy::unnecessary_wraps, reason = "the shape of a dialog's answer")]
fn pick(path: &Path) -> platform::PickResult {
    Ok(Some(path.to_path_buf()))
}

fn manifest_filter() -> FileFilter {
    FileFilter {
        name: "Manifest".to_string(),
        patterns: vec!["*.toml".to_string()],
    }
}

/// A real backend whose memory lives in `dir/app`.
fn real(dir: &Path) -> RealTarpack {
    RealTarpack::with_dirs(&AppDirs::at(dir.join("app")))
}

fn write_manifest(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, text).expect("write");
    path
}

fn display(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

// ---- copy ----

#[test]
fn text_covers_every_error_kind() {
    let mut seen = Vec::new();
    for kind in ALL_KINDS {
        let message = text::error_message(kind, None);
        assert_ne!(message, "");
        assert!(!seen.contains(&message), "{kind:?} repeats a message");
        seen.push(message);
    }
    for kind in ENTRY_KINDS {
        let named = text::error_message(kind, Some("gateway.conf"));
        assert!(named.starts_with("gateway.conf"), "{kind:?}: {named}");
        let unnamed = text::error_message(kind, None);
        assert!(unnamed.starts_with("A file"), "{kind:?}: {unnamed}");
    }
    // The source comes from the entry the error names.
    let entries = [entry("a"), entry("b")];
    let named = entry_error(TarpackErrorKind::SourceMissing, "b");
    assert_eq!(
        text::error_copy(&named, &entries),
        "b.bin is no longer at its assigned location."
    );
    let unknown = entry_error(TarpackErrorKind::SourceMissing, "ghost");
    assert_eq!(
        text::error_copy(&unknown, &entries),
        "A file is no longer at its assigned location."
    );
    assert_eq!(
        text::error_copy(&err(TarpackErrorKind::SourceMissing), &entries),
        "A file is no longer at its assigned location."
    );
}

/// `s` as it is written in a TypeScript string literal.
fn ts(s: &str) -> String {
    s.replace('\u{2014}', "\\u2014")
        .replace('\u{2026}', "\\u2026")
}

#[test]
#[expect(clippy::too_many_lines, reason = "one list per TypeScript source")]
fn text_matches_ts_sources() {
    // errorMessages.ts: every kind's message, entry-specific ones through
    // the `file(s)` placeholder.
    for kind in ALL_KINDS {
        let message = text::error_message(kind, None);
        let needle = if ENTRY_KINDS.contains(&kind) {
            let rest = message.strip_prefix("A file").expect("starts with A file");
            format!("${{file(s)}}{rest}")
        } else {
            message
        };
        assert!(
            ERROR_MESSAGES_TS.contains(&needle),
            "{kind:?}: `{needle}` is not in errorMessages.ts"
        );
    }
    let in_view = |s: &str| {
        assert!(
            TARPACK_VIEW_TSX.contains(&ts(s)) || TARPACK_VIEW_TSX.contains(s),
            "`{s}` is not in TarpackView.tsx"
        );
    };
    for s in [
        text::CHANGED_ON_DISK,
        text::MATCHING,
        text::REASON_NO_MANIFEST,
        text::REASON_FIX_ERRORS,
        text::REASON_NO_FILES,
        text::REASON_BUILDING,
        text::REASON_PENDING,
        text::EMPTY_LEAD,
        text::EMPTY_HINT,
        text::OPEN_MANIFEST,
        text::CREATE_FROM_EXAMPLE,
        text::DROP_LABEL,
        text::LOADING,
        text::CHANGED_ACTION,
        text::DETAILS,
        "No files can be built until the manifest errors are fixed.",
        "be left out of the archive.",
        "reloaded. No errors.",
    ] {
        in_view(s);
    }
    for s in [text::OPEN, text::RECENT, text::EDIT_IN_EDITOR] {
        assert!(MANIFEST_HEADER_TSX.contains(s), "`{s}`");
    }
    assert!(MANIFEST_HEADER_TSX.contains(text::RECENT_MENU_LABEL));
    assert!(MANIFEST_HEADER_TSX.contains("Reload"));
    for s in [
        text::WHOLE_MANIFEST,
        text::WHOLE_MANIFEST_ERRORS,
        text::FILES_WITH_ERRORS,
        text::WARNINGS_LABEL,
        "Hide errors",
        "Show errors",
        "No files can be listed or built until the manifest errors are fixed.",
        "1 file is left out of the list and the archive until it's fixed.",
        " files are left out of the list and the archive until they're fixed.",
        "Every file is listed and can still be built.",
        " in this manifest",
    ] {
        assert!(
            MANIFEST_ERRORS_TSX.contains(s),
            "`{s}` is not in ManifestErrors.tsx"
        );
    }
    // DropResult.tsx: every phrase of every reason, singular and plural,
    // the hint, the empty line and the disclosures.
    for reason in [
        Reason::NoEntry,
        Reason::AlreadyAssigned,
        Reason::FolderNoMatch,
        Reason::LinkNotFollowed,
        Reason::NotFound,
        Reason::Unreadable,
        Reason::NotUnicode,
    ] {
        for n in [1, 2] {
            for failed in [false, true] {
                let outcome = DropOutcome {
                    unmatched: (0..n)
                        .map(|i| Unmatched {
                            path: PathBuf::from(format!("C:\\d\\f{i}")),
                            reason,
                        })
                        .collect(),
                    ..DropOutcome::default()
                };
                let lines = text::drop_lines(&outcome, &[], failed);
                let phrase = lines[0].lead.trim_end_matches(':');
                assert!(
                    DROP_RESULT_TSX.contains(&ts(phrase)),
                    "{reason:?}: `{phrase}` is not in DropResult.tsx"
                );
            }
        }
    }
    for s in [
        "Files for entries with errors can't be matched until those errors are fixed.",
        "Nothing was matched",
        "ambiguous, not assigned \u{2014} use Browse\u{2026} on ",
        text::FULL_PATHS,
        text::DROP_RESULT_LABEL,
        text::DISMISS_DROP_RESULT,
        "fits more than one entry",
        "Matching files:",
    ] {
        assert!(
            DROP_RESULT_TSX.contains(&ts(s)),
            "`{s}` is not in DropResult.tsx"
        );
    }
}

fn lines_text(lines: &[DropLine]) -> Vec<String> {
    lines.iter().map(text::line_text).collect()
}

fn outcome_with(unmatched: &[(&str, Reason)]) -> DropOutcome {
    DropOutcome {
        unmatched: unmatched
            .iter()
            .map(|(p, reason)| Unmatched {
                path: PathBuf::from(p),
                reason: *reason,
            })
            .collect(),
        ..DropOutcome::default()
    }
}

#[test]
#[expect(clippy::too_many_lines, reason = "one case per DropResult test")]
fn drop_lines_match_today() {
    let entries = [entry("app"), entry("core"), entry("other")];
    let lines = |o: &DropOutcome, failed: bool| text::drop_lines(o, &entries, failed);

    // matched
    let matched = DropOutcome {
        matched: vec![
            ("a".to_string(), PathBuf::from("C:\\x\\a.dll")),
            ("b".to_string(), PathBuf::from("C:\\x\\b.dll")),
            ("c".to_string(), PathBuf::from("C:\\x\\c")),
        ],
        ..DropOutcome::default()
    };
    let l = lines(&matched, false);
    assert_eq!(lines_text(&l), ["3 matched."]);
    assert_eq!(l[0].tone, text::Tone::Ok);
    assert_eq!(text::drop_result_text(&l), "3 matched.");

    // every reason: copy, tone, and the full path as the item's tooltip
    let cases = [
        (Reason::NoEntry, "1 not in the manifest: notes.txt.", text::Tone::Muted),
        (
            Reason::AlreadyAssigned,
            "1 already assigned, left unchanged: notes.txt.",
            text::Tone::Muted,
        ),
        (
            Reason::FolderNoMatch,
            "1 folder with nothing to match: notes.txt.",
            text::Tone::Muted,
        ),
        (Reason::LinkNotFollowed, "1 link not followed: notes.txt.", text::Tone::Muted),
        (Reason::NotFound, "1 no longer found: notes.txt.", text::Tone::Warn),
        (Reason::Unreadable, "1 could not be read: notes.txt.", text::Tone::Warn),
        (
            Reason::NotUnicode,
            "1 not assigned, unsupported characters in its path \u{2014} rename it or its folder: notes.txt.",
            text::Tone::Warn,
        ),
    ];
    for (reason, expected, tone) in cases {
        let l = lines(&outcome_with(&[("C:\\d\\notes.txt", reason)]), false);
        assert_eq!(lines_text(&l), [expected], "{reason:?}");
        assert_eq!(l[0].tone, tone);
        assert_eq!(l[0].items[0].title, "C:\\d\\notes.txt");
    }

    // plural phrases, and two items with one reason share a line
    let l = lines(
        &outcome_with(&[
            ("C:\\a", Reason::FolderNoMatch),
            ("C:\\b", Reason::FolderNoMatch),
            ("C:\\a", Reason::LinkNotFollowed),
            ("C:\\b", Reason::LinkNotFollowed),
        ]),
        false,
    );
    assert_eq!(
        lines_text(&l),
        [
            "2 folders with nothing to match: a, b.",
            "2 links not followed: a, b."
        ]
    );
    let l = lines(
        &outcome_with(&[
            ("C:\\x\\app.dll", Reason::AlreadyAssigned),
            ("C:\\x\\core.dll", Reason::AlreadyAssigned),
        ]),
        false,
    );
    assert_eq!(
        lines_text(&l),
        ["2 already assigned, left unchanged: app.dll, core.dll."]
    );

    // ambiguous: one entry with two candidates
    let two = |id: &str, c: &[&str]| Ambiguity {
        id: id.to_string(),
        candidates: c.iter().map(PathBuf::from).collect(),
    };
    let o = DropOutcome {
        ambiguous: vec![two("app", &["C:\\a\\app.dll", "C:\\b\\app.dll"])],
        ..DropOutcome::default()
    };
    let l = lines(&o, false);
    assert_eq!(
        lines_text(&l),
        ["1 ambiguous, not assigned \u{2014} use Browse\u{2026} on its row: /opt/app (2 files)."]
    );
    assert!(l[0].items[0].mono);
    assert_eq!(
        l[0].items[0].title,
        "Matching files:\nC:\\a\\app.dll\nC:\\b\\app.dll"
    );
    // two entries sharing one candidate
    let o = DropOutcome {
        ambiguous: vec![
            two("app", &["C:\\x\\app.dll"]),
            two("core", &["C:\\x\\app.dll"]),
        ],
        ..DropOutcome::default()
    };
    assert_eq!(
        lines_text(&lines(&o, false)),
        ["2 ambiguous, not assigned \u{2014} use Browse\u{2026} on their rows: /opt/app (app.dll fits more than one entry), /opt/core (app.dll fits more than one entry)."]
    );
    // an id not in the entries falls back to the file name, never the id
    let o = DropOutcome {
        ambiguous: vec![two("ghost", &["C:\\x\\app.dll"])],
        ..DropOutcome::default()
    };
    let l = lines(&o, false);
    let said = lines_text(&l);
    assert!(said[0].contains("on its row: app.dll (app.dll fits more than one entry)"));
    assert!(!said[0].contains("ghost"));
    assert!(!l[0].items[0].mono);

    // notUnicode plural; U+FFFD paths
    let l = lines(
        &outcome_with(&[
            ("C:\\a\\x.pdf", Reason::NotUnicode),
            ("C:\\a\\y.pdf", Reason::NotUnicode),
        ]),
        false,
    );
    assert_eq!(
        lines_text(&l),
        ["2 not assigned, unsupported characters in their paths \u{2014} rename them or their folders: x.pdf, y.pdf."]
    );
    let l = lines(
        &outcome_with(&[("C:\\d\\bad\u{FFFD}name.txt", Reason::NoEntry)]),
        false,
    );
    assert_eq!(
        lines_text(&l),
        ["1 not in the manifest: bad\u{FFFD}name.txt."]
    );

    // more than 8 names: 8 shown, the full count, the same in the text
    let nine: Vec<(String, Reason)> = (0..9)
        .map(|i| (format!("C:\\f{i}"), Reason::NoEntry))
        .collect();
    let nine: Vec<(&str, Reason)> = nine.iter().map(|(p, r)| (p.as_str(), *r)).collect();
    let o = outcome_with(&nine);
    let l = lines(&o, false);
    let capped = "9 not in the manifest: f0, f1, f2, f3, f4, f5, f6, f7, and 1 more.";
    assert_eq!(lines_text(&l), [capped]);
    assert_eq!(text::drop_result_text(&l), capped);
    assert_eq!(l[0].items.len(), text::MAX_NAMES);
    assert_eq!(l[0].more, 1);

    // failed entries: noEntry reads "not matched" and the hint follows,
    // also after folderNoMatch alone, not after notFound or without failures
    let hint = "Files for entries with errors can't be matched until those errors are fixed.";
    assert_eq!(
        lines_text(&lines(
            &outcome_with(&[("C:\\notes.txt", Reason::NoEntry)]),
            true
        )),
        ["1 not matched: notes.txt.", hint]
    );
    assert_eq!(
        lines(&outcome_with(&[("C:\\dir", Reason::FolderNoMatch)]), true).len(),
        2
    );
    assert_eq!(
        lines_text(&lines(&outcome_with(&[("C:\\x", Reason::NotFound)]), true)),
        ["1 no longer found: x."]
    );
    assert_eq!(
        lines_text(&lines(
            &outcome_with(&[("C:\\notes.txt", Reason::NoEntry)]),
            false
        )),
        ["1 not in the manifest: notes.txt."]
    );

    // combined: order, and the text is the lines joined
    let combined = DropOutcome {
        matched: vec![("a".to_string(), PathBuf::from("C:\\a"))],
        unmatched: outcome_with(&[
            ("C:\\x\\notes.txt", Reason::NoEntry),
            ("C:\\x\\gone.txt", Reason::NotFound),
            ("C:\\x\\core.dll", Reason::AlreadyAssigned),
        ])
        .unmatched,
        ambiguous: vec![two("app", &["C:\\a\\app.dll", "C:\\b\\app.dll"])],
    };
    let l = lines(&combined, true);
    let heads: Vec<String> = lines_text(&l)
        .iter()
        .map(|t| t.split(':').next().unwrap_or_default().to_string())
        .collect();
    assert_eq!(
        heads,
        [
            "1 matched.",
            "1 not matched",
            "1 already assigned, left unchanged",
            "1 no longer found",
            "Files for entries with errors can't be matched until those errors are fixed.",
            "1 ambiguous, not assigned \u{2014} use Browse\u{2026} on its row",
        ]
    );
    assert_eq!(text::drop_result_text(&l), lines_text(&l).join(" "));

    // nothing in it
    let l = lines(&DropOutcome::default(), false);
    assert_eq!(lines_text(&l), ["Nothing was matched."]);
    assert_eq!(l[0].tone, text::Tone::Muted);
    assert!(l[0].full.is_none());

    // Full paths: every path past the cap; none for matched-only
    let mut many = nine.clone();
    many.push(("C:\\dir\\last", Reason::NoEntry));
    let o = DropOutcome {
        matched: vec![("app".to_string(), PathBuf::from("C:\\m\\matched-only.txt"))],
        unmatched: outcome_with(&many).unmatched,
        ambiguous: vec![two("app", &["C:\\a\\app.dll", "C:\\b\\app.dll"])],
    };
    let l = lines(&o, false);
    assert!(l[0].full.is_none(), "matched-only has no disclosure");
    match l[1].full.as_ref().expect("paths") {
        text::FullPaths::Paths { lead, paths } => {
            assert_eq!(lead, "10 not in the manifest");
            assert_eq!(paths.len(), 10);
            assert_eq!(paths[9], "C:\\dir\\last");
        }
        other @ text::FullPaths::Entries { .. } => panic!("{other:?}"),
    }
    match l[2].full.as_ref().expect("entries") {
        text::FullPaths::Entries { lead, entries } => {
            assert_eq!(
                lead,
                "1 ambiguous, not assigned \u{2014} use Browse\u{2026} on its row"
            );
            assert_eq!(entries[0].label, "/opt/app");
            assert_eq!(entries[0].candidates, ["C:\\a\\app.dll", "C:\\b\\app.dll"]);
        }
        other @ text::FullPaths::Paths { .. } => panic!("{other:?}"),
    }
    assert!(!text::drop_result_text(&l).contains("C:\\dir\\last"));
}

#[test]
fn manifest_copy_matches_today() {
    let session = FixtureTarpack::from_fixture(TarpackFixture::Errors).session();
    let manifest = session.manifest.as_ref().expect("manifest");
    assert_eq!(text::errors_heading(1), "1 error in this manifest");
    assert_eq!(text::errors_heading(2), "2 errors in this manifest");
    assert_eq!(
        text::consequence(manifest),
        "1 file is left out of the list and the archive until it's fixed."
    );
    assert_eq!(text::warnings_summary(1), "1 warning");
    assert_eq!(text::warnings_summary(3), "3 warnings");
    let failure = &manifest.failed_entries[0];
    assert_eq!(text::failure_name(failure), "gateway-ca");
    assert_eq!(text::failure_errors_label(failure), "Errors in gateway-ca");
    let diag = &failure.errors[0];
    assert_eq!(
        text::diagnostic_position(diag),
        format!("{}:{}", diag.line, diag.col)
    );
    assert_eq!(
        text::diagnostic_position_spoken(diag),
        format!("Line {}, column {}: ", diag.line, diag.col)
    );
    let mut nameless = failure.clone();
    nameless.id = None;
    nameless.index = 4;
    assert_eq!(text::failure_name(&nameless), "Entry #4");
    assert_eq!(text::errors_toggle(true), "Hide errors");
    assert_eq!(text::errors_toggle(false), "Show errors");

    let withheld = FixtureTarpack::from_fixture(TarpackFixture::Withheld).session();
    assert_eq!(
        text::consequence(withheld.manifest.as_ref().expect("manifest")),
        "No files can be listed or built until the manifest errors are fixed."
    );
    let loaded = FixtureTarpack::from_fixture(TarpackFixture::Loaded).session();
    assert_eq!(
        text::consequence(loaded.manifest.as_ref().expect("manifest")),
        "Every file is listed and can still be built."
    );
}

#[test]
fn manifest_sentences_match_today() {
    let errors = FixtureTarpack::from_fixture(TarpackFixture::Errors).session();
    let withheld = FixtureTarpack::from_fixture(TarpackFixture::Withheld).session();
    let clean = FixtureTarpack::from_fixture(TarpackFixture::Loaded).session();
    let say = |s, prev, origin| text::manifest_sentence(s, prev, origin);
    assert_eq!(
        say(&errors, 0, Origin::Load).as_deref(),
        Some("Gateway deploy has 2 errors. 1 file will be left out of the archive.")
    );
    assert_eq!(
        say(&withheld, 0, Origin::Reload).as_deref(),
        Some("Gateway deploy has 1 error. No files can be built until the manifest errors are fixed.")
    );
    assert_eq!(say(&errors, 0, Origin::Other), None);
    assert_eq!(say(&clean, 0, Origin::Load), None);
    assert_eq!(say(&clean, 0, Origin::Reload), None);
    assert_eq!(
        say(&clean, 2, Origin::Reload).as_deref(),
        Some("Gateway deploy reloaded. No errors.")
    );
    assert_eq!(say(&clean, 2, Origin::Load), None);
}

// ---- loading ----

#[gpui::test]
fn loading_is_observable_with_closed_gate(cx: &mut TestAppContext) {
    let handle = cx.update(crate::jobs::WorkerGate::install_closed);
    let (model, _) = start(cx, &FixtureTarpack::loaded());
    cx.run_until_parked();
    assert!(read(cx, &model, TarpackModel::loading));
    cx.run_until_parked();
    assert!(read(cx, &model, |m| m.loading() && m.session().is_none()));
    handle.open();
    cx.run_until_parked();
    assert!(read(cx, &model, |m| !m.loading() && !m.load_failed()));
}

#[gpui::test]
fn restores_last_manifest_on_start(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let m = write_manifest(dir.path(), "m.toml", GATEWAY);
    real(dir.path()).open(&m).expect("open");
    // A new session restores it.
    let (model, _) = start_boxed(cx, Box::new(real(dir.path())));
    assert!(read(cx, &model, TarpackModel::loading));
    idle(cx, &model);
    assert_eq!(manifest_path(cx, &model), Some(display(&m)));
    assert_eq!(read(cx, &model, |m| m.recent().to_vec()), [display(&m)]);
    assert!(read(cx, &model, |m| !m.load_failed() && m.error().is_none()));
}

#[gpui::test]
fn skeleton_only_after_150ms(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    cx.update(|cx| platform::set_platform(cx, FakePlatform::new()));
    // A load still waiting for its answer: the timer is armed, nothing is
    // fetched.
    let model = cx.new(|cx| {
        let m = TarpackModel::unstarted(Box::new(FixtureTarpack::loaded()), cx);
        TarpackModel::arm_skeleton(cx);
        m
    });
    assert!(read(cx, &model, |m| m.loading() && !m.show_skeleton()));
    assert_eq!(SKELETON_DELAY, Duration::from_millis(150));
    cx.executor().advance_clock(Duration::from_millis(149));
    assert!(read(cx, &model, |m| m.loading() && !m.show_skeleton()));
    cx.executor().advance_clock(Duration::from_millis(1));
    assert!(read(cx, &model, |m| m.loading() && m.show_skeleton()));
    // The answer ends loading, and the skeleton with it.
    act(cx, &model, |m, cx| m.fetch(Origin::Load, true, cx));
    idle(cx, &model);
    assert!(read(cx, &model, |m| !m.loading() && !m.show_skeleton()));

    // A load that answers at once never shows it.
    let (quick, _) = start(cx, &FixtureTarpack::loaded());
    idle(cx, &quick);
    assert!(read(cx, &quick, |m| !m.show_skeleton()));
}

#[gpui::test]
fn restore_failure_sets_load_failed(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::loaded();
    backend.panic_on_session(1);
    let (model, fake) = start(cx, &backend);
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.load_failed() && !m.loading()));
    assert!(read(cx, &model, |m| m.session().is_none()));
    let error = error_of(cx, &model).expect("a banner");
    assert!(matches!(error, ViewError::Unexpected { .. }));
    assert_eq!(error.copy(), text::UNEXPECTED);
    assert_eq!(error.detail(), "session exploded");
    // The tool is still usable: opening a manifest loads it.
    fake.answer_pick(pick(Path::new("C:\\m\\manifest.toml")));
    act(cx, &model, TarpackModel::open);
    idle(cx, &model);
    assert!(read(cx, &model, |m| !m.load_failed() && m.session().is_some()));
    assert!(error_of(cx, &model).is_none());
}

// ---- opening ----

#[gpui::test]
fn open_uses_platform_and_announces(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::empty().with_text(&gateway_with_errors());
    let (model, fake) = start(cx, &backend);
    idle(cx, &model);
    assert_eq!(manifest_path(cx, &model), None);
    fake.answer_pick(pick(Path::new("C:\\Packages\\gateway\\manifest.toml")));
    act(cx, &model, TarpackModel::open);
    idle(cx, &model);
    assert_eq!(
        manifest_path(cx, &model).as_deref(),
        Some("C:\\Packages\\gateway\\manifest.toml")
    );
    assert_eq!(
        say(cx, &model).as_deref(),
        Some("Gateway deploy has 2 errors. 1 file will be left out of the archive.")
    );
    assert!(backend.calls().contains(&Call::Open(PathBuf::from(
        "C:\\Packages\\gateway\\manifest.toml"
    ))));
    assert_eq!(
        read(cx, &model, |m| m.recent().to_vec()),
        ["C:\\Packages\\gateway\\manifest.toml"]
    );
}

#[gpui::test]
fn open_cancel_does_nothing(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::empty();
    let (model, _) = start(cx, &backend);
    idle(cx, &model);
    act(cx, &model, TarpackModel::open);
    idle(cx, &model);
    assert_eq!(manifest_path(cx, &model), None);
    assert!(error_of(cx, &model).is_none());
    assert!(!backend.calls().iter().any(|c| matches!(c, Call::Open(_))));
}

#[gpui::test]
fn open_passes_manifest_filter(cx: &mut TestAppContext) {
    let (model, fake) = start(cx, &FixtureTarpack::empty());
    idle(cx, &model);
    act(cx, &model, TarpackModel::open);
    idle(cx, &model);
    assert_eq!(
        fake.calls(),
        [PlatformCall::Pick(OpenSpec {
            kind: PickKind::File,
            filters: vec![manifest_filter()],
            start_dir: None,
            title: None,
        })]
    );
}

#[gpui::test]
fn open_error_shows_dialog_failure_copy(cx: &mut TestAppContext) {
    let (model, fake) = start(cx, &FixtureTarpack::empty());
    idle(cx, &model);
    fake.answer_pick(Err(PickError::Failed {
        detail: "no shell".to_string(),
    }));
    act(cx, &model, TarpackModel::open);
    idle(cx, &model);
    let error = error_of(cx, &model).expect("a banner");
    assert_eq!(error.copy(), PICK_FAILED);
    assert_eq!(error.detail(), "no shell");
    fake.answer_save(Err(PickError::Failed {
        detail: "again".to_string(),
    }));
    let before = read(cx, &model, TarpackModel::error_generation);
    act(cx, &model, TarpackModel::create_from_example);
    wait_for_calls(cx, &fake, 2);
    idle(cx, &model);
    assert_eq!(read(cx, &model, TarpackModel::error_generation), before + 1);
    assert_eq!(error_of(cx, &model).expect("banner").detail(), "again");
    act(cx, &model, TarpackModel::dismiss_error);
    assert!(error_of(cx, &model).is_none());
}

#[gpui::test]
fn open_recent_uses_stored_path(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let a = write_manifest(dir.path(), "a.toml", GATEWAY);
    let b = write_manifest(dir.path(), "b.toml", GATEWAY);
    let (model, fake) = start_boxed(cx, Box::new(real(dir.path())));
    idle(cx, &model);
    for p in [&a, &b] {
        fake.answer_pick(pick(p));
        act(cx, &model, TarpackModel::open);
        idle(cx, &model);
    }
    assert_eq!(manifest_path(cx, &model), Some(display(&b)));
    assert_eq!(
        read(cx, &model, |m| m.recent().to_vec()),
        [display(&b), display(&a)]
    );
    // Position 1 is `a`, opened by the path the backend stored.
    act(cx, &model, |m, cx| m.open_recent(1, cx));
    idle(cx, &model);
    assert_eq!(manifest_path(cx, &model), Some(display(&a)));
    assert_eq!(
        read(cx, &model, |m| m.recent().to_vec()),
        [display(&a), display(&b)]
    );
    // A position past the list does nothing.
    act(cx, &model, |m, cx| m.open_recent(7, cx));
    idle(cx, &model);
    assert_eq!(manifest_path(cx, &model), Some(display(&a)));
    assert!(error_of(cx, &model).is_none());
}

#[gpui::test]
fn open_failure_keeps_the_session(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::loaded();
    let (model, fake) = start(cx, &backend);
    idle(cx, &model);
    backend.fail_next(err(TarpackErrorKind::ManifestUnreadable));
    fake.answer_pick(pick(Path::new("C:\\gone.toml")));
    act(cx, &model, TarpackModel::open);
    idle(cx, &model);
    assert_eq!(manifest_path(cx, &model).as_deref(), Some(MANIFEST_PATH));
    let error = error_of(cx, &model).expect("a banner");
    assert_eq!(
        error.copy(),
        text::error_message(TarpackErrorKind::ManifestUnreadable, None)
    );
    // Reload failing says the same, and the same error twice is new again.
    backend.fail_next(err(TarpackErrorKind::ManifestUnreadable));
    let generation_before = read(cx, &model, TarpackModel::error_generation);
    act(cx, &model, TarpackModel::reload);
    idle(cx, &model);
    assert_eq!(
        read(cx, &model, TarpackModel::error_generation),
        generation_before + 1
    );
}

// ---- reloading and the manifest's errors ----

#[gpui::test]
fn reload_clears_changed_and_results(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::loaded();
    let (model, _) = start(cx, &backend);
    idle(cx, &model);
    dropped(cx, &model, &["C:\\Builds\\gateway\\gateway"]);
    idle(cx, &model);
    assert!(drop_lines_of(cx, &model).is_some());
    act(cx, &model, TarpackModel::file_changed);
    assert!(read(cx, &model, TarpackModel::changed_on_disk));
    assert_eq!(say(cx, &model).as_deref(), Some(text::CHANGED_ON_DISK));

    backend.set_text(&gateway_with_errors());
    act(cx, &model, TarpackModel::reload);
    idle(cx, &model);
    assert!(read(cx, &model, |m| !m.changed_on_disk()));
    assert!(read(cx, &model, |m| m.drop_state().is_none()));
    assert_eq!(hint(cx, &model), Some(FocusHint::Reload));
    assert_eq!(
        say(cx, &model).as_deref(),
        Some("Gateway deploy has 2 errors. 1 file will be left out of the archive.")
    );
    // The assignment survives a reload.
    assert_eq!(
        status_of(cx, &model, "gateway-bin").map(|s| s.1),
        Some(EntryStatus::Ready)
    );

    // A clean reload after errors says so; a reload with nothing to clear
    // hints nothing.
    backend.set_text(GATEWAY);
    act(cx, &model, TarpackModel::reload);
    idle(cx, &model);
    assert_eq!(
        say(cx, &model).as_deref(),
        Some("Gateway deploy reloaded. No errors.")
    );
    assert_eq!(hint(cx, &model), None);
}

#[gpui::test]
fn pending_drop_survives_a_reload(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::loaded();
    let gate = Gate::new();
    backend.hold_drops(&gate);
    let (model, _) = start(cx, &backend);
    idle(cx, &model);
    dropped(cx, &model, &["C:\\x\\gateway"]);
    act(cx, &model, TarpackModel::reload);
    cx.run_until_parked();
    assert!(read(cx, &model, |m| matches!(
        m.drop_state(),
        Some(DropState::Pending)
    )));
    gate.open();
    idle(cx, &model);
    assert!(drop_lines_of(cx, &model).is_some());
}

#[gpui::test]
fn load_expands_errors_when_present(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::from_fixture(TarpackFixture::Errors);
    let (model, _) = start(cx, &backend);
    idle(cx, &model);
    assert!(
        read(cx, &model, TarpackModel::errors_expanded),
        "starts open"
    );
    act(cx, &model, |m, cx| m.set_errors_expanded(false, cx));
    assert!(!read(cx, &model, TarpackModel::errors_expanded));
    // Reload re-expands, and no focus is stolen.
    act(cx, &model, TarpackModel::reload);
    idle(cx, &model);
    assert!(read(cx, &model, TarpackModel::errors_expanded));
    assert_eq!(hint(cx, &model), None);
    // Without errors a load leaves it as the user set it.
    backend.set_text(GATEWAY);
    act(cx, &model, TarpackModel::reload);
    idle(cx, &model);
    act(cx, &model, |m, cx| m.set_errors_expanded(false, cx));
    act(cx, &model, TarpackModel::reload);
    idle(cx, &model);
    assert!(!read(cx, &model, TarpackModel::errors_expanded));
    // A command that is neither a load nor a reload does not touch it.
    backend.set_text(&gateway_with_errors());
    act(cx, &model, TarpackModel::reload);
    idle(cx, &model);
    act(cx, &model, |m, cx| m.set_errors_expanded(false, cx));
    act(cx, &model, |m, cx| m.clear("gateway-bin", cx));
    idle(cx, &model);
    assert!(!read(cx, &model, TarpackModel::errors_expanded));
}

#[gpui::test]
fn show_errors_expands_and_hints_focus(cx: &mut TestAppContext) {
    let (model, _) = start(cx, &FixtureTarpack::from_fixture(TarpackFixture::Errors));
    idle(cx, &model);
    act(cx, &model, |m, cx| m.set_errors_expanded(false, cx));
    assert_eq!(hint(cx, &model), None);
    act(cx, &model, TarpackModel::show_errors);
    assert!(read(cx, &model, TarpackModel::errors_expanded));
    assert_eq!(hint(cx, &model), Some(FocusHint::Report));
    assert_eq!(hint(cx, &model), None, "read once");
}

#[gpui::test]
fn show_errors_noop_without_errors(cx: &mut TestAppContext) {
    let (model, _) = start(cx, &FixtureTarpack::loaded());
    idle(cx, &model);
    act(cx, &model, |m, cx| m.set_errors_expanded(false, cx));
    act(cx, &model, TarpackModel::show_errors);
    assert!(!read(cx, &model, TarpackModel::errors_expanded));
    assert_eq!(hint(cx, &model), None);
    // Nor with no manifest at all.
    let (empty, _) = start(cx, &FixtureTarpack::empty());
    idle(cx, &empty);
    act(cx, &empty, TarpackModel::show_errors);
    assert_eq!(hint(cx, &empty), None);
}

#[gpui::test]
fn state_warning_announced_once_and_dismissal_sticks(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::loaded().with_state_warning("The state file was reset.");
    let (model, _) = start(cx, &backend);
    idle(cx, &model);
    assert_eq!(
        read(cx, &model, |m| m.state_warning().map(String::from)).as_deref(),
        Some("The state file was reset.")
    );
    assert_eq!(
        say(cx, &model).as_deref(),
        Some("The state file was reset.")
    );
    act(cx, &model, TarpackModel::dismiss_state_warning);
    assert_eq!(
        read(cx, &model, |m| m.state_warning().map(String::from)),
        None
    );
    // The same text stays dismissed and is not announced again.
    let before = generation(cx, &model);
    act(cx, &model, TarpackModel::reload);
    idle(cx, &model);
    assert_eq!(
        read(cx, &model, |m| m.state_warning().map(String::from)),
        None
    );
    assert_eq!(generation(cx, &model), before);
    // A different text shows, and is announced with the manifest sentence.
    backend.set_state_warning(Some("Saved state could not be written."));
    backend.set_text(&gateway_with_errors());
    act(cx, &model, TarpackModel::reload);
    idle(cx, &model);
    assert_eq!(
        read(cx, &model, |m| m.state_warning().map(String::from)).as_deref(),
        Some("Saved state could not be written.")
    );
    assert_eq!(
        say(cx, &model).as_deref(),
        Some("Saved state could not be written. Gateway deploy has 2 errors. 1 file will be left out of the archive.")
    );
    assert_eq!(generation(cx, &model), before + 1, "one announcement");
}

// ---- the watcher ----

#[gpui::test]
fn watcher_marks_changed_on_disk_once(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let m = write_manifest(dir.path(), "m.toml", GATEWAY);
    real(dir.path()).open(&m).expect("open");
    let (model, _) = start_boxed(cx, Box::new(real(dir.path())));
    idle(cx, &model);
    assert!(read(cx, &model, |m| !m.changed_on_disk()));
    assert_eq!(generation(cx, &model), 0, "a clean load announces nothing");
    // Edit until the watcher (started on the worker after the load) sees it.
    // An edit per second: edits closer than the debounce keep a burst going
    // and report nothing until it ends.
    let started = Instant::now();
    let mut edits = 0;
    while !read(cx, &model, TarpackModel::changed_on_disk) {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "the watcher never reported the edit"
        );
        edits += 1;
        fs::write(&m, format!("{GATEWAY}\n# edit {edits}\n")).expect("edit");
        let round = Instant::now();
        wait_for(cx, |cx| {
            round.elapsed() > Duration::from_millis(1000)
                || read(cx, &model, TarpackModel::changed_on_disk)
        });
    }
    assert_eq!(say(cx, &model).as_deref(), Some(text::CHANGED_ON_DISK));
    assert_eq!(generation(cx, &model), 1);
    // More edits do not announce again.
    fs::write(&m, format!("{GATEWAY}\n# last edit\n")).expect("edit");
    let waited = Instant::now();
    wait_for(cx, |_| waited.elapsed() > Duration::from_millis(1500));
    assert_eq!(generation(cx, &model), 1);
    assert!(read(cx, &model, TarpackModel::changed_on_disk));
    // A reload clears the flag; the watcher is re-armed and still works.
    act(cx, &model, TarpackModel::reload);
    idle(cx, &model);
    assert!(read(cx, &model, |m| !m.changed_on_disk()));
}

#[gpui::test]
fn inject_file_changed_marks_changed(cx: &mut TestAppContext) {
    let (model, _) = start(cx, &FixtureTarpack::loaded());
    idle(cx, &model);
    let hooks = Hooks {
        model: model.clone(),
    };
    let (_, window) = cx.add_window_view(|_, _| gpui::Empty);
    window.update(|window, cx| hooks.inject_file_changed(window, cx));
    assert!(read(window, &model, TarpackModel::changed_on_disk));
    assert_eq!(say(window, &model).as_deref(), Some(text::CHANGED_ON_DISK));
    let first = generation(window, &model);
    window.update(|window, cx| hooks.inject_file_changed(window, cx));
    assert_eq!(generation(window, &model), first, "announced once");
    // Activating the tool keeps it: it is about the file.
    window.update(|window, cx| hooks.activated(window, cx));
    window.run_until_parked();
    assert!(read(window, &model, TarpackModel::changed_on_disk));
}

// ---- editing ----

#[gpui::test]
fn can_edit_needs_dev_mode_and_manifest(cx: &mut TestAppContext) {
    cx.update(DevMode::init);
    let (empty, _) = start(cx, &FixtureTarpack::empty());
    let (loaded, _) = start(cx, &FixtureTarpack::loaded());
    idle(cx, &empty);
    idle(cx, &loaded);
    let can = |cx: &mut TestAppContext, model: &Entity<TarpackModel>| {
        cx.read(|cx| model.read(cx).can_edit(cx))
    };
    assert!(
        !can(cx, &empty) && !can(cx, &loaded),
        "developer mode is off"
    );
    cx.update(DevMode::toggle);
    assert!(!can(cx, &empty), "no manifest");
    assert!(can(cx, &loaded));
    cx.update(DevMode::toggle);
    assert!(!can(cx, &loaded));
}

#[gpui::test]
fn edit_opens_manifest_path(cx: &mut TestAppContext) {
    let (model, fake) = start(cx, &FixtureTarpack::loaded());
    idle(cx, &model);
    act(cx, &model, TarpackModel::edit);
    idle(cx, &model);
    assert_eq!(
        fake.calls(),
        [PlatformCall::OpenWithSystem(PathBuf::from(MANIFEST_PATH))]
    );
    // Nothing loaded: the backend's NoManifest is the banner.
    let (empty, fake) = start(cx, &FixtureTarpack::empty());
    idle(cx, &empty);
    act(cx, &empty, TarpackModel::edit);
    idle(cx, &empty);
    assert_eq!(fake.calls(), Vec::<PlatformCall>::new());
    assert_eq!(
        error_of(cx, &empty).expect("banner").copy(),
        "Open a manifest first."
    );
}

// ---- creating from the example ----

#[gpui::test]
fn create_from_example_writes_and_loads(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let (model, fake) = start_boxed(cx, Box::new(real(dir.path())));
    idle(cx, &model);
    let target = dir.path().join("new.toml");
    fake.answer_save(pick(&target));
    act(cx, &model, TarpackModel::create_from_example);
    wait_for_calls(cx, &fake, 1);
    idle(cx, &model);
    assert_eq!(
        fake.calls(),
        [PlatformCall::PickSave(SaveSpec {
            filters: vec![manifest_filter()],
            default_extension: Some("toml".to_string()),
            start_dir: default_start_dir(),
            suggested_name: Some("manifest.toml".into()),
            title: None,
        })]
    );
    assert_eq!(fs::read_to_string(&target).expect("written"), GATEWAY);
    assert_eq!(manifest_path(cx, &model), Some(display(&target)));
    assert_eq!(
        read(cx, &model, |m| m
            .session()
            .and_then(|s| s.manifest.as_ref().map(|m| m.name.clone())))
        .as_deref(),
        Some("Gateway deploy")
    );

    // With a manifest loaded, the dialog starts in its folder.
    let elsewhere = tempfile::tempdir().expect("tempdir");
    let second = elsewhere.path().join("second.toml");
    fake.answer_save(pick(&second));
    act(cx, &model, TarpackModel::create_from_example);
    idle(cx, &model);
    let Some(PlatformCall::PickSave(spec)) = fake.calls().pop() else {
        panic!("a save dialog");
    };
    assert_eq!(spec.start_dir.as_deref(), Some(dir.path()));
    assert!(second.is_file());
    assert_eq!(manifest_path(cx, &model), Some(display(&second)));
}

#[gpui::test]
fn create_from_example_refuses_existing_file(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let existing = write_manifest(dir.path(), "mine.toml", "keep me\n");
    let (model, fake) = start_boxed(cx, Box::new(real(dir.path())));
    idle(cx, &model);
    fake.answer_save(pick(&existing));
    act(cx, &model, TarpackModel::create_from_example);
    wait_for_calls(cx, &fake, 1);
    idle(cx, &model);
    let error = error_of(cx, &model).expect("a banner");
    let ViewError::Tarpack { error: inner, .. } = &error else {
        panic!("{error:?}");
    };
    assert_eq!(inner.kind, TarpackErrorKind::PathExists);
    assert_eq!(
        error.copy(),
        text::error_message(TarpackErrorKind::PathExists, None)
    );
    assert_eq!(fs::read_to_string(&existing).expect("read"), "keep me\n");
    assert_eq!(manifest_path(cx, &model), None, "nothing was loaded");
}

// ---- assigning ----

#[gpui::test]
fn browse_starts_in_assigned_folder(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::loaded();
    let (model, fake) = start(cx, &backend);
    idle(cx, &model);
    let pick_spec = |start_dir: Option<&str>| {
        PlatformCall::Pick(OpenSpec {
            kind: PickKind::File,
            filters: Vec::new(),
            start_dir: start_dir.map(PathBuf::from),
            title: None,
        })
    };
    // Unassigned: no folder; a cancel makes no call.
    act(cx, &model, |m, cx| m.browse("gateway-bin", cx));
    idle(cx, &model);
    assert_eq!(fake.calls(), [pick_spec(None)]);
    assert!(!backend
        .calls()
        .iter()
        .any(|c| matches!(c, Call::Assign(..))));
    // Assigned: starts in its folder.
    fake.answer_pick(pick(Path::new("/work/build/gateway")));
    act(cx, &model, |m, cx| m.browse("gateway-bin", cx));
    idle(cx, &model);
    assert_eq!(
        status_of(cx, &model, "gateway-bin"),
        Some((Some("/work/build/gateway".to_string()), EntryStatus::Ready))
    );
    act(cx, &model, |m, cx| m.browse("gateway-bin", cx));
    idle(cx, &model);
    assert_eq!(fake.calls().last(), Some(&pick_spec(Some("/work/build"))));
    // Browsing is silent.
    assert_eq!(say(cx, &model), None);
}

#[gpui::test]
fn assign_and_clear_update_entries(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::loaded();
    let (model, fake) = start(cx, &backend);
    idle(cx, &model);
    fake.answer_pick(pick(Path::new("/w/gateway.conf")));
    act(cx, &model, |m, cx| m.browse("gateway-conf", cx));
    idle(cx, &model);
    assert_eq!(
        status_of(cx, &model, "gateway-conf"),
        Some((Some("/w/gateway.conf".to_string()), EntryStatus::Ready))
    );
    assert_eq!(
        read(cx, &model, |m| m.session().map(|s| s.ready_count)),
        Some(1)
    );
    act(cx, &model, |m, cx| m.clear("gateway-conf", cx));
    idle(cx, &model);
    assert_eq!(
        status_of(cx, &model, "gateway-conf"),
        Some((None, EntryStatus::Unassigned))
    );
    assert!(backend
        .calls()
        .contains(&Call::Clear("gateway-conf".to_string())));
    assert_eq!(say(cx, &model), None, "assign and clear are silent");

    // A rejection names the entry's file and leaves the table unchanged.
    backend.fail_next(entry_error(TarpackErrorKind::NotAFile, "gateway-conf"));
    fake.answer_pick(pick(Path::new("/w/dir")));
    act(cx, &model, |m, cx| m.browse("gateway-conf", cx));
    idle(cx, &model);
    let error = error_of(cx, &model).expect("a banner");
    assert_eq!(error.copy(), "gateway.conf: the chosen path is not a file.");
    assert_eq!(error.detail(), "detail");
    assert_eq!(
        status_of(cx, &model, "gateway-conf"),
        Some((None, EntryStatus::Unassigned))
    );
    // The next success clears the banner.
    act(cx, &model, |m, cx| m.clear("gateway-bin", cx));
    idle(cx, &model);
    assert!(error_of(cx, &model).is_none());
}

#[gpui::test]
fn newest_command_wins(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::loaded();
    let (model, _) = start(cx, &backend);
    idle(cx, &model);
    // Two commands before either answer lands: only the newest is shown.
    act(cx, &model, |m, cx| {
        m.clear("gateway-bin", cx);
        m.reload(cx);
    });
    idle(cx, &model);
    assert_eq!(
        backend
            .calls()
            .iter()
            .filter(|c| matches!(c, Call::Reload | Call::Clear(_)))
            .count(),
        2,
        "both ran"
    );
    assert!(error_of(cx, &model).is_none());
}

// ---- dropping ----

fn hooks_target(cx: &mut TestAppContext, model: &Entity<TarpackModel>) -> DropTarget {
    let hooks = Hooks {
        model: model.clone(),
    };
    cx.read(|cx| hooks.drop_target(cx))
}

fn disabled(reason: &str) -> DropTarget {
    DropTarget::Disabled {
        reason: reason.to_string().into(),
    }
}

#[gpui::test]
fn drop_target_reasons(cx: &mut TestAppContext) {
    let (empty, _) = start(cx, &FixtureTarpack::empty());
    idle(cx, &empty);
    assert_eq!(hooks_target(cx, &empty), disabled("Open a manifest first"));

    let backend = FixtureTarpack::loaded();
    let gate = Gate::new();
    backend.hold_drops(&gate);
    let (model, _) = start(cx, &backend);
    idle(cx, &model);
    assert_eq!(
        hooks_target(cx, &model),
        DropTarget::Enabled {
            label: "Drop files or folders to match them to the manifest".into()
        }
    );
    act(cx, &model, |m, cx| {
        m.set_drop_blocker(Some(DropBlocker::Building), cx);
    });
    assert_eq!(hooks_target(cx, &model), disabled("A build is running"));
    act(cx, &model, |m, cx| {
        m.set_drop_blocker(Some(DropBlocker::Confirming), cx);
    });
    assert_eq!(hooks_target(cx, &model), disabled(DROP_BLOCKED_BY_DIALOG));
    act(cx, &model, |m, cx| m.set_drop_blocker(None, cx));
    dropped(cx, &model, &["C:\\x\\gateway"]);
    assert_eq!(
        hooks_target(cx, &model),
        disabled("Still matching the last drop")
    );
    gate.open();
    idle(cx, &model);
    assert!(matches!(
        hooks_target(cx, &model),
        DropTarget::Enabled { .. }
    ));

    let (withheld, _) = start(cx, &FixtureTarpack::from_fixture(TarpackFixture::Withheld));
    idle(cx, &withheld);
    assert_eq!(
        hooks_target(cx, &withheld),
        disabled("Fix the manifest errors first. No files can be matched yet.")
    );
    // No entries and no errors.
    let none = FixtureTarpack::loaded();
    none.set_text("version = 1\nname = \"Empty\"\nfile = []\n");
    let (listless, _) = start(cx, &none);
    idle(cx, &listless);
    act(cx, &listless, TarpackModel::reload);
    idle(cx, &listless);
    let no_entries = read(cx, &listless, |m| {
        m.session()
            .and_then(|s| s.manifest.as_ref())
            .map(|m| (m.entries.len(), m.error_count))
    });
    assert_eq!(no_entries, Some((0, 0)));
    assert_eq!(
        hooks_target(cx, &listless),
        disabled("This manifest lists no files")
    );
}

#[gpui::test]
fn drop_assigns_and_reports_lines(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::loaded();
    let (model, _) = start(cx, &backend);
    idle(cx, &model);
    dropped(
        cx,
        &model,
        &[
            "C:\\Builds\\gateway\\release\\gateway",
            "C:\\Builds\\gateway\\scripts\\start.sh",
            "C:\\Builds\\gateway\\config\\gateway.conf",
            "C:\\Builds\\gateway\\README.md",
        ],
    );
    assert!(read(cx, &model, |m| matches!(
        m.drop_state(),
        Some(DropState::Pending)
    )));
    idle(cx, &model);
    let lines = drop_lines_of(cx, &model).expect("a result");
    assert_eq!(
        lines_text(&lines),
        ["3 matched.", "1 not in the manifest: README.md."]
    );
    assert_eq!(
        say(cx, &model).as_deref(),
        Some("3 matched. 1 not in the manifest: README.md.")
    );
    for e in entries(cx, &model) {
        let expected = if matches!(e.view.id.as_str(), "gateway-ca" | "gateway-unit") {
            EntryStatus::Unassigned
        } else {
            EntryStatus::Ready
        };
        assert_eq!(e.status, expected, "{}", e.view.id);
    }
    assert_eq!(
        status_of(cx, &model, "gateway-bin")
            .and_then(|s| s.0)
            .as_deref(),
        Some("C:\\Builds\\gateway\\release\\gateway")
    );
    assert!(error_of(cx, &model).is_none());
    // The result keeps its flag and entries when the session changes later.
    act(cx, &model, |m, cx| m.clear("gateway-bin", cx));
    idle(cx, &model);
    let kept = read(cx, &model, |m| match m.drop_state() {
        Some(DropState::Result(r)) => {
            Some((r.has_failed_entries, r.entries.len(), r.entries[0].status))
        }
        _ => None,
    });
    assert_eq!(kept, Some((false, 5, EntryStatus::Ready)));
    // Dismissing it hints focus; dismissing nothing does not.
    act(cx, &model, TarpackModel::dismiss_drop_result);
    assert!(read(cx, &model, |m| m.drop_state().is_none()));
    assert_eq!(hint(cx, &model), Some(FocusHint::AfterResult));
    act(cx, &model, TarpackModel::dismiss_drop_result);
    assert_eq!(hint(cx, &model), None);
}

#[gpui::test]
fn drop_with_failed_entries_keeps_the_flag(cx: &mut TestAppContext) {
    let (model, _) = start(cx, &FixtureTarpack::from_fixture(TarpackFixture::Errors));
    idle(cx, &model);
    dropped(cx, &model, &["C:\\x\\notes.txt"]);
    idle(cx, &model);
    let lines = drop_lines_of(cx, &model).expect("a result");
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].lead, "not matched:");
    assert!(read(cx, &model, |m| matches!(
        m.drop_state(),
        Some(DropState::Result(r)) if r.has_failed_entries
    )));
}

#[gpui::test]
fn drop_ignored_while_pending(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::loaded();
    let gate = Gate::new();
    backend.hold_drops(&gate);
    let (model, _) = start(cx, &backend);
    idle(cx, &model);
    dropped(cx, &model, &["C:\\x\\gateway"]);
    cx.run_until_parked();
    dropped(cx, &model, &["C:\\x\\gateway.conf"]);
    cx.run_until_parked();
    gate.open();
    idle(cx, &model);
    assert_eq!(
        backend
            .calls()
            .iter()
            .filter(|c| **c == Call::BeginDrop)
            .count(),
        1
    );
    assert_eq!(
        lines_text(&drop_lines_of(cx, &model).expect("a result")),
        ["1 matched."]
    );
    assert_eq!(
        status_of(cx, &model, "gateway-conf").map(|s| s.1),
        Some(EntryStatus::Unassigned)
    );
}

#[gpui::test]
fn drop_while_blocked_is_ignored(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::loaded();
    let (model, _) = start(cx, &backend);
    idle(cx, &model);
    act(cx, &model, |m, cx| {
        m.set_drop_blocker(Some(DropBlocker::Building), cx);
    });
    dropped(cx, &model, &["C:\\x\\gateway"]);
    cx.run_until_parked();
    assert!(read(cx, &model, |m| m.drop_state().is_none()));
    assert!(!backend.calls().contains(&Call::BeginDrop));
}

#[gpui::test]
fn pending_shown_after_150ms_and_announced_after_1s(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::loaded();
    let gate = Gate::new();
    backend.hold_drops(&gate);
    let (model, _) = start(cx, &backend);
    idle(cx, &model);
    let ms = Duration::from_millis;
    dropped(cx, &model, &["C:\\x\\gateway"]);
    cx.executor().advance_clock(ms(149));
    assert!(read(cx, &model, |m| !m.drop_pending_shown()));
    cx.executor().advance_clock(ms(1));
    assert!(read(cx, &model, TarpackModel::drop_pending_shown));
    assert_eq!(say(cx, &model), None);
    cx.executor().advance_clock(ms(849));
    assert_eq!(say(cx, &model), None);
    cx.executor().advance_clock(ms(1));
    assert_eq!(
        say(cx, &model).as_deref(),
        Some("Matching dropped files\u{2026}")
    );
    // The result ends it: the line goes, and the result is announced.
    gate.open();
    idle(cx, &model);
    assert!(read(cx, &model, |m| !m.drop_pending_shown()));
    assert_eq!(say(cx, &model).as_deref(), Some("1 matched."));

    // A drop that finishes at once never shows or says it, and a later timer
    // of an earlier drop does not leak into the next one.
    let quick = FixtureTarpack::loaded();
    let (second, _) = start(cx, &quick);
    idle(cx, &second);
    dropped(cx, &second, &["C:\\x\\gateway"]);
    idle(cx, &second);
    cx.executor().advance_clock(ms(2000));
    assert!(read(cx, &second, |m| !m.drop_pending_shown()));
    assert_eq!(say(cx, &second).as_deref(), Some("1 matched."));
}

#[gpui::test]
fn stale_drop_retries_then_reports(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::loaded();
    let (model, _) = start(cx, &backend);
    idle(cx, &model);
    let begins = |backend: &FixtureTarpack| {
        backend
            .calls()
            .iter()
            .filter(|c| **c == Call::BeginDrop)
            .count()
    };
    // Stale twice, then it lands on the third attempt.
    backend.stale_drops(2);
    dropped(cx, &model, &["C:\\x\\gateway"]);
    idle(cx, &model);
    assert_eq!(begins(&backend), 3);
    assert_eq!(
        lines_text(&drop_lines_of(cx, &model).expect("a result")),
        ["1 matched."]
    );
    assert!(error_of(cx, &model).is_none());

    // Stale every time: reported, nothing applied.
    backend.stale_drops(3);
    dropped(cx, &model, &["C:\\x\\gateway.conf"]);
    idle(cx, &model);
    assert_eq!(begins(&backend), 6);
    assert!(read(cx, &model, |m| m.drop_state().is_none()));
    let error = error_of(cx, &model).expect("a banner");
    let ViewError::Tarpack { error: inner, .. } = &error else {
        panic!("{error:?}");
    };
    assert_eq!(inner.kind, TarpackErrorKind::Io);
    assert_eq!(
        inner.message,
        "the session changed while the dropped files were being matched; drop them again"
    );
    assert_eq!(
        status_of(cx, &model, "gateway-conf").map(|s| s.1),
        Some(EntryStatus::Unassigned)
    );
}

#[gpui::test]
fn drop_error_clears_pending(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::loaded();
    let (model, _) = start(cx, &backend);
    idle(cx, &model);
    // A previous result goes with the failed drop.
    dropped(cx, &model, &["C:\\x\\gateway"]);
    idle(cx, &model);
    assert!(drop_lines_of(cx, &model).is_some());
    backend.fail_next(err(TarpackErrorKind::NoManifest));
    dropped(cx, &model, &["C:\\x\\gateway.conf"]);
    assert_eq!(
        hint(cx, &model),
        Some(FocusHint::AfterResult),
        "a result is replaced"
    );
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.drop_state().is_none()));
    assert_eq!(
        error_of(cx, &model).expect("a banner").copy(),
        "Open a manifest first."
    );
    // The tool takes the next drop.
    assert!(matches!(
        hooks_target(cx, &model),
        DropTarget::Enabled { .. }
    ));
}

#[test]
fn real_backend_rejects_foreign_drop_work() {
    // A work object of another backend cannot be finished: the real backend
    // reports it instead of panicking.
    let dir = tempfile::tempdir().expect("tempdir");
    let m = write_manifest(dir.path(), "m.toml", GATEWAY);
    let mut backend = real(dir.path());
    backend.open(&m).expect("open");
    let mut foreign = FixtureTarpack::loaded();
    let work = foreign.begin_drop().expect("work");
    let error = backend.finish_drop(work).expect_err("foreign work");
    assert_eq!(error.kind, TarpackErrorKind::Io);
}

#[gpui::test]
fn real_drop_assigns_files(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest = write_manifest(dir.path(), "m.toml", GATEWAY);
    let sources = dir.path().join("src");
    fs::create_dir_all(&sources).expect("mkdir");
    let bin = sources.join("gateway");
    fs::write(&bin, b"bin").expect("write");
    let (model, fake) = start_boxed(cx, Box::new(real(dir.path())));
    idle(cx, &model);
    fake.answer_pick(pick(&manifest));
    act(cx, &model, TarpackModel::open);
    idle(cx, &model);
    let paths = vec![bin.clone(), sources.join("notes.txt")];
    act(cx, &model, |m, cx| m.dropped(paths, cx));
    idle(cx, &model);
    assert_eq!(
        status_of(cx, &model, "gateway-bin"),
        Some((Some(display(&bin)), EntryStatus::Ready))
    );
    let lines = drop_lines_of(cx, &model).expect("a result");
    assert_eq!(lines[0].kind, text::LineKind::Matched);
}

#[gpui::test]
fn open_clears_build_result(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let (model, fake, _, _) = real_ready(cx, dir.path());
    build(cx, &model);
    settled(cx, &model);
    assert!(result_of(cx, &model).is_some());
    let other = write_manifest(dir.path(), "other.toml", GATEWAY);
    fake.answer_pick(pick(&other));
    act(cx, &model, TarpackModel::open);
    wait_for(cx, |cx| manifest_path(cx, &model) == Some(display(&other)));
    assert!(result_of(cx, &model).is_none());
}

#[gpui::test]
fn open_recent_clears_build_result(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let (model, _fake, _, manifest) = real_ready(cx, dir.path());
    build(cx, &model);
    settled(cx, &model);
    assert!(result_of(cx, &model).is_some());
    assert_eq!(read(cx, &model, |m| m.recent().len()), 1);
    act(cx, &model, |m, cx| m.open_recent(0, cx));
    wait_for(cx, |cx| result_of(cx, &model).is_none());
    assert_eq!(manifest_path(cx, &model), Some(display(&manifest)));
}

#[gpui::test]
fn create_from_example_clears_build_result(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let (model, fake, _, _) = real_ready(cx, dir.path());
    build(cx, &model);
    settled(cx, &model);
    assert!(result_of(cx, &model).is_some());
    let target = dir.path().join("new.toml");
    fake.answer_save(pick(&target));
    act(cx, &model, TarpackModel::create_from_example);
    wait_for(cx, |cx| manifest_path(cx, &model) == Some(display(&target)));
    assert!(result_of(cx, &model).is_none());
}

// ---- activating ----

#[gpui::test]
fn activated_clears_transient_state(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::loaded();
    let (model, _) = start(cx, &backend);
    idle(cx, &model);
    // A dismissed result leaves a focus hint; the next result, the file
    // watcher and a failed reload leave the rest.
    dropped(cx, &model, &["C:\\x\\gateway"]);
    idle(cx, &model);
    act(cx, &model, TarpackModel::dismiss_drop_result);
    dropped(cx, &model, &["C:\\x\\gateway.conf"]);
    idle(cx, &model);
    act(cx, &model, TarpackModel::file_changed);
    backend.fail_next(err(TarpackErrorKind::Io));
    act(cx, &model, TarpackModel::reload);
    idle(cx, &model);
    assert!(error_of(cx, &model).is_some());
    assert!(read(cx, &model, |m| m.drop_state().is_some()));
    assert!(say(cx, &model).is_some());

    act(cx, &model, TarpackModel::activated);
    idle(cx, &model);
    assert!(error_of(cx, &model).is_none());
    assert!(read(cx, &model, |m| m.drop_state().is_none()));
    assert_eq!(say(cx, &model), None);
    assert_eq!(hint(cx, &model), None);
    assert!(
        read(cx, &model, TarpackModel::changed_on_disk),
        "kept: about the file"
    );
    assert_eq!(
        manifest_path(cx, &model).as_deref(),
        Some(MANIFEST_PATH),
        "session kept"
    );
}

#[gpui::test]
fn activated_keeps_a_pending_drop(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::loaded();
    let gate = Gate::new();
    backend.hold_drops(&gate);
    let (model, _) = start(cx, &backend);
    idle(cx, &model);
    dropped(cx, &model, &["C:\\x\\gateway"]);
    cx.run_until_parked();
    act(cx, &model, TarpackModel::activated);
    cx.run_until_parked();
    assert!(read(cx, &model, |m| matches!(
        m.drop_state(),
        Some(DropState::Pending)
    )));
    gate.open();
    idle(cx, &model);
    assert!(drop_lines_of(cx, &model).is_some());
}

#[gpui::test]
fn activated_refreshes_the_session(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::loaded();
    let (model, _) = start(cx, &backend);
    idle(cx, &model);
    let before = backend
        .calls()
        .iter()
        .filter(|c| **c == Call::Session)
        .count();
    act(cx, &model, TarpackModel::activated);
    idle(cx, &model);
    assert_eq!(
        backend
            .calls()
            .iter()
            .filter(|c| **c == Call::Session)
            .count(),
        before + 1
    );
}

// ---- fixtures ----

#[gpui::test]
fn fixtures_build_each_manifest_state(cx: &mut TestAppContext) {
    use TarpackFixture as F;
    let all = [
        F::Empty,
        F::Recent,
        F::Loaded,
        F::Errors,
        F::Missing,
        F::Partial,
        F::Changed,
        F::Drag,
        F::Dropped,
        F::Withheld,
        F::Ready,
    ];
    for fixture in all {
        let mut backend = FixtureTarpack::from_fixture(fixture);
        let session = backend.session();
        let recent = backend.recent_manifests();
        let manifest = session.manifest.as_ref();
        let statuses = |s: EntryStatus| {
            manifest.map_or(0, |m| m.entries.iter().filter(|e| e.status == s).count())
        };
        match fixture {
            F::Empty => {
                assert!(manifest.is_none(), "{fixture:?}");
                assert_eq!(recent, Vec::<String>::new());
            }
            F::Recent | F::Loaded | F::Drag | F::Dropped | F::Shortcuts => {
                let m = manifest.expect("manifest");
                assert_eq!((m.entries.len(), m.error_count), (5, 0), "{fixture:?}");
                assert_eq!(statuses(EntryStatus::Unassigned), 5);
                assert_eq!(recent.len(), 3);
                assert_eq!(recent[0], MANIFEST_PATH);
                assert!(!session.can_build);
            }
            F::Errors => {
                let m = manifest.expect("manifest");
                assert_eq!(m.error_count, 2);
                assert_eq!(m.failed_entries.len(), 1);
                assert_eq!(m.warnings.len(), 1);
                assert_eq!(m.entries.len(), 4);
                assert_eq!(statuses(EntryStatus::Ready), 4);
                assert_eq!(session.output_path.as_deref(), Some(OUTPUT_PATH));
            }
            F::Missing => {
                assert_eq!(statuses(EntryStatus::Missing), 1);
                assert!(!session.can_build);
            }
            F::Partial | F::ResultPartial => {
                assert_eq!(statuses(EntryStatus::Ready), 3);
                assert_eq!(statuses(EntryStatus::Unassigned), 2);
                assert_eq!(session.output_path.as_deref(), Some(OUTPUT_PATH));
            }
            F::Changed
            | F::Ready
            | F::BuildBar
            | F::Building
            | F::Replace
            | F::Result
            | F::Dev
            | F::DevResult => {
                assert_eq!(statuses(EntryStatus::Ready), 5, "{fixture:?}");
                assert!(session.can_build);
            }
            F::Withheld => {
                let m = manifest.expect("manifest");
                assert!(m.entries_withheld && m.entries.is_empty());
                assert_eq!(m.error_count, 1);
            }
        }
        // Every state is there once the Worker has answered, with no wait.
        let (model, _) = start(cx, &FixtureTarpack::from_fixture(fixture));
        idle(cx, &model);
        assert_eq!(
            read(cx, &model, |m| m
                .session()
                .and_then(|s| s.manifest.as_ref())
                .is_some()),
            manifest.is_some(),
            "{fixture:?}"
        );
        assert_eq!(read(cx, &model, |m| m.recent().len()), recent.len());
    }
    // The text constants the states are built from parse as intended.
    assert_eq!(
        fm_tarpack::manifest::parse(&gateway_withheld()).error_count(),
        1
    );
}

// ---- review follow-up: opener failures ----

#[gpui::test]
fn edit_reports_an_opener_failure(cx: &mut TestAppContext) {
    let (model, fake) = start(cx, &FixtureTarpack::loaded());
    idle(cx, &model);
    // The system refuses (no `.toml` handler): the banner, with its detail.
    fake.answer_open(Err(OpenError {
        detail: "No application is associated with this file type.".to_string(),
    }));
    let before = read(cx, &model, TarpackModel::error_generation);
    act(cx, &model, TarpackModel::edit);
    idle(cx, &model);
    let error = error_of(cx, &model).expect("a banner");
    let ViewError::Tarpack { error: inner, .. } = &error else {
        panic!("{error:?}");
    };
    assert_eq!(inner.kind, TarpackErrorKind::OpenerFailed);
    assert_eq!(error.copy(), "Windows could not open it.");
    assert_eq!(
        error.detail(),
        "No application is associated with this file type."
    );
    assert_eq!(read(cx, &model, TarpackModel::error_generation), before + 1);
    // The next success (nothing scripted: `Ok`) leaves no banner behind.
    act(cx, &model, |m, cx| m.clear("gateway-bin", cx));
    idle(cx, &model);
    act(cx, &model, TarpackModel::edit);
    idle(cx, &model);
    assert!(error_of(cx, &model).is_none());
    assert_eq!(
        fake.calls()
            .iter()
            .filter(|c| matches!(c, PlatformCall::OpenWithSystem(_)))
            .count(),
        2
    );
}

// ---- review follow-up: the recent list ----

#[gpui::test]
fn open_recent_refuses_a_list_that_changed(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::loaded();
    let (model, _) = start(cx, &backend);
    idle(cx, &model);
    let shown = read(cx, &model, |m| m.recent().to_vec());
    assert_eq!(shown[1], "C:\\Packages\\sensor-agent\\manifest.toml");
    // The backend's list is reordered before the model's copy refreshes.
    backend.set_recent(&[
        "C:\\Packages\\gateway\\manifest.toml",
        "D:\\work\\edge-router\\deploy.toml",
        "C:\\Packages\\sensor-agent\\manifest.toml",
    ]);
    act(cx, &model, |m, cx| m.open_recent(1, cx));
    idle(cx, &model);
    // Nothing was opened, the list is the backend's, and the banner says why.
    assert_eq!(manifest_path(cx, &model).as_deref(), Some(MANIFEST_PATH));
    assert_eq!(
        read(cx, &model, |m| m.recent().to_vec()),
        [
            "C:\\Packages\\gateway\\manifest.toml",
            "D:\\work\\edge-router\\deploy.toml",
            "C:\\Packages\\sensor-agent\\manifest.toml",
        ]
    );
    let error = error_of(cx, &model).expect("a banner");
    assert_eq!(error, ViewError::RecentChanged);
    assert_eq!(
        error.copy(),
        "The recent list changed. Choose the manifest again."
    );
    assert_eq!(error.detail(), "");
    assert_eq!(
        backend
            .calls()
            .iter()
            .filter(|c| matches!(c, Call::Open(_)))
            .count(),
        0
    );
    // Choosing again, from the refreshed list, opens the right one.
    act(cx, &model, |m, cx| m.open_recent(2, cx));
    idle(cx, &model);
    assert_eq!(
        manifest_path(cx, &model).as_deref(),
        Some("C:\\Packages\\sensor-agent\\manifest.toml")
    );
    assert!(error_of(cx, &model).is_none());
}

#[test]
fn real_backend_open_recent_checks_the_display_string() {
    let dir = tempfile::tempdir().expect("tempdir");
    let a = write_manifest(dir.path(), "a.toml", GATEWAY);
    let b = write_manifest(dir.path(), "b.toml", GATEWAY);
    let mut backend = real(dir.path());
    backend.open(&a).expect("open a");
    backend.open(&b).expect("open b");
    assert_eq!(backend.recent_manifests(), [display(&b), display(&a)]);
    // A copy that thinks position 1 is something else opens nothing.
    assert_eq!(
        backend.open_recent(1, &display(&b)).expect("no error"),
        None
    );
    assert_eq!(
        backend.manifest_path().as_deref(),
        Some(b.as_path()),
        "still on b"
    );
    // The matching text opens it.
    let session = backend
        .open_recent(1, &display(&a))
        .expect("no error")
        .expect("opened");
    assert_eq!(
        session.manifest.map(|m| m.path),
        Some(display(&a)),
        "now on a"
    );
    assert_eq!(backend.recent_manifests(), [display(&a), display(&b)]);
}

#[gpui::test]
fn recent_list_refreshes_after_every_load(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let a = write_manifest(dir.path(), "a.toml", GATEWAY);
    let (model, fake) = start_boxed(cx, Box::new(real(dir.path())));
    idle(cx, &model);
    assert!(read(cx, &model, |m| m.recent().is_empty()));
    fake.answer_pick(pick(&a));
    act(cx, &model, TarpackModel::open);
    idle(cx, &model);
    assert_eq!(read(cx, &model, |m| m.recent().to_vec()), [display(&a)]);
    // A created example is remembered too.
    let created = dir.path().join("new.toml");
    fake.answer_save(pick(&created));
    act(cx, &model, TarpackModel::create_from_example);
    wait_for_calls(cx, &fake, 2);
    idle(cx, &model);
    assert_eq!(
        read(cx, &model, |m| m.recent().to_vec()),
        [display(&created), display(&a)]
    );
}

// ---- review follow-up: where browse starts ----

/// The folder the second browse of `gateway-bin` starts in, after the first
/// assigned `assigned` to it.
fn browse_start(cx: &mut TestAppContext, assigned: &str) -> Option<PathBuf> {
    let backend = FixtureTarpack::loaded();
    let (model, fake) = start(cx, &backend);
    idle(cx, &model);
    fake.answer_pick(pick(Path::new(assigned)));
    act(cx, &model, |m, cx| m.browse("gateway-bin", cx));
    idle(cx, &model);
    act(cx, &model, |m, cx| m.browse("gateway-bin", cx));
    idle(cx, &model);
    let Some(PlatformCall::Pick(spec)) = fake.calls().pop() else {
        panic!("a dialog");
    };
    spec.start_dir
}

#[cfg(unix)]
#[gpui::test]
fn browse_starts_in_the_parent_folder(cx: &mut TestAppContext) {
    let start = |cx: &mut TestAppContext, p: &str| browse_start(cx, p);
    assert_eq!(start(cx, "/a.txt"), Some(PathBuf::from("/")), "root");
    assert_eq!(start(cx, "/tools/a.txt"), Some(PathBuf::from("/tools")));
    assert_eq!(
        start(cx, "/tools/sub/a.txt"),
        Some(PathBuf::from("/tools/sub"))
    );
    assert_eq!(start(cx, "a.txt"), None, "no parent, no start folder");
}

// Windows path syntax: only the Windows job can run this.
#[cfg(windows)]
#[gpui::test]
fn browse_starts_in_the_parent_folder(cx: &mut TestAppContext) {
    let start = |cx: &mut TestAppContext, p: &str| browse_start(cx, p);
    assert_eq!(
        start(cx, "C:\\a.txt"),
        Some(PathBuf::from("C:\\")),
        "drive root"
    );
    assert_eq!(start(cx, "C:/a.txt"), Some(PathBuf::from("C:/")));
    assert_eq!(
        start(cx, "C:\\tools\\a.txt"),
        Some(PathBuf::from("C:\\tools"))
    );
    assert_eq!(
        start(cx, "C:/tools/sub/a.txt"),
        Some(PathBuf::from("C:/tools/sub"))
    );
    assert_eq!(
        start(cx, "\\\\srv\\share\\a.txt"),
        Some(PathBuf::from("\\\\srv\\share\\")),
        "UNC share root"
    );
    assert_eq!(
        start(cx, "\\\\srv\\share\\d\\a.txt"),
        Some(PathBuf::from("\\\\srv\\share\\d"))
    );
    assert_eq!(start(cx, "a.txt"), None, "no parent, no start folder");
}

#[cfg(unix)]
#[gpui::test]
fn browse_starts_in_a_folder_that_is_not_valid_unicode(cx: &mut TestAppContext) {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let dir = tempfile::tempdir().expect("tempdir");
    let odd = dir.path().join(OsStr::from_bytes(b"d\xff"));
    fs::create_dir(&odd).expect("dir");
    let source = odd.join("gateway");
    fs::write(&source, b"x").expect("source");
    let m = write_manifest(dir.path(), "m.toml", GATEWAY);
    real(dir.path()).open(&m).expect("open");
    let (model, fake) = start_boxed(cx, Box::new(real(dir.path())));
    idle(cx, &model);
    fake.answer_pick(pick(&source));
    act(cx, &model, |m, cx| m.browse("gateway-bin", cx));
    idle(cx, &model);
    act(cx, &model, |m, cx| m.browse("gateway-bin", cx));
    idle(cx, &model);
    let Some(PlatformCall::Pick(spec)) = fake.calls().pop() else {
        panic!("a dialog");
    };
    // The exact folder, not its display string with U+FFFD in it.
    assert_eq!(spec.start_dir.as_deref(), Some(odd.as_path()));
}

// ---- review follow-up: the start folder lookup ----

#[gpui::test]
fn default_start_dir_is_looked_up_off_the_ui_thread(cx: &mut TestAppContext) {
    let (model, fake) = start(cx, &FixtureTarpack::empty());
    idle(cx, &model);
    let ui = std::thread::current().id();
    act(cx, &model, TarpackModel::create_from_example);
    wait_for_calls(cx, &fake, 1);
    let lookups = LOOKUP_THREADS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert!(!lookups.is_empty(), "the lookup ran");
    assert!(!lookups.contains(&ui), "never on the UI thread");
    let Some(PlatformCall::PickSave(spec)) = fake.calls().pop() else {
        panic!("a save dialog");
    };
    assert_eq!(spec.start_dir, default_start_dir());
}

// ---- review follow-up: stale results, panics, the watcher ----

#[gpui::test]
fn a_stale_command_result_is_discarded(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::loaded();
    let (model, _) = start(cx, &backend);
    idle(cx, &model);
    let errors_before = read(cx, &model, TarpackModel::error_generation);
    // The older command fails; the newer one succeeds. Showing the older
    // answer would raise a banner (and bump the generation), however briefly.
    backend.fail_next(err(TarpackErrorKind::UnknownEntry));
    act(cx, &model, |m, cx| {
        m.clear("gateway-bin", cx);
        m.clear("gateway-conf", cx);
    });
    idle(cx, &model);
    assert_eq!(
        read(cx, &model, TarpackModel::error_generation),
        errors_before,
        "the older failure was never shown"
    );
    assert!(error_of(cx, &model).is_none());
    // Both commands ran in order on the backend.
    let clears: Vec<_> = backend
        .calls()
        .into_iter()
        .filter(|c| matches!(c, Call::Clear(_)))
        .collect();
    assert_eq!(
        clears,
        [
            Call::Clear("gateway-bin".to_string()),
            Call::Clear("gateway-conf".to_string())
        ]
    );
}

fn session_fetches(backend: &FixtureTarpack) -> usize {
    backend
        .calls()
        .iter()
        .filter(|c| matches!(c, Call::Session))
        .count()
}

#[gpui::test]
fn a_panicking_command_refetches_the_session(cx: &mut TestAppContext) {
    for (command, run) in [
        (
            Command::Clear,
            (|m: &mut TarpackModel, cx: &mut Context<TarpackModel>| m.clear("gateway-bin", cx))
                as fn(&mut TarpackModel, &mut Context<TarpackModel>),
        ),
        (Command::Reload, TarpackModel::reload),
    ] {
        let backend = FixtureTarpack::loaded();
        let (model, _) = start(cx, &backend);
        idle(cx, &model);
        let fetched = session_fetches(&backend);
        backend.panic_on(command);
        act(cx, &model, run);
        idle(cx, &model);
        let error = error_of(cx, &model).expect("the banner stays");
        assert!(
            matches!(&error, ViewError::Unexpected { message } if message.contains("exploded")),
            "{command:?}: {error:?}"
        );
        assert_eq!(
            session_fetches(&backend),
            fetched + 1,
            "{command:?}: the backend's state was fetched again"
        );
        assert_eq!(manifest_path(cx, &model).as_deref(), Some(MANIFEST_PATH));
    }
}

#[gpui::test]
fn a_panicking_drop_refetches_the_session(cx: &mut TestAppContext) {
    for command in [Command::BeginDrop, Command::FinishDrop] {
        let backend = FixtureTarpack::loaded();
        let (model, _) = start(cx, &backend);
        idle(cx, &model);
        let fetched = session_fetches(&backend);
        backend.panic_on(command);
        dropped(cx, &model, &["C:\\Builds\\gateway\\gateway"]);
        idle(cx, &model);
        wait_for(cx, |_| session_fetches(&backend) > fetched);
        let error = error_of(cx, &model).expect("the banner stays");
        assert!(
            matches!(&error, ViewError::Unexpected { message } if message.contains("exploded")),
            "{command:?}: {error:?}"
        );
        assert!(read(cx, &model, |m| m.drop_state().is_none()));
        assert_eq!(session_fetches(&backend), fetched + 1, "{command:?}");
    }
}

fn watcher_id(cx: &mut TestAppContext, model: &Entity<TarpackModel>) -> Option<u64> {
    read(cx, model, TarpackModel::watcher_id)
}

/// Runs until the model's watcher is not `old`.
fn wait_for_new_watcher(
    cx: &mut TestAppContext,
    model: &Entity<TarpackModel>,
    old: Option<u64>,
) -> u64 {
    wait_for(cx, |cx| {
        watcher_id(cx, model).is_some() && watcher_id(cx, model) != old
    });
    watcher_id(cx, model).expect("armed")
}

#[gpui::test]
fn the_watcher_is_re_armed_on_open_reload_and_create(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let a = write_manifest(dir.path(), "a.toml", GATEWAY);
    let b = write_manifest(dir.path(), "b.toml", GATEWAY);
    real(dir.path()).open(&a).expect("open");
    let (model, fake) = start_boxed(cx, Box::new(real(dir.path())));
    idle(cx, &model);
    let first = wait_for_new_watcher(cx, &model, None);
    // Fetching the session again keeps the watcher of the same path.
    act(cx, &model, TarpackModel::activated);
    idle(cx, &model);
    assert_eq!(watcher_id(cx, &model), Some(first), "kept");
    // Opening another manifest arms a new one.
    fake.answer_pick(pick(&b));
    act(cx, &model, TarpackModel::open);
    let second = wait_for_new_watcher(cx, &model, Some(first));
    // Reloading replaces it even though the path is the same.
    act(cx, &model, TarpackModel::reload);
    let third = wait_for_new_watcher(cx, &model, Some(second));
    // So does creating from the example.
    let created = dir.path().join("new.toml");
    fake.answer_save(pick(&created));
    act(cx, &model, TarpackModel::create_from_example);
    wait_for_calls(cx, &fake, 2);
    let fourth = wait_for_new_watcher(cx, &model, Some(third));
    let ids = [first, second, third, fourth];
    for (i, id) in ids.iter().enumerate() {
        assert!(!ids[..i].contains(id), "{ids:?}: every re-arm is new");
    }
}

#[gpui::test]
fn events_of_a_superseded_watcher_change_nothing(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let m = write_manifest(dir.path(), "m.toml", GATEWAY);
    real(dir.path()).open(&m).expect("open");
    let (model, _) = start_boxed(cx, Box::new(real(dir.path())));
    idle(cx, &model);
    let old = wait_for_new_watcher(cx, &model, None);
    act(cx, &model, TarpackModel::reload);
    let current = wait_for_new_watcher(cx, &model, Some(old));
    let generation_before = generation(cx, &model);
    // An event of the old watcher, after the re-arm.
    act(cx, &model, |m, cx| m.watcher_fired(old, cx));
    assert!(read(cx, &model, |m| !m.changed_on_disk()));
    assert_eq!(generation(cx, &model), generation_before, "no announcement");
    // The current watcher's event counts.
    act(cx, &model, |m, cx| m.watcher_fired(current, cx));
    assert!(read(cx, &model, TarpackModel::changed_on_disk));
    assert_eq!(say(cx, &model).as_deref(), Some(text::CHANGED_ON_DISK));
}

#[gpui::test]
fn a_reload_with_the_same_error_count_announces_again(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::from_fixture(TarpackFixture::Errors);
    let (model, _) = start(cx, &backend);
    idle(cx, &model);
    let sentence = "Gateway deploy has 2 errors. 1 file will be left out of the archive.";
    let mut previous = generation(cx, &model);
    for _ in 0..2 {
        act(cx, &model, TarpackModel::reload);
        idle(cx, &model);
        // The same sentence, announced again: only the generation tells.
        assert_eq!(say(cx, &model).as_deref(), Some(sentence));
        assert_eq!(generation(cx, &model), previous + 1);
        previous += 1;
    }
    // Clearing the errors announces "No errors"; a clean reload after that
    // announces nothing.
    backend.set_text(GATEWAY);
    act(cx, &model, TarpackModel::reload);
    idle(cx, &model);
    assert_eq!(
        say(cx, &model).as_deref(),
        Some("Gateway deploy reloaded. No errors.")
    );
    assert_eq!(generation(cx, &model), previous + 1);
    act(cx, &model, TarpackModel::reload);
    idle(cx, &model);
    assert_eq!(generation(cx, &model), previous + 1);
}

#[test]
fn the_errors_fixture_carries_the_reference_diagnostics() {
    let mut backend = FixtureTarpack::from_fixture(TarpackFixture::Errors);
    let session = backend.session();
    let m = session.manifest.expect("manifest");
    let ids: Vec<&str> = m.entries.iter().map(|e| e.view.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "gateway-bin",
            "gateway-conf",
            "gateway-start",
            "gateway-unit"
        ]
    );
    let failed = &m.failed_entries[0];
    assert_eq!(
        (failed.id.as_deref(), failed.line),
        (Some("gateway-ca"), 33)
    );
    let at: Vec<_> = failed.errors.iter().map(|e| (e.line, e.col)).collect();
    assert_eq!(at, [(35, 7), (36, 8)]);
    assert_eq!(
        failed.errors[0].message,
        "file `gateway-ca`: dir must be absolute (start with `/`)"
    );
    assert_eq!(
        failed.errors[1].message,
        "file `gateway-ca`: mode: mode `0898` is not an octal string"
    );
    assert_eq!(m.warnings.len(), 1);
    assert_eq!((m.warnings[0].line, m.warnings[0].col), (41, 10));
    assert_eq!(
        m.warnings[0].message,
        "file `gateway-unit`: source `gateway.service` matches file `gateway-svc` (compared case-insensitively); a dropped file cannot tell them apart, so pick their files per row"
    );
    assert_eq!(m.error_count, 2);
    // The other states show the five-entry gateway.
    let loaded = FixtureTarpack::from_fixture(TarpackFixture::Loaded).session();
    let loaded = loaded.manifest.expect("manifest");
    assert_eq!(loaded.entries.len(), 5);
    assert_eq!(loaded.entries[3].view.source, "ca.pem");
    assert_eq!(loaded.entries[4].view.source, "gateway.service");
}

// ---- the build side: copy ----

fn diag(message: &str, line: u32, col: u32) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        line,
        col,
        entry_id: None,
        message: message.to_string(),
    }
}

fn failure(index: u32, id: Option<&str>, source: Option<&str>, line: u32) -> EntryFailure {
    EntryFailure {
        index,
        id: id.map(str::to_string),
        source: source.map(str::to_string),
        line,
        errors: vec![diag(
            &format!("file `bad{index}`: missing field `dir`"),
            line + 1,
            3,
        )],
    }
}

/// `summary()` of `fixtures.ts`: a clean build of `gateway.tar.zst`.
fn summary() -> BuildSummary {
    BuildSummary {
        path: "C:\\out\\gateway.tar.zst".to_string(),
        format: ArchiveFormat::TarZst,
        entries: 3,
        files: 2,
        dirs: 1,
        bytes: 2048,
        uncompressed_bytes: 10 * 1024 * 1024,
        sha256_hex: "ab".repeat(32),
        extract_command: "tar --zstd --no-overwrite-dir -xpPf gateway.tar.zst".to_string(),
        normalized_entries: Vec::new(),
        built_ids: vec!["gateway".to_string(), "core".to_string()],
        left_out: Vec::new(),
        not_loaded: Vec::new(),
        manifest_errors: Vec::new(),
        warnings: Vec::new(),
        error_count: 0,
    }
}

#[test]
#[expect(clippy::too_many_lines, reason = "one case per reportText test")]
fn report_text_matches_today() {
    // reportText.test.ts: the full report.
    let full = BuildSummary {
        left_out: vec![
            EntryFailure {
                errors: vec![diag("bad dir", 6, 3), diag("bad mode", 7, 1)],
                ..failure(1, Some("a"), Some("a.bin"), 5)
            },
            EntryFailure {
                errors: vec![diag("no id", 10, 1)],
                ..failure(2, None, None, 9)
            },
        ],
        manifest_errors: vec![diag("bad name", 1, 1)],
        warnings: vec![Diagnostic {
            severity: Severity::Warning,
            ..diag("odd mode", 4, 2)
        }],
        error_count: 4,
        ..summary()
    };
    assert_eq!(
        text::report_text(&full),
        [
            "Created gateway.tar.zst with 2 files left out",
            "",
            "Left out of the archive:",
            "a (source a.bin, line 5)",
            "  6:3 bad dir",
            "  7:1 bad mode",
            "Entry #2 (line 9)",
            "  10:1 no id",
            "",
            "Manifest errors:",
            "  1:1 bad name",
            "",
            "Warnings:",
            "  4:2 odd mode",
        ]
        .join("\n")
    );
    // Empty sections are omitted.
    let errors_only = BuildSummary {
        manifest_errors: vec![diag("bad name", 1, 1)],
        error_count: 1,
        ..summary()
    };
    assert_eq!(
        text::report_text(&errors_only),
        "Created gateway.tar.zst with 1 manifest error\n\nManifest errors:\n  1:1 bad name"
    );
    // Announcements and headings.
    assert_eq!(
        text::result_announcement(&summary()),
        "Created gateway.tar.zst."
    );
    let one_left_out = BuildSummary {
        left_out: vec![failure(1, Some("bad1"), Some("bad1.bin"), 5)],
        error_count: 1,
        ..summary()
    };
    assert_eq!(
        text::result_heading(&one_left_out),
        "Created gateway.tar.zst with 1 file left out"
    );
    assert_eq!(
        text::result_announcement(&one_left_out),
        "Created gateway.tar.zst. 1 file was left out because of errors."
    );
    let two_left_out = BuildSummary {
        left_out: vec![
            failure(1, Some("bad1"), None, 5),
            failure(2, Some("bad2"), None, 10),
        ],
        error_count: 2,
        ..summary()
    };
    assert_eq!(
        text::result_announcement(&two_left_out),
        "Created gateway.tar.zst. 2 files were left out because of errors."
    );
    let two_errors = BuildSummary {
        manifest_errors: vec![diag("x", 3, 1), diag("y", 3, 1)],
        error_count: 2,
        ..summary()
    };
    assert_eq!(
        text::result_announcement(&two_errors),
        "Created gateway.tar.zst with 2 manifest errors."
    );
    // A partial build names the files with no source chosen.
    let partial = BuildSummary {
        not_loaded: vec!["gateway".to_string()],
        built_ids: vec!["core".to_string()],
        ..summary()
    };
    assert_eq!(
        text::result_heading(&partial),
        "Created gateway.tar.zst with 1 file not included"
    );
    assert!(text::report_text(&partial).contains("Not included (no file chosen):\n  gateway"));
    assert_eq!(
        text::result_announcement(&partial),
        "Created gateway.tar.zst. 1 file was not included because no file was chosen."
    );
    // BuildResult.tsx: ledes, facts, sizes.
    assert_eq!(
        text::not_loaded_lede(&partial).as_deref(),
        Some("The archive holds 1 of the manifest's 2 files. No file was chosen for:")
    );
    assert_eq!(text::left_out_lede(&partial), None);
    assert_eq!(
        text::left_out_lede(&two_left_out).as_deref(),
        Some("The archive holds 2 of the manifest's 4 files. The rest have errors, listed below.")
    );
    assert_eq!(
        text::result_facts(&summary()),
        "zstd \u{b7} 2 files, 1 folder \u{b7} 2.0 KB (10.0 MB uncompressed)"
    );
    let tar = BuildSummary {
        format: ArchiveFormat::Tar,
        ..summary()
    };
    assert_eq!(
        text::result_facts(&tar),
        "tar \u{b7} 2 files, 1 folder \u{b7} 2.0 KB"
    );
    assert_eq!(text::normalized_line("a.conf", 0), "a.conf: no CRLF found");
    assert_eq!(
        text::normalized_line("a.conf", 42),
        "a.conf: 42 CRLF replaced"
    );
    // `toFixed(1)` rounds a tie up: 1280 B is 1.25 KB.
    for (n, shown) in [
        (0, "0 B"),
        (1023, "1023 B"),
        (1024, "1.0 KB"),
        (1280, "1.3 KB"),
        (1024 * 1024 - 1, "1024.0 KB"),
        (1024 * 1024, "1.0 MB"),
        (24_117_248, "23.0 MB"),
    ] {
        assert_eq!(text::file_size(n), shown, "{n}");
    }
}

fn session_with(
    reason: Option<fm_tarpack::session::BuildBlockedReason>,
    manifest: Option<fm_tarpack::session::SessionManifest>,
    ready: u32,
    total: u32,
) -> fm_tarpack::session::TarpackSession {
    fm_tarpack::session::TarpackSession {
        manifest,
        output_path: None,
        format: ArchiveFormat::Tar,
        formats: fm_tarpack::session::ArchiveFormatOption::all(),
        suggested_output_name: None,
        ready_count: ready,
        total_count: total,
        can_build: reason.is_none(),
        build_blocked_reason: reason,
        state_warning: None,
    }
}

fn manifest_with(
    entries: Vec<SessionEntry>,
    errors: Vec<Diagnostic>,
    failed: Vec<EntryFailure>,
    withheld: bool,
) -> fm_tarpack::session::SessionManifest {
    let error_count = errors.len() + failed.iter().map(|f| f.errors.len()).sum::<usize>();
    fm_tarpack::session::SessionManifest {
        path: "C:\\pkgs\\gateway\\manifest.toml".to_string(),
        name: "gateway".to_string(),
        output_name: None,
        hash: "abc".to_string(),
        entries,
        entries_withheld: withheld,
        errors,
        failed_entries: failed,
        error_count: u32::try_from(error_count).expect("small"),
        warnings: Vec::new(),
    }
}

fn with_status(id: &str, status: EntryStatus) -> SessionEntry {
    SessionEntry {
        status,
        ..entry(id)
    }
}

#[test]
#[expect(clippy::too_many_lines, reason = "one case per BuildBar test")]
fn reason_and_left_out_note_cases() {
    use fm_tarpack::session::BuildBlockedReason as R;
    let reason = |s: &fm_tarpack::session::TarpackSession| text::build_reason(s);
    // BuildBar.test.tsx: "disabled reason: %s".
    assert_eq!(
        reason(&session_with(Some(R::NoManifest), None, 0, 0)).as_deref(),
        Some("Open a manifest first")
    );
    let withheld = manifest_with(vec![], vec![diag("x", 3, 1)], vec![], true);
    assert_eq!(
        reason(&session_with(Some(R::NoEntries), Some(withheld), 0, 0)).as_deref(),
        Some("No files can be built until the manifest errors are fixed")
    );
    let failed = manifest_with(
        vec![],
        vec![],
        vec![failure(1, Some("bad1"), None, 5)],
        false,
    );
    assert_eq!(
        reason(&session_with(Some(R::NoEntries), Some(failed), 0, 0)).as_deref(),
        Some("Every file in the manifest has errors")
    );
    let none = manifest_with(vec![], vec![], vec![], false);
    assert_eq!(
        reason(&session_with(Some(R::NoEntries), Some(none), 0, 0)).as_deref(),
        Some("This manifest lists no files")
    );
    let one_missing = manifest_with(
        vec![with_status("a", EntryStatus::Missing), entry("b")],
        vec![],
        vec![],
        false,
    );
    assert_eq!(
        reason(&session_with(
            Some(R::EntriesNotReady),
            Some(one_missing),
            0,
            2
        ))
        .as_deref(),
        Some("1 chosen file is missing from disk")
    );
    let two_missing = manifest_with(
        vec![
            with_status("a", EntryStatus::Missing),
            with_status("b", EntryStatus::Missing),
        ],
        vec![],
        vec![],
        false,
    );
    assert_eq!(
        reason(&session_with(
            Some(R::EntriesNotReady),
            Some(two_missing),
            0,
            2
        ))
        .as_deref(),
        Some("2 chosen files are missing from disk")
    );
    let two = manifest_with(vec![entry("a"), entry("b")], vec![], vec![], false);
    assert_eq!(
        reason(&session_with(Some(R::NothingLoaded), Some(two), 0, 2)).as_deref(),
        Some("Add at least one file to build an archive")
    );
    let ready = manifest_with(
        vec![with_status("a", EntryStatus::Ready)],
        vec![],
        vec![],
        false,
    );
    assert_eq!(
        reason(&session_with(Some(R::NoOutput), Some(ready.clone()), 1, 1)).as_deref(),
        Some("Choose where to save the archive")
    );
    assert_eq!(reason(&session_with(None, Some(ready), 1, 1)), None);

    // The left-out note.
    let left_out = |s: &fm_tarpack::session::TarpackSession| text::left_out_note(s);
    let ready_entries = || vec![with_status("a", EntryStatus::Ready)];
    assert_eq!(
        left_out(&session_with(None, None, 0, 0)),
        None,
        "no manifest"
    );
    assert_eq!(
        left_out(&session_with(
            None,
            Some(manifest_with(vec![], vec![], vec![], false)),
            0,
            0
        )),
        None,
        "no entries"
    );
    assert_eq!(
        left_out(&session_with(
            None,
            Some(manifest_with(ready_entries(), vec![], vec![], false)),
            1,
            1
        )),
        None,
        "nothing to say"
    );
    let one_failed = manifest_with(
        ready_entries(),
        vec![],
        vec![failure(1, Some("bad1"), None, 5)],
        false,
    );
    assert_eq!(
        left_out(&session_with(None, Some(one_failed), 1, 1)).as_deref(),
        Some("1 file will be left out; errors will be listed after the build")
    );
    let two_failed = manifest_with(
        ready_entries(),
        vec![],
        vec![
            failure(1, Some("bad1"), None, 5),
            failure(2, Some("bad2"), None, 10),
        ],
        false,
    );
    assert_eq!(
        left_out(&session_with(None, Some(two_failed), 1, 1)).as_deref(),
        Some("2 files will be left out; errors will be listed after the build")
    );
    let two_errors = manifest_with(
        ready_entries(),
        vec![diag("a", 3, 1), diag("b", 3, 1)],
        vec![],
        false,
    );
    assert_eq!(
        left_out(&session_with(None, Some(two_errors), 1, 1)).as_deref(),
        Some("Builds with 2 manifest errors")
    );
    let one_error = manifest_with(ready_entries(), vec![diag("a", 3, 1)], vec![], false);
    assert_eq!(
        left_out(&session_with(None, Some(one_error), 1, 1)).as_deref(),
        Some("Builds with 1 manifest error")
    );
    let partial = manifest_with(
        vec![with_status("a", EntryStatus::Ready), entry("b"), entry("c")],
        vec![],
        vec![],
        false,
    );
    assert_eq!(
        left_out(&session_with(None, Some(partial), 1, 3)).as_deref(),
        Some("Partial archive: 1 of 3 files; 2 without a file will be left out")
    );
    // nothingLoaded with errors: the reason and the note both show.
    let nothing = manifest_with(
        vec![entry("a")],
        vec![],
        vec![failure(1, Some("bad1"), None, 5)],
        false,
    );
    let s = session_with(Some(R::NothingLoaded), Some(nothing), 0, 1);
    assert_eq!(
        reason(&s).as_deref(),
        Some("Add at least one file to build an archive")
    );
    assert_eq!(
        left_out(&s).as_deref(),
        Some("1 file will be left out; errors will be listed after the build")
    );
}

#[test]
fn display_paths_split_like_today() {
    // pathParts.test.ts (fileName, displayFolder) and the overwrite copy.
    for (path, name, folder) in [
        (
            "C:\\out\\gateway.tar.zst",
            "gateway.tar.zst",
            Some("C:\\out"),
        ),
        ("C:\\gateway.tar", "gateway.tar", Some("C:\\")),
        ("C:/a/b.tar", "b.tar", Some("C:/a")),
        ("\\\\srv\\share\\x.tar", "x.tar", Some("\\\\srv\\share\\")),
        (
            "\\\\srv\\share\\d\\x.tar",
            "x.tar",
            Some("\\\\srv\\share\\d"),
        ),
        ("x.tar", "x.tar", None),
        ("/x.tar", "x.tar", Some("/")),
        (
            "C:\\out\\caf\u{e9}\u{fffd}\\x.tar",
            "x.tar",
            Some("C:\\out\\caf\u{e9}\u{fffd}"),
        ),
    ] {
        assert_eq!(text::file_name(path), name, "{path}");
        assert_eq!(text::display_folder(path).as_deref(), folder, "{path}");
    }
    assert_eq!(
        text::overwrite_title("C:\\out\\gateway.tar.zst"),
        "Replace gateway.tar.zst?"
    );
    assert_eq!(
        text::overwrite_body("C:\\out\\gateway.tar.zst"),
        "A file with this name already exists in C:\\out. Replacing it can't be undone."
    );
    assert_eq!(
        text::overwrite_body("gateway.tar"),
        "A file with this name already exists. Replacing it can't be undone."
    );
}

#[test]
fn progress_text_matches_today() {
    let ev = |phase, id: Option<&str>, done, total| Progress {
        phase,
        entry_id: id.map(str::to_string),
        bytes_done: done,
        bytes_total: total,
    };
    let w = ev(BuildPhase::Writing, Some("gateway"), 40, 100);
    assert_eq!(
        text::progress_label(None, None),
        "Step 1 of 2 \u{b7} Writing"
    );
    assert_eq!(
        text::progress_label(Some(&w), Some("gateway.conf")),
        "Step 1 of 2 \u{b7} Writing gateway.conf"
    );
    assert_eq!(text::progress_percent(Some(&w)), 40);
    assert_eq!(
        text::progress_value_text(Some(&w), Some("gateway.conf")),
        "Writing gateway.conf, 40%"
    );
    let v = ev(BuildPhase::Verifying, None, 10, 100);
    assert_eq!(
        text::progress_label(Some(&v), None),
        "Step 2 of 2 \u{b7} Verifying"
    );
    assert_eq!(text::progress_value_text(Some(&v), None), "Verifying, 10%");
    let end = ev(BuildPhase::Verifying, None, 100, 100);
    assert_eq!(
        text::progress_label(Some(&end), Some("x")),
        "Finishing\u{2026}"
    );
    // The last event of the writing phase names no file even with a
    // remembered one.
    let done = ev(BuildPhase::Writing, None, 100, 100);
    assert_eq!(
        text::progress_label(Some(&done), Some("gateway.conf")),
        "Step 1 of 2 \u{b7} Writing"
    );
    assert_eq!(
        text::progress_percent(Some(&ev(BuildPhase::Writing, None, 0, 0))),
        100
    );
    assert!((text::progress_fraction(Some(&w)) - 0.4).abs() < 1e-6);
    assert!((text::progress_fraction(None)).abs() < f32::EPSILON);
}

#[test]
fn shortcut_list_matches_today() {
    let ts = include_str!("../../../../desktop/src/tools/tarpack/ShortcutsHelp.tsx");
    for row in text::SHORTCUTS {
        assert!(
            ts.contains(&format!("action: \"{}\"", row.action))
                && ts.contains(&format!("when: \"{}\"", row.when)),
            "{} is not in ShortcutsHelp.tsx",
            row.action
        );
        assert_eq!(
            row.dev_only,
            row.action == "Edit in editor",
            "only Edit is developer-only"
        );
    }
    assert!(
        text::SHORTCUTS
            .iter()
            .all(|r| r.keys.iter().all(|c| !c.contains(&"Alt"))),
        "the developer-mode chord is not listed"
    );
    assert_eq!(text::SHORTCUTS.len(), 9);
}

#[test]
fn build_text_matches_ts_sources() {
    let sources = [
        include_str!("../../../../desktop/src/tools/tarpack/BuildBar.tsx"),
        include_str!("../../../../desktop/src/tools/tarpack/BuildResult.tsx"),
        include_str!("../../../../desktop/src/tools/tarpack/BuildProgress.tsx"),
        include_str!("../../../../desktop/src/tools/tarpack/BuildReport.tsx"),
        include_str!("../../../../desktop/src/tools/tarpack/FormatPicker.tsx"),
        include_str!("../../../../desktop/src/tools/tarpack/reportText.ts"),
        TARPACK_VIEW_TSX,
    ]
    .join("\n");
    let in_sources = |s: &str| {
        assert!(
            sources.contains(&ts(s)) || sources.contains(s),
            "`{s}` is not in the TypeScript sources"
        );
    };
    for s in [
        text::OUTPUT_LABEL,
        text::OUTPUT_NOT_CHOSEN,
        text::CREATE_ARCHIVE,
        text::SHOW_ERRORS,
        text::SHOW_ERRORS_HINT,
        text::CREATE_ARCHIVE_HINT,
        text::FORMAT_LABEL,
        text::BUILDING_ARCHIVE,
        text::FINISHING,
        text::VERIFYING_ANNOUNCEMENT,
        text::OVERWRITE_CONFIRM,
        text::RESULT_DISMISS,
        text::SHOW_IN_FOLDER,
        text::SHA256_LABEL,
        text::EXTRACT_LABEL,
        text::EXTRACT_HINT,
        text::NORMALIZED_LABEL,
        text::NO_CRLF,
        text::COPY,
        text::COPY_SHA256,
        text::COPY_EXTRACTION_COMMAND,
        text::COPY_REPORT,
        text::COPIED,
        text::REPORT_LABEL,
        text::MANIFEST_ERRORS_LABEL,
        "Left out of the archive",
        "Warnings",
        "Not included (no file chosen):",
        "Format changed to",
        "to match the file name.",
        "Partial archive:",
        "without a file will be left out",
        "will be left out; errors will be listed after the build",
        "Add at least one file to build an archive",
        "Choose where to save the archive",
        "missing from disk",
        "An archive holds",
    ]
    .into_iter()
    .filter(|s| *s != "An archive holds")
    {
        in_sources(s);
    }
    for kind in [
        ArchiveFormat::Tar,
        ArchiveFormat::TarGz,
        ArchiveFormat::TarZst,
        ArchiveFormat::TarXz,
    ] {
        let name = text::format_name(kind);
        assert!(
            sources.contains(&format!("\"{name}\"")),
            "format name {name}"
        );
    }
    assert!(sources.contains("Step {phase === \"writing\" ? 1 : 2} of 2"));
    assert!(sources.contains("A file with this name already exists"));
    assert!(sources.contains("Replacing it can't be undone."));
    assert!(sources.contains("Replace ${fileName("));
}

// ---- the build side: model ----

/// (done, phase, file, label, value) at a redraw of the progress.
type Seen = (u64, BuildPhase, Option<String>, String, String);

/// A model over the fixture `fixture`, loaded and idle.
fn start_fixture(
    cx: &mut TestAppContext,
    fixture: TarpackFixture,
) -> (Entity<TarpackModel>, FakePlatform, FixtureTarpack) {
    let backend = FixtureTarpack::from_fixture(fixture);
    let (model, fake) = start(cx, &backend);
    idle(cx, &model);
    (model, fake, backend)
}

/// Runs until the build has settled (nothing runs and no confirmation is
/// open or none is wanted).
fn settled(cx: &mut TestAppContext, model: &Entity<TarpackModel>) {
    wait_for(cx, |cx| read(cx, model, |m| !m.building() && !m.loading()));
}

fn result_of(cx: &mut TestAppContext, model: &Entity<TarpackModel>) -> Option<BuildOutcome> {
    read(cx, model, |m| m.result().cloned())
}

fn build(cx: &mut TestAppContext, model: &Entity<TarpackModel>) {
    act(cx, model, TarpackModel::build);
}

fn run_shortcut(cx: &mut TestAppContext, model: &Entity<TarpackModel>, s: Shortcut) {
    let window = cx.add_empty_window();
    window.update(|window, cx| model.update(cx, |m, cx| m.run_shortcut(s, window, cx)));
}

fn shortcut_on(cx: &mut TestAppContext, model: &Entity<TarpackModel>, s: Shortcut) -> bool {
    cx.read(|cx| model.read(cx).shortcut(s, cx))
}

fn save_spec_of(fake: &FakePlatform) -> SaveSpec {
    fake.calls()
        .into_iter()
        .find_map(|c| match c {
            PlatformCall::PickSave(spec) => Some(spec),
            _ => None,
        })
        .expect("a save dialog was shown")
}

fn session_of(
    cx: &mut TestAppContext,
    model: &Entity<TarpackModel>,
) -> fm_tarpack::session::TarpackSession {
    read(cx, model, |m| m.session().cloned().expect("a session"))
}

const ONE_FILE: &str =
    "version = 1\nname = \"t\"\n\n[[file]]\nid = \"a\"\nsource = \"a.bin\"\ndir = \"/opt\"\n";

/// A real model over a temporary directory with the entry `a` assigned and
/// the output chosen (`dir/out/a.tar`), all through the model. Returns the
/// model, the fake platform, the output path and the manifest path.
fn real_ready(
    cx: &mut TestAppContext,
    dir: &Path,
) -> (Entity<TarpackModel>, FakePlatform, PathBuf, PathBuf) {
    let manifest = write_manifest(dir, "m.toml", ONE_FILE);
    let source = dir.join("a.bin");
    fs::write(&source, vec![7_u8; 300_000]).expect("write");
    fs::create_dir_all(dir.join("out")).expect("mkdir");
    let out = dir.join("out").join("a.tar");
    let (model, fake) = start_boxed(cx, Box::new(real(dir)));
    idle(cx, &model);
    fake.answer_pick(pick(&manifest));
    act(cx, &model, TarpackModel::open);
    wait_for(cx, |cx| manifest_path(cx, &model).is_some());
    fake.answer_pick(pick(&source));
    act(cx, &model, |m, cx| m.browse("a", cx));
    wait_for(cx, |cx| {
        status_of(cx, &model, "a").is_some_and(|s| s.1 == EntryStatus::Ready)
    });
    fake.answer_save(pick(&out));
    act(cx, &model, TarpackModel::choose_output);
    wait_for(cx, |cx| read(cx, &model, TarpackModel::can_build));
    (model, fake, out, manifest)
}

// ---- output and format ----

#[gpui::test]
fn choose_output_starts_in_output_folder_with_name(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::from_fixture(TarpackFixture::Ready)
        .with_output("/releases/gateway-9.tar.zst");
    let (model, fake) = start(cx, &backend);
    idle(cx, &model);
    act(cx, &model, TarpackModel::choose_output);
    wait_for_calls(cx, &fake, 1);
    let spec = save_spec_of(&fake);
    assert_eq!(spec.start_dir.as_deref(), Some(Path::new("/releases")));
    assert_eq!(
        spec.suggested_name.as_deref(),
        Some("gateway-9.tar.zst".as_ref())
    );
    assert_eq!(spec.title, None);

    // No output yet: the manifest's folder, and the session's suggested name.
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest = write_manifest(dir.path(), "m.toml", GATEWAY);
    let (model, fake) = start_boxed(cx, Box::new(real(dir.path())));
    idle(cx, &model);
    fake.answer_pick(pick(&manifest));
    act(cx, &model, TarpackModel::open);
    wait_for(cx, |cx| manifest_path(cx, &model).is_some());
    let suggested = session_of(cx, &model)
        .suggested_output_name
        .expect("suggested");
    act(cx, &model, TarpackModel::choose_output);
    wait_for(cx, |_| {
        fake.calls()
            .iter()
            .any(|c| matches!(c, PlatformCall::PickSave(_)))
    });
    let spec = save_spec_of(&fake);
    assert_eq!(spec.start_dir.as_deref(), Some(dir.path()));
    assert_eq!(spec.suggested_name, Some(suggested.into()));
}

#[gpui::test]
fn choose_output_needs_a_manifest_and_a_name(cx: &mut TestAppContext) {
    let (model, fake, _) = start_fixture(cx, TarpackFixture::Empty);
    act(cx, &model, TarpackModel::choose_output);
    cx.run_until_parked();
    assert_eq!(fake.calls(), Vec::<PlatformCall>::new(), "nothing to name");
}

#[gpui::test]
fn choose_output_passes_format_filter(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::from_fixture(TarpackFixture::Ready)
        .with_output("/releases/gateway.tar.zst");
    let (model, fake) = start(cx, &backend);
    idle(cx, &model);
    act(cx, &model, TarpackModel::choose_output);
    wait_for_calls(cx, &fake, 1);
    let spec = save_spec_of(&fake);
    assert_eq!(
        spec.filters,
        [FileFilter {
            name: "zstd archive (.tar.zst)".to_string(),
            patterns: vec!["*.zst".to_string()],
        }]
    );
    assert_eq!(spec.default_extension.as_deref(), Some("zst"));
    // Another format, another filter.
    act(cx, &model, |m, cx| m.set_format(ArchiveFormat::Tar, cx));
    wait_for(cx, |cx| session_of(cx, &model).format == ArchiveFormat::Tar);
    act(cx, &model, TarpackModel::choose_output);
    wait_for_calls(cx, &fake, 2);
    let PlatformCall::PickSave(spec) = fake.calls()[1].clone() else {
        panic!("a save dialog");
    };
    assert_eq!(
        spec.filters,
        [FileFilter {
            name: "tar archive (.tar)".to_string(),
            patterns: vec!["*.tar".to_string()],
        }]
    );
    assert_eq!(spec.default_extension.as_deref(), Some("tar"));
    assert_eq!(
        spec.suggested_name.as_deref(),
        Some("gateway.tar".as_ref()),
        "the output kept its name, now with the new extension"
    );
}

#[gpui::test]
fn output_name_changing_format_sets_notice(cx: &mut TestAppContext) {
    let (model, fake, backend) = start_fixture(cx, TarpackFixture::Ready);
    // Same format: no notice.
    fake.answer_save(pick(Path::new("/o/other.tar.zst")));
    act(cx, &model, TarpackModel::choose_output);
    wait_for(cx, |cx| {
        session_of(cx, &model).output_path.as_deref() == Some("/o/other.tar.zst")
    });
    assert_eq!(
        read(cx, &model, |m| m.format_notice().map(String::from)),
        None
    );
    // A name that carries another format switches it and says so.
    fake.answer_save(pick(Path::new("/o/typed.tar.gz")));
    act(cx, &model, TarpackModel::choose_output);
    wait_for(cx, |cx| {
        session_of(cx, &model).format == ArchiveFormat::TarGz
    });
    let session = session_of(cx, &model);
    assert_eq!(session.output_path.as_deref(), Some("/o/typed.tar.gz"));
    assert_eq!(
        read(cx, &model, |m| m.format_notice().map(String::from)).as_deref(),
        Some("Format changed to gzip to match the file name.")
    );
    assert!(backend
        .calls()
        .contains(&Call::SetOutput(PathBuf::from("/o/typed.tar.gz"))));
    // A cancelled dialog changes neither.
    act(cx, &model, TarpackModel::choose_output);
    cx.run_until_parked();
    assert_eq!(
        read(cx, &model, |m| m.format_notice().map(String::from)).as_deref(),
        Some("Format changed to gzip to match the file name.")
    );
}

#[gpui::test]
fn notice_cleared_by_later_change(cx: &mut TestAppContext) {
    let notice = |cx: &mut TestAppContext, model: &Entity<TarpackModel>| {
        read(cx, model, |m| m.format_notice().map(String::from))
    };
    let (model, fake, _) = start_fixture(cx, TarpackFixture::Ready);
    let switch = |cx: &mut TestAppContext, name: &str, format: ArchiveFormat| {
        fake.answer_save(pick(Path::new(name)));
        act(cx, &model, TarpackModel::choose_output);
        wait_for(cx, |cx| {
            session_of(cx, &model).format == format
                && read(cx, &model, |m| m.format_notice().is_some())
        });
    };
    // A later format change.
    switch(cx, "/o/a.tar.gz", ArchiveFormat::TarGz);
    act(cx, &model, |m, cx| m.set_format(ArchiveFormat::TarXz, cx));
    assert_eq!(notice(cx, &model), None, "cleared at once");
    wait_for(cx, |cx| {
        session_of(cx, &model).format == ArchiveFormat::TarXz
    });
    assert_eq!(notice(cx, &model), None);
    // A later output change that keeps the format.
    switch(cx, "/o/b.tar.gz", ArchiveFormat::TarGz);
    fake.answer_save(pick(Path::new("/o/c.tar.gz")));
    act(cx, &model, TarpackModel::choose_output);
    wait_for(cx, |cx| {
        session_of(cx, &model).output_path.as_deref() == Some("/o/c.tar.gz")
    });
    assert_eq!(notice(cx, &model), None);
    // A new build.
    switch(cx, "/o/d.tar", ArchiveFormat::Tar);
    build(cx, &model);
    assert_eq!(notice(cx, &model), None);
    settled(cx, &model);
    // A reload.
    switch(cx, "/o/e.tar.xz", ArchiveFormat::TarXz);
    act(cx, &model, TarpackModel::reload);
    wait_for(cx, |cx| notice(cx, &model).is_none());
    // Activation.
    switch(cx, "/o/f.tar.gz", ArchiveFormat::TarGz);
    act(cx, &model, TarpackModel::activated);
    assert_eq!(notice(cx, &model), None);
}

#[gpui::test]
fn output_dialog_failure_is_the_dialog_banner(cx: &mut TestAppContext) {
    let (model, fake, _) = start_fixture(cx, TarpackFixture::Ready);
    fake.answer_save(Err(PickError::Failed {
        detail: "no shell".to_string(),
    }));
    act(cx, &model, TarpackModel::choose_output);
    wait_for(cx, |cx| error_of(cx, &model).is_some());
    assert_eq!(error_of(cx, &model).expect("banner").copy(), PICK_FAILED);
}

// ---- building ----

#[gpui::test]
fn build_runs_off_thread_with_ordered_progress(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let (model, _fake, out, _) = real_ready(cx, dir.path());
    let seen: Rc<RefCell<Vec<Progress>>> = Rc::default();
    let sink = seen.clone();
    cx.update(|cx| {
        cx.observe(&model, move |model, cx| {
            if let Some(p) = model.read(cx).progress() {
                if sink.borrow().last() != Some(p) {
                    sink.borrow_mut().push(p.clone());
                }
            }
        })
        .detach();
    });
    build(cx, &model);
    assert!(read(cx, &model, TarpackModel::building), "set at once");
    assert!(!read(cx, &model, TarpackModel::can_build));
    settled(cx, &model);
    let Some(BuildOutcome::Ok(summary)) = result_of(cx, &model) else {
        panic!("the build succeeded: {:?}", result_of(cx, &model));
    };
    assert_eq!(PathBuf::from(&summary.path), out);
    assert!(out.is_file(), "the archive is on disk");
    // Events are in order: writing before verifying, the bytes of a phase
    // never go down, and the last one delivered is the final event.
    let events = seen.borrow().clone();
    assert!(events.len() >= 4, "{events:?}");
    let first_verifying = events
        .iter()
        .position(|p| p.phase == BuildPhase::Verifying)
        .expect("a verifying phase");
    assert!(events[..first_verifying]
        .iter()
        .all(|p| p.phase == BuildPhase::Writing));
    assert!(events[first_verifying..]
        .iter()
        .all(|p| p.phase == BuildPhase::Verifying));
    for phase in events.chunk_by(|a, b| a.phase == b.phase) {
        assert!(phase.windows(2).all(|w| w[0].bytes_done <= w[1].bytes_done));
        let last = phase.last().expect("not empty");
        assert_eq!(
            (last.bytes_done, last.entry_id.as_deref()),
            (last.bytes_total, None)
        );
    }
    // Settled: the progress is gone and the controls are back.
    assert_eq!(read(cx, &model, |m| m.progress().cloned()), None);
    assert!(read(cx, &model, TarpackModel::can_build));
}

#[gpui::test]
fn build_holds_without_blocking_the_ui(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::from_fixture(TarpackFixture::Building);
    let (model, _fake) = start(cx, &backend);
    idle(cx, &model);
    build(cx, &model);
    wait_for(cx, |cx| {
        read(cx, &model, |m| {
            m.progress().is_some_and(|p| p.bytes_done > 0)
        })
    });
    // The write is held on its thread; the UI thread is free.
    assert!(read(cx, &model, TarpackModel::building));
    let progress = read(cx, &model, |m| m.progress().cloned()).expect("progress");
    assert_eq!(progress.phase, BuildPhase::Writing);
    assert_eq!(text::progress_percent(Some(&progress)), 42);
    assert_eq!(
        read(cx, &model, |m| m.progress_file().map(String::from)).as_deref(),
        Some("gateway.conf"),
        "the second ready entry's file"
    );
    // Controls are locked while it runs.
    let calls = backend.calls().len();
    act(cx, &model, TarpackModel::reload);
    act(cx, &model, TarpackModel::open);
    act(cx, &model, |m, cx| m.clear("gateway-bin", cx));
    act(cx, &model, |m, cx| m.set_format(ArchiveFormat::Tar, cx));
    act(cx, &model, TarpackModel::choose_output);
    act(cx, &model, TarpackModel::build);
    dropped(cx, &model, &["C:\\x\\gateway"]);
    cx.run_until_parked();
    assert_eq!(backend.calls().len(), calls, "no backend call was made");
    assert_eq!(
        hooks_target(cx, &model),
        disabled("A build is running"),
        "drops are refused"
    );
    backend.release_build();
    settled(cx, &model);
    assert!(matches!(result_of(cx, &model), Some(BuildOutcome::Ok(_))));
}

#[gpui::test]
fn progress_names_the_last_file_of_the_phase(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::from_fixture(TarpackFixture::Ready);
    let gate = Gate::new();
    let ev = |phase, id: Option<&str>, done| Progress {
        phase,
        entry_id: id.map(str::to_string),
        bytes_done: done,
        bytes_total: 100,
    };
    backend.script_build(
        vec![
            ev(BuildPhase::Writing, Some("gateway-bin"), 10),
            ev(BuildPhase::Writing, Some("gateway-conf"), 30),
        ],
        Some(&gate),
        vec![
            // Long after the last event, so the coalescer forwards it.
            ev(BuildPhase::Writing, None, 60),
            ev(BuildPhase::Verifying, None, 5),
        ],
    );
    let (model, _fake) = start(cx, &backend);
    idle(cx, &model);
    let seen: Rc<RefCell<Vec<Seen>>> = Rc::default();
    let sink = seen.clone();
    cx.update(|cx| {
        cx.observe(&model, move |model, cx| {
            let m = model.read(cx);
            if let Some(p) = m.progress() {
                let row = (
                    p.bytes_done,
                    p.phase,
                    m.progress_file().map(String::from),
                    m.progress_label(),
                    m.progress_value_text(),
                );
                if sink.borrow().last() != Some(&row) {
                    sink.borrow_mut().push(row);
                }
            }
        })
        .detach();
    });
    build(cx, &model);
    wait_for(cx, |cx| {
        read(cx, &model, |m| {
            m.progress().is_some_and(|p| p.bytes_done == 30)
        })
    });
    std::thread::sleep(Duration::from_millis(120));
    gate.open();
    settled(cx, &model);
    let rows = seen.borrow().clone();
    let at = |done: u64| {
        rows.iter()
            .find(|r| r.0 == done)
            .cloned()
            .expect("an event")
    };
    let (_, _, file, label, value) = at(10);
    assert_eq!(file.as_deref(), Some("gateway"));
    assert_eq!(label, "Step 1 of 2 \u{b7} Writing gateway");
    assert_eq!(value, "Writing gateway, 10%");
    let (_, _, file, _, _) = at(30);
    assert_eq!(file.as_deref(), Some("gateway.conf"));
    // An event with no file keeps the last one named, but the value names
    // the current event's file only.
    let (_, _, file, label, value) = at(60);
    assert_eq!(file.as_deref(), Some("gateway.conf"));
    assert_eq!(label, "Step 1 of 2 \u{b7} Writing gateway.conf");
    assert_eq!(value, "Writing, 60%");
    // A new phase forgets it.
    let (_, phase, file, label, _) = at(5);
    assert_eq!((phase, file), (BuildPhase::Verifying, None));
    assert_eq!(label, "Step 2 of 2 \u{b7} Verifying");
}

#[gpui::test]
fn progress_channel_closing_before_result_is_not_a_failure(cx: &mut TestAppContext) {
    // The job's result arrives after its channel closed, as in a normal
    // build: nothing special is shown and the result is kept.
    let (model, _fake, _backend) = start_fixture(cx, TarpackFixture::Result);
    build(cx, &model);
    settled(cx, &model);
    assert!(matches!(result_of(cx, &model), Some(BuildOutcome::Ok(_))));
    assert!(error_of(cx, &model).is_none());
    assert!(read(cx, &model, |m| m.progress().is_none()));

    let (tx, rx) = async_channel::unbounded::<Progress>();
    let (finish, finished) = async_channel::bounded::<u8>(1);
    drop(tx);
    let job = cx.update(|cx| {
        cx.foreground_executor().spawn(async move {
            Ok::<_, crate::jobs::JobPanicked>(finished.recv().await.unwrap_or(0))
        })
    });
    let done: Rc<RefCell<Option<u8>>> = Rc::default();
    let sink = done.clone();
    model.update(cx, |_, cx| {
        cx.spawn(async move |this, cx| {
            let got = TarpackModel::watch_build(this, rx, job, cx).await;
            *sink.borrow_mut() = got.and_then(Result::ok);
        })
        .detach();
    });
    cx.run_until_parked();
    assert_eq!(*done.borrow(), None, "the job is still waited for");
    finish.try_send(7).expect("room");
    cx.run_until_parked();
    assert_eq!(*done.borrow(), Some(7));
}

#[gpui::test]
fn build_refused_unless_can_build(cx: &mut TestAppContext) {
    let (model, _fake, backend) = start_fixture(cx, TarpackFixture::Partial);
    let (empty, _fake2, backend2) = start_fixture(cx, TarpackFixture::Loaded);
    assert!(
        read(cx, &model, TarpackModel::can_build),
        "a partial build is allowed"
    );
    assert!(!read(cx, &empty, TarpackModel::can_build));
    build(cx, &empty);
    cx.run_until_parked();
    assert!(!backend2.calls().contains(&Call::BeginBuild));
    assert!(!read(cx, &empty, TarpackModel::building));
    build(cx, &model);
    settled(cx, &model);
    assert!(backend.calls().contains(&Call::BeginBuild));
}

#[gpui::test]
fn build_success_sets_result_and_announces(cx: &mut TestAppContext) {
    let (model, _fake, backend) = start_fixture(cx, TarpackFixture::Result);
    let before = generation(cx, &model);
    build(cx, &model);
    settled(cx, &model);
    let Some(BuildOutcome::Ok(summary)) = result_of(cx, &model) else {
        panic!("a result");
    };
    assert_eq!(summary.path, OUTPUT_PATH);
    assert_eq!(summary.built_ids.len(), 5);
    assert_eq!(say(cx, &model).as_deref(), Some("Created gateway.tar.zst."));
    assert_eq!(generation(cx, &model), before + 1, "announced once");
    assert_eq!(hint(cx, &model), Some(FocusHint::CreateArchive));
    assert!(read(cx, &model, TarpackModel::can_build));
    assert_eq!(
        backend
            .calls()
            .into_iter()
            .filter(|c| matches!(c, Call::BuildRun { .. }))
            .collect::<Vec<_>>(),
        [Call::BuildRun { overwrite: false }]
    );
    // Dismissing hides it and hints at the button.
    act(cx, &model, TarpackModel::dismiss_result);
    assert_eq!(result_of(cx, &model), None);
    assert_eq!(hint(cx, &model), Some(FocusHint::CreateArchive));
    // A partial build says what it left out.
    let (partial, _fake, _) = start_fixture(cx, TarpackFixture::ResultPartial);
    build(cx, &partial);
    settled(cx, &partial);
    assert_eq!(
        say(cx, &partial).as_deref(),
        Some("Created gateway.tar.zst. 2 files were not included because no file was chosen.")
    );
}

#[gpui::test]
fn existing_output_asks_and_confirm_overwrites(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let (model, _fake, out, _) = real_ready(cx, dir.path());
    fs::write(&out, b"old archive").expect("write");
    build(cx, &model);
    settled(cx, &model);
    // Nothing was written; the confirmation names the file.
    assert_eq!(fs::read(&out).expect("read"), b"old archive");
    assert_eq!(result_of(cx, &model), None);
    assert_eq!(say(cx, &model), None, "asking announces nothing");
    assert_eq!(
        read(cx, &model, |m| m.confirming_overwrite().map(String::from)),
        Some(display(&out))
    );
    assert_eq!(text::overwrite_title(&display(&out)), "Replace a.tar?");
    assert!(
        !read(cx, &model, TarpackModel::can_build),
        "locked while asking"
    );
    assert!(!read(cx, &model, TarpackModel::building));
    assert_eq!(hint(cx, &model), None);
    // Replace builds again, with overwrite.
    act(cx, &model, TarpackModel::confirm_overwrite);
    assert!(read(cx, &model, TarpackModel::building));
    assert_eq!(
        read(cx, &model, |m| m.confirming_overwrite().map(String::from)),
        None
    );
    settled(cx, &model);
    assert!(matches!(result_of(cx, &model), Some(BuildOutcome::Ok(_))));
    assert_ne!(fs::read(&out).expect("read"), b"old archive");
    assert!(fs::metadata(&out).expect("meta").len() > 300_000);
    assert_eq!(hint(cx, &model), Some(FocusHint::CreateArchive));
}

#[gpui::test]
fn replace_asks_once_with_the_fixture(cx: &mut TestAppContext) {
    let (model, _fake, backend) = start_fixture(cx, TarpackFixture::Replace);
    build(cx, &model);
    settled(cx, &model);
    assert_eq!(
        read(cx, &model, |m| m.confirming_overwrite().map(String::from)).as_deref(),
        Some(OUTPUT_PATH)
    );
    assert_eq!(
        text::overwrite_body(OUTPUT_PATH),
        "A file with this name already exists in C:\\Releases. Replacing it can't be undone."
    );
    // The drop overlay says why it is off.
    assert_eq!(hooks_target(cx, &model), disabled(DROP_BLOCKED_BY_DIALOG));
    act(cx, &model, TarpackModel::confirm_overwrite);
    settled(cx, &model);
    assert!(matches!(result_of(cx, &model), Some(BuildOutcome::Ok(_))));
    assert_eq!(
        backend
            .calls()
            .into_iter()
            .filter(|c| matches!(c, Call::BuildRun { .. }))
            .collect::<Vec<_>>(),
        [
            Call::BuildRun { overwrite: false },
            Call::BuildRun { overwrite: true }
        ]
    );
    // Confirming with nothing to confirm does nothing.
    act(cx, &model, TarpackModel::confirm_overwrite);
    cx.run_until_parked();
    assert!(!read(cx, &model, TarpackModel::building));
}

#[gpui::test]
fn cancel_overwrite_writes_nothing(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let (model, _fake, out, _) = real_ready(cx, dir.path());
    fs::write(&out, b"old archive").expect("write");
    build(cx, &model);
    settled(cx, &model);
    assert!(read(cx, &model, |m| m.confirming_overwrite().is_some()));
    act(cx, &model, TarpackModel::cancel_overwrite);
    cx.run_until_parked();
    assert_eq!(
        fs::read(&out).expect("read"),
        b"old archive",
        "bytes unchanged"
    );
    assert_eq!(
        read(cx, &model, |m| m.confirming_overwrite().map(String::from)),
        None
    );
    assert_eq!(result_of(cx, &model), None);
    assert!(!read(cx, &model, TarpackModel::building));
    assert!(read(cx, &model, TarpackModel::can_build), "unlocked again");
    assert_eq!(hint(cx, &model), Some(FocusHint::CreateArchiveAfterDialog));
    // No temp file was left behind.
    let names: Vec<_> = fs::read_dir(dir.path().join("out"))
        .expect("dir")
        .map(|e| e.expect("entry").file_name())
        .collect();
    assert_eq!(names, [std::ffi::OsString::from("a.tar")]);
}

#[gpui::test]
fn changed_on_disk_during_build_marks_changed(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let (model, _fake, out, manifest) = real_ready(cx, dir.path());
    fs::write(&manifest, format!("{ONE_FILE}\n# edited\n")).expect("write");
    let before = generation(cx, &model);
    build(cx, &model);
    settled(cx, &model);
    assert!(read(cx, &model, TarpackModel::changed_on_disk));
    assert_eq!(result_of(cx, &model), None, "no result");
    assert!(!out.exists(), "nothing was written");
    assert_eq!(say(cx, &model).as_deref(), Some(text::CHANGED_ON_DISK));
    assert_eq!(generation(cx, &model), before + 1);
    // Announced once.
    build(cx, &model);
    settled(cx, &model);
    assert_eq!(generation(cx, &model), before + 1);
}

#[gpui::test]
fn build_error_is_result_with_message(cx: &mut TestAppContext) {
    let (model, _fake, backend) = start_fixture(cx, TarpackFixture::Ready);
    // The write fails after it has reported progress: the progress stops and
    // the error is the result.
    backend.script_build(
        vec![
            Progress {
                phase: BuildPhase::Writing,
                entry_id: Some("gateway-bin".to_string()),
                bytes_done: 10,
                bytes_total: 100,
            },
            Progress {
                phase: BuildPhase::Verifying,
                entry_id: Some("gateway-bin".to_string()),
                bytes_done: 50,
                bytes_total: 100,
            },
        ],
        None,
        Vec::new(),
    );
    backend.fail_build(TarpackError::for_entry(
        TarpackErrorKind::SourceMissing,
        "C:\\Builds\\gateway\\gateway-conf is gone",
        "gateway-conf",
    ));
    build(cx, &model);
    settled(cx, &model);
    let Some(BuildOutcome::Err(e)) = result_of(cx, &model) else {
        panic!("a failure");
    };
    assert_eq!(e.kind, TarpackErrorKind::SourceMissing);
    assert_eq!(
        e.message, "C:\\Builds\\gateway\\gateway-conf is gone",
        "the detail"
    );
    let source = entries(cx, &model)
        .into_iter()
        .find(|e| e.view.id == "gateway-conf")
        .map(|e| e.view.source)
        .expect("entry");
    assert_eq!(
        say(cx, &model).unwrap_or_default(),
        format!("{source} is no longer at its assigned location.")
    );
    assert!(
        read(cx, &model, TarpackModel::can_build),
        "free to try again"
    );
    assert_eq!(
        read(cx, &model, |m| m.progress().cloned()),
        None,
        "the progress stopped"
    );
    // An error from the checks before the write is a result too.
    let (model, _fake, backend) = start_fixture(cx, TarpackFixture::Ready);
    backend.fail_next(err(TarpackErrorKind::NoOutput));
    build(cx, &model);
    settled(cx, &model);
    assert!(matches!(
        result_of(cx, &model),
        Some(BuildOutcome::Err(TarpackError {
            kind: TarpackErrorKind::NoOutput,
            ..
        }))
    ));
    assert_eq!(
        say(cx, &model).as_deref(),
        Some("Choose where to save the archive.")
    );
}

#[gpui::test]
fn build_with_errors_reports_left_out(cx: &mut TestAppContext) {
    // A canBuild session that has errors builds with no dialog; the failed
    // entry is left out and the announcement says so.
    let (model, _fake, backend) = start_fixture(cx, TarpackFixture::Errors);
    assert!(read(cx, &model, TarpackModel::can_build));
    build(cx, &model);
    settled(cx, &model);
    assert_eq!(
        read(cx, &model, |m| m.confirming_overwrite().map(String::from)),
        None
    );
    let Some(BuildOutcome::Ok(summary)) = result_of(cx, &model) else {
        panic!("a result");
    };
    assert_eq!(summary.left_out.len(), 1);
    assert_eq!(summary.error_count, 2);
    assert_eq!(summary.built_ids.len(), 4);
    assert_eq!(
        say(cx, &model).as_deref(),
        Some("Created gateway.tar.zst. 1 file was left out because of errors.")
    );
    assert_eq!(
        backend
            .calls()
            .into_iter()
            .filter(|c| matches!(c, Call::BuildRun { .. }))
            .collect::<Vec<_>>(),
        [Call::BuildRun { overwrite: false }],
        "built once"
    );
}

#[gpui::test]
fn output_and_format_errors_show_the_banner(cx: &mut TestAppContext) {
    let (model, fake, backend) = start_fixture(cx, TarpackFixture::Ready);
    backend.fail_next(err(TarpackErrorKind::Io));
    fake.answer_save(pick(Path::new("/o/x.tar.zst")));
    act(cx, &model, TarpackModel::choose_output);
    wait_for(cx, |cx| error_of(cx, &model).is_some());
    assert_eq!(
        error_of(cx, &model).expect("banner").copy(),
        "A file could not be read or written."
    );
    assert_eq!(
        session_of(cx, &model).output_path.as_deref(),
        Some(OUTPUT_PATH),
        "the output is unchanged"
    );
    act(cx, &model, TarpackModel::dismiss_error);
    backend.fail_next(err(TarpackErrorKind::NoManifest));
    act(cx, &model, |m, cx| m.set_format(ArchiveFormat::Tar, cx));
    wait_for(cx, |cx| error_of(cx, &model).is_some());
    assert_eq!(
        error_of(cx, &model).expect("banner").copy(),
        "Open a manifest first."
    );
    assert_eq!(session_of(cx, &model).format, ArchiveFormat::TarZst);
}

#[test]
fn format_labels_match_today() {
    // FormatPicker.test.tsx: options in the given order with the session's
    // extensions.
    let options = fm_tarpack::session::ArchiveFormatOption::all();
    let labels: Vec<_> = options.iter().map(text::format_option_label).collect();
    assert_eq!(
        labels,
        [
            "tar (.tar)",
            "gzip (.tar.gz)",
            "zstd (.tar.zst)",
            "xz (.tar.xz)"
        ]
    );
    assert_eq!(
        text::format_filter_name(&options[2]),
        "zstd archive (.tar.zst)"
    );
    assert_eq!(
        text::format_changed_notice(ArchiveFormat::TarXz),
        "Format changed to xz to match the file name."
    );
}

#[gpui::test]
fn panicked_build_aborts_and_reports(cx: &mut TestAppContext) {
    let (model, _fake, backend) = start_fixture(cx, TarpackFixture::Ready);
    backend.panic_in_build();
    build(cx, &model);
    settled(cx, &model);
    let Some(BuildOutcome::Unexpected { detail }) = result_of(cx, &model) else {
        panic!("an unexpected failure");
    };
    assert!(detail.contains("the build exploded"), "{detail}");
    assert_eq!(say(cx, &model).as_deref(), Some(text::UNEXPECTED));
    let calls = backend.calls();
    assert!(calls.contains(&Call::AbortBuild));
    assert!(!calls.contains(&Call::FinishBuild));
    assert!(!read(cx, &model, TarpackModel::building));
    assert!(read(cx, &model, TarpackModel::can_build));
    assert!(say(cx, &model).is_some());
}

#[gpui::test]
fn a_panic_in_the_checks_or_the_end_aborts_too(cx: &mut TestAppContext) {
    for command in [Command::BeginBuild, Command::FinishBuild] {
        let (model, _fake, backend) = start_fixture(cx, TarpackFixture::Ready);
        backend.panic_on(command);
        build(cx, &model);
        settled(cx, &model);
        assert!(
            matches!(result_of(cx, &model), Some(BuildOutcome::Unexpected { .. })),
            "{command:?}"
        );
        assert!(backend.calls().contains(&Call::AbortBuild), "{command:?}");
        assert!(!read(cx, &model, TarpackModel::building));
        // The session is fetched again: the backend's state is unknown.
        wait_for(cx, |_| session_fetches(&backend) >= 2);
    }
}

// ---- the result: reveal and copy ----

#[gpui::test]
fn reveal_uses_held_path(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let (model, fake, out, _) = real_ready(cx, dir.path());
    // A name that is not valid Unicode cannot round-trip through a display
    // string: the held path is what is revealed.
    #[cfg(unix)]
    let odd = {
        use std::os::unix::ffi::OsStrExt as _;
        let odd = dir
            .path()
            .join("out")
            .join(std::ffi::OsStr::from_bytes(b"o\xffd.tar"));
        fake.answer_save(pick(&odd));
        act(cx, &model, TarpackModel::choose_output);
        wait_for(cx, |cx| {
            session_of(cx, &model)
                .output_path
                .is_some_and(|p| p.contains('\u{fffd}'))
        });
        drop(out);
        odd
    };
    #[cfg(not(unix))]
    let odd = out;
    build(cx, &model);
    settled(cx, &model);
    assert!(matches!(result_of(cx, &model), Some(BuildOutcome::Ok(_))));
    act(cx, &model, TarpackModel::reveal_output);
    wait_for(cx, |_| {
        fake.calls()
            .iter()
            .any(|c| matches!(c, PlatformCall::Reveal(_)))
    });
    assert_eq!(
        fake.calls().last(),
        Some(&PlatformCall::Reveal(odd.clone())),
        "the exact path, not a display string"
    );
    assert!(odd.is_file());
    assert!(error_of(cx, &model).is_none());
}

#[gpui::test]
fn reveal_failure_shows_opener_failed(cx: &mut TestAppContext) {
    let (model, fake, _) = start_fixture(cx, TarpackFixture::Result);
    build(cx, &model);
    settled(cx, &model);
    fake.answer_open(Err(OpenError {
        detail: "Explorer is not running".to_string(),
    }));
    act(cx, &model, TarpackModel::reveal_output);
    wait_for(cx, |cx| error_of(cx, &model).is_some());
    let error = error_of(cx, &model).expect("banner");
    assert_eq!(error.copy(), "Windows could not open it.");
    assert_eq!(error.detail(), "Explorer is not running");
    assert_eq!(
        fake.calls(),
        [PlatformCall::Reveal(PathBuf::from(OUTPUT_PATH))]
    );
}

#[gpui::test]
fn cancelled_open_shows_no_banner(cx: &mut TestAppContext) {
    // The platform answers `Ok(())` when the user cancels the system dialog.
    let (model, fake, _) = start_fixture(cx, TarpackFixture::Result);
    build(cx, &model);
    settled(cx, &model);
    fake.answer_open(Ok(()));
    act(cx, &model, TarpackModel::reveal_output);
    wait_for_calls(cx, &fake, 1);
    cx.run_until_parked();
    assert!(error_of(cx, &model).is_none());
}

#[gpui::test]
fn reveal_before_any_build_is_no_output(cx: &mut TestAppContext) {
    let (model, fake, _) = start_fixture(cx, TarpackFixture::Ready);
    act(cx, &model, TarpackModel::reveal_output);
    wait_for(cx, |cx| error_of(cx, &model).is_some());
    assert_eq!(fake.calls(), Vec::<PlatformCall>::new());
    assert_eq!(
        error_of(cx, &model).expect("banner").copy(),
        "Choose where to save the archive."
    );
}

#[gpui::test]
fn copy_report_copies_report_text(cx: &mut TestAppContext) {
    let (model, fake, _) = start_fixture(cx, TarpackFixture::ResultPartial);
    act(cx, &model, TarpackModel::copy_report);
    assert_eq!(fake.calls(), Vec::<PlatformCall>::new(), "no result yet");
    build(cx, &model);
    settled(cx, &model);
    let Some(BuildOutcome::Ok(summary)) = result_of(cx, &model) else {
        panic!("a result");
    };
    act(cx, &model, TarpackModel::copy_report);
    act(cx, &model, TarpackModel::copy_sha256);
    let report = text::report_text(&summary);
    assert!(report.starts_with("Created gateway.tar.zst with 2 files not included"));
    assert_eq!(
        fake.calls(),
        [
            PlatformCall::CopyText(report),
            PlatformCall::CopyText(summary.sha256_hex.clone()),
        ]
    );
}

#[gpui::test]
fn extraction_command_only_in_dev_mode(cx: &mut TestAppContext) {
    cx.update(DevMode::init);
    let (model, fake, _) = start_fixture(cx, TarpackFixture::DevResult);
    let command = |cx: &mut TestAppContext| {
        cx.read(|cx| model.read(cx).extraction_command(cx).map(String::from))
    };
    assert_eq!(command(cx), None, "no result yet");
    build(cx, &model);
    settled(cx, &model);
    assert_eq!(command(cx), None, "developer mode is off");
    act(cx, &model, TarpackModel::copy_extraction_command);
    assert_eq!(fake.calls(), Vec::<PlatformCall>::new());
    cx.update(DevMode::toggle);
    assert_eq!(
        command(cx).as_deref(),
        Some("tar --zstd --no-overwrite-dir -xpPf gateway.tar.zst")
    );
    act(cx, &model, TarpackModel::copy_extraction_command);
    assert_eq!(
        fake.calls(),
        [PlatformCall::CopyText(
            "tar --zstd --no-overwrite-dir -xpPf gateway.tar.zst".to_string()
        )]
    );
}

#[gpui::test]
fn dev_mode_off_hides_extraction_command(cx: &mut TestAppContext) {
    cx.update(DevMode::init);
    let (model, fake, _) = start_fixture(cx, TarpackFixture::DevResult);
    build(cx, &model);
    settled(cx, &model);
    cx.update(DevMode::toggle);
    let redraws: Rc<RefCell<usize>> = Rc::default();
    let counter = redraws.clone();
    cx.update(|cx| {
        cx.observe(&model, move |_, _| *counter.borrow_mut() += 1)
            .detach();
    });
    cx.update(DevMode::toggle);
    cx.run_until_parked();
    assert_eq!(*redraws.borrow(), 1, "the model redraws at once");
    assert_eq!(
        cx.read(|cx| model.read(cx).extraction_command(cx).map(String::from)),
        None
    );
    act(cx, &model, TarpackModel::copy_extraction_command);
    assert_eq!(fake.calls(), Vec::<PlatformCall>::new());
}

// ---- shortcuts ----

#[gpui::test]
fn shortcuts_gating_matches_today(cx: &mut TestAppContext) {
    use Shortcut::{Build, Edit, Escape, Open, Reload, ShowErrors};
    cx.update(DevMode::init);
    let all = [Open, Reload, Edit, Build, ShowErrors, Escape];
    let row = |cx: &mut TestAppContext, model: &Entity<TarpackModel>| {
        all.map(|s| shortcut_on(cx, model, s))
    };
    // No manifest: only Open (Ctrl+O works with or without a manifest).
    let (empty, _, _) = start_fixture(cx, TarpackFixture::Empty);
    assert_eq!(row(cx, &empty), [true, false, false, false, false, false]);
    cx.update(DevMode::toggle);
    assert_eq!(
        row(cx, &empty),
        [true, false, false, false, false, false],
        "Ctrl+E needs a manifest even in developer mode"
    );
    cx.update(DevMode::toggle);
    // A manifest nothing is assigned to: reload, not build; no errors.
    let (loaded, _, _) = start_fixture(cx, TarpackFixture::Loaded);
    assert_eq!(row(cx, &loaded), [true, true, false, false, false, false]);
    cx.update(DevMode::toggle);
    assert_eq!(
        row(cx, &loaded),
        [true, true, true, false, false, false],
        "Ctrl+E edits in developer mode only"
    );
    cx.update(DevMode::toggle);
    // Ready, with errors: a canBuild session that has errors builds, with
    // no dialog; F8 needs errors.
    let (errors, _, _) = start_fixture(cx, TarpackFixture::Errors);
    assert_eq!(row(cx, &errors), [true, true, false, true, true, false]);
    let (ready, _, _) = start_fixture(cx, TarpackFixture::Ready);
    assert_eq!(row(cx, &ready), [true, true, false, true, false, false]);
    // A running build: everything is off, the reload chords included.
    let (building, _, backend) = start_fixture(cx, TarpackFixture::Building);
    build(cx, &building);
    assert_eq!(row(cx, &building), [false; 6]);
    cx.update(DevMode::toggle);
    assert_eq!(row(cx, &building), [false; 6]);
    cx.update(DevMode::toggle);
    wait_for(cx, |cx| read(cx, &building, |m| m.progress().is_some()));
    let calls = backend.calls().len();
    for s in all {
        run_shortcut(cx, &building, s);
    }
    cx.run_until_parked();
    assert_eq!(
        backend
            .calls()
            .iter()
            .skip(calls)
            .filter(|c| matches!(c, Call::Reload | Call::BeginBuild))
            .count(),
        0
    );
    backend.release_build();
    settled(cx, &building);
    assert_eq!(
        row(cx, &building),
        [true, true, false, true, false, true],
        "a result shows"
    );
    // The replace dialog open: everything is off.
    let (replace, _, _) = start_fixture(cx, TarpackFixture::Replace);
    build(cx, &replace);
    settled(cx, &replace);
    assert_eq!(row(cx, &replace), [false; 6]);
    act(cx, &replace, TarpackModel::cancel_overwrite);
    assert_eq!(row(cx, &replace), [true, true, false, true, false, false]);
}

#[gpui::test]
fn shortcuts_run_what_they_gate(cx: &mut TestAppContext) {
    cx.update(DevMode::init);
    cx.update(DevMode::toggle);
    let (model, fake, backend) = start_fixture(cx, TarpackFixture::Errors);
    run_shortcut(cx, &model, Shortcut::Reload);
    wait_for(cx, |_| backend.calls().contains(&Call::Reload));
    run_shortcut(cx, &model, Shortcut::Edit);
    wait_for(cx, |_| {
        fake.calls()
            .iter()
            .any(|c| matches!(c, PlatformCall::OpenWithSystem(_)))
    });
    act(cx, &model, |m, cx| m.set_errors_expanded(false, cx));
    run_shortcut(cx, &model, Shortcut::ShowErrors);
    assert!(read(cx, &model, TarpackModel::errors_expanded));
    assert_eq!(hint(cx, &model), Some(FocusHint::Report));
    run_shortcut(cx, &model, Shortcut::Build);
    settled(cx, &model);
    assert!(backend.calls().contains(&Call::BeginBuild));
    assert!(result_of(cx, &model).is_some());
    fake.answer_pick(pick(Path::new("/m/other.toml")));
    run_shortcut(cx, &model, Shortcut::Open);
    wait_for(cx, |_| {
        backend.calls().iter().any(|c| matches!(c, Call::Open(_)))
    });
}

#[gpui::test]
fn escape_dismisses_drop_result_then_build_result(cx: &mut TestAppContext) {
    let (model, _fake, _) = start_fixture(cx, TarpackFixture::Ready);
    build(cx, &model);
    settled(cx, &model);
    dropped(cx, &model, &["C:\\x\\gateway"]);
    idle(cx, &model);
    assert!(drop_lines_of(cx, &model).is_some() && result_of(cx, &model).is_some());
    act(cx, &model, |m, cx| m.set_errors_expanded(true, cx));
    run_shortcut(cx, &model, Shortcut::Escape);
    assert!(drop_lines_of(cx, &model).is_none(), "the drop result first");
    assert!(result_of(cx, &model).is_some());
    assert_eq!(hint(cx, &model), Some(FocusHint::AfterResult));
    run_shortcut(cx, &model, Shortcut::Escape);
    assert!(result_of(cx, &model).is_none(), "then the build result");
    assert_eq!(hint(cx, &model), Some(FocusHint::CreateArchive));
    assert!(
        read(cx, &model, TarpackModel::errors_expanded),
        "never collapses the error report"
    );
    assert!(!shortcut_on(cx, &model, Shortcut::Escape), "nothing left");
    run_shortcut(cx, &model, Shortcut::Escape);
    assert_eq!(hint(cx, &model), None);
}

#[gpui::test]
fn escape_ignored_while_shortcuts_open(cx: &mut TestAppContext) {
    let (model, _fake, _) = start_fixture(cx, TarpackFixture::Result);
    build(cx, &model);
    settled(cx, &model);
    act(cx, &model, |m, _| m.set_shortcuts_open(true));
    assert!(!shortcut_on(cx, &model, Shortcut::Escape));
    run_shortcut(cx, &model, Shortcut::Escape);
    assert!(result_of(cx, &model).is_some(), "the popover took the key");
    act(cx, &model, |m, _| m.set_shortcuts_open(false));
    assert!(shortcut_on(cx, &model, Shortcut::Escape));
    run_shortcut(cx, &model, Shortcut::Escape);
    assert!(result_of(cx, &model).is_none());
}

#[gpui::test]
fn reload_clears_the_build_result(cx: &mut TestAppContext) {
    let (model, _fake, _) = start_fixture(cx, TarpackFixture::Result);
    build(cx, &model);
    settled(cx, &model);
    let _ = hint(cx, &model);
    act(cx, &model, TarpackModel::reload);
    wait_for(cx, |cx| result_of(cx, &model).is_none());
    assert_eq!(
        hint(cx, &model),
        Some(FocusHint::Reload),
        "focus was in the result"
    );
}

// ---- activating ----

#[gpui::test]
fn activated_keeps_running_build_and_result(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::from_fixture(TarpackFixture::Building);
    let (model, _fake) = start(cx, &backend);
    idle(cx, &model);
    build(cx, &model);
    wait_for(cx, |cx| read(cx, &model, |m| m.progress().is_some()));
    act(cx, &model, TarpackModel::activated);
    cx.run_until_parked();
    assert!(
        read(cx, &model, TarpackModel::building),
        "the build continues"
    );
    assert!(read(cx, &model, |m| m.progress().is_some()));
    backend.release_build();
    // The result lands while the tool is not current: it is kept.
    settled(cx, &model);
    assert!(result_of(cx, &model).is_some());
    act(cx, &model, TarpackModel::activated);
    idle(cx, &model);
    assert!(result_of(cx, &model).is_some(), "kept until dismissed");
    assert_eq!(say(cx, &model), None, "the announcement is forgotten");
}

#[gpui::test]
fn activated_clears_notice_and_confirmation(cx: &mut TestAppContext) {
    let (model, fake, backend) = start_fixture(cx, TarpackFixture::Replace);
    fake.answer_save(pick(Path::new("/o/a.tar.xz")));
    act(cx, &model, TarpackModel::choose_output);
    wait_for(cx, |cx| read(cx, &model, |m| m.format_notice().is_some()));
    build(cx, &model);
    settled(cx, &model);
    assert!(read(cx, &model, |m| m.confirming_overwrite().is_some()));
    act(cx, &model, TarpackModel::activated);
    assert!(read(cx, &model, |m| m.confirming_overwrite().is_none()));
    assert!(read(cx, &model, |m| m.format_notice().is_none()));
    assert_eq!(say(cx, &model), None);
    assert!(
        read(cx, &model, TarpackModel::can_build),
        "nothing is blocked"
    );
    idle(cx, &model);
    assert_eq!(
        backend
            .calls()
            .iter()
            .filter(|c| matches!(c, Call::BuildRun { overwrite: true }))
            .count(),
        0,
        "nothing was replaced"
    );
}

// ---- fixtures ----

#[gpui::test]
fn fixtures_build_each_build_state(cx: &mut TestAppContext) {
    use TarpackFixture as F;
    // tarpack-buildbar and tarpack-shortcuts are states of the manifest side.
    let (bar, _, _) = start_fixture(cx, F::BuildBar);
    assert!(read(cx, &bar, TarpackModel::can_build));
    assert_eq!(read(cx, &bar, TarpackModel::build_reason), None);
    let (shortcuts, _, _) = start_fixture(cx, F::Shortcuts);
    assert_eq!(
        read(cx, &shortcuts, TarpackModel::build_reason).as_deref(),
        Some("Add at least one file to build an archive")
    );

    // tarpack-building: progress at once, then held; release finishes it.
    let (building, _, backend) = start_fixture(cx, F::Building);
    build(cx, &building);
    wait_for(cx, |cx| {
        read(cx, &building, |m| {
            m.progress().is_some_and(|p| p.bytes_done > 0)
        })
    });
    assert_eq!(
        read(cx, &building, |m| m
            .progress()
            .map(|p| text::progress_percent(Some(p)))),
        Some(42)
    );
    assert!(read(cx, &building, TarpackModel::building));
    backend.release_build();
    settled(cx, &building);
    assert!(matches!(
        result_of(cx, &building),
        Some(BuildOutcome::Ok(_))
    ));

    // tarpack-replace: the first answer is OutputExists.
    let (replace, _, _) = start_fixture(cx, F::Replace);
    build(cx, &replace);
    settled(cx, &replace);
    assert_eq!(
        read(cx, &replace, |m| m.confirming_overwrite().map(String::from)).as_deref(),
        Some(OUTPUT_PATH)
    );
    assert_eq!(result_of(cx, &replace), None);

    // tarpack-result, -result-partial, -dev-result: full and partial summaries.
    for (fixture, built, not_included, normalized) in [
        (F::Result, 5, 0, 1),
        (F::DevResult, 5, 0, 1),
        (F::ResultPartial, 3, 2, 1),
    ] {
        let (model, _, _) = start_fixture(cx, fixture);
        build(cx, &model);
        settled(cx, &model);
        let Some(BuildOutcome::Ok(s)) = result_of(cx, &model) else {
            panic!("{fixture:?}: a result");
        };
        assert_eq!(
            (
                s.built_ids.len(),
                s.not_loaded.len(),
                s.normalized_entries.len()
            ),
            (built, not_included, normalized),
            "{fixture:?}"
        );
        assert_eq!(s.path, OUTPUT_PATH);
        assert_eq!(s.format, ArchiveFormat::TarZst);
        assert_eq!(s.sha256_hex.len(), 64);
        assert_eq!(
            s.extract_command,
            "tar --zstd --no-overwrite-dir -xpPf gateway.tar.zst"
        );
    }
    // tarpack-dev: the ready state; developer mode is the scenario's.
    let (dev, _, _) = start_fixture(cx, F::Dev);
    assert!(read(cx, &dev, TarpackModel::can_build));
}

#[gpui::test]
fn open_recent_past_the_end_is_recent_changed(cx: &mut TestAppContext) {
    let backend = FixtureTarpack::loaded();
    let (model, _fake) = start(cx, &backend);
    idle(cx, &model);
    assert_eq!(read(cx, &model, |m| m.recent().len()), 3);
    // The backend's list is shorter than the copy the model shows.
    backend.set_recent(&[MANIFEST_PATH]);
    act(cx, &model, |m, cx| m.open_recent(2, cx));
    idle(cx, &model);
    wait_for(cx, |cx| error_of(cx, &model).is_some());
    assert_eq!(error_of(cx, &model), Some(ViewError::RecentChanged));
    assert_eq!(read(cx, &model, |m| m.recent().to_vec()), [MANIFEST_PATH]);
    assert_eq!(manifest_path(cx, &model).as_deref(), Some(MANIFEST_PATH));

    // The real backend: no such position is a decline, not an error.
    let dir = tempfile::tempdir().expect("tempdir");
    let a = write_manifest(dir.path(), "a.toml", GATEWAY);
    let mut real_backend = real(dir.path());
    real_backend.open(&a).expect("open");
    assert_eq!(real_backend.open_recent(5, "x").expect("no error"), None);
}
