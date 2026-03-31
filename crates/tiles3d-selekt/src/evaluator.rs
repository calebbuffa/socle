//! LOD evaluator for 3D Tiles `geometricError`.
//!
//! # `GeometricErrorEvaluator`
//!
//! Implements the Cesium-compatible screen-space error (SSE) criterion used
//! by 3D Tiles (`geometricError`).
//!
//! The tile should refine when:
//! ```text
//! SSE = (geometric_error × viewport_height) / (camera_distance × SSE_denominator) > threshold
//! ```
//! where `SSE_denominator = 2 × tan(fov_y / 2)` for perspective projection,
//! and `SSE = geometric_error / pixel_world_size` for orthographic.
//!
//! ## Family
//!
//! Descriptors with `family != GEOMETRIC_ERROR_FAMILY` are treated as *never refine*
//! (the evaluator returns `false` for unknown families).

use selekt::{LodDescriptor, LodEvaluator, LodFamily, Projection, RefinementMode, ViewState};
use zukei::SpatialBounds;

fn _geometric_error_family_token() {}

/// The [`LodFamily`] token recognised by [`GeometricErrorEvaluator`].
///
/// Use this when constructing a [`LodDescriptor`] for a 3D Tiles node.
pub const GEOMETRIC_ERROR_FAMILY: LodFamily =
    LodFamily::from_token(_geometric_error_family_token);

/// Minimum camera-to-tile distance used in the SSE formula to avoid
/// divide-by-zero when the camera is inside or touching a tile's bounding volume.
pub const MINIMUM_CAMERA_DISTANCE: f64 = 1.0;

/// Screen-space-error LOD evaluator for the `"geometric_error"` metric family.
///
/// Compatible with Cesium 3D Tiles geometric error: refines when the
/// projected screen-space error exceeds `maximum_screen_space_error` pixels.
///
/// *Default maximum SSE*: `16.0` pixels (matches Cesium's default).
#[derive(Debug, Clone)]
pub struct GeometricErrorEvaluator {
    /// SSE threshold in pixels. Refine when computed SSE exceeds this value.
    pub maximum_screen_space_error: f64,
}

impl Default for GeometricErrorEvaluator {
    fn default() -> Self {
        Self {
            maximum_screen_space_error: 16.0,
        }
    }
}

impl GeometricErrorEvaluator {
    /// Create with a custom SSE threshold.
    pub fn new(maximum_screen_space_error: f64) -> Self {
        Self {
            maximum_screen_space_error,
        }
    }

    /// Compute the screen-space error for the given descriptor, view, and bounds.
    ///
    /// Useful for debugging or custom refinement logic.
    pub fn compute_sse(
        &self,
        geometric_error: f64,
        view: &ViewState,
        bounds: &SpatialBounds,
    ) -> f64 {
        let distance = bounds.distance_to_point(view.position);
        let multiplier = view.lod_metric_multiplier as f64;
        match &view.projection {
            Projection::Perspective { fov_y, .. } => {
                let sse_denominator = 2.0 * (fov_y * 0.5).tan();
                let distance = distance.max(MINIMUM_CAMERA_DISTANCE);
                let viewport_height = view.viewport_px[1] as f64;
                (geometric_error * viewport_height * multiplier) / (distance * sse_denominator)
            }
            Projection::Orthographic { half_height, .. } => {
                // pixel_world_size = 2*half_height / viewport_height
                let viewport_height = view.viewport_px[1] as f64;
                let pixel_world_size = 2.0 * half_height / viewport_height;
                if pixel_world_size <= 0.0 {
                    return 0.0;
                }
                (geometric_error / pixel_world_size) * multiplier
            }
        }
    }
}

