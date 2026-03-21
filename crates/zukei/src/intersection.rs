//! Ray intersection tests for geometric primitives.

use glam_dep::{DVec2, DVec3};

use crate::aabb::AxisAlignedBoundingBox;
use crate::obb::OrientedBoundingBox;
use crate::plane::Plane;
use crate::ray::Ray;
use crate::sphere::BoundingSphere;

/// Intersect a ray with a plane. Returns the parameter `t` along the ray,
/// or `None` if the ray is parallel to the plane.
pub fn ray_plane(ray: &Ray, plane: &Plane) -> Option<f64> {
    let denom = plane.normal.dot(ray.direction);
    if denom.abs() < 1e-15 {
        return None;
    }
    let t = -(plane.normal.dot(ray.origin) + plane.distance) / denom;
    Some(t)
}

/// Intersect a ray with a bounding sphere. Returns the `t` parameter of the
/// nearest intersection point, or `None` if no intersection.
/// Only returns hits with `t >= 0` (in front of the ray).
pub fn ray_sphere(ray: &Ray, sphere: &BoundingSphere) -> Option<f64> {
    let oc = ray.origin - sphere.center;
    let b = oc.dot(ray.direction);
    let c = oc.length_squared() - sphere.radius * sphere.radius;
    let discriminant = b * b - c;
    if discriminant < 0.0 {
        return None;
    }
    let sqrt_d = discriminant.sqrt();
    let t1 = -b - sqrt_d;
    if t1 >= 0.0 {
        return Some(t1);
    }
    let t2 = -b + sqrt_d;
    if t2 >= 0.0 {
        return Some(t2);
    }
    None
}

/// Intersect a ray with an axis-aligned bounding box (slab method).
/// Returns the `t` parameter of the nearest intersection, or `None`.
pub fn ray_aabb(ray: &Ray, aabb: &AxisAlignedBoundingBox) -> Option<f64> {
    let inv_dir = DVec3::new(
        if ray.direction.x.abs() > 1e-15 {
            1.0 / ray.direction.x
        } else {
            f64::INFINITY * ray.direction.x.signum()
        },
        if ray.direction.y.abs() > 1e-15 {
            1.0 / ray.direction.y
        } else {
            f64::INFINITY * ray.direction.y.signum()
        },
        if ray.direction.z.abs() > 1e-15 {
            1.0 / ray.direction.z
        } else {
            f64::INFINITY * ray.direction.z.signum()
        },
    );

    let t1 = (aabb.min - ray.origin) * inv_dir;
    let t2 = (aabb.max - ray.origin) * inv_dir;

    let t_min = t1.min(t2);
    let t_max = t1.max(t2);

    let t_enter = t_min.x.max(t_min.y).max(t_min.z);
    let t_exit = t_max.x.min(t_max.y).min(t_max.z);

    if t_enter > t_exit || t_exit < 0.0 {
        return None;
    }

    if t_enter >= 0.0 {
        Some(t_enter)
    } else {
        Some(t_exit)
    }
}

/// Intersect a ray with an oriented bounding box.
/// Transforms the ray into the OBB's local space and uses the AABB slab test.
pub fn ray_obb(ray: &Ray, obb: &OrientedBoundingBox) -> Option<f64> {
    let inv_q = obb.quaternion.inverse();
    let local_origin = inv_q * (ray.origin - obb.center);
    let local_dir = inv_q * ray.direction;
    let local_ray = Ray {
        origin: local_origin,
        direction: local_dir.normalize(),
    };
    let local_aabb = AxisAlignedBoundingBox::new(-obb.half_size, obb.half_size);
    let t_local = ray_aabb(&local_ray, &local_aabb)?;
    // Scale t back by the direction ratio (local_dir may have different length)
    Some(t_local * local_dir.length() / ray.direction.length())
}

/// Intersect a ray with a triangle using the Möller–Trumbore algorithm.
/// Returns the `t` parameter along the ray, or `None` if no hit.
///
/// Vertices are specified counter-clockwise when viewed from the front face.
/// This function tests both front and back faces.
pub fn ray_triangle(ray: &Ray, v0: DVec3, v1: DVec3, v2: DVec3) -> Option<f64> {
    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    let h = ray.direction.cross(edge2);
    let a = edge1.dot(h);

    if a.abs() < 1e-15 {
        return None; // Ray parallel to triangle
    }

    let f = 1.0 / a;
    let s = ray.origin - v0;
    let u = f * s.dot(h);
    if !(0.0..=1.0).contains(&u) {
        return None;
    }

    let q = s.cross(edge1);
    let v = f * ray.direction.dot(q);
    if v < 0.0 || u + v > 1.0 {
        return None;
    }

    let t = f * edge2.dot(q);
    if t >= 0.0 { Some(t) } else { None }
}

