//! `zukei` — low-level FFI-safe math and bounds primitives.
//!
//! This crate is the shared ownership point for foundational geometry values used
//! across the runtime stack. It intentionally stays below traversal, selection,
//! loading, and geospatial policy layers.

#[cfg(feature = "glam")]
pub mod aabb;
pub mod bounds;
#[cfg(feature = "glam")]
pub mod culling;
#[cfg(feature = "glam")]
pub mod frustum;
#[cfg(feature = "glam")]
pub mod glam;
#[cfg(feature = "glam")]
pub mod intersection;
pub mod math;
#[cfg(feature = "glam")]
pub mod obb;
#[cfg(feature = "glam")]
pub mod plane;
#[cfg(feature = "glam")]
pub mod ray;
#[cfg(feature = "glam")]
pub mod rectangle;
#[cfg(feature = "glam")]
pub mod sphere;
#[cfg(feature = "glam")]
pub mod transforms;

#[cfg(feature = "glam")]
pub use aabb::AxisAlignedBoundingBox;
pub use bounds::SpatialBounds;
#[cfg(feature = "glam")]
pub use culling::CullingResult;
#[cfg(feature = "glam")]
pub use frustum::CullingVolume;
#[cfg(feature = "glam")]
pub use intersection::{
    point_in_triangle_2d, point_in_triangle_3d, ray_aabb, ray_ellipsoid, ray_obb, ray_plane,
    ray_sphere, ray_triangle,
};
pub use math::{Mat3, Mat4, Vec2, Vec3, Vec4};
#[cfg(feature = "glam")]
pub use obb::OrientedBoundingBox;
#[cfg(feature = "glam")]
pub use plane::Plane;
#[cfg(feature = "glam")]
pub use ray::Ray;
#[cfg(feature = "glam")]
pub use rectangle::Rectangle;
#[cfg(feature = "glam")]
pub use sphere::BoundingSphere;
#[cfg(feature = "glam")]
pub use transforms::{
    Axis, Transforms, X_UP_TO_Y_UP, X_UP_TO_Z_UP, Y_UP_TO_X_UP, Y_UP_TO_Z_UP, Z_UP_TO_X_UP,
    Z_UP_TO_Y_UP,
};
