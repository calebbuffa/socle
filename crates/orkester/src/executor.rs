//! Executor trait for task dispatch.

use std::future::Future as StdFuture;
use std::pin::Pin;

type Task = Box<dyn FnOnce() + Send + 'static>;

/// A boxed, pinned, sendable future returning `()`.
pub type BoxFuture = Pin<Box<dyn StdFuture<Output = ()> + Send + 'static>>;

/// Trait for dispatching tasks to a scheduling context.
///
/// Implement this to create custom execution contexts and wrap them in a
/// [`Context`](crate::Context) via [`Context::new`](crate::Context::new).
///
/// # Example
///
/// ```rust,ignore
/// struct GpuThreadExecutor { /* ... */ }
///
/// impl Executor for GpuThreadExecutor {
///     fn execute(&self, task: Box<dyn FnOnce() + Send + 'static>) {
///         gpu_thread_queue.push(task);
///     }
/// }
///
/// let gpu_ctx = Context::new(GpuThreadExecutor::new());
/// gpu_ctx.run(|| upload_texture(data));
/// ```
pub trait Executor: Send + Sync {
    /// Dispatch a synchronous task for execution.
    fn execute(&self, task: Task);

    /// Spawn an async future on this executor.
    ///
    /// The default implementation drives the future to completion by blocking
    /// the current thread (`block_on`). This is correct for pure-Rust async
    /// chains that only await orkester [`Task`](crate::Task)s, but **will
    /// deadlock** if the future awaits an external reactor (e.g. a tokio timer
    /// or `reqwest` response). Executors backed by an async runtime must
    /// override this — [`TokioExecutor`] uses `handle.spawn()`, [`WasmExecutor`]
    /// uses `spawn_local`.
    fn spawn(&self, future: BoxFuture) {
        self.execute(Box::new(move || {
            crate::block_on::block_on(future);
        }));
    }

    /// Returns `true` if the current thread belongs to this executor.
    ///
    /// Used to optimize dispatch: if already on the target thread, the task
    /// runs inline instead of being queued.
    fn is_current(&self) -> bool {
        false
    }
}

/// Executor backed by a tokio runtime handle.
///
/// Uses `spawn_blocking` for synchronous tasks and `spawn` for async futures.
/// Create via [`TokioExecutor::new`] with an explicit handle or
/// [`TokioExecutor::current`] when inside a tokio runtime.
#[cfg(feature = "tokio-runtime")]
pub struct TokioExecutor {
    handle: tokio::runtime::Handle,
}

#[cfg(feature = "tokio-runtime")]
impl TokioExecutor {
    /// Create a TokioExecutor from an explicit runtime handle.
    pub fn new(handle: tokio::runtime::Handle) -> Self {
        Self { handle }
    }

    /// Create a TokioExecutor using the current tokio runtime.
    ///
    /// # Panics
    ///
    /// Panics if called outside a tokio runtime context.
    pub fn current() -> Self {
        Self::new(tokio::runtime::Handle::current())
    }
}

#[cfg(feature = "tokio-runtime")]
impl Executor for TokioExecutor {
    fn execute(&self, task: Task) {
        let _ = self.handle.spawn_blocking(task);
    }

    fn spawn(&self, future: BoxFuture) {
        let _ = self.handle.spawn(future);
    }
}

/// Executor for WebAssembly targets.
///
/// Runs synchronous tasks inline (WASM is single-threaded) and spawns
/// async futures via `wasm_bindgen_futures::spawn_local`.
#[cfg(feature = "wasm")]
pub struct WasmExecutor;

#[cfg(feature = "wasm")]
impl Executor for WasmExecutor {
    fn execute(&self, task: Task) {
        task();
    }

    fn spawn(&self, future: BoxFuture) {
        wasm_bindgen_futures::spawn_local(future);
    }

    fn is_current(&self) -> bool {
        true
    }
}

