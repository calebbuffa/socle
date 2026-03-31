use std::f64::consts::PI;

use glam::{DMat3, DMat4};

use crate::{Cartographic, Ellipsoid, LocalHorizontalCoordinateSystem};

/// An object anchored to the globe at a specific ECEF transform.
///
/// # Example
/// ```
/// # use terra::{Ellipsoid, GlobeAnchor};
/// # use glam::DMat4;
/// let anchor = GlobeAnchor::from_anchor_to_fixed(DMat4::IDENTITY);
/// assert_eq!(anchor.anchor_to_fixed(), DMat4::IDENTITY);
/// ```
#[derive(Debug, Clone)]
pub struct GlobeAnchor {
    anchor_to_fixed: DMat4,
}

impl GlobeAnchor {
    /// Create a `GlobeAnchor` from an explicit `anchor → ECEF` matrix.
    pub fn from_anchor_to_fixed(anchor_to_fixed: DMat4) -> Self {
        Self { anchor_to_fixed }
    }

    /// Create a `GlobeAnchor` from an `anchor → local` matrix and the
    /// `LocalHorizontalCoordinateSystem` that defines the local space.
    pub fn from_anchor_to_local(
        local: &LocalHorizontalCoordinateSystem,
        anchor_to_local: DMat4,
    ) -> Self {
        let anchor_to_fixed = local.local_to_ecef_matrix() * anchor_to_local;
        Self { anchor_to_fixed }
    }

    /// The current `anchor → ECEF` transform.
    #[inline]
    pub fn anchor_to_fixed(&self) -> DMat4 {
        self.anchor_to_fixed
    }

    /// Update the `anchor → ECEF` transform.
    ///
    /// When `adjust_orientation` is `true`, the rotational part of the matrix
    /// is adjusted so the object remains upright at its new location on the
    /// globe (i.e., the local "up" direction is kept aligned with the geodetic
    /// surface normal).  Pass `false` if you are already accounting for globe
    /// curvature in the caller.
    pub fn set_anchor_to_fixed(
        &mut self,
        new_anchor_to_fixed: DMat4,
        adjust_orientation: bool,
        ellipsoid: &Ellipsoid,
    ) {
        if adjust_orientation {
            self.anchor_to_fixed = adjust_orientation_for_curvature(
                &self.anchor_to_fixed,
                new_anchor_to_fixed,
                ellipsoid,
            );
        } else {
            self.anchor_to_fixed = new_anchor_to_fixed;
        }
    }

    /// Compute the `anchor → local` matrix for the given coordinate system.
    pub fn anchor_to_local(&self, local: &LocalHorizontalCoordinateSystem) -> DMat4 {
        local.ecef_to_local_matrix() * self.anchor_to_fixed
    }

    /// Update the `anchor → ECEF` transform by supplying a new
    /// `anchor → local` matrix.
    pub fn set_anchor_to_local(
        &mut self,
        local: &LocalHorizontalCoordinateSystem,
        new_anchor_to_local: DMat4,
        adjust_orientation: bool,
        ellipsoid: &Ellipsoid,
    ) {
        let new_anchor_to_fixed = local.local_to_ecef_matrix() * new_anchor_to_local;
        self.set_anchor_to_fixed(new_anchor_to_fixed, adjust_orientation, ellipsoid);
    }
}

