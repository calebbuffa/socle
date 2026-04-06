use orkester::ThreadPool;
use std::future::Future as StdFutureTrait;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;

/// Create a `ThreadPool` with `n` threads and return its background context.
fn bg(n: usize) -> (ThreadPool, orkester::Context) {
    let pool = ThreadPool::new(n);
    let ctx = pool.context();
    (pool, ctx)
}

/// Create an immediately-rejected task.
fn rejected<T: Send + 'static>(msg: &'static str) -> orkester::Task<T> {
    let (r, task) = orkester::pair::<T>();
    r.reject(msg);
    task
}

struct CountingWake {
    wake_count: Arc<AtomicUsize>,
}

impl Wake for CountingWake {
    fn wake(self: Arc<Self>) {
        self.wake_count.fetch_add(1, Ordering::SeqCst);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_count.fetch_add(1, Ordering::SeqCst);
    }
}

fn make_counting_waker(wake_count: Arc<AtomicUsize>) -> Waker {
    Waker::from(Arc::new(CountingWake { wake_count }))
}

fn lcg_next(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    *seed
}

fn run_cross_context_roundtrip_stress(
    bg_ctx: &orkester::Context,
    iterations: usize,
    mut seed: u64,
) {
    for _ in 0..iterations {
        let sample = lcg_next(&mut seed);
        let should_fail = (sample & 1) == 1;
        let value = ((sample >> 16) % 1000) as i32;

        let base: orkester::Task<i32> = if should_fail {
            rejected("seeded failure")
        } else {
            orkester::resolved(value)
        };

        let chain = base
            .then(&bg_ctx, |v| v + 2)
            .then(&bg_ctx, |v| v * 2)
            .catch(&bg_ctx, |_| -5)
            .then(&bg_ctx, |v| v - 1);

        let observed = chain.block().unwrap();
        let expected = if should_fail {
            -6
        } else {
            ((value + 2) * 2) - 1
        };
        assert_eq!(observed, expected);
    }
}

fn run_shared_fanout_stress(iterations: usize, waiters: usize) {
    for iteration in 0..iterations {
        let (resolver, task) = orkester::pair::<usize>();
        let shared = task.share();
        let barrier = Arc::new(Barrier::new(waiters + 1));
        let mut handles = Vec::with_capacity(waiters);

        for _ in 0..waiters {
            let shared_clone = shared.clone();
            let barrier_clone = Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                barrier_clone.wait();
                shared_clone.block().unwrap()
            }));
        }

        barrier.wait();
        resolver.resolve(iteration);

        for handle in handles {
            assert_eq!(handle.join().unwrap(), iteration);
        }
        assert_eq!(shared.block().unwrap(), iteration);
    }
}

#[test]
fn create_promise_pair_resolves() {
    let (resolver, task) = orkester::pair::<i32>();
    resolver.resolve(42_i32);
    assert_eq!(task.block().unwrap(), 42);
}

#[test]
fn run_in_main_thread_is_inline_inside_scope() {
    let (_pool, ctx) = bg(1);
    let task = ctx.run(|| 7_i32);
    assert_eq!(task.block().unwrap(), 7);
}

#[test]
fn block_with_main_pumps_queue() {
    let (_pool, ctx) = bg(1);
    let task = ctx.run(|| 9_i32);
    assert_eq!(task.block().unwrap(), 9);
}

#[test]
fn shared_future_then_chain() {
    let (_pool, ctx) = bg(1);
    let shared = orkester::resolved(10_i32).share();
    let doubled = shared.then(&ctx, |value| value * 2);

    assert_eq!(doubled.block().unwrap(), 20);
    assert_eq!(shared.block().unwrap(), 10);
}

#[test]
fn all_future_values() {
    let futures = vec![
        orkester::resolved(1_i32),
        orkester::resolved(2_i32),
        orkester::resolved(3_i32),
    ];

    let joined = orkester::join_all(futures);
    assert_eq!(joined.block().unwrap(), vec![1, 2, 3]);
}

#[test]
fn run_in_worker_thread_flattens_future_result() {
    let (_pool, ctx) = bg(2);

    let flattened: orkester::Task<i32> = ctx.run(move || orkester::resolved(21_i32));
    assert_eq!(flattened.block().unwrap(), 21);
}

#[test]
fn then_in_worker_thread_flattens_future_result() {
    let (_pool, ctx) = bg(2);
    let ctx2 = ctx.clone();

    let flattened: orkester::Task<i32> =
        orkester::resolved(5_i32).then(&ctx, move |value| ctx2.run(move || value * 3));

    assert_eq!(flattened.block().unwrap(), 15);
}

