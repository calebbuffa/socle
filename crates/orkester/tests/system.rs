use orkester::{AsyncSystem, ThreadPoolTaskProcessor};
use std::future::Future as StdFutureTrait;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;

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

fn run_cross_context_roundtrip_stress(system: &AsyncSystem, iterations: usize, mut seed: u64) {
    for _ in 0..iterations {
        let sample = lcg_next(&mut seed);
        let should_fail = (sample & 1) == 1;
        let value = ((sample >> 16) % 1000) as i32;

        let base: orkester::Future<i32> = if should_fail {
            system.create_future(|promise| promise.reject("seeded failure"))
        } else {
            system.create_resolved_future(value)
        };

        let chain = base
            .then(orkester::Context::Worker, |v| v + 2)
            .then(orkester::Context::Main, |v| v * 2)
            .catch(orkester::Context::Main, |_| -5)
            .then(orkester::Context::Worker, |v| v - 1);

        let observed = chain.wait_in_main_thread().unwrap();
        let expected = if should_fail {
            -6
        } else {
            ((value + 2) * 2) - 1
        };
        assert_eq!(observed, expected);
    }
}

fn run_shared_fanout_stress(system: &AsyncSystem, iterations: usize, waiters: usize) {
    for iteration in 0..iterations {
        let (promise, future) = system.create_promise::<usize>();
        let shared = future.share();
        let barrier = Arc::new(Barrier::new(waiters + 1));
        let mut handles = Vec::with_capacity(waiters);

        for _ in 0..waiters {
            let shared_clone = shared.clone();
            let barrier_clone = Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                barrier_clone.wait();
                shared_clone.wait().unwrap()
            }));
        }

        barrier.wait();
        promise.resolve(iteration);

        for handle in handles {
            assert_eq!(handle.join().unwrap(), iteration);
        }
        assert_eq!(shared.wait().unwrap(), iteration);
    }
}

#[test]
fn create_promise_pair_resolves() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(1)));
    let (promise, future) = system.create_promise();
    promise.resolve(42_i32);
    assert_eq!(future.wait().unwrap(), 42);
}

#[test]
fn run_in_main_thread_is_inline_inside_scope() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(1)));
    let _scope = system.enter_main_thread();
    let future = system.run(orkester::Context::Main, || 7_i32);
    assert!(future.is_ready());
    assert_eq!(future.wait().unwrap(), 7);
}

#[test]
fn wait_in_main_thread_pumps_queue() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(1)));
    let future = system.run(orkester::Context::Main, || 9_i32);
    assert_eq!(future.wait_in_main_thread().unwrap(), 9);
}

#[test]
fn shared_future_then_chain() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(1)));
    let shared = system.create_resolved_future(10_i32).share();
    let doubled = shared.then(orkester::Context::Worker, |value| value * 2);

    assert_eq!(doubled.wait().unwrap(), 20);
    assert_eq!(shared.wait().unwrap(), 10);
}

#[test]
fn all_future_values() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(2)));
    let futures = vec![
        system.create_resolved_future(1_i32),
        system.create_resolved_future(2_i32),
        system.create_resolved_future(3_i32),
    ];

    let joined = system.all(futures);
    assert_eq!(joined.wait().unwrap(), vec![1, 2, 3]);
}

#[test]
fn run_in_worker_thread_flattens_future_result() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(2)));
    let system_clone = system.clone();

    let flattened: orkester::Future<i32> =
        system.run(orkester::Context::Worker, move || system_clone.create_resolved_future(21_i32));
    assert_eq!(flattened.wait().unwrap(), 21);
}

#[test]
fn then_in_worker_thread_flattens_future_result() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(2)));
    let system_clone = system.clone();

    let flattened: orkester::Future<i32> = system
        .create_resolved_future(5_i32)
        .then(orkester::Context::Worker, move |value| system_clone.run(orkester::Context::Worker, move || value * 3));

    assert_eq!(flattened.wait().unwrap(), 15);
}

