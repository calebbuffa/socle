//! Integration tests for cancellation, combinators, join_set, and spawn.
//!
//! These live in a separate test file to avoid bloating the source modules
//! while exercising the public API thoroughly.

use orkester::{CancellationToken, Context, ErrorCode, JoinSet, Semaphore, ThreadPool};
use orkester::{race, retry, timeout};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

fn bg() -> (ThreadPool, Context) {
    let pool = ThreadPool::new(4);
    let ctx = pool.context();
    (pool, ctx)
}

// Cancellation

#[test]
fn cancel_before_completion_rejects() {
    let token = CancellationToken::new();
    let (resolver, task) = orkester::pair::<i32>();

    let guarded = task.with_cancellation(&token);
    token.cancel();

    let err = guarded.block().unwrap_err();
    assert_eq!(err.code(), ErrorCode::Cancelled);

    drop(resolver);
}

#[test]
fn cancel_after_resolution_delivers_value() {
    let token = CancellationToken::new();
    let (resolver, task) = orkester::pair::<i32>();

    resolver.resolve(42);
    let guarded = task.with_cancellation(&token);
    token.cancel();

    assert_eq!(guarded.block().unwrap(), 42);
}

#[test]
fn cancel_token_is_reusable_across_tasks() {
    let token = CancellationToken::new();

    let (p1, f1) = orkester::pair::<()>();
    let (p2, f2) = orkester::pair::<()>();
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
    let token = CancellationToken::new();
    token.cancel();

    assert!(token.is_cancelled());

    let (_p, f) = orkester::pair::<()>();
    let g = f.with_cancellation(&token);
    assert_eq!(g.block().unwrap_err().code(), ErrorCode::Cancelled);
}

// Delay

#[test]
fn delay_completes_after_duration() {
    let start = Instant::now();
    let task = orkester::delay(Duration::from_millis(50));
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
    let task = orkester::delay(Duration::ZERO);
    task.block().unwrap();
}

// Timeout

#[test]
fn timeout_expires_rejects_with_timed_out() {
    let (_p, f) = orkester::pair::<()>();
    let guarded = timeout(f, Duration::from_millis(50));

    let err = guarded.block().unwrap_err();
    assert_eq!(err.code(), ErrorCode::TimedOut);
}

#[test]
fn timeout_passes_when_upstream_is_fast() {
    let f = orkester::resolved(99i32);
    let guarded = timeout(f, Duration::from_secs(10));
    assert_eq!(guarded.block().unwrap(), 99);
}

#[test]
fn timeout_propagates_upstream_error() {
    let (p, f) = orkester::pair::<()>();
    p.reject(orkester::AsyncError::msg("boom"));

    let guarded = timeout(f, Duration::from_secs(10));
    let err = guarded.block().unwrap_err();
    assert!(err.to_string().contains("boom"));
}

// Race

#[test]
fn race_returns_first_to_resolve() {
    let (p1, f1) = orkester::pair::<i32>();
    let (p2, f2) = orkester::pair::<i32>();

    p1.resolve(1);

    let result = race(vec![f1, f2]).block().unwrap();
    assert_eq!(result, 1);
    drop(p2);
}

#[test]
fn race_empty_rejects() {
    let result = race::<()>(vec![]).block();
    assert!(result.is_err());
}

#[test]
fn race_with_delays_picks_fastest() {
    let fast = orkester::delay(Duration::from_millis(10));
    let slow = orkester::delay(Duration::from_millis(200));

    let start = Instant::now();
    race(vec![fast, slow]).block().unwrap();
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
    let (_pool, sys) = bg();
    let counter = Arc::new(AtomicUsize::new(0));
    let c = counter.clone();

    let result = retry(&sys, 3, Default::default(), move || {
        c.fetch_add(1, Ordering::SeqCst);
        orkester::resolved(Ok(42i32))
    })
    .block()
    .unwrap();

    assert_eq!(result, 42);
}

#[test]
fn retry_fails_after_max_attempts() {
    let (_pool, sys) = bg();
    let counter = Arc::new(AtomicUsize::new(0));
    let c = counter.clone();

    let result: Result<i32, _> = retry(&sys, 3, Default::default(), move || {
        c.fetch_add(1, Ordering::SeqCst);
        orkester::resolved(Err::<i32, _>(orkester::AsyncError::msg("nope")))
    })
    .block();

    assert!(result.is_err());
    assert_eq!(counter.load(Ordering::SeqCst), 3);
}

// JoinSet

#[test]
fn join_set_collects_all_results() {
    let (_pool, sys) = bg();
    let mut js = JoinSet::<i32>::new();

    for i in 0..5 {
        let f = sys.run(move || i * 10);
        js.push(f);
    }

    assert_eq!(js.len(), 5);
    let results: Vec<i32> = js.block_all().into_iter().map(|r| r.unwrap()).collect();
    assert_eq!(results, vec![0, 10, 20, 30, 40]);
}

#[test]
fn join_set_empty_returns_empty_vec() {
    let js = JoinSet::<()>::new();
    assert!(js.is_empty());
    assert!(js.block_all().is_empty());
}

#[test]
fn join_set_handles_rejected_tasks() {
    let mut js = JoinSet::<()>::new();

    let (p, f) = orkester::pair::<()>();
    p.reject(orkester::AsyncError::msg("fail"));
    js.push(f);

    let results = js.block_all();
    assert_eq!(results.len(), 1);
    assert!(results[0].is_err());
}

// Spawn

#[test]
fn spawn_runs_on_worker() {
    let (_pool, sys) = bg();
    let done = Arc::new(AtomicUsize::new(0));
    let d = done.clone();

    orkester::spawn_detached(&sys, move || {
        d.store(1, Ordering::SeqCst);
    });

    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(done.load(Ordering::SeqCst), 1);
}

#[test]
fn spawn_immediate_runs_inline() {
    let done = Arc::new(AtomicUsize::new(0));
    let d = done.clone();

    orkester::spawn_detached(&Context::IMMEDIATE, move || {
        d.store(1, Ordering::SeqCst);
    });

    assert_eq!(done.load(Ordering::SeqCst), 1);
}

// Integration: Cancellation + Timeout

#[test]
fn cancel_races_with_timeout() {
    let token = CancellationToken::new();

    let (_p, f) = orkester::pair::<()>();
    let guarded = f.with_cancellation(&token);
    let timed = timeout(guarded, Duration::from_millis(200));

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
    let sem = Semaphore::new(5);
    // Capacity intentionally smaller than producer count.
    // Works because the consumer runs concurrently — not join-then-consume.
    let (tx, rx) = orkester::mpsc::<usize>(4);

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
