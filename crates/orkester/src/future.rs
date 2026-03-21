use crate::context::Context;
use crate::error::AsyncError;
use crate::promise::Promise;
use crate::state::SharedState;
use crate::system::{AsyncSystem, SchedulerHandle};
use crate::thread_pool::ThreadPool;
use std::fmt::{self, Debug, Formatter};
use std::future::Future as StdFuture;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;

/// Single-consumer future value.
pub struct Future<T: Send + 'static> {
    pub(crate) system: AsyncSystem,
    pub(crate) state: Option<Arc<SharedState<T>>>,
}

impl<T: Send + 'static> Debug for Future<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Future")
            .field("system", &(Arc::as_ptr(&self.system.inner) as usize))
            .field(
                "state",
                &self.state.as_ref().map(|state| Arc::as_ptr(state) as usize),
            )
            .finish()
    }
}

pub(crate) fn create_pair<T: Send + 'static>(system: AsyncSystem) -> (Promise<T>, Future<T>) {
    let state = Arc::new(SharedState::new_pending());
    let promise = Promise::new(Arc::clone(&state));
    let future = Future {
        system,
        state: Some(state),
    };
    (promise, future)
}

#[doc(hidden)]
pub trait ResolveOutput<T: Send + 'static>: Send + 'static {
    fn resolve_into(self, promise: Promise<T>);
}

impl<T> ResolveOutput<T> for T
where
    T: Send + 'static,
{
    fn resolve_into(self, promise: Promise<T>) {
        promise.resolve(self);
    }
}

impl<T> ResolveOutput<T> for Future<T>
where
    T: Send + 'static,
{
    fn resolve_into(self, promise: Promise<T>) {
        self.pipe_to_promise(promise);
    }
}

