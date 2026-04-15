/// A half-space clipping plane.
///
/// A point `p` is on the *visible* (unclipped) side when `normal · p + distance >= 0`.
/// Nodes whose entire bounding volume lies on the *clipped* side (`< 0`) are
/// skipped by the traversal.
///
/// Corresponds to Cesium's `ClippingPlane`.
#[derive(Clone, Debug)]
pub struct ClippingPlane {
    /// Unit-length plane normal pointing toward the visible side.
    pub normal: glam::DVec3,
    /// Signed distance from the origin to the plane (positive = plane is shifted
    /// in the `normal` direction from the origin).
    pub distance: f64,
}

/// Options governing content load scheduling, retries, and memory budget.
///
/// Accessed via `engine.options().loading`.
#[derive(Clone, Debug)]
pub struct LoadingOptions {
    /// Maximum number of children simultaneously in the `Loading` state before the
    /// traversal stops descending further.
    pub loading_descendant_limit: usize,

    /// If `true`, the selection must not produce holes — a parent node is always
    /// selected as a fallback if replacement children are not yet `Renderable`.
    pub prevent_holes: bool,

    /// If `true`, ancestors of rendered nodes are pre-loaded at `Preload` priority.
    pub preload_ancestors: bool,

    /// If `true`, siblings of rendered nodes are pre-loaded at `Preload` priority.
    pub preload_siblings: bool,

    /// Maximum new load requests to dispatch per load pass.
    pub max_simultaneous_loads: usize,

    /// Maximum load retry attempts before a node transitions to `Failed`.
    pub retry_limit: u8,

    /// Frames to wait before re-queuing a `RetryScheduled` node.
    pub retry_backoff_frames: u32,

    /// Memory ceiling in bytes for resident content. Eviction is triggered when exceeded.
    pub max_cached_bytes: usize,
}

impl Default for LoadingOptions {
    fn default() -> Self {
        Self {
            loading_descendant_limit: 20,
            prevent_holes: true,
            preload_ancestors: true,
            preload_siblings: true,
            max_simultaneous_loads: 20,
            retry_limit: 3,
            retry_backoff_frames: 8,
            max_cached_bytes: 512 * 1024 * 1024,
        }
    }
}

/// Options governing frustum, occlusion, fog, and clipping-plane culling.
///
/// Accessed via `engine.options().culling`.
#[derive(Clone, Debug)]
pub struct CullingOptions {
    /// Whether frustum culling is enabled.
    pub enable_frustum_culling: bool,

    /// Whether occlusion culling is enabled — occluded nodes are removed from the
    /// render set. Requires an [`OcclusionTester`](crate::OcclusionTester).
    pub enable_occlusion_culling: bool,

    /// Whether to delay refinement (descent into children) for occluded nodes.
    pub delay_refinement_for_occlusion: bool,

    /// If `true`, apply fog-density-based culling.
    pub enable_fog_culling: bool,

    /// Fog density lookup table: `(height_above_ellipsoid, fog_density)` pairs,
    /// sorted by ascending height. Only used when `enable_fog_culling` is `true`.
    pub fog_density_table: Vec<(f64, f64)>,

    /// Secondary screen-space error applied to culled nodes.
    pub culled_screen_space_error: f64,

    /// If `true`, apply [`culled_screen_space_error`](Self::culled_screen_space_error) to culled nodes.
    pub enforce_culled_screen_space_error: bool,

    /// If `true`, nodes directly below the camera are always included even if outside the frustum.
    pub render_nodes_under_camera: bool,

    /// Clipping planes applied to the entire spatial hierarchy.
    pub clipping_planes: Vec<ClippingPlane>,
}

impl Default for CullingOptions {
    fn default() -> Self {
        Self {
            enable_frustum_culling: true,
            enable_occlusion_culling: false,
            delay_refinement_for_occlusion: true,
            enable_fog_culling: false,
            fog_density_table: Vec::new(),
            culled_screen_space_error: 64.0,
            enforce_culled_screen_space_error: false,
            render_nodes_under_camera: false,
            clipping_planes: Vec::new(),
        }
    }
}

/// Options governing LOD refinement heuristics: skip-LOD, dynamic reduction,
/// foveation, and progressive resolution.
///
/// Accessed via `engine.options().lod`.
#[derive(Clone, Debug)]
pub struct LodRefinementOptions {
    /// If `true`, enables skip-LOD: the engine may skip intermediate LOD levels.
    pub skip_level_of_detail: bool,

    /// Factor by which parent LOD metric must exceed threshold to trigger a skip.
    pub skip_lod_metric_factor: f64,

    /// Minimum LOD metric value to be a skip candidate.
    pub base_lod_metric_threshold: f64,

    /// Minimum levels to skip between consecutively rendered nodes when skip-LOD is on.
    pub skip_levels: u32,

