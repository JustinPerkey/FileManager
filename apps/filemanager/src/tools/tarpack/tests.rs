//! The Tar Packager model's tests (manifest side): the copy, loading and
//! restoring, opening, reloading, the watcher, the manifest's errors,
//! assignment by browse, clear and drop, and the fixtures. Model tests run
//! against [`FixtureTarpack`] and [`FakePlatform`]; those that need the disk
//! run [`RealTarpack`] over a temporary directory.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use fm_core::AppDirs;
use fm_tarpack::manifest::EntryView;
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
    default_start_dir, DropState, FocusHint, Origin, TarpackModel, ViewError, LOOKUP_THREADS,
    SKELETON_DELAY,
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
            F::Recent | F::Loaded | F::Drag | F::Dropped => {
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
            F::Partial => {
                assert_eq!(statuses(EntryStatus::Ready), 3);
                assert_eq!(statuses(EntryStatus::Unassigned), 2);
                assert_eq!(session.output_path.as_deref(), Some(OUTPUT_PATH));
            }
            F::Changed | F::Ready => {
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
