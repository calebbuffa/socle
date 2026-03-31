//! Geodetic position: longitude, latitude (radians), height above ellipsoid (metres).

use std::f64::consts::PI;

use glam::DVec2;

use crate::GlobeRectangle;

/// A geodetic position expressed as longitude, latitude, and height.
///
/// Angles are stored in **radians** (`[-π, π]` for longitude, `[-π/2, π/2]`
/// for latitude). Height is in **metres** above the reference ellipsoid
/// surface. Negative heights are below the surface.
///
/// Use [`Cartographic::from_degrees`] when working with human-readable degree values.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Cartographic {
    /// Longitude in radians, range `[-π, π]`.
    pub longitude: f64,
    /// Latitude in radians, range `[-π/2, π/2]`.
    pub latitude: f64,
    /// Height above the ellipsoid surface, in metres.
    pub height: f64,
}

impl Cartographic {
    /// Create from longitude and latitude in **radians**, height in metres.
    #[inline]
    pub const fn new(longitude: f64, latitude: f64, height: f64) -> Self {
        Self {
            longitude,
            latitude,
            height,
        }
    }

    /// Create from longitude and latitude in **degrees**, height in metres.
    #[inline]
    pub fn from_degrees(lon_deg: f64, lat_deg: f64, height: f64) -> Self {
        Self {
            longitude: lon_deg.to_radians(),
            latitude: lat_deg.to_radians(),
            height,
        }
    }

    /// Return `(longitude_deg, latitude_deg, height_m)`.
    #[inline]
    pub fn to_degrees(self) -> (f64, f64, f64) {
        (
            self.longitude.to_degrees(),
            self.latitude.to_degrees(),
            self.height,
        )
    }

    /// Zero position: prime meridian, equator, sea level.
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);

    /// Maximum valid position: `(+180°, +90°, 0m)`.
    pub const MAX: Self = Self::new(PI, PI / 2.0, 0.0);

    /// Check that longitude is in `[-π, π]` and latitude in `[-π/2, π/2]`.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.longitude.abs() <= PI && self.latitude.abs() <= PI / 2.0
    }

    /// Clamp longitude to `[-π, π]` and latitude to `[-π/2, π/2]`.
    pub fn clamped(self) -> Self {
        Self {
            longitude: self.longitude.clamp(-PI, PI),
            latitude: self.latitude.clamp(-PI / 2.0, PI / 2.0),
            height: self.height,
        }
    }
}

impl Default for Cartographic {
    fn default() -> Self {
        Self::ZERO
    }
}

/// A 2-D polygon in longitude/latitude radians.
///
/// # Example
/// ```
/// # use terra::CartographicPolygon;
/// # use glam::DVec2;
/// // A tiny square near the prime meridian / equator:
/// let verts = vec![
///     DVec2::new(-0.01, -0.01),
///     DVec2::new( 0.01, -0.01),
///     DVec2::new( 0.01,  0.01),
///     DVec2::new(-0.01,  0.01),
/// ];
/// let poly = CartographicPolygon::new(verts);
/// assert_eq!(poly.indices().len(), 6); // 2 triangles × 3 indices
/// ```
#[derive(Debug, Clone)]
pub struct CartographicPolygon {
    vertices: Vec<DVec2>,
    indices: Vec<u32>,
    bounding_rectangle: Option<GlobeRectangle>,
}

impl CartographicPolygon {
    /// Build a polygon from a list of `(longitude_rad, latitude_rad)` vertices.
    ///
    /// Longitude is wrapped to `[−π, π]`; latitude is clamped to
    /// `[−π/2, π/2]`.  Degenerate polygons (fewer than 3 vertices) get an
    /// empty index list.
    pub fn new(vertices: Vec<DVec2>) -> Self {
        let vertices: Vec<DVec2> = vertices
            .into_iter()
            .map(|v| DVec2::new(wrap_longitude(v.x), v.y.clamp(-PI / 2.0, PI / 2.0)))
            .collect();

        let bounding_rectangle = compute_bounding_rectangle(&vertices);
        let indices = if vertices.len() >= 3 {
            ear_clip(&vertices)
        } else {
            Vec::new()
        };

        Self {
            vertices,
            indices,
            bounding_rectangle,
        }
    }

