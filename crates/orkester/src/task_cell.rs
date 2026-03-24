//! Lock-free single-producer completion cell.
//!
//! `TaskCell<T>` stores the result of an async computation and notifies
//! waiters when it becomes ready. The primary path (value write, atomic
//! state transition, single-consumer waker) is lock-free. A mutex-guarded
//! extension handles callbacks, extra wakers (for `SharedTask` polls),
//! and condvar-based blocking.

use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::task::Waker;

use crate::error::AsyncError;

// Cell states — transitions in one direction only: EMPTY → WAKER → COMPLETE.
const EMPTY: u8 = 0;
const WAKER_REGISTERED: u8 = 1;
const COMPLETE: u8 = 2;

type Callback = Box<dyn FnOnce() + Send + 'static>;

/// Mutex-guarded extension for callbacks, extra wakers, and blocking.
struct Extension {
    done: bool,
    callbacks: Vec<Callback>,
    extra_wakers: Vec<Waker>,
}

/// A lock-free, single-producer completion cell.
///
/// Stores a `Result<T, AsyncError>` that is written exactly once.
/// Supports:
/// - A single lock-free waker slot (for `Task<T>` poll)
/// - Mutex-guarded extra wakers (for `SharedTask<T>` poll)
/// - Mutex-guarded callbacks (for continuations)
/// - Condvar-based blocking waits
pub(crate) struct TaskCell<T> {
    state: AtomicU8,
    taken: AtomicBool,
    value: UnsafeCell<MaybeUninit<Result<T, AsyncError>>>,
    /// Single waker for `Task<T>` poll — lock-free, single consumer only.
    waker: UnsafeCell<Option<Waker>>,
    ext: Mutex<Extension>,
    condvar: Condvar,
}

// SAFETY: T: Send is required by the Task bounds. The UnsafeCell fields are
// only accessed under the atomic state protocol: the producer writes the
// value exactly once before transitioning to COMPLETE, and consumers only
// read after observing COMPLETE. The waker UnsafeCell is only used by a
// single consumer (Task's StdFuture poll).
unsafe impl<T: Send> Send for TaskCell<T> {}
unsafe impl<T: Send> Sync for TaskCell<T> {}

impl<T> TaskCell<T> {
    /// Create a new empty cell.
    pub(crate) fn new() -> Self {
        Self {
            state: AtomicU8::new(EMPTY),
            taken: AtomicBool::new(false),
            value: UnsafeCell::new(MaybeUninit::uninit()),
            waker: UnsafeCell::new(None),
            ext: Mutex::new(Extension {
                done: false,
                callbacks: Vec::new(),
                extra_wakers: Vec::new(),
            }),
            condvar: Condvar::new(),
        }
    }

    /// Returns `true` if the cell has been completed.
    #[inline]
    pub(crate) fn is_ready(&self) -> bool {
        self.state.load(Ordering::Acquire) == COMPLETE
    }

    /// Complete the cell with a result. Wakes the registered waker,
    /// fires callbacks, and notifies blocking threads.
    ///
    /// # Panics
    ///
    /// Panics if called more than once.
    pub(crate) fn complete(&self, result: Result<T, AsyncError>) {
        // Write the value before transitioning state.
        unsafe {
            (*self.value.get()).write(result);
        }

        let prev = self.state.swap(COMPLETE, Ordering::AcqRel);
        debug_assert_ne!(prev, COMPLETE, "TaskCell completed twice");

        // Wake the primary waker (lock-free path).
        if prev == WAKER_REGISTERED {
            let waker = unsafe { (*self.waker.get()).take() };
            if let Some(w) = waker {
                w.wake();
            }
        }

        // Fire callbacks and extra wakers (under lock).
        let (callbacks, extra_wakers) = {
            let mut ext = self.ext.lock().expect("task cell ext lock");
            ext.done = true;
            (
                std::mem::take(&mut ext.callbacks),
                std::mem::take(&mut ext.extra_wakers),
            )
        };

        // Notify condvar waiters.
        self.condvar.notify_all();

        for cb in callbacks {
            cb();
        }
        for w in extra_wakers {
            w.wake();
        }
    }

