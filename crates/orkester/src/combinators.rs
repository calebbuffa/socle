//! Async combinators: `delay`, `timeout`, `race`, `retry`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::error::{AsyncError, ErrorCode};
use crate::future::Future;
use crate::system::AsyncSystem;

/// Completes after `duration` elapses.
///
/// The delay is scheduled via the system's worker scheduler. A worker
/// thread sleeps for the given duration and then resolves the promise.
/// This is appropriate for coarse timeouts; sub-millisecond precision is
/// not guaranteed.
pub fn delay(system: &AsyncSystem, duration: Duration) -> Future<()> {
    let (promise, future) = system.promise::<()>();
    system.spawn_detached(crate::context::Context::BACKGROUND, move || {
        std::thread::sleep(duration);
        promise.resolve(());
    });
    future
}

/// Wraps a future with a timeout. If the upstream future does not complete
/// within `duration`, the returned future rejects with [`ErrorCode::TimedOut`].
pub fn timeout<T: Send + 'static>(
    system: &AsyncSystem,
    future: Future<T>,
    duration: Duration,
) -> Future<T> {
    let timer = delay(system, duration);
    let (resolve_promise, output) = system.promise::<T>();
    let shared_promise = Arc::new(Mutex::new(Some(resolve_promise)));

    // Path 1: upstream completes in time
    let sp1 = shared_promise.clone();
    let mut upstream = future;
    let source = upstream.state.take().expect("future consumed");
    let source_ref = Arc::clone(&source);
    source.register_continuation(Box::new(move || {
        let promise = sp1.lock().expect("timeout lock").take();
        if let Some(promise) = promise {
            match source_ref.take_result() {
                Some(Ok(v)) => promise.resolve(v),
                Some(Err(e)) => promise.reject(e),
                None => promise.reject(AsyncError::msg("Future already consumed")),
            }
        }
    }));

    // Path 2: timer fires first → reject with TimedOut
    let sp2 = shared_promise;
    let mut timer_inner = timer;
    let timer_state = timer_inner.state.take().expect("timer consumed");
    timer_state.register_continuation(Box::new(move || {
        let promise = sp2.lock().expect("timeout lock").take();
        if let Some(promise) = promise {
            promise.reject(AsyncError::with_code(ErrorCode::TimedOut, "timed out"));
        }
    }));

    output
}

/// Completes when the **first** input future completes.
/// All other futures are dropped (their results are discarded).
///
/// If the input vector is empty, the returned future is immediately rejected.
pub fn race<T: Send + 'static>(system: &AsyncSystem, futures: Vec<Future<T>>) -> Future<T> {
    if futures.is_empty() {
        let (promise, future) = system.promise::<T>();
        promise.reject(AsyncError::msg("race called with no futures"));
        return future;
    }

    let (resolve_promise, output) = system.promise::<T>();
    let shared_promise = Arc::new(Mutex::new(Some(resolve_promise)));

    for mut f in futures {
        let sp = shared_promise.clone();
        let source = match f.state.take() {
            Some(s) => s,
            None => continue,
        };
        let source_ref = Arc::clone(&source);
        source.register_continuation(Box::new(move || {
            let promise = sp.lock().expect("race lock").take();
            if let Some(promise) = promise {
                match source_ref.take_result() {
                    Some(Ok(v)) => promise.resolve(v),
                    Some(Err(e)) => promise.reject(e),
                    None => {} // already consumed by another racer
                }
            }
        }));
    }

    output
}

/// Configuration for exponential backoff in [`retry`].
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Initial delay before the first retry (default: 50 ms).
    pub initial_backoff: Duration,
    /// Maximum delay between retries (default: 5 s).
    pub max_backoff: Duration,
    /// Multiplier applied to the backoff after each attempt (default: 2).
    pub multiplier: u32,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            initial_backoff: Duration::from_millis(50),
            max_backoff: Duration::from_secs(5),
            multiplier: 2,
        }
    }
}

/// Retry a fallible async operation with exponential backoff.
///
/// Calls `f()` up to `max_attempts` times. If an attempt returns `Ok(v)`,
/// the returned future resolves with `v`. If all attempts fail, the last
/// error is propagated.
///
/// Use [`RetryConfig::default()`] for standard backoff (50 ms initial, 5 s cap, 2x multiplier).
pub fn retry<T, F>(system: &AsyncSystem, max_attempts: u32, config: RetryConfig, f: F) -> Future<T>
where
    T: Send + 'static,
    F: Fn() -> Future<Result<T, AsyncError>> + Send + 'static,
{
    let system = system.clone();
    let (promise, future) = system.promise::<T>();

    system
        .inner
        .executor_for(crate::context::Context::BACKGROUND)
        .expect("Background executor")
        .execute(Box::new(move || {
            let mut last_err = AsyncError::msg("retry: no attempts");
            let mut backoff = config.initial_backoff;

            for _ in 0..max_attempts {
                match f().block() {
                    Ok(Ok(v)) => {
                        promise.resolve(v);
                        return;
                    }
                    Ok(Err(e)) => {
                        last_err = e;
                    }
                    Err(e) => {
                        last_err = e;
                    }
                }
                std::thread::sleep(backoff);
                backoff = (backoff * config.multiplier).min(config.max_backoff);
            }

            promise.reject(last_err);
        }));

    future
}
