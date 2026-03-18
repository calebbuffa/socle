//! Camera and viewport state for LOD evaluation.
//!
//! ## Coordinate system contract
//!
//! `ViewState` coordinates must be in the **same frame** as the converted OBBs:
//!
//! | Layer CRS | CrsTransform | OBB frame after [`crs::obb_from_spec`] | ViewState frame |
//! |---|---|---|---|
//! | Global (WKID 4326 / 4490) | — | ECEF (meters) | **ECEF** (meters) |
//! | Local / projected | `None` | Layer CRS (CRS units) | **Layer CRS** (CRS units) |
//! | Local / projected | `Some(xform)` | ECEF (meters) | **ECEF** (meters) |
//!
//! For global layers, convert your camera's lat/lon/altitude to ECEF
//! (via [`Ellipsoid::cartographic_to_cartesian`](i3s_geospatial::ellipsoid::Ellipsoid::cartographic_to_cartesian))
//! before constructing a `ViewState`.
//!
//! For local layers **without** a [`CrsTransform`](crate::crs::CrsTransform),
//! use the camera position directly in the layer's CRS.
//!
//! For local layers **with** a `CrsTransform`, the OBBs are converted to ECEF
//! by the transform, so provide the camera in **ECEF** as well.

use glam::{DMat4, DVec3};

use i3s_geometry::frustum::CullingVolume;
use i3s_geometry::plane::Plane;

/// Camera and viewport state used for I3S LOD selection.
///
/// Carries camera parameters for both frustum culling and screen-space LOD
/// projection. The culling volume is derived from these parameters — no
/// separate frustum input is needed.
///
/// Construct with [`ViewState::new`] (perspective), [`ViewState::new_orthographic`],
/// or [`ViewState::from_view_proj`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewState {
    /// Camera position in the scene layer's coordinate system (typically ECEF).
    pub position: DVec3,
    /// View direction (normalized).
    pub direction: DVec3,
    /// Up vector (normalized).
    pub up: DVec3,
    /// Viewport width in pixels.
    pub viewport_width: u32,
    /// Viewport height in pixels.
    pub viewport_height: u32,
    /// Vertical field of view in radians (perspective only; 0 for orthographic).
    pub fov_y: f64,
    /// Orthographic view width in world units. `None` for perspective cameras.
    ortho_width: Option<f64>,
}

impl ViewState {
    /// Create a perspective view state.
    pub fn new(
        position: DVec3,
        direction: DVec3,
        up: DVec3,
        viewport_width: u32,
        viewport_height: u32,
        fov_y: f64,
    ) -> Self {
        Self {
            position,
            direction,
            up,
            viewport_width,
            viewport_height,
            fov_y,
            ortho_width: None,
        }
    }

    /// Create an orthographic view state.
    ///
    /// `ortho_width` is the total width of the visible region in world units.
    /// The height is derived from the viewport aspect ratio.
    pub fn new_orthographic(
        position: DVec3,
        direction: DVec3,
        up: DVec3,
        viewport_width: u32,
        viewport_height: u32,
        ortho_width: f64,
    ) -> Self {
        Self {
            position,
            direction,
            up,
            viewport_width,
            viewport_height,
            fov_y: 0.0,
            ortho_width: Some(ortho_width),
        }
    }

    /// Decompose a view + projection matrix pair into a `ViewState`.
    ///
    /// Detects perspective vs orthographic from the projection matrix:
    /// `proj[2][3] ≈ -1` → perspective; `proj[2][3] ≈ 0` → orthographic.
    pub fn from_view_proj(
        view: DMat4,
        proj: DMat4,
        viewport_width: u32,
        viewport_height: u32,
    ) -> Self {
        let inv = view.inverse();
        let position = inv.col(3).truncate();
        // Camera local -Z = viewing direction; col(2) of view inverse = that axis
        let direction = -inv.col(2).truncate();
        let up = inv.col(1).truncate();

        // proj[2][3] == -1 for perspective (OpenGL/Vulkan), 0 for orthographic
        let is_ortho = proj.col(2).w.abs() < 1e-8;

        if is_ortho {
            // Symmetric ortho: width = 2 / proj[0][0]
            let ortho_width = if proj.col(0).x.abs() > 1e-30 {
                2.0 / proj.col(0).x
            } else {
                1.0
            };
            Self::new_orthographic(
                position,
                direction,
                up,
                viewport_width,
                viewport_height,
                ortho_width,
            )
        } else {
            // Symmetric perspective: fov_y from proj[1][1] = 1/tan(fov_y/2)
            let fov_y = if proj.col(1).y.abs() > 1e-30 {
                2.0 * (1.0 / proj.col(1).y).atan()
            } else {
                std::f64::consts::FRAC_PI_3
            };
            Self::new(
                position,
                direction,
                up,
                viewport_width,
                viewport_height,
                fov_y,
            )
        }
    }

    /// Whether this is an orthographic projection.
    #[inline]
    pub fn is_orthographic(&self) -> bool {
        self.ortho_width.is_some()
    }

    /// Orthographic view width in world units, or `None` for perspective.
    #[inline]
    pub fn ortho_width(&self) -> Option<f64> {
        self.ortho_width
    }

    /// Horizontal field of view in radians (perspective only).
    ///
    /// Derived from `fov_y` and the viewport aspect ratio.
    /// Returns 0 for orthographic cameras.
    #[inline]
    pub fn horizontal_fov(&self) -> f64 {
        if self.ortho_width.is_some() {
            return 0.0;
        }
        let aspect = self.viewport_width as f64 / self.viewport_height.max(1) as f64;
        2.0 * ((self.fov_y * 0.5).tan() * aspect).atan()
    }

