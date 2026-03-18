//! Reference ellipsoid with WGS84 constant.

use glam::DVec3;

use crate::cartographic::Cartographic;

/// A reference ellipsoid defined by three semi-axis radii.
///
/// The standard ellipsoid is [`Ellipsoid::WGS84`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ellipsoid {
    pub radii: DVec3,
    radii_squared: DVec3,
    one_over_radii_squared: DVec3,
}

impl Ellipsoid {
    /// WGS84 reference ellipsoid.
    pub const WGS84: Ellipsoid =
        Ellipsoid::from_radii(6_378_137.0, 6_378_137.0, 6_356_752.314_245_179);

    /// Unit sphere (useful for testing).
    pub const UNIT_SPHERE: Ellipsoid = Ellipsoid::from_radii(1.0, 1.0, 1.0);

    const fn from_radii(x: f64, y: f64, z: f64) -> Self {
        Self {
            radii: DVec3::new(x, y, z),
            radii_squared: DVec3::new(x * x, y * y, z * z),
            one_over_radii_squared: DVec3::new(1.0 / (x * x), 1.0 / (y * y), 1.0 / (z * z)),
        }
    }

    /// Create an ellipsoid from semi-axis radii.
    pub fn new(radii: DVec3) -> Self {
        Self {
            radii,
            radii_squared: radii * radii,
            one_over_radii_squared: DVec3::ONE / (radii * radii),
        }
    }

    /// Convert cartographic (longitude, latitude, height) to Cartesian ECEF.
    pub fn cartographic_to_cartesian(&self, carto: Cartographic) -> DVec3 {
        let cos_lat = carto.latitude.cos();
        let sin_lat = carto.latitude.sin();
        let cos_lon = carto.longitude.cos();
        let sin_lon = carto.longitude.sin();

        let n = self.geodetic_surface_normal_from_geodetic(cos_lat, sin_lat, cos_lon, sin_lon);
        let k = self.radii_squared * n;
        let gamma = (n.x * k.x + n.y * k.y + n.z * k.z).sqrt();
        let r_surface = k / gamma;
        r_surface + n * carto.height
    }

    /// Convert Cartesian ECEF to cartographic. Returns `None` if the point
    /// is at the center of the ellipsoid.
    pub fn cartesian_to_cartographic(&self, cartesian: DVec3) -> Option<Cartographic> {
        let p = self.scale_to_geodetic_surface(cartesian)?;
        let n = self.geodetic_surface_normal_cartesian(p);
        let h_vec = cartesian - p;

        let longitude = n.y.atan2(n.x);
        let latitude = n.z.asin();
        let height = h_vec.dot(cartesian).signum() * h_vec.length();
        Some(Cartographic::new(longitude, latitude, height))
    }

    /// Geodetic surface normal at a Cartesian position.
    pub fn geodetic_surface_normal_cartesian(&self, cartesian: DVec3) -> DVec3 {
        let n = cartesian * self.one_over_radii_squared;
        n.normalize()
    }

