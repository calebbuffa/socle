//! Geometry helpers: node transforms, bounding boxes, ray intersection, skirt metadata.

use crate::{GltfModel, Node, accessor::get_position_accessor};
use glam::{DMat4, DQuat, DVec3};
use zukei::{AxisAlignedBoundingBox, Ray};

/// Get the local-space transformation matrix for a node.
pub fn get_node_transform(node: &Node) -> Option<DMat4> {
    if node.matrix.len() == 16 {
        let m: [f64; 16] = node.matrix.as_slice().try_into().ok()?;
        return Some(DMat4::from_cols_array(&m));
    }
    let t = if node.translation.len() == 3 {
        DVec3::new(
            node.translation[0],
            node.translation[1],
            node.translation[2],
        )
    } else {
        DVec3::ZERO
    };
    let r = if node.rotation.len() == 4 {
        DQuat::from_xyzw(
            node.rotation[0],
            node.rotation[1],
            node.rotation[2],
            node.rotation[3],
        )
    } else {
        DQuat::IDENTITY
    };
    let s = if node.scale.len() == 3 {
        DVec3::new(node.scale[0], node.scale[1], node.scale[2])
    } else {
        DVec3::ONE
    };
    Some(DMat4::from_scale_rotation_translation(s, r, t))
}

/// Set the local-space transformation matrix for a node (overwrites TRS components).
pub fn set_node_transform(node: &mut Node, mat: DMat4) {
    node.matrix = mat.to_cols_array().to_vec();
    node.translation.clear();
    node.rotation.clear();
    node.scale.clear();
}

/// Apply the CESIUM_RTC extension RTC_CENTER to a root transform.
pub fn apply_rtc_center(model: &GltfModel, root_transform: DMat4) -> DMat4 {
    let center = model
        .extensions
        .get("CESIUM_RTC")
        .and_then(|v| v.get("center"))
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            if arr.len() >= 3 {
                Some(DVec3::new(
                    arr[0].as_f64()?,
                    arr[1].as_f64()?,
                    arr[2].as_f64()?,
                ))
            } else {
                None
            }
        });
    if let Some(c) = center {
        DMat4::from_translation(c) * root_transform
    } else {
        root_transform
    }
}

/// Apply the gltfUpAxis extras value to the root transform.
pub fn apply_gltf_up_axis_transform(model: &GltfModel, root_transform: DMat4) -> DMat4 {
    let axis = model
        .extras
        .as_ref()
        .and_then(|e| e.get("gltfUpAxis"))
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0);
    zukei::apply_up_axis_correction(root_transform, axis as i64)
}

/// Data about a ray / glTF hit.
#[derive(Debug, Clone, Copy)]
pub struct RayGltfHit {
    pub primitive_point: DVec3,
    pub world_point: DVec3,
    pub primitive_to_world: DMat4,
}

/// Test a ray against all triangle primitives in a model.
pub fn intersect_ray_gltf(
    model: &GltfModel,
    ray: &Ray,
    model_to_world: DMat4,
) -> Option<RayGltfHit> {
    let mut best_t = f64::INFINITY;
    let mut best_hit: Option<RayGltfHit> = None;

    for node in &model.nodes {
        let node_local = get_node_transform(node).unwrap_or(DMat4::IDENTITY);
        let node_world = model_to_world * node_local;
        let mesh_idx = match node.mesh {
            Some(i) => i,
            None => continue,
        };
        let mesh = match model.meshes.get(mesh_idx) {
            Some(m) => m,
            None => continue,
        };

        for prim in &mesh.primitives {
            let positions = match get_position_accessor(model, prim) {
                Ok(p) => p,
                Err(_) => continue,
            };
            let num_tris = positions.len() / 3;

            for tri in 0..num_tris {
                let (i0, i1, i2) = (tri * 3, tri * 3 + 1, tri * 3 + 2);
                let (v0, v1, v2) = match (positions.get(i0), positions.get(i1), positions.get(i2)) {
                    (Some(a), Some(b), Some(c)) => (a, b, c),
                    _ => continue,
                };
                let p0 = DVec3::new(v0.x as f64, v0.y as f64, v0.z as f64);
                let p1 = DVec3::new(v1.x as f64, v1.y as f64, v1.z as f64);
                let p2 = DVec3::new(v2.x as f64, v2.y as f64, v2.z as f64);
                if let Some(t) = zukei::ray_triangle(ray, p0, p1, p2) {
                    if t > 0.0 && t < best_t {
                        best_t = t;
                        let pp = ray.at(t);
                        best_hit = Some(RayGltfHit {
                            primitive_point: pp,
                            world_point: node_world.transform_point3(pp),
                            primitive_to_world: node_world,
                        });
                    }
                }
            }
        }
    }
    best_hit
}

