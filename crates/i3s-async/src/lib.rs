//! Async resource fetching for I3S services and SLPK archives.
//!
//! Provides the [`AssetAccessor`] trait (generic async I/O), the
//! [`ResourceUriResolver`] trait (I3S-specific URI construction), the
//! [`TaskProcessor`] trait (user-provided thread pool), and an explicit
//! [`AsyncSystem`] with [`Future`]/[`Promise`] types modeled after
//! cesium-native's `CesiumAsync` library.
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
pub mod request;
pub mod resolver;
pub mod resource;
pub mod task_processor;

#[cfg(feature = "rest")]
pub mod rest;

#[cfg(feature = "slpk")]
pub mod slpk;

pub use accessor::AssetAccessor;
pub use async_system::{AsyncSystem, Future, MainThreadQueue, Promise, SharedFuture};
pub use request::{AssetRequest, AssetResponse, Headers};
pub use resolver::{ResourceUriResolver, RestUriResolver, SlpkUriResolver};
pub use resource::TextureRequestFormat;
pub use task_processor::{TaskProcessor, ThreadPoolTaskProcessor, block_on};
