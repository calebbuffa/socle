use crate::math::{Mat3, Vec2, Vec3};

/// Spatial extent of a node.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpatialBounds {
    /// 2D rectangle (lon/lat or projected).
    Rectangle { min: Vec2, max: Vec2 },
    /// Axis-aligned box in 3D.
    AxisAlignedBox { min: Vec3, max: Vec3 },
    /// Bounding sphere.
    Sphere { center: Vec3, radius: f64 },
    /// Oriented bounding box.
    OrientedBox { center: Vec3, half_axes: Mat3 },
}
