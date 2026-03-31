//! KTX2 container parsing + basis-universal transcoding, and KTX2 encoder stub.
//!
//! Detects KTX2 images by magic bytes and transcodes them to
//! RGBA8 using the `basis-universal` transcoder.

use crate::CodecEncoder;
use moderu::{GltfModel, Image};

/// Errors that can occur during KTX2 decode or encode operations.
#[derive(thiserror::Error, Debug)]
pub enum Ktx2Error {
    #[error("KTX2 parse: {0}")]
    Parse(String),
    #[error("KTX2 has no mip levels")]
    NoMipLevels,
    #[error("invalid basis-universal header")]
    InvalidBasisHeader,
    #[error("basis transcode: {0}")]
    BasisTranscode(String),
    #[error("KTX2 RGBA data too short: {actual} < {expected}")]
    RgbaDataTooShort { actual: usize, expected: usize },
    #[error("KTX2 RGB data too short")]
    RgbDataTooShort,
    #[error("unsupported KTX2 format: {0}")]
    UnsupportedFormat(String),
    #[error("KTX2 feature not enabled")]
    FeatureDisabled,
}

// ---- Decoder ----

/// KTX2 magic: «KTX 20» (12 bytes).
const KTX2_MAGIC: [u8; 12] = [
    0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
];

/// Returns `true` if the data starts with the KTX2 magic bytes.
pub fn is_ktx2(data: &[u8]) -> bool {
    data.len() >= 12 && data[..12] == KTX2_MAGIC
}

/// Decode KTX2 images found among the model's image entries.
///
/// Only processes images whose buffer data starts with the KTX2 magic.
/// Transcodes to RGBA8 using basis-universal for supercompressed textures.
pub fn decode(model: &mut GltfModel) -> Vec<String> {
    let mut warnings = Vec::new();

    for i in 0..model.images.len() {
        // Only process images not yet decoded.
        if !model.images[i].pixels.data.is_empty() {
            continue;
        }

        let Some(bv_idx) = model.images[i].buffer_view else {
            continue;
        };
        let bv_idx = bv_idx as usize;
        let Some(bv) = model.buffer_views.get(bv_idx) else {
            continue;
        };
        let buf_idx = bv.buffer as usize;
        let byte_offset = bv.byte_offset as usize;
        let byte_length = bv.byte_length as usize;

        let raw: Vec<u8> = {
            let Some(buf) = model.buffers.get(buf_idx) else {
                continue;
            };
            let end = byte_offset + byte_length;
            if end > buf.data.len() {
                continue;
            }
            if !is_ktx2(&buf.data[byte_offset..end]) {
                continue;
            }
            buf.data[byte_offset..end].to_vec()
        };

        match transcode_ktx2_to_rgba8(&raw) {
            Ok(img_data) => {
                model.images[i].pixels = img_data;
            }
            Err(e) => {
                warnings.push(format!("image[{i}] KTX2 transcode: {e}"));
            }
        }
    }

    warnings
}

/// Low-level: decode a raw KTX2 buffer to RGBA8 pixel data.
///
/// Accepts the raw bytes of a KTX2 container and returns a decoded [`moderu::Image`].
/// Use [`is_ktx2`] to check if a buffer is KTX2 before calling this.
///
/// # Example
/// ```ignore
/// if moderu_codec::ktx2::is_ktx2(&data) {
///     let img = moderu_codec::ktx2::decode_buffer(&data)?;
/// }
/// ```
pub fn decode_buffer(data: &[u8]) -> Result<moderu::Image, Ktx2Error> {
    transcode_ktx2_to_rgba8(data)
}

/// Transcode a KTX2 blob to RGBA8 pixel data.
fn transcode_ktx2_to_rgba8(data: &[u8]) -> Result<Image, Ktx2Error> {
    use ktx2_reader::Reader;

    let reader = Reader::new(data).map_err(|e| Ktx2Error::Parse(format!("{e:?}")))?;

    let header = reader.header();
    let width = header.pixel_width;
    let height = header.pixel_height;

    // Collect level data (level 0 = base mip).
    let levels: Vec<Vec<u8>> = reader.levels().map(|level| level.data.to_vec()).collect();
    if levels.is_empty() {
        return Err(Ktx2Error::NoMipLevels);
    }

    let base_level = &levels[0];

    // Check if this is a basis-compressed texture that needs transcoding.
    if header.supercompression_scheme == Some(ktx2_reader::SupercompressionScheme::BasisLZ)
        || is_basis_format(header.format)
    {
        transcode_basis(base_level, width, height)
    } else {
        // Uncompressed or already-decoded KTX2 — try to read raw RGBA.
        extract_rgba_from_ktx2(base_level, width, height, header.format)
    }
}

