//! Counting semaphore for concurrency limiting.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::future::Future;
use crate::promise::Promise;
use crate::system::AsyncSystem;

/// A counting semaphore that limits concurrent access to a resource.
///
/// Call [`acquire`](Semaphore::acquire) to obtain a [`SemaphorePermit`].
/// When the permit is dropped, the slot is released back to the semaphore,
/// potentially waking a queued acquirer.
///
/// # Example
///
/// ```rust,ignore
/// let sem = Semaphore::new(&system, 3);
/// let permit = sem.acquire(); // blocks if 3 permits already held
/// // ... do work ...
/// drop(permit); // releases back to the semaphore
/// ```
pub struct Semaphore {
    inner: Arc<SemaphoreInner>,
}

struct SemaphoreInner {
    system: AsyncSystem,
    state: Mutex<SemaphoreState>,
}

struct SemaphoreState {
    permits: usize,
    max_permits: usize,
    waiters: VecDeque<Promise<()>>,
}

impl Semaphore {
    /// Create a semaphore with `permits` available slots.
    ///
    /// # Panics
    ///
    /// Panics if `permits` is 0.
    pub fn new(system: &AsyncSystem, permits: usize) -> Self {
        assert!(permits > 0, "semaphore requires at least 1 permit");
        Self {
            inner: Arc::new(SemaphoreInner {
                system: system.clone(),
                state: Mutex::new(SemaphoreState {
                    permits,
                    max_permits: permits,
                    waiters: VecDeque::new(),
                }),
            }),
        }
    }

    /// Acquire a permit, blocking the current thread if none are available.
    ///
    /// Returns a [`SemaphorePermit`] that releases the slot when dropped.
    pub fn acquire(&self) -> SemaphorePermit {
        // Fast path: try to grab a permit without queueing.
        {
            let mut state = self.inner.state.lock().expect("semaphore lock");
            if state.permits > 0 {
                state.permits -= 1;
                return SemaphorePermit {
                    inner: Arc::clone(&self.inner),
                };
            }
        }

        // Slow path: queue a promise and wait.
        let (promise, future) = self.inner.system.promise::<()>();
        {
            let mut state = self.inner.state.lock().expect("semaphore lock");
            // Re-check after acquiring lock (permit may have been released).
            if state.permits > 0 {
                state.permits -= 1;
                // Don't leave the promise dangling.
                promise.resolve(());
                return SemaphorePermit {
                    inner: Arc::clone(&self.inner),
                };
            }
            state.waiters.push_back(promise);
        }

        // Block until our promise is resolved.
        let _ = future.block();
        SemaphorePermit {
            inner: Arc::clone(&self.inner),
        }
    }

    /// Try to acquire a permit without blocking.
    ///
    /// Returns `Some(permit)` if a slot was available, `None` otherwise.
    pub fn try_acquire(&self) -> Option<SemaphorePermit> {
        let mut state = self.inner.state.lock().expect("semaphore lock");
        if state.permits > 0 {
            state.permits -= 1;
            Some(SemaphorePermit {
                inner: Arc::clone(&self.inner),
            })
        } else {
            None
        }
    }

    /// Returns the number of permits currently available.
    pub fn available_permits(&self) -> usize {
        self.inner.state.lock().expect("semaphore lock").permits
    }

    /// Returns the maximum number of permits.
    pub fn max_permits(&self) -> usize {
        self.inner.state.lock().expect("semaphore lock").max_permits
    }

    /// Acquire a permit asynchronously, returning a future that resolves
    /// to a [`SemaphorePermit`] when a slot becomes available.
    pub fn acquire_async(&self) -> Future<SemaphorePermit> {
        let inner = Arc::clone(&self.inner);
        let (outer_promise, outer_future) = self.inner.system.promise::<SemaphorePermit>();

        {
            let mut state = inner.state.lock().expect("semaphore lock");
            if state.permits > 0 {
                state.permits -= 1;
                outer_promise.resolve(SemaphorePermit {
                    inner: Arc::clone(&inner),
                });
                return outer_future;
            }
        }

        // Queue: create an inner promise that gets resolved when a permit
        // is released. When it fires, resolve the outer promise with a permit.
        let (inner_promise, inner_future) = self.inner.system.promise::<()>();
        {
            let mut state = inner.state.lock().expect("semaphore lock");
            if state.permits > 0 {
                state.permits -= 1;
                inner_promise.resolve(());
                outer_promise.resolve(SemaphorePermit {
                    inner: Arc::clone(&inner),
                });
                return outer_future;
            }
            state.waiters.push_back(inner_promise);
        }

        let inner2 = Arc::clone(&inner);
        let next = inner_future.then(crate::context::Context::IMMEDIATE, move |()| {
            SemaphorePermit { inner: inner2 }
        });

        // We need to wire `next` into `outer_promise`. Simplest: just return
        // `next` directly and drop the outer promise (which auto-rejects,
        // but nobody is listening since we return `next`).
        drop(outer_promise);
        drop(outer_future);
        next
    }
}

impl Clone for Semaphore {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

/// RAII guard that releases a semaphore permit when dropped.
pub struct SemaphorePermit {
    inner: Arc<SemaphoreInner>,
}

impl Drop for SemaphorePermit {
    fn drop(&mut self) {
        let mut state = self.inner.state.lock().expect("semaphore lock");
        if let Some(waiter) = state.waiters.pop_front() {
            // Hand the permit directly to a waiter — don't increment count.
            drop(state);
            waiter.resolve(());
        } else {
            state.permits += 1;
        }
    }
}