    /// Scale a Cartesian point to the closest point on the geodetic surface.
    pub fn scale_to_geodetic_surface(&self, cartesian: DVec3) -> Option<DVec3> {
        let one_over_radii = DVec3::ONE / self.radii;
        let one_over_rs = self.one_over_radii_squared;

        let pos_x = cartesian.x;
        let pos_y = cartesian.y;
        let pos_z = cartesian.z;

        // Normalized squared coordinates: (posX/rX)^2 etc.
        let x2 = pos_x * pos_x * one_over_radii.x * one_over_radii.x;
        let y2 = pos_y * pos_y * one_over_radii.y * one_over_radii.y;
        let z2 = pos_z * pos_z * one_over_radii.z * one_over_radii.z;

        let squared_norm = x2 + y2 + z2;
        let ratio = (1.0 / squared_norm).sqrt();

        if !ratio.is_finite() {
            return None;
        }

        // Initial approximation: intersection along the radial direction
        let intersection = cartesian * ratio;

        // If near center, return the initial approximation
        if squared_norm < 0.5 {
            return Some(intersection);
        }

        let gradient_x = intersection.x * one_over_rs.x * 2.0;
        let gradient_y = intersection.y * one_over_rs.y * 2.0;
        let gradient_z = intersection.z * one_over_rs.z * 2.0;
        let gradient_len =
            (gradient_x * gradient_x + gradient_y * gradient_y + gradient_z * gradient_z).sqrt();

        let mut lambda = (1.0 - ratio) * cartesian.length() / (0.5 * gradient_len);

        // Use normalized squared coords (x2, y2, z2) in the iteration
        loop {
            let x_mult = 1.0 / (1.0 + lambda * one_over_rs.x);
            let y_mult = 1.0 / (1.0 + lambda * one_over_rs.y);
            let z_mult = 1.0 / (1.0 + lambda * one_over_rs.z);

            let x_mult2 = x_mult * x_mult;
            let y_mult2 = y_mult * y_mult;
            let z_mult2 = z_mult * z_mult;

            let func = x2 * x_mult2 + y2 * y_mult2 + z2 * z_mult2 - 1.0;

            if func.abs() < 1e-12 {
                return Some(DVec3::new(pos_x * x_mult, pos_y * y_mult, pos_z * z_mult));
            }

            let x_mult3 = x_mult2 * x_mult;
            let y_mult3 = y_mult2 * y_mult;
            let z_mult3 = z_mult2 * z_mult;

            let denom = -2.0
                * (x2 * x_mult3 * one_over_rs.x
                    + y2 * y_mult3 * one_over_rs.y
                    + z2 * z_mult3 * one_over_rs.z);

            let correction = func / denom;
            lambda -= correction;
        }
    }

    /// Equatorial radius (semi-major axis) of the ellipsoid.
    ///
    /// For an oblate spheroid like WGS84, this is `radii.x == radii.y`.
    pub fn semi_major_axis(&self) -> f64 {
        self.radii.x
    }

    /// Semi-minor axis (polar radius) of the ellipsoid.
    pub fn semi_minor_axis(&self) -> f64 {
        self.radii.z
    }

    /// Flattening: `(a - b) / a` where `a` = semi-major, `b` = semi-minor.
    pub fn flattening(&self) -> f64 {
        (self.radii.x - self.radii.z) / self.radii.x
    }

    /// First eccentricity squared: `2f - f²` where `f` = flattening.
    pub fn eccentricity_squared(&self) -> f64 {
        let f = self.flattening();
        2.0 * f - f * f
    }

    /// Maximum radius among all three semi-axes.
    #[inline]
    pub fn maximum_radius(&self) -> f64 {
        self.radii.max_element()
    }

    /// Minimum radius among all three semi-axes.
    #[inline]
    pub fn minimum_radius(&self) -> f64 {
        self.radii.min_element()
    }

    /// Scale a Cartesian point to the closest point on the ellipsoid surface
    /// along the **geocentric** (radial) direction.
    ///
    /// Unlike [`scale_to_geodetic_surface`](Self::scale_to_geodetic_surface),
    /// this projects along the line from the origin through `cartesian`, not
    /// along the geodetic surface normal. Returns `None` if `cartesian` is at
    /// the origin.
    pub fn scale_to_geocentric_surface(&self, cartesian: DVec3) -> Option<DVec3> {
        // Find λ such that ||(cartesian * λ) / radii||² = 1
        // => λ² * dot(cartesian * one_over_radii_squared, cartesian) = 1
        let d2 = self.one_over_radii_squared.dot(cartesian * cartesian);
        if d2 == 0.0 {
            return None;
        }
        Some(cartesian / d2.sqrt())
    }

    fn geodetic_surface_normal_from_geodetic(
        &self,
        cos_lat: f64,
        sin_lat: f64,
        cos_lon: f64,
        sin_lon: f64,
    ) -> DVec3 {
        DVec3::new(cos_lat * cos_lon, cos_lat * sin_lon, sin_lat)
    }
}

#[cfg(test)]
mod tests {
    use i3s_util::math;

