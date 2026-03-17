//! Geospatial coordinate systems and ellipsoid math for I3S.
//!
//! Provides WGS84 ellipsoid, cartographic ↔ Cartesian (ECEF) transforms,
//! map projections, CRS classification, and globe-aware bounding regions.

pub mod bounding_region;
pub mod cartographic;
pub mod crs;
pub mod ellipsoid;
pub mod globe_rectangle;
pub mod projection;
pub mod transforms;

pub use bounding_region::BoundingRegion;
pub use cartographic::Cartographic;
pub use crs::{CrsTransform, SceneCoordinateSystem, WkidTransform};
pub use ellipsoid::Ellipsoid;
pub use globe_rectangle::GlobeRectangle;
