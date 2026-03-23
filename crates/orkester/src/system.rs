use crate::context::Context;
use crate::error::AsyncError;
use crate::executor::Executor;
use crate::future::{Future, ResolveOutput, create_pair};
use crate::main_thread::{MainThreadQueue, MainThreadScope, is_main_thread};
use crate::promise::Promise;
use crate::task_processor::TaskProcessor;
use crate::thread_pool::ThreadPool;
use std::cell::Cell;
use std::fmt::{self, Debug, Formatter};
use std::sync::{Arc, RwLock};

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

// --- Internal executor implementations ---

struct WorkerExecutor {
    task_processor: Arc<dyn TaskProcessor>,
}

impl Executor for WorkerExecutor {
    fn execute(&self, task: Task) {
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

struct MainExecutor {
    queue: Arc<MainThreadQueue>,
}

impl Executor for MainExecutor {
    fn execute(&self, task: Task) {
        self.queue.enqueue(task);
    }

    fn is_current_thread(&self) -> bool {
        is_main_thread()
    }
}

struct ThreadPoolExecutor {
    thread_pool: ThreadPool,
}

impl Executor for ThreadPoolExecutor {
    fn execute(&self, task: Task) {
        self.thread_pool.schedule(task);
    }

    fn is_current_thread(&self) -> bool {
        self.thread_pool.is_current_thread()
    }
}

// --- Context entry ---

pub(crate) struct ContextEntry {
    #[allow(dead_code)]
    pub(crate) name: String,
    pub(crate) executor: Arc<dyn Executor>,
}

// --- SystemImpl ---

pub(crate) struct SystemImpl {
    pub(crate) main_queue: Arc<MainThreadQueue>,
    contexts: RwLock<Vec<ContextEntry>>,
}

impl SystemImpl {
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
    /// Create an async system with the given background executor.
    pub fn new(executor: impl Executor + 'static) -> Self {
        let main_queue = Arc::new(MainThreadQueue::new());

        let contexts = vec![
            ContextEntry {
                name: "Background".to_string(),
                executor: Arc::new(executor) as Arc<dyn Executor>,
            },
            ContextEntry {
                name: "Main".to_string(),
                executor: Arc::new(MainExecutor {
                    queue: Arc::clone(&main_queue),
                }),
            },
        ];

        Self {
            inner: Arc::new(SystemImpl {
                main_queue,
                contexts: RwLock::new(contexts),
            }),
        }
    }

    /// Create an async system with a built-in thread pool.
    ///
    /// ```rust,ignore
    /// let system = AsyncSystem::with_threads(4);
    /// ```
    pub fn with_threads(n: usize) -> Self {
        Self::new(WorkerExecutor {
            task_processor: Arc::new(crate::task_processor::ThreadPoolTaskProcessor::new(n)),
        })
    }

    /// Create a builder for configuring an async system.
    pub fn builder() -> AsyncSystemBuilder {
        AsyncSystemBuilder {
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
    /// let gpu = system.register_context("gpu", GpuThreadExecutor::new());
    /// system.run(gpu, || upload_texture(data));
    /// ```
    pub fn register_context(&self, name: &str, executor: impl Executor + 'static) -> Context {
        let mut contexts = self.inner.contexts.write().expect("context lock");
        let id = contexts.len() as u32;
        contexts.push(ContextEntry {
            name: name.to_string(),
            executor: Arc::new(executor),
        });
        Context(id)
    }

    pub fn thread_pool(&self, number_of_threads: usize) -> ThreadPool {
        ThreadPool::new(number_of_threads)
    }

    pub fn promise<T: Send + 'static>(&self) -> (Promise<T>, Future<T>) {
        create_pair(self.clone())
    }

    pub fn future<T, F>(&self, f: F) -> Future<T>
    where
        T: Send + 'static,
        F: FnOnce(Promise<T>) + Send + 'static,
    {
        let (promise, future) = self.promise();
        f(promise);
        future
    }

    pub fn resolved<T: Send + 'static>(&self, value: T) -> Future<T> {
        let (promise, future) = self.promise();
        promise.resolve(value);
        future
    }

    /// Run a function in the given scheduling context.
    pub fn run<T, F, R>(&self, context: Context, f: F) -> Future<T>
    where
        T: Send + 'static,
        F: FnOnce() -> R + Send + 'static,
        R: ResolveOutput<T>,
    {
        let (promise, future) = self.promise();
        let task = move || {
            f().resolve_into(promise);
        };

        match self.inner.executor_for(context) {
            Some(executor) => {
                if executor.is_current_thread() {
                    task();
                } else {
                    executor.execute(Box::new(task));
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
        let executor = self.inner.pool_executor(thread_pool);
        let (promise, future) = self.promise();
        executor.execute(Box::new(move || {
            f().resolve_into(promise);
        }));
        future
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
    pub fn run_async<T, F, Fut>(&self, context: Context, f: F) -> Future<T>
    where
        T: Send + 'static,
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = T> + Send + 'static,
    {
        let (promise, future) = self.promise();
        match self.inner.executor_for(context) {
            Some(executor) => {
                executor.spawn_future(Box::pin(async move {
                    promise.resolve(f().await);
                }));
            }
            None => {
                // IMMEDIATE: block_on inline
                promise.resolve(crate::block_on::block_on(f()));
            }
        }
        future
    }

    /// Spawn an async future on the background context.
    ///
    /// Returns a `Future<T>` that resolves when the spawned future completes.
    ///
    /// ```rust,ignore
    /// let result = system.spawn(async {
    ///     let data = fetch(url).await;
    ///     process(data)
    /// }).wait().unwrap();
    /// ```
    pub fn spawn<T, Fut>(&self, future: Fut) -> Future<T>
    where
        T: Send + 'static,
        Fut: std::future::Future<Output = T> + Send + 'static,
    {
        let (promise, out_future) = self.promise();
        self.inner
            .executor_for(Context::BACKGROUND)
            .expect("Background executor")
            .spawn_future(Box::pin(async move {
                promise.resolve(future.await);
            }));
        out_future
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
    pub fn spawn_local<T, Fut>(&self, future: Fut) -> Future<T>
    where
        T: Send + 'static,
        Fut: std::future::Future<Output = T> + 'static,
    {
        let (promise, out_future) = self.promise();
        wasm_bindgen_futures::spawn_local(async move {
            promise.resolve(future.await);
        });
        out_future
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

    pub fn join_all<T, I>(&self, futures: I) -> Future<Vec<T>>
    where
        T: Send + 'static,
        I: IntoIterator<Item = Future<T>>,
    {
        let inputs: Vec<Future<T>> = futures.into_iter().collect();
        let count = inputs.len();

        if count == 0 {
            return self.resolved(Vec::new());
        }

        let (promise, output) = self.promise::<Vec<T>>();

        // Shared state for collecting results from continuations.
        let results: Arc<std::sync::Mutex<Vec<Option<Result<T, AsyncError>>>>> =
            Arc::new(std::sync::Mutex::new((0..count).map(|_| None).collect()));
        let remaining = Arc::new(std::sync::atomic::AtomicUsize::new(count));
        let shared_promise = Arc::new(std::sync::Mutex::new(Some(promise)));

        for (i, mut fut) in inputs.into_iter().enumerate() {
            let source = match fut.state.take() {
                Some(s) => s,
                None => {
                    // Already consumed — write an error into this slot.
                    let results = Arc::clone(&results);
                    let remaining = Arc::clone(&remaining);
                    let shared_promise = Arc::clone(&shared_promise);
                    let mut guard = results.lock().expect("join_all lock");
                    guard[i] = Some(Err(AsyncError::msg("Future already consumed")));
                    drop(guard);

                    if remaining.fetch_sub(1, std::sync::atomic::Ordering::AcqRel) == 1 {
                        Self::resolve_join_all(results, shared_promise);
                    }
                    continue;
                }
            };

            let results = Arc::clone(&results);
            let remaining = Arc::clone(&remaining);
            let shared_promise = Arc::clone(&shared_promise);
            let source_ref = Arc::clone(&source);

            source.register_continuation(Box::new(move || {
                let result = source_ref
                    .take_result()
                    .unwrap_or_else(|| Err(AsyncError::msg("Future already consumed")));

                let mut guard = results.lock().expect("join_all lock");
                guard[i] = Some(result);
                drop(guard);

                if remaining.fetch_sub(1, std::sync::atomic::Ordering::AcqRel) == 1 {
                    Self::resolve_join_all(results, shared_promise);
                }
            }));
        }

        output
    }

    fn resolve_join_all<T: Send + 'static>(
        results: Arc<std::sync::Mutex<Vec<Option<Result<T, AsyncError>>>>>,
        shared_promise: Arc<std::sync::Mutex<Option<Promise<Vec<T>>>>>,
    ) {
        let promise = match shared_promise.lock().expect("join_all lock").take() {
            Some(p) => p,
            None => return,
        };
        let mut guard = results.lock().expect("join_all lock");
        let mut values = Vec::with_capacity(guard.len());
        for slot in guard.iter_mut() {
            match slot.take() {
                Some(Ok(v)) => values.push(v),
                Some(Err(e)) => {
                    promise.reject(e);
                    return;
                }
                None => {
                    promise.reject(AsyncError::msg("join_all: missing result"));
                    return;
                }
            }
        }
        promise.resolve(values);
    }
}

// --- Builder ---

/// Builder for configuring an [`AsyncSystem`].
pub struct AsyncSystemBuilder {
    background_executor: Option<Arc<dyn Executor>>,
    custom_contexts: Vec<(String, Arc<dyn Executor>)>,
}

impl AsyncSystemBuilder {
    /// Set a custom background executor.
    ///
    /// If not set, a default thread pool is created using available parallelism.
    ///
    /// ```rust,ignore
    /// let system = AsyncSystem::builder()
    ///     .executor(TokioExecutor::current())
    ///     .build();
    /// ```
    pub fn executor(mut self, executor: impl Executor + 'static) -> Self {
        self.background_executor = Some(Arc::new(executor));
        self
    }

    /// Register a custom context at build time.
    ///
    /// The returned [`AsyncSystem`] will have this context pre-registered.
    /// Use [`AsyncSystem::register_context`] for runtime registration.
    pub fn context(mut self, name: &str, executor: impl Executor + 'static) -> Self {
        self.custom_contexts
            .push((name.to_string(), Arc::new(executor)));
        self
    }

    /// Build the async system.
    pub fn build(self) -> AsyncSystem {
        let main_queue = Arc::new(MainThreadQueue::new());

        let bg_executor: Arc<dyn Executor> = if let Some(exec) = self.background_executor {
            exec
        } else {
            let tp = Arc::new(crate::task_processor::ThreadPoolTaskProcessor::default_pool());
            Arc::new(WorkerExecutor { task_processor: tp })
        };

        let mut contexts = vec![
            ContextEntry {
                name: "Background".to_string(),
                executor: bg_executor,
            },
            ContextEntry {
                name: "Main".to_string(),
                executor: Arc::new(MainExecutor {
                    queue: Arc::clone(&main_queue),
                }),
            },
        ];

        for (name, executor) in self.custom_contexts {
            contexts.push(ContextEntry { name, executor });
        }

        AsyncSystem {
            inner: Arc::new(SystemImpl {
                main_queue,
                contexts: RwLock::new(contexts),
            }),
        }
    }
}

impl Default for AsyncSystem {
    /// Create an async system with a default thread pool.
    fn default() -> Self {
        Self::builder().build()
    }
}
