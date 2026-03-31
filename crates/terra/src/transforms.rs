//! Local reference frame transforms relative to the ellipsoid surface.
//!
//! These functions produce 4×4 homogeneous matrices (column-major, matching
//! glam's `DMat4` convention) that transform from a local frame at a given
//! ECEF origin into ECEF world space.

use glam::{DMat4, DVec3, DVec4};

use crate::Ellipsoid;

/// Build the East-North-Up (ENU) frame at `origin`, expressed as a 4×4
/// column-major matrix that transforms from ENU-local to ECEF world space.
///
/// The local axes are:
/// - **+X** = East
/// - **+Y** = North
/// - **+Z** = Up (geodetic surface normal at `origin`)
///
/// Equivalent to Cesium's `Transforms.eastNorthUpToFixedFrame`.
///
/// # Panics
/// Does not panic, but returns a degenerate matrix if `origin` is the
/// ellipsoid centre (surface normal undefined).
pub fn east_north_up_to_ecef(origin: DVec3, ellipsoid: &Ellipsoid) -> DMat4 {
    let up = ellipsoid.geodetic_surface_normal(origin);
    let east = enu_east_axis(up);
    let north = up.cross(east);
    DMat4::from_cols(
        DVec4::from((east, 0.0)),
        DVec4::from((north, 0.0)),
        DVec4::from((up, 0.0)),
        DVec4::from((origin, 1.0)),
    )
}

/// Build the North-East-Down (NED) frame at `origin` in ECEF space.
///
/// The local axes are:
/// - **+X** = North
/// - **+Y** = East
/// - **+Z** = Down (−geodetic surface normal)
pub fn north_east_down_to_ecef(origin: DVec3, ellipsoid: &Ellipsoid) -> DMat4 {
    let up = ellipsoid.geodetic_surface_normal(origin);
    let east = enu_east_axis(up);
    let north = up.cross(east);
    DMat4::from_cols(
        DVec4::from((north, 0.0)),
        DVec4::from((east, 0.0)),
        DVec4::from((-up, 0.0)),
        DVec4::from((origin, 1.0)),
    )
}

/// Compute the ECEF east-axis vector at a surface point whose geodetic
/// surface normal is `up`.
///
/// Uses `Z × up` (cross of the ECEF north-pole axis with the surface normal)
/// for most locations.  Near the poles (`|up.x| < ε && |up.y| < ε`) the
/// cross product is ill-defined, so `X × up` is used instead.
#[inline]
fn enu_east_axis(up: DVec3) -> DVec3 {
    const POLE_THRESHOLD: f64 = 1e-6;
    if up.x.abs() < POLE_THRESHOLD && up.y.abs() < POLE_THRESHOLD {
        // Near the geographic poles — use X-axis as reference.
        DVec3::X.cross(up).normalize()
    } else {
        DVec3::Z.cross(up).normalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Cartographic;
    use glam::DVec3;

    #[test]
    fn enu_at_equator_prime_meridian() {
        let ellipsoid = Ellipsoid::wgs84();
        let origin = ellipsoid.cartographic_to_ecef(Cartographic::from_degrees(0.0, 0.0, 0.0));
        let m = east_north_up_to_ecef(origin, &ellipsoid);

        // East axis at (lon=0, lat=0) should be +Y in ECEF.
        let east = m.col(0).truncate();
        assert!((east - DVec3::Y).length() < 1e-10, "east = {east:?}");

        // Up axis should be ~+X in ECEF (towards origin on equator at lon=0).
        let up = m.col(2).truncate();
        let expected_up = origin.normalize();
        assert!((up - expected_up).length() < 1e-10, "up = {up:?}");
    }

    #[test]
    fn enu_at_north_pole_does_not_panic() {
        let ellipsoid = Ellipsoid::wgs84();
        let origin = ellipsoid.cartographic_to_ecef(Cartographic::from_degrees(0.0, 90.0, 0.0));
        let _m = east_north_up_to_ecef(origin, &ellipsoid);
    }
}
