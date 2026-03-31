//! Explicit-tileset adapter for `selekt::SpatialHierarchy`.
//!
//! Flattens the nested [`Tileset`] / [`Tile`] tree into a compact flat array
//! so `SelectionEngine` can traverse it efficiently.
//!
//! # Usage
//!
//! ```no_run
//! # use tiles3d::{Tileset, Tile, BoundingVolume};
//! # use tiles3d_selekt::ExplicitTilesetHierarchy;
//! # fn load_tileset() -> Tileset { Tileset::default() }
//! let tileset: Tileset = load_tileset();
//! let hierarchy = ExplicitTilesetHierarchy::from_tileset(&tileset);
//! // Pass `hierarchy` to `SelectionEngine::builder(...)`.
//! ```

use glam::{DMat3, DMat4, DVec2, DVec3};
use selekt::{
    ContentKey, HierarchyExpansion, HierarchyExpansionError, LodDescriptor, NodeId, NodeKind,
    RefinementMode, SpatialHierarchy,
};
use std::collections::HashMap;
use terra::GlobeRectangle;
use zukei::SpatialBounds;

use tiles3d::implicit_tiling_utilities;
use tiles3d::{BoundingVolume, ImplicitTiling, Tile, Tileset};
use tiles3d::{OctreeAvailability, QuadtreeAvailability, TileAvailabilityFlags};
use tiles3d::{OctreeTileID, QuadtreeTileID, TileBoundingVolumes, TileTransform};

use crate::evaluator::GEOMETRIC_ERROR_FAMILY;

struct ExplicitNode {
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    kind: NodeKind,
    bounds: SpatialBounds,
    content_bounds: Option<SpatialBounds>,
    /// Optional finer bounding volume that restricts traversal to when the
    /// primary camera is inside it (3D Tiles `viewerRequestVolume`).
    viewer_request_volume: Option<SpatialBounds>,
    lod: LodDescriptor,
    refinement: RefinementMode,
    content_keys: Vec<ContentKey>,
    /// Accumulated world-space transform (product of all ancestor transforms × this tile's local transform).
    world_transform: DMat4,
    /// Geographic extent of this tile in geodetic longitude/latitude (radians),
    /// present only when the source bounding volume is a `region`.
    globe_rectangle: Option<GlobeRectangle>,
}

/// A `SpatialHierarchy` built from an explicit (non-implicit) 3D Tiles
/// [`Tileset`].
///
/// `ExplicitTilesetHierarchy::from_tileset` performs a depth-first traversal of
/// the tileset's tile tree, assigning sequential [`NodeId`]s (root = 0) and
/// propagating accumulated transforms and refinement modes from parent to child.
pub struct ExplicitTilesetHierarchy {
    nodes: Vec<ExplicitNode>,
}

impl ExplicitTilesetHierarchy {
    /// Build a hierarchy from a parsed [`Tileset`], using the given world-space
    /// root transform as the base for all tile transform accumulation.
    ///
    /// This is the common implementation for both initial load and external
    /// tileset expansion (where the parent tile's accumulated transform must be
    /// carried into the child hierarchy).
    pub fn from_tileset_with_root_transform(
        tileset: &Tileset,
        parent_world_transform: DMat4,
    ) -> Self {
        let mut nodes: Vec<ExplicitNode> = Vec::new();
        let local_root_transform =
            TileTransform::get_transform(&tileset.root).unwrap_or(DMat4::IDENTITY);
        let root_transform = parent_world_transform * local_root_transform;
        let root_refinement = parse_refinement(tileset.root.refine.as_ref());
        flatten_tile(
            &tileset.root,
            None,
            root_transform,
            root_refinement,
            &mut nodes,
        );
        // The 3D Tiles spec allows `tileset.geometricError` to serve as a
        // fallback when the root tile's `geometricError` is zero, which some
        // exporters emit instead of `null`.  Apply it here so LOD decisions
        // at the root are not collapsed to zero-error-stop.
        if let Some(root) = nodes.first_mut() {
            if root.lod.value == 0.0 && tileset.geometric_error > 0.0 {
                root.lod.value = tileset.geometric_error;
            }
        }
        Self { nodes }
    }

    /// Build a hierarchy from a parsed [`Tileset`].
    ///
    /// The root tile receives `NodeId` 0; children are assigned IDs in
    /// depth-first pre-order.  Tile transforms are accumulated top-down so
    /// that all `SpatialBounds` are expressed in the tileset's root coordinate
    /// system (typically ECEF for global datasets).
    pub fn from_tileset(tileset: &Tileset) -> Self {
        Self::from_tileset_with_root_transform(tileset, DMat4::IDENTITY)
    }

    /// Number of nodes in the flattened hierarchy.
    #[inline]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

impl SpatialHierarchy for ExplicitTilesetHierarchy {
    fn root(&self) -> NodeId {
        NodeId::from_index(0)
    }

    fn parent(&self, node: NodeId) -> Option<NodeId> {
        self.nodes.get(node.index())?.parent
    }

    fn children(&self, node: NodeId) -> &[NodeId] {
        self.nodes
            .get(node.index())
            .map_or(&[], |n| n.children.as_slice())
    }

    fn node_kind(&self, node: NodeId) -> NodeKind {
        self.nodes
            .get(node.index())
            .map_or(NodeKind::Renderable, |n| n.kind)
    }

    fn bounds(&self, node: NodeId) -> &SpatialBounds {
        // Fallback to a zero sphere; should never happen for valid hierarchies.
        static FALLBACK: SpatialBounds = SpatialBounds::Sphere {
            center: DVec3::ZERO,
            radius: 0.0,
        };
        self.nodes
            .get(node.index())
            .map_or(&FALLBACK, |n| &n.bounds)
    }

    fn content_bounds(&self, node: NodeId) -> Option<&SpatialBounds> {
        self.nodes.get(node.index())?.content_bounds.as_ref()
    }

    fn viewer_request_volume(&self, node: NodeId) -> Option<&SpatialBounds> {
        self.nodes.get(node.index())?.viewer_request_volume.as_ref()
    }

    fn lod_descriptor(&self, node: NodeId) -> &LodDescriptor {
        static FALLBACK: LodDescriptor = LodDescriptor {
            family: GEOMETRIC_ERROR_FAMILY,
            value: 0.0,
        };
        self.nodes.get(node.index()).map_or(&FALLBACK, |n| &n.lod)
    }

    fn refinement_mode(&self, node: NodeId) -> RefinementMode {
        self.nodes
            .get(node.index())
            .map_or(RefinementMode::Replace, |n| n.refinement)
    }

    fn content_keys(&self, node: NodeId) -> &[ContentKey] {
        self.nodes
            .get(node.index())
            .map_or(&[], |n| n.content_keys.as_slice())
    }

    fn world_transform(&self, node: NodeId) -> glam::DMat4 {
        self.nodes
            .get(node.index())
            .map_or(glam::DMat4::IDENTITY, |n| n.world_transform)
    }