    /// Register a waker for single-consumer poll notification.
    ///
    /// Only safe for single-consumer use (`Task<T>` poll). For
    /// multi-consumer scenarios (`SharedTask<T>`), use
    /// [`register_extra_waker`].
    pub(crate) fn register_waker(&self, waker: &Waker) {
        let current = self.state.load(Ordering::Acquire);

        if current == COMPLETE {
            waker.wake_by_ref();
            return;
        }

        // Store the waker. Single-consumer invariant: only one poller
        // accesses this UnsafeCell at a time.
        unsafe {
            let slot = &mut *self.waker.get();
            match slot {
                Some(existing) if existing.will_wake(waker) => {}
                _ => *slot = Some(waker.clone()),
            }
        }

        // Transition EMPTY → WAKER_REGISTERED if still empty.
        let _ = self.state.compare_exchange(
            EMPTY,
            WAKER_REGISTERED,
            Ordering::AcqRel,
            Ordering::Acquire,
        );

        // Re-check: completion may have arrived between our load and CAS.
        if self.state.load(Ordering::Acquire) == COMPLETE {
            let waker = unsafe { (*self.waker.get()).take() };
            if let Some(w) = waker {
                w.wake();
            }
        }
    }

    /// Register an additional waker for multi-consumer poll notification.
    ///
    /// Used by `SharedTask<T>` where multiple clones may be polled
    /// concurrently. Thread-safe (mutex-guarded).
    pub(crate) fn register_extra_waker(&self, waker: &Waker) {
        if self.is_ready() {
            waker.wake_by_ref();
            return;
        }

        let should_wake = {
            let mut ext = self.ext.lock().expect("task cell ext lock");
            if ext.done {
                true
            } else {
                if !ext.extra_wakers.iter().any(|w| w.will_wake(waker)) {
                    ext.extra_wakers.push(waker.clone());
                }
                false
            }
        };

        if should_wake {
            waker.wake_by_ref();
        }
    }

    /// Block the current thread until the cell is complete.
    pub(crate) fn wait_until_ready(&self) {
        if self.is_ready() {
            return;
        }

        let mut ext = self.ext.lock().expect("task cell ext lock");
        while !ext.done {
            ext = self.condvar.wait(ext).expect("task cell condvar");
        }
    }

