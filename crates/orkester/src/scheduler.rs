use crate::context::Context;
use crate::error::AsyncError;
use crate::executor::Executor;
use crate::main_loop::{MainThreadQueue, MainThreadScope, is_main_thread};
use crate::resolver::Resolver;
use crate::task::{ResolveOutput, Task, TaskInner, create_pair};
use crate::task_cell::TaskCell;
use crate::task_processor::TaskProcessor;
use crate::thread_pool::ThreadPool;
use crate::timer::TimerWheel;
use std::cell::Cell;
use std::fmt::{self, Debug, Formatter};
use std::sync::{Arc, RwLock};
use std::time::Duration;

type Work = Box<dyn FnOnce() + Send + 'static>;

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

// --- Internal executor implementations ---

struct WorkerExecutor {
    task_processor: Arc<dyn TaskProcessor>,
}

impl Executor for WorkerExecutor {
    fn execute(&self, work: Work) {
        let task_processor = Arc::clone(&self.task_processor);
        task_processor.start_task(Box::new(move || {
            let _scope = WorkerThreadScope::enter();
            work();
        }));
    }

    fn is_current_thread(&self) -> bool {
        is_worker_thread()
    }
}

struct MainExecutor {
    queue: Arc<MainThreadQueue>,
}

impl Executor for MainExecutor {
    fn execute(&self, work: Work) {
        self.queue.enqueue(work);
    }

    fn is_current_thread(&self) -> bool {
        is_main_thread()
    }
}

struct ThreadPoolExecutor {
    thread_pool: ThreadPool,
}

impl Executor for ThreadPoolExecutor {
    fn execute(&self, work: Work) {
        self.thread_pool.schedule(work);
    }

    fn is_current_thread(&self) -> bool {
        self.thread_pool.is_current_thread()
    }
}

// --- Context entry ---

pub(crate) struct ContextEntry {
    pub(crate) executor: Arc<dyn Executor>,
}

// --- SchedulerInner ---

pub(crate) struct SchedulerInner {
    pub(crate) main_queue: Arc<MainThreadQueue>,
    pub(crate) timer: TimerWheel,
    contexts: RwLock<Vec<ContextEntry>>,
}

impl SchedulerInner {
    /// Return the executor for a given context.
    ///
    /// Returns `None` for [`Context::IMMEDIATE`] — callers must handle
    /// immediate execution inline.
    pub(crate) fn executor_for(&self, context: Context) -> Option<Arc<dyn Executor>> {
        if context == Context::IMMEDIATE {
            return None;
        }
        let contexts = self.contexts.read().expect("context lock");
        contexts
            .get(context.0 as usize)
            .map(|entry| Arc::clone(&entry.executor))
    }

    /// Return an executor backed by a [`ThreadPool`].
    pub(crate) fn pool_executor(&self, thread_pool: &ThreadPool) -> Arc<dyn Executor> {
        Arc::new(ThreadPoolExecutor {
            thread_pool: thread_pool.clone(),
        })
    }
}

/// Root async runtime object. Cheap to clone.
#[derive(Clone)]
pub struct Scheduler {
    pub(crate) inner: Arc<SchedulerInner>,
}

impl PartialEq for Scheduler {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for Scheduler {}

impl Debug for Scheduler {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Scheduler")
            .field("inner", &(Arc::as_ptr(&self.inner) as usize))
            .finish()
    }
}

impl Scheduler {
    /// Create an async system with the given background executor.
    pub fn new(executor: impl Executor + 'static) -> Self {
        let main_queue = Arc::new(MainThreadQueue::new());

        let contexts = vec![
            ContextEntry {
                executor: Arc::new(executor) as Arc<dyn Executor>,
            },
            ContextEntry {
                executor: Arc::new(MainExecutor {
                    queue: Arc::clone(&main_queue),
                }),
            },
        ];

        Self {
            inner: Arc::new(SchedulerInner {
                main_queue,
                timer: TimerWheel::new(),
                contexts: RwLock::new(contexts),
            }),
        }
    }

