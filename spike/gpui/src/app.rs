//! The check screen: one window, one button per manual spike item. Every
//! action prints its outcome in the window and appends it to
//! `fm-spike-results.txt` beside the exe.

use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui::{
    div, px, size, App, AppContext as _, Bounds, ClickEvent, Context, Entity, ExternalPaths,
    InteractiveElement as _, IntoElement, ParentElement as _, PathPromptOptions, Render,
    SharedString, Styled as _, Task, Window, WindowBounds, WindowOptions,
};
use gpui_component::button::Button;
use gpui_component::checkbox::Checkbox;
use gpui_component::input::{Input, InputState};
use gpui_component::progress::Progress as ProgressBar;
use gpui_component::{h_flex, v_flex, ActiveTheme as _, Root};

use crate::build_job::{run_job, Progress};
use crate::fallback::{self, Outcome};
use crate::results;
use crate::watch::{watch_file, Watch};

/// Escaped rendering of a path (`{:?}`): a lone surrogate shows as `\u{d800}`
/// instead of being lost, which is exactly what the check screen must reveal.
#[expect(
    clippy::unnecessary_debug_formatting,
    reason = "the escaped form is the point: it exposes non-Unicode names"
)]
#[must_use]
pub fn shown(path: &Path) -> String {
    format!("{path:?}")
}

const LOG_LINES: usize = 14;
const BUILD_MS: u64 = 5000;

#[expect(
    clippy::struct_excessive_bools,
    reason = "independent view flags of a throwaway check screen"
)]
pub struct CheckScreen {
    pub input: Entity<InputState>,
    pub dirty: bool,
    pub confirm_close: bool,
    pub click_count: usize,
    pub dropped: Vec<PathBuf>,
    pub progress: Option<Progress>,
    pub building: bool,
    pub changed_on_disk: bool,
    pub log: Vec<SharedString>,
    results: Option<PathBuf>,
    last_picked: Option<PathBuf>,
    sample: Option<tempfile::TempDir>,
    watch: Option<Watch>,
    tasks: Vec<Task<()>>,
}

impl CheckScreen {
    /// `results`: where to append outcomes; `None` in tests.
    pub fn new(results: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input =
            cx.new(|cx| InputState::new(window, cx).placeholder("type here while a build runs"));
        let this = cx.weak_entity();
        window.on_window_should_close(cx, move |_window, cx| {
            this.update(cx, |screen, cx| {
                if screen.dirty {
                    screen.confirm_close = true;
                    cx.notify();
                    false
                } else {
                    true
                }
            })
            .unwrap_or(true)
        });
        Self {
            input,
            dirty: false,
            confirm_close: false,
            click_count: 0,
            dropped: Vec::new(),
            progress: None,
            building: false,
            changed_on_disk: false,
            log: vec![SharedString::from(crate::domain::link_report())],
            results,
            last_picked: None,
            sample: None,
            watch: None,
            tasks: Vec::new(),
        }
    }

    pub fn note(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        let text: String = text.into();
        if let Some(path) = &self.results {
            results::append(path, &text);
        }
        self.log.push(SharedString::from(text));
        if self.log.len() > LOG_LINES {
            self.log.remove(0);
        }
        cx.notify();
    }

    // A2 -------------------------------------------------------------------

    fn open_dialog(
        &mut self,
        label: &'static str,
        options: PathPromptOptions,
        cx: &mut Context<Self>,
    ) {
        let rx = cx.prompt_for_paths(options);
        let task = cx.spawn(async move |this, cx| {
            let outcome = rx.await;
            this.update(cx, |screen, cx| match outcome {
                Ok(Ok(Some(paths))) => screen.picked(label, &paths, cx),
                Ok(Ok(None)) => screen.note(format!("{label}: cancelled"), cx),
                Ok(Err(e)) => screen.note(format!("{label}: ERROR {e:#}"), cx),
                Err(e) => screen.note(format!("{label}: channel closed {e:?}"), cx),
            })
            .ok();
        });
        self.tasks.push(task);
    }

