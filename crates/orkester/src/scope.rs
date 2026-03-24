//! Structured cancellation via [`Scope`].
//!
//! A `Scope` owns a [`CancellationToken`] and automatically cancels all
//! tasks spawned through it when dropped. This provides structured
//! concurrency: child tasks cannot outlive their parent scope.

use crate::cancellation::CancellationToken;
use crate::context::Context;
use crate::scheduler::Scheduler;
use crate::task::Task;
use crate::thread_pool::ThreadPool;

/// A structured cancellation scope.
///
/// Tasks spawned through a `Scope` are automatically cancelled when the
/// scope is dropped (unless they have already completed). This ensures
/// that child work does not outlive the parent.
///
/// ```rust,ignore
/// let system = Scheduler::with_threads(4);
/// let mut scope = system.scope();
///
/// let a = scope.run(Context::BACKGROUND, || 1 + 1);
/// let b = scope.run(Context::BACKGROUND, || 2 + 2);
///
/// // Dropping `scope` cancels any tasks that haven't completed yet.
/// ```
pub struct Scope {
    scheduler: Scheduler,
    token: CancellationToken,
}

impl Scope {
    /// Create a new scope bound to the given scheduler.
    pub(crate) fn new(scheduler: Scheduler) -> Self {
        Self {
            scheduler,
            token: CancellationToken::new(),
        }
    }

    /// Returns a reference to this scope's cancellation token.
    pub fn token(&self) -> &CancellationToken {
        &self.token
    }

    /// Run a function on the given context, cancellable by this scope.
    pub fn run<T, F>(&self, context: Context, f: F) -> Task<T>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        self.scheduler
            .run(context, f)
            .with_cancellation(&self.token)
    }

    /// Run an async closure on the given context, cancellable by this scope.
    pub fn run_async<T, F, Fut>(&self, context: Context, f: F) -> Task<T>
    where
        T: Send + 'static,
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = T> + Send + 'static,
    {
        self.scheduler
            .run_async(context, f)
            .with_cancellation(&self.token)
    }

    /// Spawn an async task on the background context, cancellable by this scope.
    pub fn spawn<T, Fut>(&self, fut: Fut) -> Task<T>
    where
        T: Send + 'static,
        Fut: std::future::Future<Output = T> + Send + 'static,
    {
        self.scheduler.spawn(fut).with_cancellation(&self.token)
    }

    /// Run a function in a named thread pool, cancellable by this scope.
    pub fn run_in_pool<T, F>(&self, pool: &ThreadPool, f: F) -> Task<T>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        self.scheduler
            .run_in_pool(pool, f)
            .with_cancellation(&self.token)
    }

    /// Cancel all tasks spawned through this scope.
    pub fn cancel(&self) {
        self.token.cancel();
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        self.token.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    #[test]
    fn scope_cancel_on_drop() {
        let system = Scheduler::with_threads(2);
        let task;
        {
            let scope = system.scope();
            task = scope.run(Context::BACKGROUND, || {
                std::thread::sleep(Duration::from_secs(5));
                42
            });
            // scope drops here, cancelling the token
        }
        let err = task.block().unwrap_err();
        assert_eq!(err.code(), ErrorCode::Cancelled);
    }

    #[test]
    fn scope_allows_completed_tasks() {
        let system = Scheduler::with_threads(2);
        let task;
        {
            let scope = system.scope();
            task = scope.run(Context::BACKGROUND, || 42);
            // Let the task complete before scope drops.
            std::thread::sleep(Duration::from_millis(50));
        }
        // Task completed before cancellation — should succeed.
        assert_eq!(task.block().unwrap(), 42);
    }

    #[test]
    fn scope_explicit_cancel() {
        let system = Scheduler::with_threads(2);
        let scope = system.scope();
        let started = Arc::new(AtomicBool::new(false));
        let started2 = started.clone();

        let task = scope.run(Context::BACKGROUND, move || {
            started2.store(true, Ordering::SeqCst);
            std::thread::sleep(Duration::from_secs(5));
            42
        });

        // Wait for the task to start.
        while !started.load(Ordering::SeqCst) {
            std::thread::yield_now();
        }

        scope.cancel();
        let err = task.block().unwrap_err();
        assert_eq!(err.code(), ErrorCode::Cancelled);
    }

    #[test]
    fn scope_token_accessible() {
        let system = Scheduler::with_threads(1);
        let scope = system.scope();
        assert!(!scope.token().is_cancelled());
        scope.cancel();
        assert!(scope.token().is_cancelled());
    }
}
