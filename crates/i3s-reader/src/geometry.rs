//! Binary geometry buffer parsing for I3S.
//!
//! I3S geometry buffers are packed little-endian binary blobs. The layout
//! is driven by the `DefaultGeometrySchema` (v1.6) or `GeometryBuffer` (v1.7+)
//! definitions from the scene layer document.
//!
//! ## Buffer Layout (v1.6 / DefaultGeometrySchema)
//!
//! ```text
//! Header:     vertexCount (u32), featureCount (u32)
//! Per-vertex: position  (f32 × 3 × vertexCount)
//!             normal    (f32 × 3 × vertexCount)   [optional]
//!             uv0       (f32 × 2 × vertexCount)   [optional]
//!             color     (u8  × 4 × vertexCount)   [optional]
//!             uvRegion  (u16 × 4 × vertexCount)   [optional]
//! Per-feature: featureId (u64 × featureCount)
//!              faceRange (u32 × 2 × featureCount)
//! ```
//!
//! All values are little-endian.

use byteorder::{LittleEndian, ReadBytesExt};
use i3s_util::{I3SError, Result};
use std::io::{Cursor, Read};

/// Decoded geometry from an I3S binary geometry buffer.
#[derive(Debug, Clone, Default)]
pub struct GeometryData {
    pub vertex_count: u32,
    pub feature_count: u32,
    pub positions: Vec<[f32; 3]>,
    pub normals: Option<Vec<[f32; 3]>>,
    pub uv0: Option<Vec<[f32; 2]>>,
    pub colors: Option<Vec<[u8; 4]>>,
    pub uv_region: Option<Vec<[u16; 4]>>,
    pub feature_ids: Option<Vec<u64>>,
    pub face_ranges: Option<Vec<[u32; 2]>>,
}

/// Describes which vertex/feature attributes are present in a geometry buffer.
///
/// Constructed from an `i3s::geometry::GeometryBuffer` or
/// `i3s::geometry::DefaultGeometrySchema`.
#[derive(Debug, Clone, Default)]
pub struct GeometryLayout {
    /// Byte offset from the start of the buffer (legacy header skip).
    pub offset: u32,
    pub has_position: bool,
    pub has_normal: bool,
    pub has_uv0: bool,
    pub has_color: bool,
    pub color_components: u8,
    pub has_uv_region: bool,
    pub has_feature_id: bool,
    pub feature_id_bytes: u8,
    pub has_face_range: bool,
}

