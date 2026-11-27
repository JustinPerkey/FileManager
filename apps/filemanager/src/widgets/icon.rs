//! Icons: the SVGs of `assets/icons`, sized to the text around them.

use gpui_component::Icon;

use crate::assets::icon_path;

/// The icons the app ships (`crate::assets::ICON_NAMES`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconName {
    CheckCircle,
    AlertTriangle,
    Circle,
    XCircle,
    Info,
    ChevronDown,
    Folder,
    Copy,
    X,
    Keyboard,
    ArrowLeft,
    ArrowRight,
    Lock,
    Plus,
    Trash,
    Search,
    ChevronLeft,
    ChevronRight,
    Undo,
    Check,
}

impl IconName {
    /// Every name, in the order of `ICON_NAMES`.
    pub const ALL: [Self; 20] = [
        Self::CheckCircle,
        Self::AlertTriangle,
        Self::Circle,
        Self::XCircle,
        Self::Info,
        Self::ChevronDown,
        Self::Folder,
        Self::Copy,
        Self::X,
        Self::Keyboard,
        Self::ArrowLeft,
        Self::ArrowRight,
        Self::Lock,
        Self::Plus,
        Self::Trash,
        Self::Search,
        Self::ChevronLeft,
        Self::ChevronRight,
        Self::Undo,
        Self::Check,
    ];

    /// The asset name, as in `icons.tsx` and `ICON_NAMES`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CheckCircle => "check-circle",
            Self::AlertTriangle => "alert-triangle",
            Self::Circle => "circle",
            Self::XCircle => "x-circle",
            Self::Info => "info",
            Self::ChevronDown => "chevron-down",
            Self::Folder => "folder",
            Self::Copy => "copy",
            Self::X => "x",
            Self::Keyboard => "keyboard",
            Self::ArrowLeft => "arrow-left",
            Self::ArrowRight => "arrow-right",
            Self::Lock => "lock",
            Self::Plus => "plus",
            Self::Trash => "trash",
            Self::Search => "search",
            Self::ChevronLeft => "chevron-left",
            Self::ChevronRight => "chevron-right",
            Self::Undo => "undo",
            Self::Check => "check",
        }
    }
}

/// An icon sized 1em of the text it sits in (14 px at body size), in that
/// text's colour. gpui-component's `Icon` takes both from the text style at
/// the moment it is drawn; set a colour with `.text_color(..)` on the result.
#[must_use]
pub fn icon(name: IconName) -> Icon {
    Icon::empty().path(icon_path(name.as_str()))
}
