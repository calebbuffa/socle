//! Async combinators: `timeout`, `race`, `retry`, `join`, `join_all`, `join_all_settle`.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::context::Context;
use crate::error::{AsyncError, ErrorCode};
use crate::task::{Resolver, Task, TaskInner, create_pair};
use crate::task_cell::TaskCell;

/// Wraps a task with a timeout. If the upstream task does not complete
/// within `duration`, the returned task rejects with [`ErrorCode::TimedOut`].
pub fn timeout<T: Send + 'static>(task: Task<T>, duration: Duration) -> Task<T> {
    let timer = crate::delay(duration);
    let (resolve_resolver, output) = create_pair::<T>();
    let claimed = Arc::new(AtomicBool::new(false));
    let shared_resolver = Arc::new(Mutex::new(Some(resolve_resolver)));

    // Path 1: upstream completes in time
    let claimed1 = Arc::clone(&claimed);
    let sp1 = Arc::clone(&shared_resolver);
    match task.inner {
        TaskInner::Ready(result) => {
            if !claimed1.swap(true, Ordering::AcqRel) {
                let resolver = sp1.lock().expect("timeout lock").take();
                if let Some(resolver) = resolver {
                    match result {
                        Some(Ok(v)) => resolver.resolve(v),
                        Some(Err(e)) => resolver.reject(e),
                        None => resolver.reject(AsyncError::msg("Task already consumed")),
                    }
                }
            }
        }
        TaskInner::Pending(cell) => {
            TaskCell::on_complete(cell, move |result| {
                if !claimed1.swap(true, Ordering::AcqRel) {
                    let resolver = sp1.lock().expect("timeout lock").take();
                    if let Some(resolver) = resolver {
                        match result {
                            Ok(v) => resolver.resolve(v),
                            Err(e) => resolver.reject(e),
                        }
                    }
                }
            });
        }
    }

    // Path 2: timer fires first → reject with TimedOut
    let claimed2 = claimed;
    let sp2 = shared_resolver;
    match timer.inner {
        TaskInner::Ready(_) => {
            if !claimed2.swap(true, Ordering::AcqRel) {
                let resolver = sp2.lock().expect("timeout lock").take();
                if let Some(resolver) = resolver {
                    resolver.reject(AsyncError::with_code(ErrorCode::TimedOut, "timed out"));
                }
            }
        }
        TaskInner::Pending(timer_cell) => {
            TaskCell::on_complete(timer_cell, move |_| {
                if !claimed2.swap(true, Ordering::AcqRel) {
                    let resolver = sp2.lock().expect("timeout lock").take();
                    if let Some(resolver) = resolver {
                        resolver.reject(AsyncError::with_code(ErrorCode::TimedOut, "timed out"));
                    }
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
pub fn race<T: Send + 'static>(tasks: Vec<Task<T>>) -> Task<T> {
    if tasks.is_empty() {
        let (resolver, task) = create_pair::<T>();
        resolver.reject(AsyncError::msg("race called with no tasks"));
        return task;
    }

    let (resolve_resolver, output) = create_pair::<T>();
    let claimed = Arc::new(AtomicBool::new(false));
    let shared_resolver = Arc::new(Mutex::new(Some(resolve_resolver)));

    for task in tasks {
        let claimed = Arc::clone(&claimed);
        let sp = Arc::clone(&shared_resolver);
        match task.inner {
            TaskInner::Ready(result) => {
                if !claimed.swap(true, Ordering::AcqRel) {
                    let resolver = sp.lock().expect("race lock").take();
                    if let Some(resolver) = resolver {
                        match result {
                            Some(Ok(v)) => resolver.resolve(v),
                            Some(Err(e)) => resolver.reject(e),
                            None => {}
                        }
                    }
                }
            }
            TaskInner::Pending(cell) => {
                TaskCell::on_complete(cell, move |result| {
                    if !claimed.swap(true, Ordering::AcqRel) {
                        let resolver = sp.lock().expect("race lock").take();
                        if let Some(resolver) = resolver {
                            match result {
                                Ok(v) => resolver.resolve(v),
                                Err(e) => resolver.reject(e),
                            }
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
    /// Multiplier applied to the backoff after each attempt (default: 2.0).
    /// Fractional values (e.g. `1.5`) are supported.
    pub multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            initial_backoff: Duration::from_millis(50),
            max_backoff: Duration::from_secs(5),
            multiplier: 2.0,
        }
    }
}

/// Retry a fallible async operation with exponential backoff.
///
/// Calls `f()` up to `max_attempts` times on `context`. If an attempt returns
/// `Ok(v)`, the returned task resolves with `v`. If all attempts fail, the
/// last error is propagated.
///
/// # Thread parking
///
/// Between retry attempts this function calls `std::thread::sleep`. When
/// `context` is a [`ThreadPool`](crate::ThreadPool), that parks one worker
/// thread for the backoff duration. Avoid very large `max_backoff` values
/// combined with many concurrent retries — consider using [`delay`](crate::delay)
/// instead of retry if non-blocking backoff is critical.
pub fn retry<T, F>(context: &Context, max_attempts: u32, config: RetryConfig, f: F) -> Task<T>
where
    T: Send + 'static,
    F: Fn() -> Task<Result<T, AsyncError>> + Send + 'static,
{
    let (resolver, output) = create_pair::<T>();

    let executor = match context.executor_opt() {
        Some(e) => e,
        None => {
            // IMMEDIATE: run inline
            let mut last_err = AsyncError::msg("retry: no attempts");
            let mut backoff = config.initial_backoff;
            for _ in 0..max_attempts {
                match f().block() {
                    Ok(Ok(v)) => {
                        resolver.resolve(v);
                        return output;
                    }
                    Ok(Err(e)) => last_err = e,
                    Err(e) => last_err = e,
                }
                std::thread::sleep(backoff);
                backoff = backoff.mul_f64(config.multiplier).min(config.max_backoff);
            }
            resolver.reject(last_err);
            return output;
        }
    };

    executor.execute(Box::new(move || {
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
            backoff = backoff.mul_f64(config.multiplier).min(config.max_backoff);
        }

        resolver.reject(last_err);
    }));

    output
}

/// Resolves with `(A, B)` when both tasks complete successfully.
/// If either task rejects, the returned task rejects immediately.
///
/// Prefer [`Task::join`] which is the ergonomic entry point.
pub(crate) fn join<A, B>(a: Task<A>, b: Task<B>) -> Task<(A, B)>
where
    A: Send + 'static,
    B: Send + 'static,
{
    let (resolver, output) = create_pair::<(A, B)>();

    struct State<A: Send + 'static, B: Send + 'static> {
        a: Option<A>,
        b: Option<B>,
        resolver: Option<Resolver<(A, B)>>,
    }

    let state: Arc<Mutex<State<A, B>>> = Arc::new(Mutex::new(State {
        a: None,
        b: None,
        resolver: Some(resolver),
    }));

    let sa = Arc::clone(&state);
    let complete_a = move |result: Result<A, AsyncError>| {
        let mut s = sa.lock().expect("join lock");
        if s.resolver.is_none() {
            return;
        }
        match result {
            Ok(v) => {
                s.a = Some(v);
                if s.b.is_some() {
                    let r = s.resolver.take().unwrap();
                    r.resolve((s.a.take().unwrap(), s.b.take().unwrap()));
                }
            }
            Err(e) => {
                if let Some(r) = s.resolver.take() {
                    r.reject(e);
                }
            }
        }
    };
    match a.inner {
        TaskInner::Ready(Some(r)) => complete_a(r),
        TaskInner::Ready(None) => complete_a(Err(AsyncError::msg("Task already consumed"))),
        TaskInner::Pending(cell) => TaskCell::on_complete(cell, complete_a),
    }

    let sb = Arc::clone(&state);
    let complete_b = move |result: Result<B, AsyncError>| {
        let mut s = sb.lock().expect("join lock");
        if s.resolver.is_none() {
            return;
        }
        match result {
            Ok(v) => {
                s.b = Some(v);
                if s.a.is_some() {
                    let r = s.resolver.take().unwrap();
                    r.resolve((s.a.take().unwrap(), s.b.take().unwrap()));
                }
            }
            Err(e) => {
                if let Some(r) = s.resolver.take() {
                    r.reject(e);
                }
            }
        }
    };
    match b.inner {
        TaskInner::Ready(Some(r)) => complete_b(r),
        TaskInner::Ready(None) => complete_b(Err(AsyncError::msg("Task already consumed"))),
        TaskInner::Pending(cell) => TaskCell::on_complete(cell, complete_b),
    }

    output
}

/// Wait for all tasks to complete, collecting results in insertion order.
///
/// Rejects immediately if any task rejects. Use [`join_all_settle`] to
/// collect all results regardless of failure.
pub fn join_all<T, I>(tasks: I) -> Task<Vec<T>>
where
    T: Send + 'static,
    I: IntoIterator<Item = Task<T>>,
{
    let inputs: Vec<Task<T>> = tasks.into_iter().collect();
    let count = inputs.len();

    if count == 0 {
        return Task::ready(Vec::new());
    }

    let (res, output) = create_pair::<Vec<T>>();
    let results: Arc<Mutex<Vec<Option<Result<T, AsyncError>>>>> =
        Arc::new(Mutex::new((0..count).map(|_| None).collect()));
    let remaining = Arc::new(AtomicUsize::new(count));
    let shared_resolver = Arc::new(Mutex::new(Some(res)));

    fn settle<T: Send + 'static>(
        results: Arc<Mutex<Vec<Option<Result<T, AsyncError>>>>>,
        shared_resolver: Arc<Mutex<Option<Resolver<Vec<T>>>>>,
    ) {
        let res = match shared_resolver
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
        {
            Some(r) => r,
            None => return,
        };
        let mut guard = results.lock().unwrap_or_else(|p| p.into_inner());
        let mut values = Vec::with_capacity(guard.len());
        for slot in guard.iter_mut() {
            match slot.take() {
                Some(Ok(v)) => values.push(v),
                Some(Err(e)) => {
                    res.reject(e);
                    return;
                }
                None => {
                    res.reject(AsyncError::msg("join_all: missing result"));
                    return;
                }
            }
        }
        res.resolve(values);
    }

    for (i, t) in inputs.into_iter().enumerate() {
        let results = Arc::clone(&results);
        let remaining = Arc::clone(&remaining);
        let shared_resolver = Arc::clone(&shared_resolver);
        match t.inner {
            TaskInner::Ready(result) => {
                let result =
                    result.unwrap_or_else(|| Err(AsyncError::msg("Task already consumed")));
                results.lock().unwrap_or_else(|p| p.into_inner())[i] = Some(result);
                if remaining.fetch_sub(1, Ordering::AcqRel) == 1 {
                    settle(results, shared_resolver);
                }
            }
            TaskInner::Pending(cell) => {
                TaskCell::on_complete(cell, move |result| {
                    results.lock().unwrap_or_else(|p| p.into_inner())[i] = Some(result);
                    if remaining.fetch_sub(1, Ordering::AcqRel) == 1 {
                        settle(results, shared_resolver);
                    }
                });
            }
        }
    }

    output
}

/// Like [`join_all`] but waits for **all** tasks regardless of failures.
///
/// Returns one `Result<T, AsyncError>` per input task, in insertion order.
pub fn join_all_settle<T, I>(tasks: I) -> Task<Vec<Result<T, AsyncError>>>
where
    T: Send + 'static,
    I: IntoIterator<Item = Task<T>>,
{
    let inputs: Vec<Task<T>> = tasks.into_iter().collect();
    let count = inputs.len();

    if count == 0 {
        return Task::ready(Vec::new());
    }

    let (res, output) = create_pair::<Vec<Result<T, AsyncError>>>();
    let results: Arc<Mutex<Vec<Option<Result<T, AsyncError>>>>> =
        Arc::new(Mutex::new((0..count).map(|_| None).collect()));
    let remaining = Arc::new(AtomicUsize::new(count));
    let shared_resolver = Arc::new(Mutex::new(Some(res)));

    for (i, t) in inputs.into_iter().enumerate() {
        let results = Arc::clone(&results);
        let remaining = Arc::clone(&remaining);
        let shared_resolver = Arc::clone(&shared_resolver);

        let finish = move |result: Result<T, AsyncError>| {
            results.lock().unwrap_or_else(|p| p.into_inner())[i] = Some(result);
            if remaining.fetch_sub(1, Ordering::AcqRel) == 1 {
                let res = match shared_resolver
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .take()
                {
                    Some(r) => r,
                    None => return,
                };
                let mut guard = results.lock().unwrap_or_else(|p| p.into_inner());
                let values = guard
                    .iter_mut()
                    .map(|s| {
                        s.take().unwrap_or_else(|| {
                            Err(AsyncError::msg("join_all_settle: missing result"))
                        })
                    })
                    .collect();
                res.resolve(values);
            }
        };

        match t.inner {
            TaskInner::Ready(result) => {
                finish(result.unwrap_or_else(|| Err(AsyncError::msg("Task already consumed"))));
            }
            TaskInner::Pending(cell) => {
                TaskCell::on_complete(cell, finish);
            }
        }
    }

    output
}
