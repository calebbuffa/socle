//! `KHR_mesh_quantization` dequantization.
//!
//! Converts quantized integer attributes (BYTE, SHORT, etc.) to FLOAT,
//! with proper normalization when `accessor.normalized == true`.

use moderu::{Accessor, AccessorComponentType, BufferView, ComponentType, GltfModel};

const EXT_NAME: &str = "KHR_mesh_quantization";

/// Dequantize all mesh attributes that use non-float component types.
pub fn dequantize(model: &mut GltfModel, warnings: &mut super::error::Warnings) {
    if !model.extensions_used.iter().any(|e| e == EXT_NAME) {
        return;
    }

    // Collect (mesh_idx, prim_idx, attr_name, accessor_idx) for all
    // quantized attributes.
    let mut work = Vec::new();

    for (mesh_idx, mesh) in model.meshes.iter().enumerate() {
        for (prim_idx, prim) in mesh.primitives.iter().enumerate() {
            for (attr_name, &acc_idx) in &prim.attributes {
                if !is_quantizable_attribute(attr_name) {
                    continue;
                }
                if acc_idx >= model.accessors.len() {
                    continue;
                }
                let acc = &model.accessors[acc_idx];
                let ct = acc.component_type();
                // Already float — nothing to do.
                if ct == ComponentType::Float {
                    continue;
                }
                work.push((mesh_idx, prim_idx, attr_name.clone(), acc_idx));
            }
        }
    }

    for (mesh_idx, prim_idx, attr_name, acc_idx) in work {
        match dequantize_accessor(model, acc_idx) {
            Ok(new_acc_idx) => {
                model.meshes[mesh_idx].primitives[prim_idx]
                    .attributes
                    .insert(attr_name, new_acc_idx);
            }
            Err(e) => {
                warnings.push(super::error::Warning(format!(
                    "mesh[{mesh_idx}].primitive[{prim_idx}].{attr_name}: dequantize failed: {e}"
                )));
            }
        }
    }
}

fn is_quantizable_attribute(name: &str) -> bool {
    name == "POSITION" || name == "NORMAL" || name == "TANGENT" || name.starts_with("TEXCOORD_")
}

fn dequantize_accessor(model: &mut GltfModel, acc_idx: usize) -> Result<usize, String> {
    let ct = model.accessors[acc_idx].component_type();
    let at = model.accessors[acc_idx].accessor_type();
    let num_components = at.num_components() as usize;
    let count = model.accessors[acc_idx].count;
    let comp_size = ct.byte_size() as usize;
    let normalized = model.accessors[acc_idx].normalized;
    let bv_idx = model.accessors[acc_idx]
        .buffer_view
        .ok_or("accessor missing bufferView")?;
    let buf_idx = model
        .buffer_views
        .get(bv_idx)
        .ok_or("bufferView out of range")?
        .buffer;
    let stride = model.buffer_views[bv_idx]
        .byte_stride
        .unwrap_or(num_components * comp_size);
    let base_offset = model.buffer_views[bv_idx].byte_offset + model.accessors[acc_idx].byte_offset;

    // Read source data — limit borrow scope so we can push to model.buffers after.
    let (float_data, min_vals, max_vals) = {
        let buf = &model
            .buffers
            .get(buf_idx)
            .ok_or("buffer out of range")?
            .data;

        let mut float_data = Vec::with_capacity(count * num_components * 4);
        let mut min_vals = vec![f64::MAX; num_components];
        let mut max_vals = vec![f64::MIN; num_components];

        for i in 0..count {
            let elem_start = base_offset + i * stride;
            for j in 0..num_components {
                let comp_start = elem_start + j * comp_size;
                if comp_start + comp_size > buf.len() {
                    return Err("accessor data exceeds buffer".into());
                }
                let raw = &buf[comp_start..comp_start + comp_size];
                let float_val = read_and_convert(raw, ct, normalized);

                if float_val < min_vals[j] {
                    min_vals[j] = float_val;
                }
                if float_val > max_vals[j] {
                    max_vals[j] = float_val;
                }

                float_data.extend_from_slice(&(float_val as f32).to_le_bytes());
            }
        }
        (float_data, min_vals, max_vals)
    };

    // Create new buffer, bufferView, accessor.
    let new_buf_idx = model.buffers.len();
    let byte_len = float_data.len();
    model.buffers.push(moderu::Buffer {
        data: float_data,
        byte_length: byte_len,
        ..Default::default()
    });

    let new_bv_idx = model.buffer_views.len();
    model.buffer_views.push(BufferView {
        buffer: new_buf_idx,
        byte_offset: 0,
        byte_length: byte_len,
        byte_stride: Some((num_components) * 4),
        ..Default::default()
    });

    let new_acc_idx = model.accessors.len();
    model.accessors.push(Accessor {
        buffer_view: Some(new_bv_idx),
        byte_offset: 0,
        component_type: AccessorComponentType::Float,
        count,
        r#type: at,
        normalized: false,
        min: min_vals.iter().map(|&v| v).collect(),
        max: max_vals.iter().map(|&v| v).collect(),
        ..Default::default()
    });

    Ok(new_acc_idx)
}

fn read_and_convert(raw: &[u8], ct: ComponentType, normalized: bool) -> f64 {
    match ct {
        ComponentType::Byte => {
            let v = i8::from_le_bytes([raw[0]]);
            if normalized {
                (v as f64 / 127.0).max(-1.0)
            } else {
                v as f64
            }
        }
        ComponentType::UnsignedByte => {
            let v = raw[0];
            if normalized {
                v as f64 / 255.0
            } else {
                v as f64
            }
        }
        ComponentType::Short => {
            let v = i16::from_le_bytes([raw[0], raw[1]]);
            if normalized {
                (v as f64 / 32767.0).max(-1.0)
            } else {
                v as f64
            }
        }
        ComponentType::UnsignedShort => {
            let v = u16::from_le_bytes([raw[0], raw[1]]);
            if normalized {
                v as f64 / 65535.0
            } else {
                v as f64
            }
        }
        _ => {
            // Int, Float, etc. — just read as float.
            if raw.len() >= 4 {
                f32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) as f64
            } else {
                0.0
            }
        }
    }
}
