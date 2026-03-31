//! Efficient timer system backed by a dedicated thread.
//!
//! Provides [`TimerWheel`] — a shared, thread-safe timer registry. Callers
//! submit `(Instant, Waker)` pairs; the timer thread sleeps until the
//! earliest deadline, then wakes the corresponding waker. No worker
//! threads are parked for individual timers.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::{Arc, Condvar, Mutex};
use std::task::Waker;
#[cfg(test)]
use std::time::Duration;
use std::time::Instant;

/// Entry in the timer heap — ordered by deadline (earliest first).
struct TimerEntry {
    deadline: Instant,
    waker: Waker,
}

impl PartialEq for TimerEntry {
    fn eq(&self, other: &Self) -> bool {
        self.deadline == other.deadline
    }
}

impl Eq for TimerEntry {}

impl PartialOrd for TimerEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimerEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.deadline.cmp(&other.deadline)
    }
}

struct TimerState {
    heap: BinaryHeap<Reverse<TimerEntry>>,
    shutdown: bool,
}

/// Shared timer wheel. Cheap to clone (wraps `Arc`).
///
/// A single background thread services all registered timers. Individual
/// `delay()` calls register a waker instead of parking a whole thread.
#[derive(Clone)]
pub(crate) struct TimerWheel {
    inner: Arc<TimerInner>,
}

struct TimerInner {
    state: Mutex<TimerState>,
    condvar: Condvar,
}

impl TimerWheel {
    /// Create a new timer wheel and spawn the service thread.
    pub(crate) fn new() -> Self {
        let inner = Arc::new(TimerInner {
            state: Mutex::new(TimerState {
                heap: BinaryHeap::new(),
                shutdown: false,
            }),
            condvar: Condvar::new(),
        });

        let thread_inner = Arc::clone(&inner);
        std::thread::Builder::new()
            .name("orkester-timer".into())
            .spawn(move || Self::run(thread_inner))
            .expect("failed to spawn timer thread");

        Self { inner }
    }

    /// Register a waker to fire at or after `deadline`.
    pub(crate) fn register(&self, deadline: Instant, waker: Waker) {
        let mut state = self.inner.state.lock().expect("timer lock");
        state.heap.push(Reverse(TimerEntry { deadline, waker }));
        self.inner.condvar.notify_one();
    }

    /// Register a waker to fire after `duration` from now.
    #[cfg(test)]
    pub(crate) fn register_delay(&self, duration: Duration, waker: Waker) {
        self.register(Instant::now() + duration, waker);
    }

    /// Return the global shared timer wheel, creating it on first call.
    pub(crate) fn global() -> &'static TimerWheel {
        static GLOBAL: std::sync::OnceLock<TimerWheel> = std::sync::OnceLock::new();
        GLOBAL.get_or_init(TimerWheel::new)
    }

    /// Shut down the timer thread. Called on drop of the `Runtime`.
    pub(crate) fn shutdown(&self) {
        let mut state = self.inner.state.lock().expect("timer lock");
        state.shutdown = true;
        self.inner.condvar.notify_one();
    }

    /// Timer thread main loop.
    fn run(inner: Arc<TimerInner>) {
        loop {
            let mut state = inner.state.lock().expect("timer lock");

            if state.shutdown {
                // Fire all remaining timers before exiting.
                while let Some(Reverse(entry)) = state.heap.pop() {
                    entry.waker.wake();
                }
                return;
            }

            match state.heap.peek() {
                None => {
                    // No timers — sleep until notified.
                    state = inner.condvar.wait(state).expect("timer condvar");
                }
                Some(Reverse(entry)) => {
                    let now = Instant::now();
                    if entry.deadline <= now {
                        // Deadline passed — fire it.
                        let Reverse(entry) = state.heap.pop().unwrap();
                        drop(state); // release lock before waking
                        entry.waker.wake();
                    } else {
                        // Sleep until next deadline or new timer arrives.
                        let timeout = entry.deadline - now;
                        let _ = inner
                            .condvar
                            .wait_timeout(state, timeout)
                            .expect("timer condvar timeout");
                    }
                }
            }
        }
    }
}

impl Drop for TimerWheel {
    fn drop(&mut self) {
        // Only shut down if this is the last Arc reference.
        if Arc::strong_count(&self.inner) == 1 {
            self.shutdown();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::task::{RawWaker, RawWakerVTable};

    fn test_waker(flag: &'static AtomicBool) -> Waker {
        fn clone_fn(data: *const ()) -> RawWaker {
            RawWaker::new(data, &VTABLE)
        }
        fn wake_fn(data: *const ()) {
            let flag = unsafe { &*(data as *const AtomicBool) };
            flag.store(true, Ordering::SeqCst);
        }
        fn wake_by_ref_fn(data: *const ()) {
            let flag = unsafe { &*(data as *const AtomicBool) };
            flag.store(true, Ordering::SeqCst);
        }
        fn drop_fn(_: *const ()) {}

        static VTABLE: RawWakerVTable =
            RawWakerVTable::new(clone_fn, wake_fn, wake_by_ref_fn, drop_fn);

        let raw = RawWaker::new(flag as *const AtomicBool as *const (), &VTABLE);
        unsafe { Waker::from_raw(raw) }
    }

    #[test]
    fn timer_fires_after_delay() {
        static FIRED: AtomicBool = AtomicBool::new(false);
        let wheel = TimerWheel::new();
        let waker = test_waker(&FIRED);

        wheel.register_delay(Duration::from_millis(50), waker);
        assert!(!FIRED.load(Ordering::SeqCst));

        std::thread::sleep(Duration::from_millis(100));
        assert!(FIRED.load(Ordering::SeqCst));

        wheel.shutdown();
    }

    #[test]
    fn multiple_timers_fire_in_order() {
        use std::sync::atomic::AtomicUsize;

        static ORDER: AtomicUsize = AtomicUsize::new(0);

        struct OrderWaker {
            expected: usize,
        }
        impl Wake for OrderWaker {
            fn wake(self: Arc<Self>) {
                let prev = ORDER.fetch_add(1, Ordering::SeqCst);
                assert_eq!(prev, self.expected);
            }
        }

        use std::task::Wake;

        let wheel = TimerWheel::new();
        let now = Instant::now();

        // Register in reverse order — timer should still fire shortest first.
        let w2 = Waker::from(Arc::new(OrderWaker { expected: 1 }));
        let w1 = Waker::from(Arc::new(OrderWaker { expected: 0 }));

        wheel.register(now + Duration::from_millis(100), w2);
        wheel.register(now + Duration::from_millis(50), w1);

        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(ORDER.load(Ordering::SeqCst), 2);

        wheel.shutdown();
    }

    #[test]
    fn shutdown_fires_remaining_timers() {
        static FIRED: AtomicBool = AtomicBool::new(false);
        let wheel = TimerWheel::new();
        let waker = test_waker(&FIRED);

        // Register a timer far in the future.
        wheel.register_delay(Duration::from_secs(60), waker);
        assert!(!FIRED.load(Ordering::SeqCst));

        // Shutdown should fire it immediately.
        wheel.shutdown();
        std::thread::sleep(Duration::from_millis(50));
        assert!(FIRED.load(Ordering::SeqCst));
    }
}
