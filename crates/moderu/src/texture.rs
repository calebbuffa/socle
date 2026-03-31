//! Texture utilities: KHR_texture_transform, TextureView, FeatureIdTextureView.

use crate::sampler::WrapMode;
use crate::{GltfModel, Material, TextureInfo};

/// Parses and applies the KHR_texture_transform extension to UV coordinates.
///
/// Constructed via [`KhrTextureTransform::from_json`] (infallible — bad JSON
/// silently falls back to identity values) or [`KhrTextureTransform::identity`].
#[derive(Debug, Clone)]
pub struct KhrTextureTransform {
    offset: [f64; 2],
    rotation: f64,
    scale: [f64; 2],
    sin_cos: [f64; 2],
    tex_coord_set_index: Option<i64>,
}

impl Default for KhrTextureTransform {
    fn default() -> Self {
        Self {
            offset: [0.0, 0.0],
            rotation: 0.0,
            scale: [1.0, 1.0],
            sin_cos: [0.0, 1.0],
            tex_coord_set_index: None,
        }
    }
}

impl KhrTextureTransform {
    pub fn identity() -> Self {
        Self::default()
    }

    /// Parse from a `serde_json::Value`. Invalid sub-fields fall back to identity.
    pub fn from_json(value: &serde_json::Value) -> Self {
        let mut t = Self::default();
        if let Some(arr) = value.get("offset").and_then(|v| v.as_array()) {
            if arr.len() >= 2 {
                t.offset = [
                    arr[0].as_f64().unwrap_or(0.0),
                    arr[1].as_f64().unwrap_or(0.0),
                ];
            }
        }
        if let Some(r) = value.get("rotation").and_then(|v| v.as_f64()) {
            t.rotation = r;
            t.sin_cos = [r.sin(), r.cos()];
        }
        if let Some(arr) = value.get("scale").and_then(|v| v.as_array()) {
            if arr.len() >= 2 {
                t.scale = [
                    arr[0].as_f64().unwrap_or(1.0),
                    arr[1].as_f64().unwrap_or(1.0),
                ];
            }
        }
        if let Some(tc) = value.get("texCoord").and_then(|v| v.as_i64()) {
            t.tex_coord_set_index = Some(tc);
        }
        t
    }

    pub fn from_texture_info_extensions(
        extensions: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Option<Self> {
        Some(Self::from_json(extensions.get("KHR_texture_transform")?))
    }

    pub fn offset(&self) -> [f64; 2] {
        self.offset
    }
    pub fn rotation(&self) -> f64 {
        self.rotation
    }
    pub fn scale(&self) -> [f64; 2] {
        self.scale
    }
    pub fn tex_coord_set_index(&self) -> Option<i64> {
        self.tex_coord_set_index
    }

    /// Apply the transform to a UV pair.
    /// u' = sx*(u*cos - v*sin) + ox,  v' = sy*(u*sin + v*cos) + oy
    pub fn apply(&self, u: f64, v: f64) -> [f64; 2] {
        let [sin_r, cos_r] = self.sin_cos;
        let [ox, oy] = self.offset;
        let [sx, sy] = self.scale;
        [
            sx * (u * cos_r - v * sin_r) + ox,
            sy * (u * sin_r + v * cos_r) + oy,
        ]
    }
}

/// Options for constructing a [`TextureView`].
#[derive(Debug, Clone, Default)]
pub struct TextureViewOptions {
    pub apply_khr_texture_transform: bool,
    pub make_image_copy: bool,
}

/// Error creating a [`TextureView`] or [`FeatureIdTextureView`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureViewError {
    InvalidTexture,
    InvalidSampler,
    InvalidImage,
    EmptyImage,
    InvalidBytesPerChannel,
    /// Channels list is empty, too long, or has out-of-range indices (FeatureId).
    InvalidChannels,
}

impl std::fmt::Display for TextureViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidTexture => "invalid texture index",
            Self::InvalidSampler => "invalid sampler index",
            Self::InvalidImage => "invalid image index",
            Self::EmptyImage => "image pixel data is empty",
            Self::InvalidBytesPerChannel => "image bytes-per-channel must be 1",
            Self::InvalidChannels => "invalid channel specification",
        })
    }
}
impl std::error::Error for TextureViewError {}

enum ImageStorage<'a> {
    Borrowed(&'a crate::image::Image),
    Owned(crate::image::Image),
}
impl<'a> ImageStorage<'a> {
    fn as_ref(&self) -> &crate::image::Image {
        match self {
            Self::Borrowed(r) => r,
            Self::Owned(o) => o,
        }
    }
}

/// A view into one texture from a [`GltfModel`], with optional UV sampling.
///
/// Always valid — construction returns `Result<Self, TextureViewError>`.
pub struct TextureView<'a> {
    image: ImageStorage<'a>,
    tex_coord_set: usize,
    wrap: [WrapMode; 2],
    texture_transform: Option<KhrTextureTransform>,
}

