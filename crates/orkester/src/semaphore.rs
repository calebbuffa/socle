//! Counting semaphore for concurrency limiting.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::task::{Resolver, Task};

/// A counting semaphore that limits concurrent access to a resource.
///
/// Call [`acquire`](Semaphore::acquire) to obtain a [`SemaphorePermit`].
/// When the permit is dropped, the slot is released back to the semaphore,
/// potentially waking a queued acquirer.
///
/// # Example
///
/// ```rust,ignore
/// let sem = Semaphore::new(3);
/// let permit = sem.acquire(); // blocks if 3 permits already held
/// // ... do work ...
/// drop(permit); // releases back to the semaphore
/// ```
#[derive(Clone)]
pub struct Semaphore {
    inner: Arc<SemaphoreInner>,
}

struct SemaphoreInner {
    state: Mutex<SemaphoreState>,
}

struct SemaphoreState {
    permits: usize,
    max_permits: usize,
    waiters: VecDeque<Resolver<()>>,
}

impl Semaphore {
    /// Create a semaphore with `permits` available slots.
    ///
    /// # Panics
    ///
    /// Panics if `permits` is 0.
    pub fn new(permits: usize) -> Self {
        assert!(permits > 0, "semaphore requires at least 1 permit");
        Self {
            inner: Arc::new(SemaphoreInner {
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

        // Slow path: queue a resolver and wait.
        let (resolver, task) = crate::task::create_pair::<()>();
        {
            let mut state = self.inner.state.lock().expect("semaphore lock");
            // Re-check after acquiring lock (permit may have been released).
            if state.permits > 0 {
                state.permits -= 1;
                resolver.resolve(());
                return SemaphorePermit {
                    inner: Arc::clone(&self.inner),
                };
            }
            state.waiters.push_back(resolver);
        }

        // Block until our resolver is resolved.
        let _ = task.block();
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
}

/// RAII guard returned by [`Semaphore::acquire`].
///
/// Dropping this releases one permit back to the semaphore.
pub struct SemaphorePermit {
    inner: Arc<SemaphoreInner>,
}

impl Drop for SemaphorePermit {
    fn drop(&mut self) {
        let mut state = self.inner.state.lock().expect("semaphore lock on drop");
        if let Some(waiter) = state.waiters.pop_front() {
            // Give the permit directly to the next waiter.
            waiter.resolve(());
        } else {
            state.permits += 1;
        }
    }
}

impl<T: Send + 'static> Task<T> {
    /// Acquire a semaphore permit before executing, releasing it when done.
    pub fn with_semaphore(self, sem: &Semaphore) -> Task<T> {
        let permit = sem.acquire();
        self.map(move |v| {
            drop(permit);
            v
        })
    }
}
