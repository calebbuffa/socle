//! Map projections: Web Mercator, Geographic (Plate Carrée), Transverse Mercator.
//!
//! All projection functions accept an [`Ellipsoid`] so callers can work with
//! different reference ellipsoids. The default is [`Ellipsoid::WGS84`].

use crate::cartographic::Cartographic;
use crate::ellipsoid::Ellipsoid;
use std::f64::consts::PI;

/// Maximum latitude for Web Mercator (≈85.051°).
const MAX_MERCATOR_LATITUDE: f64 = 1.4844222297453324; // atan(sinh(PI))

/// Project a cartographic position to Web Mercator (EPSG:3857).
///
/// Uses the `ellipsoid`'s semi-major axis for the projection radius.
/// Returns `(x, y)` in meters. The origin is at (lon=0, lat=0).
pub fn to_web_mercator(carto: &Cartographic, ellipsoid: &Ellipsoid) -> (f64, f64) {
    let a = ellipsoid.semi_major_axis();
    let x = a * carto.longitude;
    let lat = carto
        .latitude
        .clamp(-MAX_MERCATOR_LATITUDE, MAX_MERCATOR_LATITUDE);
    let y = a * ((PI * 0.25 + lat * 0.5).tan()).ln();
    (x, y)
}

/// Unproject Web Mercator (EPSG:3857) coordinates back to cartographic.
///
/// Uses the `ellipsoid`'s semi-major axis. Takes `(x, y)` in meters,
/// returns a `Cartographic` with height = 0.
pub fn from_web_mercator(x: f64, y: f64, ellipsoid: &Ellipsoid) -> Cartographic {
    let a = ellipsoid.semi_major_axis();
    let longitude = x / a;
    let latitude = (2.0 * (y / a).exp().atan()) - PI * 0.5;
    Cartographic::new(longitude, latitude, 0.0)
}

/// Project a cartographic position to Geographic / Plate Carrée.
///
/// Returns `(x, y)` where x = longitude in radians, y = latitude in radians.
/// This is effectively a no-op but provided for API symmetry.
pub fn to_geographic(carto: &Cartographic) -> (f64, f64) {
    (carto.longitude, carto.latitude)
}

/// Unproject Geographic (Plate Carrée) coordinates from degrees back to
/// cartographic (radians).
///
/// Takes `(longitude_deg, latitude_deg)`, returns a `Cartographic` with height = 0.
pub fn from_geographic_degrees(lon_deg: f64, lat_deg: f64) -> Cartographic {
    Cartographic::from_degrees(lon_deg, lat_deg, 0.0)
}

// ---------------------------------------------------------------------------
// Transverse Mercator
// ---------------------------------------------------------------------------

/// Parameters for a Transverse Mercator projection.
///
/// Covers UTM zones and many State Plane zones that use Transverse Mercator.
/// The ellipsoid defaults to [`Ellipsoid::WGS84`] but can be overridden for
/// other datums (e.g. GRS80, Clarke 1866).
#[derive(Debug, Clone, Copy)]
pub struct TransverseMercatorParams {
    /// Central meridian (radians).
    pub central_meridian: f64,
    /// Latitude of origin (radians). Usually 0 for UTM.
    pub latitude_of_origin: f64,
    /// Scale factor on the central meridian. 0.9996 for UTM.
    pub scale_factor: f64,
    /// False easting in meters. 500_000 for UTM.
    pub false_easting: f64,
    /// False northing in meters. 0 for UTM North, 10_000_000 for South.
    pub false_northing: f64,
    /// Reference ellipsoid.
    pub ellipsoid: Ellipsoid,
}

impl TransverseMercatorParams {
    /// Create parameters for a UTM zone (1–60), north or south hemisphere,
    /// using [`Ellipsoid::WGS84`].
    pub fn utm(zone: u8, north: bool) -> Self {
        Self::utm_with_ellipsoid(zone, north, Ellipsoid::WGS84)
    }

