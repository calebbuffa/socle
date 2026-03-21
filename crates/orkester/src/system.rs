use crate::context::Context;
use crate::error::AsyncError;
use crate::future::{Future, ResolveOutput, SharedFuture, create_pair};
use crate::main_thread::{MainThreadQueue, MainThreadScope, is_main_thread};
use crate::promise::Promise;
use crate::task_processor::TaskProcessor;
use crate::thread_pool::ThreadPool;
use std::cell::Cell;
use std::fmt::{self, Debug, Formatter};
use std::sync::Arc;

type Task = Box<dyn FnOnce() + Send + 'static>;

thread_local! {
    static WORKER_THREAD_DEPTH: Cell<u32> = const { Cell::new(0) };
}

fn is_worker_thread() -> bool {
    WORKER_THREAD_DEPTH.with(|depth| depth.get() > 0)
}

struct WorkerThreadScope;

impl WorkerThreadScope {
    fn enter() -> Self {
        WORKER_THREAD_DEPTH.with(|depth| depth.set(depth.get().saturating_add(1)));
        Self
    }
}

impl Drop for WorkerThreadScope {
    fn drop(&mut self) {
        WORKER_THREAD_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

pub(crate) trait Scheduler: Send + Sync {
    fn schedule(&self, task: Task);
    fn is_current_thread(&self) -> bool;
}

pub(crate) type SchedulerHandle = Arc<dyn Scheduler>;

struct WorkerScheduler {
    task_processor: Arc<dyn TaskProcessor>,
}

impl Scheduler for WorkerScheduler {
    fn schedule(&self, task: Task) {
        let task_processor = Arc::clone(&self.task_processor);
        task_processor.start_task(Box::new(move || {
            let _scope = WorkerThreadScope::enter();
            task();
        }));
    }

    fn is_current_thread(&self) -> bool {
        is_worker_thread()
    }
}

struct MainThreadScheduler {
    queue: Arc<MainThreadQueue>,
}

impl Scheduler for MainThreadScheduler {
    fn schedule(&self, task: Task) {
        self.queue.enqueue(task);
    }

    fn is_current_thread(&self) -> bool {
        is_main_thread()
    }
}

struct ThreadPoolScheduler {
    thread_pool: ThreadPool,
}

impl Scheduler for ThreadPoolScheduler {
    fn schedule(&self, task: Task) {
        self.thread_pool.schedule(task);
    }

    fn is_current_thread(&self) -> bool {
        self.thread_pool.is_current_thread()
    }
}

pub(crate) struct SystemImpl {
    pub(crate) main_queue: Arc<MainThreadQueue>,
    worker_scheduler: SchedulerHandle,
    main_thread_scheduler: SchedulerHandle,
}

impl SystemImpl {
    pub(crate) fn worker_scheduler(&self) -> SchedulerHandle {
        Arc::clone(&self.worker_scheduler)
    }

    pub(crate) fn main_thread_scheduler(&self) -> SchedulerHandle {
        Arc::clone(&self.main_thread_scheduler)
    }

    pub(crate) fn thread_pool_scheduler(&self, thread_pool: &ThreadPool) -> SchedulerHandle {
        Arc::new(ThreadPoolScheduler {
            thread_pool: thread_pool.clone(),
        })
    }

    /// Return the scheduler for a given [`Context`].
    ///
    /// Returns `None` for [`Context::Immediate`] — callers must handle
    /// immediate execution inline.
    pub(crate) fn scheduler_for(&self, context: Context) -> Option<SchedulerHandle> {
        match context {
            Context::Worker => Some(self.worker_scheduler()),
            Context::Main => Some(self.main_thread_scheduler()),
            Context::Immediate => None,
        }
    }
}

/// Root async runtime object. Cheap to clone.
#[derive(Clone)]
pub struct AsyncSystem {
    pub(crate) inner: Arc<SystemImpl>,
}

impl PartialEq for AsyncSystem {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for AsyncSystem {}

impl Debug for AsyncSystem {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("AsyncSystem")
            .field("inner", &(Arc::as_ptr(&self.inner) as usize))
            .finish()
    }
}

impl AsyncSystem {
    pub fn new(task_processor: Arc<dyn TaskProcessor>) -> Self {
        let main_queue = Arc::new(MainThreadQueue::new());
        let worker_scheduler: SchedulerHandle = Arc::new(WorkerScheduler { task_processor });
        let main_thread_scheduler: SchedulerHandle = Arc::new(MainThreadScheduler {
            queue: Arc::clone(&main_queue),
        });

        Self {
            inner: Arc::new(SystemImpl {
                main_queue,
                worker_scheduler,
                main_thread_scheduler,
            }),
        }
    }

    pub fn create_thread_pool(&self, number_of_threads: usize) -> ThreadPool {
        ThreadPool::new(number_of_threads)
    }

    pub fn create_promise<T: Send + 'static>(&self) -> (Promise<T>, Future<T>) {
        create_pair(self.clone())
    }

    pub fn create_future<T, F>(&self, f: F) -> Future<T>
    where
        T: Send + 'static,
        F: FnOnce(Promise<T>) + Send + 'static,
    {
        let (promise, future) = self.create_promise();
        f(promise);
        future
    }

    pub fn create_resolved_future<T: Send + 'static>(&self, value: T) -> Future<T> {
        let (promise, future) = self.create_promise();
        promise.resolve(value);
        future
    }