#[test]
fn run_in_main_thread_flattens_future_result() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(1)));
    let _scope = system.enter_main_thread();
    let system_clone = system.clone();

    let flattened: orkester::Future<i32> =
        system.run(orkester::Context::Main, move || system_clone.create_resolved_future(33_i32));
    assert!(flattened.is_ready());
    assert_eq!(flattened.wait().unwrap(), 33);
}

#[test]
fn then_in_worker_thread_flattens_rejected_future_result() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(2)));
    let system_clone = system.clone();

    let flattened: orkester::Future<i32> = system
        .create_resolved_future(1_i32)
        .then(orkester::Context::Worker, move |_| {
            system_clone.create_future(|promise| promise.reject("boom"))
        });

    let error = flattened.wait().unwrap_err();
    assert_eq!(error.to_string(), "boom");
}

#[test]
fn then_immediately_runs_inline_for_resolved_future() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(1)));
    let caller_thread = std::thread::current().id();

    let same_thread = system
        .create_resolved_future(1_i32)
        .then_immediately(move |_| std::thread::current().id() == caller_thread);

    assert!(same_thread.wait().unwrap());
}

#[test]
fn all_accepts_shared_futures() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(1)));
    let shared = system.create_resolved_future(4_i32).share();

    let joined = system.all(vec![shared.clone(), shared]);
    assert_eq!(joined.wait().unwrap(), vec![4, 4]);
}

#[test]
fn all_rejects_when_any_input_rejects() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(2)));
    let futures = vec![
        system.create_resolved_future(1_i32),
        system.create_future(|promise| promise.reject("join failed")),
        system.create_resolved_future(3_i32),
    ];

    let joined = system.all(futures);
    let error = joined.wait().unwrap_err();
    assert_eq!(error.to_string(), "join failed");
}

#[test]
fn shared_future_wait_is_consistent_for_concurrent_waiters() {
    const WAITERS: usize = 24;
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(4)));
    let (promise, future) = system.create_promise::<usize>();
    let shared = future.share();
    let barrier = Arc::new(Barrier::new(WAITERS + 1));

    let mut handles = Vec::with_capacity(WAITERS);
    for _ in 0..WAITERS {
        let shared_clone = shared.clone();
        let barrier_clone = Arc::clone(&barrier);
        handles.push(std::thread::spawn(move || {
            barrier_clone.wait();
            shared_clone.wait().unwrap()
        }));
    }

    barrier.wait();
    promise.resolve(1234);

    for handle in handles {
        assert_eq!(handle.join().unwrap(), 1234);
    }
    assert_eq!(shared.wait().unwrap(), 1234);
}

#[test]
fn future_poll_deduplicates_same_waker_registration() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(1)));
    let (promise, mut future) = system.create_promise::<i32>();
    let wake_count = Arc::new(AtomicUsize::new(0));
    let waker = make_counting_waker(Arc::clone(&wake_count));
    let mut cx = Context::from_waker(&waker);
    let mut pinned = Pin::new(&mut future);

    assert!(matches!(
        StdFutureTrait::poll(pinned.as_mut(), &mut cx),
        Poll::Pending
    ));
    assert!(matches!(
        StdFutureTrait::poll(pinned.as_mut(), &mut cx),
        Poll::Pending
    ));

    promise.resolve(7);

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
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(4)));
    let (promise, future) = system.create_promise::<usize>();
    let shared = future.share();
    let callback_count = Arc::new(AtomicUsize::new(0));

    let mut before = Vec::with_capacity(BEFORE);
    for _ in 0..BEFORE {
        let callback_count_clone = Arc::clone(&callback_count);
        before.push(shared.then(orkester::Context::Worker, move |value| {
            callback_count_clone.fetch_add(1, Ordering::SeqCst);
            value
        }));
    }

    promise.resolve(77);

    for continuation in before {
        assert_eq!(continuation.wait().unwrap(), 77);
    }

    let mut after = Vec::with_capacity(AFTER);
    for _ in 0..AFTER {
        let callback_count_clone = Arc::clone(&callback_count);
        after.push(shared.then(orkester::Context::Worker, move |value| {
            callback_count_clone.fetch_add(1, Ordering::SeqCst);
            value
        }));
    }

    for continuation in after {
        assert_eq!(continuation.wait().unwrap(), 77);
    }

    assert_eq!(callback_count.load(Ordering::SeqCst), BEFORE + AFTER);
}

