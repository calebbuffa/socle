//! Runtime-agnostic async scheduling primitives.
//!
//! `orkester` provides:
//! - [`AsyncSystem`] — root async runtime
//! - [`Future`] / [`SharedFuture`] / [`Promise`] — async value types
//! - [`TaskProcessor`] — background thread dispatch

mod cancellation;
pub mod channel;
pub mod combinators;
mod context;
mod error;
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
pub use combinators::{delay, race, retry, timeout, RetryConfig};
pub use context::Context;
pub use error::{AsyncError, ErrorCode};
#[doc(hidden)]
pub use future::ResolveOutput;
pub use future::{Future, SharedFuture};
pub use join_set::JoinSet;
pub use main_thread::MainThreadScope;
pub use promise::Promise;
pub use semaphore::{Semaphore, SemaphorePermit};
pub use system::{AsyncSystem, Waitable};
pub use task_processor::{TaskProcessor, ThreadPoolTaskProcessor};
pub use thread_pool::ThreadPool;
