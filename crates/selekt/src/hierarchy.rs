use crate::load::ContentKey;
use crate::lod::{LodDescriptor, RefinementMode};
use crate::node::{NodeId, NodeKind};

use zukei::SpatialBounds;

/// Read-only description of a spatial scene graph.
///
/// A scene graph is an immutable structural description of spatial nodes: their
/// bounds, LOD descriptors, content keys, and spatial relationships. It does
/// **not** own loaded content — the engine manages that separately.
///
/// Implement this for your format alongside [`ContentLoader`](crate::load::ContentLoader).
/// New nodes become visible to traversal the frame after they are returned as
/// part of a [`SceneRef`](crate::load::SceneRef).
///
/// # Thread safety
///
/// Implementations must be `Send + Sync`. The engine holds a shared reference
/// to the scene graph across threads during traversal.
pub trait SceneGraph: Send + Sync + 'static {
    /// Returns the root node identifier.
    fn root(&self) -> NodeId;

    /// Returns the parent of `node`, or `None` if `node` is the root.
    fn parent(&self, node: NodeId) -> Option<NodeId>;

    /// Returns the children of `node`.
    fn children(&self, node: NodeId) -> &[NodeId];

    /// Structural classification of the node.
    fn node_kind(&self, node: NodeId) -> NodeKind;

    /// Bounding volume expressed in the engine's working coordinate system.
    fn bounds(&self, node: NodeId) -> &SpatialBounds;

    /// LOD descriptor used by `LodEvaluator` to decide refinement.
    fn lod_descriptor(&self, node: NodeId) -> &LodDescriptor;

    /// How children contribute to the parent's region.
    fn refinement_mode(&self, node: NodeId) -> RefinementMode;

    /// Stable content addresses for the node, if any.
    /// Returns an empty slice for structural/interior-only nodes.
    fn content_keys(&self, node: NodeId) -> &[ContentKey];

    /// Total number of nodes in this graph.
    ///
    /// Used by the engine's internal [`GraphSet`](crate::composite::GraphSet) to
    /// assign non-overlapping [`NodeId`] ranges when multiple scene graphs are
    /// active simultaneously.
    fn node_count(&self) -> usize;

    /// Optional tighter bounding volume covering only this node's content,
    /// excluding child contributions.
    fn content_bounds(&self, node: NodeId) -> Option<&SpatialBounds> {
        let _ = node;
        None
    }

    /// Optional viewer request volume: subtree is only traversed when the
    /// primary camera is inside this volume.
    fn viewer_request_volume(&self, node: NodeId) -> Option<&SpatialBounds> {
        let _ = node;
        None
    }

    /// Per-node LOD metric override (replaces `LodDescriptor::value` if `Some`).
    fn lod_metric_override(&self, node: NodeId) -> Option<f64> {
        let _ = node;
        None
    }

    /// Maximum age of content for this node before it is re-fetched.
    fn content_max_age(&self, node: NodeId) -> Option<std::time::Duration> {
        let _ = node;
        None
    }

    /// The accumulated world-space transform for this node.
    ///
    /// Returns the product of all ancestor local transforms down to (and
    /// including) this node.  The default is [`DMat4::IDENTITY`](glam::DMat4::IDENTITY),
    /// meaning the node has no local transform and its content is already in
    /// world space (ECEF for geospatial data).
    ///
    /// Used by the engine to:
    /// - Transform bounding volumes for frustum culling and LOD evaluation.
    /// - Propagate parent coordinate frames into sub-scenes returned via
    ///   [`SceneRef`](crate::load::SceneRef).
    fn world_transform(&self, node: NodeId) -> glam::DMat4 {
        let _ = node;
        glam::DMat4::IDENTITY
    }
}

/// Blanket impl so `Box<dyn SceneGraph>` can be used as a `SceneGraph` directly.
impl SceneGraph for Box<dyn SceneGraph> {
    fn root(&self) -> NodeId {
        (**self).root()
    }
    fn parent(&self, node: NodeId) -> Option<NodeId> {
        (**self).parent(node)
    }
    fn children(&self, node: NodeId) -> &[NodeId] {
        (**self).children(node)
    }
    fn node_kind(&self, node: NodeId) -> NodeKind {
        (**self).node_kind(node)
    }
    fn bounds(&self, node: NodeId) -> &SpatialBounds {
        (**self).bounds(node)
    }
    fn lod_descriptor(&self, node: NodeId) -> &LodDescriptor {
        (**self).lod_descriptor(node)
    }
    fn refinement_mode(&self, node: NodeId) -> RefinementMode {
        (**self).refinement_mode(node)
    }
    fn content_keys(&self, node: NodeId) -> &[ContentKey] {
        (**self).content_keys(node)
    }
    fn node_count(&self) -> usize {
        (**self).node_count()
    }
    fn content_bounds(&self, node: NodeId) -> Option<&SpatialBounds> {
        (**self).content_bounds(node)
    }
    fn viewer_request_volume(&self, node: NodeId) -> Option<&SpatialBounds> {
        (**self).viewer_request_volume(node)
    }
    fn lod_metric_override(&self, node: NodeId) -> Option<f64> {
        (**self).lod_metric_override(node)
    }
    fn content_max_age(&self, node: NodeId) -> Option<std::time::Duration> {
        (**self).content_max_age(node)
    }
    fn world_transform(&self, node: NodeId) -> glam::DMat4 {
        (**self).world_transform(node)
    }
}
