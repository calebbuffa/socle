//! I3S geometry buffer → [`GltfModel`] decoder.
//!
//! # Paths
//!
//! * **Uncompressed** — parses the binary buffer sequentially following the
//!   fixed I3S attribute order:
//!   `position → normal → uv0 → color → uvRegion → featureId → faceRange`
//!   Each present attribute is pushed as a separate accessor.
//!
//! * **Draco** (feature `draco`) — calls
//!   `moderu_codec::draco::decode_buffer` and maps the resulting
//!   `DecodedMesh` into a `GltfModel`.  Only active when
//!   `geom_buf.compressed_attributes` is `Some(...)` and the crate is built
//!   with the `draco` feature.

use i3s::cmn::{CompressedAttributesEncoding, GeometryBuffer};
use moderu::{AccessorType, GltfModel, GltfModelBuilder, PrimitiveMode};

// ── Error ────────────────────────────────────────────────────────────────────

/// Errors from the I3S geometry decode pipeline.
#[derive(Debug, thiserror::Error)]
pub enum GeometryDecodeError {
    /// Buffer is shorter than expected for the given vertex count.
    #[error("geometry buffer truncated: need {expected} bytes, got {actual}")]
    Truncated { expected: usize, actual: usize },
    /// Compressed attributes use an unsupported encoding.
    #[error("unsupported encoding: {0}")]
    UnsupportedEncoding(String),
    /// Draco decompression failed.
    #[error("draco: {0}")]
    Draco(String),
    /// No usable geometry representation was found.
    #[error("no usable geometry representation")]
    NoGeometry,
}

// ── Helper readers ────────────────────────────────────────────────────────────

/// Read `count` little-endian `f32` values starting at `*offset`.
fn read_f32s(
    data: &[u8],
    offset: &mut usize,
    count: usize,
) -> Result<Vec<f32>, GeometryDecodeError> {
    let byte_len = count * 4;
    if *offset + byte_len > data.len() {
        return Err(GeometryDecodeError::Truncated {
            expected: *offset + byte_len,
            actual: data.len(),
        });
    }
    let values = data[*offset..*offset + byte_len]
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    *offset += byte_len;
    Ok(values)
}

/// Read `count` little-endian `u16` values starting at `*offset`.
fn read_u16s(
    data: &[u8],
    offset: &mut usize,
    count: usize,
) -> Result<Vec<u16>, GeometryDecodeError> {
    let byte_len = count * 2;
    if *offset + byte_len > data.len() {
        return Err(GeometryDecodeError::Truncated {
            expected: *offset + byte_len,
            actual: data.len(),
        });
    }
    let values = data[*offset..*offset + byte_len]
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect();
    *offset += byte_len;
    Ok(values)
}

/// Read `count` raw bytes starting at `*offset` (used for u8 color).
fn read_u8s(
    data: &[u8],
    offset: &mut usize,
    count: usize,
) -> Result<Vec<u8>, GeometryDecodeError> {
    if *offset + count > data.len() {
        return Err(GeometryDecodeError::Truncated {
            expected: *offset + count,
            actual: data.len(),
        });
    }
    let values = data[*offset..*offset + count].to_vec();
    *offset += count;
    Ok(values)
}