#[test]
fn run_in_main_thread_flattens_future_result() {
    let (_pool, ctx) = bg(1);

    let flattened: orkester::Task<i32> = ctx.run(move || orkester::resolved(33_i32));
    assert_eq!(flattened.block().unwrap(), 33);
}

#[test]
fn then_in_worker_thread_flattens_rejected_future_result() {
    let (_pool, ctx) = bg(2);

    let flattened: orkester::Task<i32> =
        orkester::resolved(1_i32).then(&ctx, move |_| rejected::<i32>("boom"));

    let error = flattened.block().unwrap_err();
    assert_eq!(error.to_string(), "boom");
}

#[test]
fn map_runs_inline_for_resolved_future() {
    let caller_thread = std::thread::current().id();

    let same_thread =
        orkester::resolved(1_i32).map(move |_| std::thread::current().id() == caller_thread);

    assert!(same_thread.block().unwrap());
}

#[test]
fn all_accepts_shared_futures_via_map() {
    let shared = orkester::resolved(4_i32).share();

    let joined = orkester::join_all(vec![shared.map(|v| v), shared.map(|v| v)]);
    assert_eq!(joined.block().unwrap(), vec![4, 4]);
}

#[test]
fn all_rejects_when_any_input_rejects() {
    let futures = vec![
        orkester::resolved(1_i32),
        rejected("join failed"),
        orkester::resolved(3_i32),
    ];

    let joined = orkester::join_all(futures);
    let error = joined.block().unwrap_err();
    assert_eq!(error.to_string(), "join failed");
}

#[test]
fn shared_future_wait_is_consistent_for_concurrent_waiters() {
    const WAITERS: usize = 24;
    let (resolver, task) = orkester::pair::<usize>();
    let shared = task.share();
    let barrier = Arc::new(Barrier::new(WAITERS + 1));

    let mut handles = Vec::with_capacity(WAITERS);
    for _ in 0..WAITERS {
        let shared_clone = shared.clone();
        let barrier_clone = Arc::clone(&barrier);
        handles.push(std::thread::spawn(move || {
            barrier_clone.wait();
            shared_clone.block().unwrap()
        }));
    }

    barrier.wait();
    resolver.resolve(1234);

    for handle in handles {
        assert_eq!(handle.join().unwrap(), 1234);
    }
    assert_eq!(shared.block().unwrap(), 1234);
}

#[test]
fn future_poll_deduplicates_same_waker_registration() {
    let (resolver, mut task) = orkester::pair::<i32>();
    let wake_count = Arc::new(AtomicUsize::new(0));
    let waker = make_counting_waker(Arc::clone(&wake_count));
    let mut cx = Context::from_waker(&waker);
    let mut pinned = Pin::new(&mut task);

    assert!(matches!(
        StdFutureTrait::poll(pinned.as_mut(), &mut cx),
        Poll::Pending
    ));
    assert!(matches!(
        StdFutureTrait::poll(pinned.as_mut(), &mut cx),
        Poll::Pending
    ));

    resolver.resolve(7);

    assert_eq!(wake_count.load(Ordering::SeqCst), 1);
    assert!(matches!(
        StdFutureTrait::poll(pinned.as_mut(), &mut cx),
        Poll::Ready(Ok(7))
    ));
}

#[test]
fn shared_future_continuations_before_and_after_resolution_run_once() {
    const BEFORE: usize = 32;
    const AFTER: usize = 32;
    let (_pool, ctx) = bg(4);
    let (resolver, task) = orkester::pair::<usize>();
    let shared = task.share();
    let callback_count = Arc::new(AtomicUsize::new(0));

    let mut before = Vec::with_capacity(BEFORE);
    for _ in 0..BEFORE {
        let callback_count_clone = Arc::clone(&callback_count);
        before.push(shared.then(&ctx, move |value| {
            callback_count_clone.fetch_add(1, Ordering::SeqCst);
            value
        }));
    }

    resolver.resolve(77);

    for continuation in before {
        assert_eq!(continuation.block().unwrap(), 77);
    }

    let mut after = Vec::with_capacity(AFTER);
    for _ in 0..AFTER {
        let callback_count_clone = Arc::clone(&callback_count);
        after.push(shared.then(&ctx, move |value| {
            callback_count_clone.fetch_add(1, Ordering::SeqCst);
            value
        }));
    }

    for continuation in after {
        assert_eq!(continuation.block().unwrap(), 77);
    }

    assert_eq!(callback_count.load(Ordering::SeqCst), BEFORE + AFTER);
}

