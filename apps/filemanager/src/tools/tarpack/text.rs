//! The Tar Packager's copy, ported verbatim from the Tauri view
//! (`errorMessages.ts`, `DropResult.tsx`, `ManifestHeader.tsx`,
//! `ManifestErrors.tsx`, `DiagnosticList.tsx`, `FailureList.tsx` and the
//! sentences in `TarpackView.tsx`). Pure functions and constants: the views
//! render them, the model announces them.
//!
//! The build side's strings (the build bar, progress, results, the report, the
//! overwrite dialog, the shortcuts help) are at the end: `BuildBar.tsx`,
//! `BuildProgress.tsx`, `BuildResult.tsx`, `BuildReport.tsx`, `FormatPicker.tsx`,
//! `reportText.ts`, `ShortcutsHelp.tsx`, `pathParts.ts` and the build
//! sentences of `TarpackView.tsx`.

use std::fmt::Write as _;

use fm_tarpack::archive::{BuildPhase, BuildSummary, Progress};
use fm_tarpack::format::ArchiveFormat;
use fm_tarpack::manifest::{Diagnostic, EntryFailure};
use fm_tarpack::session::{
    ArchiveFormatOption, BuildBlockedReason, SessionEntry, SessionManifest, TarpackError,
    TarpackErrorKind, TarpackSession,
};
use fm_tarpack::sources::{Ambiguity, DropOutcome, EntryStatus, UnmatchedReason};

use super::model::Origin;

/// Copy for an error nothing is known about (a panicked backend call).
pub const UNEXPECTED: &str = "Something went wrong. See Details.";

/// The tool's title, and the heading of its empty state.
pub const TITLE: &str = "Tar Packager";

/// Hidden text of the loading state.
pub const LOADING: &str = "Loading manifest";

// ---- errors ----

/// What an entry-specific message calls an entry it cannot name.
const A_FILE: &str = "A file";

/// The user-facing sentence for `kind`. `source` is the entry's file name
/// (`SessionEntry::source`) for the kinds that name one; without it they say
/// "A file". An exhaustive `match`: a kind added in Rust fails to compile
/// until it has copy.
#[must_use]
pub fn error_message(kind: TarpackErrorKind, source: Option<&str>) -> String {
    use TarpackErrorKind as K;
    let file = source.unwrap_or(A_FILE);
    match kind {
        K::NoManifest => "Open a manifest first.".to_string(),
        K::ManifestUnreadable => {
            "The manifest could not be read. Check that the file still exists and that you can open it."
                .to_string()
        }
        K::NoEntries => {
            "There are no files that can be built. Fix the manifest errors first.".to_string()
        }
        K::ManifestChangedOnDisk => {
            "The manifest changed on disk. Reload it, then build again.".to_string()
        }
        K::UnknownEntry => {
            "That file is no longer in the manifest. Reload and try again.".to_string()
        }
        K::NotAFile => format!("{file}: the chosen path is not a file."),
        K::NoOutput => "Choose where to save the archive.".to_string(),
        K::EntriesNotReady => format!("{file} still needs a location."),
        K::OutputExists => "A file with this name already exists.".to_string(),
        K::PathExists => {
            "A file already exists there. Choose a new name; the example never replaces a file."
                .to_string()
        }
        K::SourceMissing => format!("{file} is no longer at its assigned location."),
        K::SourceUnreadable => format!("{file} could not be read."),
        K::SourceChanged => format!(
            "{file} changed while the archive was being written. Nothing was saved. Try again."
        ),
        K::VerifyFailed => {
            "The archive failed its check after writing, so it was not saved. Any existing file was left unchanged. Try again."
                .to_string()
        }
        K::BuildInProgress => "A build is already running.".to_string(),
        K::OpenerFailed => "Windows could not open it.".to_string(),
        K::Io => "A file could not be read or written.".to_string(),
    }
}

/// The entry's file name for `error`, when the error names an entry that is
/// in `entries`.
#[must_use]
pub fn error_source<'a>(error: &TarpackError, entries: &'a [SessionEntry]) -> Option<&'a str> {
    let id = error.entry_id.as_deref()?;
    entries
        .iter()
        .find(|e| e.view.id == id)
        .map(|e| e.view.source.as_str())
}

/// The sentence for a command error; `error.message` is the technical detail,
/// shown separately.
#[must_use]
pub fn error_copy(error: &TarpackError, entries: &[SessionEntry]) -> String {
    error_message(error.kind, error_source(error, entries))
}

/// The banner when a recent manifest was chosen from a list that had changed.
pub const RECENT_CHANGED: &str = "The recent list changed. Choose the manifest again.";

/// The error of a drop whose session kept changing while its files were
/// matched.
pub const DROP_STALE: &str =
    "the session changed while the dropped files were being matched; drop them again";

// ---- sentences ----

fn plural(n: usize, one: &'static str, many: &'static str) -> &'static str {
    if n == 1 {
        one
    } else {
        many
    }
}

