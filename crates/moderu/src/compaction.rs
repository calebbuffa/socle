//! Buffer and asset compaction: collapse, move, compact, remove-unused helpers.

use crate::GltfModel;

/// Shrink all buffers by removing unreferenced byte ranges.
pub(crate) fn compact_buffers(model: &mut GltfModel) {
    for i in 0..model.buffers.len() {
        compact_buffer(model, i);
    }
}

/// Shrink a single buffer by removing unreferenced byte ranges.
pub(crate) fn compact_buffer(model: &mut GltfModel, buffer_index: usize) {
    let buf_len = match model.buffers.get(buffer_index) {
        Some(b) => b.data.len(),
        None => return,
    };
    if buf_len == 0 {
        return;
    }
    let mut ranges: Vec<(usize, usize)> = model
        .buffer_views
        .iter()
        .filter(|bv| bv.buffer == buffer_index)
        .map(|bv| (bv.byte_offset, bv.byte_offset + bv.byte_length))
        .collect();
    if ranges.is_empty() {
        return;
    }
    ranges.sort_unstable();
    let data = &model.buffers[buffer_index].data;
    let mut new_data: Vec<u8> = Vec::with_capacity(data.len());
    let mut offset_map: Vec<(usize, usize)> = Vec::new();
    let mut prev_end = 0usize;
    for (start, end) in &ranges {
        let start = (*start).max(prev_end);
        if start >= *end {
            continue;
        }
        offset_map.push((start, new_data.len()));
        new_data.extend_from_slice(&data[start..(*end).min(data.len())]);
        prev_end = *end;
    }
    let remap = |old: usize| -> usize {
        offset_map
            .iter()
            .rev()
            .find(|&&(os, _)| old >= os)
            .map(|&(os, ns)| ns + (old - os))
            .unwrap_or(old)
    };
    for bv in &mut model.buffer_views {
        if bv.buffer == buffer_index {
            bv.byte_offset = remap(bv.byte_offset);
        }
    }
    if let Some(b) = model.buffers.get_mut(buffer_index) {
        b.byte_length = new_data.len();
        b.data = new_data;
    }
}

// ── Remove-unused helpers ─────────────────────────────────────────────────────

/// Build a remap table from old indices to new indices in O(n) time.
/// `used` must be sorted and deduplicated.
fn build_remap(item_count: usize, used: &[usize]) -> Vec<Option<usize>> {
    let mut remap = vec![None; item_count];
    for (new_idx, &old_idx) in used.iter().enumerate() {
        if let Some(slot) = remap.get_mut(old_idx) {
            *slot = Some(new_idx);
        }
    }
    remap
}

pub(crate) fn remove_unused_textures(model: &mut GltfModel, extra_used: &[usize]) {
    let mut used: Vec<usize> = model
        .materials
        .iter()
        .flat_map(|m| {
            let mut v = vec![];
            if let Some(pbr) = &m.pbr_metallic_roughness {
                if let Some(t) = &pbr.base_color_texture {
                    v.push(t.index);
                }
                if let Some(t) = &pbr.metallic_roughness_texture {
                    v.push(t.index);
                }
            }
            if let Some(t) = &m.normal_texture {
                v.push(t.index);
            }
            if let Some(t) = &m.occlusion_texture {
                v.push(t.index);
            }
            if let Some(t) = &m.emissive_texture {
                v.push(t.index);
            }
            v
        })
        .collect();
    used.extend_from_slice(extra_used);
    used.sort_unstable();
    used.dedup();
    let remap = build_remap(model.textures.len(), &used);
    model.textures = used
        .iter()
        .filter_map(|&i| model.textures.get(i).cloned())
        .collect();
    let ri = |i: usize| remap.get(i).and_then(|o| *o).unwrap_or(i);
    for m in &mut model.materials {
        if let Some(pbr) = &mut m.pbr_metallic_roughness {
            if let Some(t) = &mut pbr.base_color_texture {
                t.index = ri(t.index);
            }
            if let Some(t) = &mut pbr.metallic_roughness_texture {
                t.index = ri(t.index);
            }
        }
        if let Some(t) = &mut m.normal_texture {
            t.index = ri(t.index);
        }
        if let Some(t) = &mut m.occlusion_texture {
            t.index = ri(t.index);
        }
        if let Some(t) = &mut m.emissive_texture {
            t.index = ri(t.index);
        }
    }
}

pub(crate) fn remove_unused_samplers(model: &mut GltfModel, extra_used: &[usize]) {
    let mut used: Vec<usize> = model.textures.iter().filter_map(|t| t.sampler).collect();
    used.extend_from_slice(extra_used);
    used.sort_unstable();
    used.dedup();
    let remap = build_remap(model.samplers.len(), &used);
    model.samplers = used
        .iter()
        .filter_map(|&i| model.samplers.get(i).cloned())
        .collect();
    for t in &mut model.textures {
        if let Some(s) = t.sampler {
            t.sampler = remap.get(s).and_then(|o| *o);
        }
    }
}

