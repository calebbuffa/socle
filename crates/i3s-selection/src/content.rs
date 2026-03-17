//! Decoded content for a loaded I3S node.

use i3s_reader::attribute::AttributeData;
use i3s_reader::geometry::GeometryData;

/// The fully decoded content of a single I3S node, ready for rendering.
///
/// Produced by the content loader after fetching and decoding geometry,
/// textures, and attributes for a node.
#[derive(Debug, Clone)]
pub struct NodeContent {
    /// Decoded geometry (positions, normals, UVs, colors, feature IDs).
    pub geometry: GeometryData,
    /// Raw texture bytes in the format they were fetched (JPEG, PNG, KTX2, etc.).
    /// Empty if the node has no textures.
    pub texture_data: Vec<u8>,
    /// Decoded per-field attribute buffers.
    pub attributes: Vec<AttributeData>,
    /// Total byte size of all content (for cache accounting).
    pub byte_size: usize,
}

impl NodeContent {
    /// Estimate the byte size of this content.
    pub fn estimate_byte_size(
        geometry: &GeometryData,
        texture_data: &[u8],
        attributes: &[AttributeData],
    ) -> usize {
        let geo_size = geometry.positions.len() * std::mem::size_of::<[f32; 3]>()
            + geometry
                .normals
                .as_ref()
                .map_or(0, |n| n.len() * std::mem::size_of::<[f32; 3]>())
            + geometry
                .uv0
                .as_ref()
                .map_or(0, |u| u.len() * std::mem::size_of::<[f32; 2]>())
            + geometry
                .colors
                .as_ref()
                .map_or(0, |c| c.len() * std::mem::size_of::<[u8; 4]>())
            + geometry
                .feature_ids
                .as_ref()
                .map_or(0, |f| f.len() * std::mem::size_of::<u64>())
            + geometry
                .face_ranges
                .as_ref()
                .map_or(0, |f| f.len() * std::mem::size_of::<[u32; 2]>());

        let attr_size: usize = attributes.iter().map(|a| a.byte_size()).sum();

        geo_size + texture_data.len() + attr_size
    }
}
