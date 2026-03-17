//! Task processor trait and default thread-pool implementation.
//!
//! Modeled after cesium-native's `ITaskProcessor`. The engine provides an
//! implementation that dispatches work to a thread pool, and this library
//! submits CPU-heavy and I/O tasks through it.
//!
//! A default [`ThreadPoolTaskProcessor`] is provided using `std::thread` and
//! channels — no tokio or async runtime required.

use std::future::Future;
use std::pin::pin;
use std::sync::mpsc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread;

/// A task processor that dispatches work to background threads.
///
/// This is the Rust equivalent of cesium-native's `ITaskProcessor`.
/// The integration layer (game engine, viewer, etc.) implements this trait
/// to route work through its own job system or thread pool.
///
/// The library calls [`start_task`](TaskProcessor::start_task) to submit
/// work that should run off the main thread. The implementation decides
/// *how* and *where* to run it.
pub trait TaskProcessor: Send + Sync {
    /// Submit a synchronous task for execution on a worker thread.
    ///
    /// The task is a boxed closure that must be `Send` (it will be transferred
    /// to another thread). The implementation should run it as soon as a worker
    /// is available. This method must not block.
    fn start_task(&self, task: Box<dyn FnOnce() + Send>);
}

struct NoopWaker;
impl Wake for NoopWaker {
    fn wake(self: std::sync::Arc<Self>) {}
}

/// Minimal `block_on` — polls a future to completion on the current thread.
///
/// Suitable for futures that complete synchronously or with minimal suspension
/// (e.g. [`SlpkProvider`](crate::slpk::SlpkProvider) whose async methods do
/// blocking I/O and return immediately).
///
/// For truly async providers (e.g. `RestProvider` with reqwest), use a
/// runtime-aware block_on like `tokio::runtime::Handle::current().block_on(fut)`.
pub fn block_on<F: Future>(fut: F) -> F::Output {
    let waker = Waker::from(std::sync::Arc::new(NoopWaker));
    let mut cx = Context::from_waker(&waker);
    let mut fut = pin!(fut);

    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(val) => return val,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

/// A simple thread-pool [`TaskProcessor`] using `std::thread`.
///
/// Spawns a fixed number of worker threads that pull tasks from a shared
/// channel. This is the default implementation — production integrations
/// should provide their own `TaskProcessor` backed by their engine's job system.
///
/// Use [`block_on`] inside task closures to run async provider methods.
/// For truly async I/O (reqwest/REST), use a tokio-aware task processor instead.
pub struct ThreadPoolTaskProcessor {
    sender: mpsc::Sender<Box<dyn FnOnce() + Send>>,
    _workers: Vec<thread::JoinHandle<()>>,
}

impl ThreadPoolTaskProcessor {
    /// Create a thread pool with `num_threads` worker threads.
    ///
    /// A reasonable default is `std::thread::available_parallelism()` minus 1
    /// (reserving the main thread).
    pub fn new(num_threads: usize) -> Self {
        let num_threads = num_threads.max(1);
        let (sender, receiver) = mpsc::channel::<Box<dyn FnOnce() + Send>>();
        let receiver = std::sync::Arc::new(std::sync::Mutex::new(receiver));

        let mut workers = Vec::with_capacity(num_threads);
        for i in 0..num_threads {
            let rx = std::sync::Arc::clone(&receiver);
            let handle = thread::Builder::new()
                .name(format!("i3s-worker-{i}"))
                .spawn(move || {
                    loop {
                        let task = {
                            let lock = rx.lock().unwrap();
                            lock.recv()
                        };
                        match task {
                            Ok(f) => f(),
                            Err(_) => break, // channel closed, exit
                        }
                    }
                })
                .expect("failed to spawn worker thread");
            workers.push(handle);
        }

        Self {
            sender,
            _workers: workers,
        }
    }

    /// Create a thread pool with a worker count based on available parallelism.
    ///
    /// Uses `available_parallelism() - 1` (minimum 1) to leave the main thread free.
    pub fn default_pool() -> Self {
        let cpus = thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        Self::new((cpus.saturating_sub(1)).max(1))
    }
}

impl TaskProcessor for ThreadPoolTaskProcessor {
    fn start_task(&self, task: Box<dyn FnOnce() + Send>) {
        let _ = self.sender.send(task);
    }
}
