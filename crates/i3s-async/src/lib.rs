//! Concurrent resource fetching and task scheduling for I3S services and SLPK archives.
//!
//! Provides the [`AssetAccessor`] trait (synchronous blocking I/O), the
//! [`ResourceUriResolver`] trait (I3S-specific URI construction), the
//! [`TaskProcessor`] trait (user-provided thread pool), and an explicit
//! [`AsyncSystem`] with [`Future`]/[`Promise`] types modeled after
//! cesium-native's `CesiumAsync` library.
//!
//! ## Concurrency model
//!
//! Concurrency is achieved via a bounded thread pool ([`TaskProcessor`]).
//! Worker threads call [`AssetAccessor::get`] which blocks (fine on a worker
//! thread — that is the thread pool's purpose). Results flow back to the main
//! thread via [`Future`]/[`Promise`] and [`MainThreadQueue::dispatch`].
//!
//! The [`AsyncSystem`] / [`Future`] / [`Promise`] types expose an async
//! programming model to callers (including Python `await` integration) without
//! requiring a Tokio or async-std runtime.
//!
//! | cesium-native | i3s-async |
//! |---|---|
//! | `AsyncSystem` | [`AsyncSystem`] |
//! | `Future<T>` | [`Future<T>`] |
//! | `Promise<T>` | [`Promise<T>`] |
//! | `SharedFuture<T>` | [`SharedFuture<T>`] |
//! | `IAssetAccessor` | [`AssetAccessor`] |
//! | `IAssetRequest` | [`AssetRequest`] |
//! | `IAssetResponse` | [`AssetResponse`] |
//! | `ITaskProcessor` | [`TaskProcessor`] |
//! | `dispatchMainThreadTasks` | [`AsyncSystem::dispatch_main_thread_tasks`] |

pub mod accessor;
pub mod async_system;
pub mod i3s_accessor;
pub mod request;
pub mod resolver;
pub mod resource;
pub mod task_processor;

pub use accessor::AssetAccessor;
pub use async_system::{AsyncSystem, Future, MainThreadQueue, Promise, SharedFuture};
pub use i3s_accessor::I3sAssetAccessor;
pub use request::{AssetRequest, AssetResponse, Headers};
pub use resolver::{ResourceUriResolver, RestUriResolver, SlpkUriResolver};
pub use resource::TextureRequestFormat;
pub use task_processor::{TaskProcessor, ThreadPoolTaskProcessor};