#[test]
fn wait_in_main_thread_handles_large_queue_backlog() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(1)));
    let mut futures = Vec::new();

    for value in 0_i32..128_i32 {
        futures.push(system.run(orkester::Context::Main, move || value));
    }

    let last = futures.pop().unwrap();
    assert_eq!(last.wait_in_main_thread().unwrap(), 127);

    for queued in futures {
        assert!(queued.is_ready());
        assert!(queued.wait().is_ok());
    }
}

#[test]
fn long_worker_then_chain_preserves_value_ordering() {
    const STEPS: usize = 512;
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(4)));
    let mut current = system.create_resolved_future(0_usize);

    for _ in 0..STEPS {
        current = current.then(orkester::Context::Worker, |value| value + 1);
    }

    assert_eq!(current.wait().unwrap(), STEPS);
}

#[test]
fn worker_to_main_to_worker_chain_completes_with_main_pump() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(3)));
    let caller_thread = std::thread::current().id();
    let ran_on_main = Arc::new(AtomicUsize::new(0));
    let ran_on_main_clone = Arc::clone(&ran_on_main);

    let chained = system
        .run(orkester::Context::Worker, || 3_i32)
        .then(orkester::Context::Main, move |value| {
            if std::thread::current().id() == caller_thread {
                ran_on_main_clone.fetch_add(1, Ordering::SeqCst);
            }
            value + 1
        })
        .then(orkester::Context::Worker, |value| value * 2);

    assert_eq!(chained.wait_in_main_thread().unwrap(), 8);
    assert_eq!(ran_on_main.load(Ordering::SeqCst), 1);
}

#[test]
fn rejected_chain_skips_then_and_recovers_in_main_thread() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(3)));
    let caller_thread = std::thread::current().id();
    let then_called = Arc::new(AtomicUsize::new(0));
    let catch_on_main = Arc::new(AtomicUsize::new(0));
    let system_clone = system.clone();

    let failed: orkester::Future<i32> = system.run(orkester::Context::Worker, move || {
        system_clone.create_future(|promise| promise.reject("worker failure"))
    });

    let then_called_clone = Arc::clone(&then_called);
    let catch_on_main_clone = Arc::clone(&catch_on_main);
    let recovered = failed
        .then(orkester::Context::Main, move |value| {
            then_called_clone.fetch_add(1, Ordering::SeqCst);
            value + 1
        })
        .catch(orkester::Context::Main, move |error| {
            assert_eq!(error.to_string(), "worker failure");
            if std::thread::current().id() == caller_thread {
                catch_on_main_clone.fetch_add(1, Ordering::SeqCst);
            }
            42
        });

    assert_eq!(recovered.wait_in_main_thread().unwrap(), 42);
    assert_eq!(then_called.load(Ordering::SeqCst), 0);
    assert_eq!(catch_on_main.load(Ordering::SeqCst), 1);
}

#[test]
fn randomized_repeated_flatten_and_recovery_stress() {
    const ITERS: usize = 96;
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(4)));
    let mut seed = 0xDEC0_DED5_EED5_u64;

    for _ in 0..ITERS {
        let sample = lcg_next(&mut seed);
        let should_fail = (sample & 1) == 1;
        let value = ((sample >> 8) % 1000) as i32;
        let system_clone = system.clone();

        let outcome: orkester::Future<i32> = system.run(orkester::Context::Worker, move || {
            if should_fail {
                system_clone.create_future(|promise| promise.reject("random failure"))
            } else {
                system_clone.create_resolved_future(value)
            }
        });

        let recovered = outcome.catch_immediately(|_| -1);
        let observed = recovered.wait().unwrap();

        if should_fail {
            assert_eq!(observed, -1);
        } else {
            assert_eq!(observed, value);
        }
    }
}

