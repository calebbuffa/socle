# orkester

Runtime-agnostic async continuation library for Rust.

*orkester is Russian for "orchestra" — orchestrating asynchronous and concurrent tasks.*

## Overview

orkester provides a deterministic, continuation-based async runtime with explicit scheduling
contexts. It replaces traditional watcher-task-per-continuation designs with a lightweight
signal-and-slot model that integrates with any threading backend.

**Core types:**

- **`AsyncSystem`** — root runtime object; owns schedulers and the main-thread queue
- **`Promise<T>` / `Future<T>`** — single-producer / single-consumer async pair
- **`SharedFuture<T>`** — cloneable multi-consumer future (requires `T: Clone`)
- **`ThreadPool`** — dedicated thread pool created from an `AsyncSystem`
- **`TaskProcessor`** — trait for plugging in custom threading backends

## Design Principles

- No `AsyncSystem` outliving requirement for futures
- No watcher-task-per-continuation overhead
- Main-thread work queue with deterministic pumping
- `std::future::Future` integration for both `Future<T>` and `SharedFuture<T>`
- No `unsafe` in the core implementation

## Quick Start

```rust
use std::sync::Arc;
use orkester::{AsyncSystem, Context, ThreadPoolTaskProcessor};

// Create a system with a 4-thread pool
let system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::new(4)));

// Promise/future pair
let (promise, future) = system.create_promise::<i32>();
promise.resolve(42);
assert_eq!(future.wait().unwrap(), 42);

// Run work on a background thread
let result = system.run(Context::Worker, || 5);
assert_eq!(result.wait().unwrap(), 5);

// Continuation chains — closures can return values or Future<T>
let chained = system
    .create_resolved_future(3)
    .then(Context::Worker, |v| v + 1)
    .then(Context::Worker, {
        let system = system.clone();
        move |v| system.create_resolved_future(v * 2)
    });
assert_eq!(chained.wait().unwrap(), 8);
```

## Scheduling Contexts

orkester provides scheduling via the `Context` enum:

| Context | Description |
|---------|-------------|
| `Context::Worker` | Runs on `TaskProcessor` thread |
| `Context::Main` | Runs on main thread (pumped or inline) |
| `Context::Immediate` | Runs inline on the completing thread |

For dedicated thread pools, use `run_in_pool` / `then_in_pool`.

Main-thread work is either executed inline (inside `enter_main_thread` scope) or queued
for explicit pumping via `dispatch_main_thread_tasks()`.

## API Reference

### `AsyncSystem`

```rust
AsyncSystem::new(processor: Arc<dyn TaskProcessor>) -> AsyncSystem
AsyncSystem::create_thread_pool(&self, num_threads: usize) -> ThreadPool

// Promise/future creation
AsyncSystem::create_promise<T>(&self) -> (Promise<T>, Future<T>)
AsyncSystem::create_future<T, F>(&self, f: F) -> Future<T>
AsyncSystem::create_resolved_future<T>(&self, value: T) -> Future<T>

// Schedule work
AsyncSystem::run<T, F>(&self, context: Context, f: F) -> Future<T>
AsyncSystem::run_in_pool<T, F>(&self, pool: &ThreadPool, f: F) -> Future<T>

// Main-thread dispatch
AsyncSystem::dispatch_main_thread_tasks(&self) -> usize
AsyncSystem::dispatch_one_main_thread_task(&self) -> bool
AsyncSystem::enter_main_thread(&self) -> MainThreadScope

// Combinators
AsyncSystem::all<T, I, W>(&self, futures: I) -> Future<Vec<T>>
```

`AsyncSystem` is `Clone` (cheap `Arc` clone) and `PartialEq`/`Eq` by identity.

### `Promise<T>`

```rust
Promise::resolve(self, value: T)
Promise::reject(self, error: impl Into<AsyncError>)
```

Dropping an unresolved `Promise<T>` auto-rejects with `"Promise dropped without resolving"`.

### `Future<T>`

```rust
Future::is_ready(&self) -> bool
Future::wait(self) -> Result<T, AsyncError>
Future::wait_in_main_thread(self) -> Result<T, AsyncError>

// Continuations — closures may return T or Future<T> (auto-flattened)
Future::then<U, F>(self, context: Context, f: F) -> Future<U>
Future::then_in_pool<U, F>(self, pool: &ThreadPool, f: F) -> Future<U>
Future::then_immediately<U, F>(self, f: F) -> Future<U>

// Error recovery
Future::catch<F>(self, context: Context, f: F) -> Future<T>
Future::catch_immediately<F>(self, f: F) -> Future<T>

// Convert to multi-consumer (requires T: Clone)
Future::share(self) -> SharedFuture<T>
```

`Future<T>` implements `std::future::Future<Output = Result<T, AsyncError>>`.

### `SharedFuture<T>`

Same continuation API as `Future<T>`, but borrows `&self` instead of consuming.
Can be cloned and waited on multiple times — each consumer receives a cloned result.

### `AsyncError`

```rust
AsyncError::new<E: Error + Send + Sync + 'static>(error: E) -> AsyncError
AsyncError::msg(message: impl Into<String>) -> AsyncError
AsyncError::downcast_ref<E: Error + 'static>(&self) -> Option<&E>
```

Converts from `String`, `&str`, and `Box<dyn Error + Send + Sync>`.

### `TaskProcessor`

```rust
pub trait TaskProcessor: Send + Sync {
    fn start_task(&self, task: Box<dyn FnOnce() + Send + 'static>);
}
```

Built-in implementations:

- `ThreadPoolTaskProcessor::new(num_threads)` — fixed-size thread pool
- `ThreadPoolTaskProcessor::default_pool()` — uses `num_cpus` threads

## Examples

### Main-Thread Dispatch

```rust
// Outside enter_main_thread: main-thread work is queued
let main_result = system.run(Context::Main, || 7);
system.dispatch_main_thread_tasks();
assert_eq!(main_result.wait().unwrap(), 7);

// Inside enter_main_thread: main-thread work runs inline
let _scope = system.enter_main_thread();
let immediate = system.run(Context::Main, || 9);
assert!(immediate.is_ready());
```

### Error Handling

```rust
let recovered = system
    .create_future(|promise| promise.reject("boom"))
    .catch(Context::Main, |_err| 99);
assert_eq!(recovered.wait_in_main_thread().unwrap(), 99);
```

### SharedFuture Fan-Out

```rust
let shared = system.create_resolved_future(10).share();
let a = shared.then(Context::Worker, |v| v + 1);
let b = shared.then(Context::Worker, |v| v * 3);
assert_eq!(a.wait().unwrap(), 11);
assert_eq!(b.wait().unwrap(), 30);
```

### Joining Futures

```rust
let joined = system.all(vec![
    system.create_resolved_future(1),
    system.create_resolved_future(2),
    system.create_resolved_future(3),
]);
assert_eq!(joined.wait().unwrap(), vec![1, 2, 3]);
```

## Notes

- Use `wait_in_main_thread` when completion depends on queued main-thread continuations
- `Future<T>` is single-consumer; `SharedFuture<T>` is multi-consumer
- `Promise<T>` should always be resolved or rejected explicitly for predictable behavior

## License

Apache-2.0
