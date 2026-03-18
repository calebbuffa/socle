//! Geometric primitives and spatial algorithms for I3S.
//!
//! Provides bounding volumes (OBB, AABB, sphere), planes, rays,
//! rectangles, frustum culling, intersection tests, and coordinate
//! transforms used by the selection engine.

pub mod aabb;
pub mod culling;
pub mod frustum;
pub mod intersection;
pub mod obb;
pub mod plane;
pub mod ray;
pub mod rectangle;
pub mod sphere;
pub mod transforms;

pub use aabb::AxisAlignedBoundingBox;
pub use culling::CullingResult;
pub use frustum::CullingVolume;
pub use intersection::{
    point_in_triangle_2d, point_in_triangle_3d, ray_aabb, ray_ellipsoid, ray_obb, ray_plane,
    ray_sphere, ray_triangle,
};
pub use obb::OrientedBoundingBox;
pub use plane::Plane;
pub use ray::Ray;
pub use rectangle::Rectangle;
pub use sphere::BoundingSphere;
pub use transforms::{
    Axis, Transforms, X_UP_TO_Y_UP, X_UP_TO_Z_UP, Y_UP_TO_X_UP, Y_UP_TO_Z_UP, Z_UP_TO_X_UP,
    Z_UP_TO_Y_UP,
};