    pub fn run_in_worker_thread<T, F, R>(&self, f: F) -> Future<T>
    where
        T: Send + 'static,
        F: FnOnce() -> R + Send + 'static,
        R: ResolveOutput<T>,
    {
        self.run(Context::Worker, f)
    }

    pub fn run_in_main_thread<T, F, R>(&self, f: F) -> Future<T>
    where
        T: Send + 'static,
        F: FnOnce() -> R + Send + 'static,
        R: ResolveOutput<T>,
    {
        self.run(Context::Main, f)
    }

    pub fn run_in_thread_pool<T, F, R>(&self, thread_pool: &ThreadPool, f: F) -> Future<T>
    where
        T: Send + 'static,
        F: FnOnce() -> R + Send + 'static,
        R: ResolveOutput<T>,
    {
        self.run_in_pool(thread_pool, f)
    }

    /// Run a function in the given scheduling context.
    pub fn run<T, F, R>(&self, context: Context, f: F) -> Future<T>
    where
        T: Send + 'static,
        F: FnOnce() -> R + Send + 'static,
        R: ResolveOutput<T>,
    {
        let (promise, future) = self.create_promise();
        let task = move || {
            f().resolve_into(promise);
        };

        match self.inner.scheduler_for(context) {
            Some(scheduler) => {
                if scheduler.is_current_thread() {
                    task();
                } else {
                    scheduler.schedule(Box::new(task));
                }
            }
            None => {
                // Immediate: run inline right now.
                task();
            }
        }

        future
    }

    /// Run a function in a named thread pool.
    pub fn run_in_pool<T, F, R>(&self, thread_pool: &ThreadPool, f: F) -> Future<T>
    where
        T: Send + 'static,
        F: FnOnce() -> R + Send + 'static,
        R: ResolveOutput<T>,
    {
        let scheduler = self.inner.thread_pool_scheduler(thread_pool);
        let (promise, future) = self.create_promise();
        scheduler.schedule(Box::new(move || {
            f().resolve_into(promise);
        }));
        future
    }

    pub fn dispatch_main_thread_tasks(&self) -> usize {
        self.inner.main_queue.dispatch_all()
    }

    pub fn dispatch_one_main_thread_task(&self) -> bool {
        self.inner.main_queue.dispatch_one()
    }

    pub fn has_pending_main_thread_tasks(&self) -> bool {
        self.inner.main_queue.has_pending()
    }

    pub fn enter_main_thread(&self) -> MainThreadScope {
        MainThreadScope::new()
    }

    pub fn all<T, I, W>(&self, futures: I) -> Future<Vec<T>>
    where
        T: Send + 'static,
        I: IntoIterator<Item = W>,
        W: Waitable<T>,
    {
        let futures = futures.into_iter().collect::<Vec<W>>();
        let (promise, future) = self.create_promise();

        self.inner.worker_scheduler().schedule(Box::new(move || {
            let mut values = Vec::with_capacity(futures.len());
            for future_item in futures {
                match future_item.waitable_wait() {
                    Ok(value) => values.push(value),
                    Err(error) => {
                        promise.reject(error);
                        return;
                    }
                }
            }
            promise.resolve(values);
        }));

        future
    }
}

mod sealed {
    pub trait Sealed<T> {}
}

/// Internal waiting abstraction used by [`AsyncSystem::all`].
pub trait Waitable<T>: sealed::Sealed<T> + Send + 'static {
    fn waitable_wait(self) -> Result<T, AsyncError>;
}

impl<T> sealed::Sealed<T> for Future<T> where T: Send + 'static {}

impl<T> Waitable<T> for Future<T>
where
    T: Send + 'static,
{
    fn waitable_wait(self) -> Result<T, AsyncError> {
        self.wait()
    }
}

impl<T> sealed::Sealed<T> for SharedFuture<T> where T: Clone + Send + 'static {}

impl<T> Waitable<T> for SharedFuture<T>
where
    T: Clone + Send + 'static,
{
    fn waitable_wait(self) -> Result<T, AsyncError> {
        self.wait()
    }
}
