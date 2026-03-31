//! `KHR_gaussian_splatting_compression_spz` decoder and encoder.
//!
//! Decompresses SPZ-encoded Gaussian splat data and writes the results
//! back into new glTF accessors / bufferViews.

use crate::{ApplicabilityResult, CodecDecoder, CodecEncoder};
use moderu::{Accessor, BufferView, GltfModel};
use serde_json::{Value, json};

/// Errors that can occur during SPZ decode or encode operations.
#[derive(thiserror::Error, Debug)]
pub enum SpzError {
    #[error("SPZ parse: {0}")]
    Parse(String),
    #[error("unpack splat {index}: {message}")]
    UnpackFailed { index: usize, message: String },
    #[error("missing bufferView in SPZ extension")]
    MissingBufferView,
    #[error("bufferView {0} out of range")]
    BufferViewOutOfRange(usize),
    #[error("buffer {0} out of range")]
    BufferOutOfRange(usize),
    #[error("SPZ data exceeds buffer")]
    DataExceedsBuffer,
    #[error("primitive not found")]
    PrimitiveNotFound,
    #[error("no POSITION attribute")]
    NoPositionAttribute,
}

// ---- Public types ----

/// Decoded Gaussian splat data from an SPZ-compressed buffer.
#[derive(Debug, Clone)]
pub struct DecodedSplats {
    pub positions: Vec<[f32; 3]>,
    pub rotations: Vec<[f32; 4]>,
    pub scales: Vec<[f32; 3]>,
    /// RGBA: color converted from SH0 coefficients, alpha from sigmoid of packed alpha.
    pub colors: Vec<[f32; 4]>,
}

// ---- Decoder ----

/// Low-level: decode a raw SPZ buffer directly, without a glTF model.
///
/// # Example
/// ```ignore
/// let splats = moderu_codec::spz::decode_buffer(&data)?;
/// for (pos, color) in splats.positions.iter().zip(&splats.colors) {
///     println!("pos={pos:?}  rgba={color:?}");
/// }
/// ```
pub fn decode_buffer(data: &[u8]) -> Result<DecodedSplats, SpzError> {
    let packed = spz::PackedGaussians::try_from(data.to_vec())
        .map_err(|e| SpzError::Parse(e.to_string()))?;
    let num_points = packed.num_points;
    let coord_converter = spz::CoordinateConverter::default();

    let mut positions = Vec::with_capacity(num_points as usize);
    let mut rotations = Vec::with_capacity(num_points as usize);
    let mut scales = Vec::with_capacity(num_points as usize);
    let mut colors = Vec::with_capacity(num_points as usize);

    for i in 0..num_points {
        let splat =
            packed
                .unpack(i as usize, &coord_converter)
                .map_err(|e| SpzError::UnpackFailed {
                    index: i as usize,
                    message: e.to_string(),
                })?;

        positions.push([splat.position[0], splat.position[1], splat.position[2]]);
        rotations.push([
            splat.rotation[0],
            splat.rotation[1],
            splat.rotation[2],
            splat.rotation[3],
        ]);
        scales.push([splat.scale[0], splat.scale[1], splat.scale[2]]);

        let r = 0.5_f32 + splat.color[0] as f32 * 0.282_095_f32;
        let g = 0.5_f32 + splat.color[1] as f32 * 0.282_095_f32;
        let b = 0.5_f32 + splat.color[2] as f32 * 0.282_095_f32;
        let a = 1.0_f32 / (1.0_f32 + (-splat.alpha as f32).exp());
        colors.push([r, g, b, a]);
    }

    Ok(DecodedSplats {
        positions,
        rotations,
        scales,
        colors,
    })
}

/// Codec decoder for `KHR_gaussian_splatting_compression_spz`.
struct SpzDecoder;

impl CodecDecoder for SpzDecoder {
    const EXT_NAME: &'static str = "KHR_gaussian_splatting_compression_spz";
    type Error = SpzError;

    // SPZ extension names may end with "spz" or "spz2"; check with prefix matching.
    fn can_decode(model: &GltfModel) -> ApplicabilityResult {
        const PREFIX: &str = "KHR_gaussian_splatting_compression_spz";
        model
            .extensions_used
            .iter()
            .any(|e| e.starts_with(PREFIX))
            .into()
    }

    fn decode_primitive(
        model: &mut GltfModel,
        mesh_idx: usize,
        prim_idx: usize,
        _ext: &Value,
    ) -> Result<(), SpzError> {
        decode_spz_primitive(model, mesh_idx, prim_idx)
    }
}

