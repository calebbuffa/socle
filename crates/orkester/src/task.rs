use crate::context::Context;
use crate::error::AsyncError;
use crate::executor::Executor;
use crate::main_loop::MainThreadQueue;
use crate::resolver::Resolver;
use crate::scheduler::Scheduler;
use crate::task_cell::TaskCell;
use crate::thread_pool::ThreadPool;
use std::fmt::{self, Debug, Formatter};
use std::future::Future as StdFuture;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Poll, Wake};
use std::time::Duration;

/// Waker that calls `MainThreadQueue::notify()` to unblock `block_with_main`.
struct NotifyWaker(Arc<MainThreadQueue>);

impl Wake for NotifyWaker {
    fn wake(self: Arc<Self>) {
        self.0.notify();
    }
}

fn notify_waker(queue: Arc<MainThreadQueue>) -> std::task::Waker {
    std::task::Waker::from(Arc::new(NotifyWaker(queue)))
}

/// Internal state of a `Task<T>`.
///
/// `Ready` holds a synchronous result (zero heap allocation).
/// `Pending` is backed by a `TaskCell` for async completion.
///
/// There is no `Consumed` sentinel — ownership is tracked via
/// `Option::take()` inside `Ready`, and `Pending` state is consumed
/// via `Arc<TaskCell<T>>::take_result()`.
pub(crate) enum TaskInner<T: Send + 'static> {
    Ready(Option<Result<T, AsyncError>>),
    Pending(Arc<TaskCell<T>>),
}

/// Single-consumer async task.
///
/// Move-only. Use [`.share()`](Task::share) to convert to a cloneable
/// [`SharedTask<T>`]. Implements [`std::future::Future`] for async/await.
pub struct Task<T: Send + 'static> {
    pub(crate) system: Scheduler,
    pub(crate) inner: TaskInner<T>,
}

impl<T: Send + 'static> Task<T> {
    /// Create a task that is already resolved with a value.
    #[inline]
    pub(crate) fn ready(system: Scheduler, value: T) -> Self {
        Self {
            system,
            inner: TaskInner::Ready(Some(Ok(value))),
        }
    }

    /// Create a task that is already rejected with an error.
    #[inline]
    pub(crate) fn ready_err(system: Scheduler, error: AsyncError) -> Self {
        Self {
            system,
            inner: TaskInner::Ready(Some(Err(error))),
        }
    }
}

impl<T: Send + 'static> Debug for Task<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let variant = match &self.inner {
            TaskInner::Ready(Some(Ok(_))) => "Ready(Ok)",
            TaskInner::Ready(Some(Err(_))) => "Ready(Err)",
            TaskInner::Ready(None) => "Taken",
            TaskInner::Pending(s) => {
                return f
                    .debug_struct("Task")
                    .field("state", &(Arc::as_ptr(s) as usize))
                    .finish();
            }
        };
        f.debug_struct("Task").field("state", &variant).finish()
    }
}

pub(crate) fn create_pair<T: Send + 'static>(system: Scheduler) -> (Resolver<T>, Task<T>) {
    let cell = Arc::new(TaskCell::new());
    let resolver = Resolver::new(Arc::clone(&cell));
    let task = Task {
        system,
        inner: TaskInner::Pending(cell),
    };
    (resolver, task)
}

#[doc(hidden)]
pub trait ResolveOutput<T: Send + 'static>: Send + 'static {
    /// Resolve a resolver with this value (used on the async/Pending path).
    fn resolve_into(self, resolver: Resolver<T>);
    /// Convert to a ready task (used on the synchronous/Ready path).
    fn into_task(self, system: Scheduler) -> Task<T>;
}

impl<T> ResolveOutput<T> for T
where
    T: Send + 'static,
{
    fn resolve_into(self, resolver: Resolver<T>) {
        resolver.resolve(self);
    }
    #[inline]
    fn into_task(self, system: Scheduler) -> Task<T> {
        Task::ready(system, self)
    }
}

impl<T> ResolveOutput<T> for Task<T>
where
    T: Send + 'static,
{
    fn resolve_into(self, resolver: Resolver<T>) {
        self.pipe_to(resolver);
    }
    #[inline]
    fn into_task(self, _system: Scheduler) -> Task<T> {
        self
    }
}

