//! Minimal block_on implementation for driving futures to completion.
//!
//! Used by the default `Executor::spawn` implementation and async
//! convenience methods (`run_async`, `then_async`) when no async runtime
//! (tokio, smol) is available.

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake};

struct ThreadWaker(std::thread::Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

/// Block the current thread until `future` completes.
///
/// Uses thread parking for efficient waiting. Each call to `wake()` unparks
/// the blocked thread so it can re-poll.
pub(crate) fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Arc::new(ThreadWaker(std::thread::current())).into();
    let mut cx = Context::from_waker(&waker);
    let mut future = pin!(future);

    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::park(),
        }
    }
}
