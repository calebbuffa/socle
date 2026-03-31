//! Local horizontal coordinate system centred on an arbitrary origin on the ellipsoid.
//!
//! Mirrors `CesiumGeospatial::LocalHorizontalCoordinateSystem`.
//!
//! A `LocalHorizontalCoordinateSystem` lets you work in a right- or left-handed
//! local frame whose axes point in configurable compass directions (East, North,
//! West, South, Up, Down), with an optional scale-to-metres factor.
//!
//! # Example
//!
//! ```rust
//! use terra::{Cartographic, Ellipsoid, LocalDirection, LocalHorizontalCoordinateSystem};
//!
//! let ellipsoid = Ellipsoid::wgs84();
//! let origin = Cartographic::from_degrees(-87.6298, 41.8781, 182.0); // Chicago
//!
//! // East-North-Up (the default) at Chicago.
//! let lhcs = LocalHorizontalCoordinateSystem::from_cartographic(
//!     origin,
//!     LocalDirection::East,
//!     LocalDirection::North,
//!     LocalDirection::Up,
//!     1.0,
//!     &ellipsoid,
//! );
//!
//! // Round-trip a position through the local frame.
//! use glam::DVec3;
//! let local_pt = DVec3::new(100.0, 200.0, 0.0); // 100 m east, 200 m north
//! let ecef = lhcs.local_position_to_ecef(local_pt);
//! let back = lhcs.ecef_position_to_local(ecef);
//! assert!((back - local_pt).length() < 1e-6);
//! ```

use glam::{DMat4, DVec3, DVec4};

use crate::transforms::east_north_up_to_ecef;
use crate::{Cartographic, Ellipsoid};

// ── LocalDirection ────────────────────────────────────────────────────────────

/// A cardinal or vertical direction in a local horizontal coordinate system.
///
/// Used to configure which real-world direction each local axis points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalDirection {
    /// +X in ENU space.
    East,
    /// −X in ENU space.
    West,
    /// +Y in ENU space.
    North,
    /// −Y in ENU space.
    South,
    /// +Z in ENU space (away from the ellipsoid surface).
    Up,
    /// −Z in ENU space (towards the ellipsoid centre).
    Down,
}

impl LocalDirection {
    /// Map this direction to its unit vector in East-North-Up (ENU) space.
    #[inline]
    fn to_enu_vector(self) -> DVec3 {
        match self {
            LocalDirection::East => DVec3::new(1.0, 0.0, 0.0),
            LocalDirection::West => DVec3::new(-1.0, 0.0, 0.0),
            LocalDirection::North => DVec3::new(0.0, 1.0, 0.0),
            LocalDirection::South => DVec3::new(0.0, -1.0, 0.0),
            LocalDirection::Up => DVec3::new(0.0, 0.0, 1.0),
            LocalDirection::Down => DVec3::new(0.0, 0.0, -1.0),
        }
    }
}

// ── LocalHorizontalCoordinateSystem ──────────────────────────────────────────

/// A local coordinate frame centred on a point on the ellipsoid surface,
/// with configurable axis-to-compass-direction mappings.
///
/// Equivalent to `CesiumGeospatial::LocalHorizontalCoordinateSystem`.
///
/// The two stored matrices are inverses of each other.  The struct never
/// recomputes them after construction, so all accessors run in O(1).
#[derive(Clone, Debug)]
pub struct LocalHorizontalCoordinateSystem {
    local_to_ecef: DMat4,
    ecef_to_local: DMat4,
}

impl LocalHorizontalCoordinateSystem {
    // ── Constructors ──────────────────────────────────────────────────────────

    /// Create a coordinate system centred at a geodetic `origin`.
    ///
    /// * `x_axis_direction` — real-world direction for the +X axis
    /// * `y_axis_direction` — real-world direction for the +Y axis
    /// * `z_axis_direction` — real-world direction for the +Z axis
    /// * `scale_to_metres` — multiply local coordinates by this to get metres
    ///   (use `1.0` for a metric system)
    ///
    /// # Panics in debug builds
    /// When any two of the three axis directions share the same axis (e.g.
    /// `East` and `West` are both on the EW axis).
    pub fn from_cartographic(
        origin: Cartographic,
        x_axis_direction: LocalDirection,
        y_axis_direction: LocalDirection,
        z_axis_direction: LocalDirection,
        scale_to_metres: f64,
        ellipsoid: &Ellipsoid,
    ) -> Self {
        let origin_ecef = ellipsoid.cartographic_to_ecef(origin);
        Self::from_ecef(
            origin_ecef,
            x_axis_direction,
            y_axis_direction,
            z_axis_direction,
            scale_to_metres,
            ellipsoid,
        )
    }