    /// The validated (wrapped/clamped) perimeter vertices.
    #[inline]
    pub fn vertices(&self) -> &[DVec2] {
        &self.vertices
    }

    /// Triangulated index list (triples of vertex indices).
    #[inline]
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    /// Tight bounding rectangle, or `None` for degenerate inputs.
    #[inline]
    pub fn bounding_rectangle(&self) -> Option<&GlobeRectangle> {
        self.bounding_rectangle.as_ref()
    }

    /// Test whether `point` (lon, lat radians) is inside this polygon using
    /// a winding-number algorithm.
    ///
    /// Points on the boundary are considered inside.
    pub fn contains_point(&self, point: DVec2) -> bool {
        point_in_polygon(&self.vertices, point)
    }

    /// Returns `true` if `rectangle` is **completely inside** any polygon in
    /// `polygons`.
    ///
    /// Checks all four corners of the rectangle; the rectangle is inside when
    /// every corner lies inside the same polygon.
    pub fn rectangle_is_within_polygons(
        rectangle: &GlobeRectangle,
        polygons: &[CartographicPolygon],
    ) -> bool {
        let corners = rectangle_corners(rectangle);
        polygons
            .iter()
            .any(|poly| corners.iter().all(|&c| poly.contains_point(c)))
    }

    /// Returns `true` if `rectangle` is **completely outside** all polygons in
    /// `polygons`.
    ///
    /// Uses bounding-rectangle pre-culling followed per-corner winding-number
    /// tests.
    pub fn rectangle_is_outside_polygons(
        rectangle: &GlobeRectangle,
        polygons: &[CartographicPolygon],
    ) -> bool {
        polygons.iter().all(|poly| {
            // If the bounding rectangles don't intersect, definitely outside.
            if let Some(br) = poly.bounding_rectangle() {
                if br.intersection(rectangle).is_none() {
                    return true;
                }
            }
            // At least one corner of the rectangle must be outside the polygon
            // AND no corner of the rectangle may be inside.
            let corners = rectangle_corners(rectangle);
            corners.iter().all(|&c| !poly.contains_point(c))
        })
    }
}

fn wrap_longitude(lon: f64) -> f64 {
    if lon >= -PI && lon <= PI {
        return lon;
    }
    // fmod-style wrap: result in (-2π, 2π), then shift.
    let rem = lon % (2.0 * PI);
    if rem < -PI {
        rem + 2.0 * PI
    } else if rem > PI {
        rem - 2.0 * PI
    } else {
        rem
    }
}

fn compute_bounding_rectangle(verts: &[DVec2]) -> Option<GlobeRectangle> {
    if verts.is_empty() {
        return None;
    }
    let mut west = verts[0].x;
    let mut east = verts[0].x;
    let mut south = verts[0].y;
    let mut north = verts[0].y;
    for v in &verts[1..] {
        if v.x < west {
            west = v.x;
        }
        if v.x > east {
            east = v.x;
        }
        if v.y < south {
            south = v.y;
        }
        if v.y > north {
            north = v.y;
        }
    }
    Some(GlobeRectangle::new(west, south, east, north))
}

fn rectangle_corners(r: &GlobeRectangle) -> [DVec2; 4] {
    [
        DVec2::new(r.west, r.south),
        DVec2::new(r.east, r.south),
        DVec2::new(r.east, r.north),
        DVec2::new(r.west, r.north),
    ]
}

/// Winding-number algorithm — handles non-convex polygons correctly.
/// Returns `true` when `point` is inside (or on the boundary of) `polygon`.
fn point_in_polygon(polygon: &[DVec2], point: DVec2) -> bool {
    let n = polygon.len();
    if n < 3 {
        return false;
    }
    let mut winding = 0i32;
    for i in 0..n {
        let a = polygon[i];
        let b = polygon[(i + 1) % n];
        if a.y <= point.y {
            if b.y > point.y {
                // Upward crossing — check if point is left of edge.
                if cross2d(b - a, point - a) > 0.0 {
                    winding += 1;
                }
            }
        } else if b.y <= point.y {
            // Downward crossing — check if point is right of edge.
            if cross2d(b - a, point - a) < 0.0 {
                winding -= 1;
            }
        }
    }
    winding != 0
}