    /// When `true` and skip-LOD enabled, only the desired node is downloaded — no placeholder.
    pub immediately_load_desired_lod: bool,

    /// If `true`, reduces LOD detail for off-axis nodes.
    pub enable_dynamic_detail_reduction: bool,

    /// Controls how strongly distance affects off-axis detail reduction.
    pub dynamic_detail_reduction_density: f64,

    /// Scales the off-axis detail reduction effect.
    pub dynamic_detail_reduction_factor: f64,

    /// If `true`, enables foveated rendering.
    pub enable_foveated_rendering: bool,

    /// Fraction of the view cone (0..1) within which nodes receive full detail.
    pub foveated_cone_size: f64,

    /// Minimum LOD metric multiplier (0..1) applied to peripheral nodes.
    pub foveated_min_lod_metric_relaxation: f64,

    /// Seconds after the camera stops before peripheral nodes ramp back to full detail.
    pub foveated_time_delay: f32,

    /// If `true`, temporarily lowers the effective LOD threshold for progressive streaming.
    pub enable_progressive_resolution: bool,

    /// Fraction of viewport height used for the progressive-resolution pass.
    pub progressive_resolution_height_fraction: f64,
}

impl Default for LodRefinementOptions {
    fn default() -> Self {
        Self {
            skip_level_of_detail: false,
            skip_lod_metric_factor: 16.0,
            base_lod_metric_threshold: 1024.0,
            skip_levels: 1,
            immediately_load_desired_lod: false,
            enable_dynamic_detail_reduction: false,
            dynamic_detail_reduction_density: 0.00278,
            dynamic_detail_reduction_factor: 4.0,
            enable_foveated_rendering: false,
            foveated_cone_size: 0.1,
            foveated_min_lod_metric_relaxation: 0.0,
            foveated_time_delay: 0.2,
            enable_progressive_resolution: false,
            progressive_resolution_height_fraction: 0.3,
        }
    }
}

/// Options governing load prioritisation, flight preloading, and LOD transitions.
///
/// Accessed via `engine.options().streaming`.
#[derive(Clone, Debug)]
pub struct StreamingOptions {
    /// Boost load priority for nodes in the direction the camera is moving.
    pub enable_request_render_mode_priority: bool,

    /// Angle threshold (radians) for movement-direction priority boost.
    pub request_render_mode_priority_angle: f64,

    /// Declared camera flight destinations for speculative preloading.
    pub flight_destinations: Vec<Vec<crate::view::ViewState>>,

    /// If `true`, when skip-LOD skips a level, siblings at that level are preloaded.
    pub load_siblings_on_skip: bool,

    /// Cancel `Normal`-priority requests while the camera is moving fast.
    pub cull_requests_while_moving: bool,

    /// Speed multiplier threshold for `cull_requests_while_moving`.
    pub cull_requests_while_moving_multiplier: f64,

    /// If `true`, newly-visible nodes fade in over `lod_transition_length` seconds
    /// instead of popping in. Matches cesium-native `enableLodTransitionPeriod`.
    pub enable_lod_transition: bool,

    /// Duration in seconds of the fade-in/fade-out LOD transition.
    /// Matches cesium-native `lodTransitionLength` (default 1.0 s).
    pub lod_transition_length: f32,

    /// If `true` and `enable_lod_transition` is on, descendants are kicked while
    /// their parent is still fading in, preventing pop-in of children over a
    /// partially-transparent parent. Matches cesium-native `kickDescendantsWhileFadingIn`.
    pub kick_descendants_while_fading_in: bool,
}

impl Default for StreamingOptions {
    fn default() -> Self {
        Self {
            enable_request_render_mode_priority: false,
            request_render_mode_priority_angle: std::f64::consts::FRAC_PI_6,
            flight_destinations: Vec::new(),
            load_siblings_on_skip: false,
            cull_requests_while_moving: true,
            cull_requests_while_moving_multiplier: 60.0,
            enable_lod_transition: false,
            lod_transition_length: 1.0,
            kick_descendants_while_fading_in: true,
        }
    }
}

/// Debug-only engine options.
///
/// Accessed via `engine.options().debug`.
#[derive(Clone, Debug, Default)]
pub struct DebugOptions {
    /// When `true`, the engine skips traversal and returns the previous frame's result.
    pub enable_freeze_frame: bool,
}

/// Core engine options, grouped into five nested structs.
///
/// # Example
/// ```rust,ignore
/// let mut opts = engine.options().clone();
/// opts.loading.max_simultaneous_loads = 8;
/// opts.culling.enable_frustum_culling = false;
/// engine.set_options(opts);
/// ```
#[derive(Clone, Debug, Default)]
pub struct SelectionOptions {
    pub loading: LoadingOptions,
    pub culling: CullingOptions,
    pub lod: LodRefinementOptions,
    pub streaming: StreamingOptions,
    pub debug: DebugOptions,
}
