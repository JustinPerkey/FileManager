//! The Schedule Creator's copy, ported verbatim from the Tauri view
//! (`errorMessages.ts`, `ScheduleView.tsx`). The views import every string
//! from here.

use fm_schedule::session::{ApplySummary, ScheduleError, ScheduleErrorKind, ScheduleSession};

/// Copy for an error nothing is known about (a panicked backend call).
pub const UNEXPECTED: &str = "Something went wrong. See Details.";

/// `text` ending in a full stop, unless it already ends in punctuation.
fn sentence(text: &str) -> String {
    let t = text.trim();
    if t.ends_with(['.', '!', '?']) {
        t.to_string()
    } else {
        format!("{t}.")
    }
}

/// User-facing copy for an error. The error's own `message` is technical
/// detail, shown separately (except that `ParseFailed` quotes it, and its
/// `line`).
#[must_use]
pub fn error_message(e: &ScheduleError) -> String {
    let fixed: &str = match e.kind {
        ScheduleErrorKind::NoText => "Choose a schedule text file first.",
        ScheduleErrorKind::NoXml => "Choose the XML file to update first.",
        ScheduleErrorKind::NotAFile => "No file was found at this path.",
        ScheduleErrorKind::NotAbsolute => {
            "Enter the full path, starting with the drive letter, for example C:\\."
        }
        ScheduleErrorKind::FilesChanged => {
            "The chosen files changed after you confirmed. Nothing was changed. Check the files and try again."
        }
        ScheduleErrorKind::TextUnreadable => {
            "The schedule file could not be read. Check that it still exists and is saved as UTF-8 text."
        }
        ScheduleErrorKind::XmlUnreadable => {
            "The XML file could not be read. Check that it still exists and is saved as UTF-8 text."
        }
        ScheduleErrorKind::DropAmbiguous => {
            "Drop one schedule file, one XML file, or one of each. Nothing was changed."
        }
        ScheduleErrorKind::ParseFailed => {
            return match e.line {
                Some(line) => format!(
                    "Line {line} of the schedule file: {} Nothing was changed.",
                    sentence(&e.message)
                ),
                None => format!(
                    "The schedule file has a problem: {} Nothing was changed.",
                    sentence(&e.message)
                ),
            };
        }
        ScheduleErrorKind::ParseNotImplemented => {
            "Reading schedule files is not implemented yet. Nothing was changed."
        }
        ScheduleErrorKind::MergeFailed => {
            "The schedule could not be added to the XML file. Nothing was changed."
        }
        ScheduleErrorKind::MergeNotImplemented => {
            "Updating the XML file is not implemented yet. Nothing was changed."
        }
        ScheduleErrorKind::Io => {
            "A backup or the updated XML file could not be written. The XML file was not changed."
        }
    };
    fixed.to_string()
}

/// The last path component. Handles both separators; never assumes the path
/// is valid.
#[must_use]
pub fn file_name(path: &str) -> &str {
    match path.rsplit(['\\', '/']).next() {
        Some(last) if !last.is_empty() => last,
        _ => path,
    }
}

// ---- the page ----

pub const TITLE: &str = "Schedule Creator";
pub const INTRO: &str = "Choose a schedule text file and the XML file to add it to. Type or paste a path, browse for it, or drop the files on the window.";
pub const LOADING: &str = "Loading";
pub const TRY_AGAIN: &str = "Try again";
/// Visually hidden legend of the two fields.
pub const FILES_LEGEND: &str = "Files";
pub const TEXT_LABEL: &str = "Schedule file";
pub const TEXT_PLACEHOLDER: &str = "C:\\path\\to\\schedule.txt";
pub const TEXT_BROWSE_LABEL: &str = "Browse for the schedule file";
pub const XML_LABEL: &str = "XML file to update";
pub const XML_PLACEHOLDER: &str = "C:\\path\\to\\schedule.xml";
pub const XML_BROWSE_LABEL: &str = "Browse for the XML file";
pub const BROWSE: &str = "Browse…";
pub const ADD: &str = "Add to XML…";
pub const ADDING: &str = "Adding…";
pub const DETAILS: &str = "Details";
/// Before the backup path in the result banner.
pub const BACKUP_PREFIX: &str = "The previous version is saved as ";

// ---- apply ----

pub const REASON_NO_TEXT: &str = "Choose a schedule file.";
pub const REASON_NO_XML: &str = "Choose the XML file to update.";
pub const REASON_FIX_PATHS: &str = "Fix the file paths above first.";

/// Why Add is disabled, if so.
#[must_use]
pub fn apply_reason(s: &ScheduleSession) -> Option<&'static str> {
    if s.text.is_none() {
        Some(REASON_NO_TEXT)
    } else if s.xml.is_none() {
        Some(REASON_NO_XML)
    } else if s.text.as_ref().is_some_and(|f| f.error.is_some())
        || s.xml.as_ref().is_some_and(|f| f.error.is_some())
    {
        Some(REASON_FIX_PATHS)
    } else {
        None
    }
}

/// The result banner's sentence.
#[must_use]
pub fn result_sentence(r: &ApplySummary) -> String {
    format!(
        "Added {} {} to {}.",
        r.added,
        if r.added == 1 { "entry" } else { "entries" },
        file_name(&r.xml_path)
    )
}

/// What a drop announces: the files now chosen.
#[must_use]
pub fn drop_sentence(s: &ScheduleSession) -> String {
    let mut parts = Vec::new();
    for (what, slot) in [("Schedule file", &s.text), ("XML file", &s.xml)] {
        if let Some(slot) = slot {
            let problem = if slot.error.is_some() {
                " (has a problem)"
            } else {
                ""
            };
            parts.push(format!("{what}: {}{problem}.", file_name(&slot.path)));
        }
    }
    parts.join(" ")
}

// ---- the confirmation ----

pub const CONFIRM_LABEL: &str = "Update";
/// Bold lead of the duplicate warning.
pub const DUPLICATE_HEADLINE: &str = "This schedule was already added to this file.";
/// Follows [`DUPLICATE_HEADLINE`] after a space.
pub const DUPLICATE_REST: &str = "Adding it again may duplicate its entries.";

/// The confirmation's title.
#[must_use]
pub fn confirm_title(xml_path: &str) -> String {
    format!("Update {}?", file_name(xml_path))
}

/// The confirmation's body in pieces, so the view can set the paths in mono.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfirmBody {
    pub before: &'static str,
    pub text_path: String,
    pub between: &'static str,
    pub xml_path: String,
    pub after: &'static str,
}

impl ConfirmBody {
    /// The body as one string.
    #[must_use]
    pub fn plain(&self) -> String {
        format!(
            "{}{}{}{}{}",
            self.before, self.text_path, self.between, self.xml_path, self.after
        )
    }
}

/// The body naming the two files that will be used.
#[must_use]
pub fn confirm_body(text_path: &str, xml_path: &str) -> ConfirmBody {
    ConfirmBody {
        before: "The schedule in ",
        text_path: text_path.to_string(),
        between: " will be read and added to ",
        xml_path: xml_path.to_string(),
        after: ". A backup copy of the XML file is saved next to it first.",
    }
}

// ---- drops ----

pub const DROP_LABEL: &str = "Drop a schedule file, an XML file, or both";
pub const REASON_APPLYING: &str = "The XML file is being updated";
pub const REASON_CONFIRMING: &str = "Finish or cancel the update first";
pub const REASON_NOT_READY: &str = "Not ready yet";
