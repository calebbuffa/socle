//! Async system: Future/Promise types and main-thread task dispatch.

use std::cell::Cell;
use std::fmt;
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};

use crate::task_processor::TaskProcessor;

// ── Main-thread designation ──────────────────────────────────────────────────

thread_local! {
    /// Non-zero when the current thread has been designated as "the main thread"
    /// via [`AsyncSystem::enter_main_thread`]. Counting allows nested scopes.
    static MAIN_THREAD_DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// Returns `true` if the calling thread is currently designated as the main
/// thread (i.e. inside an [`AsyncSystem::enter_main_thread`] scope).
fn is_main_thread() -> bool {
    MAIN_THREAD_DEPTH.with(|d| d.get() > 0)
}

// ── MainThreadQueue ──────────────────────────────────────────────────────────

type MainThreadTask = Box<dyn FnOnce() + Send>;

/// Main-thread task queue. Workers enqueue; the main thread drains via `dispatch`.
pub struct MainThreadQueue {
    queue: Mutex<Vec<MainThreadTask>>,
    condvar: Condvar,
}

impl MainThreadQueue {
    /// Create an empty queue.
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(Vec::new()),
            condvar: Condvar::new(),
        }
    }

    /// Enqueue a task for main-thread execution.
    pub fn enqueue(&self, task: MainThreadTask) {
        self.queue
            .lock()
            .expect("task queue lock poisoned")
            .push(task);
        self.condvar.notify_one();
    }

    /// Dispatch all pending main-thread tasks. Non-blocking.
    //
    /// Returns the number of tasks executed.
    pub fn dispatch(&self) -> usize {
        let tasks: Vec<MainThreadTask> = {
            let mut q = self.queue.lock().expect("task queue lock poisoned");
            std::mem::take(&mut *q)
        };
        let count = tasks.len();
        for task in tasks {
            task();
        }
        count
    }

    /// Dispatch zero or one continuation. Non-blocking.
    pub fn dispatch_one(&self) -> bool {
        let task = {
            let mut q = self.queue.lock().expect("task queue lock poisoned");
            if q.is_empty() {
                return false;
            }
            q.remove(0)
        };
        task();
        true
    }

    /// Whether there are pending tasks.
    pub fn has_pending(&self) -> bool {
        !self
            .queue
            .lock()
            .expect("task queue lock poisoned")
            .is_empty()
    }

    /// Block until a task is available, then dispatch it.
    /// Used by `wait_in_main_thread` to pump the queue while waiting.
    fn dispatch_blocking(&self) {
        let task = {
            let mut q = self.queue.lock().expect("task queue lock poisoned");
            while q.is_empty() {
                q = self.condvar.wait(q).expect("task queue condvar poisoned");
            }
            q.remove(0)
        };
        task();
    }
}

impl Default for MainThreadQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// The async system: owns the worker thread pool and main-thread queue.
#[derive(Clone)]
pub struct AsyncSystem {
    task_processor: Arc<dyn TaskProcessor>,
    main_queue: Arc<MainThreadQueue>,
}

impl fmt::Debug for AsyncSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AsyncSystem")
            .field(
                "task_processor",
                &(Arc::as_ptr(&self.task_processor) as *const () as usize),
            )
            .field("main_queue", &(Arc::as_ptr(&self.main_queue) as usize))
            .finish()
    }
}

impl AsyncSystem {
    /// Create a new async system backed by the given task processor.
    pub fn new(task_processor: Arc<dyn TaskProcessor>) -> Self {
        Self {
            task_processor,
            main_queue: Arc::new(MainThreadQueue::new()),
        }
    }

    /// Create a new async system with a shared main-thread queue.
    ///
    /// Use this when multiple async systems should share the same
    /// main-thread dispatch queue (e.g. building sublayers).
    pub fn with_main_queue(
        task_processor: Arc<dyn TaskProcessor>,
        main_queue: Arc<MainThreadQueue>,
    ) -> Self {
        Self {
            task_processor,
            main_queue,
        }
    }

