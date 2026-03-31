use orkester::Semaphore;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn basic_acquire_release() {
    let sem = Semaphore::new(2);
    assert_eq!(sem.available_permits(), 2);

    let p1 = sem.acquire();
    assert_eq!(sem.available_permits(), 1);

    let p2 = sem.acquire();
    assert_eq!(sem.available_permits(), 0);

    drop(p1);
    assert_eq!(sem.available_permits(), 1);

    drop(p2);
    assert_eq!(sem.available_permits(), 2);
}

#[test]
fn try_acquire_succeeds_and_fails() {
    let sem = Semaphore::new(1);

    let p = sem.try_acquire();
    assert!(p.is_some());
    assert_eq!(sem.available_permits(), 0);

    assert!(sem.try_acquire().is_none());

    drop(p);
    assert_eq!(sem.available_permits(), 1);
}

#[test]
fn acquire_blocks_until_release() {
    let sem = Semaphore::new(1);
    let counter = Arc::new(AtomicUsize::new(0));

    let p = sem.acquire();
    let sem2 = sem.clone();
    let c2 = counter.clone();

    let handle = std::thread::spawn(move || {
        let _p2 = sem2.acquire(); // should block
        c2.fetch_add(1, Ordering::SeqCst);
    });

    std::thread::sleep(std::time::Duration::from_millis(50));
    assert_eq!(counter.load(Ordering::SeqCst), 0);

    drop(p);
    handle.join().unwrap();
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn concurrent_semaphore_limits_parallelism() {
    let sem = Semaphore::new(3);
    let active = Arc::new(AtomicUsize::new(0));
    let max_active = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();
    for _ in 0..10 {
        let sem = sem.clone();
        let active = active.clone();
        let max_active = max_active.clone();
        handles.push(std::thread::spawn(move || {
            let _permit = sem.acquire();
            let current = active.fetch_add(1, Ordering::SeqCst) + 1;
            max_active.fetch_max(current, Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(20));
            active.fetch_sub(1, Ordering::SeqCst);
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    assert!(max_active.load(Ordering::SeqCst) <= 3);
}

#[test]
#[should_panic(expected = "semaphore requires at least 1 permit")]
fn zero_permits_panics() {
    let _sem = Semaphore::new(0);
}
