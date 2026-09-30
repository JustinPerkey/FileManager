//! Throttles the build's progress events for the webview.
//!
//! `fm-tarpack` reports every chunk it reads (about 146,000 events for a 1 GB
//! entry). The shell forwards a subsequence of them, unchanged and in order,
//! so every guarantee of the library still holds for what the UI receives.

use std::time::{Duration, Instant};

use fm_tarpack::archive::{BuildPhase, Progress};

/// Minimum time between forwarded chunk events.
pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(50);

/// Decides which events to forward. The clock is a parameter, so tests pass
/// synthetic instants. It never holds an event back, computes, clamps or
/// merges `bytes_done`, and needs no timer.
#[derive(Debug)]
pub struct ProgressCoalescer {
    interval: Duration,
    prev_phase: Option<BuildPhase>,
    prev_entry_id: Option<String>,
    last_forwarded: Option<Instant>,
}

impl ProgressCoalescer {
    pub fn new(interval: Duration) -> Self {
        ProgressCoalescer {
            interval,
            prev_phase: None,
            prev_entry_id: None,
            last_forwarded: None,
        }
    }

    /// Returns `event` unchanged when it should be sent, `None` when dropped.
    /// Forwards: the first event of each phase; the first event of each file
    /// entry (its id differs from the previous offered event's); each phase's
    /// final event; and any event at least `interval` after the last one
    /// forwarded.
    pub fn offer(&mut self, event: Progress, now: Instant) -> Option<Progress> {
        let new_phase = self.prev_phase != Some(event.phase);
        let new_entry = event.entry_id.is_some() && event.entry_id != self.prev_entry_id;
        let is_final = event.bytes_done == event.bytes_total && event.entry_id.is_none();
        let due = self
            .last_forwarded
            .is_none_or(|t| now.saturating_duration_since(t) >= self.interval);

        self.prev_phase = Some(event.phase);
        self.prev_entry_id = event.entry_id.clone();

        if new_phase || new_entry || is_final || due {
            self.last_forwarded = Some(now);
            Some(event)
        } else {
            None
        }
    }
}

/// A progress sink for `write_archive` that coalesces with the real clock and
/// passes what survives to `emit`, synchronously and in order.
pub fn coalescing(mut emit: impl FnMut(Progress)) -> impl FnMut(Progress) {
    let mut coalescer = ProgressCoalescer::new(PROGRESS_INTERVAL);
    move |event| {
        if let Some(event) = coalescer.offer(event, Instant::now()) {
            emit(event);
        }
    }
}