    /// Create a [`Promise`].
    ///
    /// Call [`get_future`](Promise::get_future) on the returned promise to
    /// obtain the paired [`Future`]. Mirrors `AsyncSystem::createPromise` in
    /// cesium-native: the future is obtained from the promise, not returned
    /// alongside it.
    pub fn create_promise<T: Send + 'static>(&self) -> Promise<T> {
        let (tx, rx) = mpsc::sync_channel(1);
        Promise {
            sender: Some(tx),
            future_rx: Some(rx),
            async_system: self.clone(),
        }
    }

    /// Run a closure in a worker thread, returning a Future with the result.
    ///
    /// If the closure returns `Result<T>`, the future resolves or rejects
    /// accordingly. Mirrors `AsyncSystem::runInWorkerThread`.
    pub fn run_in_worker_thread<T, F>(&self, f: F) -> Future<T>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let mut promise = self.create_promise();
        let future = promise.future();
        self.task_processor.start_task(Box::new(move || {
            promise.resolve(f());
        }));
        future
    }

    /// Run a closure on the main thread, returning a Future.
    ///
    /// If the calling thread is currently designated as the main thread (inside
    /// an [`enter_main_thread`](Self::enter_main_thread) scope), the closure
    /// runs **immediately inline** and the returned Future is already resolved
    /// before this method returns — matching cesium-native's behaviour.
    ///
    /// Otherwise the closure is queued and will execute the next time
    /// [`dispatch_main_thread_tasks`](Self::dispatch_main_thread_tasks) is called.
    pub fn run_in_main_thread<T, F>(&self, f: F) -> Future<T>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        if is_main_thread() {
            // Execute inline — same thread, same semantics as cesium-native's
            // ImmediateScheduler path.
            return self.create_resolved_future(f());
        }
        let mut promise = self.create_promise();
        let future = promise.future();
        self.main_queue.enqueue(Box::new(move || {
            promise.resolve(f());
        }));
        future
    }

    /// Dispatch all pending main-thread tasks.
    ///
    /// Call this once per frame from the main thread. Returns the number
    /// of tasks that were executed.
    pub fn dispatch_main_thread_tasks(&self) -> usize {
        self.main_queue.dispatch()
    }

    /// Whether there are pending main-thread tasks.
    pub fn has_pending_main_thread_tasks(&self) -> bool {
        self.main_queue.has_pending()
    }

    /// Get the underlying task processor.
    pub fn task_processor(&self) -> &Arc<dyn TaskProcessor> {
        &self.task_processor
    }

    /// Get the main-thread queue.
    pub fn main_queue(&self) -> &Arc<MainThreadQueue> {
        &self.main_queue
    }

    /// Dispatch a single pending main-thread task.
    ///
    /// Convenience wrapper around `MainThreadQueue::dispatch_one`.
    /// Returns `true` if a task was dispatched.
    pub fn dispatch_one_main_thread_task(&self) -> bool {
        self.main_queue.dispatch_one()
    }

    /// Enter a scope in which the calling thread is treated as "the main
    /// thread".
    ///
    /// While the returned [`MainThreadScope`] is alive, calls to
    /// [`run_in_main_thread`](Self::run_in_main_thread) from this thread will
    /// execute their closures **immediately inline** rather than queueing them.
    /// Scopes are reference-counted so nesting is safe.
    ///
    /// Mirrors `AsyncSystem::enterMainThread()` in cesium-native.
    ///
    /// ```rust,ignore
    /// let _scope = sys.enter_main_thread();
    /// // now on the "main thread" — run_in_main_thread executes inline
    /// sys.dispatch_main_thread_tasks(); // also fine to call, but not required
    /// ```
    pub fn enter_main_thread(&self) -> MainThreadScope {
        MainThreadScope::new()
    }

    /// Create a future that is already resolved with a value.
    ///
    /// Mirrors `AsyncSystem::createResolvedFuture(T)` in cesium-native.
    pub fn create_resolved_future<T: Send + 'static>(&self, value: T) -> Future<T> {
        Future {
            state: Mutex::new(FutureState::Ready(Ok(value))),
            async_system: self.clone(),
        }
    }

    /// Create a future that is already resolved with no value.
    ///
    /// Mirrors `AsyncSystem::createResolvedFuture()` (void specialization) in
    /// cesium-native.
    pub fn create_resolved_future_void(&self) -> Future<()> {
        Future {
            state: Mutex::new(FutureState::Ready(Ok(()))),
            async_system: self.clone(),
        }
    }

    /// Create a Future by immediately invoking a callback that receives a
    /// [`Promise`].
    ///
    /// If the callback panics, the future is automatically rejected with the
    /// panic message rather than unwinding — the panic does not escape.
    /// Mirrors `AsyncSystem::createFuture` in cesium-native, which auto-rejects
    /// on exception.
    ///
    /// ```rust,ignore
    /// let future = sys.create_future(|mut promise| {
    ///     // resolve later, or right now
    ///     promise.resolve(42);
    /// });
    /// ```
    pub fn create_future<T, F>(&self, f: F) -> Future<T>
    where
        T: Send + 'static,
        F: FnOnce(Promise<T>),
    {
        let mut promise = self.create_promise::<T>();
        let future = promise.future();
        // Wrap in AssertUnwindSafe — Promise<T> contains a channel sender which
        // isn't UnwindSafe, but we own both ends so a panic here is safe to
        // catch: the dropped promise will auto-reject via its Drop impl.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(promise)));
        if let Err(payload) = result {
            let msg = payload
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "callback panicked".to_string());
            // The promise was dropped (auto-rejected) by catch_unwind unwinding;
            // override that with our explicit message so callers get a useful error.
            return Future {
                state: Mutex::new(FutureState::Ready(Err(AsyncError::msg(msg)))),
                async_system: self.clone(),
            };
        }
        future
    }

    /// Wait for all futures in a vector, returning a vector of results.
    ///
    /// If any future rejects, the entire result is an `Err` carrying the first
    /// rejection. Mirrors `AsyncSystem::all(Vec<Future<T>>)` in cesium-native.
    pub fn all<T: Send + 'static>(&self, futures: Vec<Future<T>>) -> Future<Vec<T>> {
        let mut promise = self.create_promise();
        let combined_future = promise.future();
        self.task_processor.start_task(Box::new(move || {
            let mut results = Vec::with_capacity(futures.len());
            for f in futures {
                match f.wait() {
                    Ok(v) => results.push(v),
                    Err(e) => {
                        promise.reject(e);
                        return;
                    }
                }
            }
            promise.resolve(results);
        }));
        combined_future
    }

    /// Wait for all shared futures in a vector, returning a vector of results.
    ///
    /// If any future rejects, the entire result is an `Err` carrying the first
    /// rejection. Mirrors `AsyncSystem::all(Vec<SharedFuture<T>>)` in
    /// cesium-native.
    pub fn all_shared<T: Clone + Send + 'static>(
        &self,
        futures: Vec<SharedFuture<T>>,
    ) -> Future<Vec<T>> {
        let mut promise = self.create_promise();
        let combined_future = promise.future();
        self.task_processor.start_task(Box::new(move || {
            let mut results = Vec::with_capacity(futures.len());
            for f in futures {
                match f.wait() {
                    Ok(v) => results.push(v),
                    Err(e) => {
                        promise.reject(e);
                        return;
                    }
                }
            }
            promise.resolve(results);
        }));
        combined_future
    }
}