fn is_basis_format(format: Option<ktx2_reader::Format>) -> bool {
    // BasisLZ compressed textures often report format as None.
    format.is_none()
}

fn transcode_basis(data: &[u8], width: u32, height: u32) -> Result<Image, Ktx2Error> {
    use basis_universal::transcoding::{Transcoder, TranscoderTextureFormat, transcoder_init};

    transcoder_init();

    let transcoder = Transcoder::new();

    if !transcoder.validate_header(data) {
        return Err(Ktx2Error::InvalidBasisHeader);
    }

    let transcode_result = transcoder
        .transcode_image_level(
            data,
            TranscoderTextureFormat::RGBA32,
            basis_universal::transcoding::TranscodeParameters {
                image_index: 0,
                level_index: 0,
                ..Default::default()
            },
        )
        .map_err(|e| Ktx2Error::BasisTranscode(format!("{e:?}")))?;

    Ok(Image {
        data: transcode_result,
        width,
        height,
        channels: 4,
        bytes_per_channel: 1,
        compressed_pixel_format: Default::default(),
        mip_positions: Vec::new(),
    })
}

fn extract_rgba_from_ktx2(
    data: &[u8],
    width: u32,
    height: u32,
    format: Option<ktx2_reader::Format>,
) -> Result<Image, Ktx2Error> {
    let expected_size = (width as usize) * (height as usize) * 4;

    match format {
        Some(ktx2_reader::Format::R8G8B8A8_UNORM) | Some(ktx2_reader::Format::R8G8B8A8_SRGB) => {
            if data.len() >= expected_size {
                Ok(Image {
                    data: data[..expected_size].to_vec(),
                    width,
                    height,
                    channels: 4,
                    bytes_per_channel: 1,
                    compressed_pixel_format: Default::default(),
                    mip_positions: Vec::new(),
                })
            } else {
                Err(Ktx2Error::RgbaDataTooShort {
                    actual: data.len(),
                    expected: expected_size,
                })
            }
        }
        Some(ktx2_reader::Format::R8G8B8_UNORM) | Some(ktx2_reader::Format::R8G8B8_SRGB) => {
            let rgb_size = (width as usize) * (height as usize) * 3;
            if data.len() < rgb_size {
                return Err(Ktx2Error::RgbDataTooShort);
            }
            // Convert RGB → RGBA.
            let mut rgba = Vec::with_capacity(expected_size);
            for pixel in data[..rgb_size].chunks_exact(3) {
                rgba.extend_from_slice(pixel);
                rgba.push(255);
            }
            Ok(Image {
                data: rgba,
                width,
                height,
                channels: 4,
                bytes_per_channel: 1,
                compressed_pixel_format: Default::default(),
                mip_positions: Vec::new(),
            })
        }
        _ => Err(Ktx2Error::UnsupportedFormat(format!("{format:?}"))),
    }
}

// ---- Encoder ----

/// KTX2 Basis Universal texture encoder.
pub(crate) struct Ktx2Encoder;

impl CodecEncoder for Ktx2Encoder {
    const EXT_NAME: &'static str = "EXT_texture_ktx2";
    type Error = Ktx2Error;

    fn encode(model: &mut GltfModel) -> Result<(), Ktx2Error> {
        #[cfg(not(feature = "ktx2"))]
        {
            return Err(Ktx2Error::FeatureDisabled);
        }

        #[cfg(feature = "ktx2")]
        {
            // KTX2/Basis Universal texture compression not yet implemented.
            // Images are passed through as-is for now.
            let _ = model;
        }
        Ok(())
    }
}

/// Encode textures with KTX2/Basis Universal compression.
pub fn encode(model: &mut GltfModel) -> Result<(), Ktx2Error> {
    Ktx2Encoder::encode(model)
}
