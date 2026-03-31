mod aabb;
mod bounds;
mod culling;
mod frustum;
mod intersection;
mod obb;
mod plane;
mod polygon;
mod ray;
mod rectangle;
mod sphere;
mod transforms;

pub use aabb::AxisAlignedBoundingBox;
pub use bounds::SpatialBounds;
pub use culling::CullingResult;
pub use frustum::CullingVolume;
pub use intersection::{
    obb_distance_half_axes, point_in_triangle_2d, point_in_triangle_3d, ray_aabb, ray_ellipsoid,
    ray_obb, ray_plane, ray_sphere, ray_triangle,
};
pub use polygon::{
    cross2, point_in_polygon_2d, point_to_segment_dist_2d, polygon_boundary_distance_2d,
};
pub use obb::OrientedBoundingBox;
pub use plane::Plane;
pub use ray::Ray;
pub use rectangle::Rectangle;
pub use sphere::BoundingSphere;
pub use transforms::{
    Axis, Transforms, X_UP_TO_Y_UP, X_UP_TO_Z_UP, Y_UP_TO_X_UP, Y_UP_TO_Z_UP, Z_UP_TO_X_UP,
    Z_UP_TO_Y_UP,
};

pub const EPSILON1: f64 = 1e-1;
pub const EPSILON2: f64 = 1e-2;
pub const EPSILON3: f64 = 1e-3;
pub const EPSILON4: f64 = 1e-4;
pub const EPSILON5: f64 = 1e-5;
pub const EPSILON6: f64 = 1e-6;
pub const EPSILON7: f64 = 1e-7;
pub const EPSILON8: f64 = 1e-8;
pub const EPSILON9: f64 = 1e-9;
pub const EPSILON10: f64 = 1e-10;
pub const EPSILON11: f64 = 1e-11;
pub const EPSILON12: f64 = 1e-12;
pub const EPSILON13: f64 = 1e-13;
pub const EPSILON14: f64 = 1e-14;
pub const EPSILON15: f64 = 1e-15;
pub const EPSILON16: f64 = 1e-16;
pub const EPSILON17: f64 = 1e-17;
pub const EPSILON18: f64 = 1e-18;
pub const EPSILON19: f64 = 1e-19;
pub const EPSILON20: f64 = 1e-20;
pub const EPSILON21: f64 = 1e-21;

pub const ONE_PI: f64 = std::f64::consts::PI;
pub const TWO_PI: f64 = ONE_PI * 2.0;
pub const PI_OVER_TWO: f64 = ONE_PI / 2.0;
pub const PI_OVER_FOUR: f64 = ONE_PI / 4.0;

pub const RADIANS_PER_DEGREE: f64 = ONE_PI / 180.0;
pub const DEGREES_PER_RADIAN: f64 = 180.0 / ONE_PI;

/// Converts degrees to radians.
#[inline]
pub fn to_radians(degrees: f64) -> f64 {
    degrees * RADIANS_PER_DEGREE
}

/// Converts radians to degrees.
#[inline]
pub fn to_degrees(radians: f64) -> f64 {
    radians * DEGREES_PER_RADIAN
}

/// Returns `true` if `|a - b| <= max(relativeEpsilon * max(|a|, |b|), absoluteEpsilon)`.
#[inline]
pub fn equals_epsilon(a: f64, b: f64, relative_epsilon: f64) -> bool {
    equals_epsilon_abs(a, b, relative_epsilon, relative_epsilon)
}

/// Returns `true` if `|a - b| <= absoluteEpsilon` or the relative tolerance holds.
#[inline]
pub fn equals_epsilon_abs(a: f64, b: f64, relative_epsilon: f64, absolute_epsilon: f64) -> bool {
    let diff = (a - b).abs();
    diff <= absolute_epsilon || diff <= relative_epsilon * a.abs().max(b.abs())
}

/// Returns 1.0 if `value >= 0.0`, otherwise -1.0.
///
/// Unlike `f64::signum`, this never returns 0.0.
#[inline]
pub fn sign_not_zero(value: f64) -> f64 {
    if value < 0.0 { -1.0 } else { 1.0 }
}

/// Converts a SNORM value in `[0, range_max]` to a scalar in `[-1.0, 1.0]`.
///
/// Maps 0 → -1.0 and `range_max` → 1.0.
#[inline]
pub fn from_snorm(value: f64, range_max: f64) -> f64 {
    (value.clamp(0.0, range_max) / range_max) * 2.0 - 1.0
}

/// Converts a scalar in `[-1.0, 1.0]` to a SNORM in `[0, range_max]`.
#[inline]
pub fn to_snorm(value: f64, range_max: f64) -> f64 {
    ((value.clamp(-1.0, 1.0) * 0.5 + 0.5) * range_max).round()
}

/// Produces an angle in `[-π, π]` equivalent to the given angle (radians).
pub fn negative_pi_to_pi(angle: f64) -> f64 {
    if angle >= -ONE_PI && angle <= ONE_PI {
        return angle;
    }
    zero_to_two_pi(angle + ONE_PI) - ONE_PI
}

/// Produces an angle in `[0, 2π]` equivalent to the given angle (radians).
pub fn zero_to_two_pi(angle: f64) -> f64 {
    if angle >= 0.0 && angle <= TWO_PI {
        return angle;
    }
    let m = mod_val(angle, TWO_PI);
    if m.abs() < EPSILON14 && angle.abs() > EPSILON14 {
        return TWO_PI;
    }
    m
}

/// Modulo that also works for negative dividends (always returns a non-negative value).
#[inline]
pub fn mod_val(m: f64, n: f64) -> f64 {
    ((m % n) + n) % n
}