impl PartialEq for AsyncSystem {
    /// Returns `true` if both instances share the same underlying task
    /// processor and main-thread queue — i.e. they are interchangeable.
    /// Mirrors `AsyncSystem::operator==` in cesium-native.
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.task_processor, &other.task_processor)
            && Arc::ptr_eq(&self.main_queue, &other.main_queue)
    }
}

impl Eq for AsyncSystem {}

// ── all_tuple! macro ─────────────────────────────────────────────────────────

/// Wait for a heterogeneous set of futures and collect their values into a
/// tuple.
///
/// Mirrors the variadic `AsyncSystem::all(Futures&&...)` overload in
/// cesium-native that returns `Future<std::tuple<...>>`.
///
/// # Usage
/// ```rust,ignore
/// let future = all_tuple!(sys, future_a, future_b, future_c);
/// let (a, b, c) = future.wait().unwrap();
/// ```
///
/// All futures must be `Future<T>` (not `SharedFuture`). The values must be
/// `Send + 'static`. Futures are waited sequentially on a worker thread; the
/// first rejection short-circuits and rejects the combined future.
#[macro_export]
macro_rules! all_tuple {
    ($sys:expr, $($fut:expr),+ $(,)?) => {{
        let sys: &$crate::AsyncSystem = &$sys;
        let mut promise = sys.create_promise();
        let combined = promise.future();
        // Capture futures by move inside a worker task.
        // Each future is waited in declaration order; first error wins.
        sys.task_processor().start_task(Box::new(move || {
            let result = (|| -> ::std::result::Result<_, $crate::AsyncError> {
                Ok(( $( $fut.wait()? ),+ ))
            })();
            match result {
                Ok(tuple) => promise.resolve(tuple),
                Err(e) => promise.reject(e),
            }
        }));
        combined
    }};
}

// ── MainThreadScope ──────────────────────────────────────────────────────────

/// RAII guard returned by [`AsyncSystem::enter_main_thread`].
///
/// While this value is alive the calling thread is designated as "the main
/// thread": [`AsyncSystem::run_in_main_thread`] will execute closures inline
/// rather than queueing them. Scopes are reference-counted so nesting is safe.
/// Mirrors `AsyncSystem::MainThreadScope` in cesium-native.
pub struct MainThreadScope {
    _private: (),
}

impl MainThreadScope {
    fn new() -> Self {
        MAIN_THREAD_DEPTH.with(|d| d.set(d.get() + 1));
        Self { _private: () }
    }
}

impl Drop for MainThreadScope {
    fn drop(&mut self) {
        MAIN_THREAD_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    }
}

// ── AsyncError ───────────────────────────────────────────────────────────────

/// A type-erased, cloneable async error.
///
/// Mirrors `std::exception_ptr` from cesium-native: preserves the original error
/// type behind a reference-counted pointer. Use [`downcast_ref`](Self::downcast_ref)
/// to recover the concrete error type.
///
/// Implements [`Clone`] via [`Arc`] so it can be shared across [`SharedFuture`]
/// consumers without copying the error.
#[derive(Clone)]
pub struct AsyncError(Arc<dyn std::error::Error + Send + Sync + 'static>);

impl AsyncError {
    /// Wrap any concrete error type.
    ///
    /// ```rust,ignore
    /// promise.reject(AsyncError::new(my_i3s_error));
    /// ```
    pub fn new<E: std::error::Error + Send + Sync + 'static>(e: E) -> Self {
        AsyncError(Arc::new(e))
    }

    /// Create from a plain string message.
    ///
    /// ```rust,ignore
    /// promise.reject(AsyncError::msg("something went wrong"));
    /// ```
    pub fn msg(s: impl Into<String>) -> Self {
        AsyncError(Arc::new(StringError(s.into())))
    }

    /// Try to downcast to the concrete error type `E`.
    pub fn downcast_ref<E: std::error::Error + 'static>(&self) -> Option<&E> {
        self.0.downcast_ref::<E>()
    }

    /// Access the underlying error as a trait object.
    pub fn inner(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
        self.0.as_ref()
    }
}

