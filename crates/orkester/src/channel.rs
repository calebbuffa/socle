//! Bounded multi-producer, single-consumer async channel.
//!
//! Create channels via the free functions [`orkester::mpsc`] and [`orkester::oneshot`].

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// Error returned when sending to a closed channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendError<T>(pub T);

impl<T> std::fmt::Display for SendError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("channel closed")
    }
}

impl<T: std::fmt::Debug> std::error::Error for SendError<T> {}

/// Error returned by [`Sender::try_send`] and [`Sender::send_timeout`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrySendError<T> {
    /// The channel is at capacity; the value is returned.
    Full(T),
    /// The receiver has been dropped; the value is returned.
    Closed(T),
}

impl<T> TrySendError<T> {
    /// Consume the error and return the unsent value.
    pub fn into_inner(self) -> T {
        match self {
            TrySendError::Full(v) | TrySendError::Closed(v) => v,
        }
    }

    /// Returns `true` if the channel was full (not closed).
    pub fn is_full(&self) -> bool {
        matches!(self, TrySendError::Full(_))
    }

    /// Returns `true` if the receiver was dropped.
    pub fn is_closed(&self) -> bool {
        matches!(self, TrySendError::Closed(_))
    }
}

impl<T> std::fmt::Display for TrySendError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrySendError::Full(_) => f.write_str("channel full"),
            TrySendError::Closed(_) => f.write_str("channel closed"),
        }
    }
}

impl<T: std::fmt::Debug> std::error::Error for TrySendError<T> {}

/// The sending half of an mpsc channel. Cloneable.
pub struct Sender<T> {
    inner: Arc<ChannelInner<T>>,
}

/// The receiving half of an mpsc channel.
pub struct Receiver<T> {
    inner: Arc<ChannelInner<T>>,
}

struct ChannelInner<T> {
    queue: Mutex<VecDeque<T>>,
    capacity: usize,
    closed: AtomicBool,
    sender_count: AtomicUsize,
    /// Notified when an item is pushed or the channel is closed.
    not_empty: Condvar,
    /// Notified when an item is popped (space available).
    not_full: Condvar,
}

/// Create a bounded mpsc channel.
///
/// `capacity` must be at least 1.
pub fn mpsc<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let capacity = capacity.max(1);
    let inner = Arc::new(ChannelInner {
        queue: Mutex::new(VecDeque::with_capacity(capacity)),
        capacity,
        closed: AtomicBool::new(false),
        sender_count: AtomicUsize::new(1),
        not_empty: Condvar::new(),
        not_full: Condvar::new(),
    });
    (
        Sender {
            inner: Arc::clone(&inner),
        },
        Receiver { inner },
    )
}

/// Create a one-shot channel (capacity 1, single send).
pub fn oneshot<T>() -> (Sender<T>, Receiver<T>) {
    mpsc(1)
}

impl<T> Sender<T> {
    /// Send a value, blocking if the channel is full.
    ///
    /// Returns `Err(SendError(value))` if the receiver has been dropped.
    pub fn send(&self, value: T) -> Result<(), SendError<T>> {
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(SendError(value));
        }

        let mut queue = self.inner.queue.lock().expect("channel lock");
        loop {
            if self.inner.closed.load(Ordering::Acquire) {
                return Err(SendError(value));
            }
            if queue.len() < self.inner.capacity {
                queue.push_back(value);
                self.inner.not_empty.notify_one();
                return Ok(());
            }
            queue = self.inner.not_full.wait(queue).expect("channel lock");
        }
    }

    /// Non-blocking send. Returns `Err(Full)` if at capacity, `Err(Closed)` if
    /// the receiver has been dropped.
    pub fn try_send(&self, value: T) -> Result<(), TrySendError<T>> {
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(TrySendError::Closed(value));
        }
        let mut queue = self.inner.queue.lock().expect("channel lock");
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(TrySendError::Closed(value));
        }
        if queue.len() < self.inner.capacity {
            queue.push_back(value);
            self.inner.not_empty.notify_one();
            Ok(())
        } else {
            Err(TrySendError::Full(value))
        }
    }

    /// Send a value, blocking for at most `timeout` if the channel is full.
    ///
    /// Returns `Err(Full)` if still full after the timeout, `Err(Closed)` if
    /// the receiver was dropped.
    pub fn send_timeout(&self, value: T, timeout: Duration) -> Result<(), TrySendError<T>> {
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(TrySendError::Closed(value));
        }

        let mut queue = self.inner.queue.lock().expect("channel lock");
        let deadline = Instant::now() + timeout;
        loop {
            if self.inner.closed.load(Ordering::Acquire) {
                return Err(TrySendError::Closed(value));
            }
            if queue.len() < self.inner.capacity {
                queue.push_back(value);
                self.inner.not_empty.notify_one();
                return Ok(());
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(TrySendError::Full(value));
            }
            let (guard, _) = self
                .inner
                .not_full
                .wait_timeout(queue, deadline - now)
                .expect("channel lock");
            queue = guard;
        }
    }

    /// Returns true if the receiver has been dropped.
    pub fn is_closed(&self) -> bool {
        self.inner.closed.load(Ordering::Acquire)
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        self.inner.sender_count.fetch_add(1, Ordering::Relaxed);
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        if self.inner.sender_count.fetch_sub(1, Ordering::AcqRel) == 1 {
            // Last sender — notify receiver that no more items will arrive.
            self.inner.not_empty.notify_all();
        }
    }
}

impl<T> Receiver<T> {
    /// Receive a value, blocking until one is available.
    ///
    /// Returns `None` when all senders have been dropped and the buffer
    /// is empty.
    pub fn recv(&self) -> Option<T> {
        let mut queue = self.inner.queue.lock().expect("channel lock");
        loop {
            if let Some(value) = queue.pop_front() {
                self.inner.not_full.notify_one();
                return Some(value);
            }
            if self.inner.sender_count.load(Ordering::Acquire) == 0 {
                return None;
            }
            queue = self.inner.not_empty.wait(queue).expect("channel lock");
        }
    }

    /// Non-blocking receive. Returns `None` if empty (not necessarily closed).
    pub fn try_recv(&self) -> Option<T> {
        let mut queue = self.inner.queue.lock().expect("channel lock");
        let value = queue.pop_front();
        if value.is_some() {
            self.inner.not_full.notify_one();
        }
        value
    }

    /// Receive a value, blocking for at most `timeout`.
    ///
    /// Returns `None` if no value arrives within the timeout or all senders
    /// have been dropped.
    pub fn recv_timeout(&self, timeout: Duration) -> Option<T> {
        let mut queue = self.inner.queue.lock().expect("channel lock");
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(value) = queue.pop_front() {
                self.inner.not_full.notify_one();
                return Some(value);
            }
            if self.inner.sender_count.load(Ordering::Acquire) == 0 {
                return None;
            }
            let now = Instant::now();
            if now >= deadline {
                return None;
            }
            let (guard, _) = self
                .inner
                .not_empty
                .wait_timeout(queue, deadline - now)
                .expect("channel lock");
            queue = guard;
        }
    }

    /// Returns `true` if the channel is closed and empty.
    pub fn is_closed(&self) -> bool {
        self.inner.sender_count.load(Ordering::Acquire) == 0
            && self.inner.queue.lock().expect("channel lock").is_empty()
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        self.inner.closed.store(true, Ordering::Release);
        self.inner.not_full.notify_all();
    }
}
