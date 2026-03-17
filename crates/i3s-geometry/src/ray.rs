//! Ray primitive for intersection tests.

use glam::{DMat4, DVec3};

/// A ray with an origin and a direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    /// Origin point of the ray.
    pub origin: DVec3,
    /// Direction of the ray (normalized).
    pub direction: DVec3,
}

impl Ray {
    /// Create a new ray. The direction will be normalized.
    pub fn new(origin: DVec3, direction: DVec3) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
        }
    }

    /// Get a point along the ray at parameter `t`.
    #[inline]
    pub fn at(&self, t: f64) -> DVec3 {
        self.origin + self.direction * t
    }

    /// Transform the ray by a 4x4 matrix.
    ///
    /// The origin is transformed as a point, the direction as a vector
    /// (and re-normalized). Mirrors cesium-native `Ray::transform`.
    pub fn transform(&self, transformation: &DMat4) -> Ray {
        let origin = transformation.transform_point3(self.origin);
        let direction = transformation.transform_vector3(self.direction).normalize();
        Ray { origin, direction }
    }

    /// Return a ray with the negated direction.
    ///
    /// Mirrors cesium-native `Ray::operator-()`.
    #[inline]
    pub fn negate(&self) -> Ray {
        Ray {
            origin: self.origin,
            direction: -self.direction,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_at() {
        let r = Ray::new(DVec3::ZERO, DVec3::X);
        let p = r.at(5.0);
        assert!((p - DVec3::new(5.0, 0.0, 0.0)).length() < 1e-12);
    }

    #[test]
    fn ray_normalizes_direction() {
        let r = Ray::new(DVec3::ZERO, DVec3::new(3.0, 0.0, 0.0));
        assert!((r.direction.length() - 1.0).abs() < 1e-12);
    }
}
