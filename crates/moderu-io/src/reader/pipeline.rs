//! Post-processing pipeline.
//!
//! Runs each codec step in the correct order, matching cesium-native's
//! `postprocess()` sequence.

use super::error::Warnings;
use super::{GltfReaderOptions, Warning};
use moderu::GltfModel;

/// Run the full post-processing pipeline on a freshly parsed glTF model.
///
/// Order matches cesium-native:
/// 1. Data URL decoding
/// 2. Embedded image decoding
/// 3. Draco decompression
/// 4. Meshopt decompression
/// 5. SPZ Gaussian splat decompression
/// 6. KTX2 transcoding (after image buffers are populated)
/// 7. Dequantization
/// 8. Texture transform
pub fn run(options: &GltfReaderOptions, model: &mut GltfModel, warnings: &mut Warnings) {
    // 0. External file URI resolution (non-data: URIs loaded from disk).
    if options.images.resolve_external_references {
        if let Some(base_path) = &options.images.base_path {
            super::external_refs::resolve_external_refs(model, base_path, warnings);
        }
    }

    // 1. Data URL decoding.
    if options.images.decode_data_urls {
        super::uri::decode_data_urls(model, options.images.clear_decoded_data_urls, warnings);
    }

    // 2. Embedded image decoding (PNG/JPEG/WebP).
    #[cfg(feature = "image")]
    if options.images.decode_embedded_images {
        for msg in moderu_codec::image::decode(model) {
            warnings.push(Warning(msg));
        }
    }

    // 3. Draco decompression.
    #[cfg(feature = "draco")]
    if options.codecs.decode_draco {
        for msg in moderu_codec::draco::decode(model) {
            warnings.push(Warning(msg));
        }
    }

    // 4. Meshopt decompression.
    #[cfg(feature = "meshopt")]
    if options.codecs.decode_meshopt {
        for msg in moderu_codec::meshopt::decode(model) {
            warnings.push(Warning(msg));
        }
    }

    // 5. SPZ Gaussian splat decompression.
    #[cfg(feature = "spz")]
    if options.codecs.decode_spz {
        for msg in moderu_codec::spz::decode(model) {
            warnings.push(Warning(msg));
        }
    }

    // 6. KTX2 transcoding (run after image buffers populated by data URL step).
    #[cfg(feature = "ktx2")]
    if options.images.decode_embedded_images {
        for msg in moderu_codec::ktx2::decode(model) {
            warnings.push(Warning(msg));
        }
    }

    // 7. Dequantization.
    if options.mesh.dequantize {
        super::dequantize::dequantize(model, warnings);
    }

    // 8. Texture transform.
    if options.mesh.apply_texture_transform {
        super::khr_texture_transform::apply_texture_transforms(model, warnings);
    }
}
