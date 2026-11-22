//! Every OS interaction a tool model needs, behind one trait so models and
//! screenshots can run against a scripted fake.
//!
//! The real implementation is [`gpui::GpuiPlatform`]; tests and screenshots use
//! [`fake::FakePlatform`]. The app holds one of them as a global
//! ([`PlatformHandle`]); models fetch it with [`platform`].

pub mod fake;
pub mod gate;
pub mod gpui;
pub mod path_loss;
#[cfg(windows)]
pub mod win_dialog;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ::gpui::{App, Global, Task};

/// What an open dialog picks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickKind {
    File,
    Folder,
}

/// One file-type filter of a dialog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileFilter {
    pub name: String,
    /// Patterns such as `*.toml`.
    pub patterns: Vec<String>,
}

/// An open dialog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenSpec {
    pub kind: PickKind,
    pub filters: Vec<FileFilter>,
    pub start_dir: Option<PathBuf>,
    pub title: Option<String>,
}

/// A save dialog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveSpec {
    pub filters: Vec<FileFilter>,
    /// Without the dot.
    pub default_extension: Option<String>,
    pub start_dir: Option<PathBuf>,
    pub suggested_name: Option<OsString>,
    pub title: Option<String>,
}

/// A dialog that could not be shown or that returned an error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickError {
    Failed { detail: String },
}

/// The outcome of a dialog: `Ok(None)` is a cancel.
pub type PickResult = Result<Option<PathBuf>, PickError>;

/// The OS operations models use. None of the methods blocks the calling
/// (UI) thread: dialogs return a task, the rest are fire and forget.
pub trait Platform: 'static {
    /// Shows an open dialog for one item; `None` is a cancel.
    fn pick(&self, spec: OpenSpec, cx: &mut App) -> Task<PickResult>;
    /// Shows a save dialog; `None` is a cancel.
    fn pick_save(&self, spec: SaveSpec, cx: &mut App) -> Task<PickResult>;
    /// Opens `path` with the system's default application.
    fn open_with_system(&self, path: &Path, cx: &mut App);
    /// Shows `path` in the system file manager.
    fn reveal(&self, path: &Path, cx: &mut App);
    /// Puts `text` on the clipboard.
    fn copy_text(&self, text: &str, cx: &mut App);
}

/// Tells the dialogs that window `closed` is gone, so none of them is given its
/// handle as an owner. A no-op off Windows.
pub fn release_owner(closed: ::gpui::WindowId) {
    #[cfg(windows)]
    win_dialog::release_owner(closed);
    #[cfg(not(windows))]
    let _ = closed;
}

/// The app's platform, as a GPUI global.
#[derive(Clone)]
pub struct PlatformHandle(pub Rc<dyn Platform>);

impl Global for PlatformHandle {}

/// Installs `platform` as the app's platform.
pub fn set_platform(cx: &mut App, platform: impl Platform) {
    cx.set_global(PlatformHandle(Rc::new(platform)));
}

/// Installs an already shared platform (what [`crate::AppSetup`] carries).
pub fn set_platform_rc(cx: &mut App, platform: Rc<dyn Platform>) {
    cx.set_global(PlatformHandle(platform));
}

/// The app's platform.
///
/// # Panics
/// If no platform was installed with [`set_platform`].
#[must_use]
pub fn platform(cx: &App) -> Rc<dyn Platform> {
    cx.global::<PlatformHandle>().0.clone()
}
