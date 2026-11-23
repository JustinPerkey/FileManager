//! Background work. The rule: models never call a backend or the filesystem
//! on the UI thread. A model owns its tool's backend in a [`Worker`] and runs
//! every backend call through it; long jobs (archive builds, folder walks,
//! pipeline steps) go through [`spawn_thread`].
//!
//! Both report a panic in the work as [`JobPanicked`] instead of taking the
//! app down. [`Worker::run`] returns `Task<Result<R, JobPanicked>>` so the
//! caller sees a panicking closure; the worker keeps serving later calls.
//!
//! The panic is caught with `AssertUnwindSafe`, so the owned `T` may be left
//! half-mutated and later closures still run on it. A tool should decide
//! whether to rebuild its backend after a [`JobPanicked`].
//!
//! # Observing the loading state in tests
//!
//! Under `TestAppContext` the background executor has already run a model's
//! initial load by the first frame a test can see. In test and `shoot` builds
//! a `WorkerGate` holds every [`Worker`] created after it is installed
//! before each queued closure, with an async wait (nothing blocks). Install
//! it before the tools are built (`AppSetup::for_tests().with_closed_worker_gate()`,
//! or `WorkerGate::install_closed` before building a model), assert
//! `loading()`, then call `open()` on the handle and `run_until_parked`.
//! The gate is a per-app global, so parallel tests do not affect each other.

use std::any::Any;
use std::fmt;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[cfg(any(test, feature = "shoot"))]
pub use gate::{WorkerGate, WorkerGateHandle};
use gpui::{App, Task};

#[cfg(any(test, feature = "shoot"))]
mod gate {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex, MutexGuard};
    use std::task::{Context, Poll, Waker};

    use gpui::{App, Global};

    #[derive(Default)]
    struct State {
        open: bool,
        permits: usize,
        wakers: Vec<Waker>,
    }

    #[derive(Default)]
    pub(super) struct Shared(Mutex<State>);

    impl Shared {
        fn lock(&self) -> MutexGuard<'_, State> {
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        }

        fn change(&self, f: impl FnOnce(&mut State)) {
            let wakers = {
                let mut state = self.lock();
                f(&mut state);
                std::mem::take(&mut state.wakers)
            };
            for waker in wakers {
                waker.wake();
            }
        }

        /// Resolves when the gate is open or a permit is available (and takes it).
        pub(super) fn pass(self: &Arc<Self>) -> Pass {
            Pass(self.clone())
        }
    }

    pub(super) struct Pass(Arc<Shared>);

    impl Future for Pass {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            let mut state = self.0.lock();
            if state.open {
                return Poll::Ready(());
            }
            if state.permits > 0 {
                state.permits -= 1;
                return Poll::Ready(());
            }
            state.wakers.push(cx.waker().clone());
            Poll::Pending
        }
    }

    /// A per-app gate that holds [`super::Worker`] jobs until it is opened.
    /// Test and `shoot` builds only.
    pub struct WorkerGate {
        shared: Arc<Shared>,
    }

    impl Global for WorkerGate {}

    impl WorkerGate {
        /// Installs a closed gate on this app: Workers created afterwards wait
        /// before running each queued closure until the gate opens.
        pub fn install_closed(cx: &mut App) -> WorkerGateHandle {
            let shared = Arc::new(Shared::default());
            cx.set_global(Self {
                shared: shared.clone(),
            });
            WorkerGateHandle { shared }
        }

        /// A handle to the installed gate, if there is one.
        #[must_use]
        pub fn handle(cx: &App) -> Option<WorkerGateHandle> {
            cx.try_global::<Self>().map(|g| WorkerGateHandle {
                shared: g.shared.clone(),
            })
        }

        pub(super) fn shared(cx: &App) -> Option<Arc<Shared>> {
            cx.try_global::<Self>().map(|g| g.shared.clone())
        }
    }

    /// Controls an installed [`WorkerGate`].
    #[derive(Clone)]
    pub struct WorkerGateHandle {
        shared: Arc<Shared>,
    }

    impl WorkerGateHandle {
        /// Lets every waiting and future job run.
        pub fn open(&self) {
            self.shared.change(|s| s.open = true);
        }

        /// Holds jobs again.
        pub fn close(&self) {
            self.shared.change(|s| s.open = false);
        }

        /// Lets exactly one queued job run (across all workers).
        pub fn release_one(&self) {
            self.shared.change(|s| s.permits += 1);
        }
    }
}

/// The work panicked or could not run. Carries the panic message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobPanicked(pub String);

impl fmt::Display for JobPanicked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "background work panicked: {}", self.0)
    }
}

impl std::error::Error for JobPanicked {}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-text panic payload".to_string())
}

