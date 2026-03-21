use crate::node::NodeId;
use zukei::Vec3;

/// Per-view camera state passed into each `update_view_group` call.
/// All positions and directions are in the engine's working coordinate system.
#[derive(Clone, Debug)]
pub struct ViewState {
    /// Viewport dimensions in physical pixels, `[width, height]`.
    pub viewport_px: [u32; 2],
    /// Camera world-space position.
    pub position: Vec3,
    /// Camera view direction (unit-length, world-space).
    pub direction: Vec3,
    /// Camera up vector (unit-length, world-space).
    pub up: Vec3,
    /// Horizontal field-of-view angle in radians.
    pub fov_x: f64,
    /// Vertical field-of-view angle in radians.
    pub fov_y: f64,
    /// Multiplier applied to the raw LOD metric before passing to `LodEvaluator`.
    /// Use values > 1.0 to over-load (sharper detail); < 1.0 to under-load.
    pub lod_metric_multiplier: f32,
}

/// Identifies a view group managed by the engine.
/// A view group is a set of related views sharing the same content stream and scheduler slot.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ViewGroupHandle {
    pub index: u32,
    pub generation: u32,
}

/// Configuration for a view group.
#[derive(Clone, Debug)]
pub struct ViewGroupOptions {
    /// Relative scheduling weight for load fair-sharing across view groups.
    /// Higher weight = more load requests per frame.
    pub weight: f64,
}

impl Default for ViewGroupOptions {
    fn default() -> Self {
        Self { weight: 1.0 }
    }
}

/// Node-selection outcome for a single view within a frame pass.
#[derive(Clone, Debug, Default)]
pub struct PerViewUpdateResult {
    /// Nodes selected for rendering by this view.
    pub selected: Vec<NodeId>,
    /// Nodes traversed during the selection walk (incl. culled and selected).
    pub visited: usize,
    /// Nodes rejected by the frustum or visibility policy.
    pub culled: usize,
}

/// Aggregated node-selection outcome for a single view group within a frame pass.
#[derive(Clone, Debug, Default)]
pub struct ViewUpdateResult {
    /// Nodes selected for rendering across all views in this group (union, deduped).
    pub selected: Vec<NodeId>,
    pub visited: usize,
    pub culled: usize,
    /// Requests newly queued for this group during this pass.
    pub queued_requests: usize,
    /// Number of nodes in the worker thread load queue.
    pub worker_thread_load_queue_length: usize,
    /// Number of nodes in the main thread load queue.
    pub main_thread_load_queue_length: usize,
    /// Monotonically increasing frame counter.
    pub frame_number: u64,
    /// Per-view breakdown.
    pub per_view: Vec<PerViewUpdateResult>,
}