/// The sentence announced after a load or reload. `prev_errors` is the error
/// count of the manifest shown before. Nothing for an `Other` origin.
#[must_use]
pub fn manifest_sentence(
    session: &TarpackSession,
    prev_errors: u32,
    origin: Origin,
) -> Option<String> {
    let m = session.manifest.as_ref()?;
    if origin == Origin::Other {
        return None;
    }
    if m.error_count > 0 {
        let n = m.error_count as usize;
        let mut text = format!("{} has {n} {}.", m.name, plural(n, "error", "errors"));
        if m.entries_withheld {
            text.push_str(" No files can be built until the manifest errors are fixed.");
        } else if !m.failed_entries.is_empty() {
            let n = m.failed_entries.len();
            let _ = write!(
                text,
                " {n} {} be left out of the archive.",
                plural(n, "file will", "files will")
            );
        }
        return Some(text);
    }
    if origin == Origin::Reload && prev_errors > 0 {
        return Some(format!("{} reloaded. No errors.", m.name));
    }
    None
}

/// Announced once when the file watcher sees the manifest change.
pub const CHANGED_ON_DISK: &str = "The manifest changed on disk.";

/// The label of the changed-on-disk banner's action.
pub const CHANGED_ACTION: &str = "Reload";

/// Announced when a drop has been matching for a second.
pub const MATCHING: &str = "Matching dropped files\u{2026}";

// ---- the empty state ----

pub const EMPTY_LEAD: &str = "Open a manifest to list the files this package needs.";
pub const EMPTY_HINT: &str =
    "A manifest is a TOML file that lists each file, its Linux path, and its permissions.";
pub const OPEN_MANIFEST: &str = "Open manifest\u{2026}";
pub const CREATE_FROM_EXAMPLE: &str = "Create from example\u{2026}";

// ---- the manifest header ----

pub const OPEN: &str = "Open\u{2026}";
pub const RECENT: &str = "Recent";
pub const RECENT_MENU_LABEL: &str = "Recent manifests";
pub const RELOAD: &str = "Reload";
pub const EDIT_IN_EDITOR: &str = "Edit in editor";
/// The label of the disclosure under an error banner that reveals its
/// technical detail ([`super::model::ViewError::detail`]).
pub const DETAILS: &str = "Details";

// ---- the errors section ----

/// The heading above the error report: "3 errors in this manifest".
#[must_use]
pub fn errors_heading(error_count: u32) -> String {
    let n = error_count as usize;
    format!("{n} {} in this manifest", plural(n, "error", "errors"))
}

/// What the errors mean for the build.
#[must_use]
pub fn consequence(m: &SessionManifest) -> String {
    if m.entries_withheld {
        return "No files can be listed or built until the manifest errors are fixed.".to_string();
    }
    match m.failed_entries.len() {
        0 => "Every file is listed and can still be built.".to_string(),
        1 => "1 file is left out of the list and the archive until it's fixed.".to_string(),
        n => format!("{n} files are left out of the list and the archive until they're fixed."),
    }
}

/// The toggle's label.
#[must_use]
pub fn errors_toggle(expanded: bool) -> &'static str {
    if expanded {
        "Hide errors"
    } else {
        "Show errors"
    }
}

pub const WHOLE_MANIFEST: &str = "Whole manifest";
pub const WHOLE_MANIFEST_ERRORS: &str = "Whole manifest errors";
pub const FILES_WITH_ERRORS: &str = "Files with errors";
pub const WARNINGS_LABEL: &str = "Warnings";

/// The warnings disclosure's summary: "2 warnings".
#[must_use]
pub fn warnings_summary(count: usize) -> String {
    format!("{count} {}", plural(count, "warning", "warnings"))
}

// ---- diagnostics and failed entries ----

/// A diagnostic's position for assistive technology: "Line 3, column 1: ".
#[must_use]
pub fn diagnostic_position_spoken(d: &Diagnostic) -> String {
    format!("Line {}, column {}: ", d.line, d.col)
}

/// A diagnostic's position as shown: "3:1".
#[must_use]
pub fn diagnostic_position(d: &Diagnostic) -> String {
    format!("{}:{}", d.line, d.col)
}

/// The name of a failed entry: its id, or "Entry #2" when it has none.
#[must_use]
pub fn failure_name(f: &EntryFailure) -> String {
    f.id.clone()
        .unwrap_or_else(|| format!("Entry #{}", f.index))
}

/// The label of a failed entry's diagnostics list.
#[must_use]
pub fn failure_errors_label(f: &EntryFailure) -> String {
    format!("Errors in {}", failure_name(f))
}

/// The header table's literal, shown in mono.
pub const FAILURE_TABLE: &str = "[[file]]";
/// Before the source name in a failed entry's location line.
pub const FAILURE_SOURCE: &str = "source";
/// Between the table literal and the line number.
pub const FAILURE_ON_LINE: &str = "on line";

// ---- drops ----

/// The drop overlay's label when a drop is taken.
pub const DROP_LABEL: &str = "Drop files or folders to match them to the manifest";

pub const REASON_NO_MANIFEST: &str = "Open a manifest first";
pub const REASON_FIX_ERRORS: &str = "Fix the manifest errors first. No files can be matched yet.";
pub const REASON_NO_FILES: &str = "This manifest lists no files";
pub const REASON_BUILDING: &str = "A build is running";
pub const REASON_PENDING: &str = "Still matching the last drop";

/// What blocks a drop apart from the manifest: the model sets it while a
/// build runs or its overwrite confirmation is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropBlocker {
    /// A build runs.
    Building,
    /// The overwrite confirmation is open.
    Confirming,
}