/// Intersect a ray with an ellipsoid defined by three radii.
///
/// Returns `Some((t_near, t_far))` with both parametric distances, or `None` if
/// the ray misses.
pub fn ray_ellipsoid(ray: &Ray, radii: DVec3) -> Option<DVec2> {
    if radii.x == 0.0 || radii.y == 0.0 || radii.z == 0.0 {
        return None;
    }

    let inv_radii = DVec3::new(1.0 / radii.x, 1.0 / radii.y, 1.0 / radii.z);
    let q = inv_radii * ray.origin;
    let w = inv_radii * ray.direction;

    let q2 = q.length_squared();
    let qw = q.dot(w);

    if q2 > 1.0 {
        // Outside ellipsoid
        if qw >= 0.0 {
            return None; // Looking outward
        }
        let qw2 = qw * qw;
        let difference = q2 - 1.0;
        let w2 = w.length_squared();
        let product = w2 * difference;

        if qw2 < product {
            return None; // Imaginary roots
        }
        if qw2 > product {
            let discriminant = qw * qw - product;
            let temp = -qw + discriminant.sqrt();
            let root0 = temp / w2;
            let root1 = difference / temp;
            if root0 < root1 {
                return Some(DVec2::new(root0, root1));
            }
            return Some(DVec2::new(root1, root0));
        }
        // Repeated roots
        let root = (difference / w2).sqrt();
        return Some(DVec2::new(root, root));
    }

    if q2 < 1.0 {
        // Inside ellipsoid
        let difference = q2 - 1.0;
        let w2 = w.length_squared();
        let product = w2 * difference;
        let discriminant = qw * qw - product;
        let temp = -qw + discriminant.sqrt();
        return Some(DVec2::new(0.0, temp / w2));
    }

    // On the ellipsoid surface
    if qw < 0.0 {
        let w2 = w.length_squared();
        return Some(DVec2::new(0.0, -qw / w2));
    }
    None
}

/// Test if a 2D point is inside a triangle.
///
/// Works regardless of winding order.
pub fn point_in_triangle_2d(point: DVec2, a: DVec2, b: DVec2, c: DVec2) -> bool {
    let ab = b - a;
    let bc = c - b;
    let ca = a - c;

    let ab_perp = DVec2::new(-ab.y, ab.x);
    let bc_perp = DVec2::new(-bc.y, bc.x);
    let ca_perp = DVec2::new(-ca.y, ca.x);

    let av = point - a;
    let cv = point - c;

    let v_proj_ab = av.dot(ab_perp);
    let v_proj_bc = cv.dot(bc_perp);
    let v_proj_ca = cv.dot(ca_perp);

    (v_proj_ab >= 0.0 && v_proj_ca >= 0.0 && v_proj_bc >= 0.0)
        || (v_proj_ab <= 0.0 && v_proj_ca <= 0.0 && v_proj_bc <= 0.0)
}

