# orkester

Context-aware task scheduling for Rust.

*orkester is Russian for "orchestra" — orchestrating asynchronous and concurrent tasks.*

## Overview

orkester is a **scheduling policy layer**. It doesn't replace tokio — it sits on top,
adding explicit context-aware dispatch, thread affinity, and a C FFI.

- **tokio** answers: *"run this async task somewhere."*
- **orkester** answers: *"run this task **here**, on **this context**, and give me the result."*

**Core types:**

| Type | Description |
|------|-------------|
| `Context` | Scheduling token — identifies where work runs |
| `ThreadPool` | Self-draining background thread pool |
| `WorkQueue` | Caller-pumped queue for main/UI threads |
| `Task<T>` | Move-only single-consumer async value |
| `Handle<T>` | Cloneable multi-consumer async value (`T: Clone`) |
| `Resolver<T>` | Completion handle for a `Task<T>` |
| `Executor` | Trait for custom execution backends |

**Primitives:**

| Type | Description |
|------|-------------|
| `CancellationToken` | Cooperative cancellation, shared across tasks |
| `Scope` | Structured cancellation — children cancelled when scope drops |
| `Semaphore` | Async-aware counting semaphore |
| `JoinSet<T>` | Tracked collection of in-flight tasks |
| `Sender<T>` / `Receiver<T>` | Bounded MPSC channels |

**Free function combinators:** `delay`, `timeout`, `race`, `retry`, `join_all`, `resolved`, `resolver`

## Quick Start

```rust
use orkester::{ThreadPool, WorkQueue};

// Background thread pool
let bg = ThreadPool::new(4);
let bg_ctx = bg.context();

// Optional: caller-pumped queue for main/UI thread
let mut wq = WorkQueue::new();
let main_ctx = wq.context();

// Resolver/task pair — resolve from anywhere
let (resolver, task) = orkester::resolver::<i32>();
resolver.resolve(42);
assert_eq!(task.block().unwrap(), 42);

// Run work on a background thread
let result = bg_ctx.run(|| expensive_computation());

// Continuation chains — closures may return T or Task<T> (flattened automatically)
let chained = bg_ctx.run(|| 3_i32)
    .then(&bg_ctx, |v| v + 1)
    .then(&bg_ctx, |v| v * 2);
assert_eq!(chained.block().unwrap(), 8);

// Chain onto main/UI thread, pump to completion
let task = bg_ctx.run(|| compute())
    .then(&main_ctx, |v| update_ui(v));
while !task.is_ready() {
    wq.pump();
}
```

## Scheduling Contexts

`Context` is a lightweight, cloneable scheduling token. Three kinds:

| Context | Description |
|---------|-------------|
| `Context::IMMEDIATE` | Runs inline on the completing thread — no overhead |
| `pool.context()` | Routes work to a `ThreadPool` |
| `wq.context()` | Routes work to a `WorkQueue` (caller pumps) |

Pass `&Context` to `.then()`, `.catch()`, `.then_async()`, and `.and_then()`.
Closures may return `T` or `Task<T>` — both are handled identically.

## Async/Await

`Task<T>` implements `std::future::Future<Output = Result<T, AsyncError>>`.

```rust
// Run an async closure in a specific context
let task = bg_ctx.run_async(|| async {
    let data = some_async_op().await;
    transform(data)
});

// Mix callback chains and async
let task = bg_ctx.run(|| fetch_bytes())
    .then_async(&bg_ctx, |bytes| async move {
        decompress(bytes).await
    });
```

For IO-bound async work, use `TokioExecutor` so futures are polled by the tokio
reactor rather than blocked on a worker thread.

## Error Flow

```rust
// .then() propagates errors without invoking the closure
// .catch() handles errors; .or_else() catches inline (no scheduling)
let task = bg_ctx.run(|| might_fail())
    .catch(&bg_ctx, |err| fallback_value())
    .or_else(|err| default_value());

// Fallible chains with Result
let task: Task<Result<Decoded, MyError>> = bg_ctx.run(|| fetch())
    .and_then(&bg_ctx, |bytes| decode(bytes));  // Err propagates without calling decode
```

