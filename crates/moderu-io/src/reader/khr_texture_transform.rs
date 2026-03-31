//! `KHR_texture_transform` — apply UV offset / rotation / scale.
//!
//! For each material texture that has this extension, rewrites the UV
//! coordinates of every primitive using that texture.

use moderu::{GltfModel, KhrTextureTransform};

const EXT_NAME: &str = "KHR_texture_transform";

/// Apply texture transforms to UV coordinates.
pub fn apply_texture_transforms(model: &mut GltfModel, warnings: &mut super::error::Warnings) {
    if !model.extensions_used.iter().any(|e| e == EXT_NAME) {
        return;
    }

    // Collect all (material_idx, texcoord_idx, transform) tuples.
    let transforms = collect_transforms(model);

    if transforms.is_empty() {
        return;
    }

    // For each primitive, check if it uses a material with a texture transform.
    for mesh_idx in 0..model.meshes.len() {
        for prim_idx in 0..model.meshes[mesh_idx].primitives.len() {
            let mat_idx = {
                let prim = &model.meshes[mesh_idx].primitives[prim_idx];
                prim.material
            };

            let Some(mat_idx) = mat_idx else {
                continue;
            };

            for (mid, texcoord, tx) in &transforms {
                if *mid != mat_idx as usize {
                    continue;
                }

                let attr_name = format!("TEXCOORD_{texcoord}");
                let acc_idx = {
                    let prim = &model.meshes[mesh_idx].primitives[prim_idx];
                    prim.attributes.get(&attr_name).copied()
                };

                let Some(acc_idx) = acc_idx else {
                    continue;
                };

                match transform_uvs(model, acc_idx, tx) {
                    Ok(new_acc_idx) => {
                        model.meshes[mesh_idx].primitives[prim_idx]
                            .attributes
                            .insert(attr_name, new_acc_idx);
                    }
                    Err(e) => {
                        warnings.push(super::error::Warning(format!(
                            "mesh[{mesh_idx}].prim[{prim_idx}].{attr_name}: tex transform: {e}"
                        )));
                    }
                }
            }
        }
    }
}

fn collect_transforms(model: &GltfModel) -> Vec<(usize, u32, KhrTextureTransform)> {
    let mut result = Vec::new();

    for (mat_idx, mat) in model.materials.iter().enumerate() {
        // Check each texture info slot for the extension.
        let texture_infos = collect_texture_infos(mat);

        for (texcoord, ext_val) in texture_infos {
            let tx = KhrTextureTransform::from_json(&ext_val);
            result.push((mat_idx, texcoord, tx));
        }
    }

    result
}

/// Extract all texture info extensions from a material.
fn collect_texture_infos(mat: &moderu::Material) -> Vec<(u32, serde_json::Value)> {
    let mut result = Vec::new();

    // Helper to check a TextureInfo-like value.
    let mut check = |info: &serde_json::Value| {
        if let Some(ext) = info.get("extensions").and_then(|e| e.get(EXT_NAME)) {
            let tc = ext
                .get("texCoord")
                .and_then(|v| v.as_u64())
                .unwrap_or_else(|| info.get("texCoord").and_then(|v| v.as_u64()).unwrap_or(0))
                as u32;
            result.push((tc, ext.clone()));
        }
    };

    // PBR metallic-roughness textures.
    if let Some(pbr) = &mat.pbr_metallic_roughness {
        if let Some(ref bt) = pbr.base_color_texture {
            check(&serde_json::to_value(bt).unwrap_or_default());
        }
        if let Some(ref mrt) = pbr.metallic_roughness_texture {
            check(&serde_json::to_value(mrt).unwrap_or_default());
        }
    }

    // Top-level material textures.
    if let Some(ref nt) = mat.normal_texture {
        check(&serde_json::to_value(nt).unwrap_or_default());
    }
    if let Some(ref ot) = mat.occlusion_texture {
        check(&serde_json::to_value(ot).unwrap_or_default());
    }
    if let Some(ref et) = mat.emissive_texture {
        check(&serde_json::to_value(et).unwrap_or_default());
    }

    result
}

fn transform_uvs(
    model: &mut GltfModel,
    acc_idx: usize,
    tx: &KhrTextureTransform,
) -> Result<usize, String> {
    let bv_idx = model
        .accessors
        .get(acc_idx)
        .ok_or("accessor out of range")?
        .buffer_view
        .ok_or("no bufferView")?;
    let buf_idx = model
        .buffer_views
        .get(bv_idx)
        .ok_or("bufferView out of range")?
        .buffer;
    let count = model.accessors[acc_idx].count;
    let comp_size = model.accessors[acc_idx].component_byte_size() as usize;
    let num_comp = model.accessors[acc_idx].num_components() as usize;
    let stride = model.buffer_views[bv_idx]
        .byte_stride
        .unwrap_or(num_comp * comp_size);
    let base = model.buffer_views[bv_idx].byte_offset + model.accessors[acc_idx].byte_offset;

    // Read UV data — limit borrow scope so we can push to model.buffers after.
    let out: Vec<u8> = {
        let buf = &model
            .buffers
            .get(buf_idx)
            .ok_or("buffer out of range")?
            .data;
        let mut out = Vec::with_capacity(count * 8); // VEC2 float

        for i in 0..count {
            let off = base + i * stride;
            let u: f64;
            let v: f64;

            if comp_size == 4 {
                // Float
                u = f32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]]) as f64;
                v = f32::from_le_bytes([buf[off + 4], buf[off + 5], buf[off + 6], buf[off + 7]])
                    as f64;
            } else {
                // Other types — read raw value.
                u = buf[off] as f64;
                v = buf[off + comp_size] as f64;
            }

            // Delegate to KhrTextureTransform::apply — single source of truth.
            let [fu, fv] = tx.apply(u, v);

            out.extend_from_slice(&(fu as f32).to_le_bytes());
            out.extend_from_slice(&(fv as f32).to_le_bytes());
        }
        out
    };

    // Create new buffer, bufferView, accessor.
    let new_buf_idx = model.buffers.len();
    let byte_len = out.len();
    model.buffers.push(moderu::Buffer {
        data: out,
        byte_length: byte_len,
        ..Default::default()
    });

    let new_bv_idx = model.buffer_views.len();
    model.buffer_views.push(moderu::BufferView {
        buffer: new_buf_idx,
        byte_offset: 0,
        byte_length: byte_len,
        byte_stride: Some(8),
        ..Default::default()
    });

    let new_acc_idx = model.accessors.len();
    model.accessors.push(moderu::Accessor {
        buffer_view: Some(new_bv_idx),
        byte_offset: 0,
        component_type: moderu::AccessorComponentType::Float,
        count: count as usize,
        r#type: moderu::AccessorType::Vec2,
        ..Default::default()
    });

    Ok(new_acc_idx)
}
