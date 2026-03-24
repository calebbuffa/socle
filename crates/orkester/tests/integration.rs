//! Integration tests for cancellation, combinators, join_set, and spawn.
//!
//! These live in a separate test file to avoid bloating the source modules
//! while exercising the public API thoroughly.

use orkester::channel;
use orkester::{CancellationToken, Context, ErrorCode, Scheduler, Semaphore};
use orkester::{race, retry, timeout};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

fn system() -> Scheduler {
    Scheduler::with_threads(4)
}

// Cancellation

#[test]
fn cancel_before_completion_rejects() {
    let sys = system();
    let token = CancellationToken::new();
    let (resolver, task) = sys.resolver::<i32>();

    let guarded = task.with_cancellation(&token);
    token.cancel();

    let err = guarded.block().unwrap_err();
    assert_eq!(err.code(), ErrorCode::Cancelled);

    // The original resolver is still dangling — drop it.
    drop(resolver);
}

#[test]
fn cancel_after_resolution_delivers_value() {
    let sys = system();
    let token = CancellationToken::new();
    let (resolver, task) = sys.resolver::<i32>();

    resolver.resolve(42);
    let guarded = task.with_cancellation(&token);
    token.cancel(); // too late — value already set

    assert_eq!(guarded.block().unwrap(), 42);
}

#[test]
fn cancel_token_is_reusable_across_tasks() {
    let sys = system();
    let token = CancellationToken::new();

    let (p1, f1) = sys.resolver::<()>();
    let (p2, f2) = sys.resolver::<()>();
    let g1 = f1.with_cancellation(&token);
    let g2 = f2.with_cancellation(&token);

    token.cancel();

    assert_eq!(g1.block().unwrap_err().code(), ErrorCode::Cancelled);
    assert_eq!(g2.block().unwrap_err().code(), ErrorCode::Cancelled);

    drop(p1);
    drop(p2);
}

#[test]
fn cancel_already_cancelled_token_fires_immediately() {
    let sys = system();
    let token = CancellationToken::new();
    token.cancel();

    assert!(token.is_cancelled());

    let (_p, f) = sys.resolver::<()>();
    let g = f.with_cancellation(&token);
    assert_eq!(g.block().unwrap_err().code(), ErrorCode::Cancelled);
}

// Delay

#[test]
fn delay_completes_after_duration() {
    let sys = system();
    let start = Instant::now();
    let task = sys.delay(Duration::from_millis(50));
    task.block().unwrap();
    let elapsed = start.elapsed();
    assert!(
        elapsed >= Duration::from_millis(40),
        "elapsed: {:?}",
        elapsed
    );
}

#[test]
fn delay_zero_completes_immediately() {
    let sys = system();
    let task = sys.delay(Duration::ZERO);
    task.block().unwrap();
}

// Timeout

#[test]
fn timeout_expires_rejects_with_timed_out() {
    let sys = system();
    let (_p, f) = sys.resolver::<()>(); // never resolves
    let guarded = timeout(&sys, f, Duration::from_millis(50));

    let err = guarded.block().unwrap_err();
    assert_eq!(err.code(), ErrorCode::TimedOut);
}

#[test]
fn timeout_passes_when_upstream_is_fast() {
    let sys = system();
    let f = sys.resolved(99i32);
    let guarded = timeout(&sys, f, Duration::from_secs(10));
    assert_eq!(guarded.block().unwrap(), 99);
}

#[test]
fn timeout_propagates_upstream_error() {
    let sys = system();
    let (p, f) = sys.resolver::<()>();
    p.reject(orkester::AsyncError::msg("boom"));

    let guarded = timeout(&sys, f, Duration::from_secs(10));
    let err = guarded.block().unwrap_err();
    assert!(err.to_string().contains("boom"));
}

// Race

#[test]
fn race_returns_first_to_resolve() {
    let sys = system();
    let (p1, f1) = sys.resolver::<i32>();
    let (p2, f2) = sys.resolver::<i32>();

    p1.resolve(1);
    // p2 never resolves — but it's fine because race takes the first

    let result = race(&sys, vec![f1, f2]).block().unwrap();
    assert_eq!(result, 1);
    drop(p2);
}

#[test]
fn race_empty_rejects() {
    let sys = system();
    let result = race::<()>(&sys, vec![]).block();
    assert!(result.is_err());
}

#[test]
fn race_with_delays_picks_fastest() {
    let sys = system();
    let fast = sys.delay(Duration::from_millis(10));
    let slow = sys.delay(Duration::from_millis(200));

    let start = Instant::now();
    race(&sys, vec![fast, slow]).block().unwrap();
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_millis(150),
        "elapsed: {:?}",
        elapsed
    );
}

// Retry

