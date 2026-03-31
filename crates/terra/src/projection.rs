//! Map projections for converting between geodetic and flat-map coordinates.
//!
//! Mirrors `CesiumGeospatial::GeographicProjection` and
//! `CesiumGeospatial::WebMercatorProjection`.
//!
//! # Geographic (equirectangular)
//!
//! Longitude and latitude (radians) are scaled by the ellipsoid's semi-major
//! axis `R`:
//! ```text
//! x = longitude  × R
//! y = latitude   × R
//! ```
//! This is EPSG:4326 / plate carrée.
//!
//! # Web Mercator (spherical Mercator)
//!
//! Longitude is scaled the same as geographic; latitude is further transformed:
//! ```text
//! x = longitude                               × R
//! y = 0.5 × ln((1 + sin φ) / (1 − sin φ))   × R   [Mercator angle]
//! ```
//! This is EPSG:3857.  The valid latitude range is clamped to
//! `±MAXIMUM_LATITUDE` (≈ ±85.051°), where the projection square-tiles.

use std::f64::consts::PI;

use glam::{DVec2, DVec3};
use zukei::Rectangle;

use crate::{Cartographic, Ellipsoid, GlobeRectangle};

// ── GeographicProjection ──────────────────────────────────────────────────────

/// Equirectangular (plate carrée) projection.
///
/// Longitude and latitude in radians are multiplied by the ellipsoid semi-major
/// axis to produce metres.  Equivalent to `CesiumGeospatial::GeographicProjection`.
#[derive(Debug, Clone)]
pub struct GeographicProjection {
    ellipsoid: Ellipsoid,
    semi_major_axis: f64,
    one_over_semi_major_axis: f64,
}

impl GeographicProjection {
    /// The maximum rectangle coverable by this projection.
    ///
    /// At WGS84 radius this is `[−π·R, −π/2·R, π·R, π/2·R]` metres.
    pub const MAXIMUM_GLOBE_RECTANGLE: GlobeRectangle =
        GlobeRectangle::new(-PI, -PI / 2.0, PI, PI / 2.0);

    /// Construct with the given ellipsoid.
    pub fn new(ellipsoid: Ellipsoid) -> Self {
        let r = ellipsoid.maximum_radius();
        Self {
            ellipsoid,
            semi_major_axis: r,
            one_over_semi_major_axis: 1.0 / r,
        }
    }

    /// Construct using WGS84.
    pub fn wgs84() -> Self {
        Self::new(Ellipsoid::wgs84())
    }

    /// The ellipsoid used by this projection.
    pub fn ellipsoid(&self) -> &Ellipsoid {
        &self.ellipsoid
    }

    /// Project a geodetic position to `(x_metres, y_metres, height_metres)`.
    pub fn project_cartographic(&self, cartographic: Cartographic) -> DVec3 {
        let r = self.semi_major_axis;
        DVec3::new(
            cartographic.longitude * r,
            cartographic.latitude * r,
            cartographic.height,
        )
    }

    /// Project a globe rectangle to a flat [`Rectangle`] in metres.
    pub fn project_rectangle(&self, rect: GlobeRectangle) -> Rectangle {
        let sw = self.project_cartographic(Cartographic::new(rect.west, rect.south, 0.0));
        let ne = self.project_cartographic(Cartographic::new(rect.east, rect.north, 0.0));
        Rectangle::new(sw.x, sw.y, ne.x, ne.y)
    }

    /// Unproject `(x, y)` metres to a [`Cartographic`] at height 0.
    pub fn unproject_2d(&self, projected: DVec2) -> Cartographic {
        let inv = self.one_over_semi_major_axis;
        Cartographic::new(projected.x * inv, projected.y * inv, 0.0)
    }

    /// Unproject `(x, y, z)` metres to a [`Cartographic`], height = z.
    pub fn unproject_3d(&self, projected: DVec3) -> Cartographic {
        let mut c = self.unproject_2d(DVec2::new(projected.x, projected.y));
        c.height = projected.z;
        c
    }

    /// Unproject a flat [`Rectangle`] back to a [`GlobeRectangle`].
    pub fn unproject_rectangle(&self, rect: &Rectangle) -> GlobeRectangle {
        let sw = self.unproject_2d(DVec2::new(rect.minimum_x, rect.minimum_y));
        let ne = self.unproject_2d(DVec2::new(rect.maximum_x, rect.maximum_y));
        GlobeRectangle::new(sw.longitude, sw.latitude, ne.longitude, ne.latitude)
    }
}

// ── WebMercatorProjection ─────────────────────────────────────────────────────

/// Spherical Web Mercator projection (EPSG:3857).
///
/// Equivalent to `CesiumGeospatial::WebMercatorProjection`.
///
/// Latitudes are clamped to `±MAXIMUM_LATITUDE` (≈ ±85.051129°) so the
/// projection tiles as a square.
#[derive(Debug, Clone)]
pub struct WebMercatorProjection {
    ellipsoid: Ellipsoid,
    semi_major_axis: f64,
    one_over_semi_major_axis: f64,
}

