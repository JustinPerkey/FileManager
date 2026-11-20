//! B7: `fm-spike --shoot <dir> [--dark]` renders the B6 screen with fixture
//! data into `<dir>/pipeline-chosen[-dark].png` and exits.
//!
//! Method 1 of the plan: GPUI's own `Window::render_to_image` (the Direct3D 11
//! renderer draws the last frame's scene into an offscreen texture, reads it
//! back and converts BGRA to RGBA), encoded by the `image` crate's PNG
//! encoder. It is behind GPUI's `test-support` feature, which the spike's
//! default `shoot` feature turns on. It captures GPUI's own scene, not the
//! screen, so no other window can end up in the picture.

use std::cell::{Cell, RefCell};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gpui::{px, size, AppContext as _, Bounds, WindowBounds, WindowHandle, WindowOptions};
use gpui_component::{ActiveTheme as _, Root};

use crate::assets::Assets;
use crate::check::{HEIGHT_PX, WIDTH_PX};
use crate::pipeline_screen::PipelineScreen;
use crate::theme;
use crate::tokens::{FONT_MONO, FONT_UI};

/// Time the first frame gets to settle (font loading, SVG rasterisation).
const SETTLE: Duration = Duration::from_millis(1500);

/// Renders and writes the PNG. Returns the process exit code. Everything
/// observed is also written to `<dir>/shoot-result.txt`.
#[must_use]
pub fn run(dir: &Path, dark: bool) -> i32 {
    let dir: PathBuf = dir.to_path_buf();
    let _ = std::fs::create_dir_all(&dir);
    let stem = if dark {
        "pipeline-chosen-dark"
    } else {
        "pipeline-chosen"
    };
    let log = Rc::new(RefCell::new(String::new()));
    let code = Rc::new(Cell::new(1));
    let (log_in, code_in, dir_in) = (log.clone(), code.clone(), dir.clone());
    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx| {
            gpui_component::init(cx);
            theme::apply(dark, cx);
            let installed = cx.text_system().all_font_names();
            for family in FONT_UI.iter().chain(FONT_MONO) {
                let _ = writeln!(
                    log_in.borrow_mut(),
                    "font {family}: {}",
                    if installed.iter().any(|n| n == family) {
                        "installed"
                    } else {
                        "NOT installed"
                    }
                );
            }
            let _ = writeln!(
                log_in.borrow_mut(),
                "font used: ui {} mono {}",
                cx.theme().font_family,
                cx.theme().mono_font_family
            );
            let bounds = Bounds::centered(None, size(px(WIDTH_PX), px(HEIGHT_PX)), cx);
            let opened: gpui::Result<WindowHandle<Root>> = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: None,
                    titlebar: Some(gpui::TitlebarOptions {
                        title: Some("FileManager".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| PipelineScreen::new(window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                },
            );
            let handle = match opened {
                Ok(h) => h,
                Err(e) => {
                    let _ = writeln!(log_in.borrow_mut(), "open_window FAILED: {e:#}");
                    cx.quit();
                    return;
                }
            };
            let (log2, code2, dir2) = (log_in.clone(), code_in.clone(), dir_in.clone());
            cx.spawn(async move |cx| {
                cx.background_executor().timer(SETTLE).await;
                let result = cx.update(|cx| capture(handle, &dir2, stem, &log2, cx));
                match result {
                    Ok(()) => code2.set(0),
                    Err(e) => {
                        let _ = writeln!(log2.borrow_mut(), "capture FAILED: {e:#}");
                    }
                }
                cx.update(|cx| cx.quit());
            })
            .detach();
        });
    let _ = writeln!(log.borrow_mut(), "exit code {}", code.get());
    let _ = std::fs::write(
        dir.join(format!("{stem}.shoot-result.txt")),
        log.borrow().as_str(),
    );
    code.get()
}

fn capture(
    handle: WindowHandle<Root>,
    dir: &Path,
    stem: &str,
    log: &Rc<RefCell<String>>,
    cx: &mut gpui::App,
) -> gpui::Result<()> {
    let (image, scale, viewport, boxes) = handle.update(cx, |root, window, cx| {
        let scale = window.scale_factor();
        let viewport = window.viewport_size();
        let boxes = root
            .view()
            .clone()
            .downcast::<PipelineScreen>()
            .map(|v| v.read(cx).boxes.all())
            .unwrap_or_default();
        window
            .render_to_image()
            .map(|img| (img, scale, viewport, boxes))
    })??;
    let _ = writeln!(
        log.borrow_mut(),
        "scale factor {scale}, viewport {:?} logical px, image {}x{} device px",
        viewport,
        image.width(),
        image.height()
    );
    image.save(dir.join(format!("{stem}.png")))?;
    let mut text = String::new();
    for (name, r) in boxes {
        let _ = writeln!(text, "{name} {} {} {} {}", r.x, r.y, r.w, r.h);
    }
    std::fs::write(dir.join(format!("{stem}.boxes.txt")), text)?;
    Ok(())
}
