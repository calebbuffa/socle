use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::executor::Executor;

type Task = Box<dyn FnOnce() + Send + 'static>;

thread_local! {
    static CURRENT_POOL_ID: Cell<Option<u64>> = const { Cell::new(None) };
}

static NEXT_POOL_ID: AtomicU64 = AtomicU64::new(1);

struct PoolInner {
    id: u64,
    sender: mpsc::Sender<Task>,
    _workers: Vec<thread::JoinHandle<()>>,
}

/// A dedicated thread-pool handle for pinning work to a fixed set of threads.
#[derive(Clone)]
pub struct ThreadPool {
    inner: Arc<PoolInner>,
}

impl Executor for ThreadPool {
    fn execute(&self, task: Box<dyn FnOnce() + Send + 'static>) {
        self.schedule(task);
    }

    fn is_current(&self) -> bool {
        CURRENT_POOL_ID.with(|slot| slot.get() == Some(self.inner.id))
    }
}

impl ThreadPool {
    pub fn new(number_of_threads: usize) -> Self {
        let number_of_threads = number_of_threads.max(1);
        let id = NEXT_POOL_ID.fetch_add(1, Ordering::Relaxed);

        let (sender, receiver) = mpsc::channel::<Task>();
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(number_of_threads);
        for index in 0..number_of_threads {
            let rx = Arc::clone(&receiver);
            let thread_name = format!("orkester-pool-{id}-worker-{index}");
            let handle = thread::Builder::new()
                .name(thread_name)
                .spawn(move || {
                    CURRENT_POOL_ID.with(|slot| slot.set(Some(id)));
                    loop {
                        let task = {
                            let lock = rx.lock().expect("thread pool receiver lock poisoned");
                            lock.recv()
                        };
                        match task {
                            Ok(task) => task(),
                            Err(_) => break,
                        }
                    }
                    CURRENT_POOL_ID.with(|slot| slot.set(None));
                })
                .expect("failed to spawn thread-pool worker");
            workers.push(handle);
        }

        Self {
            inner: Arc::new(PoolInner {
                id,
                sender,
                _workers: workers,
            }),
        }
    }

    fn schedule(&self, task: Task) {
        let _ = self.inner.sender.send(task);
    }

    /// Return a [`Context`](crate::Context) that routes tasks into this pool.
    pub fn context(&self) -> crate::context::Context {
        crate::context::Context::new(self.clone())
    }
}

impl Default for ThreadPool {
    fn default() -> Self {
        let cpus = thread::available_parallelism()
            .map(|v| v.get())
            .unwrap_or(4);
        Self::new((cpus.saturating_sub(1)).max(1))
    }
}
