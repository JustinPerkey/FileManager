//! The parsed schedule, and the preview the UI renders from it.

use serde::Serialize;

/// A parsed schedule: what [`crate::parse`] produces and [`crate::merge`]
/// consumes.
///
/// The fields are a placeholder (a header and rows of cells). Replace them
/// with whatever the merge needs; only [`Schedule::preview`] has to keep
/// producing a [`SchedulePreview`] for the UI.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Schedule {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Schedule {
    /// The table the UI shows before the XML is updated.
    pub fn preview(&self) -> SchedulePreview {
        SchedulePreview {
            columns: self.columns.clone(),
            rows: self.rows.clone(),
        }
    }
}

/// A schedule as the UI shows it: column headings and rows of display text.
/// Each row should have one cell per column.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct SchedulePreview {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_copies_columns_and_rows() {
        let s = Schedule {
            columns: vec!["When".into(), "What".into()],
            rows: vec![vec!["09:00".into(), "Standup".into()]],
        };
        let p = s.preview();
        assert_eq!(p.columns, s.columns);
        assert_eq!(p.rows, s.rows);
    }
}