#[inline]
fn cross2d(a: DVec2, b: DVec2) -> f64 {
    a.x * b.y - a.y * b.x
}

/// Simple O(n²) ear-clipping triangulation for simple (non-self-intersecting)
/// polygons.  Returns a flat list of triangle vertex indices.
fn ear_clip(polygon: &[DVec2]) -> Vec<u32> {
    let n = polygon.len();
    if n < 3 {
        return Vec::new();
    }
    if n == 3 {
        return vec![0, 1, 2];
    }

    // Determine polygon winding; we need CCW for the ear test.
    let signed_area: f64 = {
        let mut area = 0.0f64;
        for i in 0..n {
            let a = polygon[i];
            let b = polygon[(i + 1) % n];
            area += a.x * b.y - b.x * a.y;
        }
        area / 2.0
    };

    // Working index ring.
    let mut ring: Vec<usize> = (0..n).collect();
    // If CW, reverse so we work CCW.
    if signed_area < 0.0 {
        ring.reverse();
    }

    let mut indices = Vec::with_capacity((n - 2) * 3);
    let mut attempts = 0usize;
    let mut i = 0usize;

    while ring.len() > 3 {
        let len = ring.len();
        let prev = ring[(i + len - 1) % len];
        let curr = ring[i % len];
        let next = ring[(i + 1) % len];

        if is_ear(polygon, &ring, prev, curr, next) {
            indices.push(prev as u32);
            indices.push(curr as u32);
            indices.push(next as u32);
            ring.remove(i % len);
            attempts = 0;
        } else {
            i += 1;
            attempts += 1;
            if attempts > len {
                // Polygon is degenerate (self-intersecting).
                break;
            }
        }
    }

    if ring.len() == 3 {
        indices.push(ring[0] as u32);
        indices.push(ring[1] as u32);
        indices.push(ring[2] as u32);
    }

    indices
}

/// Returns `true` when vertex `curr` is an ear of the polygon reduced to `ring`.
fn is_ear(polygon: &[DVec2], ring: &[usize], prev: usize, curr: usize, next: usize) -> bool {
    let a = polygon[prev];
    let b = polygon[curr];
    let c = polygon[next];

    // The ear triangle must be CCW (positive cross product).
    if cross2d(b - a, c - a) <= 0.0 {
        return false;
    }

    // No other polygon vertex may lie inside this triangle.
    for &idx in ring {
        if idx == prev || idx == curr || idx == next {
            continue;
        }
        if point_in_triangle(polygon[idx], a, b, c) {
            return false;
        }
    }
    true
}