/// Compute an axis-aligned bounding box in world space over all triangle
/// positions in every node of the model.
///
/// `model_to_world` is applied to each position before accumulating. Pass
/// `DMat4::IDENTITY` if the model is already in world space.
///
/// Returns `None` if the model contains no geometry.
pub fn compute_bounding_box(model: &GltfModel, model_to_world: DMat4) -> Option<AxisAlignedBoundingBox> {
    let mut bbox = AxisAlignedBoundingBox::EMPTY;

    for node in &model.nodes {
        let node_local = get_node_transform(node).unwrap_or(DMat4::IDENTITY);
        let node_world = model_to_world * node_local;
        let mesh_idx = node.mesh?;
        let mesh = model.meshes.get(mesh_idx)?;

        for prim in &mesh.primitives {
            let positions = match get_position_accessor(model, prim) {
                Ok(p) => p,
                Err(_) => continue,
            };
            for i in 0..positions.len() {
                if let Some(p) = positions.get(i) {
                    let wp =
                        node_world.transform_point3(DVec3::new(p.x as f64, p.y as f64, p.z as f64));
                    bbox = bbox.expand(wp);
                }
            }
        }
    }

    if bbox.is_empty() { None } else { Some(bbox) }
}

/// Terrain-mesh skirt metadata stored in a glTF mesh's `extras` field.
///
/// Skirts are extra triangles appended to the edge of a terrain tile to hide
/// cracks between adjacent tiles of different levels of detail. This type
/// mirrors `CesiumGltfContent::SkirtMeshMetadata`.
///
/// ## Wire format (in `mesh.extras["skirtMeshMetadata"]`)
///
/// ```json
/// {
///   "skirtMeshMetadata": {
///     "noSkirtRange": [indicesBegin, indicesCount, verticesBegin, verticesCount],
///     "meshCenter": [x, y, z],
///     "skirtWestHeight": 0.0,
///     "skirtSouthHeight": 0.0,
///     "skirtEastHeight": 0.0,
///     "skirtNorthHeight": 0.0
///   }
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SkirtMeshMetadata {
    /// Index of the first index in the "no-skirt" (core) sub-range.
    pub no_skirt_indices_begin: u32,
    /// Count of indices in the "no-skirt" sub-range.
    pub no_skirt_indices_count: u32,
    /// Index of the first vertex in the "no-skirt" sub-range.
    pub no_skirt_vertices_begin: u32,
    /// Count of vertices in the "no-skirt" sub-range.
    pub no_skirt_vertices_count: u32,
    /// ECEF center of the mesh, used to reconstruct positions.
    pub mesh_center: [f64; 3],
    /// Height of the skirt along the western edge (metres).
    pub skirt_west_height: f64,
    /// Height of the skirt along the southern edge (metres).
    pub skirt_south_height: f64,
    /// Height of the skirt along the eastern edge (metres).
    pub skirt_east_height: f64,
    /// Height of the skirt along the northern edge (metres).
    pub skirt_north_height: f64,
}

impl SkirtMeshMetadata {
    /// Parse from a glTF extras `serde_json::Value`.
    ///
    /// Expects the value to contain a `"skirtMeshMetadata"` key at the top
    /// level, matching the format produced by [`SkirtMeshMetadata::to_extras`].
    ///
    /// Returns `None` if any required field is missing or has the wrong type.
    pub fn parse_from_extras(extras: &serde_json::Value) -> Option<Self> {
        let meta = extras.get("skirtMeshMetadata")?;

        let no_skirt_range = meta.get("noSkirtRange")?.as_array()?;
        if no_skirt_range.len() < 4 {
            return None;
        }
        let no_skirt_indices_begin = no_skirt_range[0].as_u64()? as u32;
        let no_skirt_indices_count = no_skirt_range[1].as_u64()? as u32;
        let no_skirt_vertices_begin = no_skirt_range[2].as_u64()? as u32;
        let no_skirt_vertices_count = no_skirt_range[3].as_u64()? as u32;

        let center = meta.get("meshCenter")?.as_array()?;
        if center.len() < 3 {
            return None;
        }
        let mesh_center = [
            center[0].as_f64()?,
            center[1].as_f64()?,
            center[2].as_f64()?,
        ];

        let skirt_west_height = meta.get("skirtWestHeight")?.as_f64()?;
        let skirt_south_height = meta.get("skirtSouthHeight")?.as_f64()?;
        let skirt_east_height = meta.get("skirtEastHeight")?.as_f64()?;
        let skirt_north_height = meta.get("skirtNorthHeight")?.as_f64()?;

        Some(Self {
            no_skirt_indices_begin,
            no_skirt_indices_count,
            no_skirt_vertices_begin,
            no_skirt_vertices_count,
            mesh_center,
            skirt_west_height,
            skirt_south_height,
            skirt_east_height,
            skirt_north_height,
        })
    }

    /// Serialize to the glTF extras JSON object format.
    pub fn to_extras(&self) -> serde_json::Value {
        serde_json::json!({
            "skirtMeshMetadata": {
                "noSkirtRange": [
                    self.no_skirt_indices_begin,
                    self.no_skirt_indices_count,
                    self.no_skirt_vertices_begin,
                    self.no_skirt_vertices_count
                ],
                "meshCenter": [
                    self.mesh_center[0],
                    self.mesh_center[1],
                    self.mesh_center[2]
                ],
                "skirtWestHeight": self.skirt_west_height,
                "skirtSouthHeight": self.skirt_south_height,
                "skirtEastHeight": self.skirt_east_height,
                "skirtNorthHeight": self.skirt_north_height
            }
        })
    }
}
