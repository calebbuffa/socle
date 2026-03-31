//! Helper functions for 3D Tiles implicit tiling.
//!
//! Mirrors `Cesium3DTilesContent::ImplicitTilingUtilities`.

use crate::tile::{OctreeTileID, QuadtreeTileID};
use kiban::resolve_url;

/// Denominator for a given implicit tile level.
///
/// Divide the root tile's geometric error by this value to get the
/// geometric error for tiles on `level`. Divide each axis of a bounding
/// volume by this factor to get the child tile size at that level.
///
/// Equivalent to `2^level`.
#[inline]
pub fn compute_level_denominator(level: u32) -> f64 {
    (1u64 << level) as f64
}

/// Compute the absolute Morton index for a quadtree tile at its level.
///
/// The Morton (Z-order) index interleaves the bits of `x` and `y`.
pub fn compute_morton_index_quad(tile: QuadtreeTileID) -> u64 {
    spread_bits_2d(tile.x) | (spread_bits_2d(tile.y) << 1)
}

/// Compute the absolute Morton index for an octree tile at its level.
///
/// Interleaves the bits of `x`, `y`, and `z`.
pub fn compute_morton_index_oct(tile: OctreeTileID) -> u64 {
    spread_bits_3d(tile.x) | (spread_bits_3d(tile.y) << 1) | (spread_bits_3d(tile.z) << 2)
}

/// Morton index of `tile` relative to the subtree rooted at `subtree_root`.
pub fn compute_relative_morton_index_quad(
    subtree_root: QuadtreeTileID,
    tile: QuadtreeTileID,
) -> u64 {
    let rel = absolute_to_relative_quad(subtree_root, tile);
    compute_morton_index_quad(rel)
}

/// Morton index of `tile` relative to the subtree rooted at `subtree_root`.
pub fn compute_relative_morton_index_oct(subtree_root: OctreeTileID, tile: OctreeTileID) -> u64 {
    let rel = absolute_to_relative_oct(subtree_root, tile);
    compute_morton_index_oct(rel)
}

/// Convert an absolute tile ID to one relative to `root`.
///
/// If `root == tile` the result is `{ level: 0, x: 0, y: 0 }`.
pub fn absolute_to_relative_quad(root: QuadtreeTileID, tile: QuadtreeTileID) -> QuadtreeTileID {
    let relative_level = tile.level - root.level;
    QuadtreeTileID::new(
        relative_level,
        tile.x - (root.x << relative_level),
        tile.y - (root.y << relative_level),
    )
}

/// Convert an absolute tile ID to one relative to `root`.
pub fn absolute_to_relative_oct(root: OctreeTileID, tile: OctreeTileID) -> OctreeTileID {
    let relative_level = tile.level - root.level;
    OctreeTileID::new(
        relative_level,
        tile.x - (root.x << relative_level),
        tile.y - (root.y << relative_level),
        tile.z - (root.z << relative_level),
    )
}

/// Return the root tile of the subtree that contains `tile`.
///
/// `subtree_levels` is the number of levels in each subtree (the
/// `subtreeLevels` field in the `ImplicitTiling` JSON object).
pub fn get_subtree_root_quad(subtree_levels: u32, tile: QuadtreeTileID) -> QuadtreeTileID {
    let subtree_level = tile.level / subtree_levels;
    let levels_left = tile.level % subtree_levels;
    QuadtreeTileID::new(
        subtree_level * subtree_levels,
        tile.x >> levels_left,
        tile.y >> levels_left,
    )
}

/// Return the root tile of the subtree that contains `tile`.
pub fn get_subtree_root_oct(subtree_levels: u32, tile: OctreeTileID) -> OctreeTileID {
    let subtree_level = tile.level / subtree_levels;
    let levels_left = tile.level % subtree_levels;
    OctreeTileID::new(
        subtree_level * subtree_levels,
        tile.x >> levels_left,
        tile.y >> levels_left,
        tile.z >> levels_left,
    )
}

/// Resolve a 3D Tiles implicit tiling URL template for a quadtree tile.
///
/// Replaces `{level}`, `{x}`, and `{y}` in `url_template`, then joins the
/// result against `base_url` (relative paths are resolved relative to
/// `base_url`'s directory).
pub fn resolve_url_quad(base_url: &str, url_template: &str, tile: QuadtreeTileID) -> String {
    let expanded = url_template
        .replace("{level}", &tile.level.to_string())
        .replace("{x}", &tile.x.to_string())
        .replace("{y}", &tile.y.to_string());
    resolve_url(base_url, &expanded)
}

/// Resolve a 3D Tiles implicit tiling URL template for an octree tile.
///
/// Replaces `{level}`, `{x}`, `{y}`, and `{z}`.
pub fn resolve_url_oct(base_url: &str, url_template: &str, tile: OctreeTileID) -> String {
    let expanded = url_template
        .replace("{level}", &tile.level.to_string())
        .replace("{x}", &tile.x.to_string())
        .replace("{y}", &tile.y.to_string())
        .replace("{z}", &tile.z.to_string());
    resolve_url(base_url, &expanded)
}
/// Spread the bits of a 32-bit integer into the even bits of a 64-bit word,
/// leaving zeros in the odd bits.  Used for 2D Morton (Z-order) encoding.
///
/// For n bits of input the output occupies bits 0, 2, 4, … 2*(n-1).
/// The implementation handles up to 32-bit inputs (64-bit output is safe for
/// up to level 32 of a quadtree before the coords overflow u32 anyway).
fn spread_bits_2d(n: u32) -> u64 {
    let mut x = n as u64;
    x = (x | (x << 16)) & 0x0000_FFFF_0000_FFFF;
    x = (x | (x << 8)) & 0x00FF_00FF_00FF_00FF;
    x = (x | (x << 4)) & 0x0F0F_0F0F_0F0F_0F0F;
    x = (x | (x << 2)) & 0x3333_3333_3333_3333;
    x = (x | (x << 1)) & 0x5555_5555_5555_5555;
    x
}

