//! Geometry helpers: node transforms.

use crate::Node;
use glam::{DMat4, DQuat, DVec3};

/// Get the local-space transformation matrix for a node.
pub(crate) fn get_node_transform(node: &Node) -> Option<DMat4> {
    if node.matrix.len() == 16 {
        let m: [f64; 16] = node.matrix.as_slice().try_into().ok()?;
        return Some(DMat4::from_cols_array(&m));
    }
    let t = if node.translation.len() == 3 {
        DVec3::new(
            node.translation[0],
            node.translation[1],
            node.translation[2],
        )
    } else {
        DVec3::ZERO
    };
    let r = if node.rotation.len() == 4 {
        DQuat::from_xyzw(
            node.rotation[0],
            node.rotation[1],
            node.rotation[2],
            node.rotation[3],
        )
        .normalize()
    } else {
        DQuat::IDENTITY
    };
    let s = if node.scale.len() == 3 {
        DVec3::new(node.scale[0], node.scale[1], node.scale[2])
    } else {
        DVec3::ONE
    };
    Some(DMat4::from_scale_rotation_translation(s, r, t))
}

/// Set the local-space transformation matrix for a node (overwrites TRS components).
pub(crate) fn set_node_transform(node: &mut Node, mat: DMat4) {
    node.matrix = mat.to_cols_array().to_vec();
    node.translation.clear();
    node.rotation.clear();
    node.scale.clear();
}

impl crate::Node {
    /// Get the local-space transformation matrix for this node.
    pub fn transform(&self) -> Option<DMat4> {
        get_node_transform(self)
    }

    /// Overwrite this node's transform with a matrix (clears TRS components).
    pub fn set_transform(&mut self, mat: DMat4) {
        set_node_transform(self, mat);
    }

    /// Zero-copy view over the `TRANSLATION` accessor from `EXT_mesh_gpu_instancing`.
    pub fn instancing_translation<'a>(
        &self,
        model: &'a crate::GltfModel,
    ) -> Result<crate::AccessorView<'a, glam::Vec3>, crate::AccessorViewError> {
        let ext = self
            .extensions
            .get("EXT_mesh_gpu_instancing")
            .ok_or_else(|| {
                crate::AccessorViewError::MissingAttribute(
                    "EXT_mesh_gpu_instancing not present".into(),
                )
            })?;
        let acc_idx = ext
            .get("attributes")
            .and_then(|a| a.get("TRANSLATION"))
            .and_then(|v| v.as_u64())
            .ok_or_else(|| {
                crate::AccessorViewError::MissingAttribute(
                    "no TRANSLATION in EXT_mesh_gpu_instancing".into(),
                )
            })? as usize;
        crate::resolve_accessor(model, acc_idx)
    }
}
