//! The Database Manager's shortcuts help rows. This task registers the three
//! of the session; U12 completes the table.

use crate::widgets::shortcuts_help::ShortcutRow;

/// The rows the help lists.
#[must_use]
pub fn rows() -> Vec<ShortcutRow> {
    vec![
        ShortcutRow {
            keys: vec![vec!["Ctrl", "O"]],
            action: "Open database\u{2026}".into(),
            when: "Always".into(),
        },
        ShortcutRow {
            keys: vec![vec!["F5"], vec!["Ctrl", "R"]],
            action: "Reload".into(),
            when: "A database is open".into(),
        },
        ShortcutRow {
            keys: vec![vec!["F8"]],
            action: "Show config problems".into(),
            when: "The config has problems".into(),
        },
    ]
}
