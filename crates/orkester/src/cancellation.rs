use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::error::{AsyncError, ErrorCode};
use crate::future::{Future, SharedFuture, create_pair};

type Callback = Box<dyn FnOnce() + Send + 'static>;

/// Cooperative cancellation token. Cheap to clone (shared `Arc`).
///
/// Use [`CancellationToken::new`] to create a fresh token, pass clones to
/// any number of futures via [`Future::with_cancellation`], then call
/// [`CancellationToken::cancel`] to signal them all.
///
/// Cancellation is cooperative — it does not abort running work.  Instead,
/// any future that has not yet completed will be rejected with an
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

    /// Register a callback to fire when the token is cancelled.
    /// If already cancelled, the callback fires immediately.
    fn on_cancel(&self, cb: Callback) {
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
                guard.push(maybe_cb.take().unwrap());
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

impl<T: Send + 'static> Future<T> {
    /// Attach a cancellation token. If the token is signalled before the
    /// upstream future completes, the returned future rejects with a
    /// `"cancelled"` error.
    pub fn with_cancellation(mut self, token: &CancellationToken) -> Future<T> {
        let source = match self.state.take() {
            Some(s) => s,
            None => {
                let (p, f) = create_pair(self.system.clone());
                p.reject(AsyncError::msg("Future already consumed"));
                return f;
            }
        };

        if token.is_cancelled() {
            let (p, f) = create_pair(self.system.clone());
            p.reject(AsyncError::with_code(ErrorCode::Cancelled, "cancelled"));
            return f;
        }

        let (promise, output) = create_pair::<T>(self.system.clone());
        let shared_promise = Arc::new(Mutex::new(Some(promise)));

        // Path 1: upstream completes first → forward result.
        let sp1 = shared_promise.clone();
        let source_ref = Arc::clone(&source);
        source.register_continuation(Box::new(move || {
            let promise = {
                let mut guard = sp1.lock().expect("cancel promise lock");
                guard.take()
            };
            if let Some(promise) = promise {
                match source_ref.take_result() {
                    Some(Ok(v)) => promise.resolve(v),
                    Some(Err(e)) => promise.reject(e),
                    None => promise.reject(AsyncError::msg("Future already consumed")),
                }
            }
        }));

        // Path 2: token cancelled first → reject.
        let sp2 = shared_promise;
        token.on_cancel(Box::new(move || {
            let promise = {
                let mut guard = sp2.lock().expect("cancel promise lock");
                guard.take()
            };
            if let Some(promise) = promise {
                promise.reject(AsyncError::with_code(ErrorCode::Cancelled, "cancelled"));
            }
        }));

        output
    }
}

impl<T: Clone + Send + 'static> SharedFuture<T> {
    /// Attach a cancellation token. If the token is signalled before the
    /// upstream future completes, the returned future rejects with a
    /// `"cancelled"` error. Does NOT consume the shared future.
    pub fn with_cancellation(&self, token: &CancellationToken) -> Future<T> {
        if token.is_cancelled() {
            let (p, f) = create_pair(self.system.clone());
            p.reject(AsyncError::with_code(ErrorCode::Cancelled, "cancelled"));
            return f;
        }

        let source = Arc::clone(&self.state);
        let (promise, output) = create_pair::<T>(self.system.clone());
        let shared_promise = Arc::new(Mutex::new(Some(promise)));

        // Path 1: upstream completes first → forward result.
        let sp1 = shared_promise.clone();
        let source_ref = Arc::clone(&source);
        source.register_continuation(Box::new(move || {
            let promise = {
                let mut guard = sp1.lock().expect("cancel promise lock");
                guard.take()
            };
            if let Some(promise) = promise {
                match source_ref.clone_result() {
                    Some(Ok(v)) => promise.resolve(v),
                    Some(Err(e)) => promise.reject(e),
                    None => promise.reject(AsyncError::msg("Future already consumed")),
                }
            }
        }));

        // Path 2: token cancelled first → reject.
        let sp2 = shared_promise;
        token.on_cancel(Box::new(move || {
            let promise = {
                let mut guard = sp2.lock().expect("cancel promise lock");
                guard.take()
            };
            if let Some(promise) = promise {
                promise.reject(AsyncError::with_code(ErrorCode::Cancelled, "cancelled"));
            }
        }));

        output
    }
}