    /// Create a coordinate system centred at an ECEF `origin_ecef`.
    ///
    /// See [`from_cartographic`](Self::from_cartographic) for parameter docs.
    pub fn from_ecef(
        origin_ecef: DVec3,
        x_axis_direction: LocalDirection,
        y_axis_direction: LocalDirection,
        z_axis_direction: LocalDirection,
        scale_to_metres: f64,
        ellipsoid: &Ellipsoid,
    ) -> Self {
        debug_assert!(
            axes_are_orthogonal(x_axis_direction, y_axis_direction, z_axis_direction),
            "x, y, z directions must each be on a different ENU axis"
        );

        // Build ENU→ECEF frame at the origin.
        let enu_to_ecef = east_north_up_to_ecef(origin_ecef, ellipsoid);

        // Build the local→ENU rotation+scale as a 4×4 (no translation).
        // Each column is the ENU vector for the corresponding local axis,
        // scaled by scale_to_metres.
        let cx = scale_to_metres * x_axis_direction.to_enu_vector();
        let cy = scale_to_metres * y_axis_direction.to_enu_vector();
        let cz = scale_to_metres * z_axis_direction.to_enu_vector();
        let local_to_enu = DMat4::from_cols(
            DVec4::from((cx, 0.0)),
            DVec4::from((cy, 0.0)),
            DVec4::from((cz, 0.0)),
            DVec4::W,
        );

        let local_to_ecef = enu_to_ecef * local_to_enu;
        let ecef_to_local = local_to_ecef.inverse();
        Self {
            local_to_ecef,
            ecef_to_local,
        }
    }

    /// Create a coordinate system from a pre-computed `local_to_ecef` matrix.
    ///
    /// The inverse is computed once at construction time.
    pub fn from_matrix(local_to_ecef: DMat4) -> Self {
        let ecef_to_local = local_to_ecef.inverse();
        Self {
            local_to_ecef,
            ecef_to_local,
        }
    }

    /// Create a coordinate system from pre-computed forward and inverse matrices.
    ///
    /// The caller must ensure the matrices are inverses of each other; no check
    /// is performed.
    pub fn from_matrices(local_to_ecef: DMat4, ecef_to_local: DMat4) -> Self {
        Self {
            local_to_ecef,
            ecef_to_local,
        }
    }

    // ── Accessors ─────────────────────────────────────────────────────────────

    /// The transformation from this local frame to ECEF world space.
    #[inline]
    pub fn local_to_ecef_matrix(&self) -> &DMat4 {
        &self.local_to_ecef
    }

    /// The transformation from ECEF world space to this local frame.
    #[inline]
    pub fn ecef_to_local_matrix(&self) -> &DMat4 {
        &self.ecef_to_local
    }

    // ── Point / direction conversions ─────────────────────────────────────────

    /// Convert a position in local coordinates to ECEF.
    #[inline]
    pub fn local_position_to_ecef(&self, local: DVec3) -> DVec3 {
        (self.local_to_ecef * DVec4::from((local, 1.0))).truncate()
    }

    /// Convert a position in ECEF to local coordinates.
    #[inline]
    pub fn ecef_position_to_local(&self, ecef: DVec3) -> DVec3 {
        (self.ecef_to_local * DVec4::from((ecef, 1.0))).truncate()
    }

    /// Convert a direction (free vector) from local coordinates to ECEF.
    ///
    /// The translation portion of the matrix is ignored.
    #[inline]
    pub fn local_direction_to_ecef(&self, local: DVec3) -> DVec3 {
        (self.local_to_ecef * DVec4::from((local, 0.0))).truncate()
    }

    /// Convert a direction (free vector) from ECEF to local coordinates.
    ///
    /// The translation portion of the matrix is ignored.
    #[inline]
    pub fn ecef_direction_to_local(&self, ecef: DVec3) -> DVec3 {
        (self.ecef_to_local * DVec4::from((ecef, 0.0))).truncate()
    }

    // ── Frame-to-frame ────────────────────────────────────────────────────────

