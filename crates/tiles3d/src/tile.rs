//! Virtual child-tile iterators for implicit quadtrees and octrees.
//!
//! Mirrors `Cesium3DTilesContent::QuadtreeChildren` and `OctreeChildren`.
//! Neither type allocates — they compute child IDs on the fly from the parent.

// ── Quadtree ─────────────────────────────────────────────────────────────────

/// A lazy, non-allocating container that yields the four child
/// [`QuadtreeTileID`]s of a given parent tile.
///
/// Children are ordered: `(x*2, y*2)`, `(x*2+1, y*2)`, `(x*2, y*2+1)`,
/// `(x*2+1, y*2+1)` — matching Cesium's `QuadtreeChildren`.
#[derive(Debug, Clone, Copy)]
pub struct QuadtreeChildren {
    parent: QuadtreeTileID,
}

impl QuadtreeChildren {
    pub(crate) fn new(parent: QuadtreeTileID) -> Self {
        Self { parent }
    }

    /// Always 4.
    pub const fn len(&self) -> usize {
        4
    }

    /// Never empty.
    pub const fn is_empty(&self) -> bool {
        false
    }
}

/// Iterator over the four children of a quadtree tile.
pub struct QuadtreeChildrenIter {
    parent: QuadtreeTileID,
    index: u32,
}

impl Iterator for QuadtreeChildrenIter {
    type Item = QuadtreeTileID;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= 4 {
            return None;
        }
        let i = self.index;
        self.index += 1;
        Some(QuadtreeTileID::new(
            self.parent.level + 1,
            self.parent.x * 2 + (i & 1),
            self.parent.y * 2 + (i >> 1),
        ))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (4 - self.index) as usize;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for QuadtreeChildrenIter {}

impl IntoIterator for QuadtreeChildren {
    type Item = QuadtreeTileID;
    type IntoIter = QuadtreeChildrenIter;

    fn into_iter(self) -> Self::IntoIter {
        QuadtreeChildrenIter {
            parent: self.parent,
            index: 0,
        }
    }
}

impl<'a> IntoIterator for &'a QuadtreeChildren {
    type Item = QuadtreeTileID;
    type IntoIter = QuadtreeChildrenIter;

    fn into_iter(self) -> Self::IntoIter {
        QuadtreeChildrenIter {
            parent: self.parent,
            index: 0,
        }
    }
}

// ── Octree ───────────────────────────────────────────────────────────────────

/// A lazy, non-allocating container that yields the eight child
/// [`OctreeTileID`]s of a given parent tile.
///
/// Children are ordered by `(dx, dy, dz)` in bit order 0–7:
/// `(0,0,0)`, `(1,0,0)`, `(0,1,0)`, `(1,1,0)`, `(0,0,1)` … `(1,1,1)`.
#[derive(Debug, Clone, Copy)]
pub struct OctreeChildren {
    parent: OctreeTileID,
}

impl OctreeChildren {
    pub(crate) fn new(parent: OctreeTileID) -> Self {
        Self { parent }
    }

    /// Always 8.
    pub const fn len(&self) -> usize {
        8
    }

    /// Never empty.
    pub const fn is_empty(&self) -> bool {
        false
    }
}

/// Iterator over the eight children of an octree tile.
pub struct OctreeChildrenIter {
    parent: OctreeTileID,
    index: u32,
}

impl Iterator for OctreeChildrenIter {
    type Item = OctreeTileID;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= 8 {
            return None;
        }
        let i = self.index;
        self.index += 1;
        Some(OctreeTileID::new(
            self.parent.level + 1,
            self.parent.x * 2 + (i & 1),
            self.parent.y * 2 + ((i >> 1) & 1),
            self.parent.z * 2 + (i >> 2),
        ))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (8 - self.index) as usize;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for OctreeChildrenIter {}

impl IntoIterator for OctreeChildren {
    type Item = OctreeTileID;
    type IntoIter = OctreeChildrenIter;

    fn into_iter(self) -> Self::IntoIter {
        OctreeChildrenIter {
            parent: self.parent,
            index: 0,
        }
    }
}

impl<'a> IntoIterator for &'a OctreeChildren {
    type Item = OctreeTileID;
    type IntoIter = OctreeChildrenIter;

