//! Local horizontal coordinate system anchored to a point on the globe.
//!
//! Equivalent to `CesiumGeospatial::LocalHorizontalCoordinateSystem`.

use glam::{DMat4, DVec3};

use crate::cartographic::Cartographic;
use crate::ellipsoid::Ellipsoid;
use crate::transforms::enu_matrix_at;

/// A principal compass or vertical direction in a local horizontal frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocalDirection {
    East,
    North,
    West,
    South,
    Up,
    Down,
}

/// A coordinate system defined by a local horizontal plane at a point on the globe.
///
/// Each principal axis points in a configurable [`LocalDirection`], letting you
/// create right-handed or left-handed systems aligned to compass directions.
///
/// Stores a pair of `DMat4` transformation matrices:
/// - `local_to_ecef`: converts positions in the local frame → ECEF
/// - `ecef_to_local`: the inverse
#[derive(Debug, Clone, PartialEq)]
pub struct LocalHorizontalCoordinateSystem {
    local_to_ecef: DMat4,
    ecef_to_local: DMat4,
}

impl LocalHorizontalCoordinateSystem {
    /// Create a coordinate system centered at a cartographic origin.
    ///
    /// Each axis points in the given `LocalDirection` at the origin.
    /// `scale_to_meters` converts local units to meters (e.g. `0.01` for cm).
    pub fn new(
        origin: Cartographic,
        x_axis: LocalDirection,
        y_axis: LocalDirection,
        z_axis: LocalDirection,
        scale_to_meters: f64,
        ellipsoid: &Ellipsoid,
    ) -> Self {
        let ecef = ellipsoid.cartographic_to_cartesian(origin);
        Self::from_ecef(ecef, x_axis, y_axis, z_axis, scale_to_meters, ellipsoid)
    }

    /// Create a coordinate system centered at an ECEF origin.
    pub fn from_ecef(
        origin_ecef: DVec3,
        x_axis: LocalDirection,
        y_axis: LocalDirection,
        z_axis: LocalDirection,
        scale_to_meters: f64,
        ellipsoid: &Ellipsoid,
    ) -> Self {
        // ENU basis at origin: columns = East, North, Up in ECEF
        let enu = enu_matrix_at(ellipsoid, origin_ecef);
        let enu_rot = enu.as_mat3();
        let east = enu_rot.x_axis; // col 0
        let north = enu_rot.y_axis; // col 1
        let up = enu_rot.z_axis; // col 2

        let dir_vec = |d: LocalDirection| match d {
            LocalDirection::East => east,
            LocalDirection::West => -east,
            LocalDirection::North => north,
            LocalDirection::South => -north,
            LocalDirection::Up => up,
            LocalDirection::Down => -up,
        };

        let x_col = dir_vec(x_axis) * scale_to_meters;
        let y_col = dir_vec(y_axis) * scale_to_meters;
        let z_col = dir_vec(z_axis) * scale_to_meters;

        let local_to_ecef = DMat4::from_cols(
            x_col.extend(0.0),
            y_col.extend(0.0),
            z_col.extend(0.0),
            origin_ecef.extend(1.0),
        );
        let ecef_to_local = local_to_ecef.inverse();
        Self {
            local_to_ecef,
            ecef_to_local,
        }
    }

    /// Create from a known `local_to_ecef` matrix (advanced use).
    ///
    /// The inverse is computed automatically.
    pub fn from_matrix(local_to_ecef: DMat4) -> Self {
        let ecef_to_local = local_to_ecef.inverse();
        Self {
            local_to_ecef,
            ecef_to_local,
        }
    }

    /// Create from a pre-computed pair of matrices (advanced use).
    ///
    /// The caller must guarantee that `ecef_to_local == local_to_ecef.inverse()`.
    pub fn from_matrices(local_to_ecef: DMat4, ecef_to_local: DMat4) -> Self {
        Self {
            local_to_ecef,
            ecef_to_local,
        }
    }

    /// The transformation matrix from this local frame to ECEF.
    #[inline]
    pub fn local_to_ecef_transform(&self) -> &DMat4 {
        &self.local_to_ecef
    }

