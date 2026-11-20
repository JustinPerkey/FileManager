//! `fm-spike --pipeline` and `fm-spike --widgets`: the B6 screen and the
//! B1-B5 widget screen in a normal window, for the human's checklist. The
//! theme follows the Windows light/dark setting, live.

use gpui::{px, size, AppContext as _, Bounds, WindowAppearance, WindowBounds, WindowOptions};
use gpui_component::Root;

use crate::assets::Assets;
use crate::pipeline_screen::PipelineScreen;
use crate::theme;
use crate::widgets::{self, WidgetsDemo};

#[derive(Clone, Copy)]
pub enum Screen {
    Pipeline,
    Widgets,
}

/// Dark for the dark and vibrant-dark appearances.
#[must_use]
pub fn is_dark(appearance: WindowAppearance) -> bool {
    matches!(
        appearance,
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    )
}

/// Opens `screen` and runs until the window closes.
#[must_use]
pub fn run(screen: Screen) -> i32 {
    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx| {
            gpui_component::init(cx);
            widgets::init(cx);
            theme::apply(is_dark(cx.window_appearance()), cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let bounds = Bounds::centered(None, size(px(1000.0), px(700.0)), cx);
            let opened = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(800.0), px(560.0))),
                    titlebar: Some(gpui::TitlebarOptions {
                        title: Some("FileManager".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| {
                    // Live: re-apply the token set when Windows switches theme.
                    window
                        .observe_window_appearance(|window, cx| {
                            theme::apply(is_dark(window.appearance()), cx);
                        })
                        .detach();
                    theme::apply(is_dark(window.appearance()), cx);
                    match screen {
                        Screen::Pipeline => {
                            let v = cx.new(|cx| PipelineScreen::new(window, cx));
                            cx.new(|cx| Root::new(v, window, cx))
                        }
                        Screen::Widgets => {
                            let v = cx.new(|cx| WidgetsDemo::new(10_000, window, cx));
                            cx.new(|cx| Root::new(v, window, cx))
                        }
                    }
                },
            );
            if opened.is_err() {
                cx.quit();
            }
        });
    0
}
