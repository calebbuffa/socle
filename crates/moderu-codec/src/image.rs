//! Image decoding: PNG / JPEG / WebP → `Image` pixel data, and image encoder stub.

use crate::CodecEncoder;
use moderu::{GltfModel, Image};

/// Errors that can occur during image decode or encode operations.
#[derive(thiserror::Error, Debug)]
pub enum ImageError {
    #[error("format detection: {0}")]
    FormatDetection(std::io::Error),
    #[error("image decode: {0}")]
    Decode(image::ImageError),
    #[error("generate_mipmaps: image is GPU-compressed; decompress first")]
    ImageCompressed,
    #[error(
        "generate_mipmaps: expected RGBA8 (channels=4, bpc=1), got channels={channels} bpc={bpc}"
    )]
    InvalidFormat { channels: u32, bpc: u32 },
    #[error("generate_mipmaps: image already has mip levels")]
    AlreadyHasMips,
    #[error("generate_mipmaps: image has zero-size dimension")]
    ZeroSizeDimension,
    #[error(
        "generate_mipmaps: data length {actual} does not match {width}×{height}×4 = {expected}"
    )]
    DataLengthMismatch {
        actual: usize,
        width: u32,
        height: u32,
        expected: usize,
    },
    #[error("generate_mipmaps: failed to create ImageBuffer")]
    ImageBufferFailed,
    #[error("image feature not enabled")]
    FeatureDisabled,
}

/// Decode all embedded images referenced by `model.images[]`.
///
/// For each image that has a `bufferView` index, decodes the raw bytes from
/// `model.buffers` and stores the result in `model.images[i].pixels`.
pub fn decode(model: &mut GltfModel) -> Vec<String> {
    let mut warnings = Vec::new();

    for i in 0..model.images.len() {
        let Some(bv_idx) = model.images[i].buffer_view else {
            continue;
        };
        let bv_idx = bv_idx as usize;
        let Some(bv) = model.buffer_views.get(bv_idx) else {
            warnings.push(format!(
                "image[{i}]: bufferView index {bv_idx} out of range"
            ));
            continue;
        };

        let buf_idx = bv.buffer as usize;
        let byte_offset = bv.byte_offset as usize;
        let byte_length = bv.byte_length as usize;

        let raw: Vec<u8> = {
            let Some(buf) = model.buffers.get(buf_idx) else {
                warnings.push(format!("image[{i}]: buffer index {buf_idx} out of range"));
                continue;
            };
            let end = byte_offset + byte_length;
            if end > buf.data.len() {
                warnings.push(format!(
                    "image[{i}]: bufferView range [{byte_offset}..{end}) exceeds buffer size {}",
                    buf.data.len()
                ));
                continue;
            }
            buf.data[byte_offset..end].to_vec()
        };

        match decode_image_bytes(&raw) {
            Ok(img_data) => {
                model.images[i].pixels = img_data;
            }
            Err(e) => {
                warnings.push(format!("image[{i}]: decode failed: {e}"));
            }
        }
    }

    warnings
}

/// Low-level: decode raw image bytes (PNG/JPEG/WebP) into RGBA8 pixel data.
///
/// Accepts raw bytes and returns an RGBA8 decoded [`moderu::Image`].
/// Supported formats: PNG, JPEG, WebP.
///
/// # Example
/// ```ignore
/// let img = moderu_codec::image::decode_buffer(&png_bytes)?;
/// // img.data is RGBA8, img.width / img.height are pixel dimensions
/// ```
pub fn decode_buffer(data: &[u8]) -> Result<moderu::Image, ImageError> {
    decode_image_bytes(data)
}

fn decode_image_bytes(data: &[u8]) -> Result<Image, ImageError> {
    use image::ImageReader;
    use std::io::Cursor;

    let reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(ImageError::FormatDetection)?;

    let img = reader.decode().map_err(ImageError::Decode)?;

    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    Ok(Image {
        data: rgba.into_raw(),
        width,
        height,
        channels: 4,
        bytes_per_channel: 1,
        compressed_pixel_format: Default::default(),
        mip_positions: Vec::new(),
    })
}