    /// Compute the matrix that transforms positions from *this* local frame to
    /// `target`'s local frame.
    ///
    /// Equivalent to `target.ecef_to_local * self.local_to_ecef`.
    #[inline]
    pub fn transform_to_another_local(&self, target: &Self) -> DMat4 {
        target.ecef_to_local * self.local_to_ecef
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Two `LocalDirection` values are on the same axis when one maps to the
/// additive inverse of the other (e.g. East / West both live on the EW axis).
#[inline]
fn same_axis(a: LocalDirection, b: LocalDirection) -> bool {
    let va = a.to_enu_vector();
    let vb = b.to_enu_vector();
    // Same or opposite — the cross product is zero.
    va.cross(vb).length_squared() < 1e-12
}

fn axes_are_orthogonal(x: LocalDirection, y: LocalDirection, z: LocalDirection) -> bool {
    !same_axis(x, y) && !same_axis(x, z) && !same_axis(y, z)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Cartographic;

    fn wgs84() -> Ellipsoid {
        Ellipsoid::wgs84()
    }

    // Tolerance for double comparisons.
    const EPS: f64 = 1e-6;

    /// Round-trip a local position through the frame.
    #[test]
    fn local_ecef_position_round_trip() {
        let e = wgs84();
        let origin = Cartographic::from_degrees(10.0, 45.0, 0.0);
        let lhcs = LocalHorizontalCoordinateSystem::from_cartographic(
            origin,
            LocalDirection::East,
            LocalDirection::North,
            LocalDirection::Up,
            1.0,
            &e,
        );
        let local = DVec3::new(100.0, 200.0, 50.0);
        let ecef = lhcs.local_position_to_ecef(local);
        let back = lhcs.ecef_position_to_local(ecef);
        assert!(
            (back - local).length() < EPS,
            "position round-trip: {back:?} vs {local:?}"
        );
    }

    /// Round-trip a local direction through the frame.
    #[test]
    fn local_ecef_direction_round_trip() {
        let e = wgs84();
        let origin = Cartographic::from_degrees(-73.9857, 40.7484, 0.0);
        let lhcs = LocalHorizontalCoordinateSystem::from_cartographic(
            origin,
            LocalDirection::East,
            LocalDirection::North,
            LocalDirection::Up,
            1.0,
            &e,
        );
        let dir = DVec3::new(1.0, 0.0, 0.0).normalize(); // pure east
        let ecef_dir = lhcs.local_direction_to_ecef(dir);
        let back = lhcs.ecef_direction_to_local(ecef_dir);
        assert!(
            (back - dir).length() < EPS,
            "direction round-trip: {back:?}"
        );
    }

    /// Two calls with same origin and default axes give matching matrices.
    #[test]
    fn from_ecef_and_from_cartographic_agree() {
        let e = wgs84();
        let c = Cartographic::from_degrees(45.0, 30.0, 0.0);
        let from_c = LocalHorizontalCoordinateSystem::from_cartographic(
            c,
            LocalDirection::East,
            LocalDirection::North,
            LocalDirection::Up,
            1.0,
            &e,
        );
        let ecef_origin = e.cartographic_to_ecef(c);
        let from_xyz = LocalHorizontalCoordinateSystem::from_ecef(
            ecef_origin,
            LocalDirection::East,
            LocalDirection::North,
            LocalDirection::Up,
            1.0,
            &e,
        );
        let diff = from_c.local_to_ecef - from_xyz.local_to_ecef;
        let max_err = diff.as_ref().iter().cloned().fold(0.0_f64, f64::max);
        assert!(max_err.abs() < EPS, "matrix mismatch: {max_err}");
    }

    /// `from_matrix` round-trip: encode → decode same position.
    #[test]
    fn from_matrix_round_trip() {
        let e = wgs84();
        let origin = Cartographic::from_degrees(0.0, 0.0, 0.0);
        let source = LocalHorizontalCoordinateSystem::from_cartographic(
            origin,
            LocalDirection::East,
            LocalDirection::North,
            LocalDirection::Up,
            1.0,
            &e,
        );
        let lhcs2 = LocalHorizontalCoordinateSystem::from_matrix(*source.local_to_ecef_matrix());
        let local = DVec3::new(1_000.0, -500.0, 100.0);
        let p1 = source.local_position_to_ecef(local);
        let p2 = lhcs2.local_position_to_ecef(local);
        assert!((p1 - p2).length() < EPS);
    }

    /// Scale factor is applied correctly.
    #[test]
    fn scale_to_metres_centimetres() {
        let e = wgs84();
        let origin = Cartographic::from_degrees(0.0, 0.0, 0.0);
        // Units in centimetres → scale = 1/100.
        let lhcs_cm = LocalHorizontalCoordinateSystem::from_cartographic(
            origin,
            LocalDirection::East,
            LocalDirection::North,
            LocalDirection::Up,
            1.0 / 100.0,
            &e,
        );
        // Units in metres → scale = 1.0.
        let lhcs_m = LocalHorizontalCoordinateSystem::from_cartographic(
            origin,
            LocalDirection::East,
            LocalDirection::North,
            LocalDirection::Up,
            1.0,
            &e,
        );
        // 100 cm east should land at the same ECEF as 1 m east.
        let ecef_cm = lhcs_cm.local_position_to_ecef(DVec3::new(100.0, 0.0, 0.0));
        let ecef_m = lhcs_m.local_position_to_ecef(DVec3::new(1.0, 0.0, 0.0));
        assert!(
            (ecef_cm - ecef_m).length() < EPS,
            "scale mismatch: cm={ecef_cm:?} m={ecef_m:?}"
        );
    }

    /// Custom axis directions: NED (North-East-Down), a common aviation frame.
    #[test]
    fn ned_frame_local_direction() {
        let e = wgs84();
        let origin = Cartographic::from_degrees(0.0, 0.0, 0.0);
        let ned = LocalHorizontalCoordinateSystem::from_cartographic(
            origin,
            LocalDirection::North,
            LocalDirection::East,
            LocalDirection::Down,
            1.0,
            &e,
        );
        let enu = LocalHorizontalCoordinateSystem::from_cartographic(
            origin,
            LocalDirection::East,
            LocalDirection::North,
            LocalDirection::Up,
            1.0,
            &e,
        );
        // A pure-East vector in ENU is a pure-Y vector in NED.
        let east_in_enu = DVec3::new(1.0, 0.0, 0.0); // local-east in ENU frame
        let ecef_east = enu.local_direction_to_ecef(east_in_enu);
        let ned_east = ned.ecef_direction_to_local(ecef_east);
        assert!(
            (ned_east - DVec3::new(0.0, 1.0, 0.0)).length() < EPS,
            "east in NED should be (0,1,0), got {ned_east:?}"
        );
    }

    /// `transform_to_another_local` correctly converts positions between frames.
    #[test]
    fn transform_to_another_local() {
        let e = wgs84();
        let origin_a = Cartographic::from_degrees(0.0, 0.0, 0.0);
        let origin_b = Cartographic::from_degrees(1.0, 0.0, 0.0); // 1 deg east

        let fa = LocalHorizontalCoordinateSystem::from_cartographic(
            origin_a,
            LocalDirection::East,
            LocalDirection::North,
            LocalDirection::Up,
            1.0,
            &e,
        );
        let fb = LocalHorizontalCoordinateSystem::from_cartographic(
            origin_b,
            LocalDirection::East,
            LocalDirection::North,
            LocalDirection::Up,
            1.0,
            &e,
        );

        let a_to_b = fa.transform_to_another_local(&fb);

        // The local origin of A in its own frame is (0,0,0).
        let origin_of_a_in_b = (a_to_b * DVec4::new(0.0, 0.0, 0.0, 1.0)).truncate();
        // Should be ~(−111 km, 0, 0) in B (A is ~1 degree west of B).
        assert!(
            origin_of_a_in_b.x < -100_000.0,
            "A should be west of B: {origin_of_a_in_b:?}"
        );
        assert!(
            origin_of_a_in_b.y.abs() < 1000.0,
            "no north–south offset: {origin_of_a_in_b:?}"
        );
    }

    /// Origin position is recovered correctly from the forward matrix.
    #[test]
    fn origin_ecef_is_translation_column() {
        let e = wgs84();
        let c = Cartographic::from_degrees(30.0, 45.0, 200.0);
        let lhcs = LocalHorizontalCoordinateSystem::from_cartographic(
            c,
            LocalDirection::East,
            LocalDirection::North,
            LocalDirection::Up,
            1.0,
            &e,
        );
        let m = lhcs.local_to_ecef_matrix();
        let translation = m.col(3).truncate();
        let expected = e.cartographic_to_ecef(c);
        assert!(
            (translation - expected).length() < 1.0, // 1 m tolerance
            "translation column should be origin ECEF: {translation:?} vs {expected:?}"
        );
    }
}