    /// Create an async system with a built-in thread pool.
    ///
    /// ```rust,ignore
    /// let system = Scheduler::with_threads(4);
    /// ```
    pub fn with_threads(n: usize) -> Self {
        Self::new(WorkerExecutor {
            task_processor: Arc::new(crate::task_processor::ThreadPoolTaskProcessor::new(n)),
        })
    }

    /// Create a builder for configuring an async system.
    pub fn builder() -> SchedulerBuilder {
        SchedulerBuilder {
            background_executor: None,
            custom_contexts: Vec::new(),
        }
    }

    /// Register a custom execution context.
    ///
    /// Returns a [`Context`] handle that can be used with `run()`, `then()`,
    /// etc. The context handle is a simple integer index — cheap to copy
    /// and compare.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let gpu = system.register_context(GpuThreadExecutor::new());
    /// system.run(gpu, || upload_texture(data));
    /// ```
    pub fn register_context(&self, executor: impl Executor + 'static) -> Context {
        let mut contexts = self.inner.contexts.write().expect("context lock");
        let id = contexts.len() as u32;
        contexts.push(ContextEntry {
            executor: Arc::new(executor),
        });
        Context(id)
    }

    pub fn thread_pool(&self, number_of_threads: usize) -> ThreadPool {
        ThreadPool::new(number_of_threads)
    }

