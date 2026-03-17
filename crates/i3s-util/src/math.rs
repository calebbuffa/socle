//! Math constants and helpers.

pub const EPSILON1: f64 = 0.1;
pub const EPSILON6: f64 = 1e-6;
pub const EPSILON7: f64 = 1e-7;
pub const EPSILON12: f64 = 1e-12;

pub const RADIANS_PER_DEGREE: f64 = std::f64::consts::PI / 180.0;
pub const DEGREES_PER_RADIAN: f64 = 180.0 / std::f64::consts::PI;

#[inline]
pub fn to_radians(degrees: f64) -> f64 {
    degrees * RADIANS_PER_DEGREE
}

#[inline]
pub fn to_degrees(radians: f64) -> f64 {
    radians * DEGREES_PER_RADIAN
}

#[inline]
pub fn equals_epsilon(a: f64, b: f64, epsilon: f64) -> bool {
    (a - b).abs() <= epsilon
}
