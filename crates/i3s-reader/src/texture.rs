//! Texture decoding for I3S scene layer textures.
//!
//! I3S serves textures in JPEG, PNG, and KTX2 formats. This module decodes
//! them into RGBA pixel data ready for GPU upload.
//!
//! Requires the `textures` feature flag.

use image::GenericImageView;

use i3s_util::{I3SError, Result};

/// Decoded texture data.
#[derive(Debug, Clone)]
pub struct TextureData {
    /// RGBA pixel data, row-major, top to bottom.
    pub pixels: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl TextureData {
    /// Byte size of the pixel data.
    pub fn byte_size(&self) -> usize {
        self.pixels.len()
    }
}

/// The detected format of a texture blob.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    Jpeg,
    Png,
    /// Basis/KTX2 — not yet supported for decode; pass through as raw bytes.
    Ktx2,
    Unknown,
}

/// Detect texture format from the first bytes of the blob.
pub fn detect_format(data: &[u8]) -> TextureFormat {
    if data.len() >= 2 && data[0] == 0xFF && data[1] == 0xD8 {
        TextureFormat::Jpeg
    } else if data.len() >= 8 && &data[0..8] == b"\x89PNG\r\n\x1A\n" {
        TextureFormat::Png
    } else if data.len() >= 12 && &data[0..12] == b"\xABKTX 20\xBB\r\n\x1A\n" {
        TextureFormat::Ktx2
    } else {
        TextureFormat::Unknown
    }
}

/// Decode a JPEG or PNG texture into RGBA pixels.
///
/// # Errors
///
/// Returns [`I3SError::Texture`] if the image cannot be decoded.
pub fn decode_texture(data: &[u8]) -> Result<TextureData> {
    let img = image::load_from_memory(data)
        .map_err(|e| I3SError::Texture(format!("image decode failed: {e}")))?;

    let (width, height) = img.dimensions();
    let rgba = img.to_rgba8();

    Ok(TextureData {
        pixels: rgba.into_raw(),
        width,
        height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_jpeg() {
        let data = [0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(detect_format(&data), TextureFormat::Jpeg);
    }

    #[test]
    fn detect_png() {
        let data = b"\x89PNG\r\n\x1A\n\x00\x00";
        assert_eq!(detect_format(data), TextureFormat::Png);
    }

    #[test]
    fn detect_unknown() {
        let data = [0x00, 0x01, 0x02];
        assert_eq!(detect_format(&data), TextureFormat::Unknown);
    }

    #[test]
    fn decode_tiny_png() {
        // Generate a valid 1x1 red PNG using the image crate
        use image::{ImageBuffer, Rgb};
        use std::io::Cursor;

        let img = ImageBuffer::from_pixel(1, 1, Rgb([255u8, 0, 0]));
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        let png_data = buf.into_inner();

        let tex = decode_texture(&png_data).unwrap();
        assert_eq!(tex.width, 1);
        assert_eq!(tex.height, 1);
        assert_eq!(tex.pixels.len(), 4); // RGBA
        assert_eq!(tex.pixels[0], 255); // R
        assert_eq!(tex.pixels[1], 0); // G
        assert_eq!(tex.pixels[2], 0); // B
        assert_eq!(tex.pixels[3], 255); // A (opaque)
    }
}