impl<'a> TextureView<'a> {
    pub fn new(
        model: &'a GltfModel,
        texture_index: usize,
        tex_coord_set: usize,
        extensions: &std::collections::HashMap<String, serde_json::Value>,
        options: &TextureViewOptions,
    ) -> Result<Self, TextureViewError> {
        let tex = model
            .textures
            .get(texture_index)
            .ok_or(TextureViewError::InvalidTexture)?;
        let wrap = if let Some(si) = tex.sampler {
            let s = model
                .samplers
                .get(si)
                .ok_or(TextureViewError::InvalidSampler)?;
            [WrapMode::from_gltf(s.wrap_s), WrapMode::from_gltf(s.wrap_t)]
        } else {
            [WrapMode::Repeat, WrapMode::Repeat]
        };
        let img_idx = tex.source.ok_or(TextureViewError::InvalidImage)?;
        let img = &model
            .images
            .get(img_idx)
            .ok_or(TextureViewError::InvalidImage)?
            .pixels;
        if img.data.is_empty() {
            return Err(TextureViewError::EmptyImage);
        }
        if img.bytes_per_channel != 1 {
            return Err(TextureViewError::InvalidBytesPerChannel);
        }
        let texture_transform = if options.apply_khr_texture_transform {
            KhrTextureTransform::from_texture_info_extensions(extensions)
        } else {
            None
        };
        let effective_tc = texture_transform
            .as_ref()
            .and_then(|t| t.tex_coord_set_index())
            .map(|i| i as usize)
            .unwrap_or(tex_coord_set);
        let image = if options.make_image_copy {
            ImageStorage::Owned(img.clone())
        } else {
            ImageStorage::Borrowed(img)
        };
        Ok(Self {
            image,
            tex_coord_set: effective_tc,
            wrap,
            texture_transform,
        })
    }

    pub fn tex_coord_set(&self) -> usize {
        self.tex_coord_set
    }
    pub fn wrap_modes(&self) -> [WrapMode; 2] {
        self.wrap
    }
    pub fn texture_transform(&self) -> Option<&KhrTextureTransform> {
        self.texture_transform.as_ref()
    }
    pub fn image(&self) -> &crate::image::Image {
        self.image.as_ref()
    }

    /// Sample at (u, v) with wrap + optional transform. Returns RGBA bytes.
    pub fn sample_nearest(&self, u: f64, v: f64) -> Option<[u8; 4]> {
        let img = self.image.as_ref();
        let [u, v] = if let Some(t) = &self.texture_transform {
            t.apply(u, v)
        } else {
            [u, v]
        };
        let u = self.wrap[0].apply(u);
        let v = self.wrap[1].apply(v);
        let x = ((u * img.width as f64) as u32).min(img.width.saturating_sub(1));
        let y = ((v * img.height as f64) as u32).min(img.height.saturating_sub(1));
        let ch = img.channels as usize;
        let px = ((y * img.width + x) as usize) * ch;
        let mut out = [0u8; 4];
        for i in 0..ch.min(4) {
            out[i] = *img.data.get(px + i)?;
        }
        Some(out)
    }
}

/// Get the TEXCOORD set index from a [`TextureInfo`].
pub fn texcoord_index(info: &TextureInfo) -> usize {
    info.tex_coord
}

/// Check if a [`TextureInfo`] carries KHR_texture_transform.
pub fn has_transform(info: &TextureInfo) -> bool {
    info.extensions.contains_key("KHR_texture_transform")
}

/// Get base-color texture index from a material.
pub fn base_color_index(material: &Material) -> Option<usize> {
    material
        .pbr_metallic_roughness
        .as_ref()
        .and_then(|pbr| pbr.base_color_texture.as_ref())
        .map(|t| t.index)
}

/// Get normal-map texture index from a material.
pub fn normal_map_index(material: &Material) -> Option<usize> {
    material.normal_texture.as_ref().map(|nt| nt.index)
}

pub struct FeatureIdTextureView<'a> {
    texture_view: TextureView<'a>,
    channels: Vec<u8>,
}

impl<'a> FeatureIdTextureView<'a> {
    pub fn new(
        model: &'a GltfModel,
        texture_index: usize,
        channels: Vec<u8>,
        tex_coord_set: usize,
        apply_transform: bool,
    ) -> Result<Self, TextureViewError> {
        if channels.is_empty() || channels.len() > 4 || channels.iter().any(|&c| c > 3) {
            return Err(TextureViewError::InvalidChannels);
        }
        let opts = TextureViewOptions {
            apply_khr_texture_transform: apply_transform,
            make_image_copy: false,
        };
        let texture_view = TextureView::new(
            model,
            texture_index,
            tex_coord_set,
            &Default::default(),
            &opts,
        )?;
        Ok(Self {
            texture_view,
            channels,
        })
    }

    pub fn channels(&self) -> &[u8] {
        &self.channels
    }
    pub fn texture_view(&self) -> &TextureView<'a> {
        &self.texture_view
    }

    /// Sample a feature ID at UV (u, v). Channels are big-endian.
    pub fn feature_id(&self, u: f64, v: f64) -> Option<i64> {
        let pixel = self.texture_view.sample_nearest(u, v)?;
        let mut id: i64 = 0;
        for &ch in &self.channels {
            id = (id << 8) | (pixel[ch as usize] as i64);
        }
        Some(id)
    }
}
