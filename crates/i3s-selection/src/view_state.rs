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

use glam::DVec3;

use i3s_geometry::frustum::CullingVolume;

/// Camera and viewport state used for I3S LOD selection.
///
/// Modeled after cesium-native's `ViewState`. Carries camera parameters for
/// both frustum culling and screen-space LOD projection. The culling volume
/// is derived from these parameters — no separate frustum input is needed.
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
    /// Vertical field of view in radians.
    pub fov_y: f64,
}

impl ViewState {
    /// Build a culling volume (4 side frustum planes) from this view state.
    ///
    /// No near/far planes — LOD selection doesn't need them.
    pub fn culling_volume(&self) -> CullingVolume {
        let aspect = self.viewport_width as f64 / self.viewport_height.max(1) as f64;
        CullingVolume::from_camera(self.position, self.direction, self.up, self.fov_y, aspect)
    }

    /// Compute the screen projection factor.
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
        let distance = self.position.distance(center);
        if distance < 1e-10 {
            return f64::MAX;
        }
        (radius / distance) * self.screen_size_factor()
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
        ViewState {
            position: DVec3::ZERO,
            direction: DVec3::NEG_Z,
            up: DVec3::Y,
            viewport_width: 1920,
            viewport_height: 1080,
            fov_y: std::f64::consts::FRAC_PI_3, // 60 degrees
        }
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
}