impl GeometryLayout {
    /// Build a layout from an `i3s::geometry::GeometryBuffer` (v1.7+).
    pub fn from_geometry_buffer(buf: &i3s::cmn::GeometryBuffer) -> Self {
        Self {
            offset: buf.offset.unwrap_or(0) as u32,
            has_position: buf.position.is_some(),
            has_normal: buf.normal.is_some(),
            has_uv0: buf.uv0.is_some(),
            has_color: buf.color.is_some(),
            color_components: buf.color.as_ref().map(|c| c.component as u8).unwrap_or(4),
            has_uv_region: buf.uv_region.is_some(),
            has_feature_id: buf.feature_id.is_some(),
            feature_id_bytes: buf
                .feature_id
                .as_ref()
                .map(|f| match &f.r#type {
                    i3s::cmn::GeometryFeatureIDType::UInt64 => 8,
                    i3s::cmn::GeometryFeatureIDType::UInt32 => 4,
                    i3s::cmn::GeometryFeatureIDType::UInt16 => 2,
                })
                .unwrap_or(8),
            has_face_range: buf.face_range.is_some(),
        }
    }

    /// Build a default layout from `DefaultGeometrySchema` (v1.6).
    ///
    /// The default schema uses a fixed attribute order: position, normal,
    /// uv0, color, uvRegion, featureId, faceRange. Presence is determined
    /// by whether the vertex_attributes/feature_attributes declare them.
    pub fn from_default_schema(schema: &i3s::cmn::DefaultGeometrySchema) -> Self {
        // In the default schema, the ordering field lists which attributes exist.
        let ordering: Vec<String> = schema.ordering.clone();
        let has = |name: &str| ordering.iter().any(|s| s == name);

        Self {
            offset: schema
                .header
                .iter()
                .map(|h| header_attr_size(&h.r#type))
                .sum::<u32>(),
            has_position: has("position"),
            has_normal: has("normal"),
            has_uv0: has("uv0"),
            has_color: has("color"),
            color_components: 4,
            has_uv_region: has("region") || has("uvRegion"),
            has_feature_id: schema
                .feature_attribute_order
                .iter()
                .any(|s| s == "id" || s == "featureId"),
            feature_id_bytes: 8,
            has_face_range: schema
                .feature_attribute_order
                .iter()
                .any(|s| s == "faceRange"),
        }
    }
}

/// Returns the byte size of a header attribute type.
fn header_attr_size(t: &i3s::cmn::HeaderAttributeType) -> u32 {
    use i3s::cmn::HeaderAttributeType::*;
    match t {
        UInt8 => 1,
        UInt16 | Int16 => 2,
        UInt32 | Int32 => 4,
        UInt64 | Int64 => 8,
        Float32 => 4,
        Float64 => 8,
    }
}

/// Parse an I3S binary geometry buffer using the given layout.
///
/// The buffer is expected to start with a header (vertexCount u32, featureCount u32)
/// followed by tightly packed vertex and feature attribute arrays.
///
/// # Errors
///
/// Returns [`I3SError::Buffer`] if the buffer is too short for the declared
/// vertex/feature counts, or if any read fails.
pub fn parse_geometry_buffer(data: &[u8], layout: &GeometryLayout) -> Result<GeometryData> {
    let mut cursor = Cursor::new(data);

    // Skip legacy header offset bytes
    if layout.offset > 0 {
        let mut skip = vec![0u8; layout.offset as usize];
        cursor
            .read_exact(&mut skip)
            .map_err(|e| I3SError::Buffer(format!("failed to skip offset: {e}")))?;
    }

    // Read header: vertexCount, featureCount
    let vertex_count = cursor
        .read_u32::<LittleEndian>()
        .map_err(|e| I3SError::Buffer(format!("failed to read vertex count: {e}")))?;
    let feature_count = cursor
        .read_u32::<LittleEndian>()
        .map_err(|e| I3SError::Buffer(format!("failed to read feature count: {e}")))?;

    let mut geo = GeometryData {
        vertex_count,
        feature_count,
        ..Default::default()
    };

    if layout.has_position {
        geo.positions = read_array(&mut cursor, vertex_count, "position")?;
    }

    if layout.has_normal {
        geo.normals = Some(read_array(&mut cursor, vertex_count, "normal")?);
    }

    if layout.has_uv0 {
        geo.uv0 = Some(read_array(&mut cursor, vertex_count, "uv0")?);
    }

    if layout.has_color {
        let n = layout.color_components;
        geo.colors = Some(read_color_array(&mut cursor, vertex_count, n, "color")?);
    }

    if layout.has_uv_region {
        geo.uv_region = Some(read_array(&mut cursor, vertex_count, "uvRegion")?);
    }

    if layout.has_feature_id && feature_count > 0 {
        geo.feature_ids = Some(read_feature_ids(
            &mut cursor,
            feature_count,
            layout.feature_id_bytes,
        )?);
    }

    if layout.has_face_range && feature_count > 0 {
        geo.face_ranges = Some(read_array(&mut cursor, feature_count, "faceRange")?);
    }

    Ok(geo)
}

/// Trait for types that can be read element-by-element from a binary cursor.
trait ReadElement: Sized {
    fn read_from(cursor: &mut Cursor<&[u8]>) -> std::io::Result<Self>;
}

impl ReadElement for [f32; 3] {
    fn read_from(cursor: &mut Cursor<&[u8]>) -> std::io::Result<Self> {
        Ok([
            cursor.read_f32::<LittleEndian>()?,
            cursor.read_f32::<LittleEndian>()?,
            cursor.read_f32::<LittleEndian>()?,
        ])
    }
}

impl ReadElement for [f32; 2] {
    fn read_from(cursor: &mut Cursor<&[u8]>) -> std::io::Result<Self> {
        Ok([
            cursor.read_f32::<LittleEndian>()?,
            cursor.read_f32::<LittleEndian>()?,
        ])
    }
}

impl ReadElement for [u16; 4] {
    fn read_from(cursor: &mut Cursor<&[u8]>) -> std::io::Result<Self> {
        Ok([
            cursor.read_u16::<LittleEndian>()?,
            cursor.read_u16::<LittleEndian>()?,
            cursor.read_u16::<LittleEndian>()?,
            cursor.read_u16::<LittleEndian>()?,
        ])
    }
}

impl ReadElement for [u32; 2] {
    fn read_from(cursor: &mut Cursor<&[u8]>) -> std::io::Result<Self> {
        Ok([
            cursor.read_u32::<LittleEndian>()?,
            cursor.read_u32::<LittleEndian>()?,
        ])
    }
}

/// Read `count` elements of type `T` from the cursor.
fn read_array<T: ReadElement>(
    cursor: &mut Cursor<&[u8]>,
    count: u32,
    name: &str,
) -> Result<Vec<T>> {
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        out.push(T::read_from(cursor).map_err(|e| I3SError::Buffer(format!("{name}: {e}")))?);
    }
    Ok(out)
}

