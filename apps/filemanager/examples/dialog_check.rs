//! Manual check of the native file dialogs (Windows only; see
//! `docs/gpui-dialogs-check.md`). Never part of the exe.
//!
//! Usage: `cargo run -p filemanager-gpui --example dialog_check -- <folder> [--click]`
//!
//! Opens a window (the dialogs' owner). By default it then shows, in turn, an
//! open dialog filtered to `*.toml`, a folder picker and a save dialog for
//! `.tar.gz`, and quits. With `--click` the window instead holds one Browse
//! button that requests two open dialogs on every click (for the one-dialog check); close the window to finish. Each result is written with `{:?}` to
//! `dialog-check.txt` in `<folder>`, so a lone surrogate shows as `\u{d800}`.

#[cfg(windows)]
mod check {
    use std::cell::Cell;
    use std::fmt::Write as _;
    use std::io::Write as _;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;

    use fm_app::platform::gpui::GpuiPlatform;
    use fm_app::platform::{
        platform, set_platform, FileFilter, OpenSpec, PickKind, PickResult, SaveSpec,
    };
    use gpui::prelude::*;
    use gpui::{div, App, Window, WindowOptions};

    const REPORT: &str = "dialog-check.txt";

    /// The owner window. With a button (`--click`), every click requests two
    /// open dialogs back to back.
    struct Owner {
        browse: Option<Browse>,
    }

    struct Browse {
        report: PathBuf,
        spec: OpenSpec,
        clicks: Rc<Cell<u32>>,
    }

    impl Render for Owner {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let root = div().size_full().p_4();
            let Some(browse) = &self.browse else {
                return root.child("dialog_check: the owner window");
            };
            let report = browse.report.clone();
            let spec = browse.spec.clone();
            let clicks = browse.clicks.clone();
            root.child(
                div()
                    .id("browse")
                    .p_4()
                    .border_1()
                    .cursor_pointer()
                    .child("Browse")
                    .on_click(move |_, _window, cx: &mut App| {
                        clicks.set(clicks.get() + 1);
                        let n = clicks.get();
                        // Two requests from one click: the second must be refused
                        // at once, whatever the machine's speed.
                        for part in ["a", "b"] {
                            let report = report.clone();
                            let task = platform(cx).pick(spec.clone(), cx);
                            cx.spawn(async move |_cx| {
                                append(&report, &format!("click {n}{part}: {:?}\n", task.await));
                            })
                            .detach();
                        }
                    }),
            )
        }
    }

    fn append(report: &Path, text: &str) {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(report);
        if let Err(e) = file.and_then(|mut f| f.write_all(text.as_bytes())) {
            eprintln!("could not write {}: {e}", report.display());
        }
    }

    fn line(out: &mut String, label: &str, result: &PickResult) {
        let _ = writeln!(out, "{label}: {result:?}");
    }

    fn open_spec(start: Option<PathBuf>) -> OpenSpec {
        OpenSpec {
            kind: PickKind::File,
            filters: vec![FileFilter {
                name: "Manifest (*.toml)".into(),
                patterns: vec!["*.toml".into()],
            }],
            start_dir: start,
            title: Some("Open a manifest".into()),
        }
    }

    pub fn main() {
        let mut args = std::env::args_os().skip(1);
        let start: Option<PathBuf> = args.next().map(PathBuf::from);
        let click = args.any(|a| a == "--click");
        let report = start.clone().unwrap_or_default().join(REPORT);
        gpui_platform::application().run(move |cx: &mut App| {
            set_platform(cx, GpuiPlatform);
            let spec = open_spec(start.clone());
            let opened = cx.open_window(WindowOptions::default(), |window, cx| {
                fm_app::platform::win_dialog::capture_owner(window);
                let browse = click.then(|| Browse {
                    report: report.clone(),
                    spec: spec.clone(),
                    clicks: Rc::new(Cell::new(0)),
                });
                cx.new(|_| Owner { browse })
            });
            if opened.is_err() {
                cx.quit();
                return;
            }
            cx.on_window_closed(|cx, closed| {
                fm_app::platform::release_owner(closed);
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            if click {
                return;
            }
            let start = start.clone();
            let report = report.clone();
            cx.spawn(async move |cx| {
                let mut out = String::new();
                let task = cx.update(|cx| platform(cx).pick(spec, cx));
                line(&mut out, "open", &task.await);
                let folder = OpenSpec {
                    kind: PickKind::Folder,
                    filters: Vec::new(),
                    start_dir: start.clone(),
                    title: Some("Pick a folder".into()),
                };
                let task = cx.update(|cx| platform(cx).pick(folder, cx));
                line(&mut out, "folder", &task.await);
                let save = SaveSpec {
                    filters: vec![FileFilter {
                        name: "Gzip archive (.tar.gz)".into(),
                        patterns: vec!["*.gz".into()],
                    }],
                    default_extension: Some("gz".into()),
                    start_dir: start,
                    suggested_name: Some("gateway.tar.gz".into()),
                    title: None,
                };
                let task = cx.update(|cx| platform(cx).pick_save(save, cx));
                line(&mut out, "save", &task.await);
                append(&report, &out);
                print!("{out}");
                cx.update(|cx| cx.quit());
            })
            .detach();
        });
    }
}

#[cfg(windows)]
fn main() {
    check::main();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("dialog_check needs Windows");
}
