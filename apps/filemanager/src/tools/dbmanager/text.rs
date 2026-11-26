//! The Database Manager's copy. The views import every string from here. The
//! error sentences are plain; a later UI task may adjust wording, not
//! meaning. The backend's own `message` is shown beside them as detail.

use fm_dbmanager::config::ConfigStatus;
use fm_dbmanager::config::TableMode;
use fm_dbmanager::model::TableView;
use fm_dbmanager::tool::{DbmanagerError, DbmanagerErrorKind, DbmanagerSnapshot};

/// Copy for an error nothing is known about (a panicked backend call).
pub const UNEXPECTED: &str = "Something went wrong. See Details.";

// ---- the page ----

pub const TITLE: &str = "Database Manager";
pub const LOADING: &str = "Loading";
pub const DETAILS: &str = "Details";
pub const OPEN: &str = "Open database…";
pub const RELOAD: &str = "Reload";
pub const CLOSE: &str = "Close database";
pub const BACK: &str = "Back";
pub const FORWARD: &str = "Forward";
pub const NO_DATABASE_OPEN: &str = "No database is open.";

// ---- the empty state ----

pub const EMPTY_TITLE: &str = "No database is open";
pub const EMPTY_HINT: &str =
    "Open a SQLite database file, or drop one on the window. Choose one of the recent databases below to open it again.";

// ---- the drop ----

pub const DROP_LABEL: &str = "Drop one database file";
pub const REASON_SAVING: &str = "Wait for the save to finish";

// ---- the file dialog ----

pub const FILTER_DATABASE: &str = "SQLite database";
pub const FILTER_ALL: &str = "All files";

// ---- the config's status ----

/// `missing`: the app-wide config does not exist.
pub const CONFIG_MISSING: &str = "No config is in use: everything is editable.";
/// The config differs from what was loaded and the app could not say more.
pub const CONFIG_CHANGED: &str = "The config changed on disk.";

/// The config's status line, when it has one: the missing notice or the
/// fail-closed reason. `None` for a config that loaded.
#[must_use]
pub fn config_notice(s: &DbmanagerSnapshot) -> Option<String> {
    if let Some(reason) = &s.config_fail_closed {
        return Some(config_fail_closed(reason));
    }
    match s.config.status {
        ConfigStatus::Missing => Some(CONFIG_MISSING.to_owned()),
        ConfigStatus::FailClosed => Some(config_fail_closed("the config cannot be used")),
        ConfigStatus::Loaded => None,
    }
}

/// The fail-closed reason as a sentence.
#[must_use]
pub fn config_fail_closed(reason: &str) -> String {
    let reason = reason.trim().trim_end_matches('.');
    format!("Everything is read-only: {reason}.")
}

// ---- read-only reasons ----

pub const TABLE_READ_ONLY: &str = "This table is read-only in the config.";
pub const TABLE_PICKABLE: &str =
    "This table is only used to pick from and link to. It cannot be changed here.";
pub const CONFIG_UNUSABLE: &str = "Everything is read-only because the config cannot be used.";

/// The database file's reason, as a sentence.
#[must_use]
pub fn database_read_only(reason: &str) -> String {
    let reason = reason.trim().trim_end_matches('.');
    format!("This database is read-only: {reason}.")
}

/// Why nothing in `table` can be changed, if so. The database's own reason
/// comes first, then the config's.
#[must_use]
pub fn table_read_only_reason(s: &DbmanagerSnapshot, table: &TableView) -> Option<String> {
    if let Some(reason) = &s.read_only_reason {
        return Some(database_read_only(reason));
    }
    if s.config_fail_closed.is_some() {
        return Some(CONFIG_UNUSABLE.to_owned());
    }
    match table.mode {
        TableMode::Editable => None,
        TableMode::ReadOnly | TableMode::Hidden => Some(TABLE_READ_ONLY.to_owned()),
        TableMode::Pickable => Some(TABLE_PICKABLE.to_owned()),
    }
}

// ---- announcements ----

/// The last path component. Handles both separators.
#[must_use]
pub fn file_name(path: &str) -> &str {
    match path.rsplit(['\\', '/']).next() {
        Some(last) if !last.is_empty() => last,
        _ => path,
    }
}

/// A database opened.
#[must_use]
pub fn opened_sentence(display: &str) -> String {
    format!("Opened {}.", file_name(display))
}

pub const CLOSED_SENTENCE: &str = "Database closed.";
pub const RELOADED_SENTENCE: &str = "Config and database reloaded.";

// ---- errors ----

/// The config's fixed location, named in the `ConfigNotDroppable` sentence.
const UNKNOWN_CONFIG_LOCATION: &str = "a fixed location that could not be determined";

/// User-facing copy for an error. `config_location` is the config's display
/// path, for `ConfigNotDroppable`.
#[must_use]
pub fn error_message(e: &DbmanagerError, config_location: Option<&str>) -> String {
    use DbmanagerErrorKind as K;
    let fixed: &str = match e.kind {
        K::NoDatabase => "No database is open. Open a database file first.",
        K::NotAbsolute => {
            "Enter the full path, starting with the drive letter, for example C:\\."
        }
        K::NotAFile => "No file was found at this path.",
        K::DatabaseUnreadable => {
            "This file could not be opened as a SQLite database. Check that it is a database and that it is not damaged."
        }
        K::OpenerFailed => "The file could not be opened with your default program. See Details.",
        K::ConfigNotDroppable => {
            return format!(
                "That is a config file. The Database Manager reads its config from {} and you cannot choose another here. Drop a database file instead.",
                config_location.unwrap_or(UNKNOWN_CONFIG_LOCATION)
            );
        }
        K::DropAmbiguous => "Drop one database file at a time. Nothing was changed.",
        K::UnknownTable => "That table is not in this database any more. Reload the database.",
        K::UnknownObject => "That item does not exist any more. It may have been deleted.",
        K::UnknownRelation => "That list is not available here. Reload the database.",
        K::NotEditable => "This cannot be changed with the current settings. Nothing was changed.",
        K::ValidationFailed => {
            "Some values are not valid. Nothing was changed. Fix the marked fields and save again."
        }
        K::Conflict => {
            "This item was changed by someone else after you opened it. Nothing was changed. Review the current values."
        }
        K::SchemaChanged => {
            "The structure of the database changed. Reload the database to continue."
        }
        K::ConfigChangedOnDisk => {
            "The config changed on disk and was reloaded. Nothing was changed. Check your changes and save again."
        }
        K::DatabaseReadOnly => "This database is read-only. Nothing was changed.",
        K::Busy => "The database is in use by another program. Try again in a moment.",
        K::BackupFailed => {
            "A backup of the database could not be made first, so nothing was saved. Nothing was changed."
        }
        K::ImpactChanged => {
            "What this delete affects changed. Nothing was deleted. Review it and confirm again."
        }
        K::DeleteBlocked => {
            "Other data depends on this item, so it cannot be deleted. Nothing was deleted."
        }
        K::ConstraintFailed => "The database refused the change. Nothing was changed.",
        K::Io => "The database could not be read or written. See Details.",
    };
    fixed.to_owned()
}