impl fmt::Display for AsyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.0.as_ref(), f)
    }
}

impl fmt::Debug for AsyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AsyncError({:?})", self.0)
    }
}

/// Private string-backed error used by [`AsyncError::msg`].
#[derive(Debug)]
struct StringError(String);

impl fmt::Display for StringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for StringError {}

/// A promise that can be resolved with a value of type `T`.
///
/// Created via [`AsyncSystem::create_promise`]. Call [`future`](Self::future)
/// to obtain the paired [`Future`] — mirrors `Promise::getFuture()` in cesium-native.
pub struct Promise<T: Send + 'static> {
    sender: Option<mpsc::SyncSender<Result<T, AsyncError>>>,
    /// Receiver-side data consumed by `future()`. `None` after the first call.
    future_rx: Option<mpsc::Receiver<Result<T, AsyncError>>>,
    async_system: AsyncSystem,
}

impl<T: Send + 'static> Promise<T> {
    /// Get the [`Future`] paired with this promise.
    ///
    /// May only be called once. Panics if called a second time.
    /// Mirrors `Promise::getFuture()` in cesium-native.
    pub fn future(&mut self) -> Future<T> {
        let rx = self
            .future_rx
            .take()
            .expect("Promise::future called more than once");
        Future {
            state: Mutex::new(FutureState::Pending(rx)),
            async_system: self.async_system.clone(),
        }
    }

    /// Resolve the promise with a value.
    pub fn resolve(mut self, value: T) {
        if let Some(tx) = self.sender.take() {
            let _ = tx.send(Ok(value));
        }
    }

    /// Reject the promise with an error.
    ///
    /// Pass an [`AsyncError`] — use [`AsyncError::new`] to wrap a typed error or
    /// [`AsyncError::msg`] for a plain string message.
    /// Mirrors `Promise::reject` in cesium-native.
    pub fn reject(mut self, error: AsyncError) {
        if let Some(tx) = self.sender.take() {
            let _ = tx.send(Err(error));
        }
    }
}

impl<T: Send + 'static> Drop for Promise<T> {
    fn drop(&mut self) {
        // If the promise is dropped without resolving, reject it
        if let Some(tx) = self.sender.take() {
            let _ = tx.send(Err(AsyncError::msg("Promise dropped without resolving")));
        }
    }
}

/// Internal state of a Future.
enum FutureState<T> {
    /// Waiting for a result from the paired Promise.
    Pending(mpsc::Receiver<Result<T, AsyncError>>),
    /// Resolved with a value.
    Ready(Result<T, AsyncError>),
    /// Already consumed (wait/then was called).
    Consumed,
}

/// A future that resolves to a value of type `T`.
///
/// Created via [`AsyncSystem::create_promise`] or [`AsyncSystem::run_in_worker_thread`].
pub struct Future<T: Send + 'static> {
    state: Mutex<FutureState<T>>,
    async_system: AsyncSystem,
}

// SAFETY: Future<T> is Send+Sync because T is Send and state is behind a Mutex
unsafe impl<T: Send + 'static> Send for Future<T> {}
unsafe impl<T: Send + 'static> Sync for Future<T> {}

