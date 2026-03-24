//! Context-aware task scheduling for Rust.
//!
//! `orkester` provides:
//! - [`Scheduler`] — root async runtime with context-aware scheduling
//! - [`Task`] / [`SharedTask`] / [`Resolver`] — async value types
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
mod join_set;
mod main_loop;
mod resolver;
mod scheduler;
mod scope;
mod semaphore;
pub(crate) mod task;
mod task_cell;
mod task_processor;
mod thread_pool;
mod timer;

pub use cancellation::CancellationToken;
pub use channel::{Receiver, SendError, Sender, TrySendError};
pub use combinators::{RetryConfig, race, retry, timeout};
pub use context::Context;
pub use error::{AsyncError, ErrorCode};
pub use executor::Executor;
#[cfg(feature = "tokio-runtime")]
pub use executor::TokioExecutor;
#[cfg(feature = "wasm")]
pub use executor::WasmExecutor;
pub use join_set::JoinSet;
pub use main_loop::MainThreadScope;
pub use resolver::Resolver;
pub use scheduler::{Scheduler, SchedulerBuilder};
pub use scope::Scope;
pub use semaphore::{Semaphore, SemaphorePermit};
pub use task::{SharedTask, Task};
pub use thread_pool::ThreadPool;