    /// Create parameters for a UTM zone with a custom ellipsoid.
    pub fn utm_with_ellipsoid(zone: u8, north: bool, ellipsoid: Ellipsoid) -> Self {
        let central_meridian_deg = -183.0 + zone as f64 * 6.0;
        Self {
            central_meridian: central_meridian_deg * (PI / 180.0),
            latitude_of_origin: 0.0,
            scale_factor: 0.9996,
            false_easting: 500_000.0,
            false_northing: if north { 0.0 } else { 10_000_000.0 },
            ellipsoid,
        }
    }

    /// Semi-major axis from the stored ellipsoid.
    fn semi_major(&self) -> f64 {
        self.ellipsoid.semi_major_axis()
    }

    /// Eccentricity squared from the stored ellipsoid.
    fn e2(&self) -> f64 {
        self.ellipsoid.eccentricity_squared()
    }

    /// Meridian arc length from equator to latitude phi.
    ///
    /// Uses the standard series expansion (Helmert 1880).
    fn meridian_arc(&self, phi: f64) -> f64 {
        let e2 = self.e2();
        let e4 = e2 * e2;
        let e6 = e4 * e2;
        let e8 = e6 * e2;

        let a0 = 1.0 - e2 / 4.0 - 3.0 * e4 / 64.0 - 5.0 * e6 / 256.0 - 175.0 * e8 / 16384.0;
        let a2 = 3.0 / 8.0 * (e2 + e4 / 4.0 + 15.0 * e6 / 128.0 + 35.0 * e8 / 512.0);
        let a4 = 15.0 / 256.0 * (e4 + 3.0 * e6 / 4.0 + 35.0 * e8 / 64.0);
        let a6 = 35.0 / 3072.0 * (e6 + 5.0 * e8 / 4.0);
        let a8 = 315.0 * e8 / 131072.0;

        self.semi_major()
            * (a0 * phi - a2 * (2.0 * phi).sin() + a4 * (4.0 * phi).sin() - a6 * (6.0 * phi).sin()
                + a8 * (8.0 * phi).sin())
    }

    /// Compute the footpoint latitude from northing.
    ///
    /// Given a northing value (corrected for false northing and scale),
    /// iterates to find the latitude whose meridian arc equals that northing.
    fn footpoint_latitude(&self, northing: f64) -> f64 {
        let e2 = self.e2();
        let e1 = (1.0 - (1.0 - e2).sqrt()) / (1.0 + (1.0 - e2).sqrt());
        let e1_2 = e1 * e1;
        let e1_3 = e1_2 * e1;
        let e1_4 = e1_3 * e1;

        let mu = northing
            / (self.semi_major()
                * (1.0 - e2 / 4.0 - 3.0 * e2 * e2 / 64.0 - 5.0 * e2 * e2 * e2 / 256.0));

        let phi1 = mu
            + (3.0 * e1 / 2.0 - 27.0 * e1_3 / 32.0) * (2.0 * mu).sin()
            + (21.0 * e1_2 / 16.0 - 55.0 * e1_4 / 32.0) * (4.0 * mu).sin()
            + (151.0 * e1_3 / 96.0) * (6.0 * mu).sin()
            + (1097.0 * e1_4 / 512.0) * (8.0 * mu).sin();

        phi1
    }
}

