use crate::node::NodeId;
use crate::view::ViewState;
use zukei::bounds::SpatialBounds;


/// Renderer-reported occlusion state for a single node.
///
/// Renderers that support occlusion queries implement this via
/// [`OcclusionTester`]. The traversal defers refinement if a node
/// reports [`OcclusionState::Occluded`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OcclusionState {
    /// Occlusion testing is not available for this node (treat as visible).
    Unavailable,
    /// The node was determined to be not occluded last frame.
    NotOccluded,
    /// The node was determined to be fully occluded last frame.
    Occluded,
}

/// Optional renderer callback for occlusion-driven refinement deferral.
///
/// Implement this to feed hardware occlusion query results back into the
/// traversal. If the renderer doesn't support occlusion queries, leave the
/// default (returns [`OcclusionState::Unavailable`] for everything).
pub trait OcclusionTester: Send + Sync + 'static {
    /// Query the renderer's last-frame occlusion state for `node_id`.
    fn occlusion_state(&self, node_id: NodeId) -> OcclusionState {
        let _ = node_id;
        OcclusionState::Unavailable
    }
}

/// No-op occlusion tester that reports all nodes as unavailable.
pub struct NoOcclusion;
impl OcclusionTester for NoOcclusion {}


/// Custom per-tile predicate that can reject nodes from the selection.
///
/// Analogous to cesium-native's `ITileExcluder`. Multiple excluders can be
/// composed via [`CompositeExcluder`]. When any excluder returns `true`,
/// the node (and its subtree) is skipped.
pub trait TileExcluder: Send + Sync + 'static {
    /// Called once at the start of each frame, before any `should_exclude` calls.
    fn start_new_frame(&mut self) {}

    /// Return `true` to exclude this node and its entire subtree from traversal.
    fn should_exclude(&self, node_id: NodeId, bounds: &SpatialBounds) -> bool;
}

/// Combines multiple [`TileExcluder`]s — excludes if ANY returns `true`.
pub struct CompositeExcluder {
    excluders: Vec<Box<dyn TileExcluder>>,
}

impl CompositeExcluder {
    pub fn new(excluders: Vec<Box<dyn TileExcluder>>) -> Self {
        Self { excluders }
    }

    pub fn push(&mut self, excluder: Box<dyn TileExcluder>) {
        self.excluders.push(excluder);
    }
}

impl TileExcluder for CompositeExcluder {
    fn start_new_frame(&mut self) {
        for e in &mut self.excluders {
            e.start_new_frame();
        }
    }

    fn should_exclude(&self, node_id: NodeId, bounds: &SpatialBounds) -> bool {
        self.excluders
            .iter()
            .any(|e| e.should_exclude(node_id, bounds))
    }
}


/// Determines whether a node's bounding volume is visible within a view.
///
/// Implementations perform frustum culling, occlusion checks, or any other
/// visibility test appropriate for the format and rendering backend.
pub trait VisibilityPolicy: Send + Sync + 'static {
    /// Returns `true` if `bounds` is at least partially visible in `view`.
    fn is_visible(&self, node_id: NodeId, bounds: &SpatialBounds, view: &ViewState) -> bool;
}


/// Decides which resident nodes should be evicted to meet a memory budget.
///
/// Implementations may prioritise by LRU, distance, priority group, or any
/// other heuristic. The engine provides the full set of resident node IDs and
/// their tracked byte-sizes.
pub trait ResidencyPolicy: Send + Sync + 'static {
    /// Select nodes to evict and append their IDs to `out`.
    ///
    /// `resident_nodes` is a slice of `(NodeId, byte_size)` pairs.
    /// The policy should fill `out` until the aggregate byte-size of the
    /// remaining nodes satisfies `memory_budget_bytes`.
    fn select_evictions(
        &self,
        resident_nodes: &[(NodeId, usize)],
        memory_budget_bytes: usize,
        out: &mut Vec<NodeId>,
    );
}