## Cancellation

```rust
let token = CancellationToken::new();

let task = bg_ctx.run(|| long_work())
    .with_cancellation(&token);

token.cancel();  // task rejects with ErrorCode::Cancelled if not yet complete
```

**Structured cancellation with `Scope`:**

```rust
let scope = Scope::new();

let a = scope.run(&bg_ctx, || work_a());
let b = scope.run(&bg_ctx, || work_b());

drop(scope);  // cancels a and b if still in-flight
```

## Thread Affinity

```rust
// Dedicated single-thread pool for GPU work
let gpu_pool = ThreadPool::new(1);
let gpu_ctx = gpu_pool.context();

bg_ctx.run(|| prepare_data())
    .then(&gpu_ctx, |data| upload_to_gpu(data));

// WorkQueue: caller decides when to drain
let mut wq = WorkQueue::new();
let ctx = wq.context();

task.then(&ctx, |v| render(v));
wq.pump();               // drain one batch
wq.pump_until_empty();   // drain everything queued so far
```

## Shared Tasks (`Handle<T>`)

```rust
let (resolver, task) = orkester::resolver::<i32>();
let handle = task.share();         // Task → Handle (requires T: Clone)
let handle2 = handle.clone();

resolver.resolve(99);

assert_eq!(handle.block().unwrap(), 99);
assert_eq!(handle2.block().unwrap(), 99);  // both see the same result
```

## Semaphore

```rust
let sem = Semaphore::new(3);  // at most 3 concurrent holders

let permit = sem.acquire();   // blocks if all permits held
// ... do limited work ...
drop(permit);                  // releases, wakes next waiter

if let Some(permit) = sem.try_acquire() { /* non-blocking */ }
```

## Channels

```rust
use orkester::channel;

let (tx, rx) = channel::mpsc::<i32>(16);
tx.send(1).unwrap();
let val = rx.recv();  // Some(1)

let (tx, rx) = channel::oneshot::<String>();
tx.send("hello".into()).unwrap();
```

## Timeout and Combinators

```rust
use std::time::Duration;

// Reject if task doesn't complete in time
let task = bg_ctx.run(|| slow_work())
    .with_timeout(Duration::from_secs(5));

// Free function versions
let task   = orkester::timeout(bg_ctx.run(|| work()), Duration::from_secs(5));
let winner = orkester::race(vec![task_a, task_b]);    // first to complete wins
let all    = orkester::join_all(vec![a, b, c]);        // wait for all, in order

// Exponential backoff retry
let task = orkester::retry(3, RetryConfig::default(), || bg_ctx.run(|| fallible()));

// Timer (single background thread — no thread parked per call)
let done = orkester::delay(Duration::from_millis(100));
```

## Feature Flags

```toml
[dependencies]
orkester = "0.3"

# With tokio backend
orkester = { version = "0.3", features = ["tokio-runtime"] }

# For WASM targets
orkester = { version = "0.3", features = ["wasm"] }
```

| Feature | Description |
|---------|-------------|
| `custom-runtime` *(default)* | Built-in `ThreadPool` executor |
| `tokio-runtime` | `TokioExecutor` via `tokio::runtime::Handle` |
| `wasm` | `WasmExecutor` + `spawn_local` for WebAssembly |

## API Summary

### Free functions

```rust
orkester::resolver<T>() -> (Resolver<T>, Task<T>)
orkester::resolved<T>(value: T) -> Task<T>
orkester::delay(duration: Duration) -> Task<()>
orkester::join_all<T>(tasks: impl IntoIterator<Item=Task<T>>) -> Task<Vec<T>>
orkester::timeout<T>(task: Task<T>, duration: Duration) -> Task<T>
orkester::race<T>(tasks: Vec<Task<T>>) -> Task<T>
orkester::retry<T, F>(attempts: u32, config: RetryConfig, f: F) -> Task<T>
```

### `Context`