/// Returns true when `p` is strictly inside triangle `(a, b, c)` (CCW).
fn point_in_triangle(p: DVec2, a: DVec2, b: DVec2, c: DVec2) -> bool {
    let d1 = cross2d(b - a, p - a);
    let d2 = cross2d(c - b, p - b);
    let d3 = cross2d(a - c, p - c);
    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_neg && has_pos)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    fn square() -> CartographicPolygon {
        CartographicPolygon::new(vec![
            DVec2::new(-0.1, -0.1),
            DVec2::new(0.1, -0.1),
            DVec2::new(0.1, 0.1),
            DVec2::new(-0.1, 0.1),
        ])
    }

    fn triangle() -> CartographicPolygon {
        CartographicPolygon::new(vec![
            DVec2::new(-0.1, -0.1),
            DVec2::new(0.1, -0.1),
            DVec2::new(0.0, 0.1),
        ])
    }

    #[test]
    fn square_has_six_indices() {
        // 4-vertex polygon → 2 triangles → 6 indices.
        assert_eq!(square().indices().len(), 6);
    }

    #[test]
    fn triangle_has_three_indices() {
        assert_eq!(triangle().indices().len(), 3);
    }

    #[test]
    fn degenerate_has_no_indices() {
        let p = CartographicPolygon::new(vec![DVec2::ZERO, DVec2::X]);
        assert_eq!(p.indices().len(), 0);
    }

    #[test]
    fn bounding_rectangle_computed() {
        let br = square().bounding_rectangle().copied().unwrap();
        assert!((br.west - (-0.1)).abs() < 1e-12);
        assert!((br.east - 0.1).abs() < 1e-12);
        assert!((br.south - (-0.1)).abs() < 1e-12);
        assert!((br.north - 0.1).abs() < 1e-12);
    }

    #[test]
    fn longitude_wrapped() {
        // 4.0 rad ≈ 229° → wraps to ~4 - 2π ≈ -2.28 rad
        let poly = CartographicPolygon::new(vec![
            DVec2::new(4.0, 0.0),
            DVec2::new(5.0, 0.0),
            DVec2::new(4.5, 0.5),
        ]);
        for v in poly.vertices() {
            assert!(v.x >= -PI && v.x <= PI, "lon={} not wrapped", v.x);
        }
    }

    #[test]
    fn latitude_clamped() {
        let poly = CartographicPolygon::new(vec![
            DVec2::new(0.0, 2.0),
            DVec2::new(0.1, 2.0),
            DVec2::new(0.05, 1.9),
        ]);
        for v in poly.vertices() {
            assert!(
                v.y >= -FRAC_PI_2 && v.y <= FRAC_PI_2,
                "lat={} not clamped",
                v.y
            );
        }
    }

    #[test]
    fn center_is_inside_square() {
        assert!(square().contains_point(DVec2::ZERO));
    }

    #[test]
    fn outside_point_not_inside_square() {
        assert!(!square().contains_point(DVec2::new(1.0, 1.0)));
    }

    #[test]
    fn center_is_inside_triangle() {
        assert!(triangle().contains_point(DVec2::new(0.0, 0.0)));
    }

    #[test]
    fn outside_point_not_inside_triangle() {
        assert!(!triangle().contains_point(DVec2::new(0.5, 0.5)));
    }

    // ── rectangle_is_within_polygons ─────────────────────────────────────────

    #[test]
    fn tiny_rectangle_within_square() {
        let rect = GlobeRectangle::new(-0.01, -0.01, 0.01, 0.01);
        let polys = vec![square()];
        assert!(CartographicPolygon::rectangle_is_within_polygons(
            &rect, &polys
        ));
    }

    #[test]
    fn large_rectangle_not_within_square() {
        let rect = GlobeRectangle::new(-0.5, -0.5, 0.5, 0.5);
        let polys = vec![square()];
        assert!(!CartographicPolygon::rectangle_is_within_polygons(
            &rect, &polys
        ));
    }

    #[test]
    fn far_rectangle_is_outside_square() {
        let rect = GlobeRectangle::new(1.0, 1.0, 1.5, 1.5);
        let polys = vec![square()];
        assert!(CartographicPolygon::rectangle_is_outside_polygons(
            &rect, &polys
        ));
    }

    #[test]
    fn overlapping_rectangle_is_not_outside_square() {
        // Partially overlaps the square — the "outside" test should return false.
        let rect = GlobeRectangle::new(0.05, 0.05, 0.5, 0.5);
        let polys = vec![square()];
        assert!(!CartographicPolygon::rectangle_is_outside_polygons(
            &rect, &polys
        ));
    }

    #[test]
    fn empty_polygon_list_outside_is_true() {
        let rect = GlobeRectangle::new(0.0, 0.0, 0.1, 0.1);
        assert!(CartographicPolygon::rectangle_is_outside_polygons(
            &rect,
            &[]
        ));
    }

    #[test]
    fn empty_polygon_list_within_is_false() {
        let rect = GlobeRectangle::new(0.0, 0.0, 0.1, 0.1);
        assert!(!CartographicPolygon::rectangle_is_within_polygons(
            &rect,
            &[]
        ));
    }
}