/// Whether a drop is taken now; `Err` is the reason shown on the overlay.
///
/// # Errors
/// The reason (one of the `REASON_*` sentences, or the app's dialog
/// sentence for an open confirmation).
pub fn drop_availability(
    manifest: Option<&SessionManifest>,
    blocker: Option<DropBlocker>,
    pending: bool,
) -> Result<(), &'static str> {
    let Some(manifest) = manifest else {
        return Err(REASON_NO_MANIFEST);
    };
    if manifest.entries.is_empty() {
        return Err(
            if manifest.entries_withheld || !manifest.failed_entries.is_empty() {
                REASON_FIX_ERRORS
            } else {
                REASON_NO_FILES
            },
        );
    }
    match blocker {
        Some(DropBlocker::Building) => return Err(REASON_BUILDING),
        Some(DropBlocker::Confirming) => return Err(crate::app::text::DROP_BLOCKED_BY_DIALOG),
        None => {}
    }
    if pending {
        return Err(REASON_PENDING);
    }
    Ok(())
}

/// The drop result's accessible name, and the pending line's text.
pub const DROP_RESULT_LABEL: &str = "Drop result";
/// The dismiss button's accessible name.
pub const DISMISS_DROP_RESULT: &str = "Dismiss drop result";
/// The disclosure that lists every path.
pub const FULL_PATHS: &str = "Full paths";

/// How much one tone of line means; the view maps it to a colour and an icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Ok,
    Warn,
    Muted,
}

/// What a result line is about; the view maps it to an icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Matched,
    Unmatched(UnmatchedReason),
    /// The note that failed entries cannot be matched.
    Hint,
    Ambiguous,
    /// Nothing happened.
    Empty,
}

/// A parenthesised note after an item's label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemNote {
    /// "(2 files)".
    Count(usize),
    Text(String),
}

/// One name on a result line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DropItem {
    pub label: String,
    /// The tooltip: the full path, or the candidates.
    pub title: String,
    /// Shown in the mono face (a target path).
    pub mono: bool,
    pub note: Option<ItemNote>,
}

/// The "Full paths" disclosure's content for one line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FullPaths {
    Paths {
        lead: String,
        paths: Vec<String>,
    },
    Entries {
        lead: String,
        entries: Vec<FullEntry>,
    },
}

/// An ambiguous entry and every file that fits it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FullEntry {
    pub label: String,
    pub mono: bool,
    pub candidates: Vec<String>,
}

/// One line of a drop result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DropLine {
    pub kind: LineKind,
    pub tone: Tone,
    /// The leading number, shown in the numeric face; none for the hint and
    /// the empty line.
    pub count: Option<usize>,
    /// The text after the count, with the colon when items follow.
    pub lead: String,
    /// At most [`MAX_NAMES`].
    pub items: Vec<DropItem>,
    /// Items beyond the cap.
    pub more: usize,
    pub full: Option<FullPaths>,
}

/// Names listed on one line; the rest are "and N more".
pub const MAX_NAMES: usize = 8;

const HINT: &str = "Files for entries with errors can't be matched until those errors are fixed.";
const EMPTY: &str = "Nothing was matched";

/// Reasons in display order.
const ORDER: [UnmatchedReason; 7] = [
    UnmatchedReason::NoEntry,
    UnmatchedReason::AlreadyAssigned,
    UnmatchedReason::FolderNoMatch,
    UnmatchedReason::LinkNotFollowed,
    UnmatchedReason::NotFound,
    UnmatchedReason::Unreadable,
    UnmatchedReason::NotUnicode,
];

fn reason_phrase(reason: UnmatchedReason, n: usize, has_failed_entries: bool) -> &'static str {
    use UnmatchedReason as R;
    match reason {
        R::NoEntry => {
            if has_failed_entries {
                "not matched"
            } else {
                "not in the manifest"
            }
        }
        R::AlreadyAssigned => "already assigned, left unchanged",
        R::FolderNoMatch => plural(n, "folder with nothing to match", "folders with nothing to match"),
        R::LinkNotFollowed => plural(n, "link not followed", "links not followed"),
        R::NotFound => "no longer found",
        R::Unreadable => "could not be read",
        R::NotUnicode => plural(
            n,
            "not assigned, unsupported characters in its path \u{2014} rename it or its folder",
            "not assigned, unsupported characters in their paths \u{2014} rename them or their folders",
        ),
    }
}

fn reason_tone(reason: UnmatchedReason) -> Tone {
    use UnmatchedReason as R;
    match reason {
        R::NoEntry | R::AlreadyAssigned | R::FolderNoMatch | R::LinkNotFollowed => Tone::Muted,
        R::NotFound | R::Unreadable | R::NotUnicode => Tone::Warn,
    }
}

/// The last path segment of a display path.
fn base_name(path: &str) -> &str {
    path.split(['\\', '/'])
        .rfind(|s| !s.is_empty())
        .unwrap_or(path)
}

fn lossy(p: &std::path::Path) -> String {
    p.to_string_lossy().into_owned()
}

fn ambiguous_item(a: &Ambiguity, entries: &[SessionEntry]) -> DropItem {
    let entry = entries.iter().find(|e| e.view.id == a.id);
    let candidates: Vec<String> = a.candidates.iter().map(|p| lossy(p)).collect();
    let file_name = candidates
        .first()
        .map_or_else(String::new, |c| base_name(c).to_string());
    DropItem {
        // An id missing from `entries` should not happen; never show the raw id.
        label: entry.map_or_else(|| file_name.clone(), |e| e.view.target_path.clone()),
        mono: entry.is_some(),
        title: format!("Matching files:\n{}", candidates.join("\n")),
        note: Some(if candidates.len() > 1 {
            ItemNote::Count(candidates.len())
        } else {
            ItemNote::Text(format!("{file_name} fits more than one entry"))
        }),
    }
}

