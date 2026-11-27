//! Copy the view needs that `text.rs` does not carry yet: the Recent and
//! Read-only labels, the config line, the empty state's sentences and the
//! config problems' summary and positions. Everything else is `text.rs`'s.

use fm_dbmanager::config::ConfigDiagnostic;

pub const RECENT: &str = "Recent";
pub const READ_ONLY: &str = "Read-only";
pub const CONFIG_LABEL: &str = "Config:";
/// The config line when there is no config file.
pub const CONFIG_MISSING_LINE: &str =
    "No config file: everything is editable unless the database prevents it.";
/// The config line when the location is unknown.
pub const CONFIG_NO_LOCATION: &str = "the location could not be determined";
pub const EMPTY_LEAD: &str = "Open a SQLite database to browse and edit it as objects.";
pub const DROP_HINT: &str = "Or drop a database file on this window.";
pub const SEVERITY_ERROR: &str = "Error";
pub const SEVERITY_WARNING: &str = "Warning";

/// "Backup: <file name>".
#[must_use]
pub fn backup_line(file_name: &str) -> String {
    format!("Backup: {file_name}")
}

fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// "Config: 2 errors, 1 warning. Entries with errors are read-only."
#[must_use]
pub fn problems_summary(errors: usize, warnings: usize) -> String {
    let mut parts = Vec::new();
    if errors > 0 {
        parts.push(count(errors, "error", "errors"));
    }
    if warnings > 0 {
        parts.push(count(warnings, "warning", "warnings"));
    }
    let mut line = format!("Config: {}.", parts.join(", "));
    if errors > 0 {
        line.push_str(" Entries with errors are read-only.");
    }
    line
}

/// Where a diagnostic is: "Line 12, column 3", or "Table book, field isbn".
/// Empty when it has no position.
#[must_use]
pub fn position(d: &ConfigDiagnostic) -> String {
    match (d.line, d.col, d.table.as_deref(), d.column.as_deref()) {
        (Some(line), Some(col), _, _) => format!("Line {line}, column {col}"),
        (Some(line), None, _, _) => format!("Line {line}"),
        (None, _, Some(table), Some(field)) => format!("Table {table}, field {field}"),
        (None, _, Some(table), None) => format!("Table {table}"),
        _ => String::new(),
    }
}
