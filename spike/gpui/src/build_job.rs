//! The progress throttle and the simulated build job (item A7).
//!
//! Mirrors `apps/desktop/src-tauri/src/tools/tarpack/progress.rs`: at most one
//! event per interval, but the final event is never dropped.

use std::time::{Duration, Instant};

/// One progress event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
}

impl Progress {
    #[must_use]
    pub fn is_final(self) -> bool {
        self.done >= self.total
    }
}

/// Lets an event through at most once per `interval`.
#[derive(Debug)]
pub struct Throttle {
    interval: Duration,
    last: Option<Instant>,
}

impl Throttle {
    #[must_use]
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            last: None,
        }
    }

    /// True when an event at `now` should be sent.
    pub fn admit(&mut self, now: Instant) -> bool {
        let pass = self
            .last
            .is_none_or(|last| now.saturating_duration_since(last) >= self.interval);
        if pass {
            self.last = Some(now);
        }
        pass
    }
}

/// Runs for `total_ms` of wall-clock time, waking every `step`, and calls
/// `emit` for each event the throttle admits. `done` is elapsed milliseconds,
/// so the job takes `total_ms` even where a 1 ms sleep really lasts 15 ms.
/// The final event always goes out, exactly once.
pub fn run_job(total_ms: u64, step: Duration, interval: Duration, mut emit: impl FnMut(Progress)) {
    let mut throttle = Throttle::new(interval);
    let start = Instant::now();
    loop {
        std::thread::sleep(step);
        let elapsed = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let event = Progress {
            done: elapsed.min(total_ms),
            total: total_ms,
        };
        if event.is_final() {
            emit(event);
            return;
        }
        if throttle.admit(Instant::now()) {
            emit(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throttle_admits_first_then_waits_for_interval() {
        let base = Instant::now();
        let mut t = Throttle::new(Duration::from_millis(50));
        assert!(t.admit(base));
        assert!(!t.admit(base + Duration::from_millis(10)));
        assert!(!t.admit(base + Duration::from_millis(49)));
        assert!(t.admit(base + Duration::from_millis(50)));
        assert!(!t.admit(base + Duration::from_millis(60)));
    }

    #[test]
    fn progress_is_throttled_and_ordered() {
        let mut seen = Vec::new();
        // 300 ms of 1 ms ticks through a 20 ms throttle.
        run_job(
            300,
            Duration::from_millis(1),
            Duration::from_millis(20),
            |p| seen.push(p),
        );
        assert!(
            seen.len() <= 16,
            "throttle let {} events through",
            seen.len()
        );
        assert!(seen.len() >= 2);
        assert!(seen.windows(2).all(|w| w[0].done < w[1].done), "ordered");
        assert_eq!(
            seen.last().copied(),
            Some(Progress {
                done: 300,
                total: 300
            })
        );
        assert_eq!(seen.iter().filter(|p| p.is_final()).count(), 1);
    }

    #[test]
    fn final_event_survives_a_huge_interval() {
        let mut seen = Vec::new();
        run_job(
            30,
            Duration::from_millis(1),
            Duration::from_secs(3600),
            |p| seen.push(p),
        );
        assert!(
            (1..=2).contains(&seen.len()),
            "first event plus the final one"
        );
        assert!(seen.last().unwrap().is_final());
    }
}