fn capped(mut items: Vec<DropItem>) -> (Vec<DropItem>, usize) {
    let more = items.len().saturating_sub(MAX_NAMES);
    items.truncate(MAX_NAMES);
    (items, more)
}

/// A line with no names: matched, the hint, the empty line.
fn plain_line(kind: LineKind, tone: Tone, count: Option<usize>, lead: &str) -> DropLine {
    DropLine {
        kind,
        tone,
        count,
        lead: lead.to_string(),
        items: Vec::new(),
        more: 0,
        full: None,
    }
}

/// The line of the dropped paths that were left out for `reason`, if any.
fn reason_line(
    outcome: &DropOutcome,
    reason: UnmatchedReason,
    has_failed_entries: bool,
) -> Option<DropLine> {
    let paths: Vec<String> = outcome
        .unmatched
        .iter()
        .filter(|u| u.reason == reason)
        .map(|u| lossy(&u.path))
        .collect();
    if paths.is_empty() {
        return None;
    }
    let n = paths.len();
    let phrase = reason_phrase(reason, n, has_failed_entries);
    let items = paths
        .iter()
        .map(|p| DropItem {
            label: base_name(p).to_string(),
            title: p.clone(),
            mono: false,
            note: None,
        })
        .collect();
    let (items, more) = capped(items);
    Some(DropLine {
        kind: LineKind::Unmatched(reason),
        tone: reason_tone(reason),
        count: Some(n),
        lead: format!("{phrase}:"),
        items,
        more,
        full: Some(FullPaths::Paths {
            lead: format!("{n} {phrase}"),
            paths,
        }),
    })
}

/// The line of the files that fit more than one entry, if any.
fn ambiguous_line(outcome: &DropOutcome, entries: &[SessionEntry]) -> Option<DropLine> {
    if outcome.ambiguous.is_empty() {
        return None;
    }
    let n = outcome.ambiguous.len();
    let rows = plural(n, "its row", "their rows");
    let all: Vec<DropItem> = outcome
        .ambiguous
        .iter()
        .map(|a| ambiguous_item(a, entries))
        .collect();
    let full_entries = outcome
        .ambiguous
        .iter()
        .zip(&all)
        .map(|(a, item)| FullEntry {
            label: item.label.clone(),
            mono: item.mono,
            candidates: a.candidates.iter().map(|p| lossy(p)).collect(),
        })
        .collect();
    let (items, more) = capped(all);
    Some(DropLine {
        kind: LineKind::Ambiguous,
        tone: Tone::Warn,
        count: Some(n),
        lead: format!("ambiguous, not assigned \u{2014} use Browse\u{2026} on {rows}:"),
        items,
        more,
        full: Some(FullPaths::Entries {
            lead: format!("{n} ambiguous, not assigned \u{2014} use Browse\u{2026} on {rows}"),
            entries: full_entries,
        }),
    })
}

/// The lines of a drop result, in display order: matched, one per reason,
/// the hint, ambiguous; or the single empty line.
#[must_use]
pub fn drop_lines(
    outcome: &DropOutcome,
    entries: &[SessionEntry],
    has_failed_entries: bool,
) -> Vec<DropLine> {
    let mut lines = Vec::new();
    if !outcome.matched.is_empty() {
        lines.push(plain_line(
            LineKind::Matched,
            Tone::Ok,
            Some(outcome.matched.len()),
            "matched",
        ));
    }
    lines.extend(
        ORDER
            .into_iter()
            .filter_map(|reason| reason_line(outcome, reason, has_failed_entries)),
    );
    let show_hint = outcome.unmatched.iter().any(|u| {
        matches!(
            u.reason,
            UnmatchedReason::NoEntry | UnmatchedReason::FolderNoMatch
        )
    });
    if has_failed_entries && show_hint {
        lines.push(plain_line(LineKind::Hint, Tone::Muted, None, HINT));
    }
    lines.extend(ambiguous_line(outcome, entries));
    if lines.is_empty() {
        lines.push(plain_line(LineKind::Empty, Tone::Muted, None, EMPTY));
    }
    lines
}

/// An item as plain text: "label (2 files)".
#[must_use]
pub fn item_text(i: &DropItem) -> String {
    match &i.note {
        None => i.label.clone(),
        Some(ItemNote::Count(n)) => format!("{} ({n} files)", i.label),
        Some(ItemNote::Text(t)) => format!("{} ({t})", i.label),
    }
}

