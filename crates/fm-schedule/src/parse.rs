//! Hook: schedule text file -> [`Schedule`].

use std::fmt;

use crate::model::Schedule;

/// Why the text could not be read as a schedule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// [`parse`] has not been written yet.
    NotImplemented,
    /// The text is not a valid schedule. `line` is 1-based, when known.
    Invalid { line: Option<u32>, message: String },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::NotImplemented => f.write_str("schedule parsing is not implemented yet"),
            ParseError::Invalid {
                line: Some(line),
                message,
            } => write!(f, "line {line}: {message}"),
            ParseError::Invalid {
                line: None,
                message,
            } => f.write_str(message),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parses the schedule text file's contents.
///
/// `text` is the whole file, decoded as UTF-8 with any byte-order mark
/// removed. Line endings are as in the file (`\r\n` or `\n`), so prefer
/// `str::lines`, which handles both.
///
/// Return [`ParseError::Invalid`] with the 1-based line number for a
/// problem in the file; the UI shows it next to the file name.
pub fn parse(text: &str) -> Result<Schedule, ParseError> {
    // TODO: parse `text` into a `Schedule`.
    let _ = text;
    Err(ParseError::NotImplemented)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_reports_not_implemented() {
        assert_eq!(parse("anything"), Err(ParseError::NotImplemented));
    }

    #[test]
    fn display_includes_the_line() {
        let e = ParseError::Invalid {
            line: Some(4),
            message: "missing time".into(),
        };
        assert_eq!(e.to_string(), "line 4: missing time");
    }
}
