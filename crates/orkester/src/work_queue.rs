//! User-owned work queue for main-thread or UI-thread task dispatch.
//!
//! A [`WorkQueue`] replaces the old `Context::MAIN` + `flush_main*` approach.
//! The caller owns the queue, obtains a [`Context`](crate::Context) from it,
//! and pumps it at their leisure.
//!
//! # Example
//!
//! ```rust,ignore
//! let pool = orkester::ThreadPool::new(4);
//! let bg_ctx = pool.context();
//!
//! let mut wq = orkester::WorkQueue::new();
//! let main_ctx = wq.context();
//!
//! let task = bg_ctx.run(|| compute())
//!     .then(main_ctx, |v| update_ui(v));
//!
//! // On the main/UI thread:
//! while !task.is_ready() {
//!     wq.pump();
//! }
//! let result = task.block().unwrap();
//! ```

use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::context::Context;
use crate::executor::Executor;

type Work = Box<dyn FnOnce() + Send + 'static>;

pub(crate) struct WorkQueueInner {
    queue: Mutex<VecDeque<Work>>,
    condvar: Condvar,
}

impl WorkQueueInner {
    fn enqueue(&self, work: Work) {
        self.queue
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push_back(work);
        self.condvar.notify_one();
    }

    fn dequeue(&self) -> Option<Work> {
        self.queue
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .pop_front()
    }

    pub(crate) fn has_pending(&self) -> bool {
        !self
            .queue
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_empty()
    }
}

/// User-owned dispatch queue for main/UI-thread task scheduling.
///
/// Create one per thread that needs to receive dispatched work, obtain a
/// [`Context`] via [`context()`](Self::context), and call
/// [`pump()`](Self::pump) regularly to execute pending items.
#[derive(Clone)]
pub struct WorkQueue {
    inner: Arc<WorkQueueInner>,
}

impl Executor for WorkQueue {
    fn execute(&self, task: Work) {
        self.inner.enqueue(task);
    }
}

impl WorkQueue {
    /// Create a new, empty work queue.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(WorkQueueInner {
                queue: Mutex::new(VecDeque::new()),
                condvar: Condvar::new(),
            }),
        }
    }

    /// Return a [`Context`] that routes tasks into this queue.
    ///
    /// Multiple clones of the returned `Context` all route to the same queue.
    pub fn context(&self) -> Context {
        Context::new(self.clone())
    }

    /// Execute all currently pending items and return the count executed.
    ///
    /// Does not block if the queue is empty.
    pub fn flush(&self) -> usize {
        let mut count = 0;
        while let Some(work) = self.inner.dequeue() {
            work();
            count += 1;
        }
        count
    }

    /// Pump a single item if available, returning `true` if an item was executed.
    pub fn pump(&self) -> bool {
        self.inner
            .dequeue()
            .map(|work| {
                work();
                true
            })
            .unwrap_or(false)
    }

    /// Execute pending items until the queue is empty **or** `budget` has
    /// elapsed. Returns the number of items executed.
    pub fn flush_timed(&self, budget: Duration) -> usize {
        let deadline = Instant::now() + budget;
        let mut count = 0;
        while let Some(work) = self.inner.dequeue() {
            work();
            count += 1;
            if Instant::now() >= deadline {
                break;
            }
        }
        count
    }

    /// Returns `true` if there are items waiting to be executed.
    pub fn has_pending(&self) -> bool {
        self.inner.has_pending()
    }

    /// Block the calling thread until at least one item is available, then
    /// execute all pending items.  Useful when the caller's thread has nothing
    /// else to do while waiting for results dispatched to this queue.
    pub fn wait_and_flush(&self) -> usize {
        {
            let mut guard = self.inner.queue.lock().unwrap_or_else(|p| p.into_inner());
            while guard.is_empty() {
                guard = self
                    .inner
                    .condvar
                    .wait(guard)
                    .unwrap_or_else(|p| p.into_inner());
            }
        }
        self.flush()
    }
}

impl Default for WorkQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for WorkQueue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorkQueue")
            .field("has_pending", &self.has_pending())
            .finish()
    }
}
