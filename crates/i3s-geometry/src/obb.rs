//! Oriented bounding box — the primary bounding volume in I3S.
//!
//! An OBB is defined by a center, half-size extents along each local axis,
//! and a quaternion rotation. The I3S spec stores OBBs as:
//! - `center`: `[x, y, z]`
//! - `halfSize`: `[sx, sy, sz]`
//! - `quaternion`: `[x, y, z, w]`

use glam::{DMat3, DMat4, DQuat, DVec3};

use crate::aabb::AxisAlignedBoundingBox;
use crate::culling::CullingResult;
use crate::plane::Plane;
use crate::sphere::BoundingSphere;

/// An oriented bounding box defined by center, half-size, and rotation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrientedBoundingBox {
    /// Center of the box in world coordinates.
    pub center: DVec3,
    /// Half-size extents along each local axis.
    pub half_size: DVec3,
    /// Rotation quaternion from local to world space.
    pub quaternion: DQuat,
}

impl OrientedBoundingBox {
    /// Create an OBB from raw I3S arrays: center `[x,y,z]`, half_size `[sx,sy,sz]`,
    /// quaternion `[x,y,z,w]`.
    pub fn from_i3s(center: [f64; 3], half_size: [f64; 3], quaternion: [f64; 4]) -> Self {
        Self {
            center: DVec3::from_array(center),
            half_size: DVec3::from_array(half_size),
            quaternion: DQuat::from_xyzw(
                quaternion[0],
                quaternion[1],
                quaternion[2],
                quaternion[3],
            ),
        }
    }

    /// The 3x3 rotation matrix (columns are the local axes in world space).
    #[inline]
    pub fn rotation_matrix(&self) -> DMat3 {
        DMat3::from_quat(self.quaternion)
    }

    /// The three half-axes of the box in world space (columns of rotation * half_size).
    pub fn half_axes(&self) -> [DVec3; 3] {
        let rot = self.rotation_matrix();
        [
            rot.x_axis * self.half_size.x,
            rot.y_axis * self.half_size.y,
            rot.z_axis * self.half_size.z,
        ]
    }

    /// The half-axes as a 3x3 matrix (columns are the scaled local axes).
    ///
    /// Equivalent to cesium-native `OrientedBoundingBox::getHalfAxes()` which
    /// returns a `dmat3`.
    pub fn half_axes_matrix(&self) -> DMat3 {
        let axes = self.half_axes();
        DMat3::from_cols(axes[0], axes[1], axes[2])
    }

    /// The inverse of the half-axes matrix.
    ///
    /// Mirrors cesium-native `OrientedBoundingBox::getInverseHalfAxes()`.
    pub fn inverse_half_axes(&self) -> DMat3 {
        self.half_axes_matrix().inverse()
    }

    /// The lengths (full extents) along each local axis (2 * half_size).
    ///
    /// Mirrors cesium-native `OrientedBoundingBox::getLengths()`.
    #[inline]
    pub fn lengths(&self) -> DVec3 {
        self.half_size * 2.0
    }

    /// Test the OBB against a plane using the separating-axis theorem.
    ///
    /// Returns [`CullingResult::Inside`] if the OBB is entirely on the positive
    /// side (same side as the normal), [`CullingResult::Outside`] if entirely
    /// on the negative side, and [`CullingResult::Intersecting`] otherwise.
    pub fn intersect_plane(&self, plane: &Plane) -> CullingResult {
        let axes = self.half_axes();
        // Project the half-axes onto the plane normal and sum their magnitudes
        let r = axes[0].dot(plane.normal).abs()
            + axes[1].dot(plane.normal).abs()
            + axes[2].dot(plane.normal).abs();
        let dist = plane.signed_distance(self.center);
        if dist > r {
            CullingResult::Inside
        } else if dist < -r {
            CullingResult::Outside
        } else {
            CullingResult::Intersecting
        }
    }

    /// Test whether a point is inside the OBB.
    pub fn contains(&self, point: DVec3) -> bool {
        let local = self.quaternion.inverse() * (point - self.center);
        local.x.abs() <= self.half_size.x
            && local.y.abs() <= self.half_size.y
            && local.z.abs() <= self.half_size.z
    }

    /// Squared distance from the OBB surface to a point.
    /// Returns 0 if the point is inside.
    pub fn distance_squared_to(&self, point: DVec3) -> f64 {
        let local = self.quaternion.inverse() * (point - self.center);
        let clamped = DVec3::new(
            local.x.clamp(-self.half_size.x, self.half_size.x),
            local.y.clamp(-self.half_size.y, self.half_size.y),
            local.z.clamp(-self.half_size.z, self.half_size.z),
        );
        (local - clamped).length_squared()
    }

    /// Compute the axis-aligned bounding box that encloses this OBB.
    pub fn to_aabb(&self) -> AxisAlignedBoundingBox {
        let axes = self.half_axes();
        // The AABB half-extents are the sum of absolute components of each half-axis
        let extent = DVec3::new(
            axes[0].x.abs() + axes[1].x.abs() + axes[2].x.abs(),
            axes[0].y.abs() + axes[1].y.abs() + axes[2].y.abs(),
            axes[0].z.abs() + axes[1].z.abs() + axes[2].z.abs(),
        );
        AxisAlignedBoundingBox::new(self.center - extent, self.center + extent)
    }

