//! Explicit-tileset adapter for `selekt::SceneGraph`.
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
use selekt::{ContentKey, LodDescriptor, NodeId, NodeKind, RefinementMode, SceneGraph};
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

/// A `SceneGraph` built from an explicit (non-implicit) 3D Tiles
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
        // Pass parent_world_transform directly as the accumulated transform.
        // flatten_tile composes it with the tile's own local transform, so
        // the root tile's transform is applied exactly once.
        let root_refinement = parse_refinement(tileset.root.refine.as_ref());
        flatten_tile(
            &tileset.root,
            None,
            parent_world_transform,
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

    /// Resolve all relative content keys against the given base URL in-place.
    ///
    /// Call this after building a hierarchy from a sub-tileset so that content
    /// keys are absolute URLs understood by the outer `TilesetLoader`.
    pub fn resolve_content_keys(&mut self, base_url: &str) {
        for node in &mut self.nodes {
            for key in &mut node.content_keys {
                key.0 = orkester_io::resolve_url(base_url, &key.0).into();
            }
        }
    }
}

impl SceneGraph for ExplicitTilesetHierarchy {
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

    fn node_count(&self) -> usize {
        self.nodes.len()
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
        // Region bounds are in geographic coordinates — their ECEF corners
        // are already in world space and must NOT be transformed by the
        // tile's local transform (which is for tile-local content only).
        let r = &bv.region;
        let (west, south, east, north, min_h, max_h) = (r[0], r[1], r[2], r[3], r[4], r[5]);
        let corners = region_ecef_corners(west, south, east, north, min_h, max_h);
        let mut mn = corners[0];
        let mut mx = corners[0];
        for &c in &corners[1..] {
            mn = mn.min(c);
            mx = mx.max(c);
        }
        let center = (mn + mx) * 0.5;
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

/// A `selekt::SceneGraph` backed by a 3D Tiles implicit octree.
///
/// Construct from the root tile's bounding volume, the implicit tiling
/// descriptor, and a loaded [`OctreeAvailability`].  Children are inserted
/// eagerly so [`SceneGraph::children`] never requires interior mutation.
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

impl SceneGraph for ImplicitOctreeHierarchy {
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

    fn node_count(&self) -> usize {
        self.nodes.len()
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

/// A `selekt::SceneGraph` backed by a 3D Tiles implicit quadtree.
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

impl SceneGraph for ImplicitQuadtreeHierarchy {
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

    fn node_count(&self) -> usize {
        self.nodes.len()
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