/// Image encoder for texture format optimization.
pub(crate) struct ImageEncoder;

impl CodecEncoder for ImageEncoder {
    const EXT_NAME: &'static str = "image";
    type Error = ImageError;

    fn encode(model: &mut GltfModel) -> Result<(), ImageError> {
        #[cfg(not(feature = "image"))]
        {
            return Err(ImageError::FeatureDisabled);
        }

        #[cfg(feature = "image")]
        {
            // Image format optimization not yet implemented.
            // Could re-encode JPEG/PNG assets to more efficient formats like WebP.
            let _ = model;
        }
        Ok(())
    }
}

/// Encode/optimize embedded images.
pub fn encode(model: &mut GltfModel) -> Result<(), ImageError> {
    ImageEncoder::encode(model)
}

/// Generate a full mipmap chain for an uncompressed [`moderu::Image`].
///
/// After this call, `image.mip_positions` contains one entry per mip level
/// (starting at mip 0 which is the full-resolution level), and `image.data`
/// contains all levels concatenated: mip 0, mip 1, mip 2 … 1×1.
///
/// The image must be RGBA8 (channels == 4, bytes_per_channel == 1) and must
/// not already have compressed pixel data or existing mip levels.
///
/// Down-scaling uses a `Triangle` (bilinear) filter, matching the quality of
/// stb_image_resize2 used in CesiumGltfReader's `ImageDecoder`.
///
/// # Errors
/// Returns an error string if the image format is not RGBA8 or the `image`
/// crate cannot resize the input.
pub fn generate_mipmaps(img: &mut moderu::Image) -> Result<(), ImageError> {
    use image::{ImageBuffer, Rgba, imageops};
    use moderu::MipPosition;

    if img.compressed_pixel_format != moderu::GpuCompressedPixelFormat::None {
        return Err(ImageError::ImageCompressed);
    }
    if img.channels != 4 || img.bytes_per_channel != 1 {
        return Err(ImageError::InvalidFormat {
            channels: img.channels as u32,
            bpc: img.bytes_per_channel as u32,
        });
    }
    if !img.mip_positions.is_empty() {
        return Err(ImageError::AlreadyHasMips);
    }
    if img.width == 0 || img.height == 0 {
        return Err(ImageError::ZeroSizeDimension);
    }

    let mip0_len = img.data.len();
    let expected = (img.width * img.height * 4) as usize;
    if mip0_len != expected {
        return Err(ImageError::DataLengthMismatch {
            actual: mip0_len,
            width: img.width,
            height: img.height,
            expected,
        });
    }

    let mut out_data: Vec<u8> = Vec::with_capacity(mip0_len * 2);
    let mut mip_positions: Vec<MipPosition> = Vec::new();

    // Mip 0 — the original image.
    mip_positions.push(MipPosition {
        byte_offset: 0,
        byte_size: mip0_len,
    });
    out_data.extend_from_slice(&img.data);

    let mut mip_width = img.width;
    let mut mip_height = img.height;

    loop {
        if mip_width == 1 && mip_height == 1 {
            break;
        }
        let next_w = (mip_width / 2).max(1);
        let next_h = (mip_height / 2).max(1);

        // Build an ImageBuffer from the *previous* mip level's bytes.
        let Some(prev_pos) = mip_positions.last() else {
            break;
        };
        let prev_bytes = &out_data[prev_pos.byte_offset..prev_pos.byte_offset + prev_pos.byte_size];
        let prev_img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(mip_width, mip_height, prev_bytes.to_vec())
                .ok_or(ImageError::ImageBufferFailed)?;

        let resized = imageops::resize(&prev_img, next_w, next_h, imageops::FilterType::Triangle);
        let resized_data = resized.into_raw();
        let byte_offset = out_data.len();
        let byte_size = resized_data.len();
        out_data.extend_from_slice(&resized_data);
        mip_positions.push(MipPosition {
            byte_offset,
            byte_size,
        });

        mip_width = next_w;
        mip_height = next_h;
    }

    img.data = out_data;
    img.mip_positions = mip_positions;
    Ok(())
}