impl LodEvaluator for GeometricErrorEvaluator {
    fn should_refine(
        &self,
        descriptor: &LodDescriptor,
        view: &ViewState,
        _multiplier: f32,
        bounds: &SpatialBounds,
        _mode: RefinementMode,
    ) -> bool {
        if descriptor.family != GEOMETRIC_ERROR_FAMILY {
            return false;
        }
        let sse = self.compute_sse(descriptor.value, view, bounds);
        sse > self.maximum_screen_space_error
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;
    use std::f64::consts::PI;

    fn perspective_view(distance_to_origin: f64, viewport_height: u32, fov_y: f64) -> ViewState {
        ViewState::perspective(
            DVec3::new(0.0, 0.0, distance_to_origin),
            DVec3::new(0.0, 0.0, -1.0),
            DVec3::Y,
            [viewport_height, viewport_height],
            fov_y,
            fov_y,
        )
    }

    fn ortho_view(half_height: f64, viewport_height: u32) -> ViewState {
        ViewState::orthographic(
            DVec3::new(0.0, 0.0, 1000.0),
            DVec3::new(0.0, 0.0, -1.0),
            DVec3::Y,
            [viewport_height, viewport_height],
            half_height,
            half_height,
        )
    }

    fn sphere_at_origin(radius: f64) -> SpatialBounds {
        SpatialBounds::Sphere {
            center: DVec3::ZERO,
            radius,
        }
    }

    #[test]
    fn sse_zero_error_never_refines() {
        let eval = GeometricErrorEvaluator::default();
        let view = perspective_view(100.0, 512, PI / 3.0);
        let bounds = sphere_at_origin(10.0);
        let desc = LodDescriptor {
            family: GEOMETRIC_ERROR_FAMILY,
            value: 0.0,
        };
        assert!(!eval.should_refine(&desc, &view, 1.0_f32, &bounds, RefinementMode::Replace));
    }

    #[test]
    fn unknown_family_never_refines() {
        let eval = GeometricErrorEvaluator::default();
        let view = perspective_view(10.0, 512, PI / 3.0);
        let bounds = sphere_at_origin(1.0);
        let desc = LodDescriptor {
            family: LodFamily::NONE,
            value: 1_000.0,
        };
        assert!(!eval.should_refine(&desc, &view, 1.0_f32, &bounds, RefinementMode::Add));
    }

    #[test]
    fn perspective_close_camera_refines() {
        let eval = GeometricErrorEvaluator::default();
        let view = perspective_view(10.0, 512, PI / 3.0);
        let bounds = sphere_at_origin(1.0);
        let desc = LodDescriptor {
            family: GEOMETRIC_ERROR_FAMILY,
            value: 100.0,
        };
        assert!(eval.should_refine(&desc, &view, 1.0_f32, &bounds, RefinementMode::Replace));
    }

    #[test]
    fn perspective_far_camera_does_not_refine() {
        let eval = GeometricErrorEvaluator::default();
        let view = perspective_view(1_000_000.0, 512, PI / 3.0);
        let bounds = sphere_at_origin(1.0);
        let desc = LodDescriptor {
            family: GEOMETRIC_ERROR_FAMILY,
            value: 1.0,
        };
        assert!(!eval.should_refine(&desc, &view, 1.0_f32, &bounds, RefinementMode::Replace));
    }

    #[test]
    fn orthographic_refines_when_error_large() {
        // pixel_world_size = 2*1000 / 512 ≈ 3.9 m/px; geometric_error = 100 m → SSE ≈ 25.6 px > 16
        let eval = GeometricErrorEvaluator::default();
        let view = ortho_view(1000.0, 512);
        let bounds = sphere_at_origin(1.0);
        let desc = LodDescriptor {
            family: GEOMETRIC_ERROR_FAMILY,
            value: 100.0,
        };
        assert!(eval.should_refine(&desc, &view, 1.0_f32, &bounds, RefinementMode::Add));
    }

    #[test]
    fn orthographic_does_not_refine_when_error_small() {
        // pixel_world_size = 2*1000 / 512 ≈ 3.9 m/px; geometric_error = 1 m → SSE ≈ 0.26 px < 16
        let eval = GeometricErrorEvaluator::default();
        let view = ortho_view(1000.0, 512);
        let bounds = sphere_at_origin(1.0);
        let desc = LodDescriptor {
            family: GEOMETRIC_ERROR_FAMILY,
            value: 1.0,
        };
        assert!(!eval.should_refine(&desc, &view, 1.0_f32, &bounds, RefinementMode::Add));
    }

    #[test]
    fn lod_metric_multiplier_scales_sse() {
        let eval = GeometricErrorEvaluator::default();
        let mut view = perspective_view(1_000.0, 512, PI / 3.0);
        let bounds = sphere_at_origin(1.0);
        let desc = LodDescriptor {
            family: GEOMETRIC_ERROR_FAMILY,
            value: 1.0,
        };
        view.lod_metric_multiplier = 1.0;
        let base_sse = eval.compute_sse(desc.value, &view, &bounds);
        assert!(base_sse < eval.maximum_screen_space_error);
        view.lod_metric_multiplier = 10_000.0;
        assert!(eval.should_refine(&desc, &view, 1.0_f32, &bounds, RefinementMode::Replace));
    }

    #[test]
    fn default_threshold_is_16() {
        assert_eq!(
            GeometricErrorEvaluator::default().maximum_screen_space_error,
            16.0
        );
    }
}
