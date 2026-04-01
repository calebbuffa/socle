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

**Free function combinators:** `delay`, `timeout`, `race`, `retry`, `join_all`, `resolved`, `pair`

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
let (resolver, task) = orkester::pair::<i32>();
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
let (resolver, task) = orkester::pair::<i32>();
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
orkester::pair<T>() -> (Resolver<T>, Task<T>)
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
