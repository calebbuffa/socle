//! Coordinate Reference System trait and built-in implementations.
//!
//! The [`Crs`] trait is the unification point between 3D Tiles (which works
//! exclusively in ECEF) and I3S (which supports arbitrary CRS identified by
//! WKID or WKT).  Every [`Crs`] impl can convert positions to and from
//! [`Cartographic`] (geodetic lon/lat/height), making it possible to inter-
//! operate between datasets in different coordinate systems.
//!
//! # Coordinate conventions
//!
//! The `DVec3` passed to / returned from [`Crs`] methods is interpreted
//! according to the CRS:
//!
//! | CRS | `DVec3` meaning |
//! |-----|-----------------|
//! | [`GeographicCrs`] (WKID 4326) | `(lon_deg, lat_deg, height_m)` |
//! | [`EcefCrs`] (WKID 4978) | `(X_m, Y_m, Z_m)` ECEF |
//! | [`WebMercatorCrs`] (WKID 3857) | `(easting_m, northing_m, height_m)` |

use glam::DVec3;

use crate::{Cartographic, Ellipsoid};

/// Abstraction over a coordinate reference system.
///
/// Implement this trait to support additional projected CRS (e.g. UTM zones)
/// beyond the built-in ones.  All implementations must be `Send + Sync` so
/// they can be shared across async tasks.
pub trait Crs: Send + Sync + 'static {
    /// Human-readable name, e.g. `"WGS 84"` or `"WGS 84 / Pseudo-Mercator"`.
    fn name(&self) -> &str;

    /// EPSG / ESRI WKID, if this CRS has one.
    fn wkid(&self) -> Option<u32> {
        None
    }

    /// Convert a position in this CRS to geodetic lon/lat/height.
    ///
    /// Returns `None` when the position is invalid (e.g. underground for ECEF).
    fn to_cartographic(&self, position: DVec3) -> Option<Cartographic>;

    /// Convert geodetic lon/lat/height to a position in this CRS.
    fn from_cartographic(&self, c: Cartographic) -> DVec3;

    /// The reference ellipsoid used by this CRS, if applicable.
    fn ellipsoid(&self) -> Option<&Ellipsoid> {
        None
    }
}

/// Geographic CRS — positions as `(longitude_deg, latitude_deg, height_m)`.
///
/// Corresponds to EPSG:4326 (WGS 84).  Commonly used as the `spatialReference`
/// for I3S scene layer packages.  This is *not* a projected CRS; easting and
/// northing are the raw degree values.
#[derive(Debug, Clone)]
pub struct GeographicCrs {
    ellipsoid: Ellipsoid,
}

impl GeographicCrs {
    pub fn new(ellipsoid: Ellipsoid) -> Self {
        Self { ellipsoid }
    }

    pub fn wgs84() -> Self {
        Self::new(Ellipsoid::wgs84())
    }
}

impl Crs for GeographicCrs {
    fn name(&self) -> &str {
        "WGS 84"
    }

    fn wkid(&self) -> Option<u32> {
        Some(4326)
    }

    /// Expected input: `(lon_deg, lat_deg, height_m)`.
    fn to_cartographic(&self, position: DVec3) -> Option<Cartographic> {
        Some(Cartographic::from_degrees(
            position.x, position.y, position.z,
        ))
    }

    /// Returns `(lon_deg, lat_deg, height_m)`.
    fn from_cartographic(&self, c: Cartographic) -> DVec3 {
        let (lon, lat, h) = c.to_degrees();
        DVec3::new(lon, lat, h)
    }

    fn ellipsoid(&self) -> Option<&Ellipsoid> {
        Some(&self.ellipsoid)
    }
}

/// Earth-Centred Earth-Fixed (ECEF) CRS — positions as `(X_m, Y_m, Z_m)`.
///
/// Corresponds to EPSG:4978 (WGS 84).  This is the world-space used by
/// 3D Tiles tile transforms and bounding volumes.
#[derive(Debug, Clone)]
pub struct EcefCrs {
    ellipsoid: Ellipsoid,
}

impl EcefCrs {
    pub fn new(ellipsoid: Ellipsoid) -> Self {
        Self { ellipsoid }
    }

    pub fn wgs84() -> Self {
        Self::new(Ellipsoid::wgs84())
    }
}

impl Crs for EcefCrs {
    fn name(&self) -> &str {
        "WGS 84 (ECEF)"
    }

    fn wkid(&self) -> Option<u32> {
        Some(4978)
    }

    /// Input is ECEF `(X_m, Y_m, Z_m)`.
    fn to_cartographic(&self, position: DVec3) -> Option<Cartographic> {
        self.ellipsoid.ecef_to_cartographic(position)
    }

    /// Returns ECEF `(X_m, Y_m, Z_m)`.
    fn from_cartographic(&self, c: Cartographic) -> DVec3 {
        self.ellipsoid.cartographic_to_ecef(c)
    }

    fn ellipsoid(&self) -> Option<&Ellipsoid> {
        Some(&self.ellipsoid)
    }
}

/// Web Mercator CRS — positions as `(easting_m, northing_m, height_m)`.
///
/// Corresponds to EPSG:3857 / ESRI:102100 / ESRI:900913.
/// This is the projection used by Google Maps, OpenStreetMap, and is a common
/// I3S `spatialReference`.
///
/// # Projection formulas
///
/// ```text
/// easting  = a · lon_rad
/// northing = a · ln(tan(π/4 + lat_rad/2))
/// ```
/// Inverse:
/// ```text
/// lon_rad = easting / a
/// lat_rad = 2·atan(exp(northing / a)) − π/2
/// ```
#[derive(Debug, Clone)]
pub struct WebMercatorCrs {
    ellipsoid: Ellipsoid,
}

impl WebMercatorCrs {
    pub fn new(ellipsoid: Ellipsoid) -> Self {
        Self { ellipsoid }
    }

    pub fn wgs84() -> Self {
        Self::new(Ellipsoid::wgs84())
    }
}

impl Crs for WebMercatorCrs {
    fn name(&self) -> &str {
        "WGS 84 / Pseudo-Mercator"
    }

    fn wkid(&self) -> Option<u32> {
        Some(3857)
    }

    /// Input is `(easting_m, northing_m, height_m)`.
    fn to_cartographic(&self, position: DVec3) -> Option<Cartographic> {
        let a = self.ellipsoid.semi_major_axis();
        let lon = position.x / a;
        let lat = 2.0 * (position.y / a).exp().atan() - std::f64::consts::FRAC_PI_2;
        if !lat.is_finite() {
            return None;
        }
        Some(Cartographic::new(lon, lat, position.z))
    }

    /// Returns `(easting_m, northing_m, height_m)`.
    fn from_cartographic(&self, c: Cartographic) -> DVec3 {
        let a = self.ellipsoid.semi_major_axis();
        let easting = a * c.longitude;
        let northing = a * (std::f64::consts::FRAC_PI_4 + c.latitude / 2.0).tan().ln();
        DVec3::new(easting, northing, c.height)
    }

    fn ellipsoid(&self) -> Option<&Ellipsoid> {
        Some(&self.ellipsoid)
    }
}
