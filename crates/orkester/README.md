# orkester

Context-aware task scheduling for Rust. Works with tokio, ships with FFI.

*orkester is Russian for "orchestra" — orchestrating asynchronous and concurrent tasks.*

## Overview

orkester is **the scheduling policy layer for Rust**. It doesn't replace tokio — it sits
on top, adding context-aware dispatch, thread affinity, and a C FFI.

- **tokio** answers: *"run this async task somewhere."*
- **orkester** answers: *"run this task **here**, on **this context**, with **this priority**, and give me the result."*

**Core types:**

- **`AsyncSystem`** — root runtime object; owns executors and the main-thread queue
- **`Promise<T>` / `Future<T>`** — single-producer / single-consumer async pair
- **`SharedFuture<T>`** — cloneable multi-consumer future (requires `T: Clone`)
- **`Context`** — lightweight handle identifying a scheduling target (u32-indexed)
- **`Executor`** — trait for custom execution backends


## Feature Flags

```toml
[dependencies]
orkester = "0.2"                            # default: custom-runtime

# With tokio backend
orkester = { version = "0.2", features = ["tokio-runtime"] }

# For WASM targets
orkester = { version = "0.2", features = ["wasm"] }
```

| Feature | Description |
|---------|-------------|
| `custom-runtime` *(default)* | Built-in thread pool executor |
| `tokio-runtime` | `TokioExecutor` backend via `tokio::runtime::Handle` |
| `wasm` | `WasmExecutor` + `spawn_local` for WebAssembly targets |

## Design Principles

- No `AsyncSystem` outliving requirement for futures
- No watcher-task-per-continuation overhead
- Main-thread work queue with deterministic pumping
- `std::future::Future` integration for both `Future<T>` and `SharedFuture<T>`
- Extensible scheduling via user-defined contexts
- Dual API: callback chains AND async/await
- No `unsafe` in the core implementation

## Quick Start

```rust
use orkester::{AsyncSystem, Context};

// Create a system with a default thread pool
let system = AsyncSystem::default();

// Promise/future pair
let (promise, future) = system.promise::<i32>();
promise.resolve(42);
assert_eq!(future.block().unwrap(), 42);

// Run work on a background thread
let result = system.run(Context::BACKGROUND, || 5);
assert_eq!(result.block().unwrap(), 5);

// Continuation chains — closures can return values or Future<T>
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
use orkester::{AsyncSystem, Context};

let system = AsyncSystem::default();

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
let gpu = system.register_context("gpu", GpuThreadExecutor::new());
system.run(gpu, || upload_texture(data));
```

For dedicated thread pools, use `run_in_pool` / `then_in_pool`.

Main-thread work is either executed inline (inside `main_scope()`) or queued
for explicit pumping via `flush_main()`.

## Runtime Backends

### Custom Runtime (default)

Create a system with a built-in thread pool:

```rust
use orkester::AsyncSystem;

let system = AsyncSystem::with_threads(4);
```

Or use the builder for more control:

```rust
let system = AsyncSystem::builder()
    .executor(MyCustomExecutor::new())
    .build();
```

### Tokio Runtime

Use tokio's runtime for async task spawning:

```rust
use orkester::{AsyncSystem, TokioExecutor};

let system = AsyncSystem::builder()
    .executor(TokioExecutor::current())
    .build();
```

### WASM

For WebAssembly targets with `wasm-bindgen-futures`:

```rust
use orkester::{AsyncSystem, WasmExecutor};

let system = AsyncSystem::builder()
    .executor(WasmExecutor)
    .build();

// spawn_local — no Send required on the future
let result = system.spawn_local(async { compute() });
```

## API Reference

### `AsyncSystem`

```rust
// Construction
AsyncSystem::new(executor: impl Executor) -> AsyncSystem
AsyncSystem::with_threads(n: usize) -> AsyncSystem
AsyncSystem::builder() -> AsyncSystemBuilder
AsyncSystem::default() -> AsyncSystem  // built-in thread pool

// Context management
AsyncSystem::register_context(&self, name: &str, executor: impl Executor) -> Context
AsyncSystem::thread_pool(&self, num_threads: usize) -> ThreadPool

// Promise/future creation
AsyncSystem::promise<T>(&self) -> (Promise<T>, Future<T>)
AsyncSystem::future<T, F>(&self, f: F) -> Future<T>
AsyncSystem::resolved<T>(&self, value: T) -> Future<T>

// Schedule work (sync closures)
AsyncSystem::run<T, F>(&self, context: Context, f: F) -> Future<T>
AsyncSystem::run_in_pool<T, F>(&self, pool: &ThreadPool, f: F) -> Future<T>
AsyncSystem::spawn_detached(&self, context: Context, f: F)  // fire-and-forget

// Schedule work (async)
AsyncSystem::run_async<T, F, Fut>(&self, context: Context, f: F) -> Future<T>
AsyncSystem::spawn<T, Fut>(&self, future: Fut) -> Future<T>  // on BACKGROUND
AsyncSystem::spawn_local<T, Fut>(&self, future: Fut) -> Future<T>  // WASM only

// Main-thread dispatch
AsyncSystem::flush_main(&self) -> usize
AsyncSystem::flush_main_one(&self) -> bool
AsyncSystem::main_pending(&self) -> bool
AsyncSystem::main_scope(&self) -> MainThreadScope

// Combinators
AsyncSystem::join_all<T, I>(&self, futures: I) -> Future<Vec<T>>
```

