use crate::context::Context;
use crate::error::AsyncError;
use crate::executor::Executor;
use crate::promise::Promise;
use crate::state::SharedState;
use crate::system::AsyncSystem;
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

    /// Returns a clone of the `AsyncSystem` that owns this future.
    pub fn system(&self) -> AsyncSystem {
        self.system.clone()
    }

    pub fn block(mut self) -> Result<T, AsyncError> {
        let state = self.state.take().ok_or_else(Self::consumed_error)?;
        state.wait_until_ready();
        state
            .take_result()
            .unwrap_or_else(|| Err(Self::consumed_error()))
    }

    pub fn block_with_main(mut self) -> Result<T, AsyncError> {
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

    fn then_with_executor<U, F, R>(mut self, executor: Arc<dyn Executor>, f: F) -> Future<U>
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
                    if executor.is_current_thread() {
                        run();
                    } else {
                        executor.execute(Box::new(run));
                    }
                }
                Some(Err(error)) => promise.reject(error),
                None => promise.reject(Self::consumed_error()),
            }
        }));

        next_future
    }

    fn catch_with_executor<F>(mut self, executor: Arc<dyn Executor>, f: F) -> Future<T>
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
                    if executor.is_current_thread() {
                        run();
                    } else {
                        executor.execute(Box::new(run));
                    }
                }
                None => promise.reject(Self::consumed_error()),
            }
        }));

        next_future
    }

    /// Transform the value inline (on the completing thread).
    /// Equivalent to `then(Context::IMMEDIATE, f)`.
    pub fn map<U, F, R>(self, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        self.then(Context::IMMEDIATE, f)
    }

    /// Chain a continuation in the given scheduling context.
    pub fn then<U, F, R>(self, context: Context, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        match self.system.inner.executor_for(context) {
            Some(executor) => self.then_with_executor(executor, f),
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
        let executor = self.system.inner.pool_executor(thread_pool);
        self.then_with_executor(executor, f)
    }

    /// Chain an async continuation in the given scheduling context.
    ///
    /// The closure receives the resolved value, returns a future, and that
    /// future is driven to completion on the target context.
    ///
    /// ```rust,ignore
    /// let result = system.resolved(42)
    ///     .then_async(Context::BACKGROUND, |v| async move {
    ///         async_transform(v).await
    ///     })
    ///     .block()
    ///     .unwrap();
    /// ```
    pub fn then_async<U, F, Fut>(mut self, context: Context, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = U> + Send + 'static,
    {
        let system = self.system.clone();
        let source = match self.state.take() {
            Some(state) => state,
            None => return Self::rejected_future(system, Self::consumed_error()),
        };

        let (promise, next_future) = create_pair::<U>(system.clone());
        let source_for_continuation = Arc::clone(&source);
        source.register_continuation(Box::new(move || {
            match source_for_continuation.take_result() {
                Some(Ok(value)) => {
                    let fut = f(value);
                    match system.inner.executor_for(context) {
                        Some(executor) => {
                            executor.spawn_future(Box::pin(async move {
                                promise.resolve(fut.await);
                            }));
                        }
                        None => {
                            // IMMEDIATE: block_on inline
                            promise.resolve(crate::block_on::block_on(fut));
                        }
                    }
                }
                Some(Err(error)) => promise.reject(error),
                None => promise.reject(Self::consumed_error()),
            }
        }));

        next_future
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

    /// Recover from an error inline (on the completing thread).
    /// Equivalent to `catch(Context::IMMEDIATE, f)`.
    pub fn or_else<F>(self, f: F) -> Future<T>
    where
        F: FnOnce(AsyncError) -> T + Send + 'static,
    {
        self.catch(Context::IMMEDIATE, f)
    }

    /// Flat-map over a `Result` value inline.
    ///
    /// If the future resolves with `Ok(v)`, applies `f(v)` and flattens.
    /// If the future resolves with `Err(e)`, the returned future resolves
    /// with `Err(e)` unchanged.
    pub fn and_then<V, E, F>(self, f: F) -> Future<Result<V, E>>
    where
        V: Send + 'static,
        E: Send + 'static,
        F: FnOnce(T) -> Result<V, E> + Send + 'static,
    {
        self.map(f)
    }

    /// Catch an error in the given scheduling context.
    pub fn catch<F>(self, context: Context, f: F) -> Future<T>
    where
        F: FnOnce(AsyncError) -> T + Send + 'static,
    {
        match self.system.inner.executor_for(context) {
            Some(executor) => self.catch_with_executor(executor, f),
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

    /// Returns a clone of the `AsyncSystem` that owns this future.
    pub fn system(&self) -> AsyncSystem {
        self.system.clone()
    }
    pub fn block(&self) -> Result<T, AsyncError> {
        self.state.wait_until_ready();
        self.state
            .clone_result()
            .unwrap_or_else(|| Err(Self::consumed_error()))
    }

    pub fn block_with_main(&self) -> Result<T, AsyncError> {
        while !self.state.is_ready() {
            if !self.system.inner.main_queue.dispatch_one() {
                self.state.wait_timeout(Duration::from_millis(2));
            }
        }

        self.state
            .clone_result()
            .unwrap_or_else(|| Err(Self::consumed_error()))
    }

    fn then_with_executor<U, F, R>(&self, executor: Arc<dyn Executor>, f: F) -> Future<U>
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
                    if executor.is_current_thread() {
                        run();
                    } else {
                        executor.execute(Box::new(run));
                    }
                }
                Some(Err(error)) => promise.reject(error),
                None => promise.reject(Self::consumed_error()),
            }
        }));

        next_future
    }

    fn catch_with_executor<F>(&self, executor: Arc<dyn Executor>, f: F) -> Future<T>
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
                    if executor.is_current_thread() {
                        run();
                    } else {
                        executor.execute(Box::new(run));
                    }
                }
                None => promise.reject(Self::consumed_error()),
            }
        }));

        next_future
    }

    /// Transform the value inline (on the completing thread).
    /// Equivalent to `then(Context::IMMEDIATE, f)`.
    pub fn map<U, F, R>(&self, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        self.then(Context::IMMEDIATE, f)
    }

    /// Chain a continuation in the given scheduling context.
    pub fn then<U, F, R>(&self, context: Context, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        match self.system.inner.executor_for(context) {
            Some(executor) => self.then_with_executor(executor, f),
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
        self.then_with_executor(self.system.inner.pool_executor(thread_pool), f)
    }

    /// Chain an async continuation in the given scheduling context.
    ///
    /// See [`Future::then_async`] for details. This variant borrows `&self`
    /// (cloning the resolved value) rather than consuming the future.
    pub fn then_async<U, F, Fut>(&self, context: Context, f: F) -> Future<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = U> + Send + 'static,
    {
        let source = Arc::clone(&self.state);
        let system = self.system.clone();
        let (promise, next_future) = create_pair::<U>(system.clone());

        let source_for_continuation = Arc::clone(&source);
        source.register_continuation(Box::new(move || {
            match source_for_continuation.clone_result() {
                Some(Ok(value)) => {
                    let fut = f(value);
                    match system.inner.executor_for(context) {
                        Some(executor) => {
                            executor.spawn_future(Box::pin(async move {
                                promise.resolve(fut.await);
                            }));
                        }
                        None => {
                            promise.resolve(crate::block_on::block_on(fut));
                        }
                    }
                }
                Some(Err(error)) => promise.reject(error),
                None => promise.reject(Self::consumed_error()),
            }
        }));

        next_future
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

    /// Recover from an error inline (on the completing thread).
    /// Equivalent to `catch(Context::IMMEDIATE, f)`.
    pub fn or_else<F>(&self, f: F) -> Future<T>
    where
        F: FnOnce(AsyncError) -> T + Send + 'static,
    {
        self.catch(Context::IMMEDIATE, f)
    }

    /// Flat-map over a `Result` value inline.
    ///
    /// If the future resolves with `Ok(v)`, applies `f(v)` and flattens.
    /// If the future resolves with `Err(e)`, the returned future resolves
    /// with `Err(e)` unchanged.
    pub fn and_then<V, E, F>(&self, f: F) -> Future<Result<V, E>>
    where
        V: Send + 'static,
        E: Send + 'static,
        F: FnOnce(T) -> Result<V, E> + Send + 'static,
    {
        self.map(f)
    }

    /// Catch an error in the given scheduling context.
    pub fn catch<F>(&self, context: Context, f: F) -> Future<T>
    where
        F: FnOnce(AsyncError) -> T + Send + 'static,
    {
        match self.system.inner.executor_for(context) {
            Some(executor) => self.catch_with_executor(executor, f),
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
