//! Structured cancellation via [`Scope`].

use crate::cancellation::CancellationToken;
use crate::context::Context;
use crate::task::Task;

/// A structured cancellation scope.
///
/// Tasks spawned through a `Scope` are automatically cancelled when the
/// scope is dropped. This ensures child work cannot outlive its parent.
///
/// ```rust,ignore
/// let bg = orkester::ThreadPool::new(4);
/// let bg_ctx = bg.context();
/// let mut scope = orkester::Scope::new();
///
/// let a = scope.run(bg_ctx.clone(), || 1 + 1);
/// let b = scope.run(bg_ctx.clone(), || 2 + 2);
///
/// // Dropping `scope` cancels any tasks that haven't completed yet.
/// ```
pub struct Scope {
    token: CancellationToken,
}

impl Scope {
    /// Create a new scope.
    pub fn new() -> Self {
        Self {
            token: CancellationToken::new(),
        }
    }

    /// Returns a reference to this scope's cancellation token.
    pub fn token(&self) -> &CancellationToken {
        &self.token
    }

    /// Run a function on the given context, cancellable by this scope.
    pub fn run<T, F>(&self, context: &Context, f: F) -> Task<T>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        context.run(f).with_cancellation(&self.token)
    }

    /// Run an async closure on the given context, cancellable by this scope.
    pub fn run_async<T, F, Fut>(&self, context: &Context, f: F) -> Task<T>
    where
        T: Send + 'static,
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = T> + Send + 'static,
    {
        context.run_async(f).with_cancellation(&self.token)
    }
}

impl Default for Scope {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        self.token.cancel();
    }
}