`AsyncSystem` is `Clone` (cheap `Arc` clone) and `PartialEq`/`Eq` by identity.

### `AsyncSystemBuilder`

```rust
AsyncSystemBuilder::executor(self, executor: impl Executor) -> Self
AsyncSystemBuilder::context(self, name: &str, executor: impl Executor) -> Self
AsyncSystemBuilder::build(self) -> AsyncSystem
```

### `Promise<T>`

```rust
Promise::resolve(self, value: T)
Promise::reject(self, error: impl Into<AsyncError>)
```

Dropping an unresolved `Promise<T>` auto-rejects with `"Promise dropped without resolving"`.

### `Future<T>`

```rust
Future::is_ready(&self) -> bool
Future::block(self) -> Result<T, AsyncError>
Future::block_with_main(self) -> Result<T, AsyncError>

// Continuations — closures may return T or Future<T> (auto-flattened)
Future::then<U, F>(self, context: Context, f: F) -> Future<U>
Future::then_in_pool<U, F>(self, pool: &ThreadPool, f: F) -> Future<U>
Future::then_async<U, F, Fut>(self, context: Context, f: F) -> Future<U>
Future::map<U, F>(self, f: F) -> Future<U>          // = then(IMMEDIATE, f)
Future::and_then<V, E, F>(self, f: F) -> Future<Result<V, E>>

// Error recovery
Future::catch<F>(self, context: Context, f: F) -> Future<T>
Future::or_else<F>(self, f: F) -> Future<T>          // = catch(IMMEDIATE, f)

// Cancellation
Future::with_cancellation(self, token: &CancellationToken) -> Future<T>

// Convert to multi-consumer (requires T: Clone)
Future::share(self) -> SharedFuture<T>
```

`Future<T>` implements `std::future::Future<Output = Result<T, AsyncError>>`.

### `SharedFuture<T>`

Same continuation API as `Future<T>` (including `then_async`), but borrows `&self`
instead of consuming. Can be cloned and waited on multiple times — each consumer
receives a cloned result.

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
// Runtime error (cancellation, timeout, dropped promise)
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
    .future(|promise| promise.reject("boom"))
    .catch(Context::MAIN, |_err| 99);
assert_eq!(recovered.block_with_main().unwrap(), 99);
```

### SharedFuture Fan-Out

```rust
let shared = system.resolved(10).share();
let a = shared.then(Context::BACKGROUND, |v| v + 1);
let b = shared.then(Context::BACKGROUND, |v| v * 3);
assert_eq!(a.block().unwrap(), 11);
assert_eq!(b.block().unwrap(), 30);
```

### Joining Futures

```rust
let joined = system.join_all(vec![
    system.resolved(1),
    system.resolved(2),
    system.resolved(3),
]);
assert_eq!(joined.block().unwrap(), vec![1, 2, 3]);
```

### Custom Context

```rust
use orkester::{AsyncSystem, Context, Executor};

struct MyExecutor;
impl Executor for MyExecutor {
    fn execute(&self, task: Box<dyn FnOnce() + Send + 'static>) {
        std::thread::spawn(task);
    }
}

let system = AsyncSystem::default();
let my_ctx = system.register_context("custom", MyExecutor);
let result = system.run(my_ctx, || 42);
assert_eq!(result.block().unwrap(), 42);
```

## Notes

- Use `block_with_main` when completion depends on queued main-thread continuations
- `Future<T>` is single-consumer; `SharedFuture<T>` is multi-consumer
- `Promise<T>` should always be resolved or rejected explicitly for predictable behavior
- Under `custom-runtime`, async methods (`run_async`, `then_async`, `spawn`) drive
  futures via the executor's `spawn_future`. For full async I/O support,
  use `tokio-runtime`.

## License

Apache-2.0
