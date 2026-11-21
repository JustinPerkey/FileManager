//! Reporting dropped names GPUI cannot deliver.
//!
//! When Explorer drops a file whose name is not valid Unicode, GPUI skips it
//! and only writes a log record. [`install`] puts a logger in place that
//! recognises that record and signals on a channel, so the shell can tell the
//! user instead of losing the name silently. The `gpui_message_text_is_pinned`
//! test (Windows) fails if GPUI changes the text.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use log::{Level, LevelFilter, Log, Metadata, Record};

/// What GPUI logs, as the start of its message (it appends the decoding
/// error), when it skips a dropped name.
pub const PATH_LOSS_MESSAGE: &str = "dragged file name is not UTF-16";

/// Whether `record` is GPUI skipping a dropped name.
fn is_path_loss(record: &Record<'_>) -> bool {
    record.level() == Level::Error
        && record.target().starts_with("gpui")
        && record
            .args()
            .to_string()
            .strip_prefix(PATH_LOSS_MESSAGE)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(':'))
}

/// A logger that signals on a match and forwards nothing else anywhere (in
/// debug builds, to stderr).
pub struct PathLossLogger {
    sender: async_channel::Sender<()>,
}

impl PathLossLogger {
    #[must_use]
    pub fn new(sender: async_channel::Sender<()>) -> Self {
        Self { sender }
    }
}

impl Log for PathLossLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= Level::Error || cfg!(debug_assertions)
    }

    fn log(&self, record: &Record<'_>) {
        if is_path_loss(record) {
            // Unbounded: never blocks, and a closed channel is not an error.
            let _ = self.sender.try_send(());
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
pub fn install() -> async_channel::Receiver<()> {
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

/// How long signals keep coalescing when no drag events arrive.
const COALESCE: Duration = Duration::from_secs(1);

/// Turns signals into at most one notice per drag.
#[derive(Debug, Default)]
pub struct Coalescer {
    drag_active: bool,
    raised: bool,
    last: Option<Instant>,
}

impl Coalescer {
    /// A file drag entered the window.
    pub fn drag_started(&mut self) {
        self.drag_active = true;
        self.raised = false;
    }

    /// The drag was dropped or left the window.
    pub fn drag_ended(&mut self, now: Instant) {
        self.drag_active = false;
        self.last = Some(now);
    }

    /// A signal arrived. True when it should raise a notice.
    pub fn signal(&mut self, now: Instant) -> bool {
        let recent = self
            .last
            .is_some_and(|last| now.saturating_duration_since(last) < COALESCE);
        let coalesced = self.raised && (self.drag_active || recent);
        self.last = Some(now);
        self.raised = true;
        !coalesced
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(level: Level, target: &str, message: &str) -> bool {
        is_path_loss(
            &Record::builder()
                .level(level)
                .target(target)
                .args(format_args!("{message}"))
                .build(),
        )
    }

    #[test]
    fn matching_record_signals() {
        let (tx, rx) = async_channel::unbounded();
        let logger = PathLossLogger::new(tx);
        logger.log(
            &Record::builder()
                .level(Level::Error)
                .target("gpui_windows::clipboard")
                .args(format_args!("{PATH_LOSS_MESSAGE}: invalid utf-16"))
                .build(),
        );
        assert_eq!(rx.try_recv(), Ok(()));
        assert!(record(Level::Error, "gpui", PATH_LOSS_MESSAGE));
    }

    #[test]
    fn other_records_do_not_signal() {
        assert!(!record(Level::Warn, "gpui_windows", PATH_LOSS_MESSAGE));
        assert!(!record(Level::Error, "other", PATH_LOSS_MESSAGE));
        assert!(!record(Level::Error, "gpui", "something else"));
        assert!(!record(
            Level::Error,
            "gpui",
            "dragged file name is not UTF-16x"
        ));
        let (tx, rx) = async_channel::unbounded();
        PathLossLogger::new(tx).log(
            &Record::builder()
                .level(Level::Error)
                .target("gpui")
                .args(format_args!("unable to read file name of dragged file"))
                .build(),
        );
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn matches_within_one_drag_coalesce() {
        let t0 = Instant::now();
        let mut c = Coalescer::default();
        c.drag_started();
        assert!(c.signal(t0), "first match raises");
        assert!(!c.signal(t0 + Duration::from_secs(30)), "same drag");
        c.drag_ended(t0 + Duration::from_secs(31));
        assert!(!c.signal(t0 + Duration::from_millis(31_500)), "just after");
        c.drag_started();
        assert!(c.signal(t0 + Duration::from_secs(40)), "next drag raises");
        // No drag events at all: coalesce for one second.
        let mut c = Coalescer::default();
        assert!(c.signal(t0));
        assert!(!c.signal(t0 + Duration::from_millis(500)));
        assert!(c.signal(t0 + Duration::from_millis(1600)));
    }

    #[cfg(windows)]
    #[test]
    fn gpui_message_text_is_pinned() {
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
        let found = std::fs::read_dir(&src)
            .expect("gpui-pre-windows has a src directory")
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "rs"))
            .any(|e| {
                std::fs::read_to_string(e.path()).is_ok_and(|text| text.contains(PATH_LOSS_MESSAGE))
            });
        assert!(
            found,
            "GPUI changed how it reports undeliverable dropped names; update \
             path_loss.rs before upgrading"
        );
    }
}