/// A line as plain text, ending in a full stop.
#[must_use]
pub fn line_text(l: &DropLine) -> String {
    let mut names = l.items.iter().map(item_text).collect::<Vec<_>>().join(", ");
    if l.more > 0 {
        let _ = write!(names, ", and {} more", l.more);
    }
    let count = l.count.map(|c| c.to_string());
    let text = [
        count.as_deref().unwrap_or(""),
        l.lead.as_str(),
        names.as_str(),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(" ");
    if text.ends_with('.') {
        text
    } else {
        format!("{text}.")
    }
}

/// The result lines as plain text, each ending in a full stop, for the live
/// region.
#[must_use]
pub fn drop_result_text(lines: &[DropLine]) -> String {
    lines.iter().map(line_text).collect::<Vec<_>>().join(" ")
}

// ---- display paths ----

fn is_sep(c: char) -> bool {
    c == '\\' || c == '/'
}

/// The characters of the drive (`C:\`, `C:/`, `C:`) or UNC
/// (`\\server\share\`) prefix of a display path.
fn prefix_length(cps: &[char]) -> usize {
    if let [letter, ':', rest @ ..] = cps {
        if letter.is_ascii_alphabetic() {
            return if rest.first().copied().is_some_and(is_sep) {
                3
            } else {
                2
            };
        }
    }
    if let ['\\', '\\', rest @ ..] = cps {
        let name = |from: usize| rest[from..].iter().take_while(|c| !is_sep(**c)).count();
        let server = name(0);
        if server > 0 && rest.get(server).copied().is_some_and(is_sep) {
            let share = name(server + 1);
            if share > 0 {
                let end = server + 1 + share;
                let trailing = usize::from(rest.get(end).copied().is_some_and(is_sep));
                return 2 + end + trailing;
            }
        }
    }
    0
}

/// The index of the last separator of a display path that is not inside its
/// drive or UNC prefix.
fn last_separator(cps: &[char], from: usize) -> Option<usize> {
    (from..cps.len()).rev().find(|i| is_sep(cps[*i]))
}

/// The file name of a display path: what follows the last separator.
#[must_use]
pub fn file_name(path: &str) -> String {
    let cps: Vec<char> = path.chars().collect();
    let start = cps.iter().rposition(|c| is_sep(*c)).map_or(0, |i| i + 1);
    cps[start..].iter().collect()
}

/// The folder of a display path: everything before the last separator,
/// keeping the separator when only a drive or UNC-share prefix remains.
/// `None` when there is no separator. For display only.
#[must_use]
pub fn display_folder(path: &str) -> Option<String> {
    let cps: Vec<char> = path.chars().collect();
    let pre = prefix_length(&cps);
    let from = if pre > 0 && is_sep(cps[pre - 1]) {
        pre - 1
    } else {
        pre
    };
    let sep = last_separator(&cps, from)?;
    let end = if sep < pre.max(1) { sep + 1 } else { sep };
    Some(cps[..end].iter().collect())
}

/// What the result's detail says when the build's thread panicked or the
/// backend stopped answering; the panic's message follows.
pub const BUILD_STOPPED: &str = "the build stopped unexpectedly";

// ---- the build bar ----

pub const OUTPUT_LABEL: &str = "Output:";
pub const OUTPUT_NOT_CHOSEN: &str = "not chosen";
pub const CHOOSE_OUTPUT: &str = "Choose\u{2026}";
pub const CREATE_ARCHIVE: &str = "Create archive";
/// The Show errors button's label (the same words as the errors section's
/// toggle when collapsed).
pub const SHOW_ERRORS: &str = "Show errors";
pub const SHOW_ERRORS_HINT: &str = "Shortcut: F8";
pub const CREATE_ARCHIVE_HINT: &str = "Shortcut: Ctrl+Enter";
pub const OPEN_HINT: &str = "Shortcut: Ctrl+O";

fn plural64(n: u64, one: &'static str, many: &'static str) -> &'static str {
    if n == 1 {
        one
    } else {
        many
    }
}

fn count_of(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// Why Create archive is disabled; `None` when it is not
/// (`BuildBar.tsx` `reasonText`).
#[must_use]
pub fn build_reason(s: &TarpackSession) -> Option<String> {
    Some(match s.build_blocked_reason? {
        BuildBlockedReason::NoManifest => REASON_NO_MANIFEST.to_string(),
        BuildBlockedReason::NoEntries => match &s.manifest {
            Some(m) if m.entries_withheld => {
                "No files can be built until the manifest errors are fixed".to_string()
            }
            Some(m) if !m.failed_entries.is_empty() => {
                "Every file in the manifest has errors".to_string()
            }
            _ => REASON_NO_FILES.to_string(),
        },
        BuildBlockedReason::EntriesNotReady => {
            let n = s.manifest.as_ref().map_or(0, |m| {
                m.entries
                    .iter()
                    .filter(|e| e.status == EntryStatus::Missing)
                    .count()
            });
            format!(
                "{n} chosen {} missing from disk",
                plural(n, "file is", "files are")
            )
        }
        BuildBlockedReason::NothingLoaded => {
            "Add at least one file to build an archive".to_string()
        }
        BuildBlockedReason::NoOutput => "Choose where to save the archive".to_string(),
    })
}

/// The note under the bar about files that will be left out of the build
/// (`BuildBar.tsx` `leftOutNote`).
#[must_use]
pub fn left_out_note(s: &TarpackSession) -> Option<String> {
    let m = s.manifest.as_ref()?;
    if m.entries.is_empty() {
        return None;
    }
    let not_loaded = m
        .entries
        .iter()
        .filter(|e| e.status == EntryStatus::Unassigned)
        .count();
    if s.ready_count > 0 && not_loaded > 0 && m.error_count == 0 {
        return Some(format!(
            "Partial archive: {} of {} {}; {not_loaded} without a file will be left out",
            s.ready_count,
            s.total_count,
            plural(s.total_count as usize, "file", "files")
        ));
    }
    if m.error_count == 0 {
        return None;
    }
    let n = m.failed_entries.len();
    if n > 0 {
        return Some(format!(
            "{n} {} will be left out; errors will be listed after the build",
            plural(n, "file", "files")
        ));
    }
    let e = m.errors.len();
    Some(format!(
        "Builds with {e} manifest {}",
        plural(e, "error", "errors")
    ))
}

// ---- the format picker ----

pub const FORMAT_LABEL: &str = "Format";

/// The short display name of a format; the extension always comes from the
/// session.
#[must_use]
pub fn format_name(format: ArchiveFormat) -> &'static str {
    match format {
        ArchiveFormat::Tar => "tar",
        ArchiveFormat::TarGz => "gzip",
        ArchiveFormat::TarZst => "zstd",
        ArchiveFormat::TarXz => "xz",
    }
}

