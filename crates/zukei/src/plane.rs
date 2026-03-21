//! Hessian normal-form plane.

use glam_dep::DVec3;

/// A plane in 3D space represented in Hessian normal form.
///
/// The plane equation is `dot(normal, p) + distance = 0`, where `normal` is
/// a unit vector and `distance` is the signed distance from the origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    /// Outward-facing unit normal.
    pub normal: DVec3,
    /// Signed distance from the origin along the normal.
    pub distance: f64,
}

impl Plane {
    /// The XY plane through the origin (normal = +Z).
    pub const ORIGIN_XY_PLANE: Self = Self {
        normal: DVec3::Z,
        distance: 0.0,
    };

    /// The YZ plane through the origin (normal = +X).
    pub const ORIGIN_YZ_PLANE: Self = Self {
        normal: DVec3::X,
        distance: 0.0,
    };

    /// The ZX plane through the origin (normal = +Y).
    pub const ORIGIN_ZX_PLANE: Self = Self {
        normal: DVec3::Y,
        distance: 0.0,
    };

    pub fn new(normal: DVec3, distance: f64) -> Self {
        let n = normal.normalize();
        Self {
            normal: n,
            distance,
        }
    }

    /// Create a plane from a normal (will be normalized) and a point on the plane.
    pub fn from_point_normal(point: DVec3, normal: DVec3) -> Self {
        let n = normal.normalize();
        Self {
            normal: n,
            distance: -n.dot(point),
        }
    }

    /// Create a plane from the general equation coefficients `ax + by + cz + d = 0`.
    pub fn from_coefficients(a: f64, b: f64, c: f64, d: f64) -> Self {
        let len = (a * a + b * b + c * c).sqrt();
        Self {
            normal: DVec3::new(a / len, b / len, c / len),
            distance: d / len,
        }
    }

    /// Signed distance from a point to this plane.
    /// Positive means the point is on the side the normal points to.
    #[inline]
    pub fn signed_distance(&self, point: DVec3) -> f64 {
        self.normal.dot(point) + self.distance
    }

    /// Project a point onto this plane.
    ///
    /// Returns the closest point on the plane to the given point.
    #[inline]
    pub fn project_point(&self, point: DVec3) -> DVec3 {
        point - self.normal * self.signed_distance(point)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_from_point_normal() {
        let p = Plane::from_point_normal(DVec3::new(0.0, 0.0, 5.0), DVec3::Z);
        assert!((p.distance - (-5.0)).abs() < 1e-12);
        assert!((p.normal - DVec3::Z).length() < 1e-12);
    }

    #[test]
    fn signed_distance_positive_side() {
        let p = Plane::from_point_normal(DVec3::ZERO, DVec3::Y);
        assert!((p.signed_distance(DVec3::new(0.0, 10.0, 0.0)) - 10.0).abs() < 1e-12);
    }

    #[test]
    fn signed_distance_negative_side() {
        let p = Plane::from_point_normal(DVec3::ZERO, DVec3::Y);
        assert!((p.signed_distance(DVec3::new(0.0, -3.0, 0.0)) - (-3.0)).abs() < 1e-12);
    }
}
