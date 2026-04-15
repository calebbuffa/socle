//! Terrain-mesh skirt metadata stored in a glTF mesh's `extras` field.
//!
//! Skirts are extra triangles appended to the edge of a terrain tile to hide
//! cracks between adjacent tiles of different levels of detail. This type
//! mirrors `CesiumGltfContent::SkirtMeshMetadata`.
//!
//! ## Wire format (in `mesh.extras["skirtMeshMetadata"]`)
//!
//! ```json
//! {
//!   "skirtMeshMetadata": {
//!     "noSkirtRange": [indicesBegin, indicesCount, verticesBegin, verticesCount],
//!     "meshCenter": [x, y, z],
//!     "skirtWestHeight": 0.0,
//!     "skirtSouthHeight": 0.0,
//!     "skirtEastHeight": 0.0,
//!     "skirtNorthHeight": 0.0
//!   }
//! }
//! ```

#[derive(Debug, Clone, PartialEq, Default)]
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
