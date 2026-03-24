use std::cell::Cell;
use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};

type Work = Box<dyn FnOnce() + Send + 'static>;

/// FIFO queue for main-thread work.
///
/// The queue uses a `Condvar` so that `block_with_main()` can sleep
/// efficiently until either new work arrives or an external event
/// (e.g. task completion) calls [`notify()`](Self::notify).
pub(crate) struct MainThreadQueue {
    queue: Mutex<VecDeque<Work>>,
    condvar: Condvar,
}

impl MainThreadQueue {
    pub(crate) fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            condvar: Condvar::new(),
        }
    }

    pub(crate) fn enqueue(&self, task: Work) {
        let mut queue = self.queue.lock().expect("main queue lock poisoned");
        queue.push_back(task);
        self.condvar.notify_all();
    }

    pub(crate) fn dispatch_all(&self) -> usize {
        let mut count = 0;
        while self.dispatch_one() {
            count += 1;
        }
        count
    }

    pub(crate) fn dispatch_one(&self) -> bool {
        let maybe_task = {
            let mut queue = self.queue.lock().expect("main queue lock poisoned");
            queue.pop_front()
        };

        if let Some(task) = maybe_task {
            task();
            true
        } else {
            false
        }
    }

    pub(crate) fn has_pending(&self) -> bool {
        let queue = self.queue.lock().expect("main queue lock poisoned");
        !queue.is_empty()
    }

    /// Wake any thread blocked in [`wait_for_work`](Self::wait_for_work).
    ///
    /// Called when an external event (e.g. a task completing) should
    /// unblock `block_with_main()` even though no new main-thread work
    /// was enqueued.
    pub(crate) fn notify(&self) {
        self.condvar.notify_all();
    }

    /// Block until either work is available or [`notify()`](Self::notify)
    /// is called externally. Falls back to a 100 ms timeout as a safety
    /// net against missed wakeups.
    pub(crate) fn wait_for_work(&self) {
        let guard = self.queue.lock().expect("main queue lock poisoned");
        if !guard.is_empty() {
            return;
        }
        let _ = self
            .condvar
            .wait_timeout(guard, std::time::Duration::from_millis(100));
    }
}

thread_local! {
    static MAIN_THREAD_DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// Returns true when the current thread is inside an active main-thread scope.
pub(crate) fn is_main_thread() -> bool {
    MAIN_THREAD_DEPTH.with(|depth| depth.get() > 0)
}

/// Scope marker that designates the current thread as the main thread for dispatch.
pub struct MainThreadScope {
    _private: (),
}

impl MainThreadScope {
    pub(crate) fn new() -> Self {
        MAIN_THREAD_DEPTH.with(|depth| depth.set(depth.get().saturating_add(1)));
        Self { _private: () }
    }
}

impl Drop for MainThreadScope {
    fn drop(&mut self) {
        MAIN_THREAD_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}
