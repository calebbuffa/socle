//! Quadtree and octree tiling schemes.
//!
//! Mirrors `CesiumGeometry::QuadtreeTilingScheme` and
//! `CesiumGeometry::OctreeTilingScheme`.
//!
//! A *tiling scheme* maps tile coordinates `(level, x, y[, z])` to a region
//! of projected space and vice-versa.  The root rectangle/box is divided by
//! `root_tiles_x × root_tiles_y [× root_tiles_z]` at level 0; each
//! subsequent level halves the per-tile extent in every axis.

use glam::{DVec2, DVec3};
use zukei::{AxisAlignedBoundingBox, Rectangle};

use crate::{OctreeTileID, QuadtreeTileID};

/// Defines how a rectangular region of projected space is divided into
/// an implicit quadtree.
///
/// Equivalent to `CesiumGeometry::QuadtreeTilingScheme`.
#[derive(Debug, Clone)]
pub struct QuadtreeTilingScheme {
    /// The overall bounding rectangle in projected coordinates.
    rectangle: Rectangle,
    /// Number of root-level tiles in the X direction.
    root_tiles_x: u32,
    /// Number of root-level tiles in the Y direction.
    root_tiles_y: u32,
}

impl QuadtreeTilingScheme {
    /// Create a new tiling scheme.
    ///
    /// # Panics
    /// Panics if `root_tiles_x` or `root_tiles_y` is zero.
    pub fn new(rectangle: Rectangle, root_tiles_x: u32, root_tiles_y: u32) -> Self {
        assert!(
            root_tiles_x > 0 && root_tiles_y > 0,
            "root tile counts must be positive"
        );
        Self {
            rectangle,
            root_tiles_x,
            root_tiles_y,
        }
    }

    /// The overall bounding rectangle in projected coordinates.
    pub fn rectangle(&self) -> &Rectangle {
        &self.rectangle
    }

    /// Number of root-level tiles in the X direction.
    pub fn root_tiles_x(&self) -> u32 {
        self.root_tiles_x
    }

    /// Number of root-level tiles in the Y direction.
    pub fn root_tiles_y(&self) -> u32 {
        self.root_tiles_y
    }

    /// Total number of tiles in the X direction at `level`.
    pub fn tiles_x_at_level(&self, level: u32) -> u32 {
        self.root_tiles_x << level
    }

    /// Total number of tiles in the Y direction at `level`.
    pub fn tiles_y_at_level(&self, level: u32) -> u32 {
        self.root_tiles_y << level
    }

    /// Return the projected [`Rectangle`] for a given tile.
    ///
    /// Returns `None` if `x` or `y` are out of range for the given `level`.
    pub fn tile_to_rectangle(&self, tile: QuadtreeTileID) -> Option<Rectangle> {
        let nx = self.tiles_x_at_level(tile.level) as f64;
        let ny = self.tiles_y_at_level(tile.level) as f64;
        if tile.x as f64 >= nx || tile.y as f64 >= ny {
            return None;
        }
        let w = self.rectangle.width() / nx;
        let h = self.rectangle.height() / ny;
        let min_x = self.rectangle.minimum_x + tile.x as f64 * w;
        let min_y = self.rectangle.minimum_y + tile.y as f64 * h;
        Some(Rectangle::new(min_x, min_y, min_x + w, min_y + h))
    }

    /// Return the tile ID that contains the given projected position at `level`.
    ///
    /// Returns `None` if the position is outside the root rectangle.
    pub fn position_to_tile(&self, x: f64, y: f64, level: u32) -> Option<QuadtreeTileID> {
        if !self.rectangle.contains(DVec2::new(x, y)) {
            return None;
        }
        let nx = self.tiles_x_at_level(level) as f64;
        let ny = self.tiles_y_at_level(level) as f64;
        let tx = ((x - self.rectangle.minimum_x) / self.rectangle.width() * nx)
            .min(nx - 1.0)
            .max(0.0) as u32;
        let ty = ((y - self.rectangle.minimum_y) / self.rectangle.height() * ny)
            .min(ny - 1.0)
            .max(0.0) as u32;
        Some(QuadtreeTileID::new(level, tx, ty))
    }

    /// Geographic tiling scheme: covers `[-π, -π/2] → [π, π/2]` with a 2×1
    /// root grid (two 90°-wide tiles at the root).
    ///
    /// Used by raster overlays and WGS84 geographic imagery.
    pub fn geographic() -> Self {
        use std::f64::consts::PI;
        Self::new(Rectangle::new(-PI, -PI / 2.0, PI, PI / 2.0), 2, 1)
    }

