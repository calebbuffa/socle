use glam::{DMat3, DVec2, DVec3};

use crate::intersection::obb_distance_half_axes;
use crate::polygon::{point_in_polygon_2d, polygon_boundary_distance_2d};

pub struct Sphere {
    pub center: DVec3,
    pub radius: f64,
}

impl Sphere {
    pub fn new(center: DVec3, radius: f64) -> Self {
        Self { center, radius }
    }
    pub fn distance_to_point(&self, point: DVec3) -> f64 {
        (point.distance(self.center) - self.radius).max(0.0)
    }
    pub fn contains_point(&self, point: DVec3) -> bool {
        self.center.distance(point) <= self.radius
    }
    pub fn is_entirely_clipped(&self, normal: DVec3, plane_distance: f64) -> bool {
        normal.dot(self.center) + self.radius + plane_distance < 0.0
    }
    pub fn is_over_footprint(&self, point: DVec3) -> bool {
        let dx = point.x - self.center.x;
        let dz = point.z - self.center.z;
        (dx * dx + dz * dz).sqrt() <= self.radius
    }
    pub fn ray_intersect(&self, origin: DVec3, direction: DVec3) -> Option<f64> {
        ray_vs_sphere(origin, direction, self.center, self.radius)
    }
}
impl From<(DVec3, f64)> for Sphere {
    fn from((center, radius): (DVec3, f64)) -> Self {
        Self { center, radius }
    }
}

impl From<SpatialBounds> for Sphere {
    fn from(bounds: SpatialBounds) -> Self {
        bounds.sphere().into()
    }
}

impl Default for Sphere {
    fn default() -> Self {
        Self {
            center: DVec3::ZERO,
            radius: 0.0,
        }
    }
}

pub struct Polygon {
    pub vertices: Vec<DVec2>,
}

pub struct AxisAlignedBox {
    pub min: DVec3,
    pub max: DVec3,
}

pub struct OrientedBox {
    pub center: DVec3,
    pub half_axes: DMat3,
}

/// Spatial extent of a node.
#[derive(Clone, Debug, PartialEq)]
pub enum SpatialBounds {
    /// 2D rectangle (lon/lat or projected).
    Rectangle { min: DVec2, max: DVec2 },
    /// 2D polygon counterclockwise
    Polygon { vertices: Vec<DVec2> },
    /// Axis-aligned box in 3D.
    AxisAlignedBox { min: DVec3, max: DVec3 },
    /// Bounding sphere.
    Sphere { center: DVec3, radius: f64 },
    /// Oriented bounding box.
    OrientedBox { center: DVec3, half_axes: DMat3 },
}

impl SpatialBounds {
    /// Compute the non-negative distance from `point` to the nearest surface of
    /// this bounding volume.  Returns `0.0` when the point is inside.
    pub fn distance_to_point(&self, point: DVec3) -> f64 {
        match self {
            SpatialBounds::Sphere { center, radius } => (point.distance(*center) - radius).max(0.0),
            SpatialBounds::AxisAlignedBox { min, max } => {
                let ex = (min.x - point.x).max(point.x - max.x).max(0.0);
                let ey = (min.y - point.y).max(point.y - max.y).max(0.0);
                let ez = (min.z - point.z).max(point.z - max.z).max(0.0);
                (ex * ex + ey * ey + ez * ez).sqrt()
            }
            SpatialBounds::OrientedBox { center, half_axes } => {
                obb_distance_half_axes(point, *center, *half_axes)
            }
            SpatialBounds::Rectangle { min, max } => {
                let ex = (min.x - point.x).max(point.x - max.x).max(0.0);
                let ey = (min.y - point.y).max(point.y - max.y).max(0.0);
                (ex * ex + ey * ey).sqrt()
            }
            SpatialBounds::Polygon { vertices } => {
                let p2 = DVec2::new(point.x, point.y);
                if point_in_polygon_2d(p2, vertices) {
                    0.0
                } else {
                    polygon_boundary_distance_2d(p2, vertices)
                }
            }
        }
    }

    /// Returns `true` if `point` is strictly inside (or on the boundary of) this volume.
    pub fn contains_point(&self, point: DVec3) -> bool {
        match self {
            SpatialBounds::Sphere { center, radius } => point.distance(*center) <= *radius,
            SpatialBounds::AxisAlignedBox { min, max } => {
                point.x >= min.x
                    && point.x <= max.x
                    && point.y >= min.y
                    && point.y <= max.y
                    && point.z >= min.z
                    && point.z <= max.z
            }
            SpatialBounds::OrientedBox { center, half_axes } => {
                obb_distance_half_axes(point, *center, *half_axes) <= 0.0
            }
            SpatialBounds::Rectangle { min, max } => {
                point.x >= min.x as f64
                    && point.x <= max.x as f64
                    && point.y >= min.y as f64
                    && point.y <= max.y as f64
            }
            SpatialBounds::Polygon { vertices } => {
                let p2 = DVec2::new(point.x, point.y);
                point_in_polygon_2d(p2, vertices)
            }
        }
    }

