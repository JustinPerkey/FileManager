//! Manual check of the native file dialogs (Windows only; see
//! `docs/gpui-dialogs-check.md`). Never part of the exe.
//!
//! Usage: `cargo run -p filemanager-gpui --example dialog_check -- <folder>`
//!
//! Opens a window (the dialogs' owner), then in turn an open dialog filtered
//! to `*.toml`, a folder picker, and a save dialog for `.tar.gz`. Each result
//! is written with `{:?}` to `dialog-check.txt` in the current directory, so a
//! lone surrogate shows as `\u{d800}`.

#[cfg(windows)]
mod check {
    use std::fmt::Write as _;
    use std::path::PathBuf;

    use fm_app::platform::{
        gpui::GpuiPlatform, platform, set_platform, FileFilter, OpenSpec, PickKind, SaveSpec,
    };
    use gpui::{div, App, AppContext as _, IntoElement, Render, Window, WindowOptions};

    struct Blank;

    impl Render for Blank {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            div()
        }
    }

    pub fn main() {
        let start: Option<PathBuf> = std::env::args_os().nth(1).map(PathBuf::from);
        gpui_platform::application().run(move |cx: &mut App| {
            set_platform(cx, GpuiPlatform);
            let opened = cx.open_window(WindowOptions::default(), |window, cx| {
                fm_app::platform::win_dialog::capture_owner(window);
                cx.new(|_| Blank)
            });
            if opened.is_err() {
                cx.quit();
                return;
            }
            cx.spawn(async move |cx| {
                let mut report = String::new();
                let open = OpenSpec {
                    kind: PickKind::File,
                    filters: vec![FileFilter {
                        name: "Manifest (*.toml)".into(),
                        patterns: vec!["*.toml".into()],
                    }],
                    start_dir: start.clone(),
                    title: Some("Open a manifest".into()),
                };
                let folder = OpenSpec {
                    kind: PickKind::Folder,
                    filters: Vec::new(),
                    start_dir: start.clone(),
                    title: Some("Pick a folder".into()),
                };
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
                let task = cx.update(|cx| platform(cx).pick(open, cx));
                let _ = writeln!(report, "open: {:?}", task.await);
                let task = cx.update(|cx| platform(cx).pick(folder, cx));
                let _ = writeln!(report, "folder: {:?}", task.await);
                let task = cx.update(|cx| platform(cx).pick_save(save, cx));
                let _ = writeln!(report, "save: {:?}", task.await);
                if let Err(e) = std::fs::write("dialog-check.txt", &report) {
                    eprintln!("could not write dialog-check.txt: {e}");
                }
                print!("{report}");
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
