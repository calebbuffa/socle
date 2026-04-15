use glam::DVec3 as Vec3;

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
    /// Reference ellipsoid for geodetic computations (ECEF → cartographic).
    ///
    /// When set, enables `render_nodes_under_camera` to convert the ECEF
    /// position to cartographic and test geographic containment against tile
    /// bounding regions.
    pub ellipsoid: Option<terra::Ellipsoid>,
}

impl ViewState {
    /// Create a perspective view state.
    ///
    /// If `ellipsoid` is provided, `position_cartographic` is computed from
    /// the ECEF `position`. Pass `Some(&Ellipsoid::wgs84())` for geospatial data.
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
            ellipsoid: None,
        }
    }

    /// Set the reference ellipsoid for geodetic computations.
    pub fn with_ellipsoid(mut self, ellipsoid: terra::Ellipsoid) -> Self {
        self.ellipsoid = Some(ellipsoid);
        self
    }

    /// Convert the ECEF position to cartographic using the stored ellipsoid.
    pub fn position_cartographic(&self) -> Option<terra::Cartographic> {
        self.ellipsoid.as_ref()?.ecef_to_cartographic(self.position)
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
            ellipsoid: None,
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

    /// Construct a `ViewState` from a column-major view matrix and a projection matrix.
    ///
    /// Extracts camera world position, direction, and up vector from the inverse of
    /// `view_matrix`, and derives the projection from the projection matrix coefficients.
    ///
    /// Assumes a standard OpenGL/Vulkan column-major convention:
    /// - `view_matrix` transforms world → camera space
    /// - `proj_matrix` is a perspective or orthographic projection matrix
    ///
    /// `viewport_px` is `[width, height]` in physical pixels.
    pub fn from_matrices(
        view_matrix: glam::DMat4,
        proj_matrix: glam::DMat4,
        viewport_px: [u32; 2],
    ) -> Self {
        // Extract camera-to-world transformation (inverse of view matrix).
        debug_assert!(
            view_matrix.determinant().abs() > 1e-10,
            "view_matrix is singular or near-singular; inverse is undefined"
        );
        let cam_to_world = view_matrix.inverse();
        let position = cam_to_world.col(3).truncate();
        // Camera looks down −Z in camera space; transform to world space.
        let direction = -(cam_to_world.col(2).truncate()).normalize();
        let up = cam_to_world.col(1).truncate().normalize();

        // Detect perspective vs orthographic from the [3][3] element.
        // Perspective: proj[3][3] == 0; Orthographic: proj[3][3] == 1.
        let projection = if proj_matrix.col(3).w.abs() < 0.5 {
            // Perspective: fov_y from proj[1][1] = 1/tan(fov_y/2)
            let fov_y = 2.0 * (1.0 / proj_matrix.col(1).y).atan();
            let aspect = proj_matrix.col(1).y / proj_matrix.col(0).x;
            let fov_x = 2.0 * (aspect / proj_matrix.col(1).y).atan();
            Projection::Perspective { fov_x, fov_y }
        } else {
            // Orthographic: half extents from proj[0][0] and proj[1][1].
            // proj[0][0] = 2 / (right - left) ≈ 2 / (2 * half_width)
            let half_width = 1.0 / proj_matrix.col(0).x;
            let half_height = 1.0 / proj_matrix.col(1).y;
            Projection::Orthographic {
                half_width,
                half_height,
            }
        };

        Self {
            viewport_px,
            position,
            direction,
            up,
            projection,
            lod_metric_multiplier: 1.0,
            ellipsoid: None,
        }
    }
}

/// Identifies a view group managed by the engine.
/// A view group is a set of related views sharing the same content stream and Runtime slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ViewGroupHandle {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

/// Aggregate statistics from a single `update_view_group` call.
///
/// All scalar fields — `Copy`, no heap allocation. Access the actual
/// selection lists via [`SelectionEngine::selected_nodes`] and
/// [`SelectionEngine::per_view_selected`].
#[derive(Clone, Copy, Debug, Default)]
pub struct ViewUpdateResult {
    pub nodes_visited: usize,
    pub nodes_culled: usize,
    /// Nodes rejected by the occlusion tester this pass.
    pub nodes_occluded: usize,
    /// Nodes whose refinement was blocked by the loading descendant limit.
    pub nodes_kicked: usize,
    /// Requests newly queued for this group during this pass.
    pub queued_requests: usize,
    /// Number of nodes in the worker thread load queue.
    pub worker_thread_load_queue_length: usize,
    /// Monotonically increasing frame counter.
    pub frame_number: u64,
    /// Count of nodes that were in the render set last frame but are not this
    /// frame. The full node list is on `FrameResult::nodes_fading_out`.
    pub nodes_fading_out: usize,
    /// Nodes that transitioned to `Renderable` during this call (content finished loading).
    pub nodes_newly_renderable: usize,
}