/// A picker option: "zstd (.tar.zst)".
#[must_use]
pub fn format_option_label(option: &ArchiveFormatOption) -> String {
    format!("{} ({})", format_name(option.format), option.extension)
}

/// The save dialog's filter name: "zstd archive (.tar.zst)".
#[must_use]
pub fn format_filter_name(option: &ArchiveFormatOption) -> String {
    format!(
        "{} archive ({})",
        format_name(option.format),
        option.extension
    )
}

/// Announced in the bar's live region when the chosen file name switched the
/// format.
#[must_use]
pub fn format_changed_notice(format: ArchiveFormat) -> String {
    format!(
        "Format changed to {} to match the file name.",
        format_name(format)
    )
}

// ---- progress ----

/// The progress bar's accessible name.
pub const BUILDING_ARCHIVE: &str = "Building archive";
/// Shown instead of the bar when its events cannot be read.
pub const PROGRESS_UNAVAILABLE: &str = "Creating the archive\u{2026} (progress is unavailable)";
/// The label of the last moment of verifying.
pub const FINISHING: &str = "Finishing\u{2026}";
/// Announced while the archive is verified.
pub const VERIFYING_ANNOUNCEMENT: &str = "Verifying the archive";

/// The step number of a phase: 1 of 2 or 2 of 2.
#[must_use]
pub fn progress_step(phase: BuildPhase) -> u8 {
    match phase {
        BuildPhase::Writing => 1,
        BuildPhase::Verifying => 2,
    }
}

/// The verb of a phase.
#[must_use]
pub fn phase_word(phase: BuildPhase) -> &'static str {
    match phase {
        BuildPhase::Writing => "Writing",
        BuildPhase::Verifying => "Verifying",
    }
}

/// How far a phase is: 0 to 1 (an event with no total counts as done).
#[must_use]
pub fn progress_fraction(p: Option<&Progress>) -> f32 {
    let Some(p) = p else { return 0.0 };
    if p.bytes_total == 0 {
        return 1.0;
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "a bar's width does not need 24 bits of precision"
    )]
    let fraction = p.bytes_done as f64 / p.bytes_total as f64;
    #[expect(clippy::cast_possible_truncation, reason = "within 0 to 1")]
    let fraction = fraction.min(1.0) as f32;
    fraction
}

/// The whole percent of [`progress_fraction`], rounded down.
#[must_use]
pub fn progress_percent(p: Option<&Progress>) -> u8 {
    match p {
        None => 0,
        Some(p) if p.bytes_total == 0 => 100,
        Some(p) => {
            let done = u128::from(p.bytes_done.min(p.bytes_total)) * 100;
            u8::try_from(done / u128::from(p.bytes_total)).unwrap_or(100)
        }
    }
}

/// The last moment of verifying: the label says "Finishing".
#[must_use]
pub fn progress_finishing(p: Option<&Progress>) -> bool {
    p.is_some_and(|p| p.phase == BuildPhase::Verifying && p.bytes_done >= p.bytes_total)
}

/// The label above the bar. `file` is the source file to name (the model's
/// [`super::model::TarpackModel::progress_file`]); the last event of a phase
/// names none.
#[must_use]
pub fn progress_label(p: Option<&Progress>, file: Option<&str>) -> String {
    if progress_finishing(p) {
        return FINISHING.to_string();
    }
    let phase = p.map_or(BuildPhase::Writing, |p| p.phase);
    let done = p.is_some_and(|p| p.bytes_done >= p.bytes_total);
    let mut label = format!(
        "Step {} of 2 \u{b7} {}",
        progress_step(phase),
        phase_word(phase)
    );
    if let (Some(file), false) = (file, done) {
        let _ = write!(label, " {file}");
    }
    label
}

/// The bar's accessible value: "Writing gateway.conf, 42%". `source` is the
/// file of the current event only, with no remembered name.
#[must_use]
pub fn progress_value_text(p: Option<&Progress>, source: Option<&str>) -> String {
    let phase = p.map_or(BuildPhase::Writing, |p| p.phase);
    let named = source.map_or_else(String::new, |s| format!(" {s}"));
    format!("{}{named}, {}%", phase_word(phase), progress_percent(p))
}

// ---- the overwrite confirmation ----

pub const OVERWRITE_CONFIRM: &str = "Replace";
pub const OVERWRITE_CANCEL: &str = "Cancel";