```rust
Context::IMMEDIATE                              // inline on completing thread
Context::new(executor: impl Executor) -> Self
context.run<T, F>(&self, f: F) -> Task<T>
context.run_async<T, F, Fut>(&self, f: F) -> Task<T>
```

### `Task<T>`

```rust
task.is_ready(&self) -> bool
task.block(self) -> Result<T, AsyncError>

// Continuations (closure may return T or Task<T>)
task.then<U, F>(self, context: &Context, f: F) -> Task<U>
task.then_async<U, F, Fut>(self, context: &Context, f: F) -> Task<U>
task.map<U, F>(self, f: F) -> Task<U>           // inline, = then(&IMMEDIATE, f)
task.catch<F>(self, context: &Context, f: F) -> Task<T>
task.or_else<F>(self, f: F) -> Task<T>          // inline, = catch(&IMMEDIATE, f)
task.and_then<U, F>(self, context: &Context, f: F) -> Task<Result<U, E>>

task.join<U>(self, other: Task<U>) -> Task<(T, U)>
task.share(self) -> Handle<T>                   // requires T: Clone
task.with_timeout(self, duration: Duration) -> Task<T>
task.with_cancellation(self, token: &CancellationToken) -> Task<T>
```

`Task<T>` implements `Future<Output = Result<T, AsyncError>>`.

### `Handle<T>` (cloneable)

Same continuation API as `Task<T>` but borrows `&self` — can be awaited multiple times.

### `Resolver<T>`

```rust
resolver.resolve(self, value: T)
resolver.reject(self, error: impl Into<AsyncError>)
// Dropping an unresolved Resolver<T> auto-rejects with ErrorCode::Dropped
```

### `Scope`

```rust
Scope::new() -> Scope
scope.token(&self) -> &CancellationToken
scope.run<T, F>(&self, context: &Context, f: F) -> Task<T>
scope.run_async<T, F, Fut>(&self, context: &Context, f: F) -> Task<T>
// Drop → cancels all in-flight tasks spawned through this scope
```

### `Semaphore`

```rust
Semaphore::new(permits: usize) -> Semaphore    // Clone
semaphore.acquire(&self) -> SemaphorePermit    // blocking
semaphore.try_acquire(&self) -> Option<SemaphorePermit>
semaphore.available_permits(&self) -> usize
semaphore.max_permits(&self) -> usize
// SemaphorePermit releases on drop
```

### `JoinSet<T>`

```rust
JoinSet::new() -> JoinSet<T>
join_set.push(&mut self, task: Task<T>)
join_set.len(&self) -> usize
join_set.block_all(self) -> Vec<Result<T, AsyncError>>
join_set.join_next(&mut self) -> Option<Result<T, AsyncError>>
```

### `Executor` trait

```rust
pub trait Executor: Send + Sync {
    fn execute(&self, task: Box<dyn FnOnce() + Send + 'static>);
    fn spawn_future(&self, future: BoxFuture) { /* default: execute + block_on */ }
    fn is_current_thread(&self) -> bool { false }
}
```

Override `spawn_future` when backed by an async runtime so futures are polled by
the reactor rather than blocked on a worker thread.

### `AsyncError`

```rust
AsyncError::msg(message: impl Into<String>) -> AsyncError
AsyncError::new<E: Error + Send + Sync>(error: E) -> AsyncError
AsyncError::with_code(code: ErrorCode, message: impl Into<String>) -> AsyncError
error.code(&self) -> ErrorCode
error.downcast_ref<E: Error>(&self) -> Option<&E>

enum ErrorCode { Generic, Cancelled, TimedOut, Dropped }
```

## Design Notes

- `TaskCell<T>` — lock-free atomic state machine; no per-continuation watcher task
- `TimerWheel` — single background thread services all timers; no thread parked per `delay`
- `WorkQueue` — deterministic, caller-controlled pumping; no background reactor needed
- `Task<T>` is move-only; use `.share()` for multi-consumer scenarios
- `Context` is `Clone + PartialEq + Hash` — safe to store, compare, and deduplicate


## Overview

orkester is **the scheduling policy layer for Rust**. It doesn't replace tokio — it sits
on top, adding context-aware dispatch, thread affinity, and a C FFI.

