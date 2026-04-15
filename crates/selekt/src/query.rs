//! Spatial query types for [`SelectionEngine::query`].

use glam::{DVec2, DVec3};
use zukei::SpatialBounds;

/// Shape used to spatially query the node hierarchy.
///
/// All coordinates must be in the same CRS as the engine's [`SpatialHierarchy`]
/// and [`ViewState`]. Format-specific adapter crates typically provide helpers
/// to build a `QueryShape` from geodetic inputs (e.g. a lon/lat polygon).
///
/// [`SpatialHierarchy`]: crate::SpatialHierarchy
/// [`ViewState`]: crate::ViewState
#[derive(Clone, Debug)]
pub enum QueryShape {
    /// A convex volume defined by a set of half-space planes.
    ///
    /// A point `p` is **inside** when `n · p + d ≥ 0` for every plane `(n, d)`.
    /// Planes should use **inward-facing normals** (pointing toward the interior).
    ///
    /// This is the most general 3D query shape. Geodetic polygon queries can be
    /// converted to a `ConvexVolume` by lifting each polygon edge into an ECEF
    /// half-space plane.
    ConvexVolume { planes: Vec<(DVec3, f64)> },

    /// Axis-aligned bounding box query (3D).
    Aabb { min: DVec3, max: DVec3 },

    /// 2D polygon query for hierarchies whose bounds use 2D variants
    /// (`Rectangle` or `Polygon`).
    ///
    /// Vertices are counterclockwise in the hierarchy's 2D coordinate space.
    Polygon { vertices: Vec<DVec2> },
}

/// Controls how deep `SelectionEngine::query` descends the hierarchy.
#[derive(Clone, Copy, Debug)]
pub enum QueryDepth {
    /// Traverse all the way to leaves. May return a large number of nodes on
    /// deep hierarchies — consider using `Level` as a safety cap.
    All,

    /// Descend at most `n` levels from the root, where `1` means the root's
    /// direct children. Gracefully returns the deepest available nodes if the
    /// hierarchy is shallower than `n`.
    Level(u32),
}

/// Returns `true` if `shape` intersects (or contains) `bounds`.
///
/// Conservative: mismatched dimensionality (e.g. a 3D `ConvexVolume` against a
/// 2D `Rectangle` bound) returns `true` to avoid incorrect pruning.
pub(crate) fn shape_intersects_bounds(shape: &QueryShape, bounds: &SpatialBounds) -> bool {
    match shape {
        QueryShape::ConvexVolume { planes } => convex_vs_bounds(planes, bounds),
        QueryShape::Aabb { min, max } => aabb_vs_bounds(*min, *max, bounds),
        QueryShape::Polygon { vertices } => polygon_vs_bounds(vertices, bounds),
    }
}

fn convex_vs_bounds(planes: &[(DVec3, f64)], bounds: &SpatialBounds) -> bool {
    if matches!(bounds, SpatialBounds::Empty) {
        return false;
    }
    for &(n, d) in planes {
        // If the support point of `bounds` along `-n` is outside the plane,
        // the entire bounds is outside → prune.
        let support = support_along(bounds, -n);
        if n.dot(support) + d < 0.0 {
            return false;
        }
    }
    true
}