#[test]
fn promise_drop_rejects_paired_future() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(1)));
    let (promise, future) = system.create_promise::<i32>();
    drop(promise);

    let error = future.wait().unwrap_err();
    assert_eq!(error.to_string(), "Promise dropped without resolving");
}

#[test]
fn all_empty_resolves_to_empty_vec() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(1)));
    let joined: orkester::Future<Vec<i32>> = system.all(Vec::<orkester::Future<i32>>::new());
    assert_eq!(joined.wait().unwrap(), Vec::<i32>::new());
}

#[test]
fn all_preserves_input_order_when_resolved_out_of_order() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(2)));
    let (promise0, future0) = system.create_promise::<i32>();
    let (promise1, future1) = system.create_promise::<i32>();
    let (promise2, future2) = system.create_promise::<i32>();

    let joined = system.all(vec![future0, future1, future2]);

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

    assert_eq!(joined.wait().unwrap(), vec![10, 20, 30]);
    handle0.join().unwrap();
    handle1.join().unwrap();
    handle2.join().unwrap();
}

#[test]
fn catch_in_main_thread_is_not_called_on_success() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(1)));
    let catch_called = Arc::new(AtomicUsize::new(0));
    let catch_called_clone = Arc::clone(&catch_called);

    let passthrough = system
        .create_resolved_future(5_i32)
        .catch(orkester::Context::Main, move |_| {
            catch_called_clone.fetch_add(1, Ordering::SeqCst);
            -1
        });

    assert_eq!(passthrough.wait().unwrap(), 5);
    assert_eq!(catch_called.load(Ordering::SeqCst), 0);
}

#[test]
fn catch_in_main_thread_recovers_on_main_when_pumped() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(2)));
    let caller_thread = std::thread::current().id();
    let ran_on_main = Arc::new(AtomicUsize::new(0));
    let ran_on_main_clone = Arc::clone(&ran_on_main);

    let recovered = system
        .create_future(|promise| promise.reject("main recover"))
        .catch(orkester::Context::Main, move |error| {
            assert_eq!(error.to_string(), "main recover");
            if std::thread::current().id() == caller_thread {
                ran_on_main_clone.fetch_add(1, Ordering::SeqCst);
            }
            11
        });

    assert_eq!(recovered.wait_in_main_thread().unwrap(), 11);
    assert_eq!(ran_on_main.load(Ordering::SeqCst), 1);
}

#[test]
fn then_in_thread_pool_runs_inline_on_same_pool_thread() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(2)));
    let pool = system.create_thread_pool(1);

    let same_thread = system
        .run_in_pool(&pool, || std::thread::current().id())
        .then_in_pool(&pool, |source_thread| {
            std::thread::current().id() == source_thread
        });

    assert!(same_thread.wait().unwrap());
}

#[test]
fn then_in_thread_pool_runs_on_target_pool_context() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(2)));
    let pool = system.create_thread_pool(1);

    let pool_thread = system
        .run_in_pool(&pool, || std::thread::current().id())
        .wait()
        .unwrap();

    let observed = system
        .run(orkester::Context::Worker, || 1_i32)
        .then_in_pool(&pool, move |_| std::thread::current().id());

    assert_eq!(observed.wait().unwrap(), pool_thread);
}