- **tokio** answers: *"run this async task somewhere."*
- **orkester** answers: *"run this task **here**, on **this context**, with **this priority**, and give me the result."*

**Core types:**

- **`Runtime`** — root runtime object; owns executors and the main-thread queue
- **`Resolver<T>` / `Task<T>`** — single-producer / single-consumer async pair
- **`SharedTask<T>`** — cloneable multi-consumer task (requires `T: Clone`)
- **`Context`** — lightweight handle identifying a scheduling target (u32-indexed)
- **`Executor`** — trait for custom execution backends

**Primitives:**

- **`CancellationToken`** — cooperative cancellation across tasks
- **`Scope`** — structured cancellation (parent cancel → children cancel)
- **`Semaphore`** — async-aware counting semaphore
- **`JoinSet<T>`** — tracked collection of spawned work
- **`Sender<T>` / `Receiver<T>`** — bounded MPSC channels

**Combinators:**

- **`delay`** / **`timeout`** / **`race`** / **`retry`** — free functions and methods for common patterns

## Feature Flags

```toml
[dependencies]
orkester = "0.3"                            # default: custom-runtime

# With tokio backend
orkester = { version = "0.3", features = ["tokio-runtime"] }

# For WASM targets
orkester = { version = "0.3", features = ["wasm"] }
```

| Feature | Description |
|---------|-------------|
| `custom-runtime` *(default)* | Built-in thread pool executor |
| `tokio-runtime` | `TokioExecutor` backend via `tokio::runtime::Handle` |
| `wasm` | `WasmExecutor` + `spawn_local` for WebAssembly targets |

## Design Principles

- Lock-free `TaskCell<T>` completion primitive (atomic state machine + waker)
- No watcher-task-per-continuation overhead
- Main-thread work queue with deterministic pumping
- `std::future::Future` integration for both `Task<T>` and `SharedTask<T>`
- Extensible scheduling via user-defined contexts
- Dual API: callback chains AND async/await
- Timer wheel for efficient `delay`/`timeout` — no thread parking
- No `unsafe` in the core implementation

## Quick Start

```rust
use orkester::{Runtime, Context};

// Create a Runtime with a default thread pool
let system = Runtime::with_threads(4);

// Resolver/task pair
let (resolver, task) = system.resolver::<i32>();
resolver.resolve(42);
assert_eq!(task.block().unwrap(), 42);

// Run work on a background thread
let result = system.run(Context::BACKGROUND, || 5);
assert_eq!(result.block().unwrap(), 5);

// Continuation chains — closures can return values or Task<T>
let chained = system
    .resolved(3)
    .then(Context::BACKGROUND, |v| v + 1)
    .then(Context::BACKGROUND, {
        let system = system.clone();
        move |v| system.resolved(v * 2)
    });
assert_eq!(chained.block().unwrap(), 8);
```

## Dual API: Callbacks and Async/Await

orkester supports both callback-chain and async/await styles, freely intermixed:

```rust
use orkester::{Runtime, Context};

let system = Runtime::with_threads(4);

// Callback chains (C++ interop style, zero-async)
let result = system.run(Context::BACKGROUND, || compute())
    .then(Context::BACKGROUND, |data| decode(data))
    .map(|decoded| transform(decoded))
    .block()
    .unwrap();

// Async/await (natural Rust)
let result = system.spawn(async {
    let data = fetch(url).await;
    process(data)
}).block().unwrap();

// Async closures in a specific context
let result = system.run_async(Context::BACKGROUND, || async {
    expensive_computation().await
}).block().unwrap();

// Mix freely: callback chain → async continuation
let result = system.run(Context::BACKGROUND, || compute())
    .then_async(Context::BACKGROUND, |val| async move {
        async_transform(val).await
    })
    .block()
    .unwrap();
```

## Scheduling Contexts

`Context` is a lightweight handle (`u32`) identifying where a task runs:

| Context | Description |
|---------|-------------|
| `Context::BACKGROUND` | Runs on background executor thread pool |
| `Context::MAIN` | Runs on main thread (pumped or inline) |
| `Context::IMMEDIATE` | Runs inline on the completing thread |

