//! Reporting dropped names GPUI cannot deliver.
//!
//! When Explorer drops a file whose name is not valid Unicode, GPUI skips it
//! and only writes a log record. [`install`] puts a logger in place that
//! recognises that record and signals on a channel, so the shell can tell the
//! user instead of losing the name silently. The `gpui_message_texts_are_pinned`
//! test (Windows) fails if GPUI changes the text.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use log::{Level, LevelFilter, Log, Metadata, Record};

/// What GPUI logs, as the start of its message (it appends the decoding
/// error), when it skips a dropped name.
pub const PATH_LOSS_MESSAGE: &str = "dragged file name is not UTF-16";

/// What GPUI logs when `DragQueryFileW` returns 0 and it skips the item.
pub const UNREADABLE_NAME_MESSAGE: &str = "unable to read file name of dragged file";

/// Which way GPUI skipped a dropped item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    /// The name is not valid Unicode ([`PATH_LOSS_MESSAGE`]).
    NonUnicodeName,
    /// The name could not be read at all ([`UNREADABLE_NAME_MESSAGE`]).
    UnreadableName,
}

/// `text` is `message`, optionally followed by `':'` and detail.
fn has_message(text: &str, message: &str) -> bool {
    text.strip_prefix(message)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(':'))
}

/// Which skip `record` reports, if it reports one.
fn classify(record: &Record<'_>) -> Option<Signal> {
    if record.level() != Level::Error || !record.target().starts_with("gpui") {
        return None;
    }
    let text = record.args().to_string();
    if has_message(&text, PATH_LOSS_MESSAGE) {
        Some(Signal::NonUnicodeName)
    } else if has_message(&text, UNREADABLE_NAME_MESSAGE) {
        Some(Signal::UnreadableName)
    } else {
        None
    }
}

/// A logger that signals on a match and forwards nothing else anywhere (in
/// debug builds, to stderr).
pub struct PathLossLogger {
    sender: async_channel::Sender<Signal>,
}

impl PathLossLogger {
    #[must_use]
    pub fn new(sender: async_channel::Sender<Signal>) -> Self {
        Self { sender }
    }
}