    fn into_iter(self) -> Self::IntoIter {
        OctreeChildrenIter {
            parent: self.parent,
            index: 0,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────

/// Unique identifier for a tile in an implicit quadtree.
///
/// The root tile is `{ level: 0, x: 0, y: 0 }`.  At each subsequent level
/// the tile count doubles in both axes, so a tile at level `L` has spatial
/// extent `1/2^L` of the root bounding volume along each axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct QuadtreeTileID {
    /// Depth from the root (0 = root).
    pub level: u32,
    /// Column within the level-grid.  Range: `[0, 2^level)`.
    pub x: u32,
    /// Row within the level-grid.  Range: `[0, 2^level)`.
    pub y: u32,
}

impl QuadtreeTileID {
    #[inline]
    pub const fn new(level: u32, x: u32, y: u32) -> Self {
        Self { level, x, y }
    }

    /// Return the parent tile ID, or `None` if this is the root.
    #[inline]
    pub const fn parent(self) -> Option<Self> {
        if self.level == 0 {
            None
        } else {
            Some(Self::new(self.level - 1, self.x >> 1, self.y >> 1))
        }
    }

    /// Return the four children of this tile as an iterable.
    #[inline]
    pub fn children(self) -> QuadtreeChildren {
        QuadtreeChildren::new(self)
    }
}

impl std::fmt::Display for QuadtreeTileID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}/{}", self.level, self.x, self.y)
    }
}

/// Unique identifier for a tile in an implicit octree.
///
/// The root tile is `{ level: 0, x: 0, y: 0, z: 0 }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct OctreeTileID {
    /// Depth from the root (0 = root).
    pub level: u32,
    /// X index within the level-grid.  Range: `[0, 2^level)`.
    pub x: u32,
    /// Y index within the level-grid.  Range: `[0, 2^level)`.
    pub y: u32,
    /// Z index within the level-grid.  Range: `[0, 2^level)`.
    pub z: u32,
}

impl OctreeTileID {
    #[inline]
    pub const fn new(level: u32, x: u32, y: u32, z: u32) -> Self {
        Self { level, x, y, z }
    }

    /// Return the parent tile ID, or `None` if this is the root.
    #[inline]
    pub const fn parent(self) -> Option<Self> {
        if self.level == 0 {
            None
        } else {
            Some(Self::new(
                self.level - 1,
                self.x >> 1,
                self.y >> 1,
                self.z >> 1,
            ))
        }
    }

    /// Return the eight children of this tile as an iterable.
    #[inline]
    pub fn children(self) -> OctreeChildren {
        OctreeChildren::new(self)
    }
}

impl std::fmt::Display for OctreeTileID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}/{}/{}", self.level, self.x, self.y, self.z)
    }
}

// ─────────────────────────────────────────────────────────────────────────────

use glam::DMat4;

use crate::generated::Tile;

/// Functions for reading and writing a [`Tile`]'s transform.
pub struct TileTransform;

impl TileTransform {
    /// Parse the tile's `transform` array into a [`DMat4`].
    ///
    /// Returns `None` if the array has fewer than 16 elements.
    /// Extra elements beyond index 15 are silently ignored.
    pub fn get_transform(tile: &Tile) -> Option<DMat4> {
        let a = &tile.transform;
        if a.len() < 16 {
            return None;
        }
        Some(DMat4::from_cols_array(&[
            a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], a[8], a[9], a[10], a[11], a[12], a[13],
            a[14], a[15],
        ]))
    }

    /// Write a [`DMat4`] into a tile's `transform` array, replacing any
    /// existing value.
    pub fn set_transform(tile: &mut Tile, transform: DMat4) {
        let a = transform.to_cols_array();
        tile.transform = a.to_vec();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{DMat4, DVec3};

    fn tile_with_identity() -> Tile {
        let mut tile = Tile::default();
        TileTransform::set_transform(&mut tile, DMat4::IDENTITY);
        tile
    }

    #[test]
    fn identity_round_trip() {
        let tile = tile_with_identity();
        let m = TileTransform::get_transform(&tile).unwrap();
        assert!((m - DMat4::IDENTITY).abs_diff_eq(DMat4::ZERO, 1e-15));
    }

    #[test]
    fn get_transform_none_on_empty() {
        let tile = Tile::default();
        assert!(TileTransform::get_transform(&tile).is_none());
    }

    #[test]
    fn translation_round_trip() {
        let t = DMat4::from_translation(DVec3::new(100.0, 200.0, 300.0));
        let mut tile = Tile::default();
        TileTransform::set_transform(&mut tile, t);
        let back = TileTransform::get_transform(&tile).unwrap();
        assert!((back.w_axis.truncate() - DVec3::new(100.0, 200.0, 300.0)).length() < 1e-10);
    }
}

use glam::DVec3;
use terra::BoundingRegion;
use zukei::{BoundingSphere, OrientedBoundingBox};

use crate::generated::BoundingVolume;

/// Functions for extracting and setting typed bounding volumes on
/// [`BoundingVolume`] values.
pub struct TileBoundingVolumes;