    /// Compute the smallest enclosing bounding sphere.
    pub fn to_bounding_sphere(&self) -> BoundingSphere {
        BoundingSphere::new(self.center, self.half_size.length())
    }

    /// Compute the 8 corner vertices of the OBB in world space.
    pub fn corners(&self) -> [DVec3; 8] {
        let axes = self.half_axes();
        [
            self.center - axes[0] - axes[1] - axes[2],
            self.center + axes[0] - axes[1] - axes[2],
            self.center - axes[0] + axes[1] - axes[2],
            self.center + axes[0] + axes[1] - axes[2],
            self.center - axes[0] - axes[1] + axes[2],
            self.center + axes[0] - axes[1] + axes[2],
            self.center - axes[0] + axes[1] + axes[2],
            self.center + axes[0] + axes[1] + axes[2],
        ]
    }

    /// Construct a tight axis-aligned OBB enclosing the given points.
    ///
    /// The resulting OBB has identity rotation (axis-aligned) with half-extents
    /// sized to enclose all points. Used after CRS transformation of corners.
    pub fn from_corners(corners: &[DVec3]) -> Self {
        assert!(!corners.is_empty());
        let mut min = corners[0];
        let mut max = corners[0];
        for &c in &corners[1..] {
            min = min.min(c);
            max = max.max(c);
        }
        let center = (min + max) * 0.5;
        let half_size = (max - min) * 0.5;
        Self {
            center,
            half_size,
            quaternion: DQuat::IDENTITY,
        }
    }

    /// Create an OBB from an axis-aligned bounding box.
    ///
    /// Mirrors cesium-native `OrientedBoundingBox::fromAxisAligned`.
    pub fn from_axis_aligned(aabb: &AxisAlignedBoundingBox) -> Self {
        Self {
            center: aabb.center(),
            half_size: aabb.half_extents(),
            quaternion: DQuat::IDENTITY,
        }
    }

    /// Create an OBB from a bounding sphere.
    ///
    /// Mirrors cesium-native `OrientedBoundingBox::fromSphere`.
    pub fn from_sphere(sphere: &BoundingSphere) -> Self {
        Self {
            center: sphere.center,
            half_size: DVec3::splat(sphere.radius),
            quaternion: DQuat::IDENTITY,
        }
    }

    /// Transform the OBB by a 4x4 matrix.
    ///
    /// Decomposes the rotation and scale from the matrix, applies them
    /// to the OBB's orientation and half-size. Mirrors cesium-native
    /// `OrientedBoundingBox::transform`.
    pub fn transform(&self, transformation: &DMat4) -> OrientedBoundingBox {
        let center = transformation.transform_point3(self.center);
        // Extract the upper-left 3x3 and apply to half-axes
        let m3 = DMat3::from_cols(
            transformation.x_axis.truncate(),
            transformation.y_axis.truncate(),
            transformation.z_axis.truncate(),
        );
        let half_axes_mat = self.half_axes_matrix();
        let new_half_axes = m3 * half_axes_mat;
        // Decompose back to quaternion + half_size via SVD-like extraction
        // Extract scale (column lengths) and rotation
        let c0 = new_half_axes.x_axis;
        let c1 = new_half_axes.y_axis;
        let c2 = new_half_axes.z_axis;
        let sx = c0.length();
        let sy = c1.length();
        let sz = c2.length();
        let half_size = DVec3::new(sx, sy, sz);
        let rot_mat = if sx > 1e-15 && sy > 1e-15 && sz > 1e-15 {
            DMat3::from_cols(c0 / sx, c1 / sy, c2 / sz)
        } else {
            DMat3::IDENTITY
        };
        let quaternion = DQuat::from_mat3(&rot_mat);
        OrientedBoundingBox {
            center,
            half_size,
            quaternion,
        }
    }

