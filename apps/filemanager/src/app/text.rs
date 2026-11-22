//! Strings the app shares between its model side and its views.

/// What developer mode announces when it turns on. Whether and how it shows
/// is the UI's decision.
pub const DEV_MODE_ON: &str = "Developer mode on.";

/// What developer mode announces when it turns off.
pub const DEV_MODE_OFF: &str = "Developer mode off.";

/// Shown when a file dialog could not be shown or returned an error. Models
/// show it with the error's `detail` as technical detail.
pub const PICK_FAILED: &str = "The file dialog could not be shown. See Details.";

/// Shown when a dropped name could not be delivered by the OS drop.
pub const DROPPED_NAMES_UNREADABLE: &str = "Some dropped items could not be read: their names are not valid Unicode. Rename them and drop them again.";

/// Shown on the drop overlay while a dialog or sheet is open: the drop is refused.
pub const DROP_BLOCKED_BY_DIALOG: &str = "Close the open dialog first.";

/// Shown when GPUI could not read a dropped item's name at all.
pub const DROPPED_ITEMS_UNREADABLE: &str =
    "Some dropped items could not be read. Drop them again, or use Browse.";