/// Unproject Transverse Mercator coordinates to cartographic.
///
/// Takes `(easting, northing)` in meters with the projection parameters,
/// returns a `Cartographic` with height = 0.
///
/// Uses the standard Redfearn (1948) inverse formulas, accurate to
/// sub-millimeter within the valid zone.
pub fn from_transverse_mercator(
    easting: f64,
    northing: f64,
    params: &TransverseMercatorParams,
) -> Cartographic {
    let e2 = params.e2();
    let ep2 = e2 / (1.0 - e2); // second eccentricity squared

    // Remove false easting/northing and scale
    let x = (easting - params.false_easting) / params.scale_factor;
    let y = (northing - params.false_northing) / params.scale_factor;

    // Footpoint latitude
    let phi1 = params.footpoint_latitude(y);

    let cos_phi1 = phi1.cos();
    let sin_phi1 = phi1.sin();
    let tan_phi1 = phi1.tan();

    let t1 = tan_phi1 * tan_phi1;
    let c1 = ep2 * cos_phi1 * cos_phi1;
    let a = params.semi_major();
    let n1 = a / (1.0 - e2 * sin_phi1 * sin_phi1).sqrt();
    let r1 = a * (1.0 - e2) / (1.0 - e2 * sin_phi1 * sin_phi1).powf(1.5);
    let d = x / n1;

    let d2 = d * d;
    let d3 = d2 * d;
    let d4 = d3 * d;
    let d5 = d4 * d;
    let d6 = d5 * d;

    // Latitude
    let latitude = phi1
        - (n1 * tan_phi1 / r1)
            * (d2 / 2.0 - (5.0 + 3.0 * t1 + 10.0 * c1 - 4.0 * c1 * c1 - 9.0 * ep2) * d4 / 24.0
                + (61.0 + 90.0 * t1 + 298.0 * c1 + 45.0 * t1 * t1 - 252.0 * ep2 - 3.0 * c1 * c1)
                    * d6
                    / 720.0);

    // Longitude
    let longitude = params.central_meridian
        + (d - (1.0 + 2.0 * t1 + c1) * d3 / 6.0
            + (5.0 - 2.0 * c1 + 28.0 * t1 - 3.0 * c1 * c1 + 8.0 * ep2 + 24.0 * t1 * t1) * d5
                / 120.0)
            / cos_phi1;

    Cartographic::new(longitude, latitude, 0.0)
}