    /// Take the result out of the cell. Returns `None` if not yet complete
    /// or if the result was already taken. Safe to call at most once.
    pub(crate) fn take_result(&self) -> Option<Result<T, AsyncError>> {
        if !self.is_ready() {
            return None;
        }
        // CAS ensures only one caller succeeds.
        if self
            .taken
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return None;
        }
        // SAFETY: Value was written before COMPLETE. The CAS above
        // guarantees we are the only reader.
        Some(unsafe { (*self.value.get()).assume_init_read() })
    }

    /// Clone the result without taking it. Requires `T: Clone`.
    /// Returns `None` if not yet complete or if the result was taken.
    pub(crate) fn clone_result(&self) -> Option<Result<T, AsyncError>>
    where
        T: Clone,
    {
        if !self.is_ready() || self.taken.load(Ordering::Acquire) {
            return None;
        }
        // SAFETY: Value written before COMPLETE, not yet taken.
        let result = unsafe { &*(*self.value.get()).as_ptr() };
        Some(match result {
            Ok(v) => Ok(v.clone()),
            Err(e) => Err(e.clone()),
        })
    }

    /// Register a callback that receives the taken result when this cell
    /// completes. If already complete, fires immediately.
    ///
    /// For single-consumer use: calls `take_result`, so the callback
    /// receives ownership. Only one `on_complete` callback should take
    /// the result.
    pub(crate) fn on_complete(
        cell: Arc<Self>,
        f: impl FnOnce(Result<T, AsyncError>) + Send + 'static,
    ) where
        T: Send + 'static,
    {
        // Fast path — already complete.
        if cell.is_ready() {
            if let Some(result) = cell.take_result() {
                f(result);
            }
            return;
        }

        // Slow path — register under lock.
        let cell_for_cb = Arc::clone(&cell);
        let mut slot: Option<(Arc<TaskCell<T>>, _)> = Some((cell_for_cb, f));
        let run_now = {
            let mut ext = cell.ext.lock().expect("task cell ext lock");
            if ext.done {
                true
            } else {
                let (c, cb) = slot.take().expect("slot consumed twice");
                ext.callbacks.push(Box::new(move || {
                    if let Some(result) = c.take_result() {
                        cb(result);
                    }
                }));
                false
            }
        };

        if run_now {
            let (c, cb) = slot.take().expect("slot consumed twice");
            if let Some(result) = c.take_result() {
                cb(result);
            }
        }
    }

    /// Register a callback that receives a cloned result when this cell
    /// completes. If already complete, fires immediately.
    ///
    /// For multi-consumer use (`SharedTask`): clones the result, leaving
    /// the original in the cell for other consumers.
    pub(crate) fn on_complete_cloned(
        cell: Arc<Self>,
        f: impl FnOnce(Result<T, AsyncError>) + Send + 'static,
    ) where
        T: Clone + Send + 'static,
    {
        if cell.is_ready() {
            if let Some(result) = cell.clone_result() {
                f(result);
            }
            return;
        }

        let cell_for_cb = Arc::clone(&cell);
        let mut slot: Option<(Arc<TaskCell<T>>, _)> = Some((cell_for_cb, f));
        let run_now = {
            let mut ext = cell.ext.lock().expect("task cell ext lock");
            if ext.done {
                true
            } else {
                let (c, cb) = slot.take().expect("slot consumed twice");
                ext.callbacks.push(Box::new(move || {
                    if let Some(result) = c.clone_result() {
                        cb(result);
                    }
                }));
                false
            }
        };

        if run_now {
            let (c, cb) = slot.take().expect("slot consumed twice");
            if let Some(result) = c.clone_result() {
                cb(result);
            }
        }
    }
}

impl<T> Drop for TaskCell<T> {
    fn drop(&mut self) {
        // Only drop the value if it was completed and not taken.
        if *self.state.get_mut() == COMPLETE && !*self.taken.get_mut() {
            unsafe {
                (*self.value.get()).assume_init_drop();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn complete_and_take() {
        let cell = TaskCell::new();
        assert!(!cell.is_ready());

        cell.complete(Ok(42));
        assert!(cell.is_ready());

        let result = cell.take_result();
        assert_eq!(result.unwrap().unwrap(), 42);
    }

    #[test]
    fn take_only_succeeds_once() {
        let cell = TaskCell::new();
        cell.complete(Ok(String::from("once")));

        assert!(cell.take_result().is_some());
        assert!(cell.take_result().is_none());
    }

    #[test]
    fn complete_with_error() {
        let cell = TaskCell::<i32>::new();
        cell.complete(Err(AsyncError::msg("boom")));
        assert!(cell.is_ready());

        let result = cell.take_result().unwrap();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "boom");
    }

    #[test]
    #[should_panic(expected = "TaskCell completed twice")]
    fn double_complete_panics() {
        let cell = TaskCell::new();
        cell.complete(Ok(1));
        cell.complete(Ok(2));
    }

    #[test]
    fn blocking_wait() {
        let cell = Arc::new(TaskCell::new());
        let cell2 = Arc::clone(&cell);

        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(10));
            cell2.complete(Ok(99));
        });

