//! View frustum culling volume composed of inward-facing planes.

use glam_dep::{DMat4, DVec3};

use crate::aabb::AxisAlignedBoundingBox;
use crate::bounds::SpatialBounds;
use crate::culling::CullingResult;
use crate::obb::OrientedBoundingBox;
use crate::plane::Plane;
use crate::sphere::BoundingSphere;
use crate::transforms::Transforms;

/// A convex culling volume defined by frustum planes.
///
/// May contain 4 planes (side-only, for LOD selection) or 6 planes
/// (including near/far, from a view-projection matrix). Normals point
/// inward (toward the visible region).
#[derive(Debug, Clone, PartialEq)]
pub struct CullingVolume {
    /// Frustum planes with inward-facing normals.
    pub planes: Vec<Plane>,
}

impl CullingVolume {
    /// Create a culling volume from an explicit set of planes.
    pub fn from_planes(planes: Vec<Plane>) -> Self {
        Self { planes: planes }
    }

    /// Build 4 side frustum planes from camera parameters.
    ///
    /// Creates left, right, bottom, top planes that pass through the camera
    /// position. No near/far planes — those are a GPU concern, not needed
    /// for LOD selection.
    pub fn from_camera(
        position: DVec3,
        direction: DVec3,
        up: DVec3,
        fov_y: f64,
        aspect_ratio: f64,
    ) -> Self {
        let dir = direction.normalize();
        let right = dir.cross(up).normalize();
        let cam_up = right.cross(dir).normalize();

        let half_v = (fov_y * 0.5).tan();
        let half_h = half_v * aspect_ratio;

        // Each plane passes through `position` with an inward-facing normal.
        // Left plane:   normal tilted toward +right from forward
        // Right plane:  normal tilted toward -right from forward
        // Bottom plane: normal tilted toward +up from forward
        // Top plane:    normal tilted toward -up from forward
        let left_normal = (dir * half_h + right).normalize();
        let right_normal = (dir * half_h - right).normalize();
        let bottom_normal = (dir * half_v + cam_up).normalize();
        let top_normal = (dir * half_v - cam_up).normalize();

        Self {
            planes: vec![
                Plane::from_point_normal(position, left_normal),
                Plane::from_point_normal(position, right_normal),
                Plane::from_point_normal(position, bottom_normal),
                Plane::from_point_normal(position, top_normal),
            ],
        }
    }

    /// Extract frustum planes from a view-projection matrix.
    ///
    /// Uses the Gribb/Hartmann plane extraction method. The resulting
    /// planes have inward-facing normals (positive side = inside frustum).
    /// Produces 6 planes (left, right, bottom, top, near, far).
    pub fn from_view_projection(vp: &DMat4) -> Self {
        let row = |r: usize| [vp.col(0)[r], vp.col(1)[r], vp.col(2)[r], vp.col(3)[r]];

        let r0 = row(0);
        let r1 = row(1);
        let r2 = row(2);
        let r3 = row(3);

        // Left:   row3 + row0
        // Right:  row3 - row0
        // Bottom: row3 + row1
        // Top:    row3 - row1
        // Near:   row3 + row2
        // Far:    row3 - row2
        let planes = vec![
            Plane::from_coefficients(r3[0] + r0[0], r3[1] + r0[1], r3[2] + r0[2], r3[3] + r0[3]),
            Plane::from_coefficients(r3[0] - r0[0], r3[1] - r0[1], r3[2] - r0[2], r3[3] - r0[3]),
            Plane::from_coefficients(r3[0] + r1[0], r3[1] + r1[1], r3[2] + r1[2], r3[3] + r1[3]),
            Plane::from_coefficients(r3[0] - r1[0], r3[1] - r1[1], r3[2] - r1[2], r3[3] - r1[3]),
            Plane::from_coefficients(r3[0] + r2[0], r3[1] + r2[1], r3[2] + r2[2], r3[3] + r2[3]),
            Plane::from_coefficients(r3[0] - r2[0], r3[1] - r2[1], r3[2] - r2[2], r3[3] - r2[3]),
        ];

        Self { planes }
    }