    /// Compute the approximate screen-space projected area in pixels
    /// of this OBB for LOD selection.
    ///
    /// Uses a simplified projection: projects the bounding sphere of the OBB
    /// and computes the projected disc area. This matches the approach used
    /// by typical I3S viewers for `maxScreenThreshold` evaluation.
    ///
    /// - `camera_position`: camera position in world coordinates
    /// - `viewport_height`: viewport height in pixels
    /// - `fov_y`: vertical field of view in radians
    pub fn projected_area(&self, camera_position: DVec3, viewport_height: f64, fov_y: f64) -> f64 {
        let dist = self.center.distance(camera_position);
        if dist < 1e-10 {
            return f64::MAX;
        }
        let radius = self.half_size.length();
        // Projected diameter in pixels
        let projected_diameter = (radius * viewport_height) / (dist * (fov_y * 0.5).tan());
        // Return area of projected disc
        std::f64::consts::PI * 0.25 * projected_diameter * projected_diameter
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity_obb(center: DVec3, half_size: DVec3) -> OrientedBoundingBox {
        OrientedBoundingBox {
            center,
            half_size,
            quaternion: DQuat::IDENTITY,
        }
    }

    #[test]
    fn from_i3s_arrays() {
        let obb =
            OrientedBoundingBox::from_i3s([1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [0.0, 0.0, 0.0, 1.0]);
        assert!((obb.center - DVec3::new(1.0, 2.0, 3.0)).length() < 1e-12);
        assert!((obb.half_size - DVec3::new(4.0, 5.0, 6.0)).length() < 1e-12);
    }

    #[test]
    fn contains_center() {
        let obb = identity_obb(DVec3::ZERO, DVec3::new(1.0, 2.0, 3.0));
        assert!(obb.contains(DVec3::ZERO));
    }

    #[test]
    fn contains_inside() {
        let obb = identity_obb(DVec3::ZERO, DVec3::new(5.0, 5.0, 5.0));
        assert!(obb.contains(DVec3::new(3.0, 3.0, 3.0)));
    }

    #[test]
    fn not_contains_outside() {
        let obb = identity_obb(DVec3::ZERO, DVec3::new(1.0, 1.0, 1.0));
        assert!(!obb.contains(DVec3::new(2.0, 0.0, 0.0)));
    }

    #[test]
    fn contains_rotated() {
        // Rotate 45 degrees around Z-axis
        let obb = OrientedBoundingBox {
            center: DVec3::ZERO,
            half_size: DVec3::new(10.0, 1.0, 1.0),
            quaternion: DQuat::from_rotation_z(std::f64::consts::FRAC_PI_4),
        };
        // Point at (5, 5, 0) should be inside because it's along the rotated X-axis
        assert!(obb.contains(DVec3::new(5.0, 5.0, 0.0)));
        // Point at (10, 0, 0) should be outside the rotated box
        assert!(!obb.contains(DVec3::new(10.0, 0.0, 0.0)));
    }

    #[test]
    fn distance_squared_inside_is_zero() {
        let obb = identity_obb(DVec3::ZERO, DVec3::new(5.0, 5.0, 5.0));
        assert!(obb.distance_squared_to(DVec3::new(1.0, 1.0, 1.0)) < 1e-12);
    }

    #[test]
    fn distance_squared_outside() {
        let obb = identity_obb(DVec3::ZERO, DVec3::ONE);
        // Point at (3, 0, 0), closest on OBB is (1, 0, 0), distance = 2
        let dsq = obb.distance_squared_to(DVec3::new(3.0, 0.0, 0.0));
        assert!((dsq - 4.0).abs() < 1e-10);
    }

    #[test]
    fn intersect_plane_inside() {
        let obb = identity_obb(DVec3::new(0.0, 10.0, 0.0), DVec3::ONE);
        let p = Plane::from_point_normal(DVec3::ZERO, DVec3::Y);
        assert_eq!(obb.intersect_plane(&p), CullingResult::Inside);
    }

    #[test]
    fn intersect_plane_outside() {
        let obb = identity_obb(DVec3::new(0.0, -10.0, 0.0), DVec3::ONE);
        let p = Plane::from_point_normal(DVec3::ZERO, DVec3::Y);
        assert_eq!(obb.intersect_plane(&p), CullingResult::Outside);
    }

    #[test]
    fn intersect_plane_intersecting() {
        let obb = identity_obb(DVec3::new(0.0, 0.5, 0.0), DVec3::ONE);
        let p = Plane::from_point_normal(DVec3::ZERO, DVec3::Y);
        assert_eq!(obb.intersect_plane(&p), CullingResult::Intersecting);
    }

    #[test]
    fn to_aabb_identity() {
        let obb = identity_obb(DVec3::new(5.0, 5.0, 5.0), DVec3::new(1.0, 2.0, 3.0));
        let aabb = obb.to_aabb();
        assert!((aabb.min - DVec3::new(4.0, 3.0, 2.0)).length() < 1e-12);
        assert!((aabb.max - DVec3::new(6.0, 7.0, 8.0)).length() < 1e-12);
    }

    #[test]
    fn to_bounding_sphere() {
        let obb = identity_obb(DVec3::new(1.0, 2.0, 3.0), DVec3::new(3.0, 4.0, 0.0));
        let s = obb.to_bounding_sphere();
        assert!((s.center - DVec3::new(1.0, 2.0, 3.0)).length() < 1e-12);
        assert!((s.radius - 5.0).abs() < 1e-12);
    }

    #[test]
    fn projected_area_far_away() {
        let obb = identity_obb(DVec3::ZERO, DVec3::new(10.0, 10.0, 10.0));
        let area1 = obb.projected_area(DVec3::new(0.0, 0.0, 100.0), 1080.0, 1.0);
        let area2 = obb.projected_area(DVec3::new(0.0, 0.0, 1000.0), 1080.0, 1.0);
        // Farther away → smaller projected area
        assert!(area2 < area1);
    }

    #[test]
    fn projected_area_very_close_is_large() {
        let obb = identity_obb(DVec3::ZERO, DVec3::new(10.0, 10.0, 10.0));
        let area = obb.projected_area(DVec3::new(0.0, 0.0, 0.001), 1080.0, 1.0);
        assert!(area > 1e6);
    }
}