impl TileBoundingVolumes {
    /// Parse the `box` field of a [`BoundingVolume`] into an
    /// [`OrientedBoundingBox`].
    ///
    /// Returns `None` if `bounding_volume.box` has fewer than 12 elements.
    pub fn get_oriented_bounding_box(
        bounding_volume: &BoundingVolume,
    ) -> Option<OrientedBoundingBox> {
        OrientedBoundingBox::from_3dtiles_box(&bounding_volume.r#box)
    }

    /// Write an [`OrientedBoundingBox`] into the `box` field of a
    /// [`BoundingVolume`], replacing any existing value.
    pub fn set_oriented_bounding_box(
        bounding_volume: &mut BoundingVolume,
        obb: OrientedBoundingBox,
    ) {
        let arr = obb.to_3dtiles_box();
        bounding_volume.r#box = arr.to_vec();
    }

    /// Parse the `region` field of a [`BoundingVolume`] into a
    /// [`BoundingRegion`].
    ///
    /// The six floats are `[west_rad, south_rad, east_rad, north_rad,
    /// min_height_m, max_height_m]`.
    ///
    /// Returns `None` if `bounding_volume.region` has fewer than 6 elements.
    pub fn get_bounding_region(bounding_volume: &BoundingVolume) -> Option<BoundingRegion> {
        BoundingRegion::from_3dtiles_region(&bounding_volume.region)
    }

    /// Write a [`BoundingRegion`] into the `region` field of a
    /// [`BoundingVolume`], replacing any existing value.
    pub fn set_bounding_region(bounding_volume: &mut BoundingVolume, region: BoundingRegion) {
        bounding_volume.region = region.to_3dtiles_region().to_vec();
    }

    /// Parse the `sphere` field of a [`BoundingVolume`] into a
    /// [`BoundingSphere`].
    ///
    /// The four floats are `[cx, cy, cz, radius]`.
    ///
    /// Returns `None` if `bounding_volume.sphere` has fewer than 4 elements.
    pub fn get_bounding_sphere(bounding_volume: &BoundingVolume) -> Option<BoundingSphere> {
        let s = &bounding_volume.sphere;
        if s.len() < 4 {
            return None;
        }
        Some(BoundingSphere::new(DVec3::new(s[0], s[1], s[2]), s[3]))
    }

    /// Write a [`BoundingSphere`] into the `sphere` field of a
    /// [`BoundingVolume`], replacing any existing value.
    pub fn set_bounding_sphere(bounding_volume: &mut BoundingVolume, sphere: BoundingSphere) {
        bounding_volume.sphere = vec![
            sphere.center.x,
            sphere.center.y,
            sphere.center.z,
            sphere.radius,
        ];
    }
}

#[cfg(test)]
mod bounding_volume_tests {
    use super::*;

    fn obb_bv() -> BoundingVolume {
        // 1 m half-cube centred at origin, identity orientation.
        BoundingVolume {
            r#box: vec![
                0.0, 0.0, 0.0, // centre
                1.0, 0.0, 0.0, // half-axis X
                0.0, 1.0, 0.0, // half-axis Y
                0.0, 0.0, 1.0, // half-axis Z
            ],
            ..Default::default()
        }
    }

    #[test]
    fn get_oriented_bounding_box_round_trip() {
        let mut bv = obb_bv();
        let obb = TileBoundingVolumes::get_oriented_bounding_box(&bv).unwrap();
        assert!((obb.center - DVec3::ZERO).length() < 1e-10);
        assert!((obb.half_size - DVec3::ONE).length() < 1e-10);

        // Mutate and round-trip.
        TileBoundingVolumes::set_oriented_bounding_box(&mut bv, obb);
        let obb2 = TileBoundingVolumes::get_oriented_bounding_box(&bv).unwrap();
        assert!((obb2.half_size - DVec3::ONE).length() < 1e-10);
    }

    #[test]
    fn get_oriented_bounding_box_none_short() {
        let bv = BoundingVolume::default();
        assert!(TileBoundingVolumes::get_oriented_bounding_box(&bv).is_none());
    }

    #[test]
    fn get_bounding_region_round_trip() {
        use std::f64::consts::PI;
        let raw = [-PI, -PI / 2.0, PI, PI / 2.0, -100.0, 500.0];
        let mut bv = BoundingVolume {
            region: raw.to_vec(),
            ..Default::default()
        };

        let region = TileBoundingVolumes::get_bounding_region(&bv).unwrap();
        assert!((region.rectangle.west + PI).abs() < 1e-15);
        assert!((region.maximum_height - 500.0).abs() < 1e-10);

        TileBoundingVolumes::set_bounding_region(&mut bv, region);
        let region2 = TileBoundingVolumes::get_bounding_region(&bv).unwrap();
        assert!((region2.minimum_height + 100.0).abs() < 1e-10);
    }

    #[test]
    fn get_bounding_sphere_round_trip() {
        let mut bv = BoundingVolume {
            sphere: vec![1.0, 2.0, 3.0, 500.0],
            ..Default::default()
        };
        let sphere = TileBoundingVolumes::get_bounding_sphere(&bv).unwrap();
        assert!((sphere.center - DVec3::new(1.0, 2.0, 3.0)).length() < 1e-10);
        assert!((sphere.radius - 500.0).abs() < 1e-10);

        TileBoundingVolumes::set_bounding_sphere(&mut bv, sphere);
        let sphere2 = TileBoundingVolumes::get_bounding_sphere(&bv).unwrap();
        assert!((sphere2.radius - 500.0).abs() < 1e-10);
    }

    #[test]
    fn get_bounding_sphere_none_short() {
        let bv = BoundingVolume::default();
        assert!(TileBoundingVolumes::get_bounding_sphere(&bv).is_none());
    }
}
