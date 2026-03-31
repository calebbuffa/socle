use orkester::{mpsc, oneshot};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn send_recv_basic() {
    let (tx, rx) = mpsc(4);
    tx.send(1).unwrap();
    tx.send(2).unwrap();
    assert_eq!(rx.recv(), Some(1));
    assert_eq!(rx.recv(), Some(2));
}

#[test]
fn recv_returns_none_when_all_senders_dropped() {
    let (tx, rx) = mpsc::<i32>(4);
    drop(tx);
    assert_eq!(rx.recv(), None);
}

#[test]
fn send_fails_when_receiver_dropped() {
    let (tx, rx) = mpsc(4);
    drop(rx);
    assert!(tx.send(1).is_err());
}

#[test]
fn bounded_blocks_when_full() {
    let (tx, rx) = mpsc(2);
    tx.send(1).unwrap();
    tx.send(2).unwrap();

    let tx2 = tx.clone();
    let handle = thread::spawn(move || {
        tx2.send(3).unwrap(); // should block until rx.recv()
    });

    thread::sleep(Duration::from_millis(20));
    assert_eq!(rx.recv(), Some(1));
    handle.join().unwrap();
    assert_eq!(rx.recv(), Some(2));
    assert_eq!(rx.recv(), Some(3));
}

#[test]
fn multiple_producers() {
    let (tx, rx) = mpsc(16);
    let mut handles = Vec::new();
    for i in 0..4 {
        let tx = tx.clone();
        handles.push(thread::spawn(move || {
            for j in 0..4 {
                tx.send(i * 10 + j).unwrap();
            }
        }));
    }
    drop(tx);
    for h in handles {
        h.join().unwrap();
    }
    let mut values = Vec::new();
    while let Some(v) = rx.recv() {
        values.push(v);
    }
    values.sort();
    assert_eq!(
        values,
        vec![0, 1, 2, 3, 10, 11, 12, 13, 20, 21, 22, 23, 30, 31, 32, 33]
    );
}

#[test]
fn oneshot_works() {
    let (tx, rx) = oneshot();
    tx.send(42).unwrap();
    assert_eq!(rx.recv(), Some(42));
}

#[test]
fn try_send_full_returns_full() {
    let (tx, rx) = mpsc(1);
    tx.try_send(1).unwrap();
    let err = tx.try_send(2).unwrap_err();
    assert!(err.is_full());
    assert_eq!(err.into_inner(), 2);
    assert_eq!(rx.recv(), Some(1));
}

#[test]
fn try_send_closed_returns_closed() {
    let (tx, rx) = mpsc(4);
    drop(rx);
    let err = tx.try_send(1).unwrap_err();
    assert!(err.is_closed());
    assert_eq!(err.into_inner(), 1);
}

#[test]
fn send_timeout_succeeds_when_space_available() {
    let (tx, rx) = mpsc(2);
    tx.send_timeout(1, Duration::from_millis(100)).unwrap();
    tx.send_timeout(2, Duration::from_millis(100)).unwrap();
    assert_eq!(rx.recv(), Some(1));
    assert_eq!(rx.recv(), Some(2));
}

#[test]
fn send_timeout_returns_full_on_expiry() {
    let (tx, _rx) = mpsc(1);
    tx.send(1).unwrap();
    let start = Instant::now();
    let err = tx.send_timeout(2, Duration::from_millis(50)).unwrap_err();
    assert!(err.is_full());
    assert!(start.elapsed() >= Duration::from_millis(40));
}

#[test]
fn send_timeout_unblocks_when_consumer_drains() {
    let (tx, rx) = mpsc(1);
    tx.send(1).unwrap(); // channel full

    let tx2 = tx.clone();
    let handle = thread::spawn(move || {
        tx2.send_timeout(2, Duration::from_secs(2)).unwrap();
    });

    thread::sleep(Duration::from_millis(30));
    assert_eq!(rx.recv(), Some(1)); // frees a slot
    handle.join().unwrap();
    assert_eq!(rx.recv(), Some(2));
}

#[test]
fn recv_timeout_returns_none_on_expiry() {
    let (_tx, rx) = mpsc::<i32>(4);
    let start = Instant::now();
    assert_eq!(rx.recv_timeout(Duration::from_millis(50)), None);
    assert!(start.elapsed() >= Duration::from_millis(40));
}

#[test]
fn recv_timeout_returns_value_before_deadline() {
    let (tx, rx) = mpsc(4);
    let handle = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        tx.send(99).unwrap();
    });
    assert_eq!(rx.recv_timeout(Duration::from_secs(2)), Some(99));
    handle.join().unwrap();
}

#[test]
fn recv_timeout_returns_none_when_closed() {
    let (tx, rx) = mpsc::<i32>(4);
    drop(tx);
    assert_eq!(rx.recv_timeout(Duration::from_millis(100)), None);
}

#[test]
fn producers_exceed_capacity_no_deadlock() {
    // 10 producers sending to a capacity-2 channel.
    // Consumer runs concurrently — the correct pattern.
    let (tx, rx) = mpsc(2);

    for i in 0..10 {
        let tx = tx.clone();
        thread::spawn(move || {
            tx.send(i).unwrap();
        });
    }
    drop(tx);

    let mut values = Vec::new();
    while let Some(v) = rx.recv() {
        values.push(v);
    }
    values.sort();
    assert_eq!(values, (0..10).collect::<Vec<_>>());
}
