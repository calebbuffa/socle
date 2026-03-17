//! Coordinate frame transforms (ENU, surface normal).

use glam::{DMat3, DVec3};

use crate::ellipsoid::Ellipsoid;

/// Compute the East-North-Up (ENU) rotation matrix at a given ECEF position
/// on the ellipsoid.
///
/// The columns of the returned matrix are the East, North, and Up unit vectors
/// in ECEF space.
pub fn enu_frame(ellipsoid: &Ellipsoid, cartesian: DVec3) -> DMat3 {
    let up = ellipsoid.geodetic_surface_normal_cartesian(cartesian);
    // East is perpendicular to up and the Z-axis (world north pole)
    let east = DVec3::new(-up.y, up.x, 0.0).normalize_or_zero();
    // If at a pole, east is undefined — pick an arbitrary east
    let east = if east.length_squared() < 1e-20 {
        DVec3::X
    } else {
        east
    };
    let north = up.cross(east);
    DMat3::from_cols(east, north, up)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartographic::Cartographic;

    #[test]
    fn enu_at_equator_prime_meridian() {
        let e = Ellipsoid::WGS84;
        let p = e.cartographic_to_cartesian(Cartographic::new(0.0, 0.0, 0.0));
        let frame = enu_frame(&e, p);
        // At (lon=0, lat=0): up ≈ +X, east ≈ +Y, north ≈ +Z
        assert!(
            (frame.z_axis - DVec3::new(1.0, 0.0, 0.0)).length() < 0.01,
            "up at (0,0) should be ~+X"
        );
    }

    #[test]
    fn enu_columns_orthonormal() {
        let e = Ellipsoid::WGS84;
        let p = e.cartographic_to_cartesian(Cartographic::from_degrees(-122.4, 37.7, 0.0));
        let frame = enu_frame(&e, p);
        let east = frame.x_axis;
        let north = frame.y_axis;
        let up = frame.z_axis;
        assert!((east.length() - 1.0).abs() < 1e-10);
        assert!((north.length() - 1.0).abs() < 1e-10);
        assert!((up.length() - 1.0).abs() < 1e-10);
        assert!(east.dot(north).abs() < 1e-10);
        assert!(east.dot(up).abs() < 1e-10);
        assert!(north.dot(up).abs() < 1e-10);
    }
}
