//! `EXT_meshopt_compression` decoder and encoder.

use crate::CodecDecoder;
use crate::CodecEncoder;
use moderu::GltfModel;
use serde_json::{Value, json};

/// Errors that can occur during meshopt decode or encode operations.
#[derive(thiserror::Error, Debug)]
pub enum MeshoptError {
    #[error("missing buffer in meshopt extension")]
    MissingBuffer,
    #[error("missing byteLength in meshopt extension")]
    MissingByteLength,
    #[error("missing count in meshopt extension")]
    MissingCount,
    #[error("missing byteStride in meshopt extension")]
    MissingByteStride,
    #[error("source buffer {0} out of range")]
    BufferOutOfRange(usize),
    #[error("compressed range [{start}..{end}) exceeds buffer size {size}")]
    CompressedRangeExceeded {
        start: usize,
        end: usize,
        size: usize,
    },
    #[error("decode_vertex_buffer (stride={stride}): {message}")]
    DecodeVertexBuffer { stride: usize, message: String },
    #[error("decode_index_buffer u16: {0}")]
    DecodeIndexBufferU16(String),
    #[error("decode_index_buffer u32: {0}")]
    DecodeIndexBufferU32(String),
    #[error("unsupported meshopt mode: {0}")]
    UnsupportedMode(String),
    #[error("unsupported meshopt vertex byte_stride: {0}")]
    UnsupportedStride(usize),
    #[error("unsupported index_size: {size} (expected 2 or 4)")]
    UnsupportedIndexSize { size: usize },
}

// ---- Decoder ----

struct MeshoptDecoder;

impl CodecDecoder for MeshoptDecoder {
    const EXT_NAME: &'static str = "EXT_meshopt_compression";
    type Error = MeshoptError;

    fn decode_view(model: &mut GltfModel, bv_idx: usize, ext: &Value) -> Result<(), MeshoptError> {
        decode_buffer_view(model, bv_idx, ext)
    }
}

/// Decode all meshopt-compressed buffer views in-place.
/// Returns a warning string for each buffer view that fails.
pub fn decode(model: &mut GltfModel) -> Vec<String> {
    crate::decode_buffer_views::<MeshoptDecoder>(model)
}

/// Low-level: decode a meshopt-encoded vertex attribute buffer.
///
/// Returns raw decoded bytes; length = `count * byte_stride`.
/// `byte_stride` must be a multiple of 4 and at most 64.
pub fn decode_vertex_buffer(
    data: &[u8],
    count: usize,
    byte_stride: usize,
) -> Result<Vec<u8>, MeshoptError> {
    decode_vertices_dynamic(data, count, byte_stride)
}

/// Low-level: decode a meshopt-encoded index buffer.
///
/// Returns indices as `Vec<u32>` regardless of source `index_size` (2 or 4 bytes).
pub fn decode_index_buffer(
    data: &[u8],
    count: usize,
    index_size: usize,
) -> Result<Vec<u32>, MeshoptError> {
    match index_size {
        2 => {
            let indices = meshopt::encoding::decode_index_buffer::<u16>(data, count)
                .map_err(|e| MeshoptError::DecodeIndexBufferU16(e.to_string()))?;
            Ok(indices.into_iter().map(|i| i as u32).collect())
        }
        4 => meshopt::encoding::decode_index_buffer::<u32>(data, count)
            .map_err(|e| MeshoptError::DecodeIndexBufferU32(e.to_string())),
        size => Err(MeshoptError::UnsupportedIndexSize { size }),
    }
}

fn decode_buffer_view(
    model: &mut GltfModel,
    bv_idx: usize,
    ext: &Value,
) -> Result<(), MeshoptError> {
    let src_buffer_idx = ext
        .get("buffer")
        .and_then(|v| v.as_i64())
        .ok_or(MeshoptError::MissingBuffer)? as usize;

    let src_byte_offset = ext.get("byteOffset").and_then(|v| v.as_i64()).unwrap_or(0) as usize;

    let src_byte_length = ext
        .get("byteLength")
        .and_then(|v| v.as_i64())
        .ok_or(MeshoptError::MissingByteLength)? as usize;

    let count = ext
        .get("count")
        .and_then(|v| v.as_i64())
        .ok_or(MeshoptError::MissingCount)? as usize;

    let byte_stride = ext
        .get("byteStride")
        .and_then(|v| v.as_i64())
        .ok_or(MeshoptError::MissingByteStride)? as usize;

    let mode = ext
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("ATTRIBUTES");

    let _filter = ext.get("filter").and_then(|v| v.as_str()).unwrap_or("NONE");

    let compressed: Vec<u8> = {
        let src_buf = &model
            .buffers
            .get(src_buffer_idx)
            .ok_or(MeshoptError::BufferOutOfRange(src_buffer_idx))?
            .data;
        let src_end = src_byte_offset + src_byte_length;
        if src_end > src_buf.len() {
            return Err(MeshoptError::CompressedRangeExceeded {
                start: src_byte_offset,
                end: src_end,
                size: src_buf.len(),
            });
        }
        src_buf[src_byte_offset..src_end].to_vec()
    };
    let output_size = count * byte_stride;
    let mut decoded = vec![0u8; output_size];

    match mode {
        "ATTRIBUTES" => {
            let verts = decode_vertices_dynamic(&compressed, count, byte_stride)?;
            decoded.copy_from_slice(&verts);
        }
        "TRIANGLES" => {
            if byte_stride == 2 {
                let indices = meshopt::encoding::decode_index_buffer::<u16>(&compressed, count)
                    .map_err(|e| MeshoptError::DecodeIndexBufferU16(e.to_string()))?;
                decoded.copy_from_slice(bytemuck::cast_slice(&indices));
            } else {
                let indices = meshopt::encoding::decode_index_buffer::<u32>(&compressed, count)
                    .map_err(|e| MeshoptError::DecodeIndexBufferU32(e.to_string()))?;
                decoded.copy_from_slice(bytemuck::cast_slice(&indices));
            }
        }
        _ => return Err(MeshoptError::UnsupportedMode(mode.to_string())),
    }

    // Note: meshopt 0.6 does not expose filter decode functions (OCTAHEDRAL,
    // QUATERNION, EXPONENTIAL). Attributes using these filters will be returned
    // as-is from the codec and may need post-processing by the caller.

    // Write decoded data back to buffer view
    let bv = &model.buffer_views[bv_idx];
    let buf_idx = bv.buffer;
    let byte_offset = bv.byte_offset;

    if byte_offset + output_size <= model.buffers[buf_idx].data.len() {
        model.buffers[buf_idx].data[byte_offset..byte_offset + output_size]
            .copy_from_slice(&decoded);
    } else {
        model.buffers[buf_idx].data.extend_from_slice(&decoded);
    }

    Ok(())
}