impl<T: Send + 'static> Future<T> {
    /// Try to transition from Pending → Ready by polling the channel.
    fn try_resolve(&self) {
        let mut state = self.state.lock().expect("future state lock poisoned");
        if let FutureState::Pending(ref rx) = *state {
            match rx.try_recv() {
                Ok(result) => *state = FutureState::Ready(result),
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    *state = FutureState::Ready(Err(AsyncError::msg(
                        "Future channel closed without result",
                    )));
                }
            }
        }
    }

    /// Block until the result is available.
    ///
    /// **Warning**: Do not call this from the main thread if the future's
    /// resolution depends on main-thread work. Use
    /// [`wait_in_main_thread`](Self::wait_in_main_thread) instead.
    pub fn wait(self) -> Result<T, AsyncError> {
        let mut state = self.state.into_inner().unwrap_or_else(|e| e.into_inner());
        match std::mem::replace(&mut state, FutureState::Consumed) {
            FutureState::Pending(rx) => match rx.recv() {
                Ok(result) => result,
                Err(_) => Err(AsyncError::msg("Future channel closed without result")),
            },
            FutureState::Ready(result) => result,
            FutureState::Consumed => Err(AsyncError::msg("Future already consumed")),
        }
    }

    /// Block until the result is available, while pumping the main-thread queue.
    ///
    /// This is safe to call from the main thread. It dispatches queued
    /// main-thread tasks while waiting, preventing deadlocks.
    /// Mirrors `Future::waitInMainThread`.
    pub fn wait_in_main_thread(self) -> Result<T, AsyncError> {
        loop {
            {
                let mut state = self.state.lock().expect("future state lock poisoned");
                if let FutureState::Pending(ref rx) = *state {
                    match rx.try_recv() {
                        Ok(result) => {
                            *state = FutureState::Consumed;
                            return result;
                        }
                        Err(mpsc::TryRecvError::Disconnected) => {
                            *state = FutureState::Consumed;
                            return Err(AsyncError::msg("Future channel closed without result"));
                        }
                        Err(mpsc::TryRecvError::Empty) => {}
                    }
                } else {
                    match std::mem::replace(&mut *state, FutureState::Consumed) {
                        FutureState::Ready(result) => return result,
                        FutureState::Consumed => {
                            return Err(AsyncError::msg("Future already consumed"));
                        }
                        FutureState::Pending(_) => unreachable!(),
                    }
                }
            }
            // Pump the main-thread queue while we wait
            if !self.async_system.main_queue.dispatch_one() {
                // No main-thread work — yield to avoid spinning
                std::thread::yield_now();
            }
        }
    }

    /// Check if the result is available without blocking.
    pub fn is_ready(&self) -> bool {
        self.try_resolve();
        let state = self.state.lock().expect("future state lock poisoned");
        matches!(&*state, FutureState::Ready(_))
    }

    /// Chain a continuation that runs in a worker thread.
    ///
    /// The continuation receives `Result<T, String>` and returns `U`.
    /// Returns a new `Future<U>`.
    pub fn then_in_worker_thread<U, F>(self, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> U + Send + 'static,
    {
        let async_sys = self.async_system.clone();
        let mut promise = async_sys.create_promise();
        let future = promise.future();

        async_sys
            .task_processor
            .start_task(Box::new(move || match self.wait() {
                Ok(value) => promise.resolve(f(value)),
                Err(e) => promise.reject(e),
            }));

        future
    }

    /// Chain a continuation that runs on the main thread.
    ///
    /// The continuation will execute when `dispatch_main_thread_tasks` is called
    /// after the current future resolves.
    pub fn then_in_main_thread<U, F>(self, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> U + Send + 'static,
    {
        let async_sys = self.async_system.clone();
        let mut promise = async_sys.create_promise();
        let next_future = promise.future();

        // Spawn a watcher on a worker that waits for this future,
        // then enqueues the continuation on the main thread
        async_sys.task_processor.start_task(Box::new(move || {
            let result = self.wait();
            let main_queue = async_sys.main_queue.clone();
            main_queue.enqueue(Box::new(move || match result {
                Ok(value) => promise.resolve(f(value)),
                Err(e) => promise.reject(e),
            }));
        }));

        next_future
    }

    /// Chain a continuation that runs immediately (inline).
    ///
    /// The continuation runs in whatever thread resolves the current future.
    pub fn then_immediately<U, F>(self, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> U + Send + 'static,
    {
        let async_sys = self.async_system.clone();
        let mut promise = async_sys.create_promise();
        let future = promise.future();

        // We can't run inline without a watcher thread, so use a worker
        async_sys
            .task_processor
            .start_task(Box::new(move || match self.wait() {
                Ok(value) => promise.resolve(f(value)),
                Err(e) => promise.reject(e),
            }));

        future
    }

    /// Chain an error handler that runs on the main thread.
    ///
    /// If this future rejects, `f` is called with the error string and can
    /// produce a recovery value.
    pub fn catch_in_main_thread<F>(self, f: F) -> Future<T>
    where
        F: FnOnce(AsyncError) -> T + Send + 'static,
    {
        let async_sys = self.async_system.clone();
        let mut promise = async_sys.create_promise();
        let next_future = promise.future();

        async_sys.task_processor.start_task(Box::new(move || {
            let result = self.wait();
            let main_queue = async_sys.main_queue.clone();
            main_queue.enqueue(Box::new(move || match result {
                Ok(value) => promise.resolve(value),
                Err(e) => promise.resolve(f(e)),
            }));
        }));

        next_future
    }

    /// Chain an error handler that runs immediately (inline from whichever
    /// thread resolves the future).
    pub fn catch_immediately<F>(self, f: F) -> Future<T>
    where
        F: FnOnce(AsyncError) -> T + Send + 'static,
    {
        let async_sys = self.async_system.clone();
        let mut promise = async_sys.create_promise();
        let next_future = promise.future();

        async_sys
            .task_processor
            .start_task(Box::new(move || match self.wait() {
                Ok(value) => promise.resolve(value),
                Err(e) => promise.resolve(f(e)),
            }));

        next_future
    }

    /// Convert to a [`SharedFuture<T>`] that can be cloned.
    ///
    /// The original future is consumed. Multiple consumers can wait on
    /// or chain continuations from the shared future.
    pub fn share(self) -> SharedFuture<T>
    where
        T: Clone,
    {
        let async_system = self.async_system.clone();
        let inner = Arc::new(SharedFutureInner {
            result: Mutex::new(None),
            condvar: Condvar::new(),
            async_system: async_system.clone(),
        });

        // Spawn a resolver that waits on the source and broadcasts the result
        let inner_clone = Arc::clone(&inner);
        async_system.task_processor.start_task(Box::new(move || {
            let result = self.wait();
            let mut slot = inner_clone
                .result
                .lock()
                .expect("shared future lock poisoned");
            *slot = Some(result);
            inner_clone.condvar.notify_all();
        }));

        SharedFuture { inner }
    }
}

/// Internal shared state for SharedFuture.
struct SharedFutureInner<T: Send + 'static> {
    result: Mutex<Option<Result<T, AsyncError>>>,
    condvar: Condvar,
    async_system: AsyncSystem,
}

/// A future that can be shared (cloned) among multiple consumers.
///
/// Created via [`Future::share`]. Each consumer can call [`wait`](SharedFuture::wait)
/// or [`is_ready`](SharedFuture::is_ready) independently.
pub struct SharedFuture<T: Clone + Send + 'static> {
    inner: Arc<SharedFutureInner<T>>,
}