impl<T: Send + 'static> Task<T> {
    fn consumed_error() -> AsyncError {
        AsyncError::msg("Task already consumed")
    }

    fn pipe_to(self, resolver: Resolver<T>) {
        match self.inner {
            TaskInner::Ready(Some(Ok(value))) => resolver.resolve(value),
            TaskInner::Ready(Some(Err(error))) => resolver.reject(error),
            TaskInner::Ready(None) => resolver.reject(Self::consumed_error()),
            TaskInner::Pending(cell) => {
                TaskCell::on_complete(cell, move |result| match result {
                    Ok(value) => resolver.resolve(value),
                    Err(error) => resolver.reject(error),
                });
            }
        }
    }

    #[inline]
    pub fn is_ready(&self) -> bool {
        match &self.inner {
            TaskInner::Ready(Some(_)) => true,
            TaskInner::Ready(None) => false,
            TaskInner::Pending(state) => state.is_ready(),
        }
    }

    /// Returns a clone of the `Scheduler` that owns this task.
    #[inline]
    pub fn system(&self) -> Scheduler {
        self.system.clone()
    }

    pub fn block(self) -> Result<T, AsyncError> {
        match self.inner {
            TaskInner::Ready(Some(result)) => result,
            TaskInner::Ready(None) => Err(Self::consumed_error()),
            TaskInner::Pending(cell) => {
                cell.wait_until_ready();
                cell.take_result()
                    .unwrap_or_else(|| Err(Self::consumed_error()))
            }
        }
    }

    pub fn block_with_main(self) -> Result<T, AsyncError> {
        match self.inner {
            TaskInner::Ready(Some(result)) => result,
            TaskInner::Ready(None) => Err(Self::consumed_error()),
            TaskInner::Pending(cell) => {
                let mq = Arc::clone(&self.system.inner.main_queue);
                cell.register_extra_waker(&notify_waker(Arc::clone(&mq)));

                loop {
                    while mq.dispatch_one() {}
                    if cell.is_ready() {
                        return cell
                            .take_result()
                            .unwrap_or_else(|| Err(Self::consumed_error()));
                    }
                    mq.wait_for_work();
                }
            }
        }
    }

    fn then_with_executor<U, F, R>(self, executor: Arc<dyn Executor>, f: F) -> Task<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        let system = self.system.clone();
        match self.inner {
            TaskInner::Ready(Some(Ok(value))) => {
                if executor.is_current_thread() {
                    f(value).into_task(system)
                } else {
                    let (resolver, next) = create_pair::<U>(system);
                    executor.execute(Box::new(move || f(value).resolve_into(resolver)));
                    next
                }
            }
            TaskInner::Ready(Some(Err(error))) => Task::ready_err(system, error),
            TaskInner::Ready(None) => Task::ready_err(system, Self::consumed_error()),
            TaskInner::Pending(cell) => {
                let (resolver, next) = create_pair::<U>(system);
                TaskCell::on_complete(cell, move |result| match result {
                    Ok(value) => {
                        let run = move || f(value).resolve_into(resolver);
                        if executor.is_current_thread() {
                            run();
                        } else {
                            executor.execute(Box::new(run));
                        }
                    }
                    Err(error) => resolver.reject(error),
                });
                next
            }
        }
    }

    fn catch_with_executor<F, R>(self, executor: Arc<dyn Executor>, f: F) -> Task<T>
    where
        F: FnOnce(AsyncError) -> R + Send + 'static,
        R: ResolveOutput<T>,
    {
        let system = self.system.clone();
        match self.inner {
            TaskInner::Ready(Some(Ok(value))) => Task::ready(system, value),
            TaskInner::Ready(Some(Err(error))) => {
                if executor.is_current_thread() {
                    f(error).into_task(system)
                } else {
                    let (resolver, next) = create_pair::<T>(system);
                    executor.execute(Box::new(move || f(error).resolve_into(resolver)));
                    next
                }
            }
            TaskInner::Ready(None) => Task::ready_err(system, Self::consumed_error()),
            TaskInner::Pending(cell) => {
                let (resolver, next) = create_pair::<T>(system);
                TaskCell::on_complete(cell, move |result| match result {
                    Ok(value) => resolver.resolve(value),
                    Err(error) => {
                        let run = move || f(error).resolve_into(resolver);
                        if executor.is_current_thread() {
                            run();
                        } else {
                            executor.execute(Box::new(run));
                        }
                    }
                });
                next
            }
        }
    }

    /// Transform the value inline (on the completing thread).
    /// Equivalent to `then(Context::IMMEDIATE, f)`.
    pub fn map<U, F, R>(self, f: F) -> Task<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        self.then(Context::IMMEDIATE, f)
    }

    /// Chain a continuation in the given scheduling context.
    pub fn then<U, F, R>(self, context: Context, f: F) -> Task<U>
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
    pub fn then_in_pool<U, F, R>(self, thread_pool: &ThreadPool, f: F) -> Task<U>
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
    pub fn then_async<U, F, Fut>(self, context: Context, f: F) -> Task<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = U> + Send + 'static,
    {
        let system = self.system.clone();
        match self.inner {
            TaskInner::Ready(Some(Ok(value))) => {
                let fut = f(value);
                match system.inner.executor_for(context) {
                    Some(executor) => {
                        let (resolver, next) = create_pair::<U>(system);
                        executor.spawn_future(Box::pin(async move {
                            resolver.resolve(fut.await);
                        }));
                        next
                    }
                    None => Task::ready(system, crate::block_on::block_on(fut)),
                }
            }
            TaskInner::Ready(Some(Err(error))) => Task::ready_err(system, error),
            TaskInner::Ready(None) => Task::ready_err(system, Self::consumed_error()),
            TaskInner::Pending(cell) => {
                let (resolver, next) = create_pair::<U>(system.clone());
                TaskCell::on_complete(cell, move |result| match result {
                    Ok(value) => {
                        let fut = f(value);
                        match system.inner.executor_for(context) {
                            Some(executor) => {
                                executor.spawn_future(Box::pin(async move {
                                    resolver.resolve(fut.await);
                                }));
                            }
                            None => {
                                resolver.resolve(crate::block_on::block_on(fut));
                            }
                        }
                    }
                    Err(error) => resolver.reject(error),
                });
                next
            }
        }
    }

    fn then_immediate<U, F, R>(self, f: F) -> Task<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        let system = self.system.clone();
        match self.inner {
            TaskInner::Ready(Some(Ok(value))) => f(value).into_task(system),
            TaskInner::Ready(Some(Err(error))) => Task::ready_err(system, error),
            TaskInner::Ready(None) => Task::ready_err(system, Self::consumed_error()),
            TaskInner::Pending(cell) => {
                let (resolver, next) = create_pair::<U>(system);
                TaskCell::on_complete(cell, move |result| match result {
                    Ok(value) => f(value).resolve_into(resolver),
                    Err(error) => resolver.reject(error),
                });
                next
            }
        }
    }

    /// Recover from an error inline (on the completing thread).
    /// Equivalent to `catch(Context::IMMEDIATE, f)`.
    pub fn or_else<F, R>(self, f: F) -> Task<T>
    where
        F: FnOnce(AsyncError) -> R + Send + 'static,
        R: ResolveOutput<T>,
    {
        self.catch(Context::IMMEDIATE, f)
    }

    /// Catch an error in the given scheduling context.
    pub fn catch<F, R>(self, context: Context, f: F) -> Task<T>
    where
        F: FnOnce(AsyncError) -> R + Send + 'static,
        R: ResolveOutput<T>,
    {
        match self.system.inner.executor_for(context) {
            Some(executor) => self.catch_with_executor(executor, f),
            None => self.catch_immediate(f),
        }
    }

    fn catch_immediate<F, R>(self, f: F) -> Task<T>
    where
        F: FnOnce(AsyncError) -> R + Send + 'static,
        R: ResolveOutput<T>,
    {
        let system = self.system.clone();
        match self.inner {
            TaskInner::Ready(Some(Ok(value))) => Task::ready(system, value),
            TaskInner::Ready(Some(Err(error))) => f(error).into_task(system),
            TaskInner::Ready(None) => Task::ready_err(system, Self::consumed_error()),
            TaskInner::Pending(cell) => {
                let (resolver, next) = create_pair::<T>(system);
                TaskCell::on_complete(cell, move |result| match result {
                    Ok(value) => resolver.resolve(value),
                    Err(error) => f(error).resolve_into(resolver),
                });
                next
            }
        }
    }

    /// Wrap this task with a timeout. If it doesn't complete within
    /// `duration`, the returned task rejects with [`ErrorCode::TimedOut`].
    pub fn with_timeout(self, duration: Duration) -> Task<T> {
        crate::combinators::timeout(&self.system.clone(), self, duration)
    }

    pub fn share(self) -> SharedTask<T>
    where
        T: Clone,
    {
        match self.inner {
            TaskInner::Ready(result) => {
                let cell = Arc::new(TaskCell::new());
                match result {
                    Some(Ok(value)) => cell.complete(Ok(value)),
                    Some(Err(error)) => cell.complete(Err(error)),
                    None => cell.complete(Err(Self::consumed_error())),
                }
                SharedTask::from_cell(self.system, cell)
            }
            TaskInner::Pending(cell) => SharedTask::from_cell(self.system, cell),
        }
    }
}

