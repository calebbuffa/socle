//! Geospatial coordinate systems and ellipsoid math for I3S.
//!
//! Provides WGS84 ellipsoid, cartographic ↔ Cartesian (ECEF) transforms,
//! map projections, CRS classification, and globe-aware bounding regions.

pub mod bounding_region;
pub mod cartographic;
pub mod crs;
pub mod ellipsoid;
pub mod globe_rectangle;
pub mod local_horizontal_cs;
pub mod projection;
pub mod transforms;

pub use bounding_region::BoundingRegion;
pub use cartographic::Cartographic;
pub use crs::{CrsTransform, SceneCoordinateSystem, WkidTransform};
pub use ellipsoid::Ellipsoid;
pub use globe_rectangle::GlobeRectangle;
pub use i3s_geometry::culling::CullingResult;
pub use i3s_geometry::plane::Plane;
pub use local_horizontal_cs::{LocalDirection, LocalHorizontalCoordinateSystem};
pub use projection::{
    TransverseMercatorParams, from_geographic_degrees, from_transverse_mercator, from_web_mercator,
    to_geographic, to_transverse_mercator, to_web_mercator,
};
pub use transforms::{enu_frame, enu_matrix_at};
