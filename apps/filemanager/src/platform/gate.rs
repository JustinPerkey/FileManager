//! At most one file dialog in flight per process.
//!
//! Windows disables a dialog's owner only once the dialog window exists, so a
//! quick double-click could open two "modal" dialogs, and closing the first
//! would re-enable the owner while the second is still open. Every dialog
//! request therefore goes through one [`DialogGate`]. A request made while a
//! dialog is open resolves at once as `Ok(None)`, exactly like a cancel: it is
//! not queued (it would pop up unasked after the user finished with the first)
//! and it is not an error.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};

use gpui::{App, Task};

use super::{PickError, PickResult};

/// A try-lock. Clones share the same lock.
#[derive(Clone, Default)]
pub struct DialogGate {
    open: Arc<AtomicBool>,
}

/// Holds the gate; releases it on drop.
pub struct DialogGuard {
    open: Arc<AtomicBool>,
}

impl DialogGate {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The process-wide gate every dialog request uses.
    #[must_use]
    pub fn global() -> Self {
        static GLOBAL: LazyLock<DialogGate> = LazyLock::new(DialogGate::new);
        GLOBAL.clone()
    }

    /// Takes the gate, or `None` if a dialog is already open.
    #[must_use]
    pub fn try_enter(&self) -> Option<DialogGuard> {
        self.open
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| DialogGuard {
                open: self.open.clone(),
            })
    }
}

impl Drop for DialogGuard {
    fn drop(&mut self) {
        self.open.store(false, Ordering::Release);
    }
}

/// Runs the blocking dialog `job` on a thread of its own, behind `gate`.
///
/// If the gate is taken the task resolves to `Ok(None)` and `job` never runs.
/// Otherwise the gate is released only after `job` has returned (also when it
/// panics), and just before the task's result is sent.
pub fn run_dialog(
    gate: &DialogGate,
    job: impl FnOnce() -> PickResult + Send + 'static,
    cx: &App,
) -> Task<PickResult> {
    let Some(guard) = gate.try_enter() else {
        return Task::ready(Ok(None));
    };
    let (tx, rx) = async_channel::bounded(1);
    let spawned = std::thread::Builder::new()
        .name("file-dialog".to_string())
        .spawn(move || {
            let outcome = catch_unwind(AssertUnwindSafe(job)).unwrap_or_else(|_| {
                Err(PickError::Failed {
                    detail: "the dialog thread panicked".to_string(),
                })
            });
            drop(guard);
            let _ = tx.send_blocking(outcome);
        });
    if let Err(e) = spawned {
        // The closure (and its guard) was dropped by the failed spawn.
        return Task::ready(Err(PickError::Failed {
            detail: e.to_string(),
        }));
    }
    cx.foreground_executor().spawn(async move {
        rx.recv().await.unwrap_or_else(|_| {
            Err(PickError::Failed {
                detail: "the dialog thread stopped".to_string(),
            })
        })
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::AtomicUsize;

    use gpui::TestAppContext;

    use super::*;

    fn counted(
        ran: &Arc<AtomicUsize>,
        answer: PickResult,
    ) -> impl FnOnce() -> PickResult + Send + 'static {
        let ran = ran.clone();
        move || {
            ran.fetch_add(1, Ordering::SeqCst);
            answer
        }
    }

    /// A job that signals it started, then blocks until released.
    fn blocking(
        ran: &Arc<AtomicUsize>,
    ) -> (
        impl FnOnce() -> PickResult + Send + 'static,
        async_channel::Receiver<()>,
        async_channel::Sender<()>,
    ) {
        let (started_tx, started_rx) = async_channel::bounded(1);
        let (release_tx, release_rx) = async_channel::bounded::<()>(1);
        let ran = ran.clone();
        let job = move || {
            ran.fetch_add(1, Ordering::SeqCst);
            let _ = started_tx.send_blocking(());
            let _ = release_rx.recv_blocking();
            Ok(Some(PathBuf::from("picked")))
        };
        (job, started_rx, release_tx)
    }

    #[gpui::test]
    async fn second_request_while_open_resolves_none_at_once(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let gate = DialogGate::new();
        let ran = Arc::new(AtomicUsize::new(0));
        let (job, started, release) = blocking(&ran);
        let first = cx.update(|cx| run_dialog(&gate, job, cx));
        started.recv().await.unwrap();
        let second = cx.update(|cx| run_dialog(&gate, counted(&ran, Ok(Some("x".into()))), cx));
        assert_eq!(second.await, Ok(None));
        release.send(()).await.unwrap();
        assert_eq!(first.await, Ok(Some(PathBuf::from("picked"))));
        assert_eq!(ran.load(Ordering::SeqCst), 1);
    }

    #[gpui::test]
    async fn gate_reopens_after_the_dialog_returns(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let gate = DialogGate::new();
        let ran = Arc::new(AtomicUsize::new(0));
        let first = cx.update(|cx| run_dialog(&gate, counted(&ran, Ok(None)), cx));
        assert_eq!(first.await, Ok(None));
        let third = cx.update(|cx| run_dialog(&gate, counted(&ran, Ok(Some("t".into()))), cx));
        assert_eq!(third.await, Ok(Some(PathBuf::from("t"))));
        assert_eq!(ran.load(Ordering::SeqCst), 2);
    }

    #[gpui::test]
    async fn gate_released_when_the_job_panics_or_errors(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let gate = DialogGate::new();
        let panicked = cx.update(|cx| run_dialog(&gate, || panic!("boom"), cx));
        assert!(matches!(panicked.await, Err(PickError::Failed { .. })));
        let err = PickError::Failed { detail: "e".into() };
        let errored = cx.update(|cx| run_dialog(&gate, move || Err(err), cx));
        assert_eq!(errored.await, Err(PickError::Failed { detail: "e".into() }));
        let ran = Arc::new(AtomicUsize::new(0));
        let after = cx.update(|cx| run_dialog(&gate, counted(&ran, Ok(None)), cx));
        assert_eq!(after.await, Ok(None));
        assert_eq!(ran.load(Ordering::SeqCst), 1);
    }

    #[gpui::test]
    async fn open_and_save_share_one_gate(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        // `GpuiPlatform` sends open and save requests through the process-wide
        // gate: an "open" job in flight turns a "save" request into a cancel.
        let gate = DialogGate::global();
        let ran = Arc::new(AtomicUsize::new(0));
        let (job, started, release) = blocking(&ran);
        let open = cx.update(|cx| run_dialog(&gate, job, cx));
        started.recv().await.unwrap();
        let save = cx.update(|cx| run_dialog(&DialogGate::global(), counted(&ran, Ok(None)), cx));
        assert_eq!(save.await, Ok(None));
        release.send(()).await.unwrap();
        drop(open.await);
        assert_eq!(ran.load(Ordering::SeqCst), 1);
        assert!(DialogGate::global().try_enter().is_some());
    }
}