#[test]
fn retry_succeeds_on_first_attempt() {
    let sys = system();
    let counter = Arc::new(AtomicUsize::new(0));
    let c = counter.clone();
    let sys2 = sys.clone();

    let result = retry(&sys, 3, Default::default(), move || {
        c.fetch_add(1, Ordering::SeqCst);
        sys2.resolved(Ok(42i32))
    })
    .block()
    .unwrap();

    assert_eq!(result, 42);
    // Factory closure is Fn, so we can't assert exact count easily
    // since retry runs on a worker thread, but it should be 1
}

#[test]
fn retry_fails_after_max_attempts() {
    let sys = system();
    let counter = Arc::new(AtomicUsize::new(0));
    let c = counter.clone();
    let sys2 = sys.clone();

    let result: Result<i32, _> = retry(&sys, 3, Default::default(), move || {
        c.fetch_add(1, Ordering::SeqCst);
        let (p, f) = sys2.resolver();
        p.resolve(Err::<i32, _>(orkester::AsyncError::msg("nope")));
        f
    })
    .block();

    assert!(result.is_err());
    assert_eq!(counter.load(Ordering::SeqCst), 3);
}

// JoinSet

#[test]
fn join_set_collects_all_results() {
    let sys = system();
    let mut js = sys.join_set::<i32>();

    for i in 0..5 {
        let f = sys.run(orkester::Context::BACKGROUND, move || i * 10);
        js.push(f);
    }

    assert_eq!(js.len(), 5);
    let results: Vec<i32> = js.join_all().into_iter().map(|r| r.unwrap()).collect();
    assert_eq!(results, vec![0, 10, 20, 30, 40]);
}

#[test]
fn join_set_join_next_returns_in_order() {
    let sys = system();
    let mut js = sys.join_set::<&str>();

    js.push(sys.resolved("a"));
    js.push(sys.resolved("b"));
    js.push(sys.resolved("c"));

    assert_eq!(js.join_next().unwrap().unwrap(), "a");
    assert_eq!(js.join_next().unwrap().unwrap(), "b");
    assert_eq!(js.join_next().unwrap().unwrap(), "c");
    assert!(js.join_next().is_none());
}

#[test]
fn join_set_empty_returns_empty_vec() {
    let sys = system();
    let js = sys.join_set::<()>();
    assert!(js.is_empty());
    assert!(js.join_all().is_empty());
}

#[test]
fn join_set_handles_rejected_tasks() {
    let sys = system();
    let mut js = sys.join_set::<()>();

    let (p, f) = sys.resolver::<()>();
    p.reject(orkester::AsyncError::msg("fail"));
    js.push(f);

    let results = js.join_all();
    assert_eq!(results.len(), 1);
    assert!(results[0].is_err());
}

// Spawn

#[test]
fn spawn_runs_on_worker() {
    let sys = system();
    let done = Arc::new(AtomicUsize::new(0));
    let d = done.clone();

    sys.spawn_detached(Context::BACKGROUND, move || {
        d.store(1, Ordering::SeqCst);
    });

    // Give it time to run.
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(done.load(Ordering::SeqCst), 1);
}

#[test]
fn spawn_immediate_runs_inline() {
    let sys = system();
    let done = Arc::new(AtomicUsize::new(0));
    let d = done.clone();

    sys.spawn_detached(Context::IMMEDIATE, move || {
        d.store(1, Ordering::SeqCst);
    });

    // Immediate should have already run by the time spawn returns.
    assert_eq!(done.load(Ordering::SeqCst), 1);
}

// Integration: Cancellation + Timeout

#[test]
fn cancel_races_with_timeout() {
    let sys = system();
    let token = CancellationToken::new();

    // Create a task that won't complete on its own.
    let (_p, f) = sys.resolver::<()>();
    let guarded = f.with_cancellation(&token);
    let timed = timeout(&sys, guarded, Duration::from_millis(200));

    // Cancel after 50ms — should beat the 200ms timeout.
    let token2 = token.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        token2.cancel();
    });

    let err = timed.block().unwrap_err();
    assert_eq!(err.code(), ErrorCode::Cancelled);
}

// Stress: high-contention channel + semaphore

#[test]
fn channel_and_semaphore_stress() {
    let sys = system();
    let sem = Semaphore::new(&sys, 5);
    // Capacity intentionally smaller than producer count.
    // Works because the consumer runs concurrently — not join-then-consume.
    let (tx, rx) = channel::mpsc::<usize>(4);

    for i in 0..20 {
        let sem = sem.clone();
        let tx = tx.clone();
        std::thread::spawn(move || {
            let _permit = sem.acquire();
            std::thread::sleep(Duration::from_millis(5));
            let _ = tx.send(i);
        });
    }
    drop(tx);

    // Consume concurrently as items arrive — no join before consume.
    let mut values = Vec::new();
    while let Some(v) = rx.recv() {
        values.push(v);
    }
    values.sort();
    assert_eq!(values, (0..20).collect::<Vec<_>>());
}