impl<T: Send + 'static> Future<T> {
    fn consumed_error() -> AsyncError {
        AsyncError::msg("Future already consumed")
    }

    fn rejected_future<U: Send + 'static>(system: AsyncSystem, error: AsyncError) -> Future<U> {
        let (promise, future) = create_pair(system);
        promise.reject(error);
        future
    }

    fn pipe_to_promise(mut self, promise: Promise<T>) {
        let source = match self.state.take() {
            Some(state) => state,
            None => {
                promise.reject(Self::consumed_error());
                return;
            }
        };

        let source_for_continuation = Arc::clone(&source);
        source.register_continuation(Box::new(move || {
            match source_for_continuation.take_result() {
                Some(Ok(value)) => promise.resolve(value),
                Some(Err(error)) => promise.reject(error),
                None => promise.reject(Self::consumed_error()),
            }
        }));
    }

    pub fn is_ready(&self) -> bool {
        self.state.as_ref().is_some_and(|state| state.is_ready())
    }

    pub fn wait(mut self) -> Result<T, AsyncError> {
        let state = self.state.take().ok_or_else(Self::consumed_error)?;
        state.wait_until_ready();
        state
            .take_result()
            .unwrap_or_else(|| Err(Self::consumed_error()))
    }

    pub fn wait_in_main_thread(mut self) -> Result<T, AsyncError> {
        let state = self.state.take().ok_or_else(Self::consumed_error)?;

        while !state.is_ready() {
            if !self.system.inner.main_queue.dispatch_one() {
                state.wait_timeout(Duration::from_millis(2));
            }
        }

        state
            .take_result()
            .unwrap_or_else(|| Err(Self::consumed_error()))
    }

    /// Access the async system that owns this future.
    pub fn system(&self) -> AsyncSystem {
        self.system.clone()
    }

    fn then_with_scheduler<U, F, R>(mut self, scheduler: SchedulerHandle, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        let system = self.system.clone();
        let source = match self.state.take() {
            Some(state) => state,
            None => return Self::rejected_future(system, Self::consumed_error()),
        };

        let (promise, next_future) = create_pair::<U>(system);
        let source_for_continuation = Arc::clone(&source);
        source.register_continuation(Box::new(move || {
            match source_for_continuation.take_result() {
                Some(Ok(value)) => {
                    let run = move || {
                        f(value).resolve_into(promise);
                    };
                    if scheduler.is_current_thread() {
                        run();
                    } else {
                        scheduler.schedule(Box::new(run));
                    }
                }
                Some(Err(error)) => promise.reject(error),
                None => promise.reject(Self::consumed_error()),
            }
        }));

        next_future
    }

    fn catch_with_scheduler<F>(mut self, scheduler: SchedulerHandle, f: F) -> Future<T>
    where
        F: FnOnce(AsyncError) -> T + Send + 'static,
    {
        let system = self.system.clone();
        let source = match self.state.take() {
            Some(state) => state,
            None => return Self::rejected_future(system, Self::consumed_error()),
        };

        let (promise, next_future) = create_pair::<T>(system);
        let source_for_continuation = Arc::clone(&source);
        source.register_continuation(Box::new(move || {
            match source_for_continuation.take_result() {
                Some(Ok(value)) => promise.resolve(value),
                Some(Err(error)) => {
                    let run = move || {
                        promise.resolve(f(error));
                    };
                    if scheduler.is_current_thread() {
                        run();
                    } else {
                        scheduler.schedule(Box::new(run));
                    }
                }
                None => promise.reject(Self::consumed_error()),
            }
        }));

        next_future
    }

    #[deprecated(since = "0.2.0", note = "use `then(Context::Worker, f)` instead")]
    pub fn then_in_worker_thread<U, F, R>(self, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        self.then(Context::Worker, f)
    }

    #[deprecated(since = "0.2.0", note = "use `then(Context::Main, f)` instead")]
    pub fn then_in_main_thread<U, F, R>(self, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        self.then(Context::Main, f)
    }

    #[deprecated(since = "0.2.0", note = "use `then_in_pool(pool, f)` instead")]
    pub fn then_in_thread_pool<U, F, R>(self, thread_pool: &ThreadPool, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        self.then_in_pool(thread_pool, f)
    }

    pub fn then_immediately<U, F, R>(self, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        self.then(Context::Immediate, f)
    }

    /// Chain a continuation in the given scheduling context.
    pub fn then<U, F, R>(self, context: Context, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        match self.system.inner.scheduler_for(context) {
            Some(scheduler) => self.then_with_scheduler(scheduler, f),
            None => self.then_immediate(f),
        }
    }

    /// Chain a continuation in a named thread pool.
    pub fn then_in_pool<U, F, R>(self, thread_pool: &ThreadPool, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        let scheduler = self.system.inner.thread_pool_scheduler(thread_pool);
        self.then_with_scheduler(scheduler, f)
    }

    fn then_immediate<U, F, R>(mut self, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        let system = self.system.clone();
        let source = match self.state.take() {
            Some(state) => state,
            None => return Self::rejected_future(system, Self::consumed_error()),
        };

        let (promise, next_future) = create_pair::<U>(system);
        let source_for_continuation = Arc::clone(&source);
        source.register_continuation(Box::new(move || {
            match source_for_continuation.take_result() {
                Some(Ok(value)) => f(value).resolve_into(promise),
                Some(Err(error)) => promise.reject(error),
                None => promise.reject(Self::consumed_error()),
            }
        }));

        next_future
    }

    #[deprecated(since = "0.2.0", note = "use `catch(Context::Main, f)` instead")]
    pub fn catch_in_main_thread<F>(self, f: F) -> Future<T>
    where
        F: FnOnce(AsyncError) -> T + Send + 'static,
    {
        self.catch(Context::Main, f)
    }

    pub fn catch_immediately<F>(self, f: F) -> Future<T>
    where
        F: FnOnce(AsyncError) -> T + Send + 'static,
    {
        self.catch(Context::Immediate, f)
    }

    /// Catch an error in the given scheduling context.
    pub fn catch<F>(self, context: Context, f: F) -> Future<T>
    where
        F: FnOnce(AsyncError) -> T + Send + 'static,
    {
        match self.system.inner.scheduler_for(context) {
            Some(scheduler) => self.catch_with_scheduler(scheduler, f),
            None => self.catch_immediate(f),
        }
    }

    fn catch_immediate<F>(mut self, f: F) -> Future<T>
    where
        F: FnOnce(AsyncError) -> T + Send + 'static,
    {
        let system = self.system.clone();
        let source = match self.state.take() {
            Some(state) => state,
            None => return Self::rejected_future(system, Self::consumed_error()),
        };

        let (promise, next_future) = create_pair::<T>(system);
        let source_for_continuation = Arc::clone(&source);
        source.register_continuation(Box::new(move || {
            match source_for_continuation.take_result() {
                Some(Ok(value)) => promise.resolve(value),
                Some(Err(error)) => promise.resolve(f(error)),
                None => promise.reject(Self::consumed_error()),
            }
        }));

        next_future
    }

    pub fn share(mut self) -> SharedFuture<T>
    where
        T: Clone,
    {
        let state = self.state.take().unwrap_or_else(|| {
            let state = Arc::new(SharedState::new_pending());
            state.reject(Self::consumed_error());
            state
        });

        SharedFuture::from_state(self.system.clone(), state)
    }
}