/// Rotate the orientation component of `new_transform` so that the local "up"
/// vector (the old ECEF surface normal at the old position) maps to the new
/// surface normal at the new position.
///
/// Algorithm:
/// 1. Extract the old surface normal from `old_transform`'s translation.
/// 2. Extract the new surface normal from `new_transform`'s translation.
/// 3. Compute the rotation `R` that takes old → new normal (axis-angle via
///    cross product).
/// 4. Premultiply the rotation-only part of `new_transform` by `R`.
fn adjust_orientation_for_curvature(
    old_transform: &DMat4,
    mut new_transform: DMat4,
    ellipsoid: &Ellipsoid,
) -> DMat4 {
    let old_position = old_transform.col(3).truncate();
    let new_position = new_transform.col(3).truncate();

    let old_normal = ellipsoid.geodetic_surface_normal(old_position);
    let new_normal = ellipsoid.geodetic_surface_normal(new_position);

    // Rotation that takes old_normal → new_normal.
    let rot = rotation_from_normals(old_normal, new_normal);

    // Apply the rotation to the upper-left 3×3 of new_transform.
    let upper3x3 = DMat3::from_cols(
        new_transform.col(0).truncate(),
        new_transform.col(1).truncate(),
        new_transform.col(2).truncate(),
    );
    let rotated = rot * upper3x3;

    new_transform.col_mut(0).x = rotated.col(0).x;
    new_transform.col_mut(0).y = rotated.col(0).y;
    new_transform.col_mut(0).z = rotated.col(0).z;
    new_transform.col_mut(1).x = rotated.col(1).x;
    new_transform.col_mut(1).y = rotated.col(1).y;
    new_transform.col_mut(1).z = rotated.col(1).z;
    new_transform.col_mut(2).x = rotated.col(2).x;
    new_transform.col_mut(2).y = rotated.col(2).y;
    new_transform.col_mut(2).z = rotated.col(2).z;

    new_transform
}

/// Compute the rotation matrix that takes unit vector `from` to unit vector `to`.
///
/// Uses Rodrigues' rotation formula.  If `from` and `to` are parallel,
/// returns `DMat3::IDENTITY`.
fn rotation_from_normals(from: glam::DVec3, to: glam::DVec3) -> DMat3 {
    let dot = from.dot(to).clamp(-1.0, 1.0);
    if (dot - 1.0).abs() < 1e-10 {
        return DMat3::IDENTITY;
    }
    if (dot + 1.0).abs() < 1e-10 {
        // Anti-parallel: 180° rotation around any perpendicular axis.
        let perp = from.any_orthogonal_vector().normalize();
        return rotation_axis_angle(perp, std::f64::consts::PI);
    }
    let axis = from.cross(to).normalize();
    let angle = dot.acos();
    rotation_axis_angle(axis, angle)
}

/// Rodrigues-formula rotation matrix around `axis` by `angle` radians.
fn rotation_axis_angle(axis: glam::DVec3, angle: f64) -> DMat3 {
    let (sin, cos) = angle.sin_cos();
    let t = 1.0 - cos;
    let (x, y, z) = (axis.x, axis.y, axis.z);
    DMat3::from_cols(
        glam::DVec3::new(t * x * x + cos, t * x * y + sin * z, t * x * z - sin * y),
        glam::DVec3::new(t * x * y - sin * z, t * y * y + cos, t * y * z + sin * x),
        glam::DVec3::new(t * x * z + sin * y, t * y * z - sin * x, t * z * z + cos),
    )
}

/// An axis-aligned geodetic bounding rectangle defined by geodetic extent
/// `[west, south, east, north]` in **radians**.
///
/// Used to represent the horizontal extent of a tile or dataset.
/// Heights are handled separately by [`crate::BoundingRegion`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GlobeRectangle {
    /// West boundary longitude (radians).
    pub west: f64,
    /// South boundary latitude (radians).
    pub south: f64,
    /// East boundary longitude (radians).
    pub east: f64,
    /// North boundary latitude (radians).
    pub north: f64,
}

impl GlobeRectangle {
    /// Construct from boundary values already in radians.
    #[inline]
    pub const fn new(west: f64, south: f64, east: f64, north: f64) -> Self {
        Self {
            west,
            south,
            east,
            north,
        }
    }

    /// Construct from boundary values in degrees.
    pub fn from_degrees(west_deg: f64, south_deg: f64, east_deg: f64, north_deg: f64) -> Self {
        Self {
            west: west_deg.to_radians(),
            south: south_deg.to_radians(),
            east: east_deg.to_radians(),
            north: north_deg.to_radians(),
        }
    }

    /// Parse from a 3D Tiles `boundingVolume.region` array (6 floats).
    ///
    /// The array format is `[west, south, east, north, minHeight, maxHeight]`
    /// with the four angular values already in radians.
    /// Returns `None` if the slice has fewer than 4 elements.
    pub fn from_3dtiles_region(region: &[f64]) -> Option<Self> {
        if region.len() < 4 {
            return None;
        }
        Some(Self {
            west: region[0],
            south: region[1],
            east: region[2],
            north: region[3],
        })
    }

