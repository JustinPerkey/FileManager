//! The Tar Packager's shortcuts table: the rows of `text::SHORTCUTS` for the
//! shared [`crate::widgets::ShortcutsHelp`]. The bindings themselves are in
//! `keys.rs`.

use crate::tools::tarpack::text;
use crate::widgets::shortcuts_help::ShortcutRow;

/// The rows the help lists: every row, but the developer-mode ones only when
/// `dev` is on.
pub const SHORTCUT_ROWS: [text::ShortcutRow; 9] = text::SHORTCUTS;

/// The help's rows for the shared widget.
#[must_use]
pub fn rows(dev: bool) -> Vec<ShortcutRow> {
    SHORTCUT_ROWS
        .iter()
        .filter(|row| dev || !row.dev_only)
        .map(|row| ShortcutRow {
            keys: row.keys.iter().map(|chord| chord.to_vec()).collect(),
            action: row.action.into(),
            when: row.when.into(),
        })
        .collect()
}