type Job<T> = Box<dyn FnOnce(&mut T) + Send>;

/// Owns a value off the main thread and runs closures on it one at a time,
/// in submission order, on GPUI's background executor. Dropping the worker
/// stops it after the queued closures it has already taken.
pub struct Worker<T: Send + 'static> {
    jobs: async_channel::Sender<Job<T>>,
    _task: Task<()>,
}

impl<T: Send + 'static> Worker<T> {
    #[must_use]
    pub fn new(value: T, cx: &App) -> Self {
        let (jobs, queue) = async_channel::unbounded::<Job<T>>();
        #[cfg(any(test, feature = "shoot"))]
        let gate = gate::WorkerGate::shared(cx);
        let task = cx.background_executor().spawn(async move {
            let mut value = value;
            while let Ok(job) = queue.recv().await {
                #[cfg(any(test, feature = "shoot"))]
                if let Some(gate) = &gate {
                    gate.pass().await;
                }
                job(&mut value);
            }
        });
        Self { jobs, _task: task }
    }

    /// Queues `f`; the task resolves, on the caller's executor, to its
    /// result, or to [`JobPanicked`] if `f` panicked or the worker stopped.
    pub fn run<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut T) -> R + Send + 'static,
        cx: &App,
    ) -> Task<Result<R, JobPanicked>> {
        let (tx, rx) = async_channel::bounded(1);
        let job: Job<T> = Box::new(move |value| {
            let outcome = catch_unwind(AssertUnwindSafe(|| f(value)))
                .map_err(|payload| JobPanicked(panic_message(payload.as_ref())));
            let _ = tx.try_send(outcome);
        });
        let queued = self.jobs.try_send(job).is_ok();
        cx.foreground_executor().spawn(async move {
            if !queued {
                return Err(JobPanicked("the worker has stopped".to_string()));
            }
            rx.recv()
                .await
                .unwrap_or_else(|_| Err(JobPanicked("the worker has stopped".to_string())))
        })
    }
}