    /// The transformation matrix from ECEF to this local frame.
    #[inline]
    pub fn ecef_to_local_transform(&self) -> &DMat4 {
        &self.ecef_to_local
    }

    /// Convert a position in the local frame to ECEF.
    #[inline]
    pub fn local_position_to_ecef(&self, local: DVec3) -> DVec3 {
        self.local_to_ecef.transform_point3(local)
    }

    /// Convert an ECEF position to the local frame.
    #[inline]
    pub fn ecef_position_to_local(&self, ecef: DVec3) -> DVec3 {
        self.ecef_to_local.transform_point3(ecef)
    }

    /// Convert a direction (vector) in the local frame to ECEF.
    ///
    /// The translation column of the matrix is ignored.
    #[inline]
    pub fn local_direction_to_ecef(&self, local: DVec3) -> DVec3 {
        self.local_to_ecef.transform_vector3(local)
    }

    /// Convert an ECEF direction to the local frame.
    ///
    /// The translation column of the matrix is ignored.
    #[inline]
    pub fn ecef_direction_to_local(&self, ecef: DVec3) -> DVec3 {
        self.ecef_to_local.transform_vector3(ecef)
    }

    /// Compute the transformation matrix from this local frame to `target`'s local frame.
    ///
    /// Multiplying a local position by the returned matrix gives its position in `target`.
    #[inline]
    pub fn compute_transformation_to_another_local(&self, target: &Self) -> DMat4 {
        target.ecef_to_local * self.local_to_ecef
    }
}

// Make DMat4::as_mat3 available via a local helper since glam doesn't expose it directly.
trait AsMat3Ext {
    fn as_mat3(&self) -> glam::DMat3;
}

impl AsMat3Ext for DMat4 {
    fn as_mat3(&self) -> glam::DMat3 {
        glam::DMat3::from_cols(
            self.col(0).truncate(),
            self.col(1).truncate(),
            self.col(2).truncate(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartographic::Cartographic;

    #[test]
    fn round_trip_position() {
        let origin = Cartographic::from_degrees(-122.4, 37.8, 0.0);
        let cs = LocalHorizontalCoordinateSystem::new(
            origin,
            LocalDirection::East,
            LocalDirection::North,
            LocalDirection::Up,
            1.0,
            &Ellipsoid::WGS84,
        );
        let local_pt = DVec3::new(100.0, 200.0, 50.0);
        let ecef = cs.local_position_to_ecef(local_pt);
        let back = cs.ecef_position_to_local(ecef);
        assert!(
            (back - local_pt).length() < 1e-6,
            "round-trip error: {back:?}"
        );
    }

    #[test]
    fn transformation_to_another_local_identity() {
        let origin = Cartographic::from_degrees(0.0, 0.0, 0.0);
        let cs = LocalHorizontalCoordinateSystem::new(
            origin,
            LocalDirection::East,
            LocalDirection::North,
            LocalDirection::Up,
            1.0,
            &Ellipsoid::WGS84,
        );
        let xform = cs.compute_transformation_to_another_local(&cs);
        // Should be very close to identity
        assert!((xform.col(0) - glam::DVec4::X).length() < 1e-10);
        assert!((xform.col(1) - glam::DVec4::Y).length() < 1e-10);
        assert!((xform.col(2) - glam::DVec4::Z).length() < 1e-10);
        assert!((xform.col(3) - glam::DVec4::W).length() < 1e-6);
    }

    #[test]
    fn from_matrix_roundtrip() {
        let origin = Cartographic::from_degrees(10.0, 45.0, 100.0);
        let cs1 = LocalHorizontalCoordinateSystem::new(
            origin,
            LocalDirection::East,
            LocalDirection::North,
            LocalDirection::Up,
            1.0,
            &Ellipsoid::WGS84,
        );
        let cs2 = LocalHorizontalCoordinateSystem::from_matrix(*cs1.local_to_ecef_transform());
        let pt = DVec3::new(1.0, 2.0, 3.0);
        assert!((cs1.local_position_to_ecef(pt) - cs2.local_position_to_ecef(pt)).length() < 1e-6);
    }
}