    /// Build 4 side frustum planes from camera position, direction, and
    /// separate horizontal/vertical FOV. Produces left, right, bottom, top
    /// planes (no near/far).
    pub fn from_fov(position: DVec3, direction: DVec3, up: DVec3, fov_x: f64, fov_y: f64) -> Self {
        let dir = direction.normalize();
        let right = dir.cross(up).normalize();
        let cam_up = right.cross(dir).normalize();

        let t = (fov_y * 0.5).tan();
        let r = (fov_x * 0.5).tan();
        let l = -r;
        let b = -t;

        let pos_len = position.length();
        let n = 1.0_f64.max(pos_len.next_up() - pos_len);
        let near_center = position + dir * n;

        // Left plane
        let left_normal = {
            let pt = near_center + right * l;
            let v = (pt - position).normalize();
            v.cross(cam_up).normalize()
        };
        let left_plane = Plane::new(left_normal, -left_normal.dot(position));

        // Right plane
        let right_normal = {
            let pt = near_center + right * r;
            let v = (pt - position).normalize();
            cam_up.cross(v).normalize()
        };
        let right_plane = Plane::new(right_normal, -right_normal.dot(position));

        // Bottom plane
        let bottom_normal = {
            let pt = near_center + cam_up * b;
            let v = (pt - position).normalize();
            right.cross(v).normalize()
        };
        let bottom_plane = Plane::new(bottom_normal, -bottom_normal.dot(position));

        // Top plane
        let top_normal = {
            let pt = near_center + cam_up * t;
            let v = (pt - position).normalize();
            v.cross(right).normalize()
        };
        let top_plane = Plane::new(top_normal, -top_normal.dot(position));

        Self {
            planes: vec![left_plane, right_plane, bottom_plane, top_plane],
        }
    }

    /// Build a frustum from asymmetric perspective bounds.
    pub fn from_asymmetric_perspective(
        position: DVec3,
        direction: DVec3,
        up: DVec3,
        left: f64,
        right: f64,
        bottom: f64,
        top: f64,
        near: f64,
    ) -> Self {
        let proj =
            Transforms::create_perspective_offcenter(left, right, bottom, top, near, f64::INFINITY);
        let view = Transforms::create_view_matrix(position, direction, up);
        let clip = proj * view;
        Self::from_view_projection(&clip)
    }

    /// Build a frustum from orthographic bounds.
    pub fn from_orthographic(
        position: DVec3,
        direction: DVec3,
        up: DVec3,
        left: f64,
        right: f64,
        bottom: f64,
        top: f64,
        near: f64,
    ) -> Self {
        let proj = Transforms::create_orthographic(left, right, bottom, top, near, f64::INFINITY);
        let view = Transforms::create_view_matrix(position, direction, up);
        let clip = proj * view;
        Self::from_view_projection(&clip)
    }

    /// Test a bounding sphere against the frustum.
    pub fn visibility_sphere(&self, sphere: &BoundingSphere) -> CullingResult {
        let mut all_inside = true;
        for plane in &self.planes {
            let dist = plane.signed_distance(sphere.center);
            if dist < -sphere.radius {
                return CullingResult::Outside;
            }
            if dist < sphere.radius {
                all_inside = false;
            }
        }
        if all_inside {
            CullingResult::Inside
        } else {
            CullingResult::Intersecting
        }
    }

    /// Test an oriented bounding box against the frustum.
    pub fn visibility_obb(&self, obb: &OrientedBoundingBox) -> CullingResult {
        let half_axes = obb.half_axes();
        let mut all_inside = true;
        for plane in &self.planes {
            let r = half_axes[0].dot(plane.normal).abs()
                + half_axes[1].dot(plane.normal).abs()
                + half_axes[2].dot(plane.normal).abs();
            let dist = plane.signed_distance(obb.center);
            if dist < -r {
                return CullingResult::Outside;
            }
            if dist < r {
                all_inside = false;
            }
        }
        if all_inside {
            CullingResult::Inside
        } else {
            CullingResult::Intersecting
        }
    }

    /// Test an axis-aligned bounding box against the frustum.
    pub fn visibility_aabb(&self, aabb: &AxisAlignedBoundingBox) -> CullingResult {
        let center = aabb.center();
        let half = aabb.half_extents();
        let mut all_inside = true;
        for plane in &self.planes {
            let r = half.x * plane.normal.x.abs()
                + half.y * plane.normal.y.abs()
                + half.z * plane.normal.z.abs();
            let dist = plane.signed_distance(center);
            if dist < -r {
                return CullingResult::Outside;
            }
            if dist < r {
                all_inside = false;
            }
        }
        if all_inside {
            CullingResult::Inside
        } else {
            CullingResult::Intersecting
        }
    }