Custom contexts can be registered at runtime:

```rust
let gpu = system.register_context(GpuThreadExecutor::new());
system.run(gpu, || upload_texture(data));
```

For dedicated thread pools, use `run_in_pool` / `then_in_pool`.

Main-thread work is either executed inline (inside `main_scope()`) or queued
for explicit pumping via `flush_main`.

## Runtime Backends

### Custom Runtime (default)

Create a Runtime with a built-in thread pool:

```rust
use orkester::Runtime;

let system = Runtime::with_threads(4);
```

Or use the builder for more control:

```rust
let system = Runtime::builder()
    .executor(MyCustomExecutor::new())
    .build();
```

### Tokio Runtime

Use tokio's runtime for async task spawning:

```rust
use orkester::{Runtime, TokioExecutor};

let system = Runtime::builder()
    .executor(TokioExecutor::current())
    .build();
```

### WASM

For WebAssembly targets with `wasm-bindgen-futures`:

```rust
use orkester::{Runtime, WasmExecutor};

let system = Runtime::builder()
    .executor(WasmExecutor)
    .build();

// spawn_local — no Send required on the future
let result = system.spawn_local(async { compute() });
```

## API Reference

### `Runtime`

```rust
// Construction
Runtime::new(executor: impl Executor) -> Runtime
Runtime::with_threads(n: usize) -> Runtime
Runtime::builder() -> RuntimeBuilder

// Context management
Runtime::register_context(&self, executor: impl Executor) -> Context
Runtime::thread_pool(&self, num_threads: usize) -> ThreadPool

// Resolver/task creation
Runtime::resolver<T>(&self) -> (Resolver<T>, Task<T>)
Runtime::task<T, F>(&self, f: F) -> Task<T>
Runtime::resolved<T>(&self, value: T) -> Task<T>

// Schedule work (sync closures)
Runtime::run<T, F>(&self, context: Context, f: F) -> Task<T>
Runtime::run_in_pool<T, F>(&self, pool: &ThreadPool, f: F) -> Task<T>

// Schedule work (async)
Runtime::run_async<T, F, Fut>(&self, context: Context, f: F) -> Task<T>
Runtime::spawn<T, Fut>(&self, future: Fut) -> Task<T>  // on BACKGROUND
Runtime::spawn_local<T, Fut>(&self, future: Fut) -> Task<T>  // WASM only
Runtime::spawn_detached<F>(&self, context: Context, f: F)

// Main-thread dispatch
Runtime::flush_main(&self) -> usize
Runtime::flush_main_one(&self) -> bool
Runtime::main_pending(&self) -> bool
Runtime::main_scope(&self) -> MainThreadScope

// Structured concurrency
Runtime::scope(&self) -> Scope
Runtime::join_set<T>(&self) -> JoinSet<T>
Runtime::join_all<T, I>(&self, tasks: I) -> Task<Vec<T>>

// Timer
Runtime::delay(&self, duration: Duration) -> Task<()>
```

`Runtime` is `Clone` (cheap `Arc` clone).

### `RuntimeBuilder`

```rust
RuntimeBuilder::executor(self, executor: impl Executor) -> Self
RuntimeBuilder::context(self, executor: impl Executor) -> Self
RuntimeBuilder::build(self) -> Runtime
```

### `Resolver<T>`

```rust
Resolver::resolve(self, value: T)
Resolver::reject(self, error: impl Into<AsyncError>)
```

Dropping an unresolved `Resolver<T>` auto-rejects with `ErrorCode::Dropped`.

### `Task<T>`