    /// Web Mercator tiling scheme: covers `[-π, -π] → [π, π]` in easting/
    /// northing-equivalent with a 1×1 root grid.
    pub fn web_mercator() -> Self {
        const HALF_SIZE: f64 = 20_037_508.342_789_244;
        Self::new(
            Rectangle::new(-HALF_SIZE, -HALF_SIZE, HALF_SIZE, HALF_SIZE),
            1,
            1,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn geo() -> QuadtreeTilingScheme {
        QuadtreeTilingScheme::geographic()
    }

    #[test]
    fn tiles_at_level() {
        let scheme = geo();
        assert_eq!(scheme.tiles_x_at_level(0), 2);
        assert_eq!(scheme.tiles_y_at_level(0), 1);
        assert_eq!(scheme.tiles_x_at_level(1), 4);
        assert_eq!(scheme.tiles_y_at_level(1), 2);
    }

    #[test]
    fn tile_to_rectangle_root_east() {
        let scheme = geo();
        let rect = scheme
            .tile_to_rectangle(QuadtreeTileID::new(0, 1, 0))
            .unwrap();
        assert!(
            (rect.minimum_x - 0.0).abs() < 1e-10,
            "west = {}",
            rect.minimum_x
        );
        assert!(
            (rect.maximum_x - PI).abs() < 1e-10,
            "east = {}",
            rect.maximum_x
        );
        assert!((rect.minimum_y + PI / 2.0).abs() < 1e-10);
    }

    #[test]
    fn tile_to_rectangle_out_of_range() {
        let scheme = geo();
        assert!(
            scheme
                .tile_to_rectangle(QuadtreeTileID::new(0, 2, 0))
                .is_none()
        );
    }

    #[test]
    fn position_to_tile_root_west() {
        let scheme = geo();
        // Point in the western half
        let tile = scheme.position_to_tile(-PI / 2.0, 0.0, 0).unwrap();
        assert_eq!(tile, QuadtreeTileID::new(0, 0, 0));
    }

    #[test]
    fn position_to_tile_outside_returns_none() {
        let scheme = geo();
        assert!(scheme.position_to_tile(4.0 * PI, 0.0, 0).is_none());
    }

    #[test]
    fn position_round_trips_through_rectangle() {
        let scheme = geo();
        let tile = QuadtreeTileID::new(3, 5, 2);
        let rect = scheme.tile_to_rectangle(tile).unwrap();
        let mid_x = (rect.minimum_x + rect.maximum_x) / 2.0;
        let mid_y = (rect.minimum_y + rect.maximum_y) / 2.0;
        let back = scheme.position_to_tile(mid_x, mid_y, 3).unwrap();
        assert_eq!(back, tile);
    }
}

// ── OctreeTilingScheme ────────────────────────────────────────────────────────

/// Defines how an axis-aligned box is divided into an implicit octree.
///
/// Equivalent to `CesiumGeometry::OctreeTilingScheme`.
#[derive(Debug, Clone)]
pub struct OctreeTilingScheme {
    /// The overall bounding box in projected / world coordinates.
    bounding_box: AxisAlignedBoundingBox,
    /// Root-level tile count in X.
    root_tiles_x: u32,
    /// Root-level tile count in Y.
    root_tiles_y: u32,
    /// Root-level tile count in Z.
    root_tiles_z: u32,
}

impl OctreeTilingScheme {
    /// Create a new octree tiling scheme.
    ///
    /// # Panics
    /// Panics if any root tile count is zero.
    pub fn new(
        bounding_box: AxisAlignedBoundingBox,
        root_tiles_x: u32,
        root_tiles_y: u32,
        root_tiles_z: u32,
    ) -> Self {
        assert!(
            root_tiles_x > 0 && root_tiles_y > 0 && root_tiles_z > 0,
            "root tile counts must be positive"
        );
        Self {
            bounding_box,
            root_tiles_x,
            root_tiles_y,
            root_tiles_z,
        }
    }

    /// The overall bounding box.
    pub fn bounding_box(&self) -> &AxisAlignedBoundingBox {
        &self.bounding_box
    }

    pub fn root_tiles_x(&self) -> u32 {
        self.root_tiles_x
    }
    pub fn root_tiles_y(&self) -> u32 {
        self.root_tiles_y
    }
    pub fn root_tiles_z(&self) -> u32 {
        self.root_tiles_z
    }

    /// Total tiles in the X direction at `level`.
    pub fn tiles_x_at_level(&self, level: u32) -> u32 {
        self.root_tiles_x << level
    }
    /// Total tiles in the Y direction at `level`.
    pub fn tiles_y_at_level(&self, level: u32) -> u32 {
        self.root_tiles_y << level
    }
    /// Total tiles in the Z direction at `level`.
    pub fn tiles_z_at_level(&self, level: u32) -> u32 {
        self.root_tiles_z << level
    }

    /// Return the AABB for `tile`.
    ///
    /// Returns `None` when any coordinate is out of range for the level.
    pub fn tile_to_box(&self, tile: OctreeTileID) -> Option<AxisAlignedBoundingBox> {
        let nx = self.tiles_x_at_level(tile.level) as f64;
        let ny = self.tiles_y_at_level(tile.level) as f64;
        let nz = self.tiles_z_at_level(tile.level) as f64;
        if tile.x as f64 >= nx || tile.y as f64 >= ny || tile.z as f64 >= nz {
            return None;
        }
        let size = self.bounding_box.max - self.bounding_box.min;
        let tw = DVec3::new(size.x / nx, size.y / ny, size.z / nz);
        let min = self.bounding_box.min
            + DVec3::new(
                tile.x as f64 * tw.x,
                tile.y as f64 * tw.y,
                tile.z as f64 * tw.z,
            );
        Some(AxisAlignedBoundingBox::new(min, min + tw))
    }

    /// Return the tile that contains `position` at `level`.
    ///
    /// Returns `None` when the position is outside the bounding box.
    pub fn position_to_tile(&self, position: DVec3, level: u32) -> Option<OctreeTileID> {
        if !self.bounding_box.contains(position) {
            return None;
        }
        let size = self.bounding_box.max - self.bounding_box.min;
        let nx = self.tiles_x_at_level(level) as f64;
        let ny = self.tiles_y_at_level(level) as f64;
        let nz = self.tiles_z_at_level(level) as f64;
        let rel = position - self.bounding_box.min;
        let tx = ((rel.x / size.x * nx).min(nx - 1.0).max(0.0)) as u32;
        let ty = ((rel.y / size.y * ny).min(ny - 1.0).max(0.0)) as u32;
        let tz = ((rel.z / size.z * nz).min(nz - 1.0).max(0.0)) as u32;
        Some(OctreeTileID {
            level,
            x: tx,
            y: ty,
            z: tz,
        })
    }
}

#[cfg(test)]
mod oct_tests {
    use super::*;
    use glam::DVec3;

    fn unit_scheme() -> OctreeTilingScheme {
        OctreeTilingScheme::new(
            AxisAlignedBoundingBox::new(DVec3::ZERO, DVec3::ONE),
            1,
            1,
            1,
        )
    }

    #[test]
    fn tiles_at_level_octree() {
        let s = unit_scheme();
        assert_eq!(s.tiles_x_at_level(0), 1);
        assert_eq!(s.tiles_x_at_level(1), 2);
        assert_eq!(s.tiles_z_at_level(2), 4);
    }

    #[test]
    fn tile_to_box_root() {
        let s = unit_scheme();
        let b = s
            .tile_to_box(OctreeTileID {
                level: 0,
                x: 0,
                y: 0,
                z: 0,
            })
            .unwrap();
        assert!((b.min - DVec3::ZERO).length() < 1e-12);
        assert!((b.max - DVec3::ONE).length() < 1e-12);
    }

    #[test]
    fn tile_to_box_level1() {
        let s = unit_scheme();
        // Level 1→ 2×2×2 grid; tile (1,1,1,1) = high-octant [0.5..1.0]³
        let b = s
            .tile_to_box(OctreeTileID {
                level: 1,
                x: 1,
                y: 1,
                z: 1,
            })
            .unwrap();
        assert!((b.min - DVec3::splat(0.5)).length() < 1e-12);
        assert!((b.max - DVec3::ONE).length() < 1e-12);
    }

    #[test]
    fn tile_to_box_out_of_range() {
        let s = unit_scheme();
        assert!(
            s.tile_to_box(OctreeTileID {
                level: 0,
                x: 1,
                y: 0,
                z: 0
            })
            .is_none()
        );
    }

    #[test]
    fn position_to_tile_root() {
        let s = unit_scheme();
        let tid = s.position_to_tile(DVec3::splat(0.5), 0).unwrap();
        assert_eq!(
            tid,
            OctreeTileID {
                level: 0,
                x: 0,
                y: 0,
                z: 0
            }
        );
    }

    #[test]
    fn position_to_tile_level1_high_octant() {
        let s = unit_scheme();
        let tid = s.position_to_tile(DVec3::splat(0.75), 1).unwrap();
        assert_eq!(
            tid,
            OctreeTileID {
                level: 1,
                x: 1,
                y: 1,
                z: 1
            }
        );
    }

    #[test]
    fn position_to_tile_outside() {
        let s = unit_scheme();
        assert!(s.position_to_tile(DVec3::splat(2.0), 0).is_none());
    }

    #[test]
    fn position_round_trips_through_box() {
        let s = unit_scheme();
        let tile = OctreeTileID {
            level: 2,
            x: 3,
            y: 1,
            z: 2,
        };
        let b = s.tile_to_box(tile).unwrap();
        let mid = (b.min + b.max) * 0.5;
        let back = s.position_to_tile(mid, 2).unwrap();
        assert_eq!(back, tile);
    }
}