#[test]
fn block_with_main_handles_large_queue_backlog() {
    let (_pool, ctx) = bg(1);
    let mut futures = Vec::new();

    for value in 0_i32..128_i32 {
        futures.push(ctx.run(move || value));
    }

    let last = futures.pop().unwrap();
    assert_eq!(last.block().unwrap(), 127);

    for queued in futures {
        assert!(queued.block().is_ok());
    }
}

#[test]
fn long_worker_then_chain_preserves_value_ordering() {
    const STEPS: usize = 512;
    let (_pool, ctx) = bg(4);
    let mut current = orkester::resolved(0_usize);

    for _ in 0..STEPS {
        current = current.then(&ctx, |value| value + 1);
    }

    assert_eq!(current.block().unwrap(), STEPS);
}

#[test]
fn worker_to_main_to_worker_chain_completes_with_main_pump() {
    let (_pool, ctx) = bg(3);

    let chained = ctx
        .run(|| 3_i32)
        .then(&ctx, |value| value + 1)
        .then(&ctx, |value| value * 2);

    assert_eq!(chained.block().unwrap(), 8);
}

#[test]
fn rejected_chain_skips_then_and_recovers_in_main_thread() {
    let (_pool, ctx) = bg(3);
    let then_called = Arc::new(AtomicUsize::new(0));

    let failed: orkester::Task<i32> = ctx.run(move || rejected::<i32>("worker failure"));

    let then_called_clone = Arc::clone(&then_called);
    let recovered = failed
        .then(&ctx, move |value| {
            then_called_clone.fetch_add(1, Ordering::SeqCst);
            value + 1
        })
        .catch(&ctx, move |error| {
            assert_eq!(error.to_string(), "worker failure");
            42
        });

    assert_eq!(recovered.block().unwrap(), 42);
    assert_eq!(then_called.load(Ordering::SeqCst), 0);
}

#[test]
fn randomized_repeated_flatten_and_recovery_stress() {
    const ITERS: usize = 96;
    let (_pool, ctx) = bg(4);
    let mut seed = 0xDEC0_DED5_EED5_u64;

    for _ in 0..ITERS {
        let sample = lcg_next(&mut seed);
        let should_fail = (sample & 1) == 1;
        let value = ((sample >> 8) % 1000) as i32;
        let ctx2 = ctx.clone();

        let outcome: orkester::Task<i32> = ctx.run(move || {
            if should_fail {
                rejected::<i32>("random failure")
            } else {
                orkester::resolved(value)
            }
        });

        let recovered = outcome.or_else(|_| -1);
        let observed = recovered.block().unwrap();
        let _ = ctx2; // keep pool alive

        if should_fail {
            assert_eq!(observed, -1);
        } else {
            assert_eq!(observed, value);
        }
    }
}

#[test]
fn promise_drop_rejects_paired_future() {
    let (resolver, task) = orkester::pair::<i32>();
    drop(resolver);

    let error = task.block().unwrap_err();
    assert_eq!(error.to_string(), "Resolver dropped without resolving");
}

#[test]
fn all_empty_resolves_to_empty_vec() {
    let joined: orkester::Task<Vec<i32>> = orkester::join_all(Vec::<orkester::Task<i32>>::new());
    assert_eq!(joined.block().unwrap(), Vec::<i32>::new());
}

#[test]
fn all_preserves_input_order_when_resolved_out_of_order() {
    let (promise0, future0) = orkester::pair::<i32>();
    let (promise1, future1) = orkester::pair::<i32>();
    let (promise2, future2) = orkester::pair::<i32>();

    let joined = orkester::join_all(vec![future0, future1, future2]);

    let handle2 = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(2));
        promise2.resolve(30);
    });
    let handle0 = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(4));
        promise0.resolve(10);
    });
    let handle1 = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(8));
        promise1.resolve(20);
    });

    assert_eq!(joined.block().unwrap(), vec![10, 20, 30]);
    handle0.join().unwrap();
    handle1.join().unwrap();
    handle2.join().unwrap();
}

#[test]
fn catch_in_main_thread_is_not_called_on_success() {
    let (_pool, ctx) = bg(1);
    let catch_called = Arc::new(AtomicUsize::new(0));
    let catch_called_clone = Arc::clone(&catch_called);

    let passthrough = orkester::resolved(5_i32).catch(&ctx, move |_| {
        catch_called_clone.fetch_add(1, Ordering::SeqCst);
        -1
    });

    assert_eq!(passthrough.block().unwrap(), 5);
    assert_eq!(catch_called.load(Ordering::SeqCst), 0);
}

