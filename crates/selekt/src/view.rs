use crate::node::NodeId;
use zukei::Vec3;

/// Projection model for a view.
#[derive(Clone, Debug)]
pub enum Projection {
    /// Symmetric perspective projection.
    Perspective {
        /// Horizontal field-of-view angle in radians.
        fov_x: f64,
        /// Vertical field-of-view angle in radians.
        fov_y: f64,
    },
    /// Orthographic projection.
    Orthographic {
        /// Half-width of the view volume in world units.
        half_width: f64,
        /// Half-height of the view volume in world units.
        half_height: f64,
    },
}

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
    /// Projection model (perspective or orthographic).
    pub projection: Projection,
    /// Multiplier applied to the raw LOD metric before passing to `LodEvaluator`.
    /// Use values > 1.0 to over-load (sharper detail); < 1.0 to under-load.
    pub lod_metric_multiplier: f32,
}

impl ViewState {
    /// Create a perspective view state.
    pub fn perspective(
        position: Vec3,
        direction: Vec3,
        up: Vec3,
        viewport_px: [u32; 2],
        fov_x: f64,
        fov_y: f64,
    ) -> Self {
        Self {
            viewport_px,
            position,
            direction,
            up,
            projection: Projection::Perspective { fov_x, fov_y },
            lod_metric_multiplier: 1.0,
        }
    }

    /// Create an orthographic view state.
    pub fn orthographic(
        position: Vec3,
        direction: Vec3,
        up: Vec3,
        viewport_px: [u32; 2],
        half_width: f64,
        half_height: f64,
    ) -> Self {
        Self {
            viewport_px,
            position,
            direction,
            up,
            projection: Projection::Orthographic {
                half_width,
                half_height,
            },
            lod_metric_multiplier: 1.0,
        }
    }

    /// Horizontal field-of-view in radians, or `None` for orthographic views.
    pub fn fov_x(&self) -> Option<f64> {
        match &self.projection {
            Projection::Perspective { fov_x, .. } => Some(*fov_x),
            Projection::Orthographic { .. } => None,
        }
    }

    /// Vertical field-of-view in radians, or `None` for orthographic views.
    pub fn fov_y(&self) -> Option<f64> {
        match &self.projection {
            Projection::Perspective { fov_y, .. } => Some(*fov_y),
            Projection::Orthographic { .. } => None,
        }
    }
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