```rust
Task::is_ready(&self) -> bool
Task::system(&self) -> Runtime
Task::block(self) -> Result<T, AsyncError>
Task::block_with_main(self) -> Result<T, AsyncError>

// Continuations — closures may return U or Task<U> (auto-flattened)
Task::then<U, F>(self, context: Context, f: F) -> Task<U>
Task::then_in_pool<U, F>(self, pool: &ThreadPool, f: F) -> Task<U>
Task::then_async<U, F, Fut>(self, context: Context, f: F) -> Task<U>
Task::map<U, F>(self, f: F) -> Task<U>          // = then(IMMEDIATE, f)

// Error recovery
Task::catch<F>(self, context: Context, f: F) -> Task<T>
Task::or_else<F>(self, f: F) -> Task<T>          // = catch(IMMEDIATE, f)

// Timeout
Task::with_timeout(self, duration: Duration) -> Task<T>

// Cancellation
Task::with_cancellation(self, token: &CancellationToken) -> Task<T>

// Convert to multi-consumer (requires T: Clone)
Task::share(self) -> SharedTask<T>
```

`Task<T>` implements `std::future::Future<Output = Result<T, AsyncError>>`.

### `SharedTask<T>`

Same continuation API as `Task<T>` (including `then_async`), but borrows `&self`
instead of consuming. Can be cloned and waited on multiple times — each consumer
receives a cloned result.

### `CancellationToken`

```rust
CancellationToken::new() -> CancellationToken      // Clone
CancellationToken::cancel(&self)
CancellationToken::is_cancelled(&self) -> bool
```

Attach to a task with `task.with_cancellation(&token)`. When cancelled,
the task rejects with `ErrorCode::Cancelled`.

### `Scope`

Structured cancellation scope — all tasks spawned within a scope are automatically
cancelled when the scope is dropped.

```rust
Scope::token(&self) -> &CancellationToken
Scope::run<T, F>(&self, context: Context, f: F) -> Task<T>
Scope::run_async<T, F, Fut>(&self, context: Context, f: F) -> Task<T>
Scope::spawn<T, Fut>(&self, fut: Fut) -> Task<T>
Scope::run_in_pool<T, F>(&self, pool: &ThreadPool, f: F) -> Task<T>
Scope::cancel(&self)
```

### `Semaphore`

```rust
Semaphore::new(system: &Runtime, permits: usize) -> Semaphore
Semaphore::acquire(&self) -> SemaphorePermit        // blocking
Semaphore::try_acquire(&self) -> Option<SemaphorePermit>
Semaphore::available_permits(&self) -> usize
Semaphore::max_permits(&self) -> usize
Semaphore::acquire_async(&self) -> Task<SemaphorePermit>
```

`SemaphorePermit` releases on drop.

### `JoinSet<T>`

```rust
JoinSet::push(&mut self, task: Task<T>)
JoinSet::len(&self) -> usize
JoinSet::is_empty(&self) -> bool
JoinSet::join_all(self) -> Vec<Result<T, AsyncError>>
JoinSet::join_next(&mut self) -> Option<Result<T, AsyncError>>
```

### Channels (`Sender<T>` / `Receiver<T>`)

```rust
channel::mpsc<T>(capacity: usize) -> (Sender<T>, Receiver<T>)
channel::oneshot<T>() -> (Sender<T>, Receiver<T>)

Sender::send(&self, value: T) -> Result<(), SendError<T>>
Sender::try_send(&self, value: T) -> Result<(), TrySendError<T>>
Receiver::recv(&self) -> Option<T>
```

### Combinators (free functions)

```rust
timeout<T>(system: &Runtime, task: Task<T>, duration: Duration) -> Task<T>
race<T>(system: &Runtime, tasks: Vec<Task<T>>) -> Task<T>
retry<T, F>(system: &Runtime, max_attempts: u32, config: RetryConfig, f: F) -> Task<T>
```

`RetryConfig` controls exponential backoff:

```rust
RetryConfig {
    initial_backoff: Duration,  // default: 50ms
    max_backoff: Duration,      // default: 5s
    multiplier: u32,            // default: 2
}
```

### `Executor`

```rust
pub trait Executor: Send + Sync {
    fn execute(&self, task: Box<dyn FnOnce() + Send + 'static>);
    fn spawn_future(&self, future: BoxFuture) { /* default: execute + block_on */ }
    fn is_current_thread(&self) -> bool { false }
}
```

