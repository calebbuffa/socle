use orkester::Scheduler;
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

fn run_cross_context_roundtrip_stress(system: &Scheduler, iterations: usize, mut seed: u64) {
    for _ in 0..iterations {
        let sample = lcg_next(&mut seed);
        let should_fail = (sample & 1) == 1;
        let value = ((sample >> 16) % 1000) as i32;

        let base: orkester::Task<i32> = if should_fail {
            system.task(|resolver| resolver.reject("seeded failure"))
        } else {
            system.resolved(value)
        };

        let chain = base
            .then(orkester::Context::BACKGROUND, |v| v + 2)
            .then(orkester::Context::MAIN, |v| v * 2)
            .catch(orkester::Context::MAIN, |_| -5)
            .then(orkester::Context::BACKGROUND, |v| v - 1);

        let observed = chain.block_with_main().unwrap();
        let expected = if should_fail {
            -6
        } else {
            ((value + 2) * 2) - 1
        };
        assert_eq!(observed, expected);
    }
}

fn run_shared_task_fanout_stress(system: &Scheduler, iterations: usize, waiters: usize) {
    for iteration in 0..iterations {
        let (resolver, task) = system.resolver::<usize>();
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
fn create_resolver_pair_resolves() {
    let system = Scheduler::with_threads(1);
    let (resolver, task) = system.resolver();
    resolver.resolve(42_i32);
    assert_eq!(task.block().unwrap(), 42);
}

#[test]
fn run_in_main_thread_is_inline_inside_scope() {
    let system = Scheduler::with_threads(1);
    let _scope = system.main_scope();
    let task = system.run(orkester::Context::MAIN, || 7_i32);
    assert!(task.is_ready());
    assert_eq!(task.block().unwrap(), 7);
}

#[test]
fn block_with_main_pumps_queue() {
    let system = Scheduler::with_threads(1);
    let task = system.run(orkester::Context::MAIN, || 9_i32);
    assert_eq!(task.block_with_main().unwrap(), 9);
}

#[test]
fn shared_task_then_chain() {
    let system = Scheduler::with_threads(1);
    let shared = system.resolved(10_i32).share();
    let doubled = shared.then(orkester::Context::BACKGROUND, |value| value * 2);

    assert_eq!(doubled.block().unwrap(), 20);
    assert_eq!(shared.block().unwrap(), 10);
}

#[test]
fn all_task_values() {
    let system = Scheduler::with_threads(2);
    let futures = vec![
        system.resolved(1_i32),
        system.resolved(2_i32),
        system.resolved(3_i32),
    ];

    let joined = system.join_all(futures);
    assert_eq!(joined.block().unwrap(), vec![1, 2, 3]);
}

#[test]
fn run_in_worker_thread_flattens_task_result() {
    let system = Scheduler::with_threads(2);
    let system_clone = system.clone();

    let flattened: orkester::Task<i32> = system.run(orkester::Context::BACKGROUND, move || {
        system_clone.resolved(21_i32)
    });
    assert_eq!(flattened.block().unwrap(), 21);
}

#[test]
fn then_in_worker_thread_flattens_task_result() {
    let system = Scheduler::with_threads(2);
    let system_clone = system.clone();

    let flattened: orkester::Task<i32> = system
        .resolved(5_i32)
        .then(orkester::Context::BACKGROUND, move |value| {
            system_clone.run(orkester::Context::BACKGROUND, move || value * 3)
        });

    assert_eq!(flattened.block().unwrap(), 15);
}

#[test]
fn run_in_main_thread_flattens_task_result() {
    let system = Scheduler::with_threads(1);
    let _scope = system.main_scope();
    let system_clone = system.clone();

    let flattened: orkester::Task<i32> = system.run(orkester::Context::MAIN, move || {
        system_clone.resolved(33_i32)
    });
    assert!(flattened.is_ready());
    assert_eq!(flattened.block().unwrap(), 33);
}

#[test]
fn then_in_worker_thread_flattens_rejected_task_result() {
    let system = Scheduler::with_threads(2);
    let system_clone = system.clone();

    let flattened: orkester::Task<i32> = system
        .resolved(1_i32)
        .then(orkester::Context::BACKGROUND, move |_| {
            system_clone.task(|resolver| resolver.reject("boom"))
        });

    let error = flattened.block().unwrap_err();
    assert_eq!(error.to_string(), "boom");
}

#[test]
fn map_runs_inline_for_resolved_task() {
    let system = Scheduler::with_threads(1);
    let caller_thread = std::thread::current().id();

    let same_thread = system
        .resolved(1_i32)
        .map(move |_| std::thread::current().id() == caller_thread);

    assert!(same_thread.block().unwrap());
}

#[test]
fn all_accepts_shared_tasks_via_map() {
    let system = Scheduler::with_threads(1);
    let shared = system.resolved(4_i32).share();

    let joined = system.join_all(vec![shared.map(|v| v), shared.map(|v| v)]);
    assert_eq!(joined.block().unwrap(), vec![4, 4]);
}

#[test]
fn all_rejects_when_any_input_rejects() {
    let system = Scheduler::with_threads(2);
    let futures = vec![
        system.resolved(1_i32),
        system.task(|resolver| resolver.reject("join failed")),
        system.resolved(3_i32),
    ];

    let joined = system.join_all(futures);
    let error = joined.block().unwrap_err();
    assert_eq!(error.to_string(), "join failed");
}

#[test]
fn shared_task_wait_is_consistent_for_concurrent_waiters() {
    const WAITERS: usize = 24;
    let system = Scheduler::with_threads(4);
    let (resolver, task) = system.resolver::<usize>();
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
fn task_poll_deduplicates_same_waker_registration() {
    let system = Scheduler::with_threads(1);
    let (resolver, mut task) = system.resolver::<i32>();
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
fn shared_task_continuations_before_and_after_resolution_run_once() {
    const BEFORE: usize = 32;
    const AFTER: usize = 32;
    let system = Scheduler::with_threads(4);
    let (resolver, task) = system.resolver::<usize>();
    let shared = task.share();
    let callback_count = Arc::new(AtomicUsize::new(0));

    let mut before = Vec::with_capacity(BEFORE);
    for _ in 0..BEFORE {
        let callback_count_clone = Arc::clone(&callback_count);
        before.push(shared.then(orkester::Context::BACKGROUND, move |value| {
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
        after.push(shared.then(orkester::Context::BACKGROUND, move |value| {
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
    let system = Scheduler::with_threads(1);
    let mut futures = Vec::new();

    for value in 0_i32..128_i32 {
        futures.push(system.run(orkester::Context::MAIN, move || value));
    }

    let last = futures.pop().unwrap();
    assert_eq!(last.block_with_main().unwrap(), 127);

    for queued in futures {
        assert!(queued.is_ready());
        assert!(queued.block().is_ok());
    }
}

#[test]
fn long_worker_then_chain_preserves_value_ordering() {
    const STEPS: usize = 512;
    let system = Scheduler::with_threads(4);
    let mut current = system.resolved(0_usize);

    for _ in 0..STEPS {
        current = current.then(orkester::Context::BACKGROUND, |value| value + 1);
    }

    assert_eq!(current.block().unwrap(), STEPS);
}

#[test]
fn worker_to_main_to_worker_chain_completes_with_main_pump() {
    let system = Scheduler::with_threads(3);
    let caller_thread = std::thread::current().id();
    let ran_on_main = Arc::new(AtomicUsize::new(0));
    let ran_on_main_clone = Arc::clone(&ran_on_main);

    let chained = system
        .run(orkester::Context::BACKGROUND, || 3_i32)
        .then(orkester::Context::MAIN, move |value| {
            if std::thread::current().id() == caller_thread {
                ran_on_main_clone.fetch_add(1, Ordering::SeqCst);
            }
            value + 1
        })
        .then(orkester::Context::BACKGROUND, |value| value * 2);

    assert_eq!(chained.block_with_main().unwrap(), 8);
    assert_eq!(ran_on_main.load(Ordering::SeqCst), 1);
}

#[test]
fn rejected_chain_skips_then_and_recovers_in_main_thread() {
    let system = Scheduler::with_threads(3);
    let caller_thread = std::thread::current().id();
    let then_called = Arc::new(AtomicUsize::new(0));
    let catch_on_main = Arc::new(AtomicUsize::new(0));
    let system_clone = system.clone();

    let failed: orkester::Task<i32> = system.run(orkester::Context::BACKGROUND, move || {
        system_clone.task(|resolver| resolver.reject("worker failure"))
    });

    let then_called_clone = Arc::clone(&then_called);
    let catch_on_main_clone = Arc::clone(&catch_on_main);
    let recovered = failed
        .then(orkester::Context::MAIN, move |value| {
            then_called_clone.fetch_add(1, Ordering::SeqCst);
            value + 1
        })
        .catch(orkester::Context::MAIN, move |error| {
            assert_eq!(error.to_string(), "worker failure");
            if std::thread::current().id() == caller_thread {
                catch_on_main_clone.fetch_add(1, Ordering::SeqCst);
            }
            42
        });

    assert_eq!(recovered.block_with_main().unwrap(), 42);
    assert_eq!(then_called.load(Ordering::SeqCst), 0);
    assert_eq!(catch_on_main.load(Ordering::SeqCst), 1);
}

#[test]
fn randomized_repeated_flatten_and_recovery_stress() {
    const ITERS: usize = 96;
    let system = Scheduler::with_threads(4);
    let mut seed = 0xDEC0_DED5_EED5_u64;

    for _ in 0..ITERS {
        let sample = lcg_next(&mut seed);
        let should_fail = (sample & 1) == 1;
        let value = ((sample >> 8) % 1000) as i32;
        let system_clone = system.clone();

        let outcome: orkester::Task<i32> = system.run(orkester::Context::BACKGROUND, move || {
            if should_fail {
                system_clone.task(|resolver| resolver.reject("random failure"))
            } else {
                system_clone.resolved(value)
            }
        });

        let recovered = outcome.or_else(|_| -1);
        let observed = recovered.block().unwrap();

        if should_fail {
            assert_eq!(observed, -1);
        } else {
            assert_eq!(observed, value);
        }
    }
}

#[test]
fn resolver_drop_rejects_paired_task() {
    let system = Scheduler::with_threads(1);
    let (resolver, task) = system.resolver::<i32>();
    drop(resolver);

    let error = task.block().unwrap_err();
    assert_eq!(error.to_string(), "Resolver dropped without resolving");
}

#[test]
fn all_empty_resolves_to_empty_vec() {
    let system = Scheduler::with_threads(1);
    let joined: orkester::Task<Vec<i32>> = system.join_all(Vec::<orkester::Task<i32>>::new());
    assert_eq!(joined.block().unwrap(), Vec::<i32>::new());
}

#[test]
fn all_preserves_input_order_when_resolved_out_of_order() {
    let system = Scheduler::with_threads(2);
    let (resolver0, task0) = system.resolver::<i32>();
    let (resolver1, task1) = system.resolver::<i32>();
    let (resolver2, task2) = system.resolver::<i32>();

    let joined = system.join_all(vec![task0, task1, task2]);

    let handle2 = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(2));
        resolver2.resolve(30);
    });
    let handle0 = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(4));
        resolver0.resolve(10);
    });
    let handle1 = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(8));
        resolver1.resolve(20);
    });

    assert_eq!(joined.block().unwrap(), vec![10, 20, 30]);
    handle0.join().unwrap();
    handle1.join().unwrap();
    handle2.join().unwrap();
}

#[test]
fn catch_in_main_thread_is_not_called_on_success() {
    let system = Scheduler::with_threads(1);
    let catch_called = Arc::new(AtomicUsize::new(0));
    let catch_called_clone = Arc::clone(&catch_called);

    let passthrough = system
        .resolved(5_i32)
        .catch(orkester::Context::MAIN, move |_| {
            catch_called_clone.fetch_add(1, Ordering::SeqCst);
            -1
        });

    assert_eq!(passthrough.block().unwrap(), 5);
    assert_eq!(catch_called.load(Ordering::SeqCst), 0);
}

#[test]
fn catch_in_main_thread_recovers_on_main_when_pumped() {
    let system = Scheduler::with_threads(2);
    let caller_thread = std::thread::current().id();
    let ran_on_main = Arc::new(AtomicUsize::new(0));
    let ran_on_main_clone = Arc::clone(&ran_on_main);

    let recovered = system
        .task(|resolver| resolver.reject("main recover"))
        .catch(orkester::Context::MAIN, move |error| {
            assert_eq!(error.to_string(), "main recover");
            if std::thread::current().id() == caller_thread {
                ran_on_main_clone.fetch_add(1, Ordering::SeqCst);
            }
            11
        });

    assert_eq!(recovered.block_with_main().unwrap(), 11);
    assert_eq!(ran_on_main.load(Ordering::SeqCst), 1);
}

#[test]
fn then_in_thread_pool_runs_inline_on_same_pool_thread() {
    let system = Scheduler::with_threads(2);
    let pool = system.thread_pool(1);

    let same_thread = system
        .run_in_pool(&pool, || std::thread::current().id())
        .then_in_pool(&pool, |source_thread| {
            std::thread::current().id() == source_thread
        });

    assert!(same_thread.block().unwrap());
}

#[test]
fn then_in_thread_pool_runs_on_target_pool_context() {
    let system = Scheduler::with_threads(2);
    let pool = system.thread_pool(1);

    let pool_thread = system
        .run_in_pool(&pool, || std::thread::current().id())
        .block()
        .unwrap();

    let observed = system
        .run(orkester::Context::BACKGROUND, || 1_i32)
        .then_in_pool(&pool, move |_| std::thread::current().id());

    assert_eq!(observed.block().unwrap(), pool_thread);
}

#[test]
fn shared_task_poll_wakes_distinct_wakers_once_each() {
    let system = Scheduler::with_threads(1);
    let (resolver, task) = system.resolver::<i32>();
    let mut shared_a = task.share();
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

    resolver.resolve(55);

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
fn task_poll_ready_then_wait_reports_consumed() {
    let system = Scheduler::with_threads(1);
    let mut task = system.resolved(9_i32);
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
    let system = Scheduler::with_threads(1);
    let futures: Vec<_> = (0_i32..3_i32)
        .map(|value| system.run(orkester::Context::MAIN, move || value))
        .collect();

    assert!(system.main_pending());
    assert!(system.flush_main_one());
    assert!(system.main_pending());
    assert!(system.flush_main_one());
    assert!(system.main_pending());
    assert!(system.flush_main_one());
    assert!(!system.flush_main_one());
    assert!(!system.main_pending());

    for (idx, task) in futures.into_iter().enumerate() {
        assert_eq!(task.block().unwrap(), idx as i32);
    }
}

#[test]
fn repeated_shared_task_fanout_stress() {
    let system = Scheduler::with_threads(4);
    run_shared_task_fanout_stress(&system, 32, 8);
}

#[test]
fn randomized_cross_context_then_catch_roundtrip_stress() {
    let system = Scheduler::with_threads(4);
    run_cross_context_roundtrip_stress(&system, 128, 0xA11C_EB0B_1357_2468_u64);
}

#[test]
#[ignore]
fn soak_randomized_cross_context_then_catch_roundtrip() {
    let system = Scheduler::with_threads(4);
    run_cross_context_roundtrip_stress(&system, 512, 0xFACE_FEED_BADC_0FFE_u64);
}

#[test]
#[ignore]
fn soak_randomized_cross_context_then_catch_roundtrip_alt_seed_a() {
    let system = Scheduler::with_threads(4);
    run_cross_context_roundtrip_stress(&system, 1024, 0x0123_4567_89AB_CDEF_u64);
}

#[test]
#[ignore]
fn soak_randomized_cross_context_then_catch_roundtrip_alt_seed_b() {
    let system = Scheduler::with_threads(4);
    run_cross_context_roundtrip_stress(&system, 2048, 0x0F0F_F0F0_AAAA_5555_u64);
}

#[test]
#[ignore]
fn soak_shared_task_fanout_high_contention() {
    let system = Scheduler::with_threads(6);
    run_shared_task_fanout_stress(&system, 96, 16);
}