#[test]
fn shared_future_poll_wakes_distinct_wakers_once_each() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(1)));
    let (promise, future) = system.create_promise::<i32>();
    let mut shared_a = future.share();
    let mut shared_b = shared_a.clone();

    let wake_count_a = Arc::new(AtomicUsize::new(0));
    let wake_count_b = Arc::new(AtomicUsize::new(0));
    let waker_a = make_counting_waker(Arc::clone(&wake_count_a));
    let waker_b = make_counting_waker(Arc::clone(&wake_count_b));
    let mut cx_a = Context::from_waker(&waker_a);
    let mut cx_b = Context::from_waker(&waker_b);
    let mut pinned_a = Pin::new(&mut shared_a);
    let mut pinned_b = Pin::new(&mut shared_b);

    assert!(matches!(
        StdFutureTrait::poll(pinned_a.as_mut(), &mut cx_a),
        Poll::Pending
    ));
    assert!(matches!(
        StdFutureTrait::poll(pinned_b.as_mut(), &mut cx_b),
        Poll::Pending
    ));

    promise.resolve(55);

    assert_eq!(wake_count_a.load(Ordering::SeqCst), 1);
    assert_eq!(wake_count_b.load(Ordering::SeqCst), 1);
    assert!(matches!(
        StdFutureTrait::poll(pinned_a.as_mut(), &mut cx_a),
        Poll::Ready(Ok(55))
    ));
    assert!(matches!(
        StdFutureTrait::poll(pinned_b.as_mut(), &mut cx_b),
        Poll::Ready(Ok(55))
    ));
}

#[test]
fn future_poll_ready_then_wait_reports_consumed() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(1)));
    let mut future = system.create_resolved_future(9_i32);
    let wake_count = Arc::new(AtomicUsize::new(0));
    let waker = make_counting_waker(Arc::clone(&wake_count));
    let mut cx = Context::from_waker(&waker);
    let mut pinned = Pin::new(&mut future);

    assert!(matches!(
        StdFutureTrait::poll(pinned.as_mut(), &mut cx),
        Poll::Ready(Ok(9))
    ));
    drop(pinned);

    let error = future.wait().unwrap_err();
    assert_eq!(wake_count.load(Ordering::SeqCst), 0);
    assert_eq!(error.to_string(), "Future already consumed");
}

#[test]
fn dispatch_one_main_thread_task_reports_queue_progress() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(1)));
    let futures: Vec<_> = (0_i32..3_i32)
        .map(|value| system.run(orkester::Context::Main, move || value))
        .collect();

    assert!(system.has_pending_main_thread_tasks());
    assert!(system.dispatch_one_main_thread_task());
    assert!(system.has_pending_main_thread_tasks());
    assert!(system.dispatch_one_main_thread_task());
    assert!(system.has_pending_main_thread_tasks());
    assert!(system.dispatch_one_main_thread_task());
    assert!(!system.dispatch_one_main_thread_task());
    assert!(!system.has_pending_main_thread_tasks());

    for (idx, future) in futures.into_iter().enumerate() {
        assert_eq!(future.wait().unwrap(), idx as i32);
    }
}

#[test]
fn repeated_shared_future_fanout_stress() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(4)));
    run_shared_fanout_stress(&system, 32, 8);
}

#[test]
fn randomized_cross_context_then_catch_roundtrip_stress() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(4)));
    run_cross_context_roundtrip_stress(&system, 128, 0xA11C_EB0B_1357_2468_u64);
}

#[test]
#[ignore]
fn soak_randomized_cross_context_then_catch_roundtrip() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(4)));
    run_cross_context_roundtrip_stress(&system, 512, 0xFACE_FEED_BADC_0FFE_u64);
}

#[test]
#[ignore]
fn soak_randomized_cross_context_then_catch_roundtrip_alt_seed_a() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(4)));
    run_cross_context_roundtrip_stress(&system, 1024, 0x0123_4567_89AB_CDEF_u64);
}

#[test]
#[ignore]
fn soak_randomized_cross_context_then_catch_roundtrip_alt_seed_b() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(4)));
    run_cross_context_roundtrip_stress(&system, 2048, 0x0F0F_F0F0_AAAA_5555_u64);
}

#[test]
#[ignore]
fn soak_shared_future_fanout_high_contention() {
    let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(6)));
    run_shared_fanout_stress(&system, 96, 16);
}
