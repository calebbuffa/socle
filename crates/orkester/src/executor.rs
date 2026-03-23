//! Executor trait for task dispatch.

use std::future::Future as StdFuture;
use std::pin::Pin;

type Task = Box<dyn FnOnce() + Send + 'static>;

/// A boxed, pinned, sendable future returning `()`.
pub type BoxFuture = Pin<Box<dyn StdFuture<Output = ()> + Send + 'static>>;

/// Trait for dispatching tasks to a scheduling context.
///
/// Implement this to create custom execution contexts that can be
/// registered via [`AsyncSystem::register_context`](crate::AsyncSystem::register_context).
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
/// let gpu = system.register_context("gpu", GpuThreadExecutor::new());
/// system.run(gpu, || upload_texture(data));
/// ```
pub trait Executor: Send + Sync {
    /// Dispatch a synchronous task for execution.
    fn execute(&self, task: Task);

    /// Spawn an async future on this executor.
    ///
    /// The default implementation dispatches the future as a blocking task
    /// via [`execute`](Executor::execute) using a minimal `block_on` driver.
    /// Executors backed by an async runtime (e.g. tokio) should override
    /// this to use their native spawn mechanism.
    fn spawn_future(&self, future: BoxFuture) {
        self.execute(Box::new(move || {
            crate::block_on::block_on(future);
        }));
    }

    /// Returns `true` if the current thread belongs to this executor.
    ///
    /// Used to optimize dispatch: if already on the target thread, the task
    /// runs inline instead of being queued.
    fn is_current_thread(&self) -> bool {
        false
    }
}

/// Executor backed by a tokio runtime handle.
///
/// Uses `spawn_blocking` for synchronous tasks and `spawn` for async futures.
///
/// # Example
///
/// ```rust,ignore
/// let rt = tokio::runtime::Runtime::new().unwrap();
/// let system = AsyncSystem::builder()
///     .executor(TokioExecutor::new(rt.handle().clone()))
///     .build();
/// ```
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

    fn spawn_future(&self, future: BoxFuture) {
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

    fn spawn_future(&self, future: BoxFuture) {
        wasm_bindgen_futures::spawn_local(future);
    }

    fn is_current_thread(&self) -> bool {
        true
    }
}