impl<T: Send + 'static> Unpin for Task<T> {}

impl<T: Send + 'static> StdFuture for Task<T> {
    type Output = Result<T, AsyncError>;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        match &mut this.inner {
            TaskInner::Ready(slot) => match slot.take() {
                Some(result) => Poll::Ready(result),
                None => Poll::Ready(Err(Self::consumed_error())),
            },
            TaskInner::Pending(cell) => {
                if cell.is_ready() {
                    Poll::Ready(
                        cell.take_result()
                            .unwrap_or_else(|| Err(Self::consumed_error())),
                    )
                } else {
                    cell.register_waker(cx.waker());
                    Poll::Pending
                }
            }
        }
    }
}

// ===========================================================================
// SharedTask<T>
// ===========================================================================

/// Cloneable multi-consumer async task.
///
/// Create via [`Task::share()`]. Multiple clones share the same underlying
/// result. Blocking or polling clones the value (requires `T: Clone`).
#[derive(Clone)]
pub struct SharedTask<T: Clone + Send + 'static> {
    pub(crate) system: Scheduler,
    pub(crate) cell: Arc<TaskCell<T>>,
}

impl<T: Clone + Send + 'static> Debug for SharedTask<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("SharedTask")
            .field("system", &(Arc::as_ptr(&self.system.inner) as usize))
            .field("cell", &(Arc::as_ptr(&self.cell) as usize))
            .finish()
    }
}

