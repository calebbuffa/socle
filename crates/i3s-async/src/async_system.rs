//! Async system: Future/Promise types and main-thread task dispatch.
//!
//! Mirrors cesium-native's `CesiumAsync` library. Provides:
//! - [`AsyncSystem`] — scheduler owner with main-thread queue + worker pool
//! - [`Future<T>`] — one-shot async result with continuation chaining
//! - [`Promise<T>`] — resolve/reject handle paired with a Future
//! - [`SharedFuture<T>`] — cloneable future for multiple consumers
//!
//! Unlike C++ (which lacks async/await), Rust has native async. However,
//! this explicit Future/Promise system is needed at the library boundary:
//! - Python bindings wrap these types directly (no Rust async runtime exposed)
//! - The main-thread queue enables `prepare_in_main_thread` callbacks
//! - Continuation chaining (`then_in_worker_thread`, `then_in_main_thread`)
//!   mirrors cesium-native's API 1:1

use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};

use crate::task_processor::TaskProcessor;

// ============================================================================
// MainThreadQueue — queued callbacks for dispatch_main_thread_tasks()
// ============================================================================

type MainThreadTask = Box<dyn FnOnce() + Send>;

/// A queue of tasks to execute on the main thread.
///
/// Workers enqueue via [`enqueue`](MainThreadQueue::enqueue). The main thread
/// drains via [`dispatch`](MainThreadQueue::dispatch). This mirrors
/// cesium-native's `QueuedScheduler::dispatchQueuedContinuations()`.
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
        self.queue.lock().unwrap().push(task);
        self.condvar.notify_one();
    }

    /// Dispatch all pending main-thread tasks. Non-blocking.
    ///
    /// Returns the number of tasks executed.
    pub fn dispatch(&self) -> usize {
        let tasks: Vec<MainThreadTask> = {
            let mut q = self.queue.lock().unwrap();
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
            let mut q = self.queue.lock().unwrap();
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
        !self.queue.lock().unwrap().is_empty()
    }

    /// Block until a task is available, then dispatch it.
    /// Used by `wait_in_main_thread` to pump the queue while waiting.
    fn dispatch_blocking(&self) {
        let task = {
            let mut q = self.queue.lock().unwrap();
            while q.is_empty() {
                q = self.condvar.wait(q).unwrap();
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

// ============================================================================
// AsyncSystem — top-level scheduler owner
// ============================================================================

/// The async system: owns the worker thread pool and main-thread queue.
///
/// Mirrors cesium-native's `AsyncSystem`. Create one per application and
/// share it (via clone — it's internally reference-counted) with all
/// subsystems that need async work.
///
/// ## Usage
///
/// ```ignore
/// let async_system = AsyncSystem::new(task_processor);
///
/// // Create a promise/future pair
/// let (promise, future) = async_system.create_promise::<String>();
///
/// // Resolve from a worker thread
/// async_system.run_in_worker_thread(move || {
///     promise.resolve(Ok("hello".into()));
/// });
///
/// // Wait for result
/// let value = future.wait();
/// ```
#[derive(Clone)]
pub struct AsyncSystem {
    task_processor: Arc<dyn TaskProcessor>,
    main_queue: Arc<MainThreadQueue>,
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

    /// Create a Promise/Future pair.
    ///
    /// The [`Promise`] is used to resolve or reject the computation.
    /// The [`Future`] is used to receive the result.
    pub fn create_promise<T: Send + 'static>(&self) -> (Promise<T>, Future<T>) {
        let (tx, rx) = mpsc::sync_channel(1);
        let promise = Promise { sender: Some(tx) };
        let future = Future {
            state: Mutex::new(FutureState::Pending(rx)),
            async_system: self.clone(),
        };
        (promise, future)
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
        let (promise, future) = self.create_promise();
        self.task_processor.start_task(Box::new(move || {
            promise.resolve(f());
        }));
        future
    }

    /// Run a closure on the main thread (queued), returning a Future.
    ///
    /// The closure will execute the next time
    /// [`dispatch_main_thread_tasks`](Self::dispatch_main_thread_tasks) is called.
    pub fn run_in_main_thread<T, F>(&self, f: F) -> Future<T>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let (promise, future) = self.create_promise();
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

    /// Create a future that is already resolved with a value.
    ///
    /// Mirrors cesium-native `AsyncSystem::createResolvedFuture`.
    pub fn create_resolved_future<T: Send + 'static>(&self, value: T) -> Future<T> {
        Future {
            state: Mutex::new(FutureState::Ready(Ok(value))),
            async_system: self.clone(),
        }
    }

    /// Wait for all futures in a vector, returning a vector of results.
    ///
    /// If any future rejects, the entire result is an error.
    /// Mirrors cesium-native `AsyncSystem::all`.
    pub fn all<T: Send + 'static>(&self, futures: Vec<Future<T>>) -> Future<Vec<T>> {
        let async_sys = self.clone();
        let (promise, combined_future) = self.create_promise();
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
        let _ = async_sys; // keep async_sys alive (clone is just for the future)
        combined_future
    }
}

// ============================================================================
// Promise<T> — resolve/reject handle
// ============================================================================

/// A promise that can be resolved with a value of type `T`.
///
/// Mirrors cesium-native's `Promise<T>`. Created via
/// [`AsyncSystem::create_promise`]. Paired with a [`Future<T>`].
///
/// Calling [`resolve`](Promise::resolve) or [`reject`](Promise::reject)
/// delivers the result to the associated Future.
pub struct Promise<T: Send + 'static> {
    sender: Option<mpsc::SyncSender<Result<T, String>>>,
}