    pub fn resolver<T: Send + 'static>(&self) -> (Resolver<T>, Task<T>) {
        create_pair(self.clone())
    }

    pub fn task<T, F>(&self, f: F) -> Task<T>
    where
        T: Send + 'static,
        F: FnOnce(Resolver<T>) + Send + 'static,
    {
        let (resolver, task) = self.resolver();
        f(resolver);
        task
    }

    pub fn resolved<T: Send + 'static>(&self, value: T) -> Task<T> {
        Task::ready(self.clone(), value)
    }

    /// Run a function in the given scheduling context.
    pub fn run<T, F, R>(&self, context: Context, f: F) -> Task<T>
    where
        T: Send + 'static,
        F: FnOnce() -> R + Send + 'static,
        R: ResolveOutput<T>,
    {
        let (resolver, out) = self.resolver();
        let work = move || {
            f().resolve_into(resolver);
        };

        match self.inner.executor_for(context) {
            Some(executor) => {
                if executor.is_current_thread() {
                    work();
                } else {
                    executor.execute(Box::new(work));
                }
            }
            None => {
                // Immediate: run inline right now.
                work();
            }
        }

        out
    }

    /// Run a function in a named thread pool.
    pub fn run_in_pool<T, F, R>(&self, thread_pool: &ThreadPool, f: F) -> Task<T>
    where
        T: Send + 'static,
        F: FnOnce() -> R + Send + 'static,
        R: ResolveOutput<T>,
    {
        let executor = self.inner.pool_executor(thread_pool);
        let (resolver, out) = self.resolver();
        executor.execute(Box::new(move || {
            f().resolve_into(resolver);
        }));
        out
    }

    /// Run an async closure in the given scheduling context.
    ///
    /// The closure is called on the target context and its returned future
    /// is driven to completion there.
    ///
    /// ```rust,ignore
    /// let result = system.run_async(Context::BACKGROUND, || async {
    ///     expensive_computation().await
    /// }).wait().unwrap();
    /// ```
    pub fn run_async<T, F, Fut>(&self, context: Context, f: F) -> Task<T>
    where
        T: Send + 'static,
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = T> + Send + 'static,
    {
        let (resolver, out) = self.resolver();
        match self.inner.executor_for(context) {
            Some(executor) => {
                executor.spawn_future(Box::pin(async move {
                    resolver.resolve(f().await);
                }));
            }
            None => {
                // IMMEDIATE: block_on inline
                resolver.resolve(crate::block_on::block_on(f()));
            }
        }
        out
    }

    /// Spawn an async task on the background context.
    ///
    /// Returns a `Task<T>` that resolves when the spawned task completes.
    ///
    /// ```rust,ignore
    /// let result = system.spawn(async {
    ///     let data = fetch(url).await;
    ///     process(data)
    /// }).wait().unwrap();
    /// ```
    pub fn spawn<T, Fut>(&self, fut: Fut) -> Task<T>
    where
        T: Send + 'static,
        Fut: std::future::Future<Output = T> + Send + 'static,
    {
        let (resolver, out) = self.resolver();
        self.inner
            .executor_for(Context::BACKGROUND)
            .expect("Background executor")
            .spawn_future(Box::pin(async move {
                resolver.resolve(fut.await);
            }));
        out
    }

    /// Spawn an async future on the local thread (WASM only).
    ///
    /// Does not require `Send` on the future, matching WASM's single-threaded
    /// execution model. The returned `Future<T>` still requires `T: Send` for
    /// API consistency.
    ///
    /// This bypasses the `Executor` abstraction intentionally — the
    /// [`Executor::spawn_future`] method requires `Send`, which is the exact
    /// constraint `spawn_local` exists to relax.
    #[cfg(feature = "wasm")]
    pub fn spawn_local<T, Fut>(&self, fut: Fut) -> Task<T>
    where
        T: Send + 'static,
        Fut: std::future::Future<Output = T> + 'static,
    {
        let (resolver, out) = self.resolver();
        wasm_bindgen_futures::spawn_local(async move {
            resolver.resolve(fut.await);
        });
        out
    }

    pub fn flush_main(&self) -> usize {
        self.inner.main_queue.dispatch_all()
    }

    pub fn flush_main_one(&self) -> bool {
        self.inner.main_queue.dispatch_one()
    }

    pub fn main_pending(&self) -> bool {
        self.inner.main_queue.has_pending()
    }

    pub fn main_scope(&self) -> MainThreadScope {
        MainThreadScope::new()
    }

    /// Create a structured cancellation scope.
    ///
    /// Tasks spawned through the returned [`Scope`] are automatically
    /// cancelled when the scope is dropped.
    pub fn scope(&self) -> crate::scope::Scope {
        crate::scope::Scope::new(self.clone())
    }

    /// Create a timer-based delay.
    ///
    /// Returns a `Task<()>` that completes after `duration`. Uses the
    /// shared timer thread — no worker threads are parked.
    pub fn delay(&self, duration: Duration) -> Task<()> {
        if duration.is_zero() {
            return self.resolved(());
        }
        let (resolver, task) = self.resolver::<()>();
        let deadline = std::time::Instant::now() + duration;

        struct ResolveOnWake(std::sync::Mutex<Option<Resolver<()>>>);
        impl std::task::Wake for ResolveOnWake {
            fn wake(self: Arc<Self>) {
                if let Some(r) = self.0.lock().expect("timer resolver").take() {
                    r.resolve(());
                }
            }
        }

        let waker = std::task::Waker::from(Arc::new(ResolveOnWake(std::sync::Mutex::new(Some(
            resolver,
        )))));
        self.inner.timer.register(deadline, waker);
        task
    }

    pub fn join_all<T, I>(&self, tasks: I) -> Task<Vec<T>>
    where
        T: Send + 'static,
        I: IntoIterator<Item = Task<T>>,
    {
        let inputs: Vec<Task<T>> = tasks.into_iter().collect();
        let count = inputs.len();

        if count == 0 {
            return self.resolved(Vec::new());
        }

        let (resolver, output) = self.resolver::<Vec<T>>();

        // Shared state for collecting results from continuations.
        let results: Arc<std::sync::Mutex<Vec<Option<Result<T, AsyncError>>>>> =
            Arc::new(std::sync::Mutex::new((0..count).map(|_| None).collect()));
        let remaining = Arc::new(std::sync::atomic::AtomicUsize::new(count));
        let shared_resolver = Arc::new(std::sync::Mutex::new(Some(resolver)));

        for (i, task) in inputs.into_iter().enumerate() {
            match task.inner {
                TaskInner::Ready(result) => {
                    let result =
                        result.unwrap_or_else(|| Err(AsyncError::msg("Task already consumed")));
                    let mut guard = results.lock().expect("join_all lock");
                    guard[i] = Some(result);
                    drop(guard);
                    if remaining.fetch_sub(1, std::sync::atomic::Ordering::AcqRel) == 1 {
                        Self::resolve_join_all(Arc::clone(&results), Arc::clone(&shared_resolver));
                    }
                }
                TaskInner::Pending(source) => {
                    let results = Arc::clone(&results);
                    let remaining = Arc::clone(&remaining);
                    let shared_resolver = Arc::clone(&shared_resolver);

                    TaskCell::on_complete(source, move |result| {
                        let mut guard = results.lock().expect("join_all lock");
                        guard[i] = Some(result);
                        drop(guard);

                        if remaining.fetch_sub(1, std::sync::atomic::Ordering::AcqRel) == 1 {
                            Self::resolve_join_all(results, shared_resolver);
                        }
                    });
                }
            }
        }

        output
    }

    fn resolve_join_all<T: Send + 'static>(
        results: Arc<std::sync::Mutex<Vec<Option<Result<T, AsyncError>>>>>,
        shared_resolver: Arc<std::sync::Mutex<Option<Resolver<Vec<T>>>>>,
    ) {
        let resolver = match shared_resolver.lock().expect("join_all lock").take() {
            Some(r) => r,
            None => return,
        };
        let mut guard = results.lock().expect("join_all lock");
        let mut values = Vec::with_capacity(guard.len());
        for slot in guard.iter_mut() {
            match slot.take() {
                Some(Ok(v)) => values.push(v),
                Some(Err(e)) => {
                    resolver.reject(e);
                    return;
                }
                None => {
                    resolver.reject(AsyncError::msg("join_all: missing result"));
                    return;
                }
            }
        }
        resolver.resolve(values);
    }
}

