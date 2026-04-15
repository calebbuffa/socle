use std::time::Duration;

use orkester::{self, Context, Task};

#[derive(Clone)]
pub struct AsyncRuntime {
    thread_pool: orkester::ThreadPool,
    work_queue: orkester::WorkQueue,
}

impl AsyncRuntime {
    pub fn new(threads: usize) -> Self {
        let thread_pool = orkester::ThreadPool::new(threads);
        let work_queue = orkester::WorkQueue::new();
        Self {
            thread_pool,
            work_queue,
        }
    }

    pub fn background(&self) -> Context {
        self.thread_pool.context()
    }

    pub fn main(&self) -> Context {
        self.work_queue.context()
    }

    pub fn flush_main(&mut self) -> usize {
        self.work_queue.flush()
    }

    pub fn pump_main(&mut self) -> bool {
        self.work_queue.pump()
    }

    /// Execute pending main-thread tasks until the queue is empty or `budget`
    /// has elapsed. Returns the number of tasks executed.
    pub fn flush_timed(&mut self, budget: Duration) -> usize {
        self.work_queue.flush_timed(budget)
    }

    pub fn start_task<T: Send + 'static>(
        &self,
        task: impl FnOnce() -> T + Send + 'static,
    ) -> Task<T> {
        let bg_ctx = self.background();
        bg_ctx.run(task)
    }
}