/// The dialog's title: "Replace gateway.tar.zst?".
#[must_use]
pub fn overwrite_title(output: &str) -> String {
    format!("Replace {}?", file_name(output))
}

/// The dialog's body; names the folder when the path has one.
#[must_use]
pub fn overwrite_body(output: &str) -> String {
    match display_folder(output) {
        Some(folder) => format!(
            "A file with this name already exists in {folder}. Replacing it can't be undone."
        ),
        None => "A file with this name already exists. Replacing it can't be undone.".to_string(),
    }
}

// ---- the result ----

pub const RESULT_DISMISS: &str = "Dismiss result";
pub const SHOW_IN_FOLDER: &str = "Show in folder";
pub const SHA256_LABEL: &str = "SHA-256";
pub const EXTRACT_LABEL: &str = "Extract on the target";
pub const EXTRACT_HINT: &str =
    "Run this in the folder that holds the file. -P keeps the absolute paths.";
pub const NORMALIZED_LABEL: &str = "Line endings converted";
pub const NO_CRLF: &str = "no CRLF found";
pub const COPY: &str = "Copy";
pub const COPY_SHA256: &str = "Copy SHA-256";
pub const COPY_EXTRACTION_COMMAND: &str = "Copy extraction command";
pub const COPY_REPORT: &str = "Copy report";
/// What a copy button says for a moment after a copy.
pub const COPIED: &str = "Copied";

/// A size in bytes, B, KB or MB with one decimal, rounded half up like
/// `toFixed(1)` (`BuildResult.tsx` `size`).
#[must_use]
pub fn file_size(n: u64) -> String {
    let tenths = |unit: u128| (u128::from(n) * 10 + unit / 2) / unit;
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        let t = tenths(1024);
        format!("{}.{} KB", t / 10, t % 10)
    } else {
        let t = tenths(1024 * 1024);
        format!("{}.{} MB", t / 10, t % 10)
    }
}

/// The summary's output file name.
#[must_use]
pub fn result_file_name(s: &BuildSummary) -> String {
    file_name(&s.path)
}

/// The result's heading, also the first line of the copied report.
#[must_use]
pub fn result_heading(s: &BuildSummary) -> String {
    let name = file_name(&s.path);
    let not_loaded = count_of(s.not_loaded.len());
    let left_out = count_of(s.left_out.len());
    let manifest_errors = count_of(s.manifest_errors.len());
    if s.error_count == 0 && not_loaded > 0 {
        return format!(
            "Created {name} with {not_loaded} {} not included",
            plural64(not_loaded, "file", "files")
        );
    }
    if s.error_count == 0 {
        return format!("Created {name}");
    }
    if left_out > 0 {
        return format!(
            "Created {name} with {left_out} {} left out",
            plural64(left_out, "file", "files")
        );
    }
    format!(
        "Created {name} with {manifest_errors} {}",
        plural64(manifest_errors, "manifest error", "manifest errors")
    )
}

/// The sentence announced once when a build resolves.
#[must_use]
pub fn result_announcement(s: &BuildSummary) -> String {
    let name = file_name(&s.path);
    if s.error_count == 0 && s.not_loaded.is_empty() {
        return format!("Created {name}.");
    }
    if s.error_count == 0 {
        let n = count_of(s.not_loaded.len());
        return format!(
            "Created {name}. {n} {} not included because no file was chosen.",
            plural64(n, "file was", "files were")
        );
    }
    if !s.left_out.is_empty() {
        let n = count_of(s.left_out.len());
        return format!(
            "Created {name}. {n} {} left out because of errors.",
            plural64(n, "file was", "files were")
        );
    }
    let n = count_of(s.manifest_errors.len());
    format!(
        "Created {name} with {n} {}.",
        plural64(n, "manifest error", "manifest errors")
    )
}

/// Whether the build holds everything the manifest lists and has no errors
/// (the result shows a check, not a warning).
#[must_use]
pub fn result_is_clean(s: &BuildSummary) -> bool {
    s.error_count == 0 && s.not_loaded.is_empty()
}

/// The lead above the entries that had no file chosen. `None` when none.
#[must_use]
pub fn not_loaded_lede(s: &BuildSummary) -> Option<String> {
    if s.not_loaded.is_empty() {
        return None;
    }
    Some(format!(
        "The archive holds {} of the manifest's {} files. No file was chosen for:",
        s.built_ids.len(),
        s.built_ids.len() + s.left_out.len() + s.not_loaded.len()
    ))
}

/// The lead over the report when entries with errors were left out. `None`
/// when none were.
#[must_use]
pub fn left_out_lede(s: &BuildSummary) -> Option<String> {
    if s.error_count == 0 || s.left_out.is_empty() {
        return None;
    }
    Some(format!(
        "The archive holds {} of the manifest's {} files. The rest have errors, listed below.",
        s.built_ids.len(),
        s.built_ids.len() + s.left_out.len()
    ))
}

/// The facts line, with middle dots between its parts: "zstd", "2 files, 1
/// folder", "2.0 KB (10.0 MB uncompressed)".
#[must_use]
pub fn result_facts(s: &BuildSummary) -> String {
    let mut facts = format!(
        "{} \u{b7} {} {}, {} {} \u{b7} {}",
        format_name(s.format),
        s.files,
        plural64(s.files, "file", "files"),
        s.dirs,
        plural64(s.dirs, "folder", "folders"),
        file_size(s.bytes)
    );
    if s.format != ArchiveFormat::Tar {
        let _ = write!(facts, " ({} uncompressed)", file_size(s.uncompressed_bytes));
    }
    facts
}