    /// Build a culling volume (4 side frustum planes) from this view state.
    ///
    /// No near/far planes — LOD selection doesn't need them.
    pub fn culling_volume(&self) -> CullingVolume {
        let aspect = self.viewport_width as f64 / self.viewport_height.max(1) as f64;

        if let Some(ortho_width) = self.ortho_width {
            // Orthographic: 4 axis-aligned side planes
            let ortho_height = ortho_width / aspect;
            let half_w = ortho_width * 0.5;
            let half_h = ortho_height * 0.5;
            let right = self.direction.cross(self.up).normalize();
            let cam_up = right.cross(self.direction).normalize();

            CullingVolume::from_planes(vec![
                Plane::from_point_normal(self.position - right * half_w, right),
                Plane::from_point_normal(self.position + right * half_w, -right),
                Plane::from_point_normal(self.position - cam_up * half_h, cam_up),
                Plane::from_point_normal(self.position + cam_up * half_h, -cam_up),
            ])
        } else {
            CullingVolume::from_camera(self.position, self.direction, self.up, self.fov_y, aspect)
        }
    }

    /// Compute the screen projection factor (perspective only).
    ///
    /// When multiplied by `(radius / distance)`, this gives the projected
    /// pixel diameter of a bounding sphere. Used to compare against
    /// `maxScreenThreshold` / `lodThreshold`.
    #[inline]
    pub fn screen_size_factor(&self) -> f64 {
        self.viewport_height as f64 / (self.fov_y * 0.5).tan()
    }

    /// Compute the projected screen diameter (in pixels) of a bounding sphere.
    ///
    /// This is the I3S `maxScreenThreshold` metric: the pixel diameter of
    /// the node's bounding volume as seen from this camera.
    #[inline]
    pub fn projected_screen_diameter(&self, center: DVec3, radius: f64) -> f64 {
        if let Some(ortho_width) = self.ortho_width {
            // Orthographic: size is distance-independent
            radius * 2.0 * self.viewport_width as f64 / ortho_width
        } else {
            let distance = self.position.distance(center);
            if distance < 1e-10 {
                return f64::MAX;
            }
            (radius / distance) * self.screen_size_factor()
        }
    }

    /// Compute the projected screen area (in pixels²) of a bounding sphere.
    ///
    /// This is the look-angle-independent form of `maxScreenThresholdSQ`.
    /// Relation: `area = π/4 × diameter²`.
    #[inline]
    pub fn projected_screen_area(&self, center: DVec3, radius: f64) -> f64 {
        let diameter = self.projected_screen_diameter(center, radius);
        std::f64::consts::PI * 0.25 * diameter * diameter
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_view() -> ViewState {
        ViewState::new(
            DVec3::ZERO,
            DVec3::NEG_Z,
            DVec3::Y,
            1920,
            1080,
            std::f64::consts::FRAC_PI_3, // 60 degrees
        )
    }

    #[test]
    fn screen_size_factor_computation() {
        let vs = test_view();
        let factor = vs.screen_size_factor();
        // tan(30 degrees) ≈ 0.5774, factor ≈ 1080 / 0.5774 ≈ 1870
        assert!(factor > 1800.0 && factor < 1950.0);
    }

    #[test]
    fn projected_diameter_decreases_with_distance() {
        let vs = test_view();
        let near = vs.projected_screen_diameter(DVec3::new(0.0, 0.0, -10.0), 5.0);
        let far = vs.projected_screen_diameter(DVec3::new(0.0, 0.0, -100.0), 5.0);
        assert!(near > far);
    }

    #[test]
    fn projected_area_is_pi_over_4_times_diameter_sq() {
        let vs = test_view();
        let center = DVec3::new(0.0, 0.0, -50.0);
        let d = vs.projected_screen_diameter(center, 5.0);
        let a = vs.projected_screen_area(center, 5.0);
        assert!((a - std::f64::consts::PI * 0.25 * d * d).abs() < 1e-6);
    }

    #[test]
    fn horizontal_fov_wider_than_vertical_for_landscape() {
        let vs = test_view();
        assert!(vs.horizontal_fov() > vs.fov_y);
    }

    #[test]
    fn orthographic_diameter_is_distance_independent() {
        let ortho =
            ViewState::new_orthographic(DVec3::ZERO, DVec3::NEG_Z, DVec3::Y, 1920, 1080, 100.0);
        let near = ortho.projected_screen_diameter(DVec3::new(0.0, 0.0, -10.0), 5.0);
        let far = ortho.projected_screen_diameter(DVec3::new(0.0, 0.0, -100.0), 5.0);
        assert!((near - far).abs() < 1e-10);
    }

    #[test]
    fn from_view_proj_roundtrip_perspective() {
        use i3s_geometry::transforms::Transforms;
        let pos = DVec3::new(0.0, 0.0, 10.0);
        let dir = DVec3::NEG_Z;
        let up = DVec3::Y;
        let fov_y = std::f64::consts::FRAC_PI_3;
        let aspect = 1920.0_f64 / 1080.0;
        let view = Transforms::create_view_matrix(pos, dir, up);
        let proj = DMat4::perspective_rh(fov_y, aspect, 0.1, 1000.0);
        let vs = ViewState::from_view_proj(view, proj, 1920, 1080);
        assert!((vs.position - pos).length() < 1e-8);
        assert!((vs.fov_y - fov_y).abs() < 1e-6);
        assert!(!vs.is_orthographic());
    }
}