    fn expand(&mut self, patch: HierarchyExpansion) -> Result<(), HierarchyExpansionError> {
        let parent_id = patch.parent;
        if self.nodes.get(parent_id.index()).is_none() {
            return Err(HierarchyExpansionError {
                message: format!("expand: parent node {parent_id} does not exist"),
            });
        }

        // Extract the child hierarchy from the payload.
        let child: Box<ExplicitTilesetHierarchy> = match patch.payload {
            Some(payload) => payload.downcast().map_err(|_| HierarchyExpansionError {
                message: "expand: payload is not an ExplicitTilesetHierarchy".into(),
            })?,
            // No payload — nothing to merge (e.g. patch was produced without content).
            None => return Ok(()),
        };

        if child.nodes.is_empty() {
            return Ok(());
        }

        // All child NodeIds are renumbered to start at the current end of self.nodes.
        // This guarantees no collision with any existing live node.
        let base = self.nodes.len();

        for (i, node) in child.nodes.into_iter().enumerate() {
            let remapped = ExplicitNode {
                // The child root (i == 0) is parented to `parent_id` in the live
                // hierarchy.  All other nodes keep their original child-relative
                // parent, shifted by `base`.
                parent: if i == 0 {
                    Some(parent_id)
                } else {
                    node.parent.map(|p| NodeId::from_index(p.index() + base))
                },
                children: node
                    .children
                    .into_iter()
                    .map(|c| NodeId::from_index(c.index() + base))
                    .collect(),
                kind: node.kind,
                bounds: node.bounds,
                content_bounds: node.content_bounds,
                viewer_request_volume: node.viewer_request_volume,
                lod: node.lod,
                refinement: node.refinement,
                content_keys: node.content_keys,
                world_transform: node.world_transform,
                globe_rectangle: node.globe_rectangle,
            };
            self.nodes.push(remapped);
        }

        // Wire the child root (now at `base`) as a child of `parent_id`.
        self.nodes[parent_id.index()]
            .children
            .push(NodeId::from_index(base));

        Ok(())
    }
}

/// Recursively flatten a tile tree into `nodes`.
///
/// Returns the `NodeId` assigned to `tile`.
fn flatten_tile(
    tile: &Tile,
    parent: Option<NodeId>,
    accumulated_transform: DMat4,
    inherited_refinement: RefinementMode,
    nodes: &mut Vec<ExplicitNode>,
) -> NodeId {
    let my_id = NodeId::from_index(nodes.len());
    debug_assert!(
        my_id.index() == nodes.len(),
        "NodeId index must match nodes.len() to maintain consistent flat indexing"
    );

    // Compose this tile's local transform with the parent's accumulated transform.
    let local_transform = TileTransform::get_transform(tile).unwrap_or(DMat4::IDENTITY);
    let world_transform = accumulated_transform * local_transform;

    let refinement = if tile.refine.is_some() {
        parse_refinement(tile.refine.as_ref())
    } else {
        inherited_refinement
    };

    let kind = if tile.content.is_none() && tile.contents.is_empty() {
        NodeKind::Empty
    } else {
        NodeKind::Renderable
    };

    let bounds = bounding_volume_to_spatial_bounds(&tile.bounding_volume, world_transform);
    let globe_rectangle = bounding_volume_to_globe_rectangle(&tile.bounding_volume);

    // Prefer tile.content.bounding_volume; fall back to tile.contents[0].bounding_volume.
    let content_bounds = tile
        .content
        .as_ref()
        .and_then(|c| c.bounding_volume.as_ref())
        .or_else(|| {
            tile.contents
                .first()
                .and_then(|c| c.bounding_volume.as_ref())
        })
        .map(|bv| bounding_volume_to_spatial_bounds(bv, world_transform));

    // Collect ALL content keys: tile.content (if present) then tile.contents.
    let content_keys: Vec<ContentKey> = if let Some(c) = &tile.content {
        // tile.content is the primary; tile.contents may have additional entries.
        std::iter::once(ContentKey(c.uri.clone()))
            .chain(tile.contents.iter().map(|c| ContentKey(c.uri.clone())))
            .collect()
    } else {
        tile.contents
            .iter()
            .map(|c| ContentKey(c.uri.clone()))
            .collect()
    };

    let viewer_request_volume = tile
        .viewer_request_volume
        .as_ref()
        .map(|bv| bounding_volume_to_spatial_bounds(bv, world_transform));

    // Push a placeholder; we fill children below after recursing.
    nodes.push(ExplicitNode {
        parent,
        children: Vec::new(),
        kind,
        bounds,
        content_bounds,
        viewer_request_volume,
        lod: LodDescriptor {
            family: GEOMETRIC_ERROR_FAMILY,
            value: tile.geometric_error,
        },
        refinement,
        content_keys,
        world_transform,
        globe_rectangle,
    });

    // Recurse into children, collecting their IDs.
    let child_ids: Vec<NodeId> = tile
        .children
        .iter()
        .map(|child| flatten_tile(child, Some(my_id), world_transform, refinement, nodes))
        .collect();

    nodes[my_id.index()].children = child_ids;
    my_id
}

/// Parse the optional `refine` field (a JSON `Value` that should be `"ADD"` or
/// `"REPLACE"`).
fn parse_refinement(refine: Option<&tiles3d::Refine>) -> RefinementMode {
    match refine {
        Some(tiles3d::Refine::Add) => RefinementMode::Add,
        Some(tiles3d::Refine::Replace) | None => RefinementMode::Replace,
    }
}

/// Convert a [`BoundingVolume`] to a selekt-compatible [`SpatialBounds`],
/// applying `world_transform` to translate ECEF-space values appropriately.
///
/// Priority: `box` > `sphere` > `region` (as a sphere approximation).
/// `region` bounding volumes are converted to an OBB approximation via the
/// sphere that encloses the region's corners.
fn bounding_volume_to_spatial_bounds(bv: &BoundingVolume, world_transform: DMat4) -> SpatialBounds {
    debug_assert!(
        bv.r#box.len() >= 12 || bv.sphere.len() >= 4 || !bv.region.is_empty(),
        "BoundingVolume has no valid box, sphere, or region data"
    );
    if bv.r#box.len() >= 12 {
        let b = &bv.r#box;
        let center = DVec3::new(b[0], b[1], b[2]);
        let col0 = DVec3::new(b[3], b[4], b[5]);
        let col1 = DVec3::new(b[6], b[7], b[8]);
        let col2 = DVec3::new(b[9], b[10], b[11]);
        // Apply transform to center and rotate (not translate) half-axes.
        let m3 = DMat3::from_cols(
            world_transform.x_axis.truncate(),
            world_transform.y_axis.truncate(),
            world_transform.z_axis.truncate(),
        );
        let w_center = world_transform.transform_point3(center);
        let w_col0 = m3 * col0;
        let w_col1 = m3 * col1;
        let w_col2 = m3 * col2;
        return SpatialBounds::OrientedBox {
            center: w_center,
            half_axes: DMat3::from_cols(w_col0, w_col1, w_col2),
        };
    }

    if bv.sphere.len() >= 4 {
        let s = &bv.sphere;
        let center = world_transform.transform_point3(DVec3::new(s[0], s[1], s[2]));
        // Scale radius by the max column scale in the transform.
        let m3 = DMat3::from_cols(
            world_transform.x_axis.truncate(),
            world_transform.y_axis.truncate(),
            world_transform.z_axis.truncate(),
        );
        let scale = m3
            .x_axis
            .length()
            .max(m3.y_axis.length())
            .max(m3.z_axis.length());
        let radius = s[3] * scale;
        return SpatialBounds::Sphere { center, radius };
    }

    if bv.region.len() >= 6 {
        // Approximate as an axis-aligned box in ECEF using the 8 corners of
        // the region at min/max heights.  This is conservative but sufficient
        // for frustum culling and LOD distance computation.
        let r = &bv.region;
        let (west, south, east, north, min_h, max_h) = (r[0], r[1], r[2], r[3], r[4], r[5]);
        let corners = region_ecef_corners(west, south, east, north, min_h, max_h);
        let mut mn = corners[0];
        let mut mx = corners[0];
        for &c in &corners[1..] {
            mn = mn.min(c);
            mx = mx.max(c);
        }
        let center = world_transform.transform_point3((mn + mx) * 0.5);
        let half = (mx - mn) * 0.5;
        // Use sphere bounding the AABB for simplicity.
        return SpatialBounds::Sphere {
            center,
            radius: half.length(),
        };
    }

    // Degenerate / empty bounding volume — point at origin.
    SpatialBounds::Sphere {
        center: DVec3::ZERO,
        radius: 0.0,
    }
}

/// Extract the geographic extent from a `region` bounding volume.
///
/// Returns `None` for `box` and `sphere` volumes, which have no exact
/// geodetic longitude/latitude extent without unprojecting.
fn bounding_volume_to_globe_rectangle(bv: &BoundingVolume) -> Option<GlobeRectangle> {
    if bv.region.len() >= 4 {
        let r = &bv.region;
        // region = [west, south, east, north, minH, maxH] in radians.
        Some(GlobeRectangle::new(r[0], r[1], r[2], r[3]))
    } else {
        None
    }
}