    /// Return the `[west, south, east, north]` values in radians.
    #[inline]
    pub fn to_radians_array(self) -> [f64; 4] {
        [self.west, self.south, self.east, self.north]
    }

    // ---- queries -------------------------------------------------------------

    /// Return true if the rectangle contains the given cartographic position.
    ///
    /// Handles antimeridian-crossing rectangles (where `east < west`).
    pub fn contains_cartographic(&self, c: Cartographic) -> bool {
        let lon_ok = if self.east >= self.west {
            c.longitude >= self.west && c.longitude <= self.east
        } else {
            // Crosses the antimeridian.
            c.longitude >= self.west || c.longitude <= self.east
        };
        lon_ok && c.latitude >= self.south && c.latitude <= self.north
    }

    /// Return the intersection of this rectangle with `other`, or `None` if
    /// they do not overlap. Does not handle antimeridian-crossing inputs.
    pub fn intersection(&self, other: &Self) -> Option<Self> {
        let west = self.west.max(other.west);
        let south = self.south.max(other.south);
        let east = self.east.min(other.east);
        let north = self.north.min(other.north);
        if west <= east && south <= north {
            Some(Self::new(west, south, east, north))
        } else {
            None
        }
    }

    /// Geodetic centre of this rectangle as a [`Cartographic`] at height 0.
    pub fn center(&self) -> Cartographic {
        let lon = if self.east >= self.west {
            (self.west + self.east) * 0.5
        } else {
            // Antimeridian crossing: average wraps around.
            let mid = (self.west + self.east + 2.0 * PI) * 0.5;
            if mid > PI { mid - 2.0 * PI } else { mid }
        };
        Cartographic::new(lon, (self.south + self.north) * 0.5, 0.0)
    }

    /// East-west angular width in radians. Handles antimeridian crossing.
    #[inline]
    pub fn width(&self) -> f64 {
        if self.east >= self.west {
            self.east - self.west
        } else {
            self.east - self.west + 2.0 * PI
        }
    }

    /// North-south angular height in radians.
    #[inline]
    pub fn height(&self) -> f64 {
        self.north - self.south
    }

    /// Return true if the rectangle covers the full globe.
    #[inline]
    pub fn is_full_globe(&self) -> bool {
        self.west <= -PI && self.east >= PI && self.south <= -PI / 2.0 && self.north >= PI / 2.0
    }

    /// Return true if this is an empty / degenerate rectangle.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.width() <= 0.0 || self.height() <= 0.0
    }

    /// The full surface of the globe.
    pub const MAX: Self = Self::new(-PI, -PI / 2.0, PI, PI / 2.0);

    /// An empty/degenerate rectangle at the origin.
    pub const EMPTY: Self = Self::new(0.0, 0.0, 0.0, 0.0);
}