impl<T: Send + 'static> Promise<T> {
    /// Resolve the promise with a value.
    pub fn resolve(mut self, value: T) {
        if let Some(tx) = self.sender.take() {
            let _ = tx.send(Ok(value));
        }
    }

    /// Reject the promise with an error message.
    pub fn reject(mut self, error: String) {
        if let Some(tx) = self.sender.take() {
            let _ = tx.send(Err(error));
        }
    }
}

impl<T: Send + 'static> Drop for Promise<T> {
    fn drop(&mut self) {
        // If the promise is dropped without resolving, reject it
        if let Some(tx) = self.sender.take() {
            let _ = tx.send(Err("Promise dropped without resolving".into()));
        }
    }
}

// ============================================================================
// Future<T> — async result handle
// ============================================================================

/// Internal state of a Future.
enum FutureState<T> {
    /// Waiting for a result from the paired Promise.
    Pending(mpsc::Receiver<Result<T, String>>),
    /// Resolved with a value.
    Ready(Result<T, String>),
    /// Already consumed (wait/then was called).
    Consumed,
}

/// A future that resolves to a value of type `T`.
///
/// Mirrors cesium-native's `Future<T>`. Created via
/// [`AsyncSystem::create_promise`] or [`AsyncSystem::run_in_worker_thread`].
///
/// Supports:
/// - [`wait`](Future::wait) — blocking wait (not from main thread)
/// - [`wait_in_main_thread`](Future::wait_in_main_thread) — blocking wait
///   while pumping the main-thread queue
/// - [`is_ready`](Future::is_ready) — non-blocking poll
/// - [`then_in_worker_thread`](Future::then_in_worker_thread) — chain a
///   continuation on a worker thread
/// - [`then_in_main_thread`](Future::then_in_main_thread) — chain a
///   continuation on the main thread
pub struct Future<T: Send + 'static> {
    state: Mutex<FutureState<T>>,
    async_system: AsyncSystem,
}

// Safety: Future<T> is Send+Sync because T is Send and state is behind a Mutex
unsafe impl<T: Send + 'static> Send for Future<T> {}
unsafe impl<T: Send + 'static> Sync for Future<T> {}