Implement this to create custom scheduling contexts. The default `spawn_future`
dispatches the future as a blocking task via `execute`. Override it when backed
by an async runtime (e.g. tokio) for native async spawning.

### Error Types

```rust
// Runtime error (cancellation, timeout, dropped resolver)
AsyncError::new<E: Error + Send + Sync + 'static>(error: E) -> AsyncError
AsyncError::msg(message: impl Into<String>) -> AsyncError
AsyncError::with_code(code: ErrorCode, message: impl Into<String>) -> AsyncError
AsyncError::code(&self) -> ErrorCode
AsyncError::downcast_ref<E: Error + 'static>(&self) -> Option<&E>

// Error codes
enum ErrorCode { Generic, Cancelled, TimedOut, Dropped }
```

Converts from `String`, `&str`, and `Box<dyn Error + Send + Sync>`.

## Examples

### Main-Thread Dispatch

```rust
// Outside main_scope: main-thread work is queued
let main_result = system.run(Context::MAIN, || 7);
system.flush_main();
assert_eq!(main_result.block().unwrap(), 7);

// Inside main_scope: main-thread work runs inline
let _scope = system.main_scope();
let immediate = system.run(Context::MAIN, || 9);
assert!(immediate.is_ready());
```

### Error Handling

```rust
let recovered = system
    .task(|resolver| resolver.reject("boom"))
    .catch(Context::MAIN, |_err| 99);
assert_eq!(recovered.block_with_main().unwrap(), 99);
```

### SharedTask Fan-Out

```rust
let shared = system.resolved(10).share();
let a = shared.then(Context::BACKGROUND, |v| v + 1);
let b = shared.then(Context::BACKGROUND, |v| v * 3);
assert_eq!(a.block().unwrap(), 11);
assert_eq!(b.block().unwrap(), 30);
```

### Joining Tasks

```rust
let joined = system.join_all(vec![
    system.resolved(1),
    system.resolved(2),
    system.resolved(3),
]);
assert_eq!(joined.block().unwrap(), vec![1, 2, 3]);
```

### Timeout and Retry

```rust
use orkester::{timeout, retry, RetryConfig};
use std::time::Duration;

// Timeout: reject if not done in 500ms
let slow = system.run(Context::BACKGROUND, || {
    std::thread::sleep(Duration::from_secs(5));
    42
});
let result = timeout(&system, slow, Duration::from_millis(500));
assert!(result.block().is_err()); // ErrorCode::TimedOut

// Retry with exponential backoff
let result = retry(&system, 3, RetryConfig::default(), || {
    system.resolved(Ok(42))
});
assert_eq!(result.block().unwrap(), 42);
```

### Cancellation

```rust
use orkester::CancellationToken;

let token = CancellationToken::new();
let task = system.run(Context::BACKGROUND, || 42)
    .with_cancellation(&token);
token.cancel();
assert!(task.block().is_err()); // ErrorCode::Cancelled
```

### Structured Scope

```rust
let scope = system.scope();
let a = scope.run(Context::BACKGROUND, || compute_a());
let b = scope.run(Context::BACKGROUND, || compute_b());
// Dropping scope cancels any unfinished tasks
```

### Custom Context

```rust
use orkester::{Runtime, Context, Executor};

struct MyExecutor;
impl Executor for MyExecutor {
    fn execute(&self, task: Box<dyn FnOnce() + Send + 'static>) {
        std::thread::spawn(task);
    }
}

let system = Runtime::with_threads(4);
let my_ctx = system.register_context(MyExecutor);
let result = system.run(my_ctx, || 42);
assert_eq!(result.block().unwrap(), 42);
```

## Notes

- Use `block_with_main` when completion depends on queued main-thread continuations
- `Task<T>` is single-consumer; `SharedTask<T>` is multi-consumer
- `Resolver<T>` should always be resolved or rejected explicitly for predictable behavior
- Under `custom-runtime`, async methods (`run_async`, `then_async`, `spawn`) drive
  futures via the executor's `spawn_future`. For full async I/O support,
  use `tokio-runtime`.

## License

Apache-2.0