    use super::*;

    #[test]
    fn cartographic_to_cartesian_at_equator_prime_meridian() {
        let e = Ellipsoid::WGS84;
        let c = Cartographic::new(0.0, 0.0, 0.0);
        let p = e.cartographic_to_cartesian(c);
        // At (0, 0, 0), x should be the equatorial radius
        assert!((p.x - 6_378_137.0).abs() < 0.01);
        assert!(p.y.abs() < 0.01);
        assert!(p.z.abs() < 0.01);
    }

    #[test]
    fn cartographic_to_cartesian_at_north_pole() {
        let e = Ellipsoid::WGS84;
        let c = Cartographic::new(0.0, std::f64::consts::FRAC_PI_2, 0.0);
        let p = e.cartographic_to_cartesian(c);
        assert!(p.x.abs() < 0.01);
        assert!(p.y.abs() < 0.01);
        // At north pole, z should be the polar radius
        assert!((p.z - 6_356_752.314_245_179).abs() < 0.01);
    }

    #[test]
    fn roundtrip_cartographic_to_cartesian() {
        let e = Ellipsoid::WGS84;
        let original = Cartographic::from_degrees(-122.4194, 37.7749, 100.0);
        let cartesian = e.cartographic_to_cartesian(original);
        let result = e.cartesian_to_cartographic(cartesian).unwrap();
        assert!(math::equals_epsilon(
            result.longitude,
            original.longitude,
            1e-10
        ));
        assert!(math::equals_epsilon(
            result.latitude,
            original.latitude,
            1e-10
        ));
        assert!(math::equals_epsilon(result.height, original.height, 1e-3));
    }

    #[test]
    fn roundtrip_at_various_locations() {
        let e = Ellipsoid::WGS84;
        let locations = [
            (0.0, 0.0, 0.0),
            (180.0, 0.0, 0.0),
            (-90.0, 45.0, 1000.0),
            (135.0, -30.0, 5000.0),
            (0.0, 89.9, 100.0),
        ];
        for (lon, lat, h) in locations {
            let original = Cartographic::from_degrees(lon, lat, h);
            let cartesian = e.cartographic_to_cartesian(original);
            let result = e.cartesian_to_cartographic(cartesian).unwrap();
            assert!(
                math::equals_epsilon(result.longitude, original.longitude, 1e-8),
                "lon mismatch at ({lon}, {lat}, {h}): {} vs {}",
                result.longitude_degrees(),
                original.longitude_degrees()
            );
            assert!(
                math::equals_epsilon(result.latitude, original.latitude, 1e-8),
                "lat mismatch at ({lon}, {lat}, {h})"
            );
            assert!(
                math::equals_epsilon(result.height, original.height, 1e-2),
                "height mismatch at ({lon}, {lat}, {h}): {} vs {}",
                result.height,
                original.height
            );
        }
    }

    #[test]
    fn geodetic_surface_normal_at_equator() {
        let e = Ellipsoid::WGS84;
        let p = e.cartographic_to_cartesian(Cartographic::new(0.0, 0.0, 0.0));
        let n = e.geodetic_surface_normal_cartesian(p);
        assert!(
            (n.length() - 1.0).abs() < 1e-12,
            "normal should be unit vector"
        );
        assert!(
            (n.x - 1.0).abs() < 1e-6,
            "normal at equator/prime meridian should be ~(1,0,0)"
        );
    }

    #[test]
    fn unit_sphere_roundtrip() {
        let e = Ellipsoid::UNIT_SPHERE;
        let original = Cartographic::from_degrees(45.0, 30.0, 0.5);
        let cartesian = e.cartographic_to_cartesian(original);
        let result = e.cartesian_to_cartographic(cartesian).unwrap();
        assert!(math::equals_epsilon(
            result.longitude,
            original.longitude,
            1e-10
        ));
        assert!(math::equals_epsilon(
            result.latitude,
            original.latitude,
            1e-10
        ));
        assert!(math::equals_epsilon(result.height, original.height, 1e-6));
    }
}
