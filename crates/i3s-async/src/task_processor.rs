//! Task processor trait and default thread-pool implementation.

use std::sync::mpsc;
use std::thread;

/// Dispatches work to background threads.
pub trait TaskProcessor: Send + Sync {
    fn start_task(&self, task: Box<dyn FnOnce() + Send>);
}

/// Thread-pool [`TaskProcessor`] backed by `std::thread`.
pub struct ThreadPoolTaskProcessor {
    sender: mpsc::Sender<Box<dyn FnOnce() + Send>>,
    _workers: Vec<thread::JoinHandle<()>>,
}

impl ThreadPoolTaskProcessor {
    pub fn new(num_threads: usize) -> Self {
        let num_threads = num_threads.max(1);
        let (sender, receiver) = mpsc::channel::<Box<dyn FnOnce() + Send>>();
        let receiver = std::sync::Arc::new(std::sync::Mutex::new(receiver));

        let mut workers = Vec::with_capacity(num_threads);
        for i in 0..num_threads {
            let rx = std::sync::Arc::clone(&receiver);
            let handle = thread::Builder::new()
                .name(format!("i3s-worker-{i}"))
                .spawn(move || {
                    loop {
                        let task = {
                            let lock = rx.lock().expect("task receiver lock poisoned");
                            lock.recv()
                        };
                        match task {
                            Ok(f) => f(),
                            Err(_) => break, // channel closed, exit
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

    /// Create a thread pool with a worker count based on available parallelism.
    ///
    /// Uses `available_parallelism() - 1` (minimum 1) to leave the main thread free.
    pub fn default_pool() -> Self {
        let cpus = thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        Self::new((cpus.saturating_sub(1)).max(1))
    }
}

impl TaskProcessor for ThreadPoolTaskProcessor {
    fn start_task(&self, task: Box<dyn FnOnce() + Send>) {
        let _ = self.sender.send(task);
    }
}