/// The source file name of the entry `id`, else the id (the result lists
/// entries by file name).
#[must_use]
pub fn source_of<'a>(id: &'a str, entries: &'a [SessionEntry]) -> &'a str {
    entries
        .iter()
        .find(|e| e.view.id == id)
        .map_or(id, |e| e.view.source.as_str())
}

/// One normalised file's line: "gateway.conf: 42 CRLF replaced".
#[must_use]
pub fn normalized_line(source: &str, crlf_replaced: u64) -> String {
    if crlf_replaced == 0 {
        format!("{source}: {NO_CRLF}")
    } else {
        format!("{source}: {crlf_replaced} CRLF replaced")
    }
}

// ---- the report ----

pub const REPORT_LABEL: &str = "Build report";

/// "Left out of the archive (2)".
#[must_use]
pub fn report_left_out_heading(n: usize) -> String {
    format!("Left out of the archive ({n})")
}

/// "Manifest errors (2)".
#[must_use]
pub fn report_errors_heading(n: usize) -> String {
    format!("Manifest errors ({n})")
}

/// "Warnings (2)".
#[must_use]
pub fn report_warnings_heading(n: usize) -> String {
    format!("Warnings ({n})")
}

/// The label of the manifest errors list.
pub const MANIFEST_ERRORS_LABEL: &str = "Manifest errors";

fn diagnostic_lines(out: &mut Vec<String>, list: &[Diagnostic], indent: &str) {
    out.extend(
        list.iter()
            .map(|d| format!("{indent}{}:{} {}", d.line, d.col, d.message)),
    );
}

/// The plain text of the final report, for Copy report. Empty sections are
/// omitted (`reportText.ts` `reportText`).
#[must_use]
pub fn report_text(s: &BuildSummary) -> String {
    let mut out = vec![result_heading(s)];
    if !s.not_loaded.is_empty() {
        out.push(String::new());
        out.push("Not included (no file chosen):".to_string());
        out.extend(s.not_loaded.iter().map(|id| format!("  {id}")));
    }
    if !s.left_out.is_empty() {
        out.push(String::new());
        out.push("Left out of the archive:".to_string());
        for f in &s.left_out {
            let name = failure_name(f);
            let place = match &f.source {
                Some(source) => format!("source {source}, line {}", f.line),
                None => format!("line {}", f.line),
            };
            out.push(format!("{name} ({place})"));
            diagnostic_lines(&mut out, &f.errors, "  ");
        }
    }
    if !s.manifest_errors.is_empty() {
        out.push(String::new());
        out.push("Manifest errors:".to_string());
        diagnostic_lines(&mut out, &s.manifest_errors, "  ");
    }
    if !s.warnings.is_empty() {
        out.push(String::new());
        out.push("Warnings:".to_string());
        diagnostic_lines(&mut out, &s.warnings, "  ");
    }
    out.join("\n")
}

// ---- the shortcuts help ----

pub const SHORTCUTS_BUTTON: &str = "Keyboard shortcuts";
pub const SHORTCUTS_TITLE: &str = "Keyboard shortcuts";
pub const SHORTCUTS_HEADERS: [&str; 3] = ["Keys", "Action", "Active when"];
/// Between two alternative key combinations.
pub const SHORTCUTS_OR: &str = "or";

/// One row of the shortcuts list. `keys` are alternatives; each is a chord.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShortcutRow {
    pub keys: &'static [&'static [&'static str]],
    pub action: &'static str,
    pub when: &'static str,
    /// Listed only in developer mode.
    pub dev_only: bool,
}

/// The shortcuts list. The developer-mode chord itself is not in it.
pub const SHORTCUTS: [ShortcutRow; 9] = [
    ShortcutRow {
        keys: &[&["Ctrl", "O"]],
        action: "Open manifest\u{2026}",
        when: "Always",
        dev_only: false,
    },
    ShortcutRow {
        keys: &[&["F5"], &["Ctrl", "R"]],
        action: "Reload manifest",
        when: "A manifest is open",
        dev_only: false,
    },
    ShortcutRow {
        keys: &[&["Ctrl", "E"]],
        action: "Edit in editor",
        when: "A manifest is open",
        dev_only: true,
    },
    ShortcutRow {
        keys: &[&["Ctrl", "Enter"]],
        action: "Create archive",
        when: "The archive can be built",
        dev_only: false,
    },
    ShortcutRow {
        keys: &[&["F8"]],
        action: "Show errors",
        when: "The manifest has errors",
        dev_only: false,
    },
    ShortcutRow {
        keys: &[&["Up"], &["Down"]],
        action: "Move between files",
        when: "Focus is in the table",
        dev_only: false,
    },
    ShortcutRow {
        keys: &[&["Enter"]],
        action: "Browse\u{2026} for the focused file",
        when: "A file row is focused",
        dev_only: false,
    },
    ShortcutRow {
        keys: &[&["Delete"]],
        action: "Clear the focused file",
        when: "The row has a file assigned",
        dev_only: false,
    },
    ShortcutRow {
        keys: &[&["Esc"]],
        action: "Dismiss the drop or build result",
        when: "A result is showing",
        dev_only: false,
    },
];