    fn picked(&mut self, label: &str, paths: &[PathBuf], cx: &mut Context<Self>) {
        for p in paths {
            self.note(
                format!(
                    "{label}: {} (valid unicode: {})",
                    shown(p),
                    p.to_str().is_some()
                ),
                cx,
            );
        }
        self.last_picked = paths.last().cloned();
        // A picked folder: list it with std, so a non-Unicode file name is
        // seen as an OsString and becomes the target of Open / Reveal.
        if let [dir] = paths {
            if dir.is_dir() {
                self.list_folder(&dir.clone(), cx);
            }
        }
    }

    fn list_folder(&mut self, dir: &Path, cx: &mut Context<Self>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            self.note(format!("read_dir {} failed", shown(dir)), cx);
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let valid = path.to_str().is_some();
            self.note(
                format!("  entry {} (valid unicode: {valid})", shown(&path)),
                cx,
            );
            if !valid {
                self.last_picked = Some(path);
            }
        }
    }

    fn save_gpui(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_new_path(&std::env::temp_dir(), Some("gateway.tar.gz"));
        let task = cx.spawn(async move |this, cx| {
            let outcome = rx.await;
            this.update(cx, |screen, cx| {
                let line = match outcome {
                    Ok(Ok(Some(p))) => format!("Save (GPUI): {}", shown(&p)),
                    Ok(Ok(None)) => "Save (GPUI): cancelled".to_string(),
                    Ok(Err(e)) => format!("Save (GPUI): ERROR {e:#}"),
                    Err(e) => format!("Save (GPUI): channel closed {e:?}"),
                };
                screen.note(line, cx);
            })
            .ok();
        });
        self.tasks.push(task);
    }

    fn fallback_dialog(
        &mut self,
        label: &'static str,
        run: fn() -> Outcome,
        cx: &mut Context<Self>,
    ) {
        let (tx, rx) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let _ = tx.send_blocking(run());
        });
        let task = cx.spawn(async move |this, cx| {
            let outcome = rx.recv().await;
            this.update(cx, |screen, cx| {
                let line = match outcome {
                    Ok(Outcome::Picked(paths)) => {
                        screen.picked(label, &paths, cx);
                        return;
                    }
                    Ok(Outcome::Cancelled) => format!("{label}: cancelled"),
                    Ok(Outcome::Panicked(m)) => format!("{label}: PANICKED {m}"),
                    Ok(Outcome::Unavailable) => format!("{label}: unavailable on this OS"),
                    Err(e) => format!("{label}: channel closed {e:?}"),
                };
                screen.note(line, cx);
            })
            .ok();
        });
        self.tasks.push(task);
    }

    // A3 --------------------------------------------------------------------

    pub fn dropped_paths(&mut self, paths: &ExternalPaths, cx: &mut Context<Self>) {
        self.dropped = paths.paths().to_vec();
        self.note(format!("Drop: {} path(s)", paths.paths().len()), cx);
        for p in paths.paths() {
            self.note(
                format!("  {} (valid unicode: {})", shown(p), p.to_str().is_some()),
                cx,
            );
        }
        self.last_picked = paths.paths().last().cloned();
    }

    // A5 --------------------------------------------------------------------

    fn sample_toml(&mut self, cx: &mut Context<Self>) -> Option<PathBuf> {
        if self.sample.is_none() {
            match tempfile::tempdir() {
                Ok(dir) => self.sample = Some(dir),
                Err(e) => {
                    self.note(format!("temp dir failed: {e}"), cx);
                    return None;
                }
            }
        }
        let path = self.sample.as_ref()?.path().join("sample.toml");
        if !path.exists() {
            if let Err(e) = std::fs::write(&path, "title = \"fm-spike sample\"\n") {
                self.note(format!("write sample failed: {e}"), cx);
                return None;
            }
        }
        Some(path)
    }

    // A7 --------------------------------------------------------------------

    pub fn start_build(&mut self, total_ms: u64, cx: &mut Context<Self>) {
        if self.building {
            return;
        }
        self.building = true;
        self.progress = None;
        let (tx, rx) = async_channel::unbounded();
        std::thread::spawn(move || {
            run_job(
                total_ms,
                Duration::from_millis(1),
                Duration::from_millis(50),
                |p| {
                    let _ = tx.send_blocking(p);
                },
            );
        });
        let task = cx.spawn(async move |this, cx| {
            let mut events = 0_u64;
            while let Ok(p) = rx.recv().await {
                events += 1;
                if this
                    .update(cx, |screen, cx| {
                        screen.progress = Some(p);
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
            }
            this.update(cx, |screen, cx| {
                screen.building = false;
                screen.note(
                    format!("Simulated build finished: {events} progress events in {total_ms} ms"),
                    cx,
                );
            })
            .ok();
        });
        self.tasks.push(task);
    }

    fn start_watch(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.sample_toml(cx) else {
            return;
        };
        let (tx, rx) = async_channel::unbounded();
        match watch_file(&path, move || {
            let _ = tx.send_blocking(());
        }) {
            Ok(w) => self.watch = Some(w),
            Err(e) => {
                self.note(format!("Watch: ERROR {e}"), cx);
                return;
            }
        }
        self.changed_on_disk = false;
        self.note(
            format!(
                "Watching {}: edit it in an editor, or use 'Edit watched file'",
                shown(&path)
            ),
            cx,
        );
        let task = cx.spawn(async move |this, cx| {
            while rx.recv().await.is_ok() {
                if this
                    .update(cx, |screen, cx| {
                        screen.changed_on_disk = true;
                        screen.note("Watch: changed on disk", cx);
                    })
                    .is_err()
                {
                    return;
                }
            }
        });
        self.tasks.push(task);
    }

    fn edit_watched(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = self.sample_toml(cx) {
            let text = format!(
                "title = \"edited\"\nstamp = {:?}\n",
                std::time::SystemTime::now()
            );
            match std::fs::write(&path, text) {
                Ok(()) => self.note("Edited the watched file", cx),
                Err(e) => self.note(format!("edit failed: {e}"), cx),
            }
        }
    }

    // Render helpers --------------------------------------------------------

    fn button(
        id: &'static str,
        label: &'static str,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        div()
            .debug_selector(|| id.to_string())
            .child(Button::new(id).label(label).on_click(cx.listener(
                move |this: &mut Self, _: &ClickEvent, window, cx| action(this, window, cx),
            )))
    }

    fn confirm_overlay(cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(cx.theme().background.opacity(0.85))
            .child(
                v_flex()
                    .gap_3()
                    .p_6()
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background)
                    .child("Close without saving?")
                    .child(
                        h_flex()
                            .gap_2()
                            .child(Self::button(
                                "keep-editing",
                                "Keep editing",
                                cx,
                                |this, _, cx| {
                                    this.confirm_close = false;
                                    this.note("Close held: kept editing", cx);
                                },
                            ))
                            .child(Self::button(
                                "close-anyway",
                                "Close",
                                cx,
                                |this, window, cx| {
                                    this.note("Closing window", cx);
                                    this.dirty = false;
                                    this.confirm_close = false;
                                    window.remove_window();
                                },
                            )),
                    ),
            )
    }

    fn dialog_row(cx: &mut Context<Self>) -> impl IntoElement {
        let prompt = |files, directories, multiple| PathPromptOptions {
            files,
            directories,
            multiple,
            prompt: None,
        };
        h_flex()
            .gap_2()
            .flex_wrap()
            .child(Self::button(
                "open-file",
                "Open file",
                cx,
                move |this, _, cx| {
                    this.open_dialog("Open file (GPUI)", prompt(true, false, false), cx);
                },
            ))
            .child(Self::button(
                "open-folder",
                "Open folder",
                cx,
                move |this, _, cx| {
                    this.open_dialog("Open folder (GPUI)", prompt(false, true, false), cx);
                },
            ))
            .child(Self::button(
                "open-multi",
                "Open multiple files",
                cx,
                move |this, _, cx| {
                    this.open_dialog("Open files (GPUI)", prompt(true, false, true), cx);
                },
            ))
            .child(Self::button(
                "save-gpui",
                "Save (GPUI)",
                cx,
                |this, _, cx| {
                    this.save_gpui(cx);
                },
            ))
            .child(Self::button(
                "save-rfd",
                "Save with filter (rfd)",
                cx,
                |this, _, cx| {
                    this.fallback_dialog("Save with filter (rfd)", fallback::save_with_filter, cx);
                },
            ))
            .child(Self::button(
                "open-rfd",
                "Open file (rfd)",
                cx,
                |this, _, cx| {
                    this.fallback_dialog("Open file (rfd)", fallback::open_file, cx);
                },
            ))
    }

    fn system_row(cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .gap_2()
            .flex_wrap()
            .child(Self::button(
                "open-toml",
                "Open .toml in default app",
                cx,
                |this, _, cx| {
                    if let Some(p) = this.sample_toml(cx) {
                        cx.open_with_system(&p);
                        this.note(format!("open_with_system({}) called", shown(&p)), cx);
                    }
                },
            ))
            .child(Self::button(
                "reveal-toml",
                "Reveal .toml in Explorer",
                cx,
                |this, _, cx| {
                    if let Some(p) = this.sample_toml(cx) {
                        cx.reveal_path(&p);
                        this.note(format!("reveal_path({}) called", shown(&p)), cx);
                    }
                },
            ))
            .child(Self::button(
                "open-last",
                "Open last picked/dropped",
                cx,
                |this, _, cx| {
                    if let Some(p) = this.last_picked.clone() {
                        cx.open_with_system(&p);
                        this.note(format!("open_with_system({}) called", shown(&p)), cx);
                    }
                },
            ))
            .child(Self::button(
                "reveal-last",
                "Reveal last picked/dropped",
                cx,
                |this, _, cx| {
                    if let Some(p) = this.last_picked.clone() {
                        cx.reveal_path(&p);
                        this.note(format!("reveal_path({}) called", shown(&p)), cx);
                    }
                },
            ))
    }

    fn state_row(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .gap_2()
            .items_center()
            .child(
                div().debug_selector(|| "dirty".to_string()).child(
                    Checkbox::new("dirty")
                        .label("Dirty (hold window close)")
                        .checked(self.dirty)
                        .on_click(cx.listener(|this, checked: &bool, _, cx| {
                            this.dirty = *checked;
                            this.note(format!("dirty = {checked}"), cx);
                        })),
                ),
            )
            .child(Self::button("count", "Count a click", cx, |this, _, cx| {
                this.click_count += 1;
                cx.notify();
            }))
            .child(format!("clicks: {}", self.click_count))
    }

    fn build_row(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let percent = self.progress.map_or(0.0, |p| {
            #[expect(clippy::cast_precision_loss, reason = "display percentage")]
            let pct = p.done as f32 * 100.0 / p.total.max(1) as f32;
            pct
        });
        let counter = self.progress.map_or_else(
            || "no progress yet".to_string(),
            |p| format!("{} / {} ms", p.done, p.total),
        );
        h_flex()
            .gap_2()
            .items_center()
            .child(Self::button(
                "build",
                "Simulated build (5 s)",
                cx,
                |this, _, cx| {
                    this.start_build(BUILD_MS, cx);
                },
            ))
            .child(
                div()
                    .w(px(240.0))
                    .child(ProgressBar::new("progress").value(percent)),
            )
            .child(counter)
            .child(Self::button("watch", "Watch file", cx, |this, _, cx| {
                this.start_watch(cx);
            }))
            .child(Self::button(
                "edit-watched",
                "Edit watched file",
                cx,
                |this, _, cx| {
                    this.edit_watched(cx);
                },
            ))
            .child(if self.changed_on_disk {
                "changed on disk"
            } else {
                "unchanged"
            })
    }

    /// Hidden until files are dragged over the window (`group_drag_over`).
    fn drag_overlay(cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(cx.theme().accent.opacity(0.6))
            .child("Drop files here")
            .invisible()
            .group_drag_over::<ExternalPaths>("drop", gpui::Styled::visible)
    }
}

