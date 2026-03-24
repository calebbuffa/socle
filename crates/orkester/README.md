# orkester

Context-aware task scheduling for Rust. Works with tokio, ships with FFI.

*orkester is Russian for "orchestra" — orchestrating asynchronous and concurrent tasks.*

## Overview

orkester is **the scheduling policy layer for Rust**. It doesn't replace tokio — it sits
on top, adding context-aware dispatch, thread affinity, and a C FFI.

- **tokio** answers: *"run this async task somewhere."*
- **orkester** answers: *"run this task **here**, on **this context**, with **this priority**, and give me the result."*

**Core types:**

- **`Scheduler`** — root runtime object; owns executors and the main-thread queue
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
use orkester::{Scheduler, Context};

// Create a scheduler with a default thread pool
let system = Scheduler::with_threads(4);

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
use orkester::{Scheduler, Context};

let system = Scheduler::with_threads(4);

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

Create a scheduler with a built-in thread pool:

```rust
use orkester::Scheduler;

let system = Scheduler::with_threads(4);
```

Or use the builder for more control:

```rust
let system = Scheduler::builder()
    .executor(MyCustomExecutor::new())
    .build();
```

### Tokio Runtime

Use tokio's runtime for async task spawning:

```rust
use orkester::{Scheduler, TokioExecutor};

let system = Scheduler::builder()
    .executor(TokioExecutor::current())
    .build();
```

### WASM

For WebAssembly targets with `wasm-bindgen-futures`:

```rust
use orkester::{Scheduler, WasmExecutor};

let system = Scheduler::builder()
    .executor(WasmExecutor)
    .build();

// spawn_local — no Send required on the future
let result = system.spawn_local(async { compute() });
```

## API Reference

### `Scheduler`

```rust
// Construction
Scheduler::new(executor: impl Executor) -> Scheduler
Scheduler::with_threads(n: usize) -> Scheduler
Scheduler::builder() -> SchedulerBuilder

// Context management
Scheduler::register_context(&self, executor: impl Executor) -> Context
Scheduler::thread_pool(&self, num_threads: usize) -> ThreadPool

// Resolver/task creation
Scheduler::resolver<T>(&self) -> (Resolver<T>, Task<T>)
Scheduler::task<T, F>(&self, f: F) -> Task<T>
Scheduler::resolved<T>(&self, value: T) -> Task<T>

// Schedule work (sync closures)
Scheduler::run<T, F>(&self, context: Context, f: F) -> Task<T>
Scheduler::run_in_pool<T, F>(&self, pool: &ThreadPool, f: F) -> Task<T>

// Schedule work (async)
Scheduler::run_async<T, F, Fut>(&self, context: Context, f: F) -> Task<T>
Scheduler::spawn<T, Fut>(&self, future: Fut) -> Task<T>  // on BACKGROUND
Scheduler::spawn_local<T, Fut>(&self, future: Fut) -> Task<T>  // WASM only
Scheduler::spawn_detached<F>(&self, context: Context, f: F)

// Main-thread dispatch
Scheduler::flush_main(&self) -> usize
Scheduler::flush_main_one(&self) -> bool
Scheduler::main_pending(&self) -> bool
Scheduler::main_scope(&self) -> MainThreadScope

// Structured concurrency
Scheduler::scope(&self) -> Scope
Scheduler::join_set<T>(&self) -> JoinSet<T>
Scheduler::join_all<T, I>(&self, tasks: I) -> Task<Vec<T>>

// Timer
Scheduler::delay(&self, duration: Duration) -> Task<()>
```

`Scheduler` is `Clone` (cheap `Arc` clone).

### `SchedulerBuilder`

```rust
SchedulerBuilder::executor(self, executor: impl Executor) -> Self
SchedulerBuilder::context(self, executor: impl Executor) -> Self
SchedulerBuilder::build(self) -> Scheduler
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
Task::system(&self) -> Scheduler
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
Semaphore::new(system: &Scheduler, permits: usize) -> Semaphore
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
timeout<T>(system: &Scheduler, task: Task<T>, duration: Duration) -> Task<T>
race<T>(system: &Scheduler, tasks: Vec<Task<T>>) -> Task<T>
retry<T, F>(system: &Scheduler, max_attempts: u32, config: RetryConfig, f: F) -> Task<T>
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
use orkester::{Scheduler, Context, Executor};

struct MyExecutor;
impl Executor for MyExecutor {
    fn execute(&self, task: Box<dyn FnOnce() + Send + 'static>) {
        std::thread::spawn(task);
    }
}

let system = Scheduler::with_threads(4);
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
