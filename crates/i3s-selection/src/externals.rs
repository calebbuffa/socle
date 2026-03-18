//! External dependencies injected into a scene layer.

use std::sync::Arc;

use i3s_async::AsyncSystem;

use crate::excluder::NodeExcluder;
use crate::prepare::PrepareRendererResources;

/// External dependencies for a [`SceneLayer`](crate::SceneLayer).
///
/// Construct one and pass it to [`SceneLayer::open`](crate::SceneLayer::open).
#[derive(Clone)]
pub struct SceneLayerExternals {
    pub async_system: AsyncSystem,
    pub prepare_renderer_resources: Arc<dyn PrepareRendererResources>,
    /// Node excluders. Each is called once per frame to optionally exclude
    /// nodes from traversal. Mirrors `TilesetOptions::excluders` in cesium-native.
    pub excluders: Vec<Arc<dyn NodeExcluder>>,
}
