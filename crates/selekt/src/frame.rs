//! Frame-level selection result and height sampling types.

use glam::DMat4;

use crate::node::NodeId;

/// A single render-ready node yielded by [`SelectionEngine::render_nodes`].
///
/// Bundles the node identity, its accumulated world-space transform (for use
/// as a model-matrix push-constant or uniform), and a reference to the
/// renderer-owned content produced by
/// [`PrepareRendererResources::prepare_in_main_thread`].
pub struct RenderNode<'a, C> {
    /// The node's identity within the hierarchy.
    pub id: NodeId,
    /// Accumulated world-space transform for this node.
    ///
    /// Equivalent to the product of all ancestor local transforms × this
    /// node's own local transform, expressed as a column-major `f64` matrix.
    /// Feed this into a GPU push-constant or per-draw uniform buffer to
    /// position the tile without baking the transform into vertex data.
    pub world_transform: DMat4,
    /// The renderer-owned GPU content for this node.
    pub content: &'a C,
}

/// Frame-level aggregate returned by [`SelectionEngine::update`].
///
/// Heap-allocated buffers are reused across frames (cleared and refilled in-place each call).
#[must_use = "read nodes_to_render to know what to render"]
#[derive(Clone, Debug, Default)]
pub struct FrameResult {
    /// Nodes whose content should be rendered this frame.
    pub nodes_to_render: Vec<NodeId>,

    /// Nodes that were rendered last frame but are absent from the render set
    /// this frame (culled, faded out, or replaced by children).
    ///
    /// Non-empty means geometry is transitioning; renderers can apply fade-out
    /// effects to each listed node.
    pub nodes_fading_out: Vec<NodeId>,

    /// Total nodes visited during traversal this frame.
    pub nodes_visited: usize,
    /// Nodes rejected by frustum culling or excluders this frame.
    pub nodes_culled: usize,
    /// Nodes rejected specifically by the occlusion tester this frame.
    pub nodes_occluded: usize,
    /// Nodes whose refinement was blocked because too many descendants are loading.
    pub nodes_kicked: usize,
    /// Monotonically increasing frame counter.
    pub frame_number: u64,
    /// Total bytes of content currently resident in the cache.
    pub bytes_resident: usize,
    /// Number of nodes that transitioned to `Renderable` during this frame.
    ///
    /// Set by tiles3d-selekt; zero when using `SelectionEngine` directly.
    pub nodes_newly_renderable: usize,
}

/// Result of a [`SelectionEngine::pick`] call for a single node.
#[derive(Clone, Debug)]
pub struct PickResult {
    /// The node whose bounding volume was intersected.
    pub node_id: NodeId,
    /// Distance along the ray to the first intersection with this node's bounding volume.
    pub distance: f64,
}