#[test]
fn catch_in_main_thread_recovers_on_main_when_pumped() {
    let (_pool, ctx) = bg(2);

    let recovered = rejected::<i32>("main recover").catch(&ctx, move |error| {
        assert_eq!(error.to_string(), "main recover");
        11
    });

    assert_eq!(recovered.block().unwrap(), 11);
}

#[test]
fn then_in_thread_pool_runs_inline_on_same_pool_thread() {
    let inner_pool = ThreadPool::new(1);
    let inner_ctx = inner_pool.context();

    // Both run on inner_ctx — the continuation should fire inline on the same thread.
    let same_thread = inner_ctx
        .run(|| std::thread::current().id())
        .then(&inner_ctx, |source_thread| {
            std::thread::current().id() == source_thread
        });

    assert!(same_thread.block().unwrap());
}

#[test]
fn then_in_thread_pool_runs_on_target_pool_context() {
    let pool = ThreadPool::new(2);
    let bg_ctx = pool.context();
    let inner_pool = ThreadPool::new(1);
    let inner_ctx = inner_pool.context();

    let pool_thread = inner_ctx
        .run(|| std::thread::current().id())
        .block()
        .unwrap();

    let observed = bg_ctx
        .run(|| 1_i32)
        .then(&inner_ctx, move |_| std::thread::current().id());

    assert_eq!(observed.block().unwrap(), pool_thread);
}

#[test]
fn shared_handle_concurrent_block_resolves_consistently() {
    let (resolver, task) = orkester::pair::<i32>();
    let shared_a = task.share();
    let shared_b = shared_a.clone();

    let join_handle = std::thread::spawn(move || shared_b.block().unwrap());
    resolver.resolve(55);

    assert_eq!(shared_a.block().unwrap(), 55);
    assert_eq!(join_handle.join().unwrap(), 55);
}

#[test]
fn future_poll_ready_then_wait_reports_consumed() {
    let mut task = orkester::resolved(9_i32);
    let wake_count = Arc::new(AtomicUsize::new(0));
    let waker = make_counting_waker(Arc::clone(&wake_count));
    let mut cx = Context::from_waker(&waker);
    let mut pinned = Pin::new(&mut task);

    assert!(matches!(
        StdFutureTrait::poll(pinned.as_mut(), &mut cx),
        Poll::Ready(Ok(9))
    ));
    drop(pinned);

    let error = task.block().unwrap_err();
    assert_eq!(wake_count.load(Ordering::SeqCst), 0);
    assert_eq!(error.to_string(), "Task already consumed");
}

#[test]
fn dispatch_one_main_thread_task_reports_queue_progress() {
    let (_pool, ctx) = bg(1);
    let tasks: Vec<_> = (0_i32..3_i32).map(|value| ctx.run(move || value)).collect();

    for (idx, task) in tasks.into_iter().enumerate() {
        assert_eq!(task.block().unwrap(), idx as i32);
    }
}

#[test]
fn repeated_shared_future_fanout_stress() {
    run_shared_fanout_stress(32, 8);
}

#[test]
fn randomized_cross_context_then_catch_roundtrip_stress() {
    let (_pool, ctx) = bg(4);
    run_cross_context_roundtrip_stress(&ctx, 128, 0xA11C_EB0B_1357_2468_u64);
}

#[test]
#[ignore]
fn soak_randomized_cross_context_then_catch_roundtrip() {
    let (_pool, ctx) = bg(4);
    run_cross_context_roundtrip_stress(&ctx, 512, 0xFACE_FEED_BADC_0FFE_u64);
}

#[test]
#[ignore]
fn soak_randomized_cross_context_then_catch_roundtrip_alt_seed_a() {
    let (_pool, ctx) = bg(4);
    run_cross_context_roundtrip_stress(&ctx, 1024, 0x0123_4567_89AB_CDEF_u64);
}

#[test]
#[ignore]
fn soak_randomized_cross_context_then_catch_roundtrip_alt_seed_b() {
    let (_pool, ctx) = bg(4);
    run_cross_context_roundtrip_stress(&ctx, 2048, 0x0F0F_F0F0_AAAA_5555_u64);
}

#[test]
#[ignore]
fn soak_shared_future_fanout_high_contention() {
    run_shared_fanout_stress(96, 16);
}
