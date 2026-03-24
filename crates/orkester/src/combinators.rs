//! Async combinators: `timeout`, `race`, `retry`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::error::{AsyncError, ErrorCode};
use crate::scheduler::Scheduler;
use crate::task::{Task, TaskInner};
use crate::task_cell::TaskCell;

/// Wraps a task with a timeout. If the upstream task does not complete
/// within `duration`, the returned task rejects with [`ErrorCode::TimedOut`].
pub fn timeout<T: Send + 'static>(
    system: &Scheduler,
    task: Task<T>,
    duration: Duration,
) -> Task<T> {
    let timer = system.delay(duration);
    let (resolve_resolver, output) = system.resolver::<T>();
    let shared_resolver = Arc::new(Mutex::new(Some(resolve_resolver)));

    // Path 1: upstream completes in time
    let sp1 = shared_resolver.clone();
    match task.inner {
        TaskInner::Ready(result) => {
            let resolver = sp1.lock().expect("timeout lock").take();
            if let Some(resolver) = resolver {
                match result {
                    Some(Ok(v)) => resolver.resolve(v),
                    Some(Err(e)) => resolver.reject(e),
                    None => resolver.reject(AsyncError::msg("Task already consumed")),
                }
            }
        }
        TaskInner::Pending(cell) => {
            let cell_ref = Arc::clone(&cell);
            TaskCell::on_complete(cell, move |result| {
                let _ = cell_ref;
                let resolver = sp1.lock().expect("timeout lock").take();
                if let Some(resolver) = resolver {
                    match result {
                        Ok(v) => resolver.resolve(v),
                        Err(e) => resolver.reject(e),
                    }
                }
            });
        }
    }

    // Path 2: timer fires first → reject with TimedOut
    let sp2 = shared_resolver;
    match timer.inner {
        TaskInner::Ready(_) => {
            let resolver = sp2.lock().expect("timeout lock").take();
            if let Some(resolver) = resolver {
                resolver.reject(AsyncError::with_code(ErrorCode::TimedOut, "timed out"));
            }
        }
        TaskInner::Pending(timer_cell) => {
            TaskCell::on_complete(timer_cell, move |_| {
                let resolver = sp2.lock().expect("timeout lock").take();
                if let Some(resolver) = resolver {
                    resolver.reject(AsyncError::with_code(ErrorCode::TimedOut, "timed out"));
                }
            });
        }
    }

    output
}

/// Completes when the **first** input task completes.
/// All other tasks are dropped (their results are discarded).
///
/// If the input vector is empty, the returned task is immediately rejected.
pub fn race<T: Send + 'static>(system: &Scheduler, tasks: Vec<Task<T>>) -> Task<T> {
    if tasks.is_empty() {
        let (resolver, task) = system.resolver::<T>();
        resolver.reject(AsyncError::msg("race called with no tasks"));
        return task;
    }

    let (resolve_resolver, output) = system.resolver::<T>();
    let shared_resolver = Arc::new(Mutex::new(Some(resolve_resolver)));

    for task in tasks {
        let sp = shared_resolver.clone();
        match task.inner {
            TaskInner::Ready(result) => {
                let resolver = sp.lock().expect("race lock").take();
                if let Some(resolver) = resolver {
                    match result {
                        Some(Ok(v)) => resolver.resolve(v),
                        Some(Err(e)) => resolver.reject(e),
                        None => {} // already consumed
                    }
                }
            }
            TaskInner::Pending(cell) => {
                TaskCell::on_complete(cell, move |result| {
                    let resolver = sp.lock().expect("race lock").take();
                    if let Some(resolver) = resolver {
                        match result {
                            Ok(v) => resolver.resolve(v),
                            Err(e) => resolver.reject(e),
                        }
                    }
                });
            }
        }
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
/// the returned task resolves with `v`. If all attempts fail, the last
/// error is propagated.
///
/// Use [`RetryConfig::default()`] for standard backoff (50 ms initial, 5 s cap, 2x multiplier).
pub fn retry<T, F>(system: &Scheduler, max_attempts: u32, config: RetryConfig, f: F) -> Task<T>
where
    T: Send + 'static,
    F: Fn() -> Task<Result<T, AsyncError>> + Send + 'static,
{
    let system = system.clone();
    let (resolver, output) = system.resolver::<T>();

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
                        resolver.resolve(v);
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

            resolver.reject(last_err);
        }));

    output
}