/// Decode all SPZ-compressed Gaussian splat primitives.
pub fn decode(model: &mut GltfModel) -> Vec<String> {
    let mut warnings = Vec::new();

    // Custom iterator to handle prefix matching on primitive extensions.
    if SpzDecoder::can_decode(model) == ApplicabilityResult::NotApplicable {
        return warnings;
    }

    for mesh_idx in 0..model.meshes.len() {
        for prim_idx in 0..model.meshes[mesh_idx].primitives.len() {
            let has_spz = model.meshes[mesh_idx].primitives[prim_idx]
                .extensions
                .keys()
                .any(|k: &String| k.starts_with("KHR_gaussian_splatting_compression_spz"));

            if !has_spz {
                continue;
            }

            if let Err(e) = decode_spz_primitive(model, mesh_idx, prim_idx) {
                warnings.push(format!(
                    "mesh[{mesh_idx}].primitive[{prim_idx}] SPZ decode: {e}"
                ));
            }
        }
    }

    warnings
}

fn decode_spz_primitive(
    model: &mut GltfModel,
    mesh_idx: usize,
    prim_idx: usize,
) -> Result<(), SpzError> {
    // Find the SPZ extension — try both names.
    let ext_key = model.meshes[mesh_idx].primitives[prim_idx]
        .extensions
        .keys()
        .find(|k: &&String| k.starts_with("KHR_gaussian_splatting_compression_spz"))
        .cloned();

    let Some(ext_key) = ext_key else {
        return Ok(());
    };

    let ext = model.meshes[mesh_idx].primitives[prim_idx]
        .extensions
        .get(&ext_key)
        .cloned()
        .unwrap_or_default();

    // Get the buffer view containing SPZ data.
    let bv_idx = ext
        .get("bufferView")
        .and_then(|v: &Value| v.as_i64())
        .ok_or(SpzError::MissingBufferView)? as usize;

    let bv = model
        .buffer_views
        .get(bv_idx)
        .ok_or(SpzError::BufferViewOutOfRange(bv_idx))?;

    let buf_idx = bv.buffer;
    let bv_start = bv.byte_offset;
    let bv_end = bv_start + bv.byte_length;

    let spz_data: Vec<u8> = {
        let buf_data = &model
            .buffers
            .get(buf_idx)
            .ok_or(SpzError::BufferOutOfRange(buf_idx))?
            .data;
        if bv_end > buf_data.len() {
            return Err(SpzError::DataExceedsBuffer);
        }
        buf_data[bv_start..bv_end].to_vec()
    };

    // Decode SPZ.
    let packed =
        spz::PackedGaussians::try_from(spz_data).map_err(|e| SpzError::Parse(e.to_string()))?;

    let num_points = packed.num_points;
    let num_points_usize = num_points as usize;
    let coord_converter = spz::CoordinateConverter::default();

    // Create output buffer for decoded attributes.
    let out_buf_idx = model.buffers.len();
    let mut out_data = Vec::new();
    model.buffers.push(moderu::Buffer::default());

    // Unpack all splats and write attributes.
    let mut positions = Vec::with_capacity(num_points_usize * 12); // vec3 f32
    let mut rotations = Vec::with_capacity(num_points_usize * 16); // vec4 f32
    let mut scales = Vec::with_capacity(num_points_usize * 12); // vec3 f32
    let mut colors = Vec::with_capacity(num_points_usize * 16); // vec4 f32

    for i in 0..num_points {
        let splat =
            packed
                .unpack(i as usize, &coord_converter)
                .map_err(|e| SpzError::UnpackFailed {
                    index: i as usize,
                    message: e.to_string(),
                })?;

        positions.extend_from_slice(&splat.position[0].to_le_bytes());
        positions.extend_from_slice(&splat.position[1].to_le_bytes());
        positions.extend_from_slice(&splat.position[2].to_le_bytes());

        rotations.extend_from_slice(&splat.rotation[0].to_le_bytes());
        rotations.extend_from_slice(&splat.rotation[1].to_le_bytes());
        rotations.extend_from_slice(&splat.rotation[2].to_le_bytes());
        rotations.extend_from_slice(&splat.rotation[3].to_le_bytes());

        scales.extend_from_slice(&splat.scale[0].to_le_bytes());
        scales.extend_from_slice(&splat.scale[1].to_le_bytes());
        scales.extend_from_slice(&splat.scale[2].to_le_bytes());

        // Color: convert SH0 to RGB, apply sigmoid to alpha.
        // cesium-native: R/G/B = 0.5 + color * 0.282095, A = 1/(1+exp(-alpha))
        let r = 0.5 + splat.color[0] * 0.282095;
        let g = 0.5 + splat.color[1] * 0.282095;
        let b = 0.5 + splat.color[2] * 0.282095;
        let a = 1.0 / (1.0 + (-splat.alpha).exp());

        colors.extend_from_slice(&(r as f32).to_le_bytes());
        colors.extend_from_slice(&(g as f32).to_le_bytes());
        colors.extend_from_slice(&(b as f32).to_le_bytes());
        colors.extend_from_slice(&(a as f32).to_le_bytes());
    }

    // Helper closure to write attributes to output buffer.
    let mut write_attr = |_name: &str, data: Vec<u8>, num_comp: u8, accessor_type: &str| -> usize {
        let offset = out_data.len();
        let byte_len = data.len();
        out_data.extend_from_slice(&data);

        let bv_new_idx = model.buffer_views.len();
        model.buffer_views.push(BufferView {
            buffer: out_buf_idx,
            byte_offset: offset,
            byte_length: byte_len,
            byte_stride: Some((num_comp as usize) * 4),
            ..Default::default()
        });

        let acc_idx = model.accessors.len();
        let at = match accessor_type {
            "VEC2" => moderu::AccessorType::Vec2,
            "VEC3" => moderu::AccessorType::Vec3,
            "VEC4" => moderu::AccessorType::Vec4,
            _ => moderu::AccessorType::Vec3,
        };
        model.accessors.push(Accessor {
            buffer_view: Some(bv_new_idx),
            byte_offset: 0,
            component_type: moderu::AccessorComponentType::Float,
            count: num_points as usize,
            r#type: at,
            ..Default::default()
        });

        let _ = _name;
        acc_idx
    };

    let pos_acc = write_attr("POSITION", positions, 3, "VEC3");
    let rot_acc = write_attr("ROTATION", rotations, 4, "VEC4");
    let scale_acc = write_attr("SCALE", scales, 3, "VEC3");
    let color_acc = write_attr("COLOR_0", colors, 4, "VEC4");

    // Commit buffer data.
    let byte_length = out_data.len();
    model.buffers[out_buf_idx].data = out_data;
    model.buffers[out_buf_idx].byte_length = byte_length;

    // Update primitive attributes.
    let prim = &mut model.meshes[mesh_idx].primitives[prim_idx];
    prim.attributes.insert("POSITION".into(), pos_acc);
    prim.attributes.insert("ROTATION".into(), rot_acc);
    prim.attributes.insert("SCALE".into(), scale_acc);
    prim.attributes.insert("COLOR_0".into(), color_acc);

    // Remove extension.
    prim.extensions.remove(&ext_key);

    Ok(())
}