impl Default for GlobeRectangle {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Cartographic, LocalDirection};
    use glam::{DMat4, DVec3, DVec4};

    fn wgs84() -> Ellipsoid {
        Ellipsoid::wgs84()
    }

    fn lhcs_at(lon_deg: f64, lat_deg: f64) -> LocalHorizontalCoordinateSystem {
        let e = wgs84();
        LocalHorizontalCoordinateSystem::from_cartographic(
            Cartographic::from_degrees(lon_deg, lat_deg, 0.0),
            LocalDirection::East,
            LocalDirection::North,
            LocalDirection::Up,
            1.0,
            &e,
        )
    }

    #[test]
    fn from_anchor_to_fixed_stores_matrix() {
        let m = DMat4::from_translation(DVec3::new(1.0, 2.0, 3.0));
        let anchor = GlobeAnchor::from_anchor_to_fixed(m);
        assert_eq!(anchor.anchor_to_fixed(), m);
    }

    #[test]
    fn from_anchor_to_local_round_trip() {
        let local = lhcs_at(0.0, 0.0);
        let anchor_to_local = DMat4::from_translation(DVec3::new(10.0, 0.0, 0.0));
        let anchor = GlobeAnchor::from_anchor_to_local(&local, anchor_to_local);
        let recovered = anchor.anchor_to_local(&local);
        for col in 0..4 {
            let diff = (recovered.col(col) - anchor_to_local.col(col)).length();
            assert!(diff < 1e-6, "col {} diff={diff}", col);
        }
    }

    #[test]
    fn set_anchor_no_orientation_adjust() {
        let m1 = DMat4::IDENTITY;
        let m2 = DMat4::from_translation(DVec3::new(6_378_137.0, 0.0, 0.0));
        let mut anchor = GlobeAnchor::from_anchor_to_fixed(m1);
        anchor.set_anchor_to_fixed(m2, false, &wgs84());
        assert_eq!(anchor.anchor_to_fixed(), m2);
    }

    #[test]
    fn set_anchor_with_orientation_adjust_changes_rotation() {
        // Move from equator/0° to equator/90° — the up-direction rotates 90°.
        let e = wgs84();
        let c0 = Cartographic::from_degrees(0.0, 0.0, 0.0);
        let c1 = Cartographic::from_degrees(90.0, 0.0, 0.0);
        let p0 = e.cartographic_to_ecef(c0);
        let p1 = e.cartographic_to_ecef(c1);

        let m0 = DMat4::from_cols(
            DVec4::new(1.0, 0.0, 0.0, 0.0),
            DVec4::new(0.0, 1.0, 0.0, 0.0),
            DVec4::new(0.0, 0.0, 1.0, 0.0),
            DVec4::from((p0, 1.0)),
        );
        let m1 = DMat4::from_cols(
            DVec4::new(1.0, 0.0, 0.0, 0.0),
            DVec4::new(0.0, 1.0, 0.0, 0.0),
            DVec4::new(0.0, 0.0, 1.0, 0.0),
            DVec4::from((p1, 1.0)),
        );
        let mut anchor = GlobeAnchor::from_anchor_to_fixed(m0);
        anchor.set_anchor_to_fixed(m1, true, &e);
        // The rotation part should be different from the naive `m1`.
        let naive_col0: DVec3 = m1.col(0).truncate();
        let adjusted_col0: DVec3 = anchor.anchor_to_fixed().col(0).truncate();
        // They should differ because orientation was rotated.
        let same = (adjusted_col0 - naive_col0).length() < 1e-6;
        assert!(!same, "orientation should have been adjusted");
    }

    #[test]
    fn set_anchor_to_local_round_trip() {
        let e = wgs84();
        let local = lhcs_at(10.0, 20.0);
        let anchor_to_local = DMat4::from_translation(DVec3::new(5.0, 5.0, 0.0));
        let mut anchor = GlobeAnchor::from_anchor_to_local(&local, anchor_to_local);
        // Update via set_anchor_to_local without orientation adjustment.
        let new_local_mat = DMat4::from_translation(DVec3::new(20.0, 0.0, 0.0));
        anchor.set_anchor_to_local(&local, new_local_mat, false, &e);
        let recovered = anchor.anchor_to_local(&local);
        for col in 0..4 {
            let diff = (recovered.col(col) - new_local_mat.col(col)).length();
            assert!(diff < 1e-4, "col {} diff={diff}", col);
        }
    }

    #[test]
    fn rotation_from_same_normal_is_identity() {
        let n = DVec3::new(0.0, 0.0, 1.0);
        let r = rotation_from_normals(n, n);
        let identity = DMat3::IDENTITY;
        for c in 0..3 {
            let diff = (r.col(c) - identity.col(c)).length();
            assert!(diff < 1e-10, "col {} not identity", c);
        }
    }

    #[test]
    fn rotation_from_antipodal_normals_is_180_degrees() {
        let r = rotation_from_normals(DVec3::Z, -DVec3::Z);
        // Z should map to -Z.
        let mapped = r * DVec3::Z;
        assert!((mapped - (-DVec3::Z)).length() < 1e-6, "mapped={mapped}");
    }
}
