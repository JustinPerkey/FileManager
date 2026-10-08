//! The parsed schedule.

/// A parsed schedule: what [`crate::parse`] produces and [`crate::merge`]
/// consumes. It never crosses to the UI.
///
/// The fields are a placeholder (a header and rows of cells). Replace them
/// with whatever the merge needs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Schedule {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}
