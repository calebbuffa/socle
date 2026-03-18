//! Spatial node exclusion — mirrors `ITileExcluder` in cesium-native.

use std::sync::Arc;

use i3s_geometry::obb::OrientedBoundingBox;

/// Callback that can exclude individual nodes from LOD selection.
///
/// Mirrors `ITileExcluder` in `Cesium3DTilesSelection`. Register instances via
/// [`SceneLayerExternals::excluders`](crate::SceneLayerExternals).
///
/// If `should_exclude` returns `true` for a node, that node and all its
/// descendants are skipped. Excluded nodes are added to
/// `ViewUpdateResult::nodes_to_unload` so their GPU resources are freed.
pub trait NodeExcluder: Send + Sync {
    /// Called once at the start of each [`SceneLayer::update_view`](crate::SceneLayer::update_view).
    ///
    /// Use this to reset per-frame state. The default implementation is a no-op.
    fn start_new_frame(&mut self) {}

    /// Return `true` if the node with this bounding box should be excluded.
    fn should_exclude(&self, obb: &OrientedBoundingBox) -> bool;
}

/// Test a slice of excluders against one OBB.
pub(crate) fn is_excluded(excluders: &[Arc<dyn NodeExcluder>], obb: &OrientedBoundingBox) -> bool {
    excluders.iter().any(|e| e.should_exclude(obb))
}