/// Compute the 8 ECEF corners of a geodetic region at both height levels.
///
/// Uses simple spherical-Earth approximation — adequate for bounding-volume
/// culling (not geodetic accuracy).
fn region_ecef_corners(
    west: f64,
    south: f64,
    east: f64,
    north: f64,
    min_h: f64,
    max_h: f64,
) -> [DVec3; 8] {
    const WGS84_A: f64 = 6_378_137.0;
    let corners_ll = [(west, south), (east, south), (east, north), (west, north)];
    let mut out = [DVec3::ZERO; 8];
    for (i, &(lon, lat)) in corners_ll.iter().enumerate() {
        let cos_lat = lat.cos();
        let sin_lat = lat.sin();
        let cos_lon = lon.cos();
        let sin_lon = lon.sin();
        for (j, &h) in [min_h, max_h].iter().enumerate() {
            let r = WGS84_A + h;
            out[i * 2 + j] = DVec3::new(r * cos_lat * cos_lon, r * cos_lat * sin_lon, r * sin_lat);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GEOMETRIC_ERROR_FAMILY;
    use tiles3d::{Asset, BoundingVolume, Content, Tile, Tileset};

    fn nid(n: usize) -> NodeId {
        NodeId::from_index(n)
    }

    fn sphere_bv(cx: f64, cy: f64, cz: f64, r: f64) -> BoundingVolume {
        BoundingVolume {
            sphere: vec![cx, cy, cz, r],
            ..Default::default()
        }
    }

    fn box_bv(cx: f64, cy: f64, cz: f64, hx: f64, hy: f64, hz: f64) -> BoundingVolume {
        BoundingVolume {
            r#box: vec![cx, cy, cz, hx, 0.0, 0.0, 0.0, hy, 0.0, 0.0, 0.0, hz],
            ..Default::default()
        }
    }

    fn make_tileset(root: Tile) -> Tileset {
        Tileset {
            asset: Asset {
                version: "1.1".into(),
                ..Default::default()
            },
            geometric_error: root.geometric_error,
            root,
            ..Default::default()
        }
    }

    fn leaf_tile(geometric_error: f64, bv: BoundingVolume, uri: &str) -> Tile {
        Tile {
            bounding_volume: bv,
            geometric_error,
            content: Some(Content {
                uri: uri.to_owned(),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn internal_tile(geometric_error: f64, bv: BoundingVolume, children: Vec<Tile>) -> Tile {
        Tile {
            bounding_volume: bv,
            geometric_error,
            refine: Some(tiles3d::Refine::Replace),
            children,
            ..Default::default()
        }
    }

    #[test]
    fn single_root_leaf() {
        let tile = leaf_tile(1.0, sphere_bv(0.0, 0.0, 0.0, 10.0), "root.glb");
        let tileset = make_tileset(tile);
        let h = ExplicitTilesetHierarchy::from_tileset(&tileset);
        assert_eq!(h.node_count(), 1);
        assert_eq!(h.root(), nid(0));
        assert_eq!(h.parent(nid(0)), None);
        assert_eq!(h.children(nid(0)), &[] as &[NodeId]);
    }

    #[test]
    fn root_with_two_children() {
        let c0 = leaf_tile(5.0, sphere_bv(-50.0, 0.0, 0.0, 20.0), "c0.glb");
        let c1 = leaf_tile(5.0, sphere_bv(50.0, 0.0, 0.0, 20.0), "c1.glb");
        let root = internal_tile(100.0, sphere_bv(0.0, 0.0, 0.0, 100.0), vec![c0, c1]);
        let tileset = make_tileset(root);
        let h = ExplicitTilesetHierarchy::from_tileset(&tileset);
        assert_eq!(h.node_count(), 3, "root + 2 children");
        assert_eq!(h.root(), nid(0));
        assert_eq!(h.children(nid(0)), &[nid(1), nid(2)]);
        assert_eq!(h.parent(nid(1)), Some(nid(0)));
        assert_eq!(h.parent(nid(2)), Some(nid(0)));
        assert_eq!(h.parent(nid(0)), None);
    }

    #[test]
    fn internal_node_has_empty_kind() {
        let root = internal_tile(
            100.0,
            sphere_bv(0.0, 0.0, 0.0, 100.0),
            vec![leaf_tile(5.0, sphere_bv(0.0, 0.0, 0.0, 50.0), "c.glb")],
        );
        let tileset = make_tileset(root);
        let h = ExplicitTilesetHierarchy::from_tileset(&tileset);
        assert_eq!(h.node_kind(nid(0)), NodeKind::Empty);
        assert_eq!(h.node_kind(nid(1)), NodeKind::Renderable);
    }

    #[test]
    fn leaf_node_has_renderable_kind() {
        let tile = leaf_tile(1.0, sphere_bv(0.0, 0.0, 0.0, 10.0), "leaf.glb");
        let tileset = make_tileset(tile);
        let h = ExplicitTilesetHierarchy::from_tileset(&tileset);
        assert_eq!(h.node_kind(nid(0)), NodeKind::Renderable);
    }

    #[test]
    fn content_key_from_uri() {
        let tile = leaf_tile(1.0, sphere_bv(0.0, 0.0, 0.0, 10.0), "tiles/0/0.glb");
        let tileset = make_tileset(tile);
        let h = ExplicitTilesetHierarchy::from_tileset(&tileset);
        assert_eq!(
            h.content_keys(nid(0)).first().map(|k| k.0.as_str()),
            Some("tiles/0/0.glb")
        );
    }

    #[test]
    fn internal_node_has_no_content_key() {
        let root = internal_tile(
            100.0,
            sphere_bv(0.0, 0.0, 0.0, 100.0),
            vec![leaf_tile(1.0, sphere_bv(0.0, 0.0, 0.0, 10.0), "c.glb")],
        );
        let tileset = make_tileset(root);
        let h = ExplicitTilesetHierarchy::from_tileset(&tileset);
        assert!(h.content_keys(nid(0)).is_empty());
    }

    #[test]
    fn lod_family_is_geometric_error() {
        let tile = leaf_tile(42.0, sphere_bv(0.0, 0.0, 0.0, 10.0), "t.glb");
        let tileset = make_tileset(tile);
        let h = ExplicitTilesetHierarchy::from_tileset(&tileset);
        let lod = h.lod_descriptor(nid(0));
        assert_eq!(lod.family, GEOMETRIC_ERROR_FAMILY);
        assert!((lod.value - 42.0).abs() < 1e-12);
    }

    #[test]
    fn refinement_replace_is_default() {
        // Root with no `refine` field → defaults to Replace.
        let mut tile = leaf_tile(1.0, sphere_bv(0.0, 0.0, 0.0, 10.0), "t.glb");
        tile.refine = None;
        let tileset = make_tileset(tile);
        let h = ExplicitTilesetHierarchy::from_tileset(&tileset);
        assert_eq!(h.refinement_mode(nid(0)), RefinementMode::Replace);
    }

    #[test]
    fn refinement_add_parsed() {
        let mut tile = leaf_tile(1.0, sphere_bv(0.0, 0.0, 0.0, 10.0), "t.glb");
        tile.refine = Some(tiles3d::Refine::Add);
        let tileset = make_tileset(tile);
        let h = ExplicitTilesetHierarchy::from_tileset(&tileset);
        assert_eq!(h.refinement_mode(nid(0)), RefinementMode::Add);
    }

    #[test]
    fn refinement_inherited_by_children() {
        let mut child = leaf_tile(1.0, sphere_bv(0.0, 0.0, 0.0, 10.0), "c.glb");
        child.refine = None; // no explicit refine → inherit from parent
        let mut root = internal_tile(100.0, sphere_bv(0.0, 0.0, 0.0, 100.0), vec![child]);
        root.refine = Some(tiles3d::Refine::Add);
        let tileset = make_tileset(root);
        let h = ExplicitTilesetHierarchy::from_tileset(&tileset);
        assert_eq!(h.refinement_mode(nid(0)), RefinementMode::Add);
        assert_eq!(h.refinement_mode(nid(1)), RefinementMode::Add);
    }

    #[test]
    fn sphere_bounds_parsed() {
        let tile = leaf_tile(1.0, sphere_bv(1.0, 2.0, 3.0, 50.0), "t.glb");
        let tileset = make_tileset(tile);
        let h = ExplicitTilesetHierarchy::from_tileset(&tileset);
        match h.bounds(nid(0)) {
            SpatialBounds::Sphere { center, radius } => {
                assert!((center - DVec3::new(1.0, 2.0, 3.0)).length() < 1e-10);
                assert!((radius - 50.0).abs() < 1e-10);
            }
            other => panic!("expected Sphere, got {other:?}"),
        }
    }

    #[test]
    fn obb_bounds_parsed() {
        let tile = leaf_tile(1.0, box_bv(0.0, 0.0, 0.0, 5.0, 3.0, 2.0), "t.glb");
        let tileset = make_tileset(tile);
        let h = ExplicitTilesetHierarchy::from_tileset(&tileset);
        match h.bounds(nid(0)) {
            SpatialBounds::OrientedBox { center, half_axes } => {
                assert!(center.length() < 1e-10);
                let hx = half_axes.x_axis.length();
                let hy = half_axes.y_axis.length();
                let hz = half_axes.z_axis.length();
                assert!((hx - 5.0).abs() < 1e-10, "hx={hx}");
                assert!((hy - 3.0).abs() < 1e-10, "hy={hy}");
                assert!((hz - 2.0).abs() < 1e-10, "hz={hz}");
            }
            other => panic!("expected OrientedBox, got {other:?}"),
        }
    }

    #[test]
    fn region_bounds_gives_sphere() {
        // A small region near the equator.
        let lat = 0.1f64;
        let lon = 0.1f64;
        let bv = BoundingVolume {
            region: vec![lon, lat, lon + 0.01, lat + 0.01, 0.0, 100.0],
            ..Default::default()
        };
        let tile = leaf_tile(1.0, bv, "t.glb");
        let tileset = make_tileset(tile);
        let h = ExplicitTilesetHierarchy::from_tileset(&tileset);
        match h.bounds(nid(0)) {
            SpatialBounds::Sphere { center, radius } => {
                // Centre should be roughly WGS84 surface near the equator.
                assert!(
                    center.length() > 6_000_000.0,
                    "centre is near Earth surface"
                );
                assert!(*radius > 0.0);
            }
            other => panic!("expected Sphere, got {other:?}"),
        }
    }

    #[test]
    fn deep_tree_ids_are_depth_first_preorder() {
        // root(0) → a(1) → aa(2), root(0) → b(3)
        let aa = leaf_tile(1.0, sphere_bv(0.0, 0.0, 0.0, 5.0), "aa.glb");
        let a = internal_tile(10.0, sphere_bv(0.0, 0.0, 0.0, 20.0), vec![aa]);
        let b = leaf_tile(10.0, sphere_bv(50.0, 0.0, 0.0, 20.0), "b.glb");
        let root = internal_tile(100.0, sphere_bv(0.0, 0.0, 0.0, 100.0), vec![a, b]);
        let tileset = make_tileset(root);
        let h = ExplicitTilesetHierarchy::from_tileset(&tileset);
        assert_eq!(h.node_count(), 4);
        assert_eq!(h.children(nid(0)), &[nid(1), nid(3)]); // root → a, b
        assert_eq!(h.children(nid(1)), &[nid(2)]); // a → aa
        assert_eq!(h.parent(nid(2)), Some(nid(1))); // aa's parent = a
        assert_eq!(h.parent(nid(3)), Some(nid(0))); // b's parent = root
    }

    #[test]
    fn expand_invalid_parent_errors() {
        let tile = leaf_tile(1.0, sphere_bv(0.0, 0.0, 0.0, 10.0), "root.glb");
        let mut h = ExplicitTilesetHierarchy::from_tileset(&make_tileset(tile));
        let patch = selekt::HierarchyExpansion::new(nid(9999));
        assert!(h.expand(patch).is_err());
    }

    #[test]
    fn expand_no_payload_is_noop() {
        let tile = leaf_tile(1.0, sphere_bv(0.0, 0.0, 0.0, 10.0), "root.glb");
        let mut h = ExplicitTilesetHierarchy::from_tileset(&make_tileset(tile));
        let before = h.node_count();
        let patch = selekt::HierarchyExpansion::new(nid(0));
        h.expand(patch).unwrap();
        assert_eq!(
            h.node_count(),
            before,
            "no-payload patch must not add nodes"
        );
    }

    #[test]
    fn expand_merges_child_hierarchy() {
        // Live hierarchy: single root leaf (node 0).
        let live_tile = leaf_tile(100.0, sphere_bv(0.0, 0.0, 0.0, 100.0), "live.glb");
        let mut live = ExplicitTilesetHierarchy::from_tileset(&make_tileset(live_tile));
        assert_eq!(live.node_count(), 1);

        // Child hierarchy: root (0) + one child (1).
        let child_leaf = leaf_tile(5.0, sphere_bv(10.0, 0.0, 0.0, 10.0), "child_leaf.glb");
        let child_root = internal_tile(50.0, sphere_bv(0.0, 0.0, 0.0, 50.0), vec![child_leaf]);
        let child = ExplicitTilesetHierarchy::from_tileset(&make_tileset(child_root));
        assert_eq!(child.node_count(), 2);

        // Merge the child under live node 0.
        let patch = selekt::HierarchyExpansion::with_payload(nid(0), child);
        live.expand(patch).unwrap();

        // After merge: 1 (live) + 2 (child) = 3 nodes total.
        assert_eq!(live.node_count(), 3, "live + child nodes");

        // Child root is now at ID 1 (base = old live.node_count() = 1).
        let child_root_id: NodeId = NodeId::from_index(1);
        let child_leaf_id: NodeId = NodeId::from_index(2);

        // Live root (0) must have child root (1) in its children list.
        assert!(
            live.children(NodeId::from_index(0))
                .contains(&child_root_id),
            "live root should list child root as a child"
        );

        // Child root's parent must be live root (0).
        assert_eq!(
            live.parent(child_root_id),
            Some(NodeId::from_index(0)),
            "child root's parent must be the live root"
        );

        // Child leaf's parent must be child root (1).
        assert_eq!(
            live.parent(child_leaf_id),
            Some(child_root_id),
            "child leaf's parent must be the child root"
        );

        // Children of child_root must include child_leaf.
        assert!(
            live.children(child_root_id).contains(&child_leaf_id),
            "child root's children must include child leaf"
        );

        // Content keys are preserved.
        assert_eq!(
            live.content_keys(child_leaf_id)
                .first()
                .map(|k| k.0.as_str()),
            Some("child_leaf.glb")
        );
    }

    #[test]
    fn engine_traverses_explicit_hierarchy() {
        use orkester::{CancellationToken, Context, Task, ThreadPool};
        use selekt::{
            ContentKey, ContentLoader, LoadResult, SelectionEngine, SelectionEngineBuilder,
            ViewState,
        };

        // A minimal single-tile tileset.
        let tile = leaf_tile(
            1000.0,
            sphere_bv(6_378_137.0, 0.0, 0.0, 500_000.0),
            "root.glb",
        );
        let tileset = make_tileset(tile);
        let hierarchy = ExplicitTilesetHierarchy::from_tileset(&tileset);

        // Stub content loader.
        struct StubLoader;
        #[derive(Debug)]
        struct StubError;
        impl std::fmt::Display for StubError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("stub")
            }
        }
        impl std::error::Error for StubError {}

        impl ContentLoader<String> for StubLoader {
            type Error = StubError;
            fn load(
                &self,
                _bg: &Context,
                _main: &Context,
                _node_id: selekt::NodeId,
                key: &ContentKey,
                _cancel: CancellationToken,
            ) -> Task<Result<LoadResult<String>, StubError>> {
                let content = key.0.clone();
                orkester::resolved(Ok(LoadResult::Content {
                    content: Some(content),
                    byte_size: 8,
                }))
            }
        }

        let pool = ThreadPool::new(1);
        let bg_context = pool.context();
        let config = SelectionEngineBuilder::new(
            bg_context,
            hierarchy,
            crate::GeometricErrorEvaluator::new(16.0),
            StubLoader,
        );
        let mut engine = config.build();

        // Camera far enough away that root should be selected (low SSE).
        let view = ViewState::perspective(
            DVec3::new(6_378_137.0 + 10_000_000.0, 0.0, 0.0),
            DVec3::new(-1.0, 0.0, 0.0),
            DVec3::Z,
            [1920, 1080],
            60_f64.to_radians(),
            45_f64.to_radians(),
        );

        let handle = engine.add_view_group(1.0);

        // Frame 1: queue loads.
        engine.update_view_group(handle, &[view.clone()]);
        engine.load();

        // Frame 2: content is now resident; traversal will select the root.
        engine.update_view_group(handle, &[view]);
        engine.load();

        let selected = engine.view_group_result(handle).map(|r| r.nodes_to_render.clone()).unwrap_or_default();
        assert!(!selected.is_empty(), "engine must select at least one node");
        // Root (node 0) should be selected; content should be "root.glb"
        assert!(
            selected.contains(&nid(0)),
            "root tile (node 0) should be selected; got {selected:?}"
        );
        let content = engine.content(nid(0));
        assert_eq!(content.map(|s| s.as_str()), Some("root.glb"));
    }
}

struct OctNodeData {
    tile_id: OctreeTileID,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    expanded: bool,
    bounds: SpatialBounds,
    lod: LodDescriptor,
    kind: NodeKind,
    refinement: RefinementMode,
    content_keys: Vec<ContentKey>,
}

/// A `selekt::SpatialHierarchy` backed by a 3D Tiles implicit octree.
///
/// Construct from the root tile's bounding volume, the implicit tiling
/// descriptor, and a loaded [`OctreeAvailability`].  Children are inserted
/// eagerly so [`SpatialHierarchy::children`] never requires interior mutation.
pub struct ImplicitOctreeHierarchy {
    nodes: Vec<OctNodeData>,
    tile_to_node: HashMap<OctreeTileID, NodeId>,
    availability: OctreeAvailability,
    root_bounds: SpatialBounds,
    root_geometric_error: f64,
    content_url_template: String,
    refinement: RefinementMode,
}

impl ImplicitOctreeHierarchy {
    /// Build a new hierarchy.
    ///
    /// * `root_bv` — bounding volume on the root implicit tile
    /// * `root_geometric_error` — `geometricError` of the root tile
    /// * `_implicit_tiling` — the `implicitTiling` descriptor (reserved)
    /// * `availability` — loaded [`OctreeAvailability`]
    /// * `content_url_template` — URI template, e.g. `"content/{level}/{x}/{y}/{z}.glb"`
    /// * `use_additive_refinement` — `true` → ADD, `false` → REPLACE
    pub fn new(
        root_bv: &BoundingVolume,
        root_geometric_error: f64,
        _implicit_tiling: &ImplicitTiling,
        availability: OctreeAvailability,
        content_url_template: impl Into<String>,
        use_additive_refinement: bool,
    ) -> Self {
        let root_bounds = implicit_bvol_to_bounds(root_bv);
        let refinement = if use_additive_refinement {
            RefinementMode::Add
        } else {
            RefinementMode::Replace
        };
        let content_url_template = content_url_template.into();

        let root_tile = OctreeTileID::new(0, 0, 0, 0);
        let mut tile_to_node = HashMap::new();
        tile_to_node.insert(root_tile, NodeId::from_index(0));

        let root_node = Self::make_node(
            root_tile,
            None,
            &root_bounds,
            root_geometric_error,
            refinement,
            &content_url_template,
            &availability,
        );

        let mut h = Self {
            nodes: vec![root_node],
            tile_to_node,
            availability,
            root_bounds,
            root_geometric_error,
            content_url_template,
            refinement,
        };
        // Eagerly expand all available levels via BFS.
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(NodeId::from_index(0));
        while let Some(node_id) = queue.pop_front() {
            h.expand_node(node_id);
            let children = h.nodes[node_id.index()].children.clone();
            queue.extend(children);
        }
        h
    }

    /// Return the `NodeId` for `tile_id`, if it was already inserted.
    pub fn node_for_tile(&self, tile_id: OctreeTileID) -> Option<NodeId> {
        self.tile_to_node.get(&tile_id).copied()
    }

    fn make_node(
        tile_id: OctreeTileID,
        parent: Option<NodeId>,
        root_bounds: &SpatialBounds,
        geometric_error: f64,
        refinement: RefinementMode,
        template: &str,
        availability: &OctreeAvailability,
    ) -> OctNodeData {
        let flags = availability.compute_availability(tile_id);
        let has_content = flags.contains(TileAvailabilityFlags::CONTENT_AVAILABLE);
        let kind = if has_content {
            NodeKind::Renderable
        } else {
            NodeKind::Empty
        };
        let content_keys = if has_content {
            vec![ContentKey(implicit_tiling_utilities::resolve_url_oct(
                "", template, tile_id,
            ))]
        } else {
            vec![]
        };
        OctNodeData {
            tile_id,
            parent,
            children: Vec::new(),
            expanded: false,
            bounds: split_bounds_oct(root_bounds, tile_id),
            lod: LodDescriptor {
                family: GEOMETRIC_ERROR_FAMILY,
                value: geometric_error,
            },
            kind,
            refinement,
            content_keys,
        }
    }

    fn expand_node(&mut self, node_id: NodeId) {
        if self.nodes[node_id.index()].expanded {
            return;
        }
        self.nodes[node_id.index()].expanded = true;

        let tile_id = self.nodes[node_id.index()].tile_id;
        let child_level = tile_id.level + 1;
        let child_error = self.root_geometric_error
            / implicit_tiling_utilities::compute_level_denominator(child_level);

        for child_tile in tile_id.children() {
            let flags = self.availability.compute_availability(child_tile);
            if !flags.contains(TileAvailabilityFlags::TILE_AVAILABLE) {
                continue;
            }

            let child_id = NodeId::from_index(self.nodes.len());
            let child_node = Self::make_node(
                child_tile,
                Some(node_id),
                &self.root_bounds,
                child_error,
                self.refinement,
                &self.content_url_template,
                &self.availability,
            );

            self.nodes[node_id.index()].children.push(child_id);
            self.tile_to_node.insert(child_tile, child_id);
            self.nodes.push(child_node);
        }
    }
}

impl SpatialHierarchy for ImplicitOctreeHierarchy {
    fn root(&self) -> NodeId {
        NodeId::from_index(0)
    }

    fn parent(&self, node: NodeId) -> Option<NodeId> {
        self.nodes[node.index()].parent
    }

    fn children(&self, node: NodeId) -> &[NodeId] {
        &self.nodes[node.index()].children
    }

    fn node_kind(&self, node: NodeId) -> NodeKind {
        self.nodes[node.index()].kind
    }

    fn bounds(&self, node: NodeId) -> &SpatialBounds {
        &self.nodes[node.index()].bounds
    }

    fn lod_descriptor(&self, node: NodeId) -> &LodDescriptor {
        &self.nodes[node.index()].lod
    }

    fn refinement_mode(&self, node: NodeId) -> RefinementMode {
        self.nodes[node.index()].refinement
    }

    fn content_keys(&self, node: NodeId) -> &[ContentKey] {
        self.nodes[node.index()].content_keys.as_slice()
    }

    fn expand(&mut self, patch: HierarchyExpansion) -> Result<(), HierarchyExpansionError> {
        let parent = patch.parent.index();
        if parent >= self.nodes.len() {
            return Err(HierarchyExpansionError {
                message: format!(
                    "expand: parent node {} out of range (len={})",
                    parent,
                    self.nodes.len()
                ),
            });
        }
        // Re-expand the node: clears cached children and re-derives them from
        // the updated availability tree.
        self.nodes[parent].children.clear();
        self.nodes[parent].expanded = false;
        self.expand_node(patch.parent);
        Ok(())
    }
}

/// Compute the [`SpatialBounds`] for an octree `tile` by subdividing the root
/// bounds.  All three axes are divided (unlike the quadtree which keeps Z
/// constant).
fn split_bounds_oct(root: &SpatialBounds, tile: OctreeTileID) -> SpatialBounds {
    let denom = implicit_tiling_utilities::compute_level_denominator(tile.level);
    let n = denom as u32;

    match root {
        SpatialBounds::OrientedBox { center, half_axes } => {
            // All three half-axes scale by 1/denom for an octree.
            let scale = 1.0 / denom;
            let col0 = half_axes.x_axis * scale;
            let col1 = half_axes.y_axis * scale;
            let col2 = half_axes.z_axis * scale;

            // Fractional centre offset in [-1, 1] for each axis.
            let cx = ((tile.x as f64 + 0.5) / n as f64 - 0.5) * 2.0;
            let cy = ((tile.y as f64 + 0.5) / n as f64 - 0.5) * 2.0;
            let cz = ((tile.z as f64 + 0.5) / n as f64 - 0.5) * 2.0;

            let child_center =
                center + half_axes.x_axis * cx + half_axes.y_axis * cy + half_axes.z_axis * cz;

            let new_half_axes = DMat3::from_cols(col0, col1, col2);
            SpatialBounds::OrientedBox {
                center: child_center,
                half_axes: new_half_axes,
            }
        }
        SpatialBounds::AxisAlignedBox { min, max } => {
            let size = (*max - *min) / denom;
            let child_min = *min
                + DVec3::new(
                    tile.x as f64 * size.x,
                    tile.y as f64 * size.y,
                    tile.z as f64 * size.z,
                );
            SpatialBounds::AxisAlignedBox {
                min: child_min,
                max: child_min + size,
            }
        }
        SpatialBounds::Sphere { center, radius } => {
            // All 3 axes subdivided → radius shrinks by 1/denom^(1/3).
            SpatialBounds::Sphere {
                center: *center,
                radius: radius / denom.cbrt(),
            }
        }
        SpatialBounds::Rectangle { min, max } => {
            // Rectangles are 2-D; octrees normally use OBBs, but fall back
            // gracefully by treating this as the quadtree case (ignore z).
            let w = (max.x - min.x) / n as f64;
            let h = (max.y - min.y) / n as f64;
            let child_min = DVec2::new(min.x + tile.x as f64 * w, min.y + tile.y as f64 * h);
            SpatialBounds::Rectangle {
                min: child_min,
                max: child_min + DVec2::new(w, h),
            }
        }
        SpatialBounds::Polygon { .. } => {
            // 2D polygon bounds are not subdivided for implicit tilesets — return as-is.
            root.clone()
        }
    }
}

#[cfg(test)]
mod octree_tests {
    use super::*;
    use tiles3d::{
        AvailabilityView as AV, OctreeAvailability, SubdivisionScheme, SubtreeAvailability,
    };

    fn nid(n: usize) -> NodeId {
        NodeId::from_index(n)
    }

    fn all_available(subtree_levels: u32) -> OctreeAvailability {
        let subtree = SubtreeAvailability::new(
            SubdivisionScheme::Octree,
            subtree_levels,
            AV::Constant(true),       // tile availability
            AV::Constant(true),       // child subtree availability
            vec![AV::Constant(true)], // content availability
        )
        .unwrap();
        let mut avail = OctreeAvailability::new(subtree_levels, subtree_levels * 4);
        avail.add_subtree(OctreeTileID::new(0, 0, 0, 0), subtree);
        avail
    }

    fn obb_root_bv() -> BoundingVolume {
        let mut bv = BoundingVolume::default();
        bv.r#box = vec![
            0.0, 0.0, 0.0, // centre
            100.0, 0.0, 0.0, // x half-axis
            0.0, 100.0, 0.0, // y half-axis
            0.0, 0.0, 100.0, // z half-axis (note: also scaled for octrees)
        ];
        bv
    }

    fn make_h(subtree_levels: u32) -> ImplicitOctreeHierarchy {
        ImplicitOctreeHierarchy::new(
            &obb_root_bv(),
            1024.0,
            &tiles3d::ImplicitTiling::default(),
            all_available(subtree_levels),
            "content/{level}/{x}/{y}/{z}.glb",
            false,
        )
    }

    #[test]
    fn root_is_zero() {
        assert_eq!(make_h(2).root(), nid(0));
    }

    #[test]
    fn root_has_no_parent() {
        assert_eq!(make_h(2).parent(nid(0)), None);
    }

    #[test]
    fn root_has_eight_children() {
        let h = make_h(2);
        assert_eq!(h.children(nid(0)).len(), 8, "root should have 8 children");
    }

    #[test]
    fn children_have_root_as_parent() {
        let h = make_h(2);
        for &child in h.children(nid(0)) {
            assert_eq!(h.parent(child), Some(nid(0)));
        }
    }

    #[test]
    fn root_geometric_error_matches() {
        assert!((make_h(2).lod_descriptor(nid(0)).value - 1024.0).abs() < 1e-10);
    }

    #[test]
    fn child_geometric_error_is_halved() {
        let h = make_h(2);
        let child = h.children(nid(0))[0];
        assert!((h.lod_descriptor(child).value - 512.0).abs() < 1e-10);
    }

    #[test]
    fn root_content_key() {
        let h = make_h(2);
        assert_eq!(
            h.content_keys(nid(0)).first().map(|k| k.0.as_str()),
            Some("content/0/0/0/0.glb"),
        );
    }

    #[test]
    fn child_content_keys_include_z() {
        let h = make_h(2);
        // Children are ordered x*2+dx | y*2+dy | z*2+dz, dx/dy/dz in {0,1}.
        // Two of the eight should be level=1, various x/y/z combos.
        let keys: Vec<String> = h
            .children(nid(0))
            .iter()
            .map(|&c| h.content_keys(c).first().unwrap().0.clone())
            .collect();
        // All keys must belong to level 1.
        assert!(keys.iter().all(|k| k.starts_with("content/1/")));
        // All 8 must be unique.
        let unique: std::collections::HashSet<_> = keys.iter().collect();
        assert_eq!(unique.len(), 8);
    }

    #[test]
    fn obb_all_three_halfaxes_halved_for_children() {
        let h = make_h(2);
        let SpatialBounds::OrientedBox {
            half_axes: root_ha, ..
        } = h.bounds(nid(0))
        else {
            panic!("expected OBB");
        };
        let child = h.children(nid(0))[0];
        let SpatialBounds::OrientedBox {
            half_axes: child_ha,
            ..
        } = h.bounds(child)
        else {
            panic!("expected OBB");
        };
        let rx = child_ha.x_axis.length() / root_ha.x_axis.length();
        let ry = child_ha.y_axis.length() / root_ha.y_axis.length();
        let rz = child_ha.z_axis.length() / root_ha.z_axis.length();
        assert!((rx - 0.5).abs() < 1e-6, "x ratio={rx}");
        assert!((ry - 0.5).abs() < 1e-6, "y ratio={ry}");
        assert!((rz - 0.5).abs() < 1e-6, "z ratio={rz}");
    }

    #[test]
    fn expand_invalid_parent_errors() {
        let mut h = make_h(2);
        let patch = HierarchyExpansion::new(nid(9999));
        assert!(h.expand(patch).is_err());
    }

    #[test]
    fn expand_re_expands_node() {
        let mut h = make_h(2);
        assert_eq!(h.children(nid(0)).len(), 8);
        let patch = HierarchyExpansion::new(nid(0));
        h.expand(patch).unwrap();
        assert!(h.nodes[0].expanded);
        assert_eq!(h.children(nid(0)).len(), 8);
    }

    #[test]
    fn refinement_mode_add() {
        let h = ImplicitOctreeHierarchy::new(
            &obb_root_bv(),
            512.0,
            &tiles3d::ImplicitTiling::default(),
            all_available(2),
            "c/{level}/{x}/{y}/{z}.glb",
            true,
        );
        assert_eq!(h.refinement_mode(nid(0)), RefinementMode::Add);
    }

    #[test]
    fn node_for_tile_root() {
        let h = make_h(2);
        assert_eq!(h.node_for_tile(OctreeTileID::new(0, 0, 0, 0)), Some(nid(0)));
    }

    #[test]
    fn node_for_tile_child() {
        let h = make_h(2);
        let child_tile = OctreeTileID::new(1, 0, 0, 0);
        let node_id = h.node_for_tile(child_tile);
        assert!(node_id.is_some(), "child tile should have a node");
        let id = node_id.unwrap();
        assert_eq!(h.nodes[id.index()].tile_id, child_tile);
    }
}

struct QuadNodeData {
    tile_id: QuadtreeTileID,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    /// Whether children have been expanded yet.
    expanded: bool,
    bounds: SpatialBounds,
    lod: LodDescriptor,
    kind: NodeKind,
    refinement: RefinementMode,
    content_keys: Vec<ContentKey>,
}

/// A `selekt::SpatialHierarchy` backed by a 3D Tiles implicit quadtree.
///
/// Build one from an implicit root tile's bounding volume, implicit tiling
/// descriptor, and an initially-loaded [`QuadtreeAvailability`].
pub struct ImplicitQuadtreeHierarchy {
    /// All nodes; index = NodeId.
    nodes: Vec<QuadNodeData>,
    /// Map from tile ID to NodeId for quick lookup.
    tile_to_node: HashMap<QuadtreeTileID, NodeId>,
    /// Availability information.
    availability: QuadtreeAvailability,
    /// Root bounding volume (split into children by halving).
    root_bounds: SpatialBounds,
    /// Geometric error at the root; halved per level.
    root_geometric_error: f64,
    /// URL template for content keys: `{level}`, `{x}`, `{y}` placeholders.
    content_url_template: String,
    /// Whether children use REPLACE (default) or ADD refinement.
    refinement: RefinementMode,
}

impl ImplicitQuadtreeHierarchy {
    /// Create a new hierarchy from the root tile's bounding volume and the
    /// implicit tiling descriptor.
    ///
    /// * `root_bv` — the bounding volume from the root implicit tile
    /// * `root_geometric_error` — `geometricError` of the root tile
    /// * `implicit_tiling` — the `implicitTiling` object on the root tile
    /// * `availability` — freshly-loaded [`QuadtreeAvailability`]
    /// * `content_url_template` — content URI template (e.g.
    ///   `"content/{level}/{x}/{y}.glb"`)
    /// * `use_additive_refinement` — `true` for ADD, `false` (default) for REPLACE
    pub fn new(
        root_bv: &BoundingVolume,
        root_geometric_error: f64,
        _implicit_tiling: &ImplicitTiling,
        availability: QuadtreeAvailability,
        content_url_template: impl Into<String>,
        use_additive_refinement: bool,
    ) -> Self {
        let root_bounds = implicit_bvol_to_bounds(root_bv);
        let refinement = if use_additive_refinement {
            RefinementMode::Add
        } else {
            RefinementMode::Replace
        };

        let root_id = QuadtreeTileID::new(0, 0, 0);
        let flags = availability.compute_availability(root_id);
        let has_content = flags.contains(TileAvailabilityFlags::CONTENT_AVAILABLE);
        let kind = if has_content {
            NodeKind::Renderable
        } else {
            NodeKind::Empty
        };
        let content_url_template = content_url_template.into();

        let content_keys = if has_content {
            vec![ContentKey(implicit_tiling_utilities::resolve_url_quad(
                "",
                &content_url_template,
                root_id,
            ))]
        } else {
            vec![]
        };

        let root_lod = LodDescriptor {
            family: GEOMETRIC_ERROR_FAMILY,
            value: root_geometric_error,
        };

        let root_node = QuadNodeData {
            tile_id: root_id,
            parent: None,
            children: Vec::new(),
            expanded: false,
            bounds: root_bounds.clone(),
            lod: root_lod,
            kind,
            refinement,
            content_keys,
        };

        let mut tile_to_node = HashMap::new();
        tile_to_node.insert(root_id, NodeId::from_index(0));

        let mut h = Self {
            nodes: vec![root_node],
            tile_to_node,
            availability,
            root_bounds,
            root_geometric_error,
            content_url_template,
            refinement,
        };
        // Eagerly expand all available levels via BFS so children() never
        // needs to mutate self.
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(NodeId::from_index(0));
        while let Some(node_id) = queue.pop_front() {
            h.expand_node(node_id);
            let children = h.nodes[node_id.index()].children.clone();
            queue.extend(children);
        }
        h
    }

    /// Insert all available children of `node_id` into `self.nodes`.
    ///
    /// No-ops if already expanded or if the tile has no available children.
    fn expand_node(&mut self, node_id: NodeId) {
        let tile_id = self.nodes[node_id.index()].tile_id;
        if self.nodes[node_id.index()].expanded {
            return;
        }
        self.nodes[node_id.index()].expanded = true;

        let child_level = tile_id.level + 1;
        let child_error = self.root_geometric_error
            / implicit_tiling_utilities::compute_level_denominator(child_level);

        for child_tile in tile_id.children() {
            let flags = self.availability.compute_availability(child_tile);
            if !flags.contains(TileAvailabilityFlags::TILE_AVAILABLE) {
                continue;
            }

            let child_id = NodeId::from_index(self.nodes.len());
            let child_bounds = split_bounds(&self.root_bounds, child_tile);
            let has_content = flags.contains(TileAvailabilityFlags::CONTENT_AVAILABLE);
            let kind = if has_content {
                NodeKind::Renderable
            } else {
                NodeKind::Empty
            };

            let content_keys = if has_content {
                vec![ContentKey(implicit_tiling_utilities::resolve_url_quad(
                    "",
                    &self.content_url_template,
                    child_tile,
                ))]
            } else {
                vec![]
            };

            let child_node = QuadNodeData {
                tile_id: child_tile,
                parent: Some(node_id),
                children: Vec::new(),
                expanded: false,
                bounds: child_bounds,
                lod: LodDescriptor {
                    family: GEOMETRIC_ERROR_FAMILY,
                    value: child_error,
                },
                kind,
                refinement: self.refinement,
                content_keys,
            };

            self.nodes[node_id.index()].children.push(child_id);
            self.tile_to_node.insert(child_tile, child_id);
            self.nodes.push(child_node);
        }
    }
}

impl SpatialHierarchy for ImplicitQuadtreeHierarchy {
    fn root(&self) -> NodeId {
        NodeId::from_index(0)
    }

    fn parent(&self, node: NodeId) -> Option<NodeId> {
        self.nodes[node.index()].parent
    }

    fn children(&self, node: NodeId) -> &[NodeId] {
        &self.nodes[node.index()].children
    }

    fn node_kind(&self, node: NodeId) -> NodeKind {
        self.nodes[node.index()].kind
    }

    fn bounds(&self, node: NodeId) -> &SpatialBounds {
        &self.nodes[node.index()].bounds
    }

    fn lod_descriptor(&self, node: NodeId) -> &LodDescriptor {
        &self.nodes[node.index()].lod
    }

    fn refinement_mode(&self, node: NodeId) -> RefinementMode {
        self.nodes[node.index()].refinement
    }

    fn content_keys(&self, node: NodeId) -> &[ContentKey] {
        self.nodes[node.index()].content_keys.as_slice()
    }

    fn expand(&mut self, patch: HierarchyExpansion) -> Result<(), HierarchyExpansionError> {
        // For implicit tilesets, a patch signals that a new subtree has been
        // loaded and the parent node must be re-expanded against the updated
        // availability tree.
        let parent = patch.parent.index();
        if parent >= self.nodes.len() {
            return Err(HierarchyExpansionError {
                message: format!(
                    "expand: parent node {} out of range (len={})",
                    parent,
                    self.nodes.len()
                ),
            });
        }
        // Clear existing children and immediately re-expand with the updated
        // availability (we have &mut self here so no unsafe trick needed).
        self.nodes[parent].children.clear();
        self.nodes[parent].expanded = false;
        self.expand_node(patch.parent);
        Ok(())
    }
}

fn implicit_bvol_to_bounds(bv: &BoundingVolume) -> SpatialBounds {
    // Prefer OBB, then sphere.
    if let Some(obb) = TileBoundingVolumes::get_oriented_bounding_box(bv) {
        return SpatialBounds::OrientedBox {
            center: obb.center,
            half_axes: obb.half_axes_matrix(),
        };
    }
    if let Some(s) = TileBoundingVolumes::get_bounding_sphere(bv) {
        return SpatialBounds::Sphere {
            center: s.center,
            radius: s.radius,
        };
    }
    // Fall back to an AABB centred at the origin with zero extent.
    SpatialBounds::AxisAlignedBox {
        min: DVec3::ZERO,
        max: DVec3::ZERO,
    }
}

/// Compute the [`SpatialBounds`] for `tile` by subdividing the root bounds.
///
/// For an OBB root, each child OBB is the parent shrunk by half along the two
/// horizontal axes and shifted to the child's quadrant.
///
/// For sphere / AABB roots, we simply shrink and shift the bounding box.
fn split_bounds(root: &SpatialBounds, tile: QuadtreeTileID) -> SpatialBounds {
    let denom = implicit_tiling_utilities::compute_level_denominator(tile.level);
    let tiles_at_level = denom as u32; // power of two

    match root {
        SpatialBounds::OrientedBox { center, half_axes } => {
            // The root OBB's two horizontal half-axes span the whole rectangle.
            // Each child tile covers 1/tiles_at_level of each axis.
            let scale = 1.0 / denom;

            // Column 0 (X half-axis) and column 1 (Y half-axis) are divided.
            // Column 2 (Z / vertical) is unchanged.
            let col0 = half_axes.x_axis * scale;
            let col1 = half_axes.y_axis * scale;
            let col2 = half_axes.z_axis;

            // Tile (x, y) is offset from the root centre along the two axes.
            // The tile occupies [(x/n - 0.5)*2, ((x+1)/n - 0.5)*2] in unit coords.
            // Its centre in unit coords is ((x + 0.5)/n - 0.5) * 2 per axis.
            let cx = ((tile.x as f64 + 0.5) / tiles_at_level as f64 - 0.5) * 2.0;
            let cy = ((tile.y as f64 + 0.5) / tiles_at_level as f64 - 0.5) * 2.0;

            let child_center = center + half_axes.x_axis * cx + half_axes.y_axis * cy;

            let new_half_axes = DMat3::from_cols(col0, col1, col2);
            SpatialBounds::OrientedBox {
                center: child_center,
                half_axes: new_half_axes,
            }
        }
        SpatialBounds::AxisAlignedBox { min, max } => {
            let size = (*max - *min) / denom;
            let child_min = *min + DVec3::new(tile.x as f64 * size.x, tile.y as f64 * size.y, 0.0);
            SpatialBounds::AxisAlignedBox {
                min: child_min,
                max: child_min + DVec3::new(size.x, size.y, max.z - min.z),
            }
        }
        SpatialBounds::Sphere { center, radius } => {
            // Approximate: shrink radius proportionally (very rough for quadtrees)
            SpatialBounds::Sphere {
                center: *center,
                radius: radius / denom.sqrt(),
            }
        }
        SpatialBounds::Rectangle { min, max } => {
            let w = (max.x - min.x) / tiles_at_level as f64;
            let h = (max.y - min.y) / tiles_at_level as f64;
            let child_min = DVec2::new(min.x + tile.x as f64 * w, min.y + tile.y as f64 * h);
            SpatialBounds::Rectangle {
                min: child_min,
                max: child_min + DVec2::new(w, h),
            }
        }
        SpatialBounds::Polygon { .. } => {
            // 2D polygon bounds are not subdivided for implicit tilesets — return as-is.
            root.clone()
        }
    }
}

#[cfg(test)]
mod quadtree_tests {
    use super::*;
    use tiles3d::{
        AvailabilityView as AV, QuadtreeAvailability, SubdivisionScheme, SubtreeAvailability,
    };

    fn nid(n: usize) -> NodeId {
        NodeId::from_index(n)
    }

    /// Build an all-available quadtree with 2 subtree levels.
    fn all_available_availability(subtree_levels: u32) -> QuadtreeAvailability {
        let subtree = SubtreeAvailability::new(
            SubdivisionScheme::Quadtree,
            subtree_levels,
            AV::Constant(true),       // tile availability
            AV::Constant(true),       // child_subtree availability
            vec![AV::Constant(true)], // content availability
        )
        .unwrap();
        let mut avail = QuadtreeAvailability::new(subtree_levels, subtree_levels * 4);
        avail.add_subtree(QuadtreeTileID::new(0, 0, 0), subtree);
        avail
    }

    fn obb_root_bv() -> BoundingVolume {
        // Box: centre (0,0,0), half-axes = identity * 100
        let mut bv = BoundingVolume::default();
        bv.r#box = vec![
            0.0, 0.0, 0.0, // centre
            100.0, 0.0, 0.0, // x half-axis
            0.0, 100.0, 0.0, // y half-axis
            0.0, 0.0, 50.0, // z half-axis
        ];
        bv
    }

    fn make_hierarchy(subtree_levels: u32) -> ImplicitQuadtreeHierarchy {
        let avail = all_available_availability(subtree_levels);
        ImplicitQuadtreeHierarchy::new(
            &obb_root_bv(),
            1024.0,
            &tiles3d::ImplicitTiling::default(),
            avail,
            "content/{level}/{x}/{y}.glb",
            false,
        )
    }

    #[test]
    fn root_is_node_zero() {
        let h = make_hierarchy(2);
        assert_eq!(h.root(), nid(0));
    }

    #[test]
    fn root_has_no_parent() {
        let h = make_hierarchy(2);
        assert_eq!(h.parent(nid(0)), None);
    }

    #[test]
    fn root_has_four_children() {
        let h = make_hierarchy(2);
        assert_eq!(h.children(nid(0)).len(), 4, "root should have 4 children");
    }

    #[test]
    fn children_have_root_as_parent() {
        let h = make_hierarchy(2);
        let children = SpatialHierarchy::children(&h, nid(0)).to_vec();
        for child in children {
            assert_eq!(
                h.parent(child),
                Some(nid(0)),
                "child {} should have root as parent",
                child
            );
        }
    }

    #[test]
    fn root_geometric_error() {
        let h = make_hierarchy(2);
        assert!((h.lod_descriptor(nid(0)).value - 1024.0).abs() < 1e-10);
    }

    #[test]
    fn child_geometric_error_is_halved() {
        let h = make_hierarchy(2);
        let child = SpatialHierarchy::children(&h, nid(0))[0];
        assert!((h.lod_descriptor(child).value - 512.0).abs() < 1e-10);
    }

    #[test]
    fn root_content_key_matches_template() {
        let h = make_hierarchy(2);
        let key = h
            .content_keys(nid(0))
            .first()
            .expect("root should have content key");
        assert_eq!(key.0, "content/0/0/0.glb");
    }

    #[test]
    fn child_content_key_matches_template() {
        let h = make_hierarchy(2);
        let children = SpatialHierarchy::children(&h, nid(0)).to_vec();
        // Children are ordered: (1,0,0), (1,1,0), (1,0,1), (1,1,1)
        let expected: Vec<&str> = vec![
            "content/1/0/0.glb",
            "content/1/1/0.glb",
            "content/1/0/1.glb",
            "content/1/1/1.glb",
        ];
        for (child_id, exp) in children.iter().zip(expected.iter()) {
            let key = h
                .content_keys(*child_id)
                .first()
                .expect("child should have content key");
            assert_eq!(&key.0, exp, "child {} key mismatch", child_id);
        }
    }

    #[test]
    fn obb_child_bounds_are_smaller() {
        let h = make_hierarchy(2);
        let root_bounds = h.bounds(nid(0));
        let SpatialBounds::OrientedBox {
            half_axes: root_ha, ..
        } = root_bounds
        else {
            panic!("expected OBB");
        };
        let child = SpatialHierarchy::children(&h, nid(0))[0];
        let SpatialBounds::OrientedBox {
            half_axes: child_ha,
            ..
        } = h.bounds(child)
        else {
            panic!("expected OBB");
        };
        // X and Y half-axes should be half as large.
        assert!(
            (child_ha.x_axis.length() - root_ha.x_axis.length() * 0.5).abs() < 1e-6,
            "x half-axis should halve: root={} child={}",
            root_ha.x_axis.length(),
            child_ha.x_axis.length()
        );
        assert!(
            (child_ha.y_axis.length() - root_ha.y_axis.length() * 0.5).abs() < 1e-6,
            "y half-axis should halve"
        );
        // Z should be unchanged.
        assert!(
            (child_ha.z_axis.length() - root_ha.z_axis.length()).abs() < 1e-6,
            "z half-axis should be unchanged"
        );
    }

    #[test]
    fn expand_invalid_parent_returns_error() {
        let mut h = make_hierarchy(2);
        let patch = HierarchyExpansion::new(nid(9999));
        assert!(h.expand(patch).is_err());
    }

    #[test]
    fn expand_re_expands_node() {
        let mut h = make_hierarchy(2);
        assert_eq!(h.children(nid(0)).len(), 4);
        let patch = HierarchyExpansion::new(nid(0));
        h.expand(patch).unwrap();
        // expand re-expands immediately; children stay intact.
        assert!(h.nodes[0].expanded);
        assert_eq!(h.children(nid(0)).len(), 4);
    }

    #[test]
    fn refinement_mode_propagates() {
        let avail = all_available_availability(2);
        let h = ImplicitQuadtreeHierarchy::new(
            &obb_root_bv(),
            512.0,
            &tiles3d::ImplicitTiling::default(),
            avail,
            "c/{level}/{x}/{y}.glb",
            true, // additive
        );
        assert_eq!(h.refinement_mode(nid(0)), RefinementMode::Add);
    }
}