/// Spread the bits of a 21-bit integer into every third bit of a 64-bit word.
/// The output occupies bits 0, 3, 6, … 60.  Handles octree levels 0–21.
fn spread_bits_3d(n: u32) -> u64 {
    // Limit to 21 bits — octree coordinates at level 21 are at most 2^21-1.
    let mut x = (n & 0x001F_FFFF) as u64;
    x = (x | (x << 32)) & 0x001F_0000_0000_FFFF;
    x = (x | (x << 16)) & 0x001F_0000_FF00_00FF;
    x = (x | (x << 8)) & 0x100F_00F0_0F00_F00F;
    x = (x | (x << 4)) & 0x10C3_0C30_C30C_30C3;
    x = (x | (x << 2)) & 0x1249_2492_4924_9249;
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_denominator() {
        assert_eq!(compute_level_denominator(0), 1.0);
        assert_eq!(compute_level_denominator(1), 2.0);
        assert_eq!(compute_level_denominator(4), 16.0);
    }

    #[test]
    fn morton_quad_root() {
        let id = QuadtreeTileID::new(0, 0, 0);
        assert_eq!(compute_morton_index_quad(id), 0);
    }

    #[test]
    fn morton_quad_level1() {
        // Morton index interleaves x (LSB) and y: x=bit0, y=bit1.
        // (x=0,y=0)→0, (x=1,y=0)→1, (x=0,y=1)→2, (x=1,y=1)→3
        let expected: &[(u64, u32, u32)] = &[(0, 0, 0), (1, 1, 0), (2, 0, 1), (3, 1, 1)];
        for &(want, x, y) in expected {
            let id = QuadtreeTileID::new(1, x, y);
            assert_eq!(compute_morton_index_quad(id), want, "x={x} y={y}");
        }
    }

    #[test]
    fn morton_oct_root() {
        let id = OctreeTileID::new(0, 0, 0, 0);
        assert_eq!(compute_morton_index_oct(id), 0);
    }

    #[test]
    fn morton_oct_level1() {
        // 8 children; Morton index is the 3-bit interleaved index of (z,y,x).
        for i in 0u32..8 {
            let x = i & 1;
            let y = (i >> 1) & 1;
            let z = i >> 2;
            let id = OctreeTileID::new(1, x, y, z);
            assert_eq!(
                compute_morton_index_oct(id),
                i as u64,
                "i={i} x={x} y={y} z={z}"
            );
        }
    }

    #[test]
    fn test_get_subtree_root_quad() {
        // Subtree levels = 4: tiles 0-3 in first subtree, 4-7 in next.
        let tile = QuadtreeTileID::new(5, 6, 7);
        let root = get_subtree_root_quad(4, tile);
        assert_eq!(root.level, 4);
        assert_eq!(root.x, 6 >> 1); // 5 % 4 = 1 level below subtree root
        assert_eq!(root.y, 7 >> 1);
    }

    #[test]
    fn test_get_subtree_root_oct() {
        let tile = OctreeTileID::new(5, 6, 7, 4);
        let root = get_subtree_root_oct(4, tile);
        assert_eq!(root.level, 4);
        assert_eq!(root.x, 6 >> 1);
        assert_eq!(root.z, 4 >> 1);
    }

    #[test]
    fn test_absolute_to_relative_quad() {
        let id = QuadtreeTileID::new(3, 5, 6);
        let rel = absolute_to_relative_quad(id, id);
        assert_eq!(rel, QuadtreeTileID::new(0, 0, 0));
    }

    #[test]
    fn test_resolve_url_quad() {
        let url = resolve_url_quad(
            "https://example.com/tileset/tileset.json",
            "subtrees/{level}/{x}/{y}.subtree",
            QuadtreeTileID::new(3, 5, 2),
        );
        assert_eq!(url, "https://example.com/tileset/subtrees/3/5/2.subtree");
    }

    #[test]
    fn test_resolve_url_oct() {
        let url = resolve_url_oct(
            "https://example.com/tileset.json",
            "subtrees/{level}/{x}/{y}/{z}.subtree",
            OctreeTileID::new(2, 1, 3, 0),
        );
        assert_eq!(url, "https://example.com/subtrees/2/1/3/0.subtree");
    }

    #[test]
    fn test_resolve_url_absolute_passthrough() {
        let url = resolve_url_quad(
            "https://example.com/tileset.json",
            "https://cdn.example.com/subtrees/{level}/{x}/{y}.subtree",
            QuadtreeTileID::new(0, 0, 0),
        );
        assert!(url.starts_with("https://cdn.example.com/"));
    }
}
