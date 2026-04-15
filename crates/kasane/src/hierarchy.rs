//! Minimal spatial hierarchy trait for overlay resolution.
//!
//! [`OverlayHierarchy`] describes just enough structure for the overlay engine
//! to walk parent chains and compute geographic extents.  It is intentionally
//! decoupled from any tile selection engine — the composition layer (e.g.
//! `kiban`) bridges a full `SceneGraph` into this trait.

/// Read-only spatial hierarchy used by the overlay engine.
///
/// Only two capabilities are required:
/// - Walk up the tree via [`parent`](Self::parent).
/// - Obtain the geographic extent of a node via [`globe_rectangle`](Self::globe_rectangle).
pub trait OverlayHierarchy: Send + Sync {
    /// Returns the parent of `node`, or `None` if it is a root.
    fn parent(&self, node: u64) -> Option<u64>;

    /// Geographic extent of this node in geodetic longitude/latitude (radians).
    fn globe_rectangle(&self, node: u64) -> Option<terra::GlobeRectangle>;
}