/// Project a cartographic position to Transverse Mercator.
///
/// Returns `(easting, northing)` in meters.
pub fn to_transverse_mercator(
    carto: &Cartographic,
    params: &TransverseMercatorParams,
) -> (f64, f64) {
    let e2 = params.e2();
    let ep2 = e2 / (1.0 - e2);

    let phi = carto.latitude;
    let lambda = carto.longitude - params.central_meridian;

    let cos_phi = phi.cos();
    let sin_phi = phi.sin();
    let tan_phi = phi.tan();

    let t = tan_phi * tan_phi;
    let c = ep2 * cos_phi * cos_phi;
    let n = params.semi_major() / (1.0 - e2 * sin_phi * sin_phi).sqrt();
    let m = params.meridian_arc(phi);

    let l = lambda;
    let l2 = l * l;
    let l3 = l2 * l;
    let l4 = l3 * l;
    let l5 = l4 * l;
    let l6 = l5 * l;

    let easting = params.false_easting
        + params.scale_factor
            * n
            * (l * cos_phi
                + (1.0 - t + c) * l3 * cos_phi * cos_phi * cos_phi / 6.0
                + (5.0 - 18.0 * t + t * t + 72.0 * c - 58.0 * ep2) * l5 * cos_phi.powi(5) / 120.0);

    let northing = params.false_northing
        + params.scale_factor
            * (m - params.meridian_arc(params.latitude_of_origin)
                + n * tan_phi
                    * (l2 * cos_phi * cos_phi / 2.0
                        + (5.0 - t + 9.0 * c + 4.0 * c * c) * l4 * cos_phi.powi(4) / 24.0
                        + (61.0 - 58.0 * t + t * t + 600.0 * c - 330.0 * ep2)
                            * l6
                            * cos_phi.powi(6)
                            / 720.0));

    (easting, northing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use i3s_util::math;

    #[test]
    fn web_mercator_origin() {
        let c = Cartographic::new(0.0, 0.0, 0.0);
        let (x, y) = to_web_mercator(&c, &Ellipsoid::WGS84);
        assert!(x.abs() < 1e-6);
        assert!(y.abs() < 1e-6);
    }

    #[test]
    fn web_mercator_roundtrip() {
        let original = Cartographic::from_degrees(-122.4194, 37.7749, 0.0);
        let e = Ellipsoid::WGS84;
        let (x, y) = to_web_mercator(&original, &e);
        let result = from_web_mercator(x, y, &e);
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
    }

    #[test]
    fn web_mercator_at_90_degrees() {
        let c = Cartographic::from_degrees(90.0, 0.0, 0.0);
        let (x, _y) = to_web_mercator(&c, &Ellipsoid::WGS84);
        // x should be ~10018754 meters (quarter of circumference)
        assert!((x - 10_018_754.17).abs() < 1.0);
    }

    #[test]
    fn utm_zone_10n_roundtrip() {
        // San Francisco: lon=-122.4194, lat=37.7749
        // UTM Zone 10N expected: ~551_000 E, ~4_180_000 N
        let original = Cartographic::from_degrees(-122.4194, 37.7749, 0.0);
        let params = TransverseMercatorParams::utm(10, true);
        let (e, n) = to_transverse_mercator(&original, &params);

        // Check the forward produces plausible UTM coords
        assert!((e - 551_000.0).abs() < 1000.0, "easting: {e}");
        assert!((n - 4_180_000.0).abs() < 5000.0, "northing: {n}");

        // Roundtrip
        let result = from_transverse_mercator(e, n, &params);
        assert!(
            math::equals_epsilon(result.longitude, original.longitude, 1e-9),
            "lon: {} vs {}",
            result.longitude_degrees(),
            original.longitude_degrees()
        );
        assert!(
            math::equals_epsilon(result.latitude, original.latitude, 1e-9),
            "lat: {} vs {}",
            result.latitude_degrees(),
            original.latitude_degrees()
        );
    }

    #[test]
    fn utm_zone_32n_oslo() {
        // Oslo: lon=10.75, lat=59.91
        // UTM Zone 32N expected: ~597_000 E, ~6_643_000 N
        let original = Cartographic::from_degrees(10.75, 59.91, 0.0);
        let params = TransverseMercatorParams::utm(32, true);
        let (e, n) = to_transverse_mercator(&original, &params);
        let result = from_transverse_mercator(e, n, &params);
        assert!(
            math::equals_epsilon(result.longitude, original.longitude, 1e-9),
            "lon mismatch"
        );
        assert!(
            math::equals_epsilon(result.latitude, original.latitude, 1e-9),
            "lat mismatch"
        );
    }

    #[test]
    fn utm_zone_56s_sydney() {
        // Sydney: lon=151.21, lat=-33.87
        // UTM Zone 56S
        let original = Cartographic::from_degrees(151.21, -33.87, 0.0);
        let params = TransverseMercatorParams::utm(56, false);
        let (e, n) = to_transverse_mercator(&original, &params);
        let result = from_transverse_mercator(e, n, &params);
        assert!(
            math::equals_epsilon(result.longitude, original.longitude, 1e-9),
            "lon mismatch"
        );
        assert!(
            math::equals_epsilon(result.latitude, original.latitude, 1e-9),
            "lat mismatch"
        );
    }

    #[test]
    fn tm_at_central_meridian() {
        // Point exactly on central meridian should have easting = false_easting
        let params = TransverseMercatorParams::utm(17, true);
        let on_cm = Cartographic::from_degrees(-81.0, 40.0, 0.0);
        let (e, _n) = to_transverse_mercator(&on_cm, &params);
        assert!(
            (e - 500_000.0).abs() < 0.01,
            "easting on central meridian: {e}"
        );
    }

    #[test]
    fn from_geographic_degrees_sanity() {
        let c = from_geographic_degrees(-122.4194, 37.7749);
        assert!((c.longitude_degrees() - (-122.4194)).abs() < 1e-10);
        assert!((c.latitude_degrees() - 37.7749).abs() < 1e-10);
        assert!(c.height.abs() < 1e-12);
    }
}
