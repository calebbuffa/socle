//! External dependencies injected into a scene layer.
//!
//! Modeled after cesium-native's `TilesetExternals` (renamed to match I3S terminology). Bundles the
//! user-provided interfaces that the library needs to function:
//!
//! 1. **Async system** ([`AsyncSystem`]) — worker pool + main-thread queue
//! 2. **Renderer resources** ([`PrepareRendererResources`]) — creates GPU objects

use std::sync::Arc;

use i3s_async::AsyncSystem;

use crate::prepare::PrepareRendererResources;

/// External dependencies for a [`SceneLayer`](crate::SceneLayer).
///
/// This is the Rust equivalent of cesium-native's `TilesetExternals`,
/// renamed to match I3S terminology (`SceneLayer` instead of `Tileset`).
/// Construct one and pass it to [`SceneLayer::open`](crate::SceneLayer::open).
///
/// # Example
///
/// ```ignore
/// use i3s_async::{AsyncSystem, ThreadPoolTaskProcessor};
/// use i3s_selection::{SceneLayerExternals, NoopPrepareRendererResources};
///
/// let async_system = AsyncSystem::new(Arc::new(ThreadPoolTaskProcessor::default_pool()));
/// let externals = SceneLayerExternals {
///     async_system,
///     prepare_renderer_resources: Arc::new(NoopPrepareRendererResources),
/// };
/// ```
#[derive(Clone)]
pub struct SceneLayerExternals {
    /// The async system for dispatching work and main-thread callbacks.
    ///
    /// Shared across all scene layers. Wraps the user-provided
    /// [`TaskProcessor`](i3s_async::TaskProcessor) and a main-thread queue.
    pub async_system: AsyncSystem,

    /// The renderer resource preparer.
    ///
    /// Creates GPU-ready objects from decoded I3S content. Use
    /// [`NoopPrepareRendererResources`](crate::NoopPrepareRendererResources)
    /// for headless / testing scenarios.
    pub prepare_renderer_resources: Arc<dyn PrepareRendererResources>,
}
