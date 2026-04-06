use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::error::{AsyncError, ErrorCode};
use crate::shared_cell::SharedCell;
use crate::task::{Handle, Task, TaskInner, create_pair};
use crate::task_cell::TaskCell;

type Callback = Box<dyn FnOnce() + Send + 'static>;

/// Cooperative cancellation token. Cheap to clone (shared `Arc`).
///
/// Use [`CancellationToken::new`] to create a fresh token, pass clones to
/// any number of tasks via [`Task::with_cancellation`], then call
/// [`CancellationToken::cancel`] to signal them all.
///
/// Cancellation is cooperative — it does not abort running work.  Instead,
/// any task that has not yet completed will be rejected with an
/// [`AsyncError`] whose message is `"cancelled"`.
#[derive(Clone)]
pub struct CancellationToken {
    inner: Arc<TokenInner>,
}

struct TokenInner {
    signalled: AtomicBool,
    callbacks: Mutex<Vec<Callback>>,
}

impl CancellationToken {
    /// Create a new, unsignalled token.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(TokenInner {
                signalled: AtomicBool::new(false),
                callbacks: Mutex::new(Vec::new()),
            }),
        }
    }

    /// Signal cancellation. Fires all registered callbacks.
    pub fn cancel(&self) {
        if self
            .inner
            .signalled
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            let cbs = {
                let mut guard = self.inner.callbacks.lock().expect("token lock");
                std::mem::take(&mut *guard)
            };
            for cb in cbs {
                cb();
            }
        }
    }

    /// Returns `true` once [`cancel`](Self::cancel) has been called.
    pub fn is_cancelled(&self) -> bool {
        self.inner.signalled.load(Ordering::Acquire)
    }

    /// Register a callback that fires when the token is cancelled.
    /// If already cancelled, the callback fires immediately.
    ///
    /// This can be used to hook into external cancellation sources — for
    /// example, cancelling an in-flight HTTP request when the token is
    /// signalled.
    pub fn on_cancel(&self, cb: impl FnOnce() + Send + 'static) {
        if self.is_cancelled() {
            cb();
            return;
        }

        let mut maybe_cb = Some(cb);
        let run_now = {
            let mut guard = self.inner.callbacks.lock().expect("token lock");
            if self.inner.signalled.load(Ordering::Acquire) {
                true
            } else {
                guard.push(Box::new(maybe_cb.take().unwrap()));
                false
            }
        };

        if run_now {
            maybe_cb.take().unwrap()();
        }
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Send + 'static> Task<T> {
    /// Attach a cancellation token. If the token is signalled before the
    /// upstream task completes, the returned task rejects with a
    /// `"cancelled"` error.
    pub fn with_cancellation(self, token: &CancellationToken) -> Task<T> {
        if token.is_cancelled() {
            let (resolver, task) = create_pair();
            resolver.reject(AsyncError::with_code(ErrorCode::Cancelled, "cancelled"));
            return task;
        }

        let (resolver, output) = create_pair::<T>();
        let shared_resolver = Arc::new(Mutex::new(Some(resolver)));

        // Path 1: upstream completes first → forward result.
        let sp1 = shared_resolver.clone();
        match self.inner {
            TaskInner::Ready(result) => {
                let resolver = sp1.lock().expect("cancel resolver lock").take();
                if let Some(resolver) = resolver {
                    match result {
                        Some(Ok(v)) => resolver.resolve(v),
                        Some(Err(e)) => resolver.reject(e),
                        None => resolver.reject(AsyncError::msg("Task already consumed")),
                    }
                }
            }
            TaskInner::Pending(cell) => {
                TaskCell::on_complete(cell, move |result| {
                    let resolver = {
                        let mut guard = sp1.lock().expect("cancel resolver lock");
                        guard.take()
                    };
                    if let Some(resolver) = resolver {
                        match result {
                            Ok(v) => resolver.resolve(v),
                            Err(e) => resolver.reject(e),
                        }
                    }
                });
            }
        }

        // Path 2: token cancelled first → reject.
        let sp2 = shared_resolver;
        token.on_cancel(Box::new(move || {
            let resolver = {
                let mut guard = sp2.lock().expect("cancel resolver lock");
                guard.take()
            };
            if let Some(resolver) = resolver {
                resolver.reject(AsyncError::with_code(ErrorCode::Cancelled, "cancelled"));
            }
        }));

        output
    }
}

impl<T: Clone + Send + 'static> Handle<T> {
    /// Attach a cancellation token. If the token is signalled before the
    /// upstream task completes, the returned task rejects with a
    /// `"cancelled"` error. Does NOT consume the shared task.
    pub fn with_cancellation(&self, token: &CancellationToken) -> Task<T> {
        if token.is_cancelled() {
            let (resolver, task) = create_pair();
            resolver.reject(AsyncError::with_code(ErrorCode::Cancelled, "cancelled"));
            return task;
        }

        let source = Arc::clone(&self.cell);
        let (resolver, output) = create_pair::<T>();
        let shared_resolver = Arc::new(Mutex::new(Some(resolver)));

        // Path 1: upstream completes first → forward result.
        let sp1 = shared_resolver.clone();
        SharedCell::on_complete(source, move |result| {
            let resolver = {
                let mut guard = sp1.lock().expect("cancel resolver lock");
                guard.take()
            };
            if let Some(resolver) = resolver {
                match result {
                    Ok(v) => resolver.resolve(v),
                    Err(e) => resolver.reject(e),
                }
            }
        });

        // Path 2: token cancelled first → reject.
        let sp2 = shared_resolver;
        token.on_cancel(Box::new(move || {
            let resolver = {
                let mut guard = sp2.lock().expect("cancel resolver lock");
                guard.take()
            };
            if let Some(resolver) = resolver {
                resolver.reject(AsyncError::with_code(ErrorCode::Cancelled, "cancelled"));
            }
        }));

        output
    }
}