impl WebMercatorProjection {
    /// Maximum latitude (both N and S) supported by the projection.
    ///
    /// Computed as `mercator_angle_to_geodetic_latitude(π)` ≈ 1.484_422 rad ≈ 85.051°.
    pub const MAXIMUM_LATITUDE: f64 = 1.484_422_229_745_332_7_f64;

    /// The maximum globe rectangle coverable by this projection.
    pub const MAXIMUM_GLOBE_RECTANGLE: GlobeRectangle =
        GlobeRectangle::new(-PI, -Self::MAXIMUM_LATITUDE, PI, Self::MAXIMUM_LATITUDE);

    /// Construct with the given ellipsoid.
    pub fn new(ellipsoid: Ellipsoid) -> Self {
        let r = ellipsoid.maximum_radius();
        Self {
            ellipsoid,
            semi_major_axis: r,
            one_over_semi_major_axis: 1.0 / r,
        }
    }

    /// Construct using WGS84.
    pub fn wgs84() -> Self {
        Self::new(Ellipsoid::wgs84())
    }

    /// The ellipsoid used by this projection.
    pub fn ellipsoid(&self) -> &Ellipsoid {
        &self.ellipsoid
    }

    /// Convert a Mercator angle (−π … π) to geodetic latitude (−π/2 … π/2).
    ///
    /// Equivalent to `CesiumGeospatial::WebMercatorProjection::mercatorAngleToGeodeticLatitude`.
    #[inline]
    pub fn mercator_angle_to_geodetic_latitude(mercator_angle: f64) -> f64 {
        PI / 2.0 - 2.0 * (-mercator_angle).exp().atan()
    }

    /// Convert geodetic latitude (radians) to a Mercator angle.
    ///
    /// Equivalent to `CesiumGeospatial::WebMercatorProjection::geodeticLatitudeToMercatorAngle`.
    #[inline]
    pub fn geodetic_latitude_to_mercator_angle(latitude: f64) -> f64 {
        let lat = latitude.clamp(-Self::MAXIMUM_LATITUDE, Self::MAXIMUM_LATITUDE);
        let sin_lat = lat.sin();
        0.5 * ((1.0 + sin_lat) / (1.0 - sin_lat)).ln()
    }

    /// Project a geodetic position to `(x_metres, y_metres, height_metres)`.
    pub fn project_cartographic(&self, cartographic: Cartographic) -> DVec3 {
        let r = self.semi_major_axis;
        DVec3::new(
            cartographic.longitude * r,
            Self::geodetic_latitude_to_mercator_angle(cartographic.latitude) * r,
            cartographic.height,
        )
    }

    /// Project a globe rectangle to a flat [`Rectangle`] in metres.
    pub fn project_rectangle(&self, rect: GlobeRectangle) -> Rectangle {
        let sw = self.project_cartographic(Cartographic::new(rect.west, rect.south, 0.0));
        let ne = self.project_cartographic(Cartographic::new(rect.east, rect.north, 0.0));
        Rectangle::new(sw.x, sw.y, ne.x, ne.y)
    }

    /// Unproject `(x, y)` metres to a [`Cartographic`] at height 0.
    pub fn unproject_2d(&self, projected: DVec2) -> Cartographic {
        let inv = self.one_over_semi_major_axis;
        Cartographic::new(
            projected.x * inv,
            Self::mercator_angle_to_geodetic_latitude(projected.y * inv),
            0.0,
        )
    }

    /// Unproject `(x, y, z)` metres to a [`Cartographic`], height = z.
    pub fn unproject_3d(&self, projected: DVec3) -> Cartographic {
        let mut c = self.unproject_2d(DVec2::new(projected.x, projected.y));
        c.height = projected.z;
        c
    }

    /// Unproject a flat [`Rectangle`] back to a [`GlobeRectangle`].
    pub fn unproject_rectangle(&self, rect: &Rectangle) -> GlobeRectangle {
        let sw = self.unproject_2d(DVec2::new(rect.minimum_x, rect.minimum_y));
        let ne = self.unproject_2d(DVec2::new(rect.maximum_x, rect.maximum_y));
        GlobeRectangle::new(sw.longitude, sw.latitude, ne.longitude, ne.latitude)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    const EPSILON: f64 = 1e-9;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < EPSILON
    }

    // ── GeographicProjection ──────────────────────────────────────────────────

    #[test]
    fn geo_project_origin() {
        let proj = GeographicProjection::wgs84();
        let pt = proj.project_cartographic(Cartographic::new(0.0, 0.0, 0.0));
        assert!(approx_eq(pt.x, 0.0));
        assert!(approx_eq(pt.y, 0.0));
        assert!(approx_eq(pt.z, 0.0));
    }

    #[test]
    fn geo_project_prime_meridian_equator_height() {
        let proj = GeographicProjection::wgs84();
        let c = Cartographic::new(0.0, 0.0, 500.0);
        let pt = proj.project_cartographic(c);
        assert!(approx_eq(pt.z, 500.0));
    }

