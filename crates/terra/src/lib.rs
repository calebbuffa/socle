//! `terra` — geospatial primitives for any coordinate reference system.
//!
//! `terra` models [`CesiumGeospatial`] functionality in a format-agnostic way,
//! unifying the Earth-Centred Earth-Fixed (ECEF) world-space used by
//! **3D Tiles** and the arbitrary projected / geographic CRS used by **I3S**.
//!
//! # Key types
//!
//! | Type | Purpose |
//! |------|---------|
//! | [`Ellipsoid`] | Mathematical ellipsoid model (WGS84, unit sphere, …) |
//! | [`Cartographic`] | Geodetic position (lon/lat radians, height metres) |
//! | [`GlobeRectangle`] | Axis-aligned geodetic rectangle (radians) |
//! | [`BoundingRegion`] | 3-D geodetic bounding volume (rectangle + heights) |
//! | [`crs::Crs`] | Trait: convert any CRS ↔ geodetic |
//! | [`crs::EcefCrs`] | ECEF CRS — EPSG:4978 (3D Tiles world space) |
//! | [`crs::GeographicCrs`] | Geographic lon/lat — EPSG:4326 |
//! | [`crs::WebMercatorCrs`] | Web Mercator — EPSG:3857 |
//! | [`SpatialReference`] | WKID/WKT reference (I3S bridge) |
//! | [`transforms`] | ENU / NED local frame matrices in ECEF |
//!
//! [`CesiumGeospatial`]: https://cesium.com/learn/cesiumjs/ref-doc/module-CesiumGeospatial.html

mod bounding_region;
mod cartographic;
mod crs;
mod ellipsoid;
mod globe;
mod lhcs;
mod projection;
mod sr;
mod transforms;

pub use bounding_region::{
    BoundingRegion, BoundingRegionBuilder, BoundingRegionWithLooseFittingHeights,
};
pub use cartographic::{Cartographic, CartographicPolygon};
pub use crs::{Crs, EcefCrs, GeographicCrs, WebMercatorCrs};
pub use ellipsoid::{Ellipsoid, EllipsoidTangentPlane, SimplePlanarEllipsoidCurve};
pub use globe::{GlobeAnchor, GlobeRectangle};
pub use lhcs::{LocalDirection, LocalHorizontalCoordinateSystem};
pub use projection::{GeographicProjection, WebMercatorProjection};
pub use sr::{CrsRegistry, SpatialReference};
pub use transforms::{east_north_up_to_ecef, enu_quaternion};

/// Maximum geometric error per radian for a quadtree tile, assuming a 65×65
/// heightmap with 25 % vertical error relative to horizontal sample spacing
/// at the equator.
///
/// Mirrors `CesiumGeospatial::calcQuadtreeMaxGeometricError`.
///
/// Multiply by the tile's angular width in radians to get the tile error.
#[inline]
pub fn calc_quadtree_max_geometric_error(ellipsoid: &Ellipsoid) -> f64 {
    ellipsoid.maximum_radius() * 0.25 / 65.0
}
