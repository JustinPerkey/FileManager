//! The `FileManager` GPUI app. The library name (`fm_app`) stays stable across
//! the rename that follows the cutover from the Tauri app.

// The domain crates the tools will use. The placeholder tools do not call them
// yet, and Cargo's `cargo::unused_dependencies` manifest lint warns about an
// unused dependency; these lines go once the tools use the crates.
use fm_dbmanager as _;
use fm_schedule as _;

pub mod app;
pub mod assets;
pub mod fonts;
pub mod jobs;
pub mod platform;
#[cfg(feature = "shoot")]
pub mod shoot;
pub mod theme;
pub mod tools;
pub mod widgets;

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    px, size, App, AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowHandle,
    WindowOptions,
};
use gpui_component::Root;

use crate::app::dev_mode::DevMode;
use crate::app::shell::{PathLossFeed, Shell};
use crate::tools::BackendSet;

/// How long `--self-check` lets the window live after opening it.
const SELF_CHECK_MS: u64 = 500;

/// What differs between the shipped app and a harness run: the platform that
/// answers dialogs and opens things, and which backend each tool uses.
pub struct AppSetup {
    /// Installed as the app's [`platform::Platform`].
    pub platform: Rc<dyn platform::Platform>,
    /// Real or fixture backends, per tool.
    pub backends: BackendSet,
    /// Quit by itself after the first frame (`--self-check`).
    pub self_check: bool,
    /// Hold every tool's background work until the test opens the gate.
    #[cfg(any(test, feature = "shoot"))]
    pub closed_worker_gate: bool,
}

impl AppSetup {
    /// The shipped app: the real platform and the real backends.
    #[must_use]
    pub fn real(self_check: bool) -> Self {
        Self {
            platform: Rc::new(platform::gpui::GpuiPlatform),
            backends: BackendSet::default(),
            self_check,
            #[cfg(any(test, feature = "shoot"))]
            closed_worker_gate: false,
        }
    }
}

impl AppSetup {
    /// Tests and the screenshot harness: fixture backends and a scripted
    /// platform; nothing reaches the real folders or dialogs.
    #[cfg(any(test, feature = "shoot"))]
    #[must_use]
    pub fn for_tests() -> Self {
        Self {
            platform: Rc::new(platform::fake::FakePlatform::new()),
            backends: BackendSet::fixtures(),
            self_check: false,
            closed_worker_gate: false,
        }
    }

    /// Installs a closed [`jobs::WorkerGate`] before the tools are built, so a
    /// test can see the loading state; get the handle with
    /// `jobs::WorkerGate::handle`.
    #[cfg(any(test, feature = "shoot"))]
    #[must_use]
    pub fn with_closed_worker_gate(mut self) -> Self {
        self.closed_worker_gate = true;
        self
    }
}

/// Opens the main window and runs until it closes. Returns the process exit
/// code: non-zero if the window could not be opened. With `self_check` the app
/// quits by itself shortly after the window has rendered its first frame.
#[must_use]
pub fn run(self_check: bool) -> i32 {
    run_with(AppSetup::real(self_check))
}

/// [`run`] with the platform and backends chosen by `setup`.
#[must_use]
pub fn run_with(setup: AppSetup) -> i32 {
    let failed = Rc::new(Cell::new(false));
    let failed_in = failed.clone();
    gpui_platform::application()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            let self_check = setup.self_check;
            init_app(cx, setup);
            if let Err(e) = open_main_window(cx, MAIN_SIZE) {
                eprintln!("could not open the window: {e:#}");
                failed_in.set(true);
                cx.quit();
                return;
            }
            if self_check {
                cx.spawn(async move |cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(SELF_CHECK_MS))
                        .await;
                    cx.update(|cx| cx.quit());
                })
                .detach();
            }
        });
    i32::from(failed.get())
}

/// The default window size, in logical pixels.
pub const MAIN_SIZE: (f32, f32) = (1000.0, 700.0);

/// Everything the app sets up before its window opens: widgets, fonts, dev
/// mode, the platform and backend choice, the path-loss feed, and the quit on
/// last window closed.
pub fn init_app(cx: &mut App, setup: AppSetup) {
    #[cfg(any(test, feature = "shoot"))]
    if setup.closed_worker_gate {
        jobs::WorkerGate::install_closed(cx);
    }
    gpui_component::init(cx);
    fonts::init(cx);
    DevMode::init(cx);
    platform::set_platform_rc(cx, setup.platform);
    cx.set_global(setup.backends);
    cx.set_global(PathLossFeed(platform::path_loss::install()));
    cx.on_window_closed(|cx, closed| {
        platform::release_owner(closed);
        if cx.windows().is_empty() {
            cx.quit();
        }
    })
    .detach();
}

/// Opens the main window at `size` (logical pixels).
///
/// # Errors
/// If the window cannot be opened.
pub fn open_main_window(cx: &mut App, size_px: (f32, f32)) -> gpui::Result<WindowHandle<Root>> {
    let bounds = Bounds::centered(None, size(px(size_px.0), px(size_px.1)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(800.0), px(560.0))),
            titlebar: Some(TitlebarOptions {
                title: Some("FileManager".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| {
            #[cfg(windows)]
            platform::win_dialog::capture_owner(window);
            let shell = Shell::open(window, cx);
            cx.new(|cx| Root::new(shell, window, cx))
        },
    )
}
