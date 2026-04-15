//! Raster overlay application to glTF models.
//!
//! This mirrors cesium-native's `RasterOverlayUtilities` — the framework
//! computes overlay texture coordinates from existing TEXCOORD_0 using the
//! provided translation and scale, adds the overlay image as a texture,
//! and assigns it as the base-color texture on every material.

use image::ImageEncoder;

use crate::overlay::RasterOverlayTile;

/// Apply a raster overlay tile to a [`GltfModel`](moderu::GltfModel).
///
/// Like cesium-native, this generates **new texture coordinates** for the
/// overlay rather than relying on `KHR_texture_transform`. For each primitive's
/// TEXCOORD_0, it computes:
///
///   `overlay_uv = translation + base_uv * scale`
///
/// and writes the result as a new TEXCOORD attribute. The overlay texture
/// references this new texcoord set directly.
///
/// `translation` and `scale` are the UV transform values computed by the
/// framework and passed to [`OverlayTarget::attach_raster`].
pub fn apply_raster_overlay(
    model: &mut moderu::GltfModel,
    tile: &RasterOverlayTile,
    translation: [f64; 2],
    scale: [f64; 2],
) -> bool {
    let [tx, ty] = translation;
    let [sx, sy] = scale;

    // 1. For each primitive, read TEXCOORD_0, compute overlay UVs, write as new accessor.
    //    Collect (primitive mesh_idx, prim_idx, new_accessor_idx) to assign later.
    let overlay_texcoord_set = find_next_texcoord(model);
    let mut prim_accessors: Vec<(usize, usize, usize)> = Vec::new();

    for mesh_idx in 0..model.meshes.len() {
        for prim_idx in 0..model.meshes[mesh_idx].primitives.len() {
            let tc0_acc = model.meshes[mesh_idx].primitives[prim_idx]
                .attributes
                .get("TEXCOORD_0")
                .copied();
            let Some(tc0_acc) = tc0_acc else { continue };

            // Read existing UVs.
            let base_uvs: Vec<[f32; 2]> = match moderu::resolve_accessor::<[f32; 2]>(model, tc0_acc)
            {
                Ok(view) => view.iter().collect(),
                Err(_) => continue,
            };

            // Compute overlay UVs: overlay_uv = translation + base_uv * scale
            let overlay_uvs: Vec<[f32; 2]> = base_uvs
                .iter()
                .map(|uv| {
                    [
                        (tx + uv[0] as f64 * sx) as f32,
                        (ty + uv[1] as f64 * sy) as f32,
                    ]
                })
                .collect();

            // Write as a new accessor.
            let acc_idx = model.append_accessor(&overlay_uvs);
            prim_accessors.push((mesh_idx, prim_idx, acc_idx));
        }
    }

    // Assign the new texcoord attribute to each primitive.
    let attr_name = format!("TEXCOORD_{overlay_texcoord_set}");
    for (mesh_idx, prim_idx, acc_idx) in &prim_accessors {
        model.meshes[*mesh_idx].primitives[*prim_idx]
            .attributes
            .insert(attr_name.clone(), *acc_idx);
    }

    // 2. Encode RGBA pixels to PNG (fast: no compression, no filtering).
    let png_bytes = {
        let mut buf = std::io::Cursor::new(Vec::with_capacity(
            tile.pixels.len() + 1024, // raw pixels + PNG overhead
        ));
        let encoder = image::codecs::png::PngEncoder::new_with_quality(
            &mut buf,
            image::codecs::png::CompressionType::Fast,
            image::codecs::png::FilterType::NoFilter,
        );
        let ok = encoder.write_image(
            &tile.pixels,
            tile.width,
            tile.height,
            image::ColorType::Rgba8.into(),
        );
        if ok.is_err() {
            return false;
        }
        buf.into_inner()
    };

    // 3. Append PNG to buffer 0.
    if model.buffers.is_empty() {
        model.buffers.push(moderu::Buffer::default());
    }
    let buf = &mut model.buffers[0];
    while buf.data.len() % 4 != 0 {
        buf.data.push(0);
    }
    let byte_offset = buf.data.len();
    buf.data.extend_from_slice(&png_bytes);
    buf.byte_length = buf.data.len();

    // 4. Buffer view for the image.
    let bv_index = model.buffer_views.len();
    model.buffer_views.push(moderu::BufferView {
        buffer: 0,
        byte_offset,
        byte_length: png_bytes.len(),
        ..Default::default()
    });

    // 5. Image.
    let img_index = model.images.len();
    model.images.push(moderu::Image {
        buffer_view: Some(bv_index),
        mime_type: Some("image/png".into()),
        name: Some("overlay".into()),
        ..Default::default()
    });

    // 6. Sampler (linear, clamp-to-edge).
    let sampler_index = model.samplers.len();
    model.samplers.push(moderu::Sampler {
        mag_filter: Some(9729),
        min_filter: Some(9729),
        wrap_s: 33071,
        wrap_t: 33071,
        ..Default::default()
    });

    // 7. Texture.
    let tex_index = model.textures.len();
    model.textures.push(moderu::Texture {
        source: Some(img_index),
        sampler: Some(sampler_index),
        ..Default::default()
    });

    // 8. TextureInfo referencing the overlay texcoord set directly — no extensions needed.
    let tex_info = moderu::TextureInfo {
        index: tex_index,
        tex_coord: overlay_texcoord_set,
        ..Default::default()
    };

    // 9. Apply to all materials — reset base_color_factor to white so the
    //    overlay texture isn't tinted by the original material colour.
    for mat in &mut model.materials {
        let pbr = mat
            .pbr_metallic_roughness
            .get_or_insert_with(Default::default);
        pbr.base_color_texture = Some(tex_info.clone());
        pbr.base_color_factor = vec![1.0, 1.0, 1.0, 1.0];
    }

    // If no materials exist, create one and assign to unassigned primitives.
    if model.materials.is_empty() {
        model.materials.push(moderu::Material {
            pbr_metallic_roughness: Some(moderu::MaterialPbrMetallicRoughness {
                base_color_texture: Some(tex_info),
                ..Default::default()
            }),
            ..Default::default()
        });
        for mesh in &mut model.meshes {
            for prim in &mut mesh.primitives {
                if prim.material.is_none() {
                    prim.material = Some(0);
                }
            }
        }
    }

    true
}

/// Find the next available TEXCOORD_N index across all primitives.
fn find_next_texcoord(model: &moderu::GltfModel) -> usize {
    let mut max_tc = 0usize;
    for mesh in &model.meshes {
        for prim in &mesh.primitives {
            for key in prim.attributes.keys() {
                if let Some(n) = key.strip_prefix("TEXCOORD_") {
                    if let Ok(n) = n.parse::<usize>() {
                        max_tc = max_tc.max(n + 1);
                    }
                }
            }
        }
    }
    max_tc
}
