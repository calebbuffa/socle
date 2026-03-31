//! Axis-aligned bounding box.

use glam::DVec3;

use crate::culling::CullingResult;
use crate::plane::Plane;
use crate::sphere::BoundingSphere;

/// An axis-aligned bounding box defined by minimum and maximum corners.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisAlignedBoundingBox {
    /// Minimum corner (smallest x, y, z).
    pub min: DVec3,
    /// Maximum corner (largest x, y, z).
    pub max: DVec3,
}

impl AxisAlignedBoundingBox {
    /// Create a new AABB from min and max corners.
    pub fn new(min: DVec3, max: DVec3) -> Self {
        Self { min, max }
    }

    /// Create an AABB from a center and half-extents.
    pub fn from_center_half_extents(center: DVec3, half_extents: DVec3) -> Self {
        Self {
            min: center - half_extents,
            max: center + half_extents,
        }
    }

    /// Center of the bounding box.
    #[inline]
    pub fn center(&self) -> DVec3 {
        (self.min + self.max) * 0.5
    }

    /// Half-size along each axis.
    #[inline]
    pub fn half_extents(&self) -> DVec3 {
        (self.max - self.min) * 0.5
    }

    /// Test whether a point is inside the AABB.
    #[inline]
    pub fn contains(&self, point: DVec3) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
            && point.z >= self.min.z
            && point.z <= self.max.z
    }

    /// Compute the smallest enclosing bounding sphere.
    pub fn to_bounding_sphere(&self) -> BoundingSphere {
        let center = self.center();
        let radius = self.half_extents().length();
        BoundingSphere::new(center, radius)
    }

    /// Squared distance from the AABB to a point.
    /// Returns 0 if the point is inside.
    pub fn distance_squared_to(&self, point: DVec3) -> f64 {
        let clamped = point.clamp(self.min, self.max);
        clamped.distance_squared(point)
    }

    /// Test the AABB against a plane.
    pub fn intersect_plane(&self, plane: &Plane) -> CullingResult {
        let center = self.center();
        let half = self.half_extents();
        // Project half-extents onto the plane normal
        let r = half.x * plane.normal.x.abs()
            + half.y * plane.normal.y.abs()
            + half.z * plane.normal.z.abs();
        let dist = plane.signed_distance(center);
        if dist > r {
            CullingResult::Inside
        } else if dist < -r {
            CullingResult::Outside
        } else {
            CullingResult::Intersecting
        }
    }

    /// Compute the union of this AABB and another.
    pub fn union(&self, other: &AxisAlignedBoundingBox) -> AxisAlignedBoundingBox {
        AxisAlignedBoundingBox {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    /// Expand the AABB to include a point.
    pub fn expand(&self, point: DVec3) -> AxisAlignedBoundingBox {
        AxisAlignedBoundingBox {
            min: self.min.min(point),
            max: self.max.max(point),
        }
    }

    /// Compute the AABB that encloses a set of positions. Panics if the slice is empty.
    pub fn from_positions(positions: &[DVec3]) -> Self {
        assert!(!positions.is_empty());
        let mut min = positions[0];
        let mut max = positions[0];
        for &p in &positions[1..] {
            min = min.min(p);
            max = max.max(p);
        }
        Self { min, max }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_and_half_extents() {
        let aabb =
            AxisAlignedBoundingBox::new(DVec3::new(-1.0, -2.0, -3.0), DVec3::new(1.0, 2.0, 3.0));
        assert!((aabb.center() - DVec3::ZERO).length() < 1e-12);
        assert!((aabb.half_extents() - DVec3::new(1.0, 2.0, 3.0)).length() < 1e-12);
    }

    #[test]
    fn contains_point() {
        let aabb = AxisAlignedBoundingBox::new(DVec3::ZERO, DVec3::new(10.0, 10.0, 10.0));
        assert!(aabb.contains(DVec3::new(5.0, 5.0, 5.0)));
        assert!(!aabb.contains(DVec3::new(-1.0, 5.0, 5.0)));
    }

    #[test]
    fn from_center_half_extents() {
        let aabb = AxisAlignedBoundingBox::from_center_half_extents(
            DVec3::new(5.0, 5.0, 5.0),
            DVec3::new(2.0, 2.0, 2.0),
        );
        assert!((aabb.min - DVec3::new(3.0, 3.0, 3.0)).length() < 1e-12);
        assert!((aabb.max - DVec3::new(7.0, 7.0, 7.0)).length() < 1e-12);
    }

    #[test]
    fn intersect_plane_inside() {
        let aabb =
            AxisAlignedBoundingBox::new(DVec3::new(-1.0, 5.0, -1.0), DVec3::new(1.0, 7.0, 1.0));
        let p = Plane::from_point_normal(DVec3::ZERO, DVec3::Y);
        assert_eq!(aabb.intersect_plane(&p), CullingResult::Inside);
    }

    #[test]
    fn intersect_plane_outside() {
        let aabb =
            AxisAlignedBoundingBox::new(DVec3::new(-1.0, -7.0, -1.0), DVec3::new(1.0, -5.0, 1.0));
        let p = Plane::from_point_normal(DVec3::ZERO, DVec3::Y);
        assert_eq!(aabb.intersect_plane(&p), CullingResult::Outside);
    }

    #[test]
    fn distance_squared_inside_is_zero() {
        let aabb = AxisAlignedBoundingBox::new(DVec3::ZERO, DVec3::new(10.0, 10.0, 10.0));
        assert!(aabb.distance_squared_to(DVec3::new(5.0, 5.0, 5.0)) < 1e-12);
    }

    #[test]
    fn distance_squared_outside() {
        let aabb = AxisAlignedBoundingBox::new(DVec3::ZERO, DVec3::new(1.0, 1.0, 1.0));
        // Point at (3, 0, 0), closest point on AABB is (1, 0, 0), distance = 2
        let dsq = aabb.distance_squared_to(DVec3::new(3.0, 0.0, 0.0));
        assert!((dsq - 4.0).abs() < 1e-12);
    }

    #[test]
    fn union_aabbs() {
        let a = AxisAlignedBoundingBox::new(DVec3::ZERO, DVec3::ONE);
        let b = AxisAlignedBoundingBox::new(DVec3::new(5.0, 5.0, 5.0), DVec3::new(6.0, 6.0, 6.0));
        let u = a.union(&b);
        assert!((u.min - DVec3::ZERO).length() < 1e-12);
        assert!((u.max - DVec3::new(6.0, 6.0, 6.0)).length() < 1e-12);
    }
}
