//! Shared geometric utilities for [`LodEvaluator`](crate::LodEvaluator) implementations.
//!
//! Concrete evaluators live in format-specific crates:
//! - `tiles3d-selekt` — `GeometricErrorEvaluator` (3D Tiles `geometricError`)
//! - `i3s-selekt` — `MaxScreenThresholdSQEvaluator`, `MaxScreenThresholdEvaluator`,
//!   `EffectiveDensityEvaluator`
//!
//! The geometric predicates here (`distance_to_bounds`, `ray_vs_bounds`, etc.) are
//! thin wrappers over the canonical implementations on [`zukei::SpatialBounds`].

use glam::DVec3;
use zukei::SpatialBounds;

/// Returns `true` if `point` is inside (or on the boundary of) `bounds`.
///
/// Delegates to [`SpatialBounds::contains_point`].
#[inline]
pub fn point_inside_bounds(point: DVec3, bounds: &SpatialBounds) -> bool {
    bounds.contains_point(point)
}

/// Returns `true` if `bounds` lies entirely on the clipped (negative) side of
/// the half-space `(normal, plane_distance)`.
///
/// Delegates to [`SpatialBounds::is_entirely_clipped`].
#[inline]
pub fn bounds_entirely_clipped(bounds: &SpatialBounds, normal: DVec3, plane_distance: f64) -> bool {
    bounds.is_entirely_clipped(normal, plane_distance)
}

/// Returns `true` if the horizontal projection of `camera` falls within `bounds`.
///
/// Delegates to [`SpatialBounds::is_over_footprint`].
#[inline]
pub fn camera_is_over_bounds(camera: DVec3, bounds: &SpatialBounds) -> bool {
    bounds.is_over_footprint(camera)
}

/// Test a ray against `bounds`, returning distance `t ≥ 0` to the first
/// intersection, or `None` on miss.
///
/// Delegates to [`SpatialBounds::ray_intersect`].
#[inline]
pub fn ray_vs_bounds(origin: DVec3, direction: DVec3, bounds: &SpatialBounds) -> Option<f64> {
    bounds.ray_intersect(origin, direction)
}


#[cfg(test)]
mod tests {
    use super::*;
    use glam::{DMat3, DVec3};

    fn distance_to_bounds(point: DVec3, bounds: &SpatialBounds) -> f64 {
        bounds.distance_to_point(point)
    }

    fn sphere_at_origin(radius: f64) -> SpatialBounds {
        SpatialBounds::Sphere {
            center: DVec3::ZERO,
            radius,
        }
    }

    fn aabb(half: f64) -> SpatialBounds {
        SpatialBounds::AxisAlignedBox {
            min: DVec3::splat(-half),
            max: DVec3::splat(half),
        }
    }

    fn unit_obb() -> SpatialBounds {
        SpatialBounds::OrientedBox {
            center: DVec3::ZERO,
            half_axes: DMat3::IDENTITY,
        }
    }

    #[test]
    fn sphere_distance_outside() {
        let d = distance_to_bounds(DVec3::new(0.0, 0.0, 10.0), &sphere_at_origin(3.0));
        assert!((d - 7.0).abs() < 1e-10, "dist={d}");
    }

    #[test]
    fn sphere_distance_inside() {
        let d = distance_to_bounds(DVec3::new(0.0, 0.0, 2.0), &sphere_at_origin(3.0));
        assert_eq!(d, 0.0, "inside sphere → 0");
    }

    #[test]
    fn aabb_distance_outside() {
        let d = distance_to_bounds(DVec3::new(2.0, 0.0, 0.0), &aabb(1.0));
        assert!((d - 1.0).abs() < 1e-10, "dist={d}");
    }

    #[test]
    fn aabb_distance_inside() {
        let d = distance_to_bounds(DVec3::ZERO, &aabb(1.0));
        assert_eq!(d, 0.0);
    }

    #[test]
    fn aabb_distance_corner() {
        // Point at (2, 2, 2) from unit cube [-1,1]: excess = (1,1,1)
        let d = distance_to_bounds(DVec3::new(2.0, 2.0, 2.0), &aabb(1.0));
        assert!((d - 3.0_f64.sqrt()).abs() < 1e-10, "dist={d}");
    }

    #[test]
    fn obb_distance_identity_like_aabb() {
        // With identity half_axes the OBB is [-1,1]^3, same as the unit aabb.
        let d_obb = distance_to_bounds(DVec3::new(2.0, 0.0, 0.0), &unit_obb());
        let d_aab = distance_to_bounds(DVec3::new(2.0, 0.0, 0.0), &aabb(1.0));
        assert!((d_obb - d_aab).abs() < 1e-10, "obb={d_obb} aabb={d_aab}");
    }
}
