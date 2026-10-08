//! Hook: [`Schedule`] + existing XML -> updated XML.

use std::fmt;

use crate::model::Schedule;

/// The updated document and what changed in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergeOutcome {
    /// The whole updated XML document. The shell writes it over the original
    /// after saving a backup copy.
    pub xml: String,
    /// How many schedule entries were added, for the result message.
    pub added: u32,
}

/// Why the schedule could not be added to the XML.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MergeError {
    /// [`merge`] has not been written yet.
    NotImplemented,
    /// The XML is not a document the schedule can be added to.
    InvalidXml(String),
    /// The schedule conflicts with what the XML already holds.
    Conflict(String),
}

impl fmt::Display for MergeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MergeError::NotImplemented => f.write_str("updating the XML is not implemented yet"),
            MergeError::InvalidXml(m) | MergeError::Conflict(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for MergeError {}

/// Adds `schedule` to the XML document `xml` and returns the whole updated
/// document.
///
/// `xml` is the file as it is on disk when the user confirms, decoded as
/// UTF-8 with any byte-order mark removed (the shell writes it back, so do
/// not add one). This function must not touch the
/// filesystem: the shell owns the backup and the write, and writes nothing
/// when this returns an error.
pub fn merge(schedule: &Schedule, xml: &str) -> Result<MergeOutcome, MergeError> {
    // TODO: add `schedule` to `xml`.
    let _ = (schedule, xml);
    Err(MergeError::NotImplemented)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_reports_not_implemented() {
        assert_eq!(
            merge(&Schedule::default(), "<schedule/>"),
            Err(MergeError::NotImplemented)
        );
    }
}
