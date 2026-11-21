//! Explorer drop routing: what the current tool says about a drop.

use gpui::SharedString;

/// Whether, and how, a tool takes dropped files right now.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum DropTarget {
    /// The tool takes no drops.
    #[default]
    None,
    /// The tool takes a drop; `label` says what it does with it.
    Enabled { label: SharedString },
    /// The tool takes drops, but not now; `reason` says why.
    Disabled { reason: SharedString },
}