        cell.wait_until_ready();
        assert_eq!(cell.take_result().unwrap().unwrap(), 99);
    }

    #[test]
    fn waker_notification() {
        use std::sync::atomic::AtomicBool;
        use std::task::{RawWaker, RawWakerVTable};

        static WOKEN: AtomicBool = AtomicBool::new(false);

        fn clone_fn(data: *const ()) -> RawWaker {
            RawWaker::new(data, &VTABLE)
        }
        fn wake_fn(_: *const ()) {
            WOKEN.store(true, Ordering::SeqCst);
        }
        fn wake_by_ref_fn(_: *const ()) {
            WOKEN.store(true, Ordering::SeqCst);
        }
        fn drop_fn(_: *const ()) {}

        static VTABLE: RawWakerVTable =
            RawWakerVTable::new(clone_fn, wake_fn, wake_by_ref_fn, drop_fn);

        let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };

        let cell = TaskCell::<i32>::new();
        cell.register_waker(&waker);
        assert!(!WOKEN.load(Ordering::SeqCst));

        cell.complete(Ok(42));
        assert!(WOKEN.load(Ordering::SeqCst));
    }

    #[test]
    fn clone_result() {
        let cell = TaskCell::new();
        cell.complete(Ok(String::from("hello")));

        let r1 = cell.clone_result().unwrap().unwrap();
        let r2 = cell.clone_result().unwrap().unwrap();
        assert_eq!(r1, "hello");
        assert_eq!(r2, "hello");
    }

    #[test]
    fn clone_result_returns_none_after_take() {
        let cell = TaskCell::new();
        cell.complete(Ok(String::from("gone")));

        assert!(cell.take_result().is_some());
        assert!(cell.clone_result().is_none());
    }

    #[test]
    fn on_complete_fires_when_already_ready() {
        use std::sync::atomic::{AtomicI32, Ordering};

        let cell = Arc::new(TaskCell::new());
        cell.complete(Ok(42));

        let observed = Arc::new(AtomicI32::new(0));
        let obs = Arc::clone(&observed);
        TaskCell::on_complete(cell, move |result| {
            obs.store(result.unwrap(), Ordering::SeqCst);
        });

        assert_eq!(observed.load(Ordering::SeqCst), 42);
    }

    #[test]
    fn on_complete_fires_on_completion() {
        use std::sync::atomic::{AtomicI32, Ordering};

        let cell = Arc::new(TaskCell::new());
        let observed = Arc::new(AtomicI32::new(0));
        let obs = Arc::clone(&observed);

        TaskCell::on_complete(Arc::clone(&cell), move |result| {
            obs.store(result.unwrap(), Ordering::SeqCst);
        });

        assert_eq!(observed.load(Ordering::SeqCst), 0);
        cell.complete(Ok(99));
        assert_eq!(observed.load(Ordering::SeqCst), 99);
    }

    #[test]
    fn on_complete_cloned_leaves_value() {
        let cell = Arc::new(TaskCell::new());
        cell.complete(Ok(String::from("shared")));

        use std::sync::Mutex;
        let captured = Arc::new(Mutex::new(String::new()));
        let cap = Arc::clone(&captured);
        TaskCell::on_complete_cloned(Arc::clone(&cell), move |result| {
            *cap.lock().unwrap() = result.unwrap();
        });
        assert_eq!(&*captured.lock().unwrap(), "shared");
        // Value wasn't taken, so clone_result still works.
        assert_eq!(cell.clone_result().unwrap().unwrap(), "shared");
    }

    #[test]
    fn extra_wakers_all_notified() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::task::Wake;

        struct Counter(AtomicUsize);
        impl Wake for Counter {
            fn wake(self: Arc<Self>) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        let cell = Arc::new(TaskCell::<i32>::new());

        let c1 = Arc::new(Counter(AtomicUsize::new(0)));
        let c2 = Arc::new(Counter(AtomicUsize::new(0)));
        let w1 = Waker::from(Arc::clone(&c1));
        let w2 = Waker::from(Arc::clone(&c2));

        cell.register_extra_waker(&w1);
        cell.register_extra_waker(&w2);

        cell.complete(Ok(1));

        assert_eq!(c1.0.load(Ordering::SeqCst), 1);
        assert_eq!(c2.0.load(Ordering::SeqCst), 1);
    }
}