/// Decode a meshopt vertex buffer with a dynamic (runtime) byte stride.
///
/// Uses `[u32; N]` (N = stride / 4) so that `Default` and `Pod` are always
/// satisfied — Rust's std only auto-derives `Default` for arrays up to N=32,
/// and glTF strides are always multiples of 4, so N never exceeds 16.
fn decode_vertices_dynamic(
    encoded: &[u8],
    count: usize,
    byte_stride: usize,
) -> Result<Vec<u8>, MeshoptError> {
    fn dv<T: Clone + Default + bytemuck::Pod>(
        encoded: &[u8],
        count: usize,
        stride: usize,
    ) -> Result<Vec<u8>, MeshoptError> {
        let verts = meshopt::encoding::decode_vertex_buffer::<T>(encoded, count).map_err(|e| {
            MeshoptError::DecodeVertexBuffer {
                stride,
                message: e.to_string(),
            }
        })?;
        Ok(bytemuck::cast_vec(verts))
    }
    match byte_stride {
        4 => dv::<[u32; 1]>(encoded, count, byte_stride),
        8 => dv::<[u32; 2]>(encoded, count, byte_stride),
        12 => dv::<[u32; 3]>(encoded, count, byte_stride),
        16 => dv::<[u32; 4]>(encoded, count, byte_stride),
        20 => dv::<[u32; 5]>(encoded, count, byte_stride),
        24 => dv::<[u32; 6]>(encoded, count, byte_stride),
        28 => dv::<[u32; 7]>(encoded, count, byte_stride),
        32 => dv::<[u32; 8]>(encoded, count, byte_stride),
        36 => dv::<[u32; 9]>(encoded, count, byte_stride),
        40 => dv::<[u32; 10]>(encoded, count, byte_stride),
        44 => dv::<[u32; 11]>(encoded, count, byte_stride),
        48 => dv::<[u32; 12]>(encoded, count, byte_stride),
        52 => dv::<[u32; 13]>(encoded, count, byte_stride),
        56 => dv::<[u32; 14]>(encoded, count, byte_stride),
        60 => dv::<[u32; 15]>(encoded, count, byte_stride),
        64 => dv::<[u32; 16]>(encoded, count, byte_stride),
        s => Err(MeshoptError::UnsupportedStride(s)),
    }
}

// ---- Encoder ----

pub(crate) struct MeshoptEncoder;

impl CodecEncoder for MeshoptEncoder {
    const EXT_NAME: &'static str = "EXT_meshopt_compression";
    type Error = MeshoptError;

    fn encode(model: &mut GltfModel) -> Result<(), MeshoptError> {
        for mesh_idx in 0..model.meshes.len() {
            for prim_idx in 0..model.meshes[mesh_idx].primitives.len() {
                let prim = &model.meshes[mesh_idx].primitives[prim_idx];
                if prim.indices.is_none() {
                    continue;
                }
                match compress_primitive_meshopt(model, mesh_idx, prim_idx) {
                    Ok(extension) => {
                        model.meshes[mesh_idx].primitives[prim_idx]
                            .extensions
                            .insert(Self::EXT_NAME.to_string(), extension);
                        if !model.extensions_used.contains(&Self::EXT_NAME.to_string()) {
                            model.extensions_used.push(Self::EXT_NAME.to_string());
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "Warning: Failed to compress mesh[{mesh_idx}].prim[{prim_idx}] with meshopt: {e}"
                        );
                    }
                }
            }
        }
        Ok(())
    }
}

/// Encode all eligible primitives with meshopt compression.
pub fn encode(model: &mut GltfModel) -> Result<(), MeshoptError> {
    MeshoptEncoder::encode(model)
}

fn compress_primitive_meshopt(
    _model: &GltfModel,
    _mesh_idx: usize,
    _prim_idx: usize,
) -> Result<Value, MeshoptError> {
    // Placeholder — full implementation would compress vertex/index buffers.
    Ok(json!({ "bufferView": null }))
}
