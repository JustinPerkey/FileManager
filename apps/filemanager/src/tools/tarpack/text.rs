//! The Tar Packager's copy, ported verbatim from the Tauri view
//! (`errorMessages.ts`, `DropResult.tsx`, `ManifestHeader.tsx`,
//! `ManifestErrors.tsx`, `DiagnosticList.tsx`, `FailureList.tsx` and the
//! sentences in `TarpackView.tsx`). Pure functions and constants: the views
//! render them, the model announces them.
//!
//! The build side's strings (the build bar, progress, results, the report)
//! come with part B.

use std::fmt::Write as _;

use fm_tarpack::manifest::{Diagnostic, EntryFailure};
use fm_tarpack::session::{
    SessionEntry, SessionManifest, TarpackError, TarpackErrorKind, TarpackSession,
};
use fm_tarpack::sources::{Ambiguity, DropOutcome, UnmatchedReason};

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

/// What blocks a drop apart from the manifest. Part B sets these.
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
