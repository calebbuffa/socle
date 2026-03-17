//! Runtime node state tracking.

use crate::prepare::RendererResources;

/// Maximum number of load retries before a node is permanently failed.
pub const MAX_LOAD_RETRIES: u32 = 3;

/// Number of frames to wait before retrying a failed load.
pub const RETRY_COOLDOWN_FRAMES: u64 = 60;

/// Load state of a node's content (geometry, textures, attributes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeLoadState {
    /// Content has not been requested.
    Unloaded,
    /// Content fetch is in progress.
    Loading,
    /// Content is loaded and ready to render.
    Loaded,
    /// Content failed to load. May be retried if attempts < MAX_LOAD_RETRIES.
    Failed,
}

/// Runtime state for a single I3S node.
///
/// Tracks load state, per-frame selection results, the last projected
/// screen size used for LOD evaluation, and renderer resources.
///
/// Modeled after cesium-native's `Tile` — each node owns its renderer
/// resources directly rather than storing them in a global map.
pub struct NodeState {
    /// Node index in the flat node array.
    pub node_id: u32,
    /// Current load state of this node's content.
    pub load_state: NodeLoadState,
    /// Whether this node was selected for rendering in the last frame.
    pub selected: bool,
    /// Whether this node was visible (not frustum-culled) in the last frame.
    pub visible: bool,
    /// Projected screen size from the last frame (diameter or area depending
    /// on `lodSelectionMetricType`).
    pub projected_screen_size: f64,
    /// Frame number when this node was last visited during traversal.
    pub last_visited_frame: u64,
    /// Number of load attempts that have failed.
    pub failed_attempts: u32,
    /// Frame when the last failure occurred (for retry cooldown).
    pub last_failed_frame: u64,
    /// Opaque renderer resources owned by this node.
    ///
    /// Set by [`PrepareRendererResources::prepare_in_main_thread`] when the
    /// node's content finishes loading. Passed back to
    /// [`PrepareRendererResources::free`] when the node is unloaded.
    pub renderer_resources: Option<RendererResources>,
}

impl NodeState {
    /// Create a new unloaded node state.
    pub fn new(node_id: u32) -> Self {
        Self {
            node_id,
            load_state: NodeLoadState::Unloaded,
            selected: false,
            visible: false,
            projected_screen_size: 0.0,
            last_visited_frame: 0,
            failed_attempts: 0,
            last_failed_frame: 0,
            renderer_resources: None,
        }
    }

    /// Check whether this node can be retried after a failure.
    pub fn can_retry(&self, current_frame: u64) -> bool {
        self.load_state == NodeLoadState::Failed
            && self.failed_attempts < MAX_LOAD_RETRIES
            && current_frame >= self.last_failed_frame + RETRY_COOLDOWN_FRAMES
    }
}
