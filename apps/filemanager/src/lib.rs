//! The `FileManager` GPUI app. The library name (`fm_app`) stays stable across
//! the rename that follows the cutover from the Tauri app.

// The domain crates the tools will use. The placeholder tools do not call them
// yet, and Cargo's `cargo::unused_dependencies` manifest lint warns about an
// unused dependency; these lines go once the tools use the crates.
use fm_core as _;
use fm_dbmanager as _;
use fm_pipeline as _;
use fm_schedule as _;
use fm_tarpack as _;

pub mod app;
pub mod assets;
pub mod fonts;
pub mod tools;

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{px, size, App, AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions};
use gpui_component::Root;

use crate::app::dev_mode::DevMode;
use crate::app::shell::Shell;

/// How long `--self-check` lets the window live after opening it.
const SELF_CHECK_MS: u64 = 500;

/// Opens the main window and runs until it closes. Returns the process exit
/// code: non-zero if the window could not be opened. With `self_check` the app
/// quits by itself shortly after the window has rendered its first frame.
#[must_use]
pub fn run(self_check: bool) -> i32 {
    let failed = Rc::new(Cell::new(false));
    let failed_in = failed.clone();
    gpui_platform::application()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            gpui_component::init(cx);
            fonts::init(cx);
            DevMode::init(cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            if let Err(e) = open_main_window(cx) {
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

fn open_main_window(cx: &mut App) -> gpui::Result<()> {
    let bounds = Bounds::centered(None, size(px(1000.0), px(700.0)), cx);
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
            let shell = Shell::open(window, cx);
            cx.new(|cx| Root::new(shell, window, cx))
        },
    )?;
    Ok(())
}
