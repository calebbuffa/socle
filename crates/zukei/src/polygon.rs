//! 2D polygon geometry utilities.

use glam::DVec2;

/// Returns `true` if `p` is inside the 2D polygon using the winding-number rule.
///
/// Returns `false` for degenerate polygons with fewer than 3 vertices.
pub fn point_in_polygon_2d(p: DVec2, verts: &[DVec2]) -> bool {
    if verts.len() < 3 {
        return false;
    }
    let mut winding = 0i32;
    let n = verts.len();
    for i in 0..n {
        let a = verts[i];
        let b = verts[(i + 1) % n];
        if a.y <= p.y {
            if b.y > p.y && cross2(a, b, p) > 0.0 {
                winding += 1;
            }
        } else if b.y <= p.y && cross2(a, b, p) < 0.0 {
            winding -= 1;
        }
    }
    winding != 0
}

/// Minimum 2D distance from `p` to the nearest edge of a polygon.
///
/// Does not test whether `p` is inside; call [`point_in_polygon_2d`] first and
/// return `0.0` if it returns `true`.
pub fn polygon_boundary_distance_2d(p: DVec2, verts: &[DVec2]) -> f64 {
    let n = verts.len();
    if n == 0 {
        return f64::MAX;
    }
    let mut min_dist = f64::MAX;
    for i in 0..n {
        let a = verts[i];
        let b = verts[(i + 1) % n];
        min_dist = min_dist.min(point_to_segment_dist_2d(p, a, b));
    }
    min_dist
}

/// Shortest distance from point `p` to segment `[a, b]`.
#[inline]
pub fn point_to_segment_dist_2d(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq < f64::EPSILON {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    p.distance(a + t * ab)
}

/// 2D cross product of vectors (a→b) and (a→p).
#[inline]
pub fn cross2(a: DVec2, b: DVec2, p: DVec2) -> f64 {
    (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
}
