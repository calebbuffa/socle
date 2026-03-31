//! Codec implementations for glTF 2.0 models.
//!
//! Two levels of API:
//!
//! ### Buffer-level (format-agnostic)
//! Operate directly on raw bytes — no glTF model required.
//! Good for transcoding from formats like i3S that store compressed data
//! in plain buffers without a glTF wrapper.
//!
//! | Function | Input | Output |
//! |---|---|---|
//! | [`draco::decode_buffer`] | compressed bytes + attr id map | [`draco::DecodedMesh`] |
//! | [`meshopt::decode_vertex_buffer`] | compressed bytes | `Vec<u8>` |
//! | [`meshopt::decode_index_buffer`] | compressed bytes | `Vec<u32>` |
//! | [`spz::decode_buffer`] | SPZ bytes | [`spz::DecodedSplats`] |
//! | [`ktx2::decode_buffer`] | KTX2 bytes | `moderu::Image` |
//! | [`image::decode_buffer`] | PNG/JPEG/WebP bytes | `moderu::Image` |
//!
//! ### Model-level (glTF in-place)
//! Operate on a [`moderu::GltfModel`], decompressing or compressing all
//! relevant primitives/buffer-views in one call.
//!
//! ```ignore
//! // Decode all Draco primitives in a model loaded from i3S:
//! let warnings = moderu_codec::draco::decode(&mut model);
//!
//! // Or go buffer-level to build the model yourself:
//! let mesh = moderu_codec::draco::decode_buffer(&data, &attr_ids)?;
//! ```

/// Result of checking if a codec is applicable to the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicabilityResult {
    /// Codec can decode this model; proceed with decoding.
    Applicable,
    /// Codec cannot decode this model; skip it.
    NotApplicable,
}

impl From<bool> for ApplicabilityResult {
    fn from(b: bool) -> Self {
        if b {
            Self::Applicable
        } else {
            Self::NotApplicable
        }
    }
}

/// Trait for standardized codec decompression.
///
/// Implement this to add a new codec that integrates with the model-level pipeline.
pub trait CodecDecoder: Sized + Send + Sync {
    /// The registered glTF extension name (e.g., `"KHR_draco_mesh_compression"`).
    const EXT_NAME: &'static str;

    /// The error type returned by decode operations.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Check if this codec can decode the model.
    ///
    /// Default: checks `model.extensions_used` for `EXT_NAME`.
    fn can_decode(model: &moderu::GltfModel) -> ApplicabilityResult {
        model
            .extensions_used
            .iter()
            .any(|e| e == Self::EXT_NAME)
            .into()
    }

    /// Decode the contents of a mesh primitive.
    fn decode_primitive(
        model: &mut moderu::GltfModel,
        mesh_idx: usize,
        prim_idx: usize,
        ext: &serde_json::Value,
    ) -> Result<(), Self::Error> {
        let _ = (model, mesh_idx, prim_idx, ext);
        Ok(())
    }

    /// Decode the contents of a buffer view.
    fn decode_view(
        model: &mut moderu::GltfModel,
        bv_idx: usize,
        ext: &serde_json::Value,
    ) -> Result<(), Self::Error> {
        let _ = (model, bv_idx, ext);
        Ok(())
    }
}

/// Trait for standardized codec compression.
///
/// Implement this to add a new codec that integrates with the model-level pipeline.
pub trait CodecEncoder: Sized + Send + Sync {
    /// The registered glTF extension name (e.g., `"KHR_draco_mesh_compression"`).
    const EXT_NAME: &'static str;

    /// The error type returned by encode operations.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Encode the model using this codec.
    fn encode(model: &mut moderu::GltfModel) -> Result<(), Self::Error> {
        let _ = model;
        Ok(())
    }
}

/// Run `C::decode_primitive` over all mesh primitives, collecting warnings.
pub(crate) fn decode_primitives<C: CodecDecoder>(model: &mut moderu::GltfModel) -> Vec<String> {
    if C::can_decode(model) == ApplicabilityResult::NotApplicable {
        return vec![];
    }
    let mut warnings = Vec::new();
    for mesh_idx in 0..model.meshes.len() {
        for prim_idx in 0..model.meshes[mesh_idx].primitives.len() {
            let ext_value = model.meshes[mesh_idx].primitives[prim_idx]
                .extensions
                .get(C::EXT_NAME)
                .cloned();
            let Some(ext) = ext_value else { continue };
            if let Err(e) = C::decode_primitive(model, mesh_idx, prim_idx, &ext) {
                warnings.push(format!(
                    "mesh[{mesh_idx}].primitive[{prim_idx}] {}: {e}",
                    C::EXT_NAME
                ));
            }
        }
    }
    warnings
}

/// Run `C::decode_view` over all buffer views, collecting warnings.
pub(crate) fn decode_buffer_views<C: CodecDecoder>(model: &mut moderu::GltfModel) -> Vec<String> {
    if C::can_decode(model) == ApplicabilityResult::NotApplicable {
        return vec![];
    }
    let mut warnings = Vec::new();
    for bv_idx in 0..model.buffer_views.len() {
        let ext_value = model.buffer_views[bv_idx]
            .extensions
            .get(C::EXT_NAME)
            .cloned();
        let Some(ext) = ext_value else { continue };
        if let Err(e) = C::decode_view(model, bv_idx, &ext) {
            warnings.push(format!("bufferView[{bv_idx}] {}: {e}", C::EXT_NAME));
        }
    }
    warnings
}

#[cfg(feature = "draco")]
pub mod draco;

#[cfg(feature = "meshopt")]
pub mod meshopt;

#[cfg(feature = "spz")]
pub mod spz;

#[cfg(feature = "ktx2")]
pub mod ktx2;

#[cfg(feature = "image")]
pub mod image;