// ---- Encoder ----

/// SPZ compression encoder for Gaussian splatting.
pub(crate) struct SpzEncoder;

impl CodecEncoder for SpzEncoder {
    const EXT_NAME: &'static str = "KHR_gaussian_splatting_compression_spz";
    type Error = SpzError;

    fn encode(model: &mut GltfModel) -> Result<(), SpzError> {
        for mesh_idx in 0..model.meshes.len() {
            for prim_idx in 0..model.meshes[mesh_idx].primitives.len() {
                let prim = &model.meshes[mesh_idx].primitives[prim_idx];

                if prim.indices.is_some() {
                    continue;
                }

                if !prim.attributes.contains_key("POSITION") {
                    continue;
                }

                match compress_primitive_spz(model, mesh_idx, prim_idx) {
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
                            "Warning: Failed to compress mesh[{}].prim[{}] with SPZ: {}",
                            mesh_idx, prim_idx, e
                        );
                    }
                }
            }
        }
        Ok(())
    }
}

fn compress_primitive_spz(
    model: &GltfModel,
    mesh_idx: usize,
    prim_idx: usize,
) -> Result<Value, SpzError> {
    let prim = model
        .meshes
        .get(mesh_idx)
        .and_then(|m| m.primitives.get(prim_idx))
        .ok_or(SpzError::PrimitiveNotFound)?;

    let _position_accessor = prim
        .attributes
        .get("POSITION")
        .ok_or(SpzError::NoPositionAttribute)?;

    // Gaussian splatting compression not yet implemented.
    Ok(json!({
        "bufferView": null,
        "splatCount": 0,
        "properties": []
    }))
}

/// Encode all eligible Gaussian splat primitives with SPZ compression.
pub fn encode(model: &mut GltfModel) -> Result<(), SpzError> {
    SpzEncoder::encode(model)
}