impl<T: Clone + Send + 'static> Clone for SharedFuture<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T: Clone + Send + 'static> SharedFuture<T> {
    /// Check if the result is available.
    pub fn is_ready(&self) -> bool {
        self.inner
            .result
            .lock()
            .expect("shared future lock poisoned")
            .is_some()
    }

    /// Block until the shared result is available (clones the result).
    pub fn wait(&self) -> Result<T, AsyncError> {
        let mut guard = self
            .inner
            .result
            .lock()
            .expect("shared future lock poisoned");
        while guard.is_none() {
            guard = self
                .inner
                .condvar
                .wait(guard)
                .expect("shared future condvar poisoned");
        }
        guard
            .as_ref()
            .expect("guard is Some after wait loop")
            .clone()
    }

    /// Block until the result is available, while pumping the main-thread queue.
    ///
    /// Safe to call from the main thread. Mirrors `Future::wait_in_main_thread`.
    pub fn wait_in_main_thread(&self) -> Result<T, AsyncError> {
        loop {
            {
                let guard = self
                    .inner
                    .result
                    .lock()
                    .expect("shared future lock poisoned");
                if let Some(ref result) = *guard {
                    return result.clone();
                }
            }
            if !self.inner.async_system.main_queue.dispatch_one() {
                std::thread::yield_now();
            }
        }
    }

