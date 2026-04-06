//! Typed extension wrappers for common glTF extensions.
//!
//! Provides type-safe access to extension data without casting from generic JSON.

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::collections::HashMap;

/// Trait for types that represent a named glTF extension.
///
/// Implementors must derive (or manually implement) [`Serialize`] and
/// [`Deserialize`]. The [`parse_extension`] free function uses `serde_json::from_value`
/// and `serde_json::to_value` for round-tripping, so no manual conversion code is needed.
pub trait GltfExtension: Sized + Serialize + DeserializeOwned {
    /// glTF extension name string (e.g. `"KHR_draco_mesh_compression"`).
    const NAME: &'static str;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrDracoMeshCompression {
    pub buffer_view: usize,
    pub attributes: HashMap<String, u32>,
}

impl GltfExtension for KhrDracoMeshCompression {
    const NAME: &'static str = "KHR_draco_mesh_compression";
}

/// Data bag for the `KHR_texture_transform` extension.
///
/// For UV-math operations (applying the transform to texture coordinates)
/// see [`crate::TextureTransform`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrTextureTransform {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<[f32; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<[f32; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tex_coord: Option<u32>,
}

impl GltfExtension for KhrTextureTransform {
    const NAME: &'static str = "KHR_texture_transform";
}

/// Data bag for the `KHR_mesh_quantization` extension.
///
/// This extension has no additional JSON fields beyond its presence in
/// `extensionsUsed`/`extensionsRequired`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KhrMeshQuantization;

impl GltfExtension for KhrMeshQuantization {
    const NAME: &'static str = "KHR_mesh_quantization";
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KhrLightsPunctual {
    pub light: usize,
}

impl GltfExtension for KhrLightsPunctual {
    const NAME: &'static str = "KHR_lights_punctual";
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KhrMaterialsUnlit;

impl GltfExtension for KhrMaterialsUnlit {
    const NAME: &'static str = "KHR_materials_unlit";
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExtMeshGpuInstancing {
    pub attributes: HashMap<String, usize>,
}

impl GltfExtension for ExtMeshGpuInstancing {
    const NAME: &'static str = "EXT_mesh_gpu_instancing";
}

/// Try to parse a known glTF extension from a JSON value.
///
/// Returns a `Box<dyn Any>` on success; downcast with `Box::downcast::<T>()`.
/// Returns `None` if `name` is unrecognised or the JSON is malformed.
pub fn parse_extension(name: &str, value: &Value) -> Option<Box<dyn std::any::Any>> {
    fn parse<T: GltfExtension + 'static>(value: &Value) -> Option<Box<dyn std::any::Any>> {
        serde_json::from_value::<T>(value.clone())
            .ok()
            .map(|e| Box::new(e) as Box<dyn std::any::Any>)
    }
    match name {
        KhrDracoMeshCompression::NAME => parse::<KhrDracoMeshCompression>(value),
        KhrTextureTransform::NAME => parse::<KhrTextureTransform>(value),
        KhrMeshQuantization::NAME => parse::<KhrMeshQuantization>(value),
        KhrLightsPunctual::NAME => parse::<KhrLightsPunctual>(value),
        KhrMaterialsUnlit::NAME => parse::<KhrMaterialsUnlit>(value),
        ExtMeshGpuInstancing::NAME => parse::<ExtMeshGpuInstancing>(value),
        _ => None,
    }
}

/// Returns `true` if `name` is a glTF extension name recognised by this module.
pub fn is_known_extension(name: &str) -> bool {
    matches!(
        name,
        KhrDracoMeshCompression::NAME
            | KhrTextureTransform::NAME
            | KhrMeshQuantization::NAME
            | KhrLightsPunctual::NAME
            | KhrMaterialsUnlit::NAME
            | ExtMeshGpuInstancing::NAME
    )
}
