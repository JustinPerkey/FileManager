//! The screenshot harness (feature `shoot`): renders the real GPUI app to PNG
//! from named scenarios, unattended. It is driven by the `filemanager-shoot`
//! binary and is never part of the release exe.
//!
//! A [`Scenario`] names a PNG, a window size, a setup hook (fixture backends,
//! scripted platform answers), the [`Step`]s to play and an optional element
//! to crop to. [`run_scenario`] starts a fresh app for it, plays the steps,
//! waits for the window to settle and writes `<out>/<prefix><name>.png`. One
//! app per scenario, one process per scenario in the binary, so scenarios
//! cannot see each other's state.
//!
//! Notes on how this differs from the plan's wording:
//!
//! - A "debug selector" is a tag, [`crate::app::probe::Tag::tag`]. GPUI's
//!   `debug_selector` can only be read from a test context, not from a real
//!   window, and capture needs a real window (GPUI has no headless renderer on
//!   Windows). A view that a scenario clicks or clips calls `.tag("name")` on
//!   the element. Tags work in debug and release builds of the harness.
//! - `WaitIdle` is a heuristic: three consecutive frames have been presented
//!   with a short pause before each. GPUI exposes no "no task is pending".
//! - `--dark` is a no-op until the theme task provides
//!   `fm_app::theme::set_override`; the harness records the request and the
//!   binary says so.
//! - Capture is Windows only: elsewhere GPUI cannot render to an image.

mod steps;

pub mod scenarios;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gpui::{AnyWindowHandle, App, AsyncApp, Bounds, Pixels};

use crate::app::probe;
use crate::tools::ToolId;
use crate::{assets, AppSetup};

/// One scripted state of the app and the PNG it produces.
pub struct Scenario {
    /// The PNG's stem, for example `pipeline-chosen`.
    pub name: &'static str,
    /// The window's size in logical pixels: 1000x700 by default, 1366x800 for
    /// the Tar Packager.
    pub size: (f32, f32),
    /// Chooses fixtures and scripts the platform's answers.
    pub setup: fn(&mut AppSetup),
    pub steps: &'static [Step],
    /// A tag: crop the PNG to that element's bounds (no margin).
    pub clip: Option<&'static str>,
}

/// One action of a scenario.
pub enum Step {
    SelectTool(ToolId),
    /// Click the centre of the element with this tag.
    Click(&'static str),
    /// Keystrokes in GPUI syntax, space separated: `ctrl-alt-shift-d`.
    Keys(&'static str),
    /// Characters typed into the focused input.
    Type(&'static str),
    /// Files enter the window (the drop overlay shows).
    DragEnter(&'static [&'static str]),
    /// Files are dropped on the window.
    Drop(&'static [&'static str]),
    /// Injects a "file changed on disk" signal into the tool's model.
    Watcher(ToolId),
    /// Escape hatch: drive a model directly.
    Call(fn(&mut gpui::Window, &mut App)),
    /// Run until the window has settled.
    WaitIdle,
    /// Wait this many milliseconds, for timed states (progress).
    Wait(u64),
}

/// Every scenario, in registry order.
#[must_use]
pub fn scenarios() -> Vec<Scenario> {
    scenarios::all()
}

/// The longest any wait may take.
const WAIT_LIMIT: Duration = Duration::from_secs(30);
/// How long the first frame gets to settle (font loading, SVG rasterisation).
const FIRST_FRAME: Duration = Duration::from_millis(1500);
/// The pause before each of the frames `WaitIdle` observes.
const IDLE_PAUSE: Duration = Duration::from_millis(30);
/// How many consecutive frames `WaitIdle` observes.
const IDLE_FRAMES: usize = 3;

/// The reason a scenario failed, ready for a `FAIL <name>: <reason>` line.
pub type Failure = String;

/// Runs `scenario` in a fresh app and writes its PNG. Blocks until the app has
/// quit. Returns the PNG's path.
///
/// # Errors
/// The first thing that went wrong: a selector that does not resolve, a wait
/// that timed out, a display scale other than 100%, a capture or write
/// failure, a window that did not open.
pub fn run_scenario(scenario: &Scenario, out_dir: &Path, prefix: &str) -> Result<PathBuf, Failure> {
    probe::reset();
    let mut setup = AppSetup::real(false);
    // Fixtures and a scripted platform: no real dialogs, no real files.
    setup.platform = Rc::new(crate::platform::fake::FakePlatform::new());
    (scenario.setup)(&mut setup);
    let result: Rc<RefCell<Option<Result<PathBuf, Failure>>>> = Rc::default();
    let result_in = result.clone();
    let target = out_dir.join(format!("{prefix}{}.png", scenario.name));
    let (size, steps, clip) = (scenario.size, scenario.steps, scenario.clip);
    gpui_platform::application()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            crate::init_app(cx, setup);
            let handle = match crate::open_main_window(cx, size) {
                Ok(handle) => handle,
                Err(e) => {
                    *result_in.borrow_mut() =
                        Some(Err(format!("could not open the window: {e:#}")));
                    cx.quit();
                    return;
                }
            };
            let any: AnyWindowHandle = handle.into();
            cx.spawn(async move |cx| {
                let outcome = drive(cx, any, steps, clip, &target).await;
                *result_in.borrow_mut() = Some(outcome.map(|()| target));
                cx.update(|cx| cx.quit());
            })
            .detach();
        });
    let outcome = result.borrow_mut().take();
    outcome.unwrap_or_else(|| Err("the app quit before the scenario finished".into()))
}

