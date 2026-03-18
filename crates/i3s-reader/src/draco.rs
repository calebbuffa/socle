//! Draco-compressed geometry buffer decoding for I3S.
//!
//! I3S v1.7+ supports Draco compression for triangle-mesh geometry buffers.
//! When `GeometryBuffer.compressed_attributes` is present, the entire buffer
//! is a raw Draco-encoded blob that decodes to a mesh with named attributes.
//!
//! Requires the `draco` feature flag.

use draco_core::{DecoderBuffer, GeometryAttributeType, MeshDecoder, PointIndex};

use i3s_util::{I3SError, Result};

use crate::codec::GeometryDecoder;
use crate::geometry::{GeometryData, GeometryLayout};

pub struct DracoGeometryDecoder;

impl GeometryDecoder for DracoGeometryDecoder {
    fn decode(&self, data: &[u8], _layout: &GeometryLayout) -> Result<GeometryData> {
        decode_draco_geometry(data, None, None)
    }
}

/// Decode a Draco-compressed geometry buffer into [`GeometryData`].
///
/// The input `data` is the raw bytes fetched from the I3S geometry resource.
/// Scale factors `scale_x` and `scale_y` are applied to positions if provided
/// (from the Draco metadata `i3s-scale_x` / `i3s-scale_y`).
///
/// # Errors
///
/// Returns [`I3SError::Draco`] if the Draco decoder fails.
pub fn decode_draco_geometry(
    data: &[u8],
    scale_x: Option<f64>,
    scale_y: Option<f64>,
) -> Result<GeometryData> {
    let mut buffer = DecoderBuffer::new(data);
    let mut decoder = MeshDecoder::new();
    let mut mesh = draco_core::Mesh::new();

    decoder
        .decode(&mut buffer, &mut mesh)
        .map_err(|e| I3SError::Draco(format!("{e}")))?;

    let num_points = mesh.num_points() as u32;

    let mut geo = GeometryData {
        vertex_count: num_points,
        feature_count: 0,
        ..Default::default()
    };

    // Extract position attribute
    if let Some(att) = mesh.named_attribute(GeometryAttributeType::Position) {
        let mut positions = Vec::with_capacity(num_points as usize);
        let sx = scale_x.unwrap_or(1.0) as f32;
        let sy = scale_y.unwrap_or(1.0) as f32;
        let apply_scale = (sx - 1.0).abs() > f32::EPSILON || (sy - 1.0).abs() > f32::EPSILON;

        for i in 0..num_points {
            let val = read_f32x3(att, PointIndex(i));
            if apply_scale {
                positions.push([val[0] * sx, val[1] * sy, val[2]]);
            } else {
                positions.push(val);
            }
        }
        geo.positions = positions;
    }

    // Extract normal attribute
    if let Some(att) = mesh.named_attribute(GeometryAttributeType::Normal) {
        let mut normals = Vec::with_capacity(num_points as usize);
        for i in 0..num_points {
            normals.push(read_f32x3(att, PointIndex(i)));
        }
        geo.normals = Some(normals);
    }

    // Extract texture coordinates
    if let Some(att) = mesh.named_attribute(GeometryAttributeType::TexCoord) {
        let mut uvs = Vec::with_capacity(num_points as usize);
        for i in 0..num_points {
            uvs.push(read_f32x2(att, PointIndex(i)));
        }
        geo.uv0 = Some(uvs);
    }

    // Extract color attribute
    if let Some(att) = mesh.named_attribute(GeometryAttributeType::Color) {
        let mut colors = Vec::with_capacity(num_points as usize);
        for i in 0..num_points {
            colors.push(read_u8x4(att, PointIndex(i)));
        }
        geo.colors = Some(colors);
    }

    // Extract per-face feature IDs from generic attributes
    if let Some(att) = mesh.named_attribute(GeometryAttributeType::Generic) {
        let num_faces = mesh.num_faces();
        let mut ids = Vec::with_capacity(num_faces);
        for i in 0..num_faces {
            let face = mesh.face(draco_core::FaceIndex(i as u32));
            // Feature ID is mapped per-point; use first vertex of each face
            let avi = att.mapped_index(face[0]);
            let byte_offset = u32::from(avi) as usize * att.byte_stride() as usize;
            let buf = att.buffer().data();
            // Read as u32 or u64 depending on component size
            let id = if att.num_components() == 1 && att.byte_stride() >= 8 {
                read_u64_le(buf, byte_offset)
            } else {
                read_u32_le(buf, byte_offset) as u64
            };
            ids.push(id);
        }
        geo.feature_ids = Some(ids);
        geo.feature_count = num_faces as u32;
    }

    Ok(geo)
}

/// Check whether a geometry buffer is Draco-compressed.
pub fn is_draco_compressed(buf: &i3s::geometry::GeometryBuffer) -> bool {
    buf.compressed_attributes.is_some()
}

/// Read a 3-component f32 from a PointAttribute at the given point index.
fn read_f32x3(att: &draco_core::PointAttribute, pi: PointIndex) -> [f32; 3] {
    let avi = att.mapped_index(pi);
    let byte_offset = u32::from(avi) as usize * att.byte_stride() as usize;
    let buf = att.buffer().data();
    [
        read_f32_le(buf, byte_offset),
        read_f32_le(buf, byte_offset + 4),
        read_f32_le(buf, byte_offset + 8),
    ]
}

/// Read a 2-component f32 from a PointAttribute at the given point index.
fn read_f32x2(att: &draco_core::PointAttribute, pi: PointIndex) -> [f32; 2] {
    let avi = att.mapped_index(pi);
    let byte_offset = u32::from(avi) as usize * att.byte_stride() as usize;
    let buf = att.buffer().data();
    [
        read_f32_le(buf, byte_offset),
        read_f32_le(buf, byte_offset + 4),
    ]
}

/// Read a 4-component u8 from a PointAttribute at the given point index.
fn read_u8x4(att: &draco_core::PointAttribute, pi: PointIndex) -> [u8; 4] {
    let avi = att.mapped_index(pi);
    let byte_offset = u32::from(avi) as usize * att.byte_stride() as usize;
    let buf = att.buffer().data();
    [
        buf[byte_offset],
        buf[byte_offset + 1],
        buf[byte_offset + 2],
        buf[byte_offset + 3],
    ]
}

fn read_f32_le(buf: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
    ])
}

fn read_u32_le(buf: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
    ])
}

fn read_u64_le(buf: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
        buf[offset + 4],
        buf[offset + 5],
        buf[offset + 6],
        buf[offset + 7],
    ])
}
