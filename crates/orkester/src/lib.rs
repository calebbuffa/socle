//! Context-aware task scheduling for Rust.
//!
//! `orkester` provides:
//! - [`AsyncSystem`] — root async runtime with context-aware scheduling
//! - [`Future`] / [`SharedFuture`] / [`Promise`] — async value types
//! - [`Context`] — lightweight scheduling handle (u32-indexed)
//! - [`Executor`] — trait for custom execution backends
//!
//! # Feature Flags
//!
//! | Feature | Description |
//! |---------|-------------|
//! | `custom-runtime` *(default)* | Built-in thread pool executor |
//! | `tokio-runtime` | [`TokioExecutor`] backend via `tokio::runtime::Handle` |
//! | `wasm` | [`WasmExecutor`] + `spawn_local` for WebAssembly targets |

mod block_on;
mod cancellation;
pub mod channel;
mod combinators;
mod context;
mod error;
mod executor;
mod future;
mod join_set;
mod main_thread;
mod promise;
mod semaphore;
mod state;
mod system;
mod task_processor;
mod thread_pool;

pub use cancellation::CancellationToken;
pub use channel::{Receiver, SendError, Sender, TrySendError};
pub use combinators::{RetryConfig, delay, race, retry, timeout};
pub use context::Context;
pub use error::{AsyncError, ErrorCode};
pub use executor::Executor;
#[cfg(feature = "tokio-runtime")]
pub use executor::TokioExecutor;
#[cfg(feature = "wasm")]
pub use executor::WasmExecutor;
pub use future::{Future, SharedFuture};
pub use join_set::JoinSet;
pub use main_thread::MainThreadScope;
pub use promise::Promise;
pub use semaphore::{Semaphore, SemaphorePermit};
pub use system::{AsyncSystem, AsyncSystemBuilder};

pub use thread_pool::ThreadPool;