    /// Test a [`SpatialBounds`] enum against the frustum.
    ///
    /// Dispatches to the appropriate typed visibility test. `Rectangle` bounds
    /// are conservatively treated as visible (no 2D frustum test).
    pub fn visibility_bounds(&self, bounds: &SpatialBounds) -> CullingResult {
        match bounds {
            SpatialBounds::Sphere { center, radius } => {
                let sphere = BoundingSphere::new(DVec3::from(center), *radius);
                self.visibility_sphere(&sphere)
            }
            SpatialBounds::OrientedBox { center, half_axes } => {
                // Test directly using the half_axes columns against frustum planes,
                // matching the same math as visibility_obb without needing OBB conversion.
                let c = DVec3::from(center);
                let axes = [
                    DVec3::from(&half_axes.cols[0]),
                    DVec3::from(&half_axes.cols[1]),
                    DVec3::from(&half_axes.cols[2]),
                ];
                let mut all_inside = true;
                for plane in &self.planes {
                    let r = axes[0].dot(plane.normal).abs()
                        + axes[1].dot(plane.normal).abs()
                        + axes[2].dot(plane.normal).abs();
                    let dist = plane.signed_distance(c);
                    if dist < -r {
                        return CullingResult::Outside;
                    }
                    if dist < r {
                        all_inside = false;
                    }
                }
                if all_inside {
                    CullingResult::Inside
                } else {
                    CullingResult::Intersecting
                }
            }
            SpatialBounds::AxisAlignedBox { min, max } => {
                let aabb = AxisAlignedBoundingBox::new(DVec3::from(min), DVec3::from(max));
                self.visibility_aabb(&aabb)
            }
            SpatialBounds::Rectangle { .. } => {
                // 2D rectangle bounds — conservatively visible.
                CullingResult::Intersecting
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use glam_dep::DVec3;

    use super::*;

    /// Build an orthographic frustum from -1..1 in each axis — the identity clip cube.
    fn identity_frustum() -> CullingVolume {
        CullingVolume::from_view_projection(&DMat4::IDENTITY)
    }

    #[test]
    fn sphere_inside_identity_frustum() {
        let cv = identity_frustum();
        let s = BoundingSphere::new(DVec3::ZERO, 0.5);
        assert_eq!(cv.visibility_sphere(&s), CullingResult::Inside);
    }

    #[test]
    fn sphere_outside_identity_frustum() {
        let cv = identity_frustum();
        let s = BoundingSphere::new(DVec3::new(10.0, 0.0, 0.0), 0.5);
        assert_eq!(cv.visibility_sphere(&s), CullingResult::Outside);
    }

    #[test]
    fn sphere_intersecting_identity_frustum() {
        let cv = identity_frustum();
        // Sphere at edge of frustum
        let s = BoundingSphere::new(DVec3::new(0.9, 0.0, 0.0), 0.5);
        assert_eq!(cv.visibility_sphere(&s), CullingResult::Intersecting);
    }

    #[test]
    fn obb_inside_identity_frustum() {
        let cv = identity_frustum();
        let obb =
            OrientedBoundingBox::from_i3s([0.0, 0.0, 0.0], [0.3, 0.3, 0.3], [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(cv.visibility_obb(&obb), CullingResult::Inside);
    }

    #[test]
    fn obb_outside_identity_frustum() {
        let cv = identity_frustum();
        let obb =
            OrientedBoundingBox::from_i3s([10.0, 0.0, 0.0], [1.0, 1.0, 1.0], [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(cv.visibility_obb(&obb), CullingResult::Outside);
    }

    #[test]
    fn obb_intersecting_identity_frustum() {
        let cv = identity_frustum();
        let obb =
            OrientedBoundingBox::from_i3s([0.8, 0.0, 0.0], [0.5, 0.5, 0.5], [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(cv.visibility_obb(&obb), CullingResult::Intersecting);
    }

    #[test]
    fn perspective_frustum() {
        // Build a simple perspective projection looking down -Z
        let proj = DMat4::perspective_lh(
            std::f64::consts::FRAC_PI_2, // 90 degree FOV
            1.0,                         // aspect ratio
            0.1,                         // near
            100.0,                       // far
        );
        let view = DMat4::look_at_lh(
            DVec3::new(0.0, 0.0, -5.0), // eye
            DVec3::ZERO,                // target
            DVec3::Y,                   // up
        );
        let vp = proj * view;
        let cv = CullingVolume::from_view_projection(&vp);

        // Object at origin should be visible
        let s_visible = BoundingSphere::new(DVec3::ZERO, 1.0);
        assert_ne!(cv.visibility_sphere(&s_visible), CullingResult::Outside);

        // Object far behind the camera should be culled
        let s_behind = BoundingSphere::new(DVec3::new(0.0, 0.0, -100.0), 1.0);
        assert_eq!(cv.visibility_sphere(&s_behind), CullingResult::Outside);
    }

    #[test]
    fn camera_frustum_4_planes() {
        // Camera at origin looking down -Z, 90° vertical FOV, square viewport
        let cv = CullingVolume::from_camera(
            DVec3::ZERO,
            DVec3::NEG_Z,
            DVec3::Y,
            std::f64::consts::FRAC_PI_2, // 90° fov_y
            1.0,                         // aspect ratio
        );

        assert_eq!(cv.planes.len(), 4);

        // Sphere directly ahead → visible
        let s_ahead = BoundingSphere::new(DVec3::new(0.0, 0.0, -10.0), 1.0);
        assert_ne!(cv.visibility_sphere(&s_ahead), CullingResult::Outside);

        // Sphere far to the side → culled
        let s_side = BoundingSphere::new(DVec3::new(100.0, 0.0, -1.0), 1.0);
        assert_eq!(cv.visibility_sphere(&s_side), CullingResult::Outside);

        // Sphere behind the camera → culled
        let s_behind = BoundingSphere::new(DVec3::new(0.0, 0.0, 10.0), 1.0);
        assert_eq!(cv.visibility_sphere(&s_behind), CullingResult::Outside);
    }
}