fn read_feature_ids(cursor: &mut Cursor<&[u8]>, count: u32, bytes_per_id: u8) -> Result<Vec<u64>> {
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let id = match bytes_per_id {
            2 => cursor
                .read_u16::<LittleEndian>()
                .map(u64::from)
                .map_err(|e| I3SError::Buffer(format!("featureId: {e}")))?,
            4 => cursor
                .read_u32::<LittleEndian>()
                .map(u64::from)
                .map_err(|e| I3SError::Buffer(format!("featureId: {e}")))?,
            _ => cursor
                .read_u64::<LittleEndian>()
                .map_err(|e| I3SError::Buffer(format!("featureId: {e}")))?,
        };
        out.push(id);
    }
    Ok(out)
}

/// Read color components — special case because the component count is variable
/// and missing alpha defaults to 255.
fn read_color_array(
    cursor: &mut Cursor<&[u8]>,
    count: u32,
    components: u8,
    name: &str,
) -> Result<Vec<[u8; 4]>> {
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let mut rgba = [255u8; 4];
        for c in 0..components.min(4) {
            rgba[c as usize] = cursor
                .read_u8()
                .map_err(|e| I3SError::Buffer(format!("{name}: {e}")))?;
        }
        out.push(rgba);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use byteorder::WriteBytesExt;

    /// Build a synthetic geometry buffer: 3 vertices with position only, 1 feature.
    fn make_test_buffer() -> Vec<u8> {
        let mut buf = Vec::new();
        // Header
        buf.write_u32::<LittleEndian>(3).unwrap(); // vertexCount
        buf.write_u32::<LittleEndian>(1).unwrap(); // featureCount
        // Positions: 3 vertices × 3 floats
        for v in &[[1.0f32, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]] {
            for &c in v {
                buf.write_f32::<LittleEndian>(c).unwrap();
            }
        }
        // FeatureId: 1 × u64
        buf.write_u64::<LittleEndian>(42).unwrap();
        // FaceRange: 1 × 2 × u32
        buf.write_u32::<LittleEndian>(0).unwrap();
        buf.write_u32::<LittleEndian>(0).unwrap();
        buf
    }

    #[test]
    fn parse_position_only() {
        let buf = make_test_buffer();
        let layout = GeometryLayout {
            has_position: true,
            has_feature_id: true,
            feature_id_bytes: 8,
            has_face_range: true,
            ..Default::default()
        };
        let geo = parse_geometry_buffer(&buf, &layout).unwrap();
        assert_eq!(geo.vertex_count, 3);
        assert_eq!(geo.feature_count, 1);
        assert_eq!(geo.positions.len(), 3);
        assert_eq!(geo.positions[0], [1.0, 2.0, 3.0]);
        assert_eq!(geo.positions[2], [7.0, 8.0, 9.0]);
        assert_eq!(geo.feature_ids.unwrap(), vec![42]);
        assert_eq!(geo.face_ranges.unwrap(), vec![[0, 0]]);
        assert!(geo.normals.is_none());
        assert!(geo.uv0.is_none());
        assert!(geo.colors.is_none());
    }

    #[test]
    fn parse_with_normals_and_uv() {
        let mut buf = Vec::new();
        let vc = 2u32;
        let fc = 0u32;
        buf.write_u32::<LittleEndian>(vc).unwrap();
        buf.write_u32::<LittleEndian>(fc).unwrap();
        // Positions
        for &c in &[0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0] {
            buf.write_f32::<LittleEndian>(c).unwrap();
        }
        // Normals
        for &c in &[0.0f32, 1.0, 0.0, 0.0, 1.0, 0.0] {
            buf.write_f32::<LittleEndian>(c).unwrap();
        }
        // UV0
        for &c in &[0.0f32, 0.0, 1.0, 1.0] {
            buf.write_f32::<LittleEndian>(c).unwrap();
        }

        let layout = GeometryLayout {
            has_position: true,
            has_normal: true,
            has_uv0: true,
            ..Default::default()
        };
        let geo = parse_geometry_buffer(&buf, &layout).unwrap();
        assert_eq!(geo.positions.len(), 2);
        assert_eq!(geo.normals.as_ref().unwrap().len(), 2);
        assert_eq!(geo.normals.as_ref().unwrap()[0], [0.0, 1.0, 0.0]);
        assert_eq!(geo.uv0.as_ref().unwrap().len(), 2);
        assert_eq!(geo.uv0.as_ref().unwrap()[1], [1.0, 1.0]);
    }

    #[test]
    fn parse_with_colors() {
        let mut buf = Vec::new();
        buf.write_u32::<LittleEndian>(1).unwrap(); // 1 vertex
        buf.write_u32::<LittleEndian>(0).unwrap(); // 0 features
        // Position
        for &c in &[0.0f32, 0.0, 0.0] {
            buf.write_f32::<LittleEndian>(c).unwrap();
        }
        // Color: RGBA
        buf.extend_from_slice(&[255, 128, 64, 200]);

        let layout = GeometryLayout {
            has_position: true,
            has_color: true,
            color_components: 4,
            ..Default::default()
        };
        let geo = parse_geometry_buffer(&buf, &layout).unwrap();
        assert_eq!(geo.colors.as_ref().unwrap()[0], [255, 128, 64, 200]);
    }

    #[test]
    fn truncated_buffer_error() {
        let buf = vec![0u8; 4]; // only 4 bytes, needs 8 for header
        let layout = GeometryLayout {
            has_position: true,
            ..Default::default()
        };
        let result = parse_geometry_buffer(&buf, &layout);
        assert!(result.is_err());
    }

    #[test]
    fn parse_u16_feature_ids() {
        let mut buf = Vec::new();
        buf.write_u32::<LittleEndian>(0).unwrap(); // 0 vertices
        buf.write_u32::<LittleEndian>(2).unwrap(); // 2 features
        // FeatureIds as u16
        buf.write_u16::<LittleEndian>(100).unwrap();
        buf.write_u16::<LittleEndian>(200).unwrap();

        let layout = GeometryLayout {
            has_feature_id: true,
            feature_id_bytes: 2,
            ..Default::default()
        };
        let geo = parse_geometry_buffer(&buf, &layout).unwrap();
        assert_eq!(geo.feature_ids.unwrap(), vec![100, 200]);
    }
}
