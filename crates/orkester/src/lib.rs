//! Context-aware task scheduling for Rust.
//!
//! `orkester` provides:
//! - [`Context`] — scheduling token: where should this task run?
//! - [`ThreadPool`] — self-draining background thread pool
//! - [`WorkQueue`] — user-pumped dispatch queue (e.g. for a UI/main thread)
//! - [`Task<T>`](Task) / [`Handle<T>`](Handle) / [`Resolver<T>`](Resolver) — async value types
//! - [`CancellationToken`] — cooperative cancellation
//! - [`Scope`] — structured cancellation (children cancelled when scope drops)
//! - [`Semaphore`] / [`SemaphorePermit`] — async-aware counting semaphore
//! - [`JoinSet<T>`](JoinSet) — track a collection of in-flight tasks
//! - [`LoadOnce<K,V>`](LoadOnce) — deduplicate concurrent in-flight loads by key
//! - [`Sender<T>`](Sender) / [`Receiver<T>`](Receiver) — bounded MPSC channels
//! - [`Executor`] — trait for custom execution backends
//!
//! # API Stability
//!
//! This crate is at version `0.1`. The API may change before `1.0`. Feedback
//! and bug reports are welcome via the [repository][repo].
//!
//! [repo]: https://github.com/calebbuffa/socle
//!
//! # Quick start
//!
//! ```rust,ignore
//! // Background thread pool
//! let bg = orkester::ThreadPool::new(4);
//! let bg_ctx = bg.context();
//!
//! // Optional: user-pumped queue for a main/UI thread
//! let wq = orkester::WorkQueue::new();
//! let main_ctx = wq.context();
//!
//! let task = bg_ctx.run(|| expensive_computation())
//!     .then(&main_ctx, |v| update_ui(v));
//!
//! while !task.is_ready() {
//!     wq.pump();
//! }
//! ```
//!
//! # Feature Flags
//!
//! | Feature | Description |
//! |---------|-------------|
//! | `custom-runtime` *(default)* | Built-in thread pool executor |
//! | `tokio-runtime` | [`TokioExecutor`] backend via `tokio::runtime::Handle` |
//! | `wasm` | [`WasmExecutor`] + `spawn_local` for WebAssembly targets |

mod block_on;
mod cancellation;
mod channel;
mod combinators;
mod context;
mod error;
mod executor;
mod join_set;
mod load_once;
mod scope;
mod semaphore;
mod shared_cell;
pub(crate) mod task;
mod task_cell;
mod thread_pool;
mod timer;
mod work_queue;

pub use cancellation::CancellationToken;
pub use channel::{Receiver, SendError, Sender, TryRecvError, TrySendError};
pub use combinators::{RetryConfig, join_all, join_all_settle, race, retry, timeout};
pub use context::Context;
pub use error::{AsyncError, ErrorCode};
pub use executor::Executor;
#[cfg(feature = "tokio-runtime")]
pub use executor::TokioExecutor;
#[cfg(feature = "wasm")]
pub use executor::WasmExecutor;
pub use join_set::JoinSet;
pub use load_once::LoadOnce;
pub use scope::Scope;
pub use semaphore::{Semaphore, SemaphorePermit};
pub use task::{Handle, ResolveOutput, Resolver, Task};
pub use thread_pool::ThreadPool;
pub use work_queue::WorkQueue;

/// Create a `(Resolver<T>, Task<T>)` pair.
///
/// The resolver is the write side — call [`Resolver::resolve`] or
/// [`Resolver::reject`] to complete the task. The task is the read side.
pub fn pair<T: Send + 'static>() -> (Resolver<T>, Task<T>) {
    task::create_pair()
}

/// Create a bounded multi-producer, single-consumer channel.
pub fn mpsc<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    channel::mpsc(capacity)
}

/// Create a one-shot channel (capacity 1, single send).
pub fn oneshot<T>() -> (Sender<T>, Receiver<T>) {
    channel::oneshot()
}

/// Create a task that is already resolved with `value`.
pub fn resolved<T: Send + 'static>(value: T) -> Task<T> {
    Task::ready(value)
}

/// Spawn a detached, fire-and-forget task in the given context.
///
/// The task runs to completion with no way to observe its result.
pub fn spawn_detached(context: &Context, f: impl FnOnce() + Send + 'static) {
    match context.executor_opt() {
        Some(executor) => executor.execute(Box::new(f)),
        None => f(),
    }
}

/// Create a task that completes after `duration`.
///
/// Uses a shared global timer thread — no worker threads are parked per call.
pub fn delay(duration: std::time::Duration) -> Task<()> {
    if duration.is_zero() {
        return Task::ready(());
    }
    let (res, task) = task::create_pair::<()>();
    let deadline = std::time::Instant::now() + duration;

    struct ResolveOnWake(std::sync::Mutex<Option<Resolver<()>>>);
    impl std::task::Wake for ResolveOnWake {
        fn wake(self: std::sync::Arc<Self>) {
            if let Some(r) = self.0.lock().unwrap_or_else(|p| p.into_inner()).take() {
                r.resolve(());
            }
        }
    }

    let waker = std::task::Waker::from(std::sync::Arc::new(ResolveOnWake(std::sync::Mutex::new(
        Some(res),
    ))));
    timer::TimerWheel::global().register(deadline, waker);
    task
}
