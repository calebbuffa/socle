//! [`EllipsoidTilesetLoader`] — generates an in-memory 3D Tiles [`Tileset`]
//! that tessellates the surface of an ellipsoid.
//!
//! Mirrors `Cesium3DTilesSelection::EllipsoidTilesetLoader`. Useful as a
//! terrain fallback (smooth globe with no elevation features) and for testing
//! the tile-streaming pipeline without a server.
//!
//! # Example
//!
//! ```
//! use tiles3d_selekt::EllipsoidTilesetLoader;
//! use terra::Ellipsoid;
//!
//! let tileset = EllipsoidTilesetLoader::new(Ellipsoid::wgs84()).create_tileset();
//! assert_eq!(tileset.asset.version, "1.1");
//! // Root has two level-0 children (western / eastern hemisphere).
//! assert_eq!(tileset.root.children.len(), 2);
//! ```

use std::f64::consts::PI;

use terra::{Cartographic, Ellipsoid, calc_quadtree_max_geometric_error};
use tiles3d::{Asset, BoundingVolume, Content, Refine, Tile, Tileset};
use tiles3d::{QuadtreeTileID, QuadtreeTilingScheme};
use zukei::Rectangle;

/// Generates an in-memory [`Tileset`] covering the full globe surface using
/// a quadtree subdivision of the ellipsoid.
///
/// Content URIs are left empty — the tileset is intended for use with a
/// custom content loader that generates tile mesh geometry on the fly (as
/// in the Cesium reference implementation). You can attach URIs or swap in
/// your own geometry source by iterating [`Tileset::for_each_content_mut`].
pub struct EllipsoidTilesetLoader {
    ellipsoid: Ellipsoid,
    tiling_scheme: QuadtreeTilingScheme,
    /// Maximum depth of the generated tile tree (inclusive of level 0).
    max_depth: u32,
}

impl EllipsoidTilesetLoader {
    /// Create a loader for the given ellipsoid.
    ///
    /// The tiling scheme is 2×1 at level 0 (matching Cesium's default), using
    /// a geographic (equirectangular) projection over the full globe rectangle.
    ///
    /// `max_depth` controls how many levels of children are pre-generated in
    /// the returned tileset tree. Pass `0` for root-only (no children),
    /// `1` for one level of subdivision, etc.
    pub fn new(ellipsoid: Ellipsoid) -> Self {
        let rect = Rectangle::new(-PI, -PI / 2.0, PI, PI / 2.0);
        let tiling_scheme = QuadtreeTilingScheme::new(rect, 2, 1);
        Self {
            ellipsoid,
            tiling_scheme,
            max_depth: 2,
        }
    }

    /// Override the maximum pre-generated depth (default: 2).
    pub fn with_max_depth(mut self, max_depth: u32) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// The ellipsoid used for this loader.
    pub fn ellipsoid(&self) -> &Ellipsoid {
        &self.ellipsoid
    }

    /// Build and return the in-memory [`Tileset`].
    pub fn create_tileset(&self) -> Tileset {
        let root_geometric_error = self.tile_geometric_error(0, 0, 0);

        // Dummy root — unconditionally refined (geometric error = ∞).
        let mut root = Tile {
            bounding_volume: self.globe_bounding_volume(),
            geometric_error: f64::MAX,
            refine: Some(Refine::Replace),
            ..Default::default()
        };

        // Level-0 tiles: 2 columns × 1 row.
        for x in 0..self.tiling_scheme.root_tiles_x() {
            if let Some(child) = self.build_tile(QuadtreeTileID::new(0, x, 0), 0) {
                root.children.push(child);
            }
        }

        Tileset {
            asset: Asset {
                version: "1.1".into(),
                ..Default::default()
            },
            geometric_error: root_geometric_error,
            root,
            ..Default::default()
        }
    }

