//! Typed extension wrappers for common glTF extensions.
//!
//! Provides type-safe access to extension data without casting from generic JSON.

use serde_json::Value;
use std::collections::HashMap;

/// Trait for types that can wrap extension data.
pub trait TypedExtension: Sized {
    /// Extension name (e.g., "KHR_draco_mesh_compression").
    const NAME: &'static str;

    /// Try to parse from JSON value.
    fn from_json(value: &Value) -> Option<Self>;

    /// Convert back to JSON.
    fn to_json(&self) -> Value;
}

#[derive(Clone, Debug)]
pub struct KhrDracoMeshCompression {
    pub buffer_view: usize,
    pub attributes: HashMap<String, u32>,
}

impl TypedExtension for KhrDracoMeshCompression {
    const NAME: &'static str = "KHR_draco_mesh_compression";

    fn from_json(value: &Value) -> Option<Self> {
        Some(KhrDracoMeshCompression {
            buffer_view: value.get("bufferView")?.as_u64()? as usize,
            attributes: value
                .get("attributes")?
                .as_object()?
                .iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_u64()? as u32)))
                .collect(),
        })
    }

    fn to_json(&self) -> Value {
        serde_json::json!({
            "bufferView": self.buffer_view,
            "attributes": self.attributes
        })
    }
}

#[derive(Clone, Debug)]
pub struct KhrTextureTransform {
    pub offset: Option<[f32; 2]>,
    pub rotation: Option<f32>,
    pub scale: Option<[f32; 2]>,
    pub tex_coord: Option<u32>,
}

impl TypedExtension for KhrTextureTransform {
    const NAME: &'static str = "KHR_texture_transform";

    fn from_json(value: &Value) -> Option<Self> {
        let offset = value
            .get("offset")
            .and_then(|v| v.as_array())
            .and_then(|a| {
                if a.len() >= 2 {
                    Some([a[0].as_f64()? as f32, a[1].as_f64()? as f32])
                } else {
                    None
                }
            });
        let rotation = value
            .get("rotation")
            .and_then(|v| v.as_f64().map(|f| f as f32));
        let scale = value.get("scale").and_then(|v| v.as_array()).and_then(|a| {
            if a.len() >= 2 {
                Some([a[0].as_f64()? as f32, a[1].as_f64()? as f32])
            } else {
                None
            }
        });
        let tex_coord = value
            .get("texCoord")
            .and_then(|v| v.as_u64().map(|u| u as u32));
        Some(KhrTextureTransform {
            offset,
            rotation,
            scale,
            tex_coord,
        })
    }

    fn to_json(&self) -> Value {
        let mut obj = serde_json::Map::new();
        if let Some([ox, oy]) = self.offset {
            obj.insert("offset".to_string(), serde_json::json!([ox, oy]));
        }
        if let Some(rot) = self.rotation {
            obj.insert("rotation".to_string(), serde_json::json!(rot));
        }
        if let Some([sx, sy]) = self.scale {
            obj.insert("scale".to_string(), serde_json::json!([sx, sy]));
        }
        if let Some(tc) = self.tex_coord {
            obj.insert("texCoord".to_string(), serde_json::json!(tc));
        }
        Value::Object(obj)
    }
}

#[derive(Clone, Debug)]
pub struct KhrMeshQuantization {
    pub encoded_definer: Option<String>,
}

impl TypedExtension for KhrMeshQuantization {
    const NAME: &'static str = "KHR_mesh_quantization";

    fn from_json(value: &Value) -> Option<Self> {
        Some(KhrMeshQuantization {
            encoded_definer: value
                .get("encoded_definer")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        })
    }

    fn to_json(&self) -> Value {
        serde_json::json!({ "encoded_definer": self.encoded_definer })
    }
}

#[derive(Clone, Debug)]
pub struct KhrLightsPunctual {
    pub light: usize,
}

impl TypedExtension for KhrLightsPunctual {
    const NAME: &'static str = "KHR_lights_punctual";

    fn from_json(value: &Value) -> Option<Self> {
        Some(KhrLightsPunctual {
            light: value.get("light")?.as_u64()? as usize,
        })
    }

    fn to_json(&self) -> Value {
        serde_json::json!({ "light": self.light })
    }
}

#[derive(Clone, Debug)]
pub struct KhrMaterialsUnlit;

impl TypedExtension for KhrMaterialsUnlit {
    const NAME: &'static str = "KHR_materials_unlit";

    fn from_json(_value: &Value) -> Option<Self> {
        Some(KhrMaterialsUnlit)
    }

    fn to_json(&self) -> Value {
        Value::Object(Default::default())
    }
}

#[derive(Clone, Debug)]
pub struct ExtMeshGpuInstancing {
    pub attributes: HashMap<String, usize>,
}

impl TypedExtension for ExtMeshGpuInstancing {
    const NAME: &'static str = "EXT_mesh_gpu_instancing";

    fn from_json(value: &Value) -> Option<Self> {
        Some(ExtMeshGpuInstancing {
            attributes: value
                .get("attributes")?
                .as_object()?
                .iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_u64()? as usize)))
                .collect(),
        })
    }

    fn to_json(&self) -> Value {
        serde_json::json!({ "attributes": self.attributes })
    }
}

/// Registry for looking up and parsing typed extensions.
pub struct ExtensionRegistry;

impl ExtensionRegistry {
    /// Try to parse a known extension from JSON.
    pub fn parse_extension(name: &str, value: &Value) -> Option<Box<dyn std::any::Any>> {
        match name {
            KhrDracoMeshCompression::NAME => KhrDracoMeshCompression::from_json(value)
                .map(|e| Box::new(e) as Box<dyn std::any::Any>),
            KhrTextureTransform::NAME => {
                KhrTextureTransform::from_json(value).map(|e| Box::new(e) as Box<dyn std::any::Any>)
            }
            KhrMeshQuantization::NAME => {
                KhrMeshQuantization::from_json(value).map(|e| Box::new(e) as Box<dyn std::any::Any>)
            }
            KhrLightsPunctual::NAME => {
                KhrLightsPunctual::from_json(value).map(|e| Box::new(e) as Box<dyn std::any::Any>)
            }
            KhrMaterialsUnlit::NAME => {
                KhrMaterialsUnlit::from_json(value).map(|e| Box::new(e) as Box<dyn std::any::Any>)
            }
            ExtMeshGpuInstancing::NAME => ExtMeshGpuInstancing::from_json(value)
                .map(|e| Box::new(e) as Box<dyn std::any::Any>),
            _ => None,
        }
    }

    /// Check if an extension name is registered.
    pub fn is_known(name: &str) -> bool {
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
}
