//! GltfModel merging — appends all arrays from one model into another,
//! remapping every cross-array index to avoid collisions.

use crate::generated::{
    Accessor, Animation, BufferView, Material, Mesh, Node, Skin, Texture,
};
use crate::GltfModel;

impl GltfModel {
    /// Consume `other` and append all of its assets into `self`, remapping
    /// every index so the two models do not collide.
    ///
    /// The merge is intentionally shallow: animation targets, skins, and
    /// extensions are concatenated without cross-model validation.
    ///
    /// Model-level extensions use a *first-wins* policy so that an already-set
    /// `CESIUM_RTC` on `self` is not overwritten by `other`'s value.
    pub fn merge(mut self, other: GltfModel) -> GltfModel {
        let buf_off  = self.buffers.len();
        let bv_off   = self.buffer_views.len();
        let acc_off  = self.accessors.len();
        let img_off  = self.images.len();
        let samp_off = self.samplers.len();
        let tex_off  = self.textures.len();
        let mat_off  = self.materials.len();
        let mesh_off = self.meshes.len();
        let node_off = self.nodes.len();
        let skin_off = self.skins.len();

        self.buffers.extend(other.buffers);
        merge_buffer_views(&mut self, other.buffer_views, buf_off);
        merge_accessors(&mut self, other.accessors, bv_off);
        self.images.extend(other.images);
        self.samplers.extend(other.samplers);
        merge_textures(&mut self, other.textures, img_off, samp_off);
        for mut mat in other.materials {
            remap_material_textures(&mut mat, tex_off);
            self.materials.push(mat);
        }
        merge_meshes(&mut self, other.meshes, acc_off, mat_off);
        merge_skins(&mut self, other.skins, acc_off, node_off);
        merge_nodes(&mut self, other.nodes, mesh_off, skin_off, node_off);
        for mut scene in other.scenes {
            if let Some(nodes) = scene.nodes.as_mut() {
                for n in nodes {
                    *n += node_off;
                }
            }
            self.scenes.push(scene);
        }
        merge_animations(&mut self, other.animations, node_off, acc_off);
        self.extensions_used.extend(other.extensions_used);
        self.extensions_required.extend(other.extensions_required);
        self.extensions_used.sort();
        self.extensions_used.dedup();
        self.extensions_required.sort();
        self.extensions_required.dedup();
        for (k, v) in other.extensions {
            self.extensions.entry(k).or_insert(v);
        }
        self
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn merge_buffer_views(base: &mut GltfModel, bvs: Vec<BufferView>, buf_off: usize) {
    for mut bv in bvs {
        bv.buffer += buf_off;
        base.buffer_views.push(bv);
    }
}

fn merge_accessors(base: &mut GltfModel, accs: Vec<Accessor>, bv_off: usize) {
    for mut acc in accs {
        if let Some(bv) = acc.buffer_view.as_mut() {
            *bv += bv_off;
        }
        if let Some(sparse) = acc.sparse.as_mut() {
            sparse.indices.buffer_view += bv_off;
            sparse.values.buffer_view += bv_off;
        }
        base.accessors.push(acc);
    }
}

fn merge_textures(base: &mut GltfModel, textures: Vec<Texture>, img_off: usize, samp_off: usize) {
    for mut tex in textures {
        if let Some(s) = tex.sampler.as_mut() {
            *s += samp_off;
        }
        if let Some(s) = tex.source.as_mut() {
            *s += img_off;
        }
        base.textures.push(tex);
    }
}

fn merge_meshes(base: &mut GltfModel, meshes: Vec<Mesh>, acc_off: usize, mat_off: usize) {
    for mut mesh in meshes {
        for prim in &mut mesh.primitives {
            if let Some(idx) = prim.indices.as_mut() {
                *idx += acc_off;
            }
            for v in prim.attributes.values_mut() {
                *v += acc_off;
            }
            if let Some(m) = prim.material.as_mut() {
                *m += mat_off;
            }
            if let Some(targets) = prim.targets.as_mut() {
                for morph in targets {
                    for v in morph.values_mut() {
                        *v += acc_off;
                    }
                }
            }
        }
        base.meshes.push(mesh);
    }
}

fn merge_skins(base: &mut GltfModel, skins: Vec<Skin>, acc_off: usize, node_off: usize) {
    for mut skin in skins {
        if let Some(ibm) = skin.inverse_bind_matrices.as_mut() {
            *ibm += acc_off;
        }
        if let Some(sk) = skin.skeleton.as_mut() {
            *sk += node_off;
        }
        for j in &mut skin.joints {
            *j += node_off;
        }
        base.skins.push(skin);
    }
}

fn merge_nodes(
    base: &mut GltfModel,
    nodes: Vec<Node>,
    mesh_off: usize,
    skin_off: usize,
    node_off: usize,
) {
    for mut node in nodes {
        if let Some(m) = node.mesh.as_mut() {
            *m += mesh_off;
        }
        if let Some(s) = node.skin.as_mut() {
            *s += skin_off;
        }
        if let Some(cs) = node.children.as_mut() {
            for c in cs {
                *c += node_off;
            }
        }
        base.nodes.push(node);
    }
}

fn merge_animations(base: &mut GltfModel, anims: Vec<Animation>, node_off: usize, acc_off: usize) {
    for mut anim in anims {
        for ch in &mut anim.channels {
            if let Some(n) = ch.target.node.as_mut() {
                *n += node_off;
            }
        }
        for s in &mut anim.samplers {
            s.input += acc_off;
            s.output += acc_off;
        }
        base.animations.push(anim);
    }
}

fn remap_material_textures(mat: &mut Material, tex_off: usize) {
    if let Some(pbr) = mat.pbr_metallic_roughness.as_mut() {
        if let Some(t) = pbr.base_color_texture.as_mut() {
            t.index += tex_off;
        }
        if let Some(t) = pbr.metallic_roughness_texture.as_mut() {
            t.index += tex_off;
        }
    }
    if let Some(t) = mat.normal_texture.as_mut() {
        t.index += tex_off;
    }
    if let Some(t) = mat.occlusion_texture.as_mut() {
        t.index += tex_off;
    }
    if let Some(t) = mat.emissive_texture.as_mut() {
        t.index += tex_off;
    }
}