/// Test if a 3D point is inside a triangle, returning barycentric coordinates.
///
/// Returns `Some((u, v, w))` barycentric coordinates if the point is inside,
/// or `None` if outside.
pub fn point_in_triangle_3d(point: DVec3, a: DVec3, b: DVec3, c: DVec3) -> Option<DVec3> {
    let ab = b - a;
    let bc = c - b;

    let triangle_normal = ab.cross(bc);
    let length_sq = triangle_normal.length_squared();
    if length_sq < 1e-8 {
        return None; // Degenerate triangle
    }

    let triangle_area_inv = 1.0 / length_sq.sqrt();

    let ap = point - a;
    let abp_ratio = ab.cross(ap).length() * triangle_area_inv;
    if abp_ratio > 1.0 {
        return None;
    }

    let bp = point - b;
    let bcp_ratio = bc.cross(bp).length() * triangle_area_inv;
    if bcp_ratio > 1.0 {
        return None;
    }

    let cap_ratio = 1.0 - abp_ratio - bcp_ratio;
    if cap_ratio < 0.0 {
        return None;
    }

    Some(DVec3::new(bcp_ratio, cap_ratio, abp_ratio))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_hits_plane() {
        let ray = Ray::new(DVec3::new(0.0, 5.0, 0.0), DVec3::new(0.0, -1.0, 0.0));
        let plane = Plane::from_point_normal(DVec3::ZERO, DVec3::Y);
        let t = ray_plane(&ray, &plane).unwrap();
        assert!((t - 5.0).abs() < 1e-12);
    }

    #[test]
    fn ray_parallel_to_plane() {
        let ray = Ray::new(DVec3::new(0.0, 5.0, 0.0), DVec3::X);
        let plane = Plane::from_point_normal(DVec3::ZERO, DVec3::Y);
        assert!(ray_plane(&ray, &plane).is_none());
    }

    #[test]
    fn ray_hits_sphere() {
        let ray = Ray::new(DVec3::new(0.0, 0.0, -10.0), DVec3::Z);
        let sphere = BoundingSphere::new(DVec3::ZERO, 1.0);
        let t = ray_sphere(&ray, &sphere).unwrap();
        assert!((t - 9.0).abs() < 1e-12);
    }

    #[test]
    fn ray_misses_sphere() {
        let ray = Ray::new(DVec3::new(0.0, 10.0, -10.0), DVec3::Z);
        let sphere = BoundingSphere::new(DVec3::ZERO, 1.0);
        assert!(ray_sphere(&ray, &sphere).is_none());
    }

    #[test]
    fn ray_inside_sphere() {
        let ray = Ray::new(DVec3::ZERO, DVec3::X);
        let sphere = BoundingSphere::new(DVec3::ZERO, 5.0);
        let t = ray_sphere(&ray, &sphere).unwrap();
        assert!((t - 5.0).abs() < 1e-12);
    }

    #[test]
    fn ray_hits_aabb() {
        let ray = Ray::new(DVec3::new(-5.0, 0.5, 0.5), DVec3::X);
        let aabb = AxisAlignedBoundingBox::new(DVec3::ZERO, DVec3::ONE);
        let t = ray_aabb(&ray, &aabb).unwrap();
        assert!((t - 5.0).abs() < 1e-12);
    }

    #[test]
    fn ray_misses_aabb() {
        let ray = Ray::new(DVec3::new(-5.0, 5.0, 0.5), DVec3::X);
        let aabb = AxisAlignedBoundingBox::new(DVec3::ZERO, DVec3::ONE);
        assert!(ray_aabb(&ray, &aabb).is_none());
    }

    #[test]
    fn ray_hits_obb_identity() {
        let ray = Ray::new(DVec3::new(-5.0, 0.0, 0.0), DVec3::X);
        let obb =
            OrientedBoundingBox::from_i3s([0.0, 0.0, 0.0], [1.0, 1.0, 1.0], [0.0, 0.0, 0.0, 1.0]);
        let t = ray_obb(&ray, &obb).unwrap();
        assert!((t - 4.0).abs() < 1e-10);
    }

    #[test]
    fn ray_misses_obb() {
        let ray = Ray::new(DVec3::new(-5.0, 10.0, 0.0), DVec3::X);
        let obb =
            OrientedBoundingBox::from_i3s([0.0, 0.0, 0.0], [1.0, 1.0, 1.0], [0.0, 0.0, 0.0, 1.0]);
        assert!(ray_obb(&ray, &obb).is_none());
    }

    #[test]
    fn ray_hits_triangle() {
        let ray = Ray::new(DVec3::new(0.25, 0.25, -5.0), DVec3::Z);
        let v0 = DVec3::new(0.0, 0.0, 0.0);
        let v1 = DVec3::new(1.0, 0.0, 0.0);
        let v2 = DVec3::new(0.0, 1.0, 0.0);
        let t = ray_triangle(&ray, v0, v1, v2).unwrap();
        assert!((t - 5.0).abs() < 1e-12);
    }

    #[test]
    fn ray_misses_triangle() {
        let ray = Ray::new(DVec3::new(2.0, 2.0, -5.0), DVec3::Z);
        let v0 = DVec3::new(0.0, 0.0, 0.0);
        let v1 = DVec3::new(1.0, 0.0, 0.0);
        let v2 = DVec3::new(0.0, 1.0, 0.0);
        assert!(ray_triangle(&ray, v0, v1, v2).is_none());
    }
}