impl<T: Send + 'static> StdFuture for Future<T> {
    type Output = Result<T, AsyncError>;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let state = match this.state.as_ref() {
            Some(state) => Arc::clone(state),
            None => return Poll::Ready(Err(Self::consumed_error())),
        };

        if state.is_ready() {
            let result = state
                .take_result()
                .unwrap_or_else(|| Err(Self::consumed_error()));
            this.state = None;
            return Poll::Ready(result);
        }

        state.register_waker(cx.waker());
        Poll::Pending
    }
}

/// Cloneable multi-consumer future value.
#[derive(Clone)]
pub struct SharedFuture<T: Clone + Send + 'static> {
    pub(crate) system: AsyncSystem,
    pub(crate) state: Arc<SharedState<T>>,
}

impl<T: Clone + Send + 'static> Debug for SharedFuture<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("SharedFuture")
            .field("system", &(Arc::as_ptr(&self.system.inner) as usize))
            .field("state", &(Arc::as_ptr(&self.state) as usize))
            .finish()
    }
}

impl<T: Clone + Send + 'static> SharedFuture<T> {
    fn consumed_error() -> AsyncError {
        AsyncError::msg("Future already consumed")
    }

    pub(crate) fn from_state(system: AsyncSystem, state: Arc<SharedState<T>>) -> Self {
        Self { system, state }
    }

    pub fn is_ready(&self) -> bool {
        self.state.is_ready()
    }

    pub fn wait(&self) -> Result<T, AsyncError> {
        self.state.wait_until_ready();
        self.state
            .clone_result()
            .unwrap_or_else(|| Err(Self::consumed_error()))
    }

    pub fn wait_in_main_thread(&self) -> Result<T, AsyncError> {
        while !self.state.is_ready() {
            if !self.system.inner.main_queue.dispatch_one() {
                self.state.wait_timeout(Duration::from_millis(2));
            }
        }

        self.state
            .clone_result()
            .unwrap_or_else(|| Err(Self::consumed_error()))
    }

    /// Access the async system that owns this future.
    pub fn system(&self) -> AsyncSystem {
        self.system.clone()
    }

    fn then_with_scheduler<U, F, R>(&self, scheduler: SchedulerHandle, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        let source = Arc::clone(&self.state);
        let (promise, next_future) = create_pair::<U>(self.system.clone());

        let source_for_continuation = Arc::clone(&source);
        source.register_continuation(Box::new(move || {
            match source_for_continuation.clone_result() {
                Some(Ok(value)) => {
                    let run = move || {
                        f(value).resolve_into(promise);
                    };
                    if scheduler.is_current_thread() {
                        run();
                    } else {
                        scheduler.schedule(Box::new(run));
                    }
                }
                Some(Err(error)) => promise.reject(error),
                None => promise.reject(Self::consumed_error()),
            }
        }));

        next_future
    }

    fn catch_with_scheduler<F>(&self, scheduler: SchedulerHandle, f: F) -> Future<T>
    where
        F: FnOnce(AsyncError) -> T + Send + 'static,
    {
        let source = Arc::clone(&self.state);
        let (promise, next_future) = create_pair::<T>(self.system.clone());

        let source_for_continuation = Arc::clone(&source);
        source.register_continuation(Box::new(move || {
            match source_for_continuation.clone_result() {
                Some(Ok(value)) => promise.resolve(value),
                Some(Err(error)) => {
                    let run = move || {
                        promise.resolve(f(error));
                    };
                    if scheduler.is_current_thread() {
                        run();
                    } else {
                        scheduler.schedule(Box::new(run));
                    }
                }
                None => promise.reject(Self::consumed_error()),
            }
        }));

        next_future
    }

    #[deprecated(since = "0.2.0", note = "use `then(Context::Worker, f)` instead")]
    pub fn then_in_worker_thread<U, F, R>(&self, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        self.then(Context::Worker, f)
    }

    #[deprecated(since = "0.2.0", note = "use `then(Context::Main, f)` instead")]
    pub fn then_in_main_thread<U, F, R>(&self, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        self.then(Context::Main, f)
    }

    #[deprecated(since = "0.2.0", note = "use `then_in_pool(pool, f)` instead")]
    pub fn then_in_thread_pool<U, F, R>(&self, thread_pool: &ThreadPool, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        self.then_in_pool(thread_pool, f)
    }

    pub fn then_immediately<U, F, R>(&self, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        self.then(Context::Immediate, f)
    }

    /// Chain a continuation in the given scheduling context.
    pub fn then<U, F, R>(&self, context: Context, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        match self.system.inner.scheduler_for(context) {
            Some(scheduler) => self.then_with_scheduler(scheduler, f),
            None => self.then_immediate(f),
        }
    }

    /// Chain a continuation in a named thread pool.
    pub fn then_in_pool<U, F, R>(&self, thread_pool: &ThreadPool, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        self.then_with_scheduler(self.system.inner.thread_pool_scheduler(thread_pool), f)
    }

    fn then_immediate<U, F, R>(&self, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        let source = Arc::clone(&self.state);
        let (promise, next_future) = create_pair::<U>(self.system.clone());

        let source_for_continuation = Arc::clone(&source);
        source.register_continuation(Box::new(move || {
            match source_for_continuation.clone_result() {
                Some(Ok(value)) => f(value).resolve_into(promise),
                Some(Err(error)) => promise.reject(error),
                None => promise.reject(Self::consumed_error()),
            }
        }));

        next_future
    }

    #[deprecated(since = "0.2.0", note = "use `catch(Context::Main, f)` instead")]
    pub fn catch_in_main_thread<F>(&self, f: F) -> Future<T>
    where
        F: FnOnce(AsyncError) -> T + Send + 'static,
    {
        self.catch(Context::Main, f)
    }

    pub fn catch_immediately<F>(&self, f: F) -> Future<T>
    where
        F: FnOnce(AsyncError) -> T + Send + 'static,
    {
        self.catch(Context::Immediate, f)
    }

    /// Catch an error in the given scheduling context.
    pub fn catch<F>(&self, context: Context, f: F) -> Future<T>
    where
        F: FnOnce(AsyncError) -> T + Send + 'static,
    {
        match self.system.inner.scheduler_for(context) {
            Some(scheduler) => self.catch_with_scheduler(scheduler, f),
            None => self.catch_immediate(f),
        }
    }

    fn catch_immediate<F>(&self, f: F) -> Future<T>
    where
        F: FnOnce(AsyncError) -> T + Send + 'static,
    {
        let source = Arc::clone(&self.state);
        let (promise, next_future) = create_pair::<T>(self.system.clone());

        let source_for_continuation = Arc::clone(&source);
        source.register_continuation(Box::new(move || {
            match source_for_continuation.clone_result() {
                Some(Ok(value)) => promise.resolve(value),
                Some(Err(error)) => promise.resolve(f(error)),
                None => promise.reject(Self::consumed_error()),
            }
        }));

        next_future
    }
}

impl<T: Clone + Send + 'static> StdFuture for SharedFuture<T> {
    type Output = Result<T, AsyncError>;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        if self.state.is_ready() {
            return Poll::Ready(
                self.state
                    .clone_result()
                    .unwrap_or_else(|| Err(Self::consumed_error())),
            );
        }

        self.state.register_waker(cx.waker());
        Poll::Pending
    }
}