/// Skip `n` bytes.
fn skip(
    data: &[u8],
    offset: &mut usize,
    n: usize,
) -> Result<(), GeometryDecodeError> {
    if *offset + n > data.len() {
        return Err(GeometryDecodeError::Truncated {
            expected: *offset + n,
            actual: data.len(),
        });
    }
    *offset += n;
    Ok(())
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Decode a raw I3S geometry buffer into a [`GltfModel`].
///
/// # Arguments
///
/// * `data` — raw bytes as fetched from
///   `layers/{id}/nodes/{n}/geometries/{buf_idx}`.
/// * `geom_buf` — the `GeometryBuffer` descriptor from
///   `layer.geometryDefinitions[def].geometryBuffers[buf_idx]`.
/// * `vertex_count` — number of vertices from `node.mesh.geometry.vertexCount`.
///   Ignored for Draco (vertex count comes from the decompressed stream).
pub fn decode_geometry(
    data: &[u8],
    geom_buf: &GeometryBuffer,
    vertex_count: usize,
) -> Result<GltfModel, GeometryDecodeError> {
    // Draco-compressed path
    if let Some(ca) = &geom_buf.compressed_attributes {
        return decode_draco(data, ca);
    }

    // Uncompressed path
    decode_uncompressed(data, geom_buf, vertex_count)
}

// ── Uncompressed path ─────────────────────────────────────────────────────────

fn decode_uncompressed(
    data: &[u8],
    desc: &GeometryBuffer,
    vertex_count: usize,
) -> Result<GltfModel, GeometryDecodeError> {
    if vertex_count == 0 {
        return Err(GeometryDecodeError::NoGeometry);
    }

    let mut offset = desc.offset.unwrap_or(0) as usize;

    let mut b = GltfModelBuilder::new();
    let mut prim = b.primitive();

    // ── POSITION (Float32×3, required) ────────────────────────────────────
    {
        let pos = read_f32s(data, &mut offset, vertex_count * 3)?;
        let acc = b.push_accessor(&pos, AccessorType::Vec3);
        prim = prim.attribute("POSITION", acc);
    }

    // ── NORMAL (Float32×3, optional) ──────────────────────────────────────
    if desc.normal.is_some() {
        let n = read_f32s(data, &mut offset, vertex_count * 3)?;
        let acc = b.push_accessor(&n, AccessorType::Vec3);
        prim = prim.attribute("NORMAL", acc);
    }

    // ── UV0 (Float32×2, optional) ─────────────────────────────────────────
    if desc.uv0.is_some() {
        let uv = read_f32s(data, &mut offset, vertex_count * 2)?;
        let acc = b.push_accessor(&uv, AccessorType::Vec2);
        prim = prim.attribute("TEXCOORD_0", acc);
    }

    // ── COLOR (UInt8×component, optional) ─────────────────────────────────
    if let Some(color_desc) = &desc.color {
        let components = color_desc.component.max(1) as usize;
        let c = read_u8s(data, &mut offset, vertex_count * components)?;
        // Map component count to accessor type; always normalised u8.
        let acc_type = match components {
            1 | 3 => AccessorType::Vec3, // expand grayscale / RGB (no alpha) to Vec3
            4 => AccessorType::Vec4,
            _ => AccessorType::Vec4,
        };
        // Pad to 3 or 4 components if needed.
        let padded: Vec<u8> = match components {
            1 => c.iter().flat_map(|&r| [r, r, r]).collect(),
            3 | 4 => c,
            _ => c,
        };
        let acc = b.push_accessor(&padded, acc_type);
        prim = prim.attribute("COLOR_0", acc);
    }

    // ── UV_REGION (UInt16×4, optional — texture atlas) ────────────────────
    if desc.uv_region.is_some() {
        let uv_r = read_u16s(data, &mut offset, vertex_count * 4)?;
        let acc = b.push_accessor(&uv_r, AccessorType::Vec4);
        prim = prim.attribute("_UV_REGION", acc);
    }

    // ── FEATURE_ID (UInt16 or UInt32 ×1, optional, per-feature) ──────────
    if let Some(feat) = &desc.feature_id {
        // Per spec: binding is per-feature (one id per feature, not per vertex)
        // We skip encoding this in the glTF model for now; it's used for
        // attribute lookup by i3s-selekt, not for rendering.
        let _ = feat;
        // TODO: encode as GLTF feature ID extension if needed by renderer
    }

    // ── FACE_RANGE (UInt32×2, per-feature, skip for rendering) ───────────
    // Not pushed into the glTF model; used by the attribute decoder.

    let prim_built = prim.build();
    b.push_mesh(prim_built);
    Ok(b.finish())
}

// ── Draco-compressed path ─────────────────────────────────────────────────────

fn decode_draco(
    data: &[u8],
    ca: &i3s::cmn::CompressedAttributes,
) -> Result<GltfModel, GeometryDecodeError> {
    use i3s::cmn::CompressedAttributesAttributes::*;

    // Only Draco encoding is supported.
    if ca.encoding != CompressedAttributesEncoding::Draco {
        return Err(GeometryDecodeError::UnsupportedEncoding(format!(
            "{:?}",
            ca.encoding
        )));
    }

    decode_draco_impl(data, &ca.attributes)
}

#[cfg(feature = "draco")]
fn decode_draco_impl(
    data: &[u8],
    attrs: &[i3s::cmn::CompressedAttributesAttributes],
) -> Result<GltfModel, GeometryDecodeError> {
    use i3s::cmn::CompressedAttributesAttributes as CAttr;
    use moderu_codec::draco;

    // Build the (semantic, draco_id) map. I3S assigns Draco unique IDs in the
    // order the attributes appear in the `compressedAttributes.attributes` array.
    let attr_map: Vec<(&str, u32)> = attrs
        .iter()
        .enumerate()
        .map(|(i, attr)| {
            let sem = match attr {
                CAttr::Position => "POSITION",
                CAttr::Normal => "NORMAL",
                CAttr::Uv0 => "TEXCOORD_0",
                CAttr::Color => "COLOR_0",
                CAttr::UvRegion => "_UV_REGION",
                CAttr::FeatureIndex => "_FEATURE_INDEX",
            };
            (sem, i as u32)
        })
        .collect();

    let mesh =
        draco::decode_buffer(data, &attr_map).map_err(GeometryDecodeError::Draco)?;

    build_model_from_decoded_mesh(&mesh)
}

#[cfg(not(feature = "draco"))]
fn decode_draco_impl(
    _data: &[u8],
    _attrs: &[i3s::cmn::CompressedAttributesAttributes],
) -> Result<GltfModel, GeometryDecodeError> {
    Err(GeometryDecodeError::Draco(
        "draco feature not enabled — rebuild i3s-content with --features draco".into(),
    ))
}

// ── Model assembly from DecodedMesh ──────────────────────────────────────────

#[cfg(feature = "draco")]
fn build_model_from_decoded_mesh(
    mesh: &moderu_codec::draco::DecodedMesh,
) -> Result<GltfModel, GeometryDecodeError> {
    let mut b = GltfModelBuilder::new();

    let idx_acc = b.push_indices(&mesh.indices);
    let mut prim = b.primitive().indices(idx_acc);

    for attr in &mesh.attributes {
        let acc_type = match attr.num_components {
            1 => AccessorType::Scalar,
            2 => AccessorType::Vec2,
            3 => AccessorType::Vec3,
            4 => AccessorType::Vec4,
            _ => AccessorType::Vec4,
        };
        let acc = b.push_accessor(&attr.data, acc_type);
        prim = prim.attribute(attr.name.clone(), acc);
    }

    let prim_built = prim.build();
    b.push_mesh(prim_built);
    Ok(b.finish())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use i3s::cmn::{
        GeometryBuffer, GeometryNormal, GeometryNormalType, GeometryPosition,
        GeometryPositionType, GeometryUV, GeometryUVType,
    };

    fn f32_le(vals: &[f32]) -> Vec<u8> {
        vals.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    fn u8s(vals: &[u8]) -> Vec<u8> {
        vals.to_vec()
    }

    fn u16_le(vals: &[u16]) -> Vec<u8> {
        vals.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    fn pos_buf() -> GeometryBuffer {
        GeometryBuffer {
            position: Some(GeometryPosition {
                r#type: GeometryPositionType::Float32,
                component: 3,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn position_only_decode() {
        let verts: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0];
        let data = f32_le(&verts);
        let model = decode_geometry(&data, &pos_buf(), 3).unwrap();
        assert_eq!(model.meshes.len(), 1);
        let prim = &model.meshes[0].primitives[0];
        assert!(prim.attributes.contains_key("POSITION"));
        assert!(!prim.attributes.contains_key("NORMAL"));
    }

    #[test]
    fn position_and_normal() {
        let pos: Vec<f32> = vec![0.0; 9]; // 3 verts × 3
        let norm: Vec<f32> = vec![0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0];
        let mut data = f32_le(&pos);
        data.extend(f32_le(&norm));

        let desc = GeometryBuffer {
            position: Some(GeometryPosition {
                r#type: GeometryPositionType::Float32,
                component: 3,
                ..Default::default()
            }),
            normal: Some(GeometryNormal {
                r#type: GeometryNormalType::Float32,
                component: 3,
                ..Default::default()
            }),
            ..Default::default()
        };

        let model = decode_geometry(&data, &desc, 3).unwrap();
        let prim = &model.meshes[0].primitives[0];
        assert!(prim.attributes.contains_key("POSITION"));
        assert!(prim.attributes.contains_key("NORMAL"));
    }

    #[test]
    fn position_and_uv0() {
        let pos: Vec<f32> = vec![0.0; 6]; // 2 verts × 3
        let uv: Vec<f32> = vec![0.5, 0.5, 1.0, 0.0];
        let mut data = f32_le(&pos);
        data.extend(f32_le(&uv));

        let desc = GeometryBuffer {
            position: Some(GeometryPosition {
                r#type: GeometryPositionType::Float32,
                component: 3,
                ..Default::default()
            }),
            uv0: Some(GeometryUV {
                r#type: GeometryUVType::Float32,
                component: 2,
                ..Default::default()
            }),
            ..Default::default()
        };

        let model = decode_geometry(&data, &desc, 2).unwrap();
        let prim = &model.meshes[0].primitives[0];
        assert!(prim.attributes.contains_key("TEXCOORD_0"));
    }

    #[test]
    fn truncated_buffer_returns_error() {
        let data: Vec<u8> = vec![0u8; 4]; // too short for 3 verts of positions
        let result = decode_geometry(&data, &pos_buf(), 3);
        assert!(matches!(result, Err(GeometryDecodeError::Truncated { .. })));
    }

    #[test]
    fn zero_vertex_count_returns_error() {
        let result = decode_geometry(&[], &pos_buf(), 0);
        assert!(matches!(result, Err(GeometryDecodeError::NoGeometry)));
    }

    #[test]
    fn legacy_offset_is_skipped() {
        // 8-byte legacy header followed by 1 vertex position
        let mut data = vec![0xFFu8; 8]; // header bytes to skip
        data.extend(f32_le(&[10.0, 20.0, 30.0]));

        let desc = GeometryBuffer {
            offset: Some(8),
            position: Some(GeometryPosition {
                r#type: GeometryPositionType::Float32,
                component: 3,
                ..Default::default()
            }),
            ..Default::default()
        };

        let model = decode_geometry(&data, &desc, 1).unwrap();
        // The position accessor should have 1 element.
        let prim = &model.meshes[0].primitives[0];
        let pos_acc_idx = prim.attributes["POSITION"];
        assert_eq!(model.accessors[pos_acc_idx].count, 1);
    }

    #[test]
    fn color_rgba_produces_vec4() {
        use i3s::cmn::{GeometryColor, GeometryColorType};
        let pos: Vec<f32> = vec![0.0; 3]; // 1 vert × 3
        let color: Vec<u8> = vec![255, 128, 64, 200]; // 1 vert × 4 RGBA
        let mut data = f32_le(&pos);
        data.extend(u8s(&color));

        let desc = GeometryBuffer {
            position: Some(GeometryPosition {
                r#type: GeometryPositionType::Float32,
                component: 3,
                ..Default::default()
            }),
            color: Some(GeometryColor {
                r#type: GeometryColorType::UInt8,
                component: 4,
                ..Default::default()
            }),
            ..Default::default()
        };

        let model = decode_geometry(&data, &desc, 1).unwrap();
        let prim = &model.meshes[0].primitives[0];
        assert!(prim.attributes.contains_key("COLOR_0"));
        let acc_idx = prim.attributes["COLOR_0"];
        assert_eq!(model.accessors[acc_idx].r#type, AccessorType::Vec4);
    }
}