/// Combined policy trait required by the engine.
/// Implement this on any type that satisfies both [`VisibilityPolicy`] and [`ResidencyPolicy`].
pub trait Policy: VisibilityPolicy + ResidencyPolicy {}

impl<T: VisibilityPolicy + ResidencyPolicy> Policy for T {}

// Allow `Box<dyn Policy>` to be used as `M: Policy`.
impl VisibilityPolicy for Box<dyn Policy> {
    fn is_visible(&self, node_id: NodeId, bounds: &SpatialBounds, view: &ViewState) -> bool {
        (**self).is_visible(node_id, bounds, view)
    }
}

impl ResidencyPolicy for Box<dyn Policy> {
    fn select_evictions(
        &self,
        resident_nodes: &[(NodeId, usize)],
        memory_budget_bytes: usize,
        out: &mut Vec<NodeId>,
    ) {
        (**self).select_evictions(resident_nodes, memory_budget_bytes, out);
    }
}
// Policy for Box<dyn Policy> is covered by the blanket impl above.


/// Frustum-culling visibility policy that builds a [`CullingVolume`] from each
/// [`ViewState`] and tests the node's [`SpatialBounds`] against it.
///
/// This is the standard implementation suitable for most adapters. Uses
/// zukei's `CullingVolume::from_fov` which produces 4 side planes (no near/far)
/// — appropriate for LOD selection where near/far clipping is a GPU concern.
#[cfg(feature = "glam")]
pub struct FrustumVisibilityPolicy;

#[cfg(feature = "glam")]
impl VisibilityPolicy for FrustumVisibilityPolicy {
    fn is_visible(&self, _node_id: NodeId, bounds: &SpatialBounds, view: &ViewState) -> bool {
        use zukei::frustum::CullingVolume;
        use zukei::glam::DVec3;

        let position = DVec3::from(&view.position);
        let direction = DVec3::from(&view.direction);
        let up = DVec3::from(&view.up);

        let cv = CullingVolume::from_fov(position, direction, up, view.fov_x, view.fov_y);
        let result = cv.visibility_bounds(bounds);
        result != zukei::CullingResult::Outside
    }
}


/// Simple LRU-based eviction policy: evicts nodes that were least recently
/// rendered until memory fits within budget.
///
/// Adapters that need more sophisticated eviction (distance-based, depth-based)
/// can implement [`ResidencyPolicy`] directly. This is a reasonable default.
pub struct LruResidencyPolicy;

impl ResidencyPolicy for LruResidencyPolicy {
    fn select_evictions(
        &self,
        resident_nodes: &[(NodeId, usize)],
        memory_budget_bytes: usize,
        out: &mut Vec<NodeId>,
    ) {
        let total: usize = resident_nodes.iter().map(|&(_, sz)| sz).sum();
        if total <= memory_budget_bytes {
            return;
        }

        // Evict from back (oldest / least important) until budget met.
        // The engine provides resident_nodes ordered by last-use frame.
        let mut freed = 0usize;
        let need_to_free = total - memory_budget_bytes;
        for &(id, sz) in resident_nodes.iter().rev() {
            if freed >= need_to_free {
                break;
            }
            out.push(id);
            freed += sz;
        }
    }
}


/// Default policy combining [`FrustumVisibilityPolicy`] and [`LruResidencyPolicy`].
///
/// Use this when you want frustum culling and LRU eviction out of the box.
#[cfg(feature = "glam")]
pub struct DefaultPolicy;

#[cfg(feature = "glam")]
impl VisibilityPolicy for DefaultPolicy {
    fn is_visible(&self, node_id: NodeId, bounds: &SpatialBounds, view: &ViewState) -> bool {
        FrustumVisibilityPolicy.is_visible(node_id, bounds, view)
    }
}

#[cfg(feature = "glam")]
impl ResidencyPolicy for DefaultPolicy {
    fn select_evictions(
        &self,
        resident_nodes: &[(NodeId, usize)],
        memory_budget_bytes: usize,
        out: &mut Vec<NodeId>,
    ) {
        LruResidencyPolicy.select_evictions(resident_nodes, memory_budget_bytes, out);
    }
}