    /// Returns `true` if this bounding volume lies entirely on the clipped
    /// (negative) side of the half-space defined by `(normal, plane_distance)`.
    ///
    /// Every point `p` in the volume satisfying `normal · p + plane_distance < 0`
    /// means the whole volume is clipped.  Used by the clipping-plane traversal
    /// filter.
    pub fn is_entirely_clipped(&self, normal: DVec3, plane_distance: f64) -> bool {
        let support_dot = bounds_support_dot(self, normal);
        support_dot + plane_distance < 0.0
    }

    /// Returns `true` if the horizontal projection of `point` falls within this
    /// bounding volume's footprint.
    ///
    /// For 3D volumes the horizontal plane is XZ (Y-up assumed).  For 2D volumes
    /// (`Rectangle`, `Polygon`) the full 2D extent is used.
    ///
    /// Used to include terrain nodes below the camera even when outside the view
    /// frustum.
    pub fn is_over_footprint(&self, point: DVec3) -> bool {
        match self {
            SpatialBounds::Sphere { center, radius } => {
                let dx = point.x - center.x;
                let dz = point.z - center.z;
                (dx * dx + dz * dz).sqrt() <= *radius
            }
            SpatialBounds::AxisAlignedBox { min, max } => {
                point.x >= min.x && point.x <= max.x && point.z >= min.z && point.z <= max.z
            }
            SpatialBounds::OrientedBox { center, half_axes } => {
                let d = point - *center;
                for col in [half_axes.x_axis, half_axes.z_axis] {
                    let len = col.length();
                    if len < f64::EPSILON {
                        continue;
                    }
                    if d.dot(col / len).abs() > len {
                        return false;
                    }
                }
                true
            }
            SpatialBounds::Rectangle { min, max } => {
                point.x >= min.x as f64
                    && point.x <= max.x as f64
                    && point.y >= min.y as f64
                    && point.y <= max.y as f64
            }
            SpatialBounds::Polygon { vertices } => {
                point_in_polygon_2d(DVec2::new(point.x, point.z), vertices)
            }
        }
    }

    /// Test a ray against this bounding volume, returning the distance `t ≥ 0`
    /// to the first intersection, or `None` if the ray misses or the intersection
    /// is behind the origin.
    ///
    /// `direction` need not be normalised; `t` is in the same units as
    /// `direction`'s magnitude.
    pub fn ray_intersect(&self, origin: DVec3, direction: DVec3) -> Option<f64> {
        match self {
            SpatialBounds::Sphere { center, radius } => {
                ray_vs_sphere(origin, direction, *center, *radius)
            }
            SpatialBounds::AxisAlignedBox { min, max } => {
                ray_vs_aabb(origin, direction, *min, *max)
            }
            SpatialBounds::OrientedBox { center, half_axes } => {
                ray_vs_obb(origin, direction, *center, *half_axes)
            }
            SpatialBounds::Rectangle { min, max } => {
                let min3 = DVec3::new(min.x, min.y, -f64::EPSILON);
                let max3 = DVec3::new(max.x, max.y, f64::EPSILON);
                ray_vs_aabb(origin, direction, min3, max3)
            }
            SpatialBounds::Polygon { vertices } => ray_vs_polygon_2d(origin, direction, vertices),
        }
    }

    pub fn sphere(&self) -> SpatialBounds {
        match self {
            SpatialBounds::Sphere { center, radius } => SpatialBounds::Sphere {
                center: *center,
                radius: *radius,
            },
            SpatialBounds::AxisAlignedBox { min, max } => {
                let center = (*min + *max) * 0.5;
                let radius = center.distance(*max);
                SpatialBounds::Sphere { center, radius }
            }
            SpatialBounds::OrientedBox { center, half_axes } => {
                let radius = half_axes
                    .x_axis
                    .length()
                    .max(half_axes.y_axis.length())
                    .max(half_axes.z_axis.length());
                SpatialBounds::Sphere {
                    center: *center,
                    radius,
                }
            }
            SpatialBounds::Rectangle { min, max } => {
                let center = (*min + *max) * 0.5;
                let radius = center.distance(DVec2::new(max.x, max.y));
                SpatialBounds::Sphere {
                    center: center.extend(0.0),
                    radius,
                }
            }
            SpatialBounds::Polygon { vertices } => {
                // Simple approach: bounding box of the polygon.
                let min = vertices
                    .iter()
                    .fold(DVec2::splat(f64::INFINITY), |acc, v| acc.min(*v));
                let max = vertices
                    .iter()
                    .fold(DVec2::splat(f64::NEG_INFINITY), |acc, v| acc.max(*v));
                let center = (min + max) * 0.5;
                let radius = vertices
                    .iter()
                    .map(|v| v.distance(center))
                    .fold(0.0, f64::max);
                SpatialBounds::Sphere {
                    center: center.extend(0.0),
                    radius,
                }
            }
        }
    }
}

// ── Private geometric helpers ─────────────────────────────────────────────────