/// Runs a long job on its own `std::thread`. The job gets a progress sender;
/// the caller gets the receiver and a task for the result. A panic in the job
/// resolves the task to `Err(JobPanicked)`. The receiver closes when the job
/// ends.
pub fn spawn_thread<P: Send + 'static, R: Send + 'static>(
    name: &str,
    job: impl FnOnce(async_channel::Sender<P>) -> R + Send + 'static,
    cx: &App,
) -> (async_channel::Receiver<P>, Task<Result<R, JobPanicked>>) {
    let (progress_tx, progress_rx) = async_channel::unbounded();
    let (result_tx, result_rx) = async_channel::bounded(1);
    let spawned = std::thread::Builder::new().name(name.to_string()).spawn({
        let result_tx = result_tx.clone();
        move || {
            let outcome = catch_unwind(AssertUnwindSafe(|| job(progress_tx)))
                .map_err(|payload| JobPanicked(panic_message(payload.as_ref())));
            let _ = result_tx.send_blocking(outcome);
        }
    });
    if let Err(e) = spawned {
        let _ = result_tx.try_send(Err(JobPanicked(format!("could not start a thread: {e}"))));
    }
    let task = cx.foreground_executor().spawn(async move {
        result_rx
            .recv()
            .await
            .unwrap_or_else(|_| Err(JobPanicked("the job ended without a result".to_string())))
    });
    (progress_rx, task)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::thread::ThreadId;
    use std::time::{Duration, Instant};

    use gpui::TestAppContext;

    use super::*;

    /// Polls until `done`, letting the real thread make progress.
    fn wait_for(cx: &mut TestAppContext, done: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !done() {
            assert!(Instant::now() < deadline, "the job never finished");
            cx.executor().run_until_parked();
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    #[gpui::test]
    async fn worker_runs_in_order(cx: &mut TestAppContext) {
        let worker = cx.update(|cx| Worker::new(Vec::<usize>::new(), cx));
        let tasks: Vec<_> = (0..100)
            .map(|i| cx.update(|cx| worker.run(move |v| v.push(i), cx)))
            .collect();
        for t in tasks {
            t.await.expect("no panic");
        }
        let all = cx.update(|cx| worker.run(|v| v.clone(), cx)).await;
        assert_eq!(all, Ok((0..100).collect::<Vec<_>>()));
    }

    type Log = std::sync::Arc<std::sync::Mutex<Vec<u32>>>;

    fn gated_worker(cx: &mut TestAppContext) -> (Worker<()>, WorkerGateHandle, Log) {
        cx.update(|cx| {
            let handle = WorkerGate::install_closed(cx);
            (Worker::new((), cx), handle, Log::default())
        })
    }

    fn queue(cx: &mut TestAppContext, w: &Worker<()>, log: &Log, n: u32) {
        let log = log.clone();
        cx.update(|cx| {
            w.run(move |()| log.lock().expect("log").push(n), cx)
                .detach();
        });
    }

    fn ran(cx: &mut TestAppContext, log: &Log) -> Vec<u32> {
        cx.executor().run_until_parked();
        log.lock().expect("log").clone()
    }

    #[gpui::test]
    fn closed_gate_holds_jobs_without_blocking(cx: &mut TestAppContext) {
        let (worker, handle, log) = gated_worker(cx);
        queue(cx, &worker, &log, 1);
        queue(cx, &worker, &log, 2);
        assert_eq!(ran(cx, &log), Vec::<u32>::new());
        assert_eq!(ran(cx, &log), Vec::<u32>::new());
        handle.open();
        assert_eq!(ran(cx, &log), [1, 2]);
    }

    #[gpui::test]
    fn release_one_runs_exactly_one(cx: &mut TestAppContext) {
        let (worker, handle, log) = gated_worker(cx);
        queue(cx, &worker, &log, 1);
        queue(cx, &worker, &log, 2);
        assert_eq!(ran(cx, &log), Vec::<u32>::new());
        handle.release_one();
        assert_eq!(ran(cx, &log), [1]);
        handle.release_one();
        assert_eq!(ran(cx, &log), [1, 2]);
    }

    #[gpui::test]
    fn open_gate_runs_queued_jobs_in_order(cx: &mut TestAppContext) {
        let (worker, handle, log) = gated_worker(cx);
        for i in 0..5 {
            queue(cx, &worker, &log, i);
        }
        assert_eq!(ran(cx, &log), Vec::<u32>::new());
        handle.open();
        assert_eq!(ran(cx, &log), [0, 1, 2, 3, 4]);
        handle.close();
        queue(cx, &worker, &log, 9);
        assert_eq!(ran(cx, &log).len(), 5);
    }

    #[gpui::test]
    async fn worker_result_arrives_on_main_thread(cx: &mut TestAppContext) {
        let main = std::thread::current().id();
        let worker = cx.update(|cx| Worker::new(0_u32, cx));
        let ran_on: Result<ThreadId, _> = cx
            .update(|cx| worker.run(|_| std::thread::current().id(), cx))
            .await;
        assert!(ran_on.is_ok());
        assert_eq!(std::thread::current().id(), main, "awaited on main");
    }

    #[gpui::test]
    async fn worker_survives_a_panicking_closure(cx: &mut TestAppContext) {
        let worker = cx.update(|cx| Worker::new(1_u32, cx));
        let bad = cx
            .update(|cx| worker.run(|_| -> u32 { panic!("boom") }, cx))
            .await;
        assert_eq!(bad, Err(JobPanicked("boom".to_string())));
        let good = cx.update(|cx| worker.run(|v| *v + 1, cx)).await;
        assert_eq!(good, Ok(2));
    }

    #[gpui::test]
    fn spawn_thread_progress_is_ordered_and_final_arrives(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let (rx, task) = cx.update(|cx| {
            spawn_thread(
                "test-job",
                |tx: async_channel::Sender<u32>| {
                    for i in 0..50 {
                        tx.send_blocking(i).expect("receiver alive");
                    }
                    "done"
                },
                cx,
            )
        });
        let seen = Rc::new(RefCell::new(Vec::new()));
        let result = Rc::new(RefCell::new(None));
        let (seen_in, result_in) = (seen.clone(), result.clone());
        cx.update(|cx| {
            cx.spawn(async move |_cx| {
                while let Ok(p) = rx.recv().await {
                    seen_in.borrow_mut().push(p);
                }
                *result_in.borrow_mut() = Some(task.await);
            })
            .detach();
        });
        wait_for(cx, || result.borrow().is_some());
        assert_eq!(*result.borrow(), Some(Ok("done")));
        assert_eq!(*seen.borrow(), (0..50).collect::<Vec<_>>());
    }

    #[gpui::test]
    fn spawn_thread_panic_is_reported(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let (_rx, task) = cx.update(|cx| {
            spawn_thread(
                "test-panic",
                |_tx: async_channel::Sender<()>| -> u8 { panic!("job failed") },
                cx,
            )
        });
        let result = Rc::new(RefCell::new(None));
        let result_in = result.clone();
        cx.update(|cx| {
            cx.spawn(async move |_cx| *result_in.borrow_mut() = Some(task.await))
                .detach();
        });
        wait_for(cx, || result.borrow().is_some());
        assert_eq!(
            *result.borrow(),
            Some(Err(JobPanicked("job failed".to_string())))
        );
    }
}