impl<T: Send + 'static> Future<T> {
    /// Try to transition from Pending → Ready by polling the channel.
    fn try_resolve(&self) {
        let mut state = self.state.lock().unwrap();
        if let FutureState::Pending(ref rx) = *state {
            match rx.try_recv() {
                Ok(result) => *state = FutureState::Ready(result),
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    *state = FutureState::Ready(Err(
                        "Future channel closed without result".into(),
                    ));
                }
            }
        }
    }

    /// Block until the result is available.
    ///
    /// **Warning**: Do not call this from the main thread if the future's
    /// resolution depends on main-thread work. Use
    /// [`wait_in_main_thread`](Self::wait_in_main_thread) instead.
    pub fn wait(self) -> Result<T, String> {
        let mut state = self.state.into_inner().unwrap();
        match std::mem::replace(&mut state, FutureState::Consumed) {
            FutureState::Pending(rx) => match rx.recv() {
                Ok(result) => result,
                Err(_) => Err("Future channel closed without result".into()),
            },
            FutureState::Ready(result) => result,
            FutureState::Consumed => Err("Future already consumed".into()),
        }
    }

    /// Block until the result is available, while pumping the main-thread queue.
    ///
    /// This is safe to call from the main thread. It dispatches queued
    /// main-thread tasks while waiting, preventing deadlocks.
    /// Mirrors `Future::waitInMainThread`.
    pub fn wait_in_main_thread(self) -> Result<T, String> {
        loop {
            {
                let mut state = self.state.lock().unwrap();
                if let FutureState::Pending(ref rx) = *state {
                    match rx.try_recv() {
                        Ok(result) => {
                            *state = FutureState::Consumed;
                            return result;
                        }
                        Err(mpsc::TryRecvError::Disconnected) => {
                            *state = FutureState::Consumed;
                            return Err("Future channel closed without result".into());
                        }
                        Err(mpsc::TryRecvError::Empty) => {}
                    }
                } else {
                    match std::mem::replace(&mut *state, FutureState::Consumed) {
                        FutureState::Ready(result) => return result,
                        FutureState::Consumed => return Err("Future already consumed".into()),
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
        let state = self.state.lock().unwrap();
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
        let (promise, future) = async_sys.create_promise();

        async_sys.task_processor.start_task(Box::new(move || {
            match self.wait() {
                Ok(value) => promise.resolve(f(value)),
                Err(e) => promise.reject(e),
            }
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
        let (promise, next_future) = async_sys.create_promise();

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
        let (promise, future) = async_sys.create_promise();

        // We can't run inline without a watcher thread, so use a worker
        async_sys.task_processor.start_task(Box::new(move || {
            match self.wait() {
                Ok(value) => promise.resolve(f(value)),
                Err(e) => promise.reject(e),
            }
        }));

        future
    }

    /// Chain an error handler that runs on the main thread.
    ///
    /// If this future rejects, `f` is called with the error string and can
    /// produce a recovery value.
    pub fn catch_in_main_thread<F>(self, f: F) -> Future<T>
    where
        F: FnOnce(String) -> T + Send + 'static,
    {
        let async_sys = self.async_system.clone();
        let (promise, next_future) = async_sys.create_promise();

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
    ///
    /// Mirrors cesium-native `Future::catchImmediately`.
    pub fn catch_immediately<F>(self, f: F) -> Future<T>
    where
        F: FnOnce(String) -> T + Send + 'static,
    {
        let async_sys = self.async_system.clone();
        let (promise, next_future) = async_sys.create_promise();

        async_sys.task_processor.start_task(Box::new(move || {
            match self.wait() {
                Ok(value) => promise.resolve(value),
                Err(e) => promise.resolve(f(e)),
            }
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
            let mut slot = inner_clone.result.lock().unwrap();
            *slot = Some(result);
            inner_clone.condvar.notify_all();
        }));

        SharedFuture { inner }
    }
}

// ============================================================================
// SharedFuture<T> — cloneable future
// ============================================================================

/// Internal shared state for SharedFuture.
struct SharedFutureInner<T: Send + 'static> {
    result: Mutex<Option<Result<T, String>>>,
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
        self.inner.result.lock().unwrap().is_some()
    }

    /// Block until the shared result is available (clones the result).
    pub fn wait(&self) -> Result<T, String> {
        let mut guard = self.inner.result.lock().unwrap();
        while guard.is_none() {
            guard = self.inner.condvar.wait(guard).unwrap();
        }
        guard.as_ref().unwrap().clone()
    }

    /// Block until the result is available, while pumping the main-thread queue.
    ///
    /// Safe to call from the main thread. Mirrors `Future::wait_in_main_thread`.
    pub fn wait_in_main_thread(&self) -> Result<T, String> {
        loop {
            {
                let guard = self.inner.result.lock().unwrap();
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
        let (promise, future) = async_sys.create_promise();
        let shared = self.clone();

        async_sys.task_processor.start_task(Box::new(move || {
            match shared.wait() {
                Ok(value) => promise.resolve(f(value)),
                Err(e) => promise.reject(e),
            }
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
        let (promise, next_future) = async_sys.create_promise();
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
        let (promise, future) = async_sys.create_promise();
        let shared = self.clone();

        async_sys.task_processor.start_task(Box::new(move || {
            match shared.wait() {
                Ok(value) => promise.resolve(f(value)),
                Err(e) => promise.reject(e),
            }
        }));

        future
    }

    /// Chain an error handler that runs on the main thread.
    pub fn catch_in_main_thread<F>(&self, f: F) -> Future<T>
    where
        F: FnOnce(String) -> T + Send + 'static,
    {
        let async_sys = self.inner.async_system.clone();
        let (promise, next_future) = async_sys.create_promise();
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
        F: FnOnce(String) -> T + Send + 'static,
    {
        let async_sys = self.inner.async_system.clone();
        let (promise, next_future) = async_sys.create_promise();
        let shared = self.clone();

        async_sys.task_processor.start_task(Box::new(move || {
            match shared.wait() {
                Ok(value) => promise.resolve(value),
                Err(e) => promise.resolve(f(e)),
            }
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
        let (promise, future) = sys.create_promise::<i32>();
        promise.resolve(42);
        assert_eq!(future.wait(), Ok(42));
    }

    #[test]
    fn promise_reject() {
        let sys = test_async_system();
        let (promise, future) = sys.create_promise::<i32>();
        promise.reject("oops".into());
        assert_eq!(future.wait(), Err("oops".into()));
    }

    #[test]
    fn promise_drop_rejects() {
        let sys = test_async_system();
        let (promise, future) = sys.create_promise::<i32>();
        drop(promise);
        assert!(future.wait().is_err());
    }

    #[test]
    fn run_in_worker_thread() {
        let sys = test_async_system();
        let future = sys.run_in_worker_thread(|| 2 + 2);
        assert_eq!(future.wait(), Ok(4));
    }

    #[test]
    fn run_in_main_thread() {
        let sys = test_async_system();
        let future = sys.run_in_main_thread(|| 7);
        // Must dispatch to get the result
        assert!(!future.is_ready());
        sys.dispatch_main_thread_tasks();
        assert!(future.is_ready());
        assert_eq!(future.wait(), Ok(7));
    }

    #[test]
    fn then_in_worker_thread() {
        let sys = test_async_system();
        let future = sys
            .run_in_worker_thread(|| 10)
            .then_in_worker_thread(|v| v * 2);
        assert_eq!(future.wait(), Ok(20));
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
        assert_eq!(future.wait(), Ok(6));
    }

    #[test]
    fn wait_in_main_thread_pumps_queue() {
        let sys = test_async_system();
        let sys2 = sys.clone();
        let future = sys
            .run_in_worker_thread(move || {
                // Enqueue a main-thread task that contributes to the result
                let (promise, f) = sys2.create_promise::<i32>();
                sys2.main_queue().enqueue(Box::new(move || {
                    promise.resolve(99);
                }));
                f.wait_in_main_thread().unwrap()
            });
        // wait_in_main_thread won't work here since future.wait consumes
        // but demonstrates the pattern
        assert_eq!(future.wait(), Ok(99));
    }

    #[test]
    fn is_ready_polling() {
        let sys = test_async_system();
        let (promise, future) = sys.create_promise::<i32>();
        assert!(!future.is_ready());
        promise.resolve(1);
        // May need a moment for channel to deliver
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(future.is_ready());
    }

    #[test]
    fn catch_in_main_thread_recovers() {
        let sys = test_async_system();
        let (promise, future) = sys.create_promise::<i32>();
        promise.reject("fail".into());

        let recovered = future.catch_in_main_thread(|_err| -1);
        // Need to dispatch main thread for the catch to run
        std::thread::sleep(std::time::Duration::from_millis(50));
        sys.dispatch_main_thread_tasks();
        assert_eq!(recovered.wait(), Ok(-1));
    }

    #[test]
    fn create_resolved_future() {
        let sys = test_async_system();
        let future = sys.create_resolved_future(42);
        assert!(future.is_ready());
        assert_eq!(future.wait(), Ok(42));
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
        assert_eq!(combined.wait(), Ok(vec![1, 2, 3]));
    }

    #[test]
    fn all_futures_one_fails() {
        let sys = test_async_system();
        let (promise, bad) = sys.create_promise::<i32>();
        promise.reject("boom".into());
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
        let (promise, future) = sys.create_promise::<i32>();
        promise.reject("fail".into());
        let recovered = future.catch_immediately(|_err| -99);
        assert_eq!(recovered.wait(), Ok(-99));
    }

    #[test]
    fn shared_future_wait() {
        let sys = test_async_system();
        let future = sys.run_in_worker_thread(|| 42);
        let shared = future.share();
        let s2 = shared.clone();

        // Both clones should get the same result
        assert_eq!(shared.wait(), Ok(42));
        assert_eq!(s2.wait(), Ok(42));
    }

    #[test]
    fn shared_future_then_in_worker() {
        let sys = test_async_system();
        let future = sys.run_in_worker_thread(|| 10);
        let shared = future.share();
        let doubled = shared.then_in_worker_thread(|v| v * 2);
        assert_eq!(doubled.wait(), Ok(20));
    }

    #[test]
    fn shared_future_is_ready() {
        let sys = test_async_system();
        let (promise, future) = sys.create_promise::<i32>();
        let shared = future.share();
        assert!(!shared.is_ready());
        promise.resolve(1);
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(shared.is_ready());
    }
}
