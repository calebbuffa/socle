//! Concurrent resource fetching and task scheduling for I3S services and SLPK archives.

pub mod accessor;
pub mod async_system;
pub mod gunzip;
pub mod i3s_accessor;
pub mod request;
pub mod resolver;
pub mod resource;
pub mod rest;
pub mod slpk;
pub mod task_processor;

pub use accessor::AssetAccessor;
pub use async_system::{
    AsyncError, AsyncSystem, Future, MainThreadQueue, MainThreadScope, Promise, SharedFuture,
};
pub use gunzip::GunzipAssetAccessor;
pub use i3s_accessor::I3sAssetAccessor;
pub use request::{AssetRequest, AssetResponse, Headers};
pub use resolver::{ResourceUriResolver, RestUriResolver, SlpkUriResolver};
pub use resource::TextureRequestFormat;
pub use rest::RestAssetAccessor;
pub use slpk::SlpkAssetAccessor;
pub use task_processor::{TaskProcessor, ThreadPoolTaskProcessor};
