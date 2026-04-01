use std::sync::Arc;

use crate::executor::Executor;
use crate::task::{ResolveOutput, Task, create_pair};

pub(crate) enum ContextKind {
    Immediate,
    Custom(Arc<dyn Executor>),
}

/// A scheduling context identifying where a task should execute.
///
/// Built-in contexts:
/// - [`Context::IMMEDIATE`] — inline on current thread
///
/// Custom contexts are created via [`Context::new`],
/// [`ThreadPool::context`](crate::ThreadPool::context), or
/// [`WorkQueue::context`](crate::WorkQueue::context).
pub struct Context(pub(crate) ContextKind);

impl Context {
    /// Inline on the thread that completed the prior stage.
    /// No scheduling overhead, but blocks the completing thread.
    pub const IMMEDIATE: Context = Context(ContextKind::Immediate);

    /// Create a custom context backed by the given executor.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let gpu = Context::new(GpuThreadExecutor::new());
    /// gpu.run(|| upload_texture(data));
    /// ```
    pub fn new(executor: impl Executor + 'static) -> Self {
        Context(ContextKind::Custom(Arc::new(executor)))
    }

    /// Returns the executor if this context dispatches asynchronously,
    /// or `None` for `IMMEDIATE` (run inline on the calling thread).
    pub(crate) fn executor_opt(&self) -> Option<Arc<dyn Executor>> {
        match &self.0 {
            ContextKind::Immediate => None,
            ContextKind::Custom(e) => Some(Arc::clone(e)),
        }
    }

    /// Run a closure in this context. Returns a task that resolves to the output.
    ///
    /// For `IMMEDIATE`, runs the closure inline and returns a ready task.
    pub fn run<T, F, R>(&self, f: F) -> Task<T>
    where
        T: Send + 'static,
        F: FnOnce() -> R + Send + 'static,
        R: ResolveOutput<T>,
    {
        let (resolver, out) = create_pair::<T>();
        let work = move || f().resolve_into(resolver);
        match &self.0 {
            ContextKind::Immediate => work(),
            ContextKind::Custom(executor) => {
                if executor.is_current() {
                    work();
                } else {
                    executor.execute(Box::new(work));
                }
            }
        }
        out
    }

    /// Run an async closure in this context.
    pub fn run_async<T, F, Fut>(&self, f: F) -> Task<T>
    where
        T: Send + 'static,
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = T> + Send + 'static,
    {
        let (resolver, out) = create_pair::<T>();
        match &self.0 {
            ContextKind::Immediate => resolver.resolve(crate::block_on::block_on(f())),
            ContextKind::Custom(executor) => {
                executor.spawn(Box::pin(async move {
                    resolver.resolve(f().await);
                }));
            }
        }
        out
    }
}

impl Clone for Context {
    fn clone(&self) -> Self {
        match &self.0 {
            ContextKind::Immediate => Context(ContextKind::Immediate),
            ContextKind::Custom(e) => Context(ContextKind::Custom(Arc::clone(e))),
        }
    }
}

impl PartialEq for Context {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (ContextKind::Immediate, ContextKind::Immediate) => true,
            (ContextKind::Custom(a), ContextKind::Custom(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl Eq for Context {}

impl std::hash::Hash for Context {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match &self.0 {
            ContextKind::Immediate => 0u8.hash(state),
            ContextKind::Custom(e) => {
                1u8.hash(state);
                (Arc::as_ptr(e) as *const () as usize).hash(state);
            }
        }
    }
}