async fn drive(
    cx: &mut AsyncApp,
    window: AnyWindowHandle,
    steps: &'static [Step],
    clip: Option<&'static str>,
    target: &Path,
) -> Result<(), Failure> {
    cx.background_executor().timer(FIRST_FRAME).await;
    let scale = cx
        .update(|cx| window.update(cx, |_, w, _| w.scale_factor()))
        .map_err(|e| format!("window is gone: {e:#}"))?;
    #[expect(
        clippy::float_cmp,
        reason = "1.0 is exactly representable; this is a check for 100%"
    )]
    if scale != 1.0 {
        return Err("display scale is not 100%; screenshots are taken at 100%".into());
    }
    for step in steps {
        match step {
            Step::Wait(ms) => {
                cx.background_executor()
                    .timer(Duration::from_millis(*ms))
                    .await;
            }
            Step::WaitIdle => settle(cx, window).await?,
            other => cx.update(|cx| steps::apply(other, window, cx))?,
        }
    }
    settle(cx, window).await?;
    let image = cx.update(|cx| capture(window, clip, cx))?;
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    }
    image
        .save(target)
        .map_err(|e| format!("could not write {}: {e}", target.display()))
}

/// Waits until [`IDLE_FRAMES`] frames have been presented in a row, each after
/// a short pause.
async fn settle(cx: &mut AsyncApp, window: AnyWindowHandle) -> Result<(), Failure> {
    for _ in 0..IDLE_FRAMES {
        cx.background_executor().timer(IDLE_PAUSE).await;
        let presented = Rc::new(RefCell::new(false));
        let flag = presented.clone();
        cx.update(|cx| {
            window.update(cx, |_, w, _| {
                w.on_next_frame(move |_, _| *flag.borrow_mut() = true);
            })
        })
        .map_err(|e| format!("window is gone: {e:#}"))?;
        let mut waited = Duration::ZERO;
        while !*presented.borrow() {
            let step = Duration::from_millis(10);
            cx.background_executor().timer(step).await;
            waited += step;
            if waited >= WAIT_LIMIT {
                return Err(format!("wait timed out after {}s", WAIT_LIMIT.as_secs()));
            }
        }
    }
    Ok(())
}

fn capture(
    window: AnyWindowHandle,
    clip: Option<&'static str>,
    cx: &mut App,
) -> Result<image::RgbaImage, Failure> {
    let (image, scale) = window
        .update(cx, |_, w, _| (w.render_to_image(), w.scale_factor()))
        .map_err(|e| format!("window is gone: {e:#}"))?;
    let image = image.map_err(|e| format!("capture failed: {e:#}"))?;
    match clip {
        None => Ok(image),
        Some(selector) => crop(&image, selector, scale),
    }
}

/// Crops `image` to the tagged element's bounds, scaled to device pixels.
fn crop(image: &image::RgbaImage, selector: &str, scale: f32) -> Result<image::RgbaImage, Failure> {
    let bounds =
        probe::bounds(selector).ok_or_else(|| format!("clip selector not found: {selector}"))?;
    let (x, y, w, h) = device_rect(bounds, scale, (image.width(), image.height()));
    if w == 0 || h == 0 {
        return Err(format!("clip selector {selector} has no area on screen"));
    }
    Ok(image::imageops::crop_imm(image, x, y, w, h).to_image())
}

/// `bounds` in device pixels, rounded to whole pixels and clamped to an image
/// of `limit` (width, height).
fn device_rect(bounds: Bounds<Pixels>, scale: f32, limit: (u32, u32)) -> (u32, u32, u32, u32) {
    let edge = |v: Pixels, max: u32| -> u32 {
        let device = (f32::from(v) * scale).round().max(0.0);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "non-negative and rounded; `as` saturates and `min` clamps to the image"
        )]
        let n = device as u32;
        n.min(max)
    };
    let x0 = edge(bounds.origin.x, limit.0);
    let y0 = edge(bounds.origin.y, limit.1);
    let x1 = edge(bounds.origin.x + bounds.size.width, limit.0);
    let y1 = edge(bounds.origin.y + bounds.size.height, limit.1);
    (x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0))
}

#[cfg(test)]
mod tests;