    /// Chain a continuation that runs in a worker thread.
    pub fn then_in_worker_thread<U, F>(&self, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> U + Send + 'static,
    {
        let async_sys = self.inner.async_system.clone();
        let mut promise = async_sys.create_promise();
        let future = promise.future();
        let shared = self.clone();

        async_sys
            .task_processor
            .start_task(Box::new(move || match shared.wait() {
                Ok(value) => promise.resolve(f(value)),
                Err(e) => promise.reject(e),
            }));

        future
    }

    /// Chain a continuation that runs on the main thread.
    pub fn then_in_main_thread<U, F>(&self, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> U + Send + 'static,
    {
        let async_sys = self.inner.async_system.clone();
        let mut promise = async_sys.create_promise();
        let next_future = promise.future();
        let shared = self.clone();

        async_sys.task_processor.start_task(Box::new(move || {
            let result = shared.wait();
            let main_queue = async_sys.main_queue.clone();
            main_queue.enqueue(Box::new(move || match result {
                Ok(value) => promise.resolve(f(value)),
                Err(e) => promise.reject(e),
            }));
        }));

        next_future
    }

    /// Chain a continuation that runs immediately (inline).
    pub fn then_immediately<U, F>(&self, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> U + Send + 'static,
    {
        let async_sys = self.inner.async_system.clone();
        let mut promise = async_sys.create_promise();
        let future = promise.future();
        let shared = self.clone();

        async_sys
            .task_processor
            .start_task(Box::new(move || match shared.wait() {
                Ok(value) => promise.resolve(f(value)),
                Err(e) => promise.reject(e),
            }));

        future
    }

    /// Chain an error handler that runs on the main thread.
    pub fn catch_in_main_thread<F>(&self, f: F) -> Future<T>
    where
        F: FnOnce(AsyncError) -> T + Send + 'static,
    {
        let async_sys = self.inner.async_system.clone();
        let mut promise = async_sys.create_promise();
        let next_future = promise.future();
        let shared = self.clone();

        async_sys.task_processor.start_task(Box::new(move || {
            let result = shared.wait();
            let main_queue = async_sys.main_queue.clone();
            main_queue.enqueue(Box::new(move || match result {
                Ok(value) => promise.resolve(value),
                Err(e) => promise.resolve(f(e)),
            }));
        }));

        next_future
    }

    /// Chain an error handler that runs immediately.
    pub fn catch_immediately<F>(&self, f: F) -> Future<T>
    where
        F: FnOnce(AsyncError) -> T + Send + 'static,
    {
        let async_sys = self.inner.async_system.clone();
        let mut promise = async_sys.create_promise();
        let next_future = promise.future();
        let shared = self.clone();

        async_sys
            .task_processor
            .start_task(Box::new(move || match shared.wait() {
                Ok(value) => promise.resolve(value),
                Err(e) => promise.resolve(f(e)),
            }));

        next_future
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ThreadPoolTaskProcessor;

    fn test_async_system() -> AsyncSystem {
        AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(2)))
    }

    #[test]
    fn promise_resolve() {
        let sys = test_async_system();
        let mut promise = sys.create_promise::<i32>();
        let future = promise.future();
        promise.resolve(42);
        assert_eq!(future.wait().unwrap(), 42);
    }

    #[test]
    fn promise_reject() {
        let sys = test_async_system();
        let mut promise = sys.create_promise::<i32>();
        let future = promise.future();
        promise.reject(AsyncError::msg("oops"));
        let result = future.wait();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "oops");
    }

    #[test]
    fn promise_drop_rejects() {
        let sys = test_async_system();
        let mut promise = sys.create_promise::<i32>();
        let future = promise.future();
        drop(promise);
        assert!(future.wait().is_err());
    }

    #[test]
    fn run_in_worker_thread() {
        let sys = test_async_system();
        let future = sys.run_in_worker_thread(|| 2 + 2);
        assert_eq!(future.wait().unwrap(), 4);
    }

    #[test]
    fn run_in_main_thread() {
        let sys = test_async_system();
        let future = sys.run_in_main_thread(|| 7);
        // Must dispatch to get the result
        assert!(!future.is_ready());
        sys.dispatch_main_thread_tasks();
        assert!(future.is_ready());
        assert_eq!(future.wait().unwrap(), 7);
    }

    #[test]
    fn then_in_worker_thread() {
        let sys = test_async_system();
        let future = sys
            .run_in_worker_thread(|| 10)
            .then_in_worker_thread(|v| v * 2);
        assert_eq!(future.wait().unwrap(), 20);
    }

    #[test]
    fn then_in_main_thread_chain() {
        let sys = test_async_system();
        let future = sys
            .run_in_worker_thread(|| 5)
            .then_in_main_thread(|v| v + 1);

        // The continuation won't run until we dispatch
        // Give the worker time to finish
        std::thread::sleep(std::time::Duration::from_millis(50));
        sys.dispatch_main_thread_tasks();
        assert_eq!(future.wait().unwrap(), 6);
    }

    #[test]
    fn wait_in_main_thread_pumps_queue() {
        let sys = test_async_system();
        let sys2 = sys.clone();
        let future = sys.run_in_worker_thread(move || {
            // Enqueue a main-thread task that contributes to the result
            let mut promise = sys2.create_promise::<i32>();
            let f = promise.future();
            sys2.main_queue().enqueue(Box::new(move || {
                promise.resolve(99);
            }));
            f.wait_in_main_thread().unwrap()
        });
        // wait_in_main_thread won't work here since future.wait consumes
        // but demonstrates the pattern
        assert_eq!(future.wait().unwrap(), 99);
    }

    #[test]
    fn is_ready_polling() {
        let sys = test_async_system();
        let mut promise = sys.create_promise::<i32>();
        let future = promise.future();
        assert!(!future.is_ready());
        promise.resolve(1);
        // May need a moment for channel to deliver
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(future.is_ready());
    }

    #[test]
    fn catch_in_main_thread_recovers() {
        let sys = test_async_system();
        let mut promise = sys.create_promise::<i32>();
        let future = promise.future();
        promise.reject(AsyncError::msg("fail"));

        let recovered = future.catch_in_main_thread(|_err| -1);
        // Need to dispatch main thread for the catch to run
        std::thread::sleep(std::time::Duration::from_millis(50));
        sys.dispatch_main_thread_tasks();
        assert_eq!(recovered.wait().unwrap(), -1);
    }

    #[test]
    fn create_resolved_future() {
        let sys = test_async_system();
        let future = sys.create_resolved_future(42);
        assert!(future.is_ready());
        assert_eq!(future.wait().unwrap(), 42);
    }

    #[test]
    fn all_futures_success() {
        let sys = test_async_system();
        let futures = vec![
            sys.run_in_worker_thread(|| 1),
            sys.run_in_worker_thread(|| 2),
            sys.run_in_worker_thread(|| 3),
        ];
        let combined = sys.all(futures);
        assert_eq!(combined.wait().unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn all_futures_one_fails() {
        let sys = test_async_system();
        let mut promise = sys.create_promise::<i32>();
        let bad = promise.future();
        promise.reject(AsyncError::msg("boom"));
        let futures = vec![
            sys.run_in_worker_thread(|| 1),
            bad,
            sys.run_in_worker_thread(|| 3),
        ];
        let combined = sys.all(futures);
        assert!(combined.wait().is_err());
    }

    #[test]
    fn catch_immediately_recovers() {
        let sys = test_async_system();
        let mut promise = sys.create_promise::<i32>();
        let future = promise.future();
        promise.reject(AsyncError::msg("fail"));
        let recovered = future.catch_immediately(|_err| -99);
        assert_eq!(recovered.wait().unwrap(), -99);
    }

    #[test]
    fn shared_future_wait() {
        let sys = test_async_system();
        let future = sys.run_in_worker_thread(|| 42);
        let shared = future.share();
        let s2 = shared.clone();

        // Both clones should get the same result
        assert_eq!(shared.wait().unwrap(), 42);
        assert_eq!(s2.wait().unwrap(), 42);
    }

    #[test]
    fn shared_future_then_in_worker() {
        let sys = test_async_system();
        let future = sys.run_in_worker_thread(|| 10);
        let shared = future.share();
        let doubled = shared.then_in_worker_thread(|v| v * 2);
        assert_eq!(doubled.wait().unwrap(), 20);
    }

    #[test]
    fn shared_future_is_ready() {
        let sys = test_async_system();
        let mut promise = sys.create_promise::<i32>();
        let future = promise.future();
        let shared = future.share();
        assert!(!shared.is_ready());
        promise.resolve(1);
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(shared.is_ready());
    }

    #[test]
    fn create_resolved_future_void() {
        let sys = test_async_system();
        let future = sys.create_resolved_future_void();
        assert!(future.is_ready());
        assert!(future.wait().is_ok());
    }

    #[test]
    fn create_future_resolves() {
        let sys = test_async_system();
        let future = sys.create_future::<i32, _>(|mut p| p.resolve(99));
        assert!(future.is_ready());
        assert_eq!(future.wait().unwrap(), 99);
    }

    #[test]
    fn create_future_panic_rejects() {
        let sys = test_async_system();
        let future = sys.create_future::<i32, _>(|_p| panic!("intentional panic"));
        assert!(future.is_ready());
        assert!(future.wait().is_err());
    }

    #[test]
    fn all_shared_futures() {
        let sys = test_async_system();
        let shared_futures = vec![
            sys.run_in_worker_thread(|| 10).share(),
            sys.run_in_worker_thread(|| 20).share(),
            sys.run_in_worker_thread(|| 30).share(),
        ];
        let combined = sys.all_shared(shared_futures);
        assert_eq!(combined.wait().unwrap(), vec![10, 20, 30]);
    }

    #[test]
    fn all_tuple_macro() {
        let sys = test_async_system();
        let fa = sys.run_in_worker_thread(|| 1i32);
        let fb = sys.run_in_worker_thread(|| "hello");
        let fc = sys.run_in_worker_thread(|| true);
        let combined = all_tuple!(sys, fa, fb, fc);
        assert_eq!(combined.wait().unwrap(), (1i32, "hello", true));
    }

    #[test]
    fn all_tuple_macro_first_error_wins() {
        let sys = test_async_system();
        let mut p = sys.create_promise::<i32>();
        let bad = p.future();
        p.reject(AsyncError::msg("boom"));
        let good = sys.run_in_worker_thread(|| 99i32);
        let combined = all_tuple!(sys, bad, good);
        let err = combined.wait().unwrap_err();
        assert_eq!(err.to_string(), "boom");
    }

    #[test]
    fn enter_main_thread_inline_execution() {
        let sys = test_async_system();
        // Without a scope, run_in_main_thread queues the work.
        let future = sys.run_in_main_thread(|| 7);
        assert!(!future.is_ready());
        sys.dispatch_main_thread_tasks();
        assert_eq!(future.wait().unwrap(), 7);

        // With a scope, run_in_main_thread executes inline immediately.
        let _scope = sys.enter_main_thread();
        let future2 = sys.run_in_main_thread(|| 42);
        assert!(future2.is_ready());
        assert_eq!(future2.wait().unwrap(), 42);
    }

    #[test]
    fn enter_main_thread_nested_scopes() {
        let sys = test_async_system();
        {
            let _s1 = sys.enter_main_thread();
            {
                let _s2 = sys.enter_main_thread();
                assert!(is_main_thread());
            }
            // s2 dropped — still inside s1, so still main thread
            assert!(is_main_thread());
        }
        // s1 dropped — no longer main thread
        assert!(!is_main_thread());
    }

    #[test]
    fn async_system_equality() {
        let sys = test_async_system();
        let sys2 = sys.clone();
        assert_eq!(sys, sys2);

        let sys3 = test_async_system(); // different processor + queue
        assert_ne!(sys, sys3);
    }
}
