//! Task processor trait and default thread-pool implementation.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

type Task = Box<dyn FnOnce() + Send + 'static>;

/// Dispatches work to background threads.
pub(crate) trait TaskProcessor: Send + Sync {
    fn start_task(&self, task: Task);
}

/// Thread-pool [`TaskProcessor`] backed by `std::thread`.
pub(crate) struct ThreadPoolTaskProcessor {
    sender: mpsc::Sender<Task>,
    _workers: Vec<thread::JoinHandle<()>>,
}

impl ThreadPoolTaskProcessor {
    pub fn new(number_of_threads: usize) -> Self {
        let number_of_threads = number_of_threads.max(1);
        let (sender, receiver) = mpsc::channel::<Task>();
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(number_of_threads);
        for index in 0..number_of_threads {
            let rx = Arc::clone(&receiver);
            let handle = thread::Builder::new()
                .name(format!("orkester-worker-{index}"))
                .spawn(move || {
                    loop {
                        let task = {
                            let lock = rx.lock().expect("task receiver lock poisoned");
                            lock.recv()
                        };
                        match task {
                            Ok(task) => task(),
                            Err(_) => break,
                        }
                    }
                })
                .expect("failed to spawn worker thread");
            workers.push(handle);
        }

        Self {
            sender,
            _workers: workers,
        }
    }

    /// Build a pool from available parallelism.
    pub fn default_pool() -> Self {
        let cpus = thread::available_parallelism()
            .map(|v| v.get())
            .unwrap_or(4);
        Self::new((cpus.saturating_sub(1)).max(1))
    }
}

impl TaskProcessor for ThreadPoolTaskProcessor {
    fn start_task(&self, task: Task) {
        let _ = self.sender.send(task);
    }
}