    #[test]
    fn geo_round_trip_cartographic() {
        let proj = GeographicProjection::wgs84();
        let c = Cartographic::from_degrees(45.0, 30.0, 200.0);
        let pt3 = proj.project_cartographic(c);
        let back = proj.unproject_3d(pt3);
        assert!(approx_eq(back.longitude, c.longitude), "lon");
        assert!(approx_eq(back.latitude, c.latitude), "lat");
        assert!(approx_eq(back.height, c.height), "height");
    }

    #[test]
    fn geo_project_unproject_rectangle() {
        let proj = GeographicProjection::wgs84();
        let globe_rect = GlobeRectangle::from_degrees(-90.0, -45.0, 90.0, 45.0);
        let flat = proj.project_rectangle(globe_rect);
        let back = proj.unproject_rectangle(&flat);
        assert!(approx_eq(back.west, globe_rect.west), "west");
        assert!(approx_eq(back.south, globe_rect.south), "south");
        assert!(approx_eq(back.east, globe_rect.east), "east");
        assert!(approx_eq(back.north, globe_rect.north), "north");
    }

    #[test]
    fn geo_x_equals_longitude_times_radius() {
        let ellipsoid = Ellipsoid::wgs84();
        let r = ellipsoid.maximum_radius();
        let proj = GeographicProjection::new(ellipsoid);
        let c = Cartographic::new(1.0, 0.5, 0.0);
        let pt = proj.project_cartographic(c);
        assert!((pt.x - r).abs() < 1e-6, "x={} expected={}", pt.x, r);
        assert!((pt.y - 0.5 * r).abs() < 1e-6);
    }

    // ── WebMercatorProjection ─────────────────────────────────────────────────

    #[test]
    fn web_mercator_project_origin() {
        let proj = WebMercatorProjection::wgs84();
        let pt = proj.project_cartographic(Cartographic::new(0.0, 0.0, 0.0));
        assert!(approx_eq(pt.x, 0.0));
        assert!(approx_eq(pt.y, 0.0));
    }

    #[test]
    fn web_mercator_height_passthrough() {
        let proj = WebMercatorProjection::wgs84();
        let c = Cartographic::new(0.0, 0.0, 1234.0);
        assert!(approx_eq(proj.project_cartographic(c).z, 1234.0));
    }

    #[test]
    fn web_mercator_round_trip_cartographic() {
        let proj = WebMercatorProjection::wgs84();
        let c = Cartographic::from_degrees(10.0, 45.0, 100.0);
        let pt3 = proj.project_cartographic(c);
        let back = proj.unproject_3d(pt3);
        assert!(approx_eq(back.longitude, c.longitude), "lon");
        assert!((back.latitude - c.latitude).abs() < 1e-8, "lat");
        assert!(approx_eq(back.height, c.height), "height");
    }

    #[test]
    fn web_mercator_maximum_latitude_constant() {
        // MAXIMUM_LATITUDE = mercator_angle_to_geodetic_latitude(PI)
        let computed = WebMercatorProjection::mercator_angle_to_geodetic_latitude(PI);
        assert!(
            (computed - WebMercatorProjection::MAXIMUM_LATITUDE).abs() < 1e-10,
            "max_lat constant mismatch: computed={computed} const={}",
            WebMercatorProjection::MAXIMUM_LATITUDE,
        );
    }

    #[test]
    fn web_mercator_latitude_clamped_at_maximum() {
        let proj = WebMercatorProjection::wgs84();
        let c_max = Cartographic::new(0.0, WebMercatorProjection::MAXIMUM_LATITUDE, 0.0);
        let c_over = Cartographic::new(0.0, WebMercatorProjection::MAXIMUM_LATITUDE + 0.01, 0.0);
        // Projecting above MAXIMUM_LATITUDE should give the same y as at MAXIMUM_LATITUDE.
        let y_max = proj.project_cartographic(c_max).y;
        let y_over = proj.project_cartographic(c_over).y;
        assert!(
            (y_max - y_over).abs() < 1e-6,
            "y should clamp: y_max={y_max} y_over={y_over}"
        );
    }

    #[test]
    fn web_mercator_project_unproject_rectangle() {
        let proj = WebMercatorProjection::wgs84();
        let globe_rect = GlobeRectangle::from_degrees(-90.0, -60.0, 90.0, 60.0);
        let flat = proj.project_rectangle(globe_rect);
        let back = proj.unproject_rectangle(&flat);
        assert!(approx_eq(back.west, globe_rect.west), "west");
        assert!((back.south - globe_rect.south).abs() < 1e-8, "south");
        assert!(approx_eq(back.east, globe_rect.east), "east");
        assert!((back.north - globe_rect.north).abs() < 1e-8, "north");
    }

    #[test]
    fn mercator_angle_inverse() {
        // geodeticLatitudeToMercatorAngle and mercatorAngleToGeodeticLatitude should be inverses.
        for lat_deg in [-60.0_f64, -30.0, 0.0, 30.0, 60.0] {
            let lat = lat_deg.to_radians();
            let angle = WebMercatorProjection::geodetic_latitude_to_mercator_angle(lat);
            let back = WebMercatorProjection::mercator_angle_to_geodetic_latitude(angle);
            assert!(
                (back - lat).abs() < 1e-12,
                "lat_deg={lat_deg} back={back} lat={lat}"
            );
        }
    }
}