    /// Recursively build a tile and its children up to `self.max_depth`.
    fn build_tile(&self, id: QuadtreeTileID, depth: u32) -> Option<Tile> {
        let rect = self.tiling_scheme.tile_to_rectangle(id)?;

        // Lon/lat rectangle in radians.
        let lon_west = rect.minimum_x;
        let lat_south = rect.minimum_y;
        let lon_east = rect.maximum_x;
        let lat_north = rect.maximum_y;

        let bounding_volume = BoundingVolume {
            region: vec![lon_west, lat_south, lon_east, lat_north, 0.0, 0.0],
            ..Default::default()
        };

        let geometric_error = self.tile_geometric_error(id.level, id.x, id.y);

        // Tile-local origin = center of the tile in ECEF.
        // Vertices in the content loader are stored relative to this origin,
        // and this transform places them back in ECEF world space.
        let center_lon = (lon_west + lon_east) * 0.5;
        let center_lat = (lat_south + lat_north) * 0.5;
        let origin = self
            .ellipsoid
            .cartographic_to_ecef(Cartographic::new(center_lon, center_lat, 0.0));
        // Column-major 4×4 translation matrix.
        let transform = vec![
            1.0, 0.0, 0.0, 0.0, // col 0
            0.0, 1.0, 0.0, 0.0, // col 1
            0.0, 0.0, 1.0, 0.0, // col 2
            origin.x, origin.y, origin.z, 1.0, // col 3
        ];

        let mut tile = Tile {
            bounding_volume,
            geometric_error,
            refine: Some(Refine::Replace),
            transform,
            // Encode bounding rect into content URI so EllipsoidContentLoader
            // can tessellate the patch without a network fetch.
            content: Some(Content {
                uri: format!("{lon_west},{lat_south},{lon_east},{lat_north}"),
                ..Default::default()
            }),
            ..Default::default()
        };

        if depth < self.max_depth {
            for child_id in self.child_ids(id) {
                if let Some(child) = self.build_tile(child_id, depth + 1) {
                    tile.children.push(child);
                }
            }
        }

        Some(tile)
    }

    /// Geometric error for a tile
    fn tile_geometric_error(&self, level: u32, _x: u32, _y: u32) -> f64 {
        let max_err = calc_quadtree_max_geometric_error(&self.ellipsoid);
        // Full-globe angular width at level 0; halved each level.
        let denom = (1u64 << level) as f64;
        max_err * (2.0 * PI) / (self.tiling_scheme.root_tiles_x() as f64 * denom)
    }

    fn child_ids(&self, parent: QuadtreeTileID) -> [QuadtreeTileID; 4] {
        let next = parent.level + 1;
        let px = parent.x * 2;
        let py = parent.y * 2;
        [
            QuadtreeTileID::new(next, px, py),
            QuadtreeTileID::new(next, px + 1, py),
            QuadtreeTileID::new(next, px, py + 1),
            QuadtreeTileID::new(next, px + 1, py + 1),
        ]
    }

    /// A bounding volume covering the full globe.
    fn globe_bounding_volume(&self) -> BoundingVolume {
        BoundingVolume {
            region: vec![-PI, -PI / 2.0, PI, PI / 2.0, 0.0, 0.0],
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_tileset_with_two_root_children() {
        let loader = EllipsoidTilesetLoader::new(Ellipsoid::wgs84());
        let ts = loader.create_tileset();
        assert_eq!(ts.asset.version, "1.1");
        // 2×1 tiling scheme → root has 2 children at level 0.
        assert_eq!(ts.root.children.len(), 2);
    }

    #[test]
    fn root_geometric_error_is_positive() {
        let loader = EllipsoidTilesetLoader::new(Ellipsoid::wgs84());
        let ts = loader.create_tileset();
        assert!(ts.geometric_error > 0.0);
    }

    #[test]
    fn children_subdivide_at_each_level() {
        let loader = EllipsoidTilesetLoader::new(Ellipsoid::wgs84()).with_max_depth(2);
        let ts = loader.create_tileset();
        // Each level-0 tile has 4 children at level 1.
        for child in &ts.root.children {
            assert_eq!(child.children.len(), 4);
        }
    }

    #[test]
    fn geometric_error_decreases_with_depth() {
        let loader = EllipsoidTilesetLoader::new(Ellipsoid::wgs84()).with_max_depth(2);
        let ts = loader.create_tileset();
        let level0_err = ts.root.children[0].geometric_error;
        let level1_err = ts.root.children[0].children[0].geometric_error;
        assert!(level1_err < level0_err);
    }

    #[test]
    fn refine_is_replace() {
        let loader = EllipsoidTilesetLoader::new(Ellipsoid::wgs84()).with_max_depth(1);
        let ts = loader.create_tileset();
        for child in &ts.root.children {
            assert_eq!(child.refine, Some(Refine::Replace));
        }
    }

    #[test]
    fn unit_sphere_has_smaller_error() {
        let wgs84_err =
            EllipsoidTilesetLoader::new(Ellipsoid::wgs84()).tile_geometric_error(0, 0, 0);
        let unit_err =
            EllipsoidTilesetLoader::new(Ellipsoid::unit_sphere()).tile_geometric_error(0, 0, 0);
        assert!(unit_err < wgs84_err);
    }
}