impl<T: Clone + Send + 'static> SharedTask<T> {
    fn consumed_error() -> AsyncError {
        AsyncError::msg("Task already consumed")
    }

    pub(crate) fn from_cell(system: Scheduler, cell: Arc<TaskCell<T>>) -> Self {
        Self { system, cell }
    }

    #[inline]
    pub fn is_ready(&self) -> bool {
        self.cell.is_ready()
    }

    /// Returns a clone of the `Scheduler` that owns this task.
    #[inline]
    pub fn system(&self) -> Scheduler {
        self.system.clone()
    }

    pub fn block(&self) -> Result<T, AsyncError> {
        self.cell.wait_until_ready();
        self.cell
            .clone_result()
            .unwrap_or_else(|| Err(Self::consumed_error()))
    }

    pub fn block_with_main(&self) -> Result<T, AsyncError> {
        let mq = Arc::clone(&self.system.inner.main_queue);
        self.cell
            .register_extra_waker(&notify_waker(Arc::clone(&mq)));

        loop {
            while mq.dispatch_one() {}
            if self.cell.is_ready() {
                return self
                    .cell
                    .clone_result()
                    .unwrap_or_else(|| Err(Self::consumed_error()));
            }
            mq.wait_for_work();
        }
    }

    fn then_with_executor<U, F, R>(&self, executor: Arc<dyn Executor>, f: F) -> Task<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        let source = Arc::clone(&self.cell);
        let (resolver, next_task) = create_pair::<U>(self.system.clone());

        TaskCell::on_complete_cloned(source, move |result| match result {
            Ok(value) => {
                let run = move || f(value).resolve_into(resolver);
                if executor.is_current_thread() {
                    run();
                } else {
                    executor.execute(Box::new(run));
                }
            }
            Err(error) => resolver.reject(error),
        });

        next_task
    }

    fn catch_with_executor<F, R>(&self, executor: Arc<dyn Executor>, f: F) -> Task<T>
    where
        F: FnOnce(AsyncError) -> R + Send + 'static,
        R: ResolveOutput<T>,
    {
        let source = Arc::clone(&self.cell);
        let (resolver, next_task) = create_pair::<T>(self.system.clone());

        TaskCell::on_complete_cloned(source, move |result| match result {
            Ok(value) => resolver.resolve(value),
            Err(error) => {
                let run = move || f(error).resolve_into(resolver);
                if executor.is_current_thread() {
                    run();
                } else {
                    executor.execute(Box::new(run));
                }
            }
        });

        next_task
    }

    /// Transform the value inline (on the completing thread).
    /// Equivalent to `then(Context::IMMEDIATE, f)`.
    pub fn map<U, F, R>(&self, f: F) -> Task<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        self.then(Context::IMMEDIATE, f)
    }

    /// Chain a continuation in the given scheduling context.
    pub fn then<U, F, R>(&self, context: Context, f: F) -> Task<U>
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
    pub fn then_in_pool<U, F, R>(&self, thread_pool: &ThreadPool, f: F) -> Task<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        self.then_with_executor(self.system.inner.pool_executor(thread_pool), f)
    }

    /// Chain an async continuation in the given scheduling context.
    ///
    /// See [`Task::then_async`] for details. This variant borrows `&self`
    /// (cloning the resolved value) rather than consuming the task.
    pub fn then_async<U, F, Fut>(&self, context: Context, f: F) -> Task<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = U> + Send + 'static,
    {
        let source = Arc::clone(&self.cell);
        let system = self.system.clone();
        let (resolver, next_task) = create_pair::<U>(system.clone());

        TaskCell::on_complete_cloned(source, move |result| match result {
            Ok(value) => {
                let fut = f(value);
                match system.inner.executor_for(context) {
                    Some(executor) => {
                        executor.spawn_future(Box::pin(async move {
                            resolver.resolve(fut.await);
                        }));
                    }
                    None => {
                        resolver.resolve(crate::block_on::block_on(fut));
                    }
                }
            }
            Err(error) => resolver.reject(error),
        });

        next_task
    }

    fn then_immediate<U, F, R>(&self, f: F) -> Task<U>
    where
        U: Send + 'static,
        F: FnOnce(T) -> R + Send + 'static,
        R: ResolveOutput<U>,
    {
        let source = Arc::clone(&self.cell);
        let (resolver, next_task) = create_pair::<U>(self.system.clone());

        TaskCell::on_complete_cloned(source, move |result| match result {
            Ok(value) => f(value).resolve_into(resolver),
            Err(error) => resolver.reject(error),
        });

        next_task
    }

    /// Recover from an error inline (on the completing thread).
    /// Equivalent to `catch(Context::IMMEDIATE, f)`.
    pub fn or_else<F, R>(&self, f: F) -> Task<T>
    where
        F: FnOnce(AsyncError) -> R + Send + 'static,
        R: ResolveOutput<T>,
    {
        self.catch(Context::IMMEDIATE, f)
    }

    /// Catch an error in the given scheduling context.
    pub fn catch<F, R>(&self, context: Context, f: F) -> Task<T>
    where
        F: FnOnce(AsyncError) -> R + Send + 'static,
        R: ResolveOutput<T>,
    {
        match self.system.inner.executor_for(context) {
            Some(executor) => self.catch_with_executor(executor, f),
            None => self.catch_immediate(f),
        }
    }

    fn catch_immediate<F, R>(&self, f: F) -> Task<T>
    where
        F: FnOnce(AsyncError) -> R + Send + 'static,
        R: ResolveOutput<T>,
    {
        let source = Arc::clone(&self.cell);
        let (resolver, next_task) = create_pair::<T>(self.system.clone());

        TaskCell::on_complete_cloned(source, move |result| match result {
            Ok(value) => resolver.resolve(value),
            Err(error) => f(error).resolve_into(resolver),
        });

        next_task
    }
}

impl<T: Clone + Send + 'static> StdFuture for SharedTask<T> {
    type Output = Result<T, AsyncError>;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        if self.cell.is_ready() {
            return Poll::Ready(
                self.cell
                    .clone_result()
                    .unwrap_or_else(|| Err(Self::consumed_error())),
            );
        }

        self.cell.register_extra_waker(cx.waker());
        Poll::Pending
    }
}