impl Render for CheckScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let log: Vec<_> = self.log.iter().map(|l| div().child(l.clone())).collect();
        let mut root = div()
            .id("drop-zone")
            .group("drop")
            .relative()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .p_4()
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                this.dropped_paths(paths, cx);
            }))
            .child(
                v_flex()
                    .gap_3()
                    .child("FileManager - GPUI spike check screen (part A)")
                    .child(Self::dialog_row(cx))
                    .child(Self::system_row(cx))
                    .child(self.state_row(cx))
                    .child(self.build_row(cx))
                    .child(div().w(px(420.0)).child(Input::new(&self.input)))
                    .child(div().text_sm().children(log)),
            )
            .child(Self::drag_overlay(cx));
        if self.confirm_close {
            root = root.child(Self::confirm_overlay(cx));
        }
        root
    }
}

/// Opens the window and runs until it closes. Returns the process exit code.
#[must_use]
pub fn run(self_check: bool) -> i32 {
    let results = results::results_path();
    let failed = std::rc::Rc::new(std::cell::Cell::new(false));
    let failed_in = failed.clone();
    let results_in = results.clone();
    gpui_platform::application()
        .with_assets(crate::assets::Assets)
        .run(move |cx: &mut App| {
            gpui_component::init(cx);
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
                    let results = results_in.clone();
                    let view = cx.new(|cx| CheckScreen::new(Some(results), window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                },
            );
            if let Err(e) = opened {
                results::append(&results_in, &format!("open_window FAILED: {e:#}"));
                failed_in.set(true);
                cx.quit();
                return;
            }
            if self_check {
                results::append(
                    &results_in,
                    "self-check: window opened and rendered one frame",
                );
                cx.spawn(async move |cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(500))
                        .await;
                    cx.update(|cx| cx.quit());
                })
                .detach();
            }
        });
    i32::from(failed.get())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, FileDropEvent, Modifiers, TestAppContext, VisualTestContext};

    fn open(cx: &mut TestAppContext) -> (Entity<CheckScreen>, &mut VisualTestContext) {
        cx.update(gpui_component::init);
        cx.add_window_view(|window, cx| CheckScreen::new(None, window, cx))
    }

    fn center(cx: &mut VisualTestContext, selector: &'static str) -> gpui::Point<gpui::Pixels> {
        cx.debug_bounds(selector)
            .unwrap_or_else(|| panic!("no element with debug selector {selector}"))
            .center()
    }

    /// A7 / A6 model test: an entity method starts a background job; the
    /// entity's state changes on the main thread when it finishes.
    struct Counter {
        started: bool,
        value: Option<u32>,
        finished_on_main: bool,
    }

    impl Counter {
        fn start(&mut self, cx: &mut Context<Self>) {
            self.started = true;
            let main = std::thread::current().id();
            cx.spawn(async move |this, cx| {
                let value = cx.background_spawn(async { 21 * 2 }).await;
                this.update(cx, |c, _| {
                    c.value = Some(value);
                    c.finished_on_main = std::thread::current().id() == main;
                })
                .ok();
            })
            .detach();
        }
    }

    #[gpui::test]
    fn job_finishes_on_main_thread(cx: &mut TestAppContext) {
        let counter = cx.new(|_| Counter {
            started: false,
            value: None,
            finished_on_main: false,
        });
        counter.update(cx, Counter::start);
        assert_eq!(
            counter.read_with(cx, |c, _| c.value),
            None,
            "not finished yet"
        );
        cx.run_until_parked();
        assert_eq!(counter.read_with(cx, |c, _| c.value), Some(42));
        assert!(counter.read_with(cx, |c, _| c.finished_on_main));
    }

    #[gpui::test]
    fn input_accepts_keystrokes(cx: &mut TestAppContext) {
        let (view, cx) = open(cx);
        let input = view.read_with(cx, |v, _| v.input.clone());
        cx.update(|window, cx| input.update(cx, |state, cx| state.focus(window, cx)));
        cx.simulate_input("abc");
        assert_eq!(input.read_with(cx, |s, _| s.value()), "abc");
    }

    #[gpui::test]
    fn button_click_runs_handler(cx: &mut TestAppContext) {
        let (view, cx) = open(cx);
        assert_eq!(view.read_with(cx, |v, _| v.click_count), 0);
        let at = center(cx, "count");
        cx.simulate_click(at, Modifiers::none());
        assert_eq!(view.read_with(cx, |v, _| v.click_count), 1);
    }

    #[gpui::test]
    fn external_drop_delivers_paths(cx: &mut TestAppContext) {
        let (view, cx) = open(cx);
        let paths = ExternalPaths(
            [
                PathBuf::from("C:\\drop\\a.txt"),
                PathBuf::from("C:\\drop\\folder"),
            ]
            .into_iter()
            .collect(),
        );
        let inside = point(px(300.0), px(300.0));
        cx.simulate_event(FileDropEvent::Entered {
            position: inside,
            paths: paths.clone(),
        });
        assert!(
            cx.update(|_, cx| cx.has_active_drag()),
            "overlay condition: drag is active"
        );
        cx.simulate_event(FileDropEvent::Exited);
        assert!(
            !cx.update(|_, cx| cx.has_active_drag()),
            "leave clears the drag"
        );

        cx.simulate_event(FileDropEvent::Entered {
            position: inside,
            paths: paths.clone(),
        });
        cx.simulate_event(FileDropEvent::Pending { position: inside });
        cx.simulate_event(FileDropEvent::Submit { position: inside });
        assert_eq!(view.read_with(cx, |v, _| v.dropped.clone()), paths.paths());
        assert!(
            !cx.update(|_, cx| cx.has_active_drag()),
            "drop clears the drag"
        );
    }

    /// Paths with a lone surrogate must survive routing through the view.
    #[cfg(windows)]
    #[gpui::test]
    fn dropped_lone_surrogate_name_survives_routing(cx: &mut TestAppContext) {
        use std::os::windows::ffi::OsStringExt as _;
        let name = std::ffi::OsString::from_wide(&[0x61, 0xD800, 0x62]);
        let path = PathBuf::from("C:\\drop").join(name);
        assert!(path.to_str().is_none());
        let (view, cx) = open(cx);
        let paths = ExternalPaths([path.clone()].into_iter().collect());
        let inside = point(px(300.0), px(300.0));
        cx.simulate_event(FileDropEvent::Entered {
            position: inside,
            paths,
        });
        cx.simulate_event(FileDropEvent::Submit { position: inside });
        assert_eq!(view.read_with(cx, |v, _| v.dropped.clone()), vec![path]);
    }

    #[gpui::test]
    fn close_is_held_while_dirty(cx: &mut TestAppContext) {
        let (view, cx) = open(cx);
        // Clean: close goes through.
        assert!(cx.simulate_close(), "clean window closes at once");
        // Dirty: close is held and the confirmation shows.
        let at = center(cx, "dirty");
        cx.simulate_click(at, Modifiers::none());
        assert!(view.read_with(cx, |v, _| v.dirty));
        assert!(!cx.simulate_close(), "dirty window must not close");
        assert!(view.read_with(cx, |v, _| v.confirm_close));
        cx.run_until_parked();
        // Keep editing: still open, confirmation gone.
        let at = center(cx, "keep-editing");
        cx.simulate_click(at, Modifiers::none());
        assert!(!view.read_with(cx, |v, _| v.confirm_close));
        assert_eq!(cx.update(|_, cx| cx.windows().len()), 1);
        // Hold again, then Close removes the window.
        assert!(!cx.simulate_close());
        let at = center(cx, "close-anyway");
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(cx.windows().len(), 0, "Close closed the window");
    }

    /// A7: progress reaches the view in order and the final event shows, while
    /// the background thread is a real one.
    #[gpui::test]
    fn build_progress_reaches_the_view(cx: &mut TestAppContext) {
        let (view, cx) = open(cx);
        cx.executor().allow_parking();
        view.update(cx, |v, cx| v.start_build(200, cx));
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while view.read_with(cx, |v, _| v.building) {
            assert!(std::time::Instant::now() < deadline, "build never finished");
            cx.executor().run_until_parked();
            std::thread::sleep(Duration::from_millis(5));
        }
        let last = view
            .read_with(cx, |v, _| v.progress)
            .expect("progress arrived");
        assert!(last.is_final());
    }
}