/// Compute `max_{p ∈ bounds} (normal · p)` — the support of `bounds` in direction
/// `normal`.  Used by [`SpatialBounds::is_entirely_clipped`].
fn bounds_support_dot(bounds: &SpatialBounds, normal: DVec3) -> f64 {
    match bounds {
        SpatialBounds::Sphere { center, radius } => normal.dot(*center) + radius,
        SpatialBounds::AxisAlignedBox { min, max } => {
            let cx = if normal.x >= 0.0 { max.x } else { min.x };
            let cy = if normal.y >= 0.0 { max.y } else { min.y };
            let cz = if normal.z >= 0.0 { max.z } else { min.z };
            normal.dot(DVec3::new(cx, cy, cz))
        }
        SpatialBounds::OrientedBox { center, half_axes } => {
            normal.dot(*center)
                + normal.dot(half_axes.x_axis).abs()
                + normal.dot(half_axes.y_axis).abs()
                + normal.dot(half_axes.z_axis).abs()
        }
        SpatialBounds::Rectangle { min, max } => {
            let cx = if normal.x >= 0.0 {
                max.x as f64
            } else {
                min.x as f64
            };
            let cy = if normal.y >= 0.0 {
                max.y as f64
            } else {
                min.y as f64
            };
            normal.x * cx + normal.y * cy
        }
        SpatialBounds::Polygon { vertices } => vertices
            .iter()
            .map(|v| normal.x * v.x + normal.y * v.y)
            .fold(f64::NEG_INFINITY, f64::max),
    }
}

#[inline]
fn ray_vs_sphere(origin: DVec3, dir: DVec3, center: DVec3, radius: f64) -> Option<f64> {
    let oc = origin - center;
    let a = dir.dot(dir);
    let b = 2.0 * oc.dot(dir);
    let c = oc.dot(oc) - radius * radius;
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return None;
    }
    let sqrt_disc = disc.sqrt();
    let t0 = (-b - sqrt_disc) / (2.0 * a);
    let t1 = (-b + sqrt_disc) / (2.0 * a);
    if t1 < 0.0 {
        return None;
    }
    Some(if t0 >= 0.0 { t0 } else { t1 })
}

#[inline]
fn ray_vs_aabb(origin: DVec3, dir: DVec3, min: DVec3, max: DVec3) -> Option<f64> {
    let inv = DVec3::new(
        if dir.x.abs() > f64::EPSILON {
            1.0 / dir.x
        } else {
            f64::INFINITY
        },
        if dir.y.abs() > f64::EPSILON {
            1.0 / dir.y
        } else {
            f64::INFINITY
        },
        if dir.z.abs() > f64::EPSILON {
            1.0 / dir.z
        } else {
            f64::INFINITY
        },
    );
    let t1 = (min - origin) * inv;
    let t2 = (max - origin) * inv;
    let t_min = t1.min(t2);
    let t_max = t1.max(t2);
    let t_enter = t_min.x.max(t_min.y).max(t_min.z);
    let t_exit = t_max.x.min(t_max.y).min(t_max.z);
    if t_exit < 0.0 || t_enter > t_exit {
        return None;
    }
    Some(t_enter.max(0.0))
}

#[inline]
fn ray_vs_obb(origin: DVec3, dir: DVec3, center: DVec3, half_axes: glam::DMat3) -> Option<f64> {
    let d = origin - center;
    let mut t_min = f64::NEG_INFINITY;
    let mut t_max = f64::INFINITY;
    for col in [half_axes.x_axis, half_axes.y_axis, half_axes.z_axis] {
        let len = col.length();
        if len < f64::EPSILON {
            continue;
        }
        let axis = col / len;
        let e = axis.dot(d);
        let f = axis.dot(dir);
        if f.abs() > f64::EPSILON {
            let t1 = (-e - len) / f;
            let t2 = (-e + len) / f;
            let (t1, t2) = if t1 > t2 { (t2, t1) } else { (t1, t2) };
            t_min = t_min.max(t1);
            t_max = t_max.min(t2);
            if t_max < t_min {
                return None;
            }
        } else if (-e - len) > 0.0 || (-e + len) < 0.0 {
            return None;
        }
    }
    if t_max < 0.0 {
        return None;
    }
    Some(t_min.max(0.0))
}

fn ray_vs_polygon_2d(origin: DVec3, dir: DVec3, verts: &[DVec2]) -> Option<f64> {
    if verts.len() < 3 {
        return None;
    }
    let min2 = verts.iter().fold(DVec2::splat(f64::MAX), |a, &v| a.min(v));
    let max2 = verts.iter().fold(DVec2::splat(f64::MIN), |a, &v| a.max(v));
    let t = ray_vs_aabb(
        origin,
        dir,
        DVec3::new(min2.x, min2.y, -f64::EPSILON),
        DVec3::new(max2.x, max2.y, f64::EPSILON),
    )?;
    let hit = origin + t * dir;
    if point_in_polygon_2d(DVec2::new(hit.x, hit.y), verts) {
        Some(t)
    } else {
        None
    }
}