// --- Builder ---

/// Builder for configuring an [`Scheduler`].
pub struct SchedulerBuilder {
    background_executor: Option<Arc<dyn Executor>>,
    custom_contexts: Vec<Arc<dyn Executor>>,
}

impl SchedulerBuilder {
    /// Set a custom background executor.
    ///
    /// If not set, a default thread pool is created using available parallelism.
    ///
    /// ```rust,ignore
    /// let system = Scheduler::builder()
    ///     .executor(TokioExecutor::current())
    ///     .build();
    /// ```
    pub fn executor(mut self, executor: impl Executor + 'static) -> Self {
        self.background_executor = Some(Arc::new(executor));
        self
    }

    /// Register a custom context at build time.
    ///
    /// The returned [`Scheduler`] will have this context pre-registered.
    /// Use [`Scheduler::register_context`] for runtime registration.
    pub fn context(mut self, executor: impl Executor + 'static) -> Self {
        self.custom_contexts.push(Arc::new(executor));
        self
    }

    /// Build the async system.
    pub fn build(self) -> Scheduler {
        let main_queue = Arc::new(MainThreadQueue::new());

        let bg_executor: Arc<dyn Executor> = if let Some(exec) = self.background_executor {
            exec
        } else {
            let tp = Arc::new(crate::task_processor::ThreadPoolTaskProcessor::default_pool());
            Arc::new(WorkerExecutor { task_processor: tp })
        };

        let mut contexts = vec![
            ContextEntry {
                executor: bg_executor,
            },
            ContextEntry {
                executor: Arc::new(MainExecutor {
                    queue: Arc::clone(&main_queue),
                }),
            },
        ];

        for executor in self.custom_contexts {
            contexts.push(ContextEntry { executor });
        }

        Scheduler {
            inner: Arc::new(SchedulerInner {
                main_queue,
                timer: TimerWheel::new(),
                contexts: RwLock::new(contexts),
            }),
        }
    }
}

impl Default for Scheduler {
    /// Create an async system with a default thread pool.
    fn default() -> Self {
        Self::builder().build()
    }
}