pub(crate) fn remove_unused_images(model: &mut GltfModel, extra_used: &[usize]) {
    let mut used: Vec<usize> = model.textures.iter().filter_map(|t| t.source).collect();
    used.extend_from_slice(extra_used);
    used.sort_unstable();
    used.dedup();
    let remap = build_remap(model.images.len(), &used);
    model.images = used
        .iter()
        .filter_map(|&i| model.images.get(i).cloned())
        .collect();
    for t in &mut model.textures {
        if let Some(s) = t.source {
            t.source = remap.get(s).and_then(|o| *o);
        }
    }
}

pub(crate) fn remove_unused_accessors(model: &mut GltfModel, extra_used: &[usize]) {
    let mut used: Vec<usize> = model
        .meshes
        .iter()
        .flat_map(|m| {
            m.primitives.iter().flat_map(|p| {
                let mut v: Vec<usize> = p.attributes.values().copied().collect();
                if let Some(i) = p.indices {
                    v.push(i);
                }
                v
            })
        })
        .collect();
    for s in &model.skins {
        if let Some(i) = s.inverse_bind_matrices {
            used.push(i);
        }
    }
    for a in &model.animations {
        for s in &a.samplers {
            used.push(s.input);
            used.push(s.output);
        }
    }
    used.extend_from_slice(extra_used);
    used.sort_unstable();
    used.dedup();
    let remap = build_remap(model.accessors.len(), &used);
    model.accessors = used
        .iter()
        .filter_map(|&i| model.accessors.get(i).cloned())
        .collect();
    let ri = |i: usize| remap.get(i).and_then(|o| *o).unwrap_or(i);
    for m in &mut model.meshes {
        for p in &mut m.primitives {
            for v in p.attributes.values_mut() {
                *v = ri(*v);
            }
            if let Some(i) = p.indices {
                p.indices = Some(ri(i));
            }
        }
    }
    for s in &mut model.skins {
        if let Some(i) = s.inverse_bind_matrices {
            s.inverse_bind_matrices = Some(ri(i));
        }
    }
    for a in &mut model.animations {
        for s in &mut a.samplers {
            s.input = ri(s.input);
            s.output = ri(s.output);
        }
    }
}

pub(crate) fn remove_unused_buffer_views(model: &mut GltfModel, extra_used: &[usize]) {
    let mut used: Vec<usize> = model
        .accessors
        .iter()
        .flat_map(|a| {
            // Dense buffer view.
            let mut bvs: Vec<usize> = a.buffer_view.into_iter().collect();
            // Sparse accessor buffer views (indices + values).
            if let Some(sparse) = &a.sparse {
                bvs.push(sparse.indices.buffer_view);
                bvs.push(sparse.values.buffer_view);
            }
            bvs
        })
        .chain(model.images.iter().filter_map(|i| i.buffer_view))
        .collect();
    used.extend_from_slice(extra_used);
    used.sort_unstable();
    used.dedup();
    let remap = build_remap(model.buffer_views.len(), &used);
    model.buffer_views = used
        .iter()
        .filter_map(|&i| model.buffer_views.get(i).cloned())
        .collect();
    let ri = |i: usize| remap.get(i).and_then(|o| *o).unwrap_or(i);
    for a in &mut model.accessors {
        if let Some(bv) = a.buffer_view {
            a.buffer_view = Some(ri(bv));
        }
    }
    for i in &mut model.images {
        if let Some(bv) = i.buffer_view {
            i.buffer_view = Some(ri(bv));
        }
    }
}

pub(crate) fn remove_unused_meshes(model: &mut GltfModel, extra_used: &[usize]) {
    let mut used: Vec<usize> = model.nodes.iter().filter_map(|n| n.mesh).collect();
    used.extend_from_slice(extra_used);
    used.sort_unstable();
    used.dedup();
    let remap = build_remap(model.meshes.len(), &used);
    model.meshes = used
        .iter()
        .filter_map(|&i| model.meshes.get(i).cloned())
        .collect();
    for n in &mut model.nodes {
        if let Some(m) = n.mesh {
            n.mesh = remap.get(m).and_then(|o| *o);
        }
    }
}

pub(crate) fn remove_unused_materials(model: &mut GltfModel, extra_used: &[usize]) {
    let mut used: Vec<usize> = model
        .meshes
        .iter()
        .flat_map(|m| m.primitives.iter().filter_map(|p| p.material))
        .collect();
    used.extend_from_slice(extra_used);
    used.sort_unstable();
    used.dedup();
    let remap = build_remap(model.materials.len(), &used);
    model.materials = used
        .iter()
        .filter_map(|&i| model.materials.get(i).cloned())
        .collect();
    for m in &mut model.meshes {
        for p in &mut m.primitives {
            if let Some(mat) = p.material {
                p.material = remap.get(mat).and_then(|o| *o);
            }
        }
    }
}

impl GltfModel {
    /// Remove all unreferenced accessors, buffer views, buffers, images,
    /// samplers, textures, meshes, and materials, then compact all buffers.
    ///
    /// Equivalent to calling each `remove_unused_*` function in order followed
    /// by [`compact_buffers`].
    pub fn compact(&mut self) {
        remove_unused_accessors(self, &[]);
        remove_unused_buffer_views(self, &[]);
        remove_unused_images(self, &[]);
        remove_unused_samplers(self, &[]);
        remove_unused_textures(self, &[]);
        remove_unused_meshes(self, &[]);
        remove_unused_materials(self, &[]);
        compact_buffers(self);
    }
}
