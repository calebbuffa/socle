//! Culling result for visibility tests.

/// Result of testing a bounding volume against a culling volume or plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CullingResult {
    /// Entirely inside the culling volume.
    Inside,
    /// Entirely outside the culling volume.
    Outside,
    /// Partially inside and partially outside.
    Intersecting,
}
