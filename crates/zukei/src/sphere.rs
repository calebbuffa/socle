//! Bounding sphere.

use glam_dep::{DMat4, DVec3};

use crate::culling::CullingResult;
use crate::plane::Plane;

/// A bounding sphere defined by a center point and radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingSphere {
    pub center: DVec3,
    pub radius: f64,
}

impl BoundingSphere {
    /// Create a new bounding sphere.
    pub fn new(center: DVec3, radius: f64) -> Self {
        Self { center, radius }
    }

    /// Test whether a point is inside the sphere.
    #[inline]
    pub fn contains(&self, point: DVec3) -> bool {
        self.center.distance_squared(point) <= self.radius * self.radius
    }

    /// Squared distance from the sphere surface to a point.
    /// Returns 0 if the point is inside the sphere.
    #[inline]
    pub fn distance_squared_to(&self, point: DVec3) -> f64 {
        let d = self.center.distance(point) - self.radius;
        if d <= 0.0 { 0.0 } else { d * d }
    }

    /// Test the sphere against a plane.
    pub fn intersect_plane(&self, plane: &Plane) -> CullingResult {
        let dist = plane.signed_distance(self.center);
        if dist > self.radius {
            CullingResult::Inside
        } else if dist < -self.radius {
            CullingResult::Outside
        } else {
            CullingResult::Intersecting
        }
    }

    pub fn transform(&self, transformation: &DMat4) -> BoundingSphere {
        let center = transformation.transform_point3(self.center);
        // Maximum scale factor = max column length of the upper-left 3x3
        let scale = {
            let sx = transformation.x_axis.truncate().length();
            let sy = transformation.y_axis.truncate().length();
            let sz = transformation.z_axis.truncate().length();
            sx.max(sy).max(sz)
        };
        BoundingSphere::new(center, self.radius * scale)
    }

    /// Compute the union of this sphere and another, returning a sphere
    /// that encloses both.
    pub fn union(&self, other: &BoundingSphere) -> BoundingSphere {
        let to_other = other.center - self.center;
        let dist = to_other.length();

        // One sphere contains the other
        if dist + other.radius <= self.radius {
            return *self;
        }
        if dist + self.radius <= other.radius {
            return *other;
        }

        // General case
        let new_radius = (dist + self.radius + other.radius) * 0.5;
        let new_center = self.center + to_other * ((new_radius - self.radius) / dist);
        BoundingSphere::new(new_center, new_radius)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_point() {
        let s = BoundingSphere::new(DVec3::ZERO, 10.0);
        assert!(s.contains(DVec3::new(5.0, 0.0, 0.0)));
        assert!(!s.contains(DVec3::new(11.0, 0.0, 0.0)));
    }

    #[test]
    fn intersect_plane_inside() {
        let s = BoundingSphere::new(DVec3::new(0.0, 10.0, 0.0), 1.0);
        let p = Plane::from_point_normal(DVec3::ZERO, DVec3::Y);
        assert_eq!(s.intersect_plane(&p), CullingResult::Inside);
    }

    #[test]
    fn intersect_plane_outside() {
        let s = BoundingSphere::new(DVec3::new(0.0, -10.0, 0.0), 1.0);
        let p = Plane::from_point_normal(DVec3::ZERO, DVec3::Y);
        assert_eq!(s.intersect_plane(&p), CullingResult::Outside);
    }

    #[test]
    fn intersect_plane_intersecting() {
        let s = BoundingSphere::new(DVec3::new(0.0, 0.5, 0.0), 1.0);
        let p = Plane::from_point_normal(DVec3::ZERO, DVec3::Y);
        assert_eq!(s.intersect_plane(&p), CullingResult::Intersecting);
    }

    #[test]
    fn union_spheres() {
        let s1 = BoundingSphere::new(DVec3::ZERO, 1.0);
        let s2 = BoundingSphere::new(DVec3::new(10.0, 0.0, 0.0), 1.0);
        let u = s1.union(&s2);
        assert!(u.contains(DVec3::new(-1.0, 0.0, 0.0)));
        assert!(u.contains(DVec3::new(11.0, 0.0, 0.0)));
        assert!((u.radius - 6.0).abs() < 1e-10);
    }

    #[test]
    fn union_contained() {
        let big = BoundingSphere::new(DVec3::ZERO, 100.0);
        let small = BoundingSphere::new(DVec3::new(1.0, 0.0, 0.0), 1.0);
        let u = big.union(&small);
        assert!((u.center - big.center).length() < 1e-12);
        assert!((u.radius - big.radius).abs() < 1e-12);
    }
}