/// Returns the point on (or inside) `bounds` that is furthest in direction `dir`.
fn support_along(bounds: &SpatialBounds, dir: DVec3) -> DVec3 {
    match bounds {
        SpatialBounds::Sphere { center, radius } => {
            let len = dir.length();
            if len < f64::EPSILON {
                *center
            } else {
                *center + (dir / len) * radius
            }
        }
        SpatialBounds::AxisAlignedBox { min, max } => DVec3::new(
            if dir.x >= 0.0 { max.x } else { min.x },
            if dir.y >= 0.0 { max.y } else { min.y },
            if dir.z >= 0.0 { max.z } else { min.z },
        ),
        SpatialBounds::OrientedBox { center, half_axes } => {
            let mut result = *center;
            for col in [half_axes.x_axis, half_axes.y_axis, half_axes.z_axis] {
                result += if dir.dot(col) >= 0.0 { col } else { -col };
            }
            result
        }
        // 2D bounds: conservative — return a point that will pass the plane test.
        SpatialBounds::Rectangle { min, max } => DVec3::new(
            if dir.x >= 0.0 { max.x } else { min.x },
            if dir.y >= 0.0 { max.y } else { min.y },
            0.0,
        ),
        SpatialBounds::Polygon { vertices } => {
            // Pick the vertex furthest along dir in XY.
            vertices
                .iter()
                .map(|v| DVec3::new(v.x, v.y, 0.0))
                .max_by(|a, b| {
                    a.dot(dir)
                        .partial_cmp(&b.dot(dir))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap_or(DVec3::ZERO)
        }
        SpatialBounds::Empty => DVec3::ZERO,
    }
}

fn aabb_vs_bounds(qmin: DVec3, qmax: DVec3, bounds: &SpatialBounds) -> bool {
    match bounds {
        SpatialBounds::Sphere { center, radius } => {
            // AABB-sphere: clamp center to AABB, check distance ≤ radius.
            let cx = center.x.clamp(qmin.x, qmax.x);
            let cy = center.y.clamp(qmin.y, qmax.y);
            let cz = center.z.clamp(qmin.z, qmax.z);
            let dx = center.x - cx;
            let dy = center.y - cy;
            let dz = center.z - cz;
            dx * dx + dy * dy + dz * dz <= radius * radius
        }
        SpatialBounds::AxisAlignedBox { min, max } => {
            // AABB-AABB: no overlap if separated on any axis.
            qmin.x <= max.x
                && qmax.x >= min.x
                && qmin.y <= max.y
                && qmax.y >= min.y
                && qmin.z <= max.z
                && qmax.z >= min.z
        }
        SpatialBounds::OrientedBox { center, half_axes } => {
            // Use ConvexVolume SAT via support vectors — build 6 planes from AABB faces.
            let planes = aabb_to_planes(qmin, qmax);
            convex_vs_bounds(
                &planes,
                &SpatialBounds::OrientedBox {
                    center: *center,
                    half_axes: *half_axes,
                },
            )
        }
        // 2D: conservative — project to XY and do 2D AABB test.
        SpatialBounds::Rectangle { min, max } => {
            qmin.x <= max.x as f64
                && qmax.x >= min.x as f64
                && qmin.y <= max.y as f64
                && qmax.y >= min.y as f64
        }
        SpatialBounds::Polygon { vertices } => {
            if vertices.is_empty() {
                return false;
            }
            let pmin = vertices
                .iter()
                .fold(DVec2::splat(f64::MAX), |a, &v| a.min(v));
            let pmax = vertices
                .iter()
                .fold(DVec2::splat(f64::MIN), |a, &v| a.max(v));
            qmin.x <= pmax.x && qmax.x >= pmin.x && qmin.y <= pmax.y && qmax.y >= pmin.y
        }
        SpatialBounds::Empty => false,
    }
}

fn aabb_to_planes(min: DVec3, max: DVec3) -> Vec<(DVec3, f64)> {
    vec![
        (DVec3::X, max.x),
        (-DVec3::X, -min.x),
        (DVec3::Y, max.y),
        (-DVec3::Y, -min.y),
        (DVec3::Z, max.z),
        (-DVec3::Z, -min.z),
    ]
}

fn polygon_vs_bounds(poly: &[DVec2], bounds: &SpatialBounds) -> bool {
    if poly.len() < 3 {
        return false;
    }

    // 2D AABB of the polygon.
    let pmin = poly.iter().fold(DVec2::splat(f64::MAX), |a, &v| a.min(v));
    let pmax = poly.iter().fold(DVec2::splat(f64::MIN), |a, &v| a.max(v));

    // Project bounds to a 2D AABB in XY (or XZ for 3D bounds).
    let (bmin2, bmax2) = match bounds {
        SpatialBounds::Rectangle { min, max } => {
            (DVec2::new(min.x, min.y), DVec2::new(max.x, max.y))
        }
        SpatialBounds::Polygon { vertices } => {
            if vertices.is_empty() {
                return false;
            }
            (
                vertices
                    .iter()
                    .fold(DVec2::splat(f64::MAX), |a, &v| a.min(v)),
                vertices
                    .iter()
                    .fold(DVec2::splat(f64::MIN), |a, &v| a.max(v)),
            )
        }
        // 3D bounds projected to XZ for horizontal polygon queries — conservative.
        SpatialBounds::Sphere { center, radius } => (
            DVec2::new(center.x - radius, center.z - radius),
            DVec2::new(center.x + radius, center.z + radius),
        ),
        SpatialBounds::AxisAlignedBox { min, max } => {
            (DVec2::new(min.x, min.z), DVec2::new(max.x, max.z))
        }
        SpatialBounds::OrientedBox { center, half_axes } => {
            let rx = half_axes.x_axis.x.abs() + half_axes.y_axis.x.abs() + half_axes.z_axis.x.abs();
            let rz = half_axes.x_axis.z.abs() + half_axes.y_axis.z.abs() + half_axes.z_axis.z.abs();
            (
                DVec2::new(center.x - rx, center.z - rz),
                DVec2::new(center.x + rx, center.z + rz),
            )
        }
        SpatialBounds::Empty => return false,
    };

    // Quick AABB reject.
    if pmin.x > bmax2.x || pmax.x < bmin2.x || pmin.y > bmax2.y || pmax.y < bmin2.y {
        return false;
    }

    // SAT on polygon edges: for each edge (a→b), check if bounds AABB is entirely
    // on the outside half-plane. If so, they are separated.
    let n = poly.len();
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        // Edge normal (outward for CCW polygon = right-perpendicular of edge dir).
        let edge = b - a;
        let normal = DVec2::new(edge.y, -edge.x); // outward normal
        // Project bounds AABB corners onto normal; take min (furthest "outside").
        let corners = [
            DVec2::new(bmin2.x, bmin2.y),
            DVec2::new(bmax2.x, bmin2.y),
            DVec2::new(bmax2.x, bmax2.y),
            DVec2::new(bmin2.x, bmax2.y),
        ];
        let d_ref = normal.dot(a);
        let all_outside = corners.iter().all(|&c| normal.dot(c) > d_ref);
        if all_outside {
            return false;
        }
    }
    true
}