impl Log for PathLossLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= Level::Error || cfg!(debug_assertions)
    }

    fn log(&self, record: &Record<'_>) {
        if let Some(signal) = classify(record) {
            // Unbounded: never blocks, and a closed channel is not an error.
            let _ = self.sender.try_send(signal);
        }
        if cfg!(debug_assertions) && self.enabled(record.metadata()) {
            eprintln!(
                "[{}] {}: {}",
                record.level(),
                record.target(),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}

static LOGGER: OnceLock<PathLossLogger> = OnceLock::new();

/// Installs the logger once per process and returns the channel it signals
/// on. Later calls (tests that build several apps) return a channel nothing
/// signals on.
#[must_use]
pub fn install() -> async_channel::Receiver<Signal> {
    let (tx, rx) = async_channel::unbounded();
    let logger = PathLossLogger::new(tx);
    if LOGGER.set(logger).is_ok() {
        if let Some(logger) = LOGGER.get() {
            // Another logger already installed is fine: ignore the error.
            if log::set_logger(logger).is_ok() {
                log::set_max_level(LevelFilter::Error);
            }
        }
    }
    rx
}

/// How long after a drag ends its signals still count.
const TAIL: Duration = Duration::from_secs(1);

/// Counts signals only around a drag (from entering until the drop or exit,
/// plus [`TAIL`]) and turns them into at most one notice per drag. A signal
/// outside that window is not about a drop (a paste logs the same text).
#[derive(Debug, Default)]
pub struct Coalescer {
    drag_active: bool,
    ended: Option<Instant>,
    raised: Option<Signal>,
}

impl Coalescer {
    /// A file drag entered the window.
    pub fn drag_started(&mut self) {
        self.drag_active = true;
        self.ended = None;
        self.raised = None;
    }

    /// The drag was dropped or left the window.
    pub fn drag_ended(&mut self, now: Instant) {
        self.drag_active = false;
        self.ended = Some(now);
    }

    /// A signal arrived. Returns the notice kind to show, or `None` when it
    /// is outside a drag or the drag already raised one. The non-Unicode kind
    /// replaces an unreadable-name one raised earlier in the same drag.
    pub fn signal(&mut self, signal: Signal, now: Instant) -> Option<Signal> {
        let in_window = self.drag_active
            || self
                .ended
                .is_some_and(|ended| now.saturating_duration_since(ended) < TAIL);
        if !in_window {
            return None;
        }
        let raise = match self.raised {
            None => true,
            Some(Signal::UnreadableName) => signal == Signal::NonUnicodeName,
            Some(Signal::NonUnicodeName) => false,
        };
        if raise {
            self.raised = Some(signal);
            Some(signal)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(level: Level, target: &str, message: &str) -> Option<Signal> {
        classify(
            &Record::builder()
                .level(level)
                .target(target)
                .args(format_args!("{message}"))
                .build(),
        )
    }

    #[test]
    fn matches_prefix_with_detail() {
        let (tx, rx) = async_channel::unbounded();
        let logger = PathLossLogger::new(tx);
        logger.log(
            &Record::builder()
                .level(Level::Error)
                .target("gpui_windows::clipboard")
                .args(format_args!("{PATH_LOSS_MESSAGE}: invalid utf-16"))
                .build(),
        );
        assert_eq!(rx.try_recv(), Ok(Signal::NonUnicodeName));
        assert_eq!(
            record(Level::Error, "gpui", PATH_LOSS_MESSAGE),
            Some(Signal::NonUnicodeName)
        );
    }

    #[test]
    fn matches_unreadable_name_message() {
        for text in [
            UNREADABLE_NAME_MESSAGE.to_string(),
            format!("{UNREADABLE_NAME_MESSAGE}: os error"),
        ] {
            assert_eq!(
                record(Level::Error, "gpui_windows::clipboard", &text),
                Some(Signal::UnreadableName)
            );
        }
        let (tx, rx) = async_channel::unbounded();
        PathLossLogger::new(tx).log(
            &Record::builder()
                .level(Level::Error)
                .target("gpui")
                .args(format_args!("{UNREADABLE_NAME_MESSAGE}"))
                .build(),
        );
        assert_eq!(rx.try_recv(), Ok(Signal::UnreadableName));
    }

    #[test]
    fn other_records_do_not_signal() {
        assert_eq!(record(Level::Warn, "gpui_windows", PATH_LOSS_MESSAGE), None);
        assert_eq!(record(Level::Error, "other", PATH_LOSS_MESSAGE), None);
        assert_eq!(record(Level::Error, "other", UNREADABLE_NAME_MESSAGE), None);
        assert_eq!(record(Level::Warn, "gpui", UNREADABLE_NAME_MESSAGE), None);
        assert_eq!(record(Level::Error, "gpui", "something else"), None);
        assert_eq!(
            record(Level::Error, "gpui", "dragged file name is not UTF-16x"),
            None
        );
        assert_eq!(
            record(
                Level::Error,
                "gpui",
                "unable to read file name of dragged filex"
            ),
            None
        );
    }

    #[test]
    fn signals_count_only_around_a_drag() {
        let t0 = Instant::now();
        let s = Signal::NonUnicodeName;
        let mut c = Coalescer::default();
        assert_eq!(c.signal(s, t0), None, "no drag yet");
        c.drag_started();
        assert_eq!(c.signal(s, t0), Some(s), "first raises");
        assert_eq!(c.signal(s, t0 + Duration::from_secs(30)), None, "same drag");
        c.drag_ended(t0 + Duration::from_secs(31));
        assert_eq!(c.signal(s, t0 + Duration::from_millis(31_500)), None);
        assert_eq!(c.signal(s, t0 + Duration::from_secs(33)), None, "too late");
        c.drag_started();
        assert_eq!(c.signal(s, t0 + Duration::from_secs(40)), Some(s));
    }

    #[test]
    fn tail_window_is_one_second() {
        let t0 = Instant::now();
        let s = Signal::UnreadableName;
        let mut c = Coalescer::default();
        c.drag_started();
        c.drag_ended(t0);
        assert_eq!(c.signal(s, t0 + Duration::from_millis(999)), Some(s));
        let mut c = Coalescer::default();
        c.drag_started();
        c.drag_ended(t0);
        assert_eq!(c.signal(s, t0 + Duration::from_secs(1)), None);
    }

    #[test]
    fn non_unicode_replaces_unreadable_once() {
        let t0 = Instant::now();
        let mut c = Coalescer::default();
        c.drag_started();
        assert_eq!(
            c.signal(Signal::UnreadableName, t0),
            Some(Signal::UnreadableName)
        );
        assert_eq!(c.signal(Signal::UnreadableName, t0), None);
        assert_eq!(
            c.signal(Signal::NonUnicodeName, t0),
            Some(Signal::NonUnicodeName)
        );
        assert_eq!(c.signal(Signal::UnreadableName, t0), None);
    }

    #[cfg(windows)]
    #[test]
    fn gpui_message_texts_are_pinned() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root")
            .to_path_buf();
        let out = std::process::Command::new(env!("CARGO"))
            .args(["metadata", "--format-version", "1", "--locked"])
            .current_dir(&root)
            .output()
            .expect("cargo metadata runs");
        assert!(out.status.success(), "cargo metadata failed");
        let meta = String::from_utf8(out.stdout).expect("metadata is UTF-8");
        let manifest = meta
            .split("\"manifest_path\":\"")
            .skip(1)
            .filter_map(|s| s.split('"').next())
            .find(|p| p.contains("gpui-pre-windows"))
            .expect("gpui-pre-windows is in the graph")
            .replace("\\\\", "\\");
        let src = std::path::Path::new(&manifest)
            .parent()
            .expect("manifest has a parent")
            .join("src");
        let sources: String = std::fs::read_dir(&src)
            .expect("gpui-pre-windows has a src directory")
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "rs"))
            .filter_map(|e| std::fs::read_to_string(e.path()).ok())
            .collect();
        // The level is part of what the logger matches.
        let found = [PATH_LOSS_MESSAGE, UNREADABLE_NAME_MESSAGE]
            .iter()
            .all(|m| sources.contains(&format!("log::error!(\"{m}")));
        assert!(
            found,
            "GPUI changed how it reports undeliverable dropped names; update \
             path_loss.rs before upgrading"
        );
    }
}
