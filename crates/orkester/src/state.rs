use crate::error::AsyncError;
use std::mem;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};
use std::task::Waker;
use std::time::Duration;

type Continuation = Box<dyn FnOnce() + Send + 'static>;

pub(crate) enum FutureInner<T> {
    Pending {
        continuations: Vec<Continuation>,
        wakers: Vec<Waker>,
    },
    Complete(Option<Result<T, AsyncError>>),
}

pub(crate) struct SharedState<T> {
    is_complete: AtomicBool,
    inner: Mutex<FutureInner<T>>,
    condvar: Condvar,
}

impl<T> SharedState<T> {
    pub(crate) fn new_pending() -> Self {
        Self {
            is_complete: AtomicBool::new(false),
            inner: Mutex::new(FutureInner::Pending {
                continuations: Vec::new(),
                wakers: Vec::new(),
            }),
            condvar: Condvar::new(),
        }
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.is_complete.load(Ordering::Acquire)
    }

    pub(crate) fn resolve(&self, value: T) {
        self.complete(Ok(value));
    }

    pub(crate) fn reject(&self, error: AsyncError) {
        self.complete(Err(error));
    }

    fn complete(&self, result: Result<T, AsyncError>) {
        let (continuations, wakers) = {
            let mut inner = self.inner.lock().expect("future state lock poisoned");
            match &mut *inner {
                FutureInner::Pending {
                    continuations,
                    wakers,
                } => {
                    let continuations = mem::take(continuations);
                    let wakers = mem::take(wakers);
                    *inner = FutureInner::Complete(Some(result));
                    self.is_complete.store(true, Ordering::Release);
                    (continuations, wakers)
                }
                FutureInner::Complete(_) => return,
            }
        };

        self.condvar.notify_all();

        for continuation in continuations {
            continuation();
        }
        for waker in wakers {
            waker.wake();
        }
    }

    pub(crate) fn register_continuation(&self, continuation: Continuation) {
        if self.is_ready() {
            continuation();
            return;
        }

        let mut maybe_continuation = Some(continuation);
        let run_now = {
            let mut inner = self.inner.lock().expect("future state lock poisoned");
            match &mut *inner {
                FutureInner::Pending { continuations, .. } => {
                    continuations.push(maybe_continuation.take().expect("continuation missing"));
                    false
                }
                FutureInner::Complete(_) => true,
            }
        };

        if run_now {
            maybe_continuation.take().expect("continuation missing")();
        }
    }

    pub(crate) fn register_waker(&self, waker: &Waker) {
        if self.is_ready() {
            waker.wake_by_ref();
            return;
        }

        let should_wake = {
            let mut inner = self.inner.lock().expect("future state lock poisoned");
            match &mut *inner {
                FutureInner::Pending { wakers, .. } => {
                    if !wakers.iter().any(|candidate| candidate.will_wake(waker)) {
                        wakers.push(waker.clone());
                    }
                    false
                }
                FutureInner::Complete(_) => true,
            }
        };

        if should_wake {
            waker.wake_by_ref();
        }
    }

    pub(crate) fn wait_until_ready(&self) {
        if self.is_ready() {
            return;
        }

        let mut inner = self.inner.lock().expect("future state lock poisoned");
        while matches!(&*inner, FutureInner::Pending { .. }) {
            inner = self
                .condvar
                .wait(inner)
                .expect("future state lock poisoned after wait");
        }
    }

    pub(crate) fn wait_timeout(&self, timeout: Duration) {
        if self.is_ready() {
            return;
        }

        let inner = self.inner.lock().expect("future state lock poisoned");
        if matches!(&*inner, FutureInner::Pending { .. }) {
            let _ = self
                .condvar
                .wait_timeout(inner, timeout)
                .expect("future state lock poisoned after timeout wait");
        }
    }

    pub(crate) fn take_result(&self) -> Option<Result<T, AsyncError>> {
        let mut inner = self.inner.lock().expect("future state lock poisoned");
        match &mut *inner {
            FutureInner::Pending { .. } => None,
            FutureInner::Complete(result) => result.take(),
        }
    }

    pub(crate) fn clone_result(&self) -> Option<Result<T, AsyncError>>
    where
        T: Clone,
    {
        let inner = self.inner.lock().expect("future state lock poisoned");
        match &*inner {
            FutureInner::Pending { .. } => None,
            FutureInner::Complete(Some(Ok(value))) => Some(Ok(value.clone())),
            FutureInner::Complete(Some(Err(error))) => Some(Err(error.clone())),
            FutureInner::Complete(None) => None,
        }
    }
}
