/// Position of one mip level within an image's pixel data.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MipPosition {
    pub byte_offset: usize,
    pub byte_size: usize,
}

/// Runtime image data — decoded pixel data of a glTF image.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImageData {
    /// Raw decoded pixel bytes (for mip 0 unless `mip_positions` is set).
    pub data: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Number of channels (e.g. 3 for RGB, 4 for RGBA).
    pub channels: u32,
    /// Bytes per channel (e.g. 1 for u8, 2 for u16).
    pub bytes_per_channel: u32,
    /// GPU-compressed format, if the image is block-compressed.
    pub compressed_pixel_format: crate::image::GpuCompressedPixelFormat,
    /// Byte positions of each mip level within `data`.
    /// Empty for uncompressed images with a single mip level.
    pub mip_positions: Vec<MipPosition>,
}

/// A rectangle within an image in pixel coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rectangle {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rectangle {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
    pub fn x_end(self) -> i32 {
        self.x + self.width
    }
    pub fn y_end(self) -> i32 {
        self.y + self.height
    }
}
/// GPU-compressed pixel formats supported by transcoded KTX2 images.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GpuCompressedPixelFormat {
    #[default]
    None,
    Etc1Rgb,
    Etc2Rgba,
    Bc1Rgb,
    Bc3Rgba,
    Bc4R,
    Bc5Rg,
    Bc7Rgba,
    Pvrtc1_4Rgb,
    Pvrtc1_4Rgba,
    Astc4x4Rgba,
    Pvrtc2_4Rgb,
    Pvrtc2_4Rgba,
    Etc2EacR11,
    Etc2EacRg11,
}

bitflags::bitflags! {
    /// Bitset of GPU-compressed pixel formats a device supports.
    /// Use `|` to combine formats, `.contains()` to query.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
    pub struct SupportedGpuCompressedPixelFormats: u16 {
        const ETC1_RGB      = 0b0000_0000_0000_0001;
        const ETC2_RGBA     = 0b0000_0000_0000_0010;
        const BC1_RGB       = 0b0000_0000_0000_0100;
        const BC3_RGBA      = 0b0000_0000_0000_1000;
        const BC4_R         = 0b0000_0000_0001_0000;
        const BC5_RG        = 0b0000_0000_0010_0000;
        const BC7_RGBA      = 0b0000_0000_0100_0000;
        const PVRTC1_4_RGB  = 0b0000_0000_1000_0000;
        const PVRTC1_4_RGBA = 0b0000_0001_0000_0000;
        const ASTC_4X4_RGBA = 0b0000_0010_0000_0000;
        const PVRTC2_4_RGB  = 0b0000_0100_0000_0000;
        const PVRTC2_4_RGBA = 0b0000_1000_0000_0000;
        const ETC2_EAC_R11  = 0b0001_0000_0000_0000;
        const ETC2_EAC_RG11 = 0b0010_0000_0000_0000;
    }
}

/// Maps from KTX2 container channel type to the best available GPU format.
#[derive(Debug, Clone, Copy, Default)]
pub struct Ktx2TranscodeTargets {
    pub rgba32: GpuCompressedPixelFormat,
    pub rgb8: GpuCompressedPixelFormat,
    pub rg8: GpuCompressedPixelFormat,
    pub r8: GpuCompressedPixelFormat,
    pub rgba8_srgb: GpuCompressedPixelFormat,
    pub rgb8_srgb: GpuCompressedPixelFormat,
}

impl Ktx2TranscodeTargets {
    /// Choose the best available target format for each channel layout.
    pub fn from_supported(s: SupportedGpuCompressedPixelFormats) -> Self {
        use GpuCompressedPixelFormat as F;
        use SupportedGpuCompressedPixelFormats as S;
        let pick = |choices: &[(S, F)]| -> F {
            choices
                .iter()
                .find(|(flag, _)| s.contains(*flag))
                .map_or(F::None, |&(_, fmt)| fmt)
        };
        Self {
            rgba32: pick(&[
                (S::BC7_RGBA, F::Bc7Rgba),
                (S::ETC2_RGBA, F::Etc2Rgba),
                (S::BC3_RGBA, F::Bc3Rgba),
                (S::PVRTC1_4_RGBA, F::Pvrtc1_4Rgba),
                (S::ASTC_4X4_RGBA, F::Astc4x4Rgba),
            ]),
            rgba8_srgb: pick(&[(S::BC7_RGBA, F::Bc7Rgba), (S::ETC2_RGBA, F::Etc2Rgba)]),
            rgb8: pick(&[
                (S::BC1_RGB, F::Bc1Rgb),
                (S::ETC1_RGB, F::Etc1Rgb),
                (S::PVRTC1_4_RGB, F::Pvrtc1_4Rgb),
            ]),
            rgb8_srgb: pick(&[(S::BC1_RGB, F::Bc1Rgb), (S::ETC1_RGB, F::Etc1Rgb)]),
            rg8: pick(&[(S::BC5_RG, F::Bc5Rg), (S::ETC2_EAC_RG11, F::Etc2EacRg11)]),
            r8: pick(&[(S::BC4_R, F::Bc4R), (S::ETC2_EAC_R11, F::Etc2EacR11)]),
        }
    }
}

/// Error returned by image manipulation operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BlitError {
    /// Source and target have different channel counts.
    #[error("channel count mismatch: src={src_channels}, dst={dst_channels}")]
    ChannelMismatch {
        src_channels: u32,
        dst_channels: u32,
    },
    /// Source and target have different bytes-per-channel.
    #[error("bytes per channel mismatch: src={src_bpc}, dst={dst_bpc}")]
    BytesPerChannelMismatch { src_bpc: u32, dst_bpc: u32 },
    /// The source rectangle is out of bounds for the source image.
    #[error("source rectangle out of bounds")]
    SrcOutOfBounds,
    /// The target rectangle is out of bounds for the target image.
    #[error("target rectangle out of bounds")]
    DstOutOfBounds,
    /// Cannot resize images with bytes_per_channel != 1.
    #[error("resize only supports 1 byte/channel, got {bpc}")]
    ResizeUnsupportedBpc { bpc: u32 },
    /// Image has a GPU-compressed format and cannot be manipulated at the pixel level.
    #[error("cannot manipulate GPU-compressed images")]
    CompressedFormat,
}

impl ImageData {
    /// Bytes per pixel (`channels * bytes_per_channel`).
    #[inline]
    pub fn bytes_per_pixel(&self) -> u32 {
        self.channels * self.bytes_per_channel
    }

    /// Row stride in bytes.
    #[inline]
    pub fn row_stride(&self) -> usize {
        (self.width * self.bytes_per_pixel()) as usize
    }

    /// Copy a rectangular region from `src` into `self`.
    ///
    /// If `src_rect` and `dst_rect` have different dimensions, the source
    /// region is resampled with bilinear interpolation (8-bit images only).
    /// For equal dimensions the copy is a direct `memcpy` per row.
    pub fn blit(
        &mut self,
        src: &ImageData,
        src_rect: Rectangle,
        dst_rect: Rectangle,
    ) -> Result<(), BlitError> {
        if src.compressed_pixel_format != GpuCompressedPixelFormat::None
            || self.compressed_pixel_format != GpuCompressedPixelFormat::None
        {
            return Err(BlitError::CompressedFormat);
        }
        if src.channels != self.channels {
            return Err(BlitError::ChannelMismatch {
                src_channels: src.channels,
                dst_channels: self.channels,
            });
        }
        if src.bytes_per_channel != self.bytes_per_channel {
            return Err(BlitError::BytesPerChannelMismatch {
                src_bpc: src.bytes_per_channel,
                dst_bpc: self.bytes_per_channel,
            });
        }

        // Bounds checks.
        if src_rect.x < 0
            || src_rect.y < 0
            || src_rect.x_end() > src.width as i32
            || src_rect.y_end() > src.height as i32
        {
            return Err(BlitError::SrcOutOfBounds);
        }
        if dst_rect.x < 0
            || dst_rect.y < 0
            || dst_rect.x_end() > self.width as i32
            || dst_rect.y_end() > self.height as i32
        {
            return Err(BlitError::DstOutOfBounds);
        }

        let bpp = self.bytes_per_pixel() as usize;

        if src_rect.width == dst_rect.width && src_rect.height == dst_rect.height {
            // Direct copy — row by row.
            let src_stride = src.row_stride();
            let dst_stride = self.row_stride();
            let row_bytes = dst_rect.width as usize * bpp;

            for row in 0..dst_rect.height as usize {
                let sy = src_rect.y as usize + row;
                let dy = dst_rect.y as usize + row;
                let src_off = sy * src_stride + src_rect.x as usize * bpp;
                let dst_off = dy * dst_stride + dst_rect.x as usize * bpp;
                self.data[dst_off..dst_off + row_bytes]
                    .copy_from_slice(&src.data[src_off..src_off + row_bytes]);
            }
        } else {
            // Resample — bilinear interpolation (8-bit only).
            if self.bytes_per_channel != 1 {
                return Err(BlitError::ResizeUnsupportedBpc {
                    bpc: self.bytes_per_channel,
                });
            }
            let channels = self.channels as usize;
            let src_stride = src.row_stride();
            let dst_stride = self.row_stride();

            for dy in 0..dst_rect.height as usize {
                let v = (dy as f64 + 0.5) / dst_rect.height as f64;
                let sy_f = v * src_rect.height as f64 - 0.5;
                let sy0 = (sy_f.floor() as i64).max(0) as usize;
                let sy1 = (sy0 + 1).min(src_rect.height as usize - 1);
                let fy = sy_f - sy_f.floor();

                for dx in 0..dst_rect.width as usize {
                    let u = (dx as f64 + 0.5) / dst_rect.width as f64;
                    let sx_f = u * src_rect.width as f64 - 0.5;
                    let sx0 = (sx_f.floor() as i64).max(0) as usize;
                    let sx1 = (sx0 + 1).min(src_rect.width as usize - 1);
                    let fx = sx_f - sx_f.floor();

                    for c in 0..channels {
                        let sample = |row: usize, col: usize| -> f64 {
                            let off = (src_rect.y as usize + row) * src_stride
                                + (src_rect.x as usize + col) * bpp
                                + c;
                            src.data[off] as f64
                        };

                        let val = sample(sy0, sx0) * (1.0 - fx) * (1.0 - fy)
                            + sample(sy0, sx1) * fx * (1.0 - fy)
                            + sample(sy1, sx0) * (1.0 - fx) * fy
                            + sample(sy1, sx1) * fx * fy;

                        let dst_off = (dst_rect.y as usize + dy) * dst_stride
                            + (dst_rect.x as usize + dx) * bpp
                            + c;
                        self.data[dst_off] = val.round().clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }

        Ok(())
    }

    /// Create a new image by resizing `self` with bilinear interpolation.
    ///
    /// Only supported for 8-bit-per-channel uncompressed images.
    pub fn resize(&self, new_width: u32, new_height: u32) -> Result<ImageData, BlitError> {
        if self.compressed_pixel_format != GpuCompressedPixelFormat::None {
            return Err(BlitError::CompressedFormat);
        }
        if self.bytes_per_channel != 1 {
            return Err(BlitError::ResizeUnsupportedBpc {
                bpc: self.bytes_per_channel,
            });
        }

        let mut dst = ImageData {
            data: vec![0u8; (new_width * new_height * self.bytes_per_pixel()) as usize],
            width: new_width,
            height: new_height,
            channels: self.channels,
            bytes_per_channel: self.bytes_per_channel,
            compressed_pixel_format: GpuCompressedPixelFormat::None,
            mip_positions: Vec::new(),
        };

        dst.blit(
            self,
            Rectangle::new(0, 0, self.width as i32, self.height as i32),
            Rectangle::new(0, 0, new_width as i32, new_height as i32),
        )?;

        Ok(dst)
    }

    /// Create a new RGBA image filled with a solid color.
    pub fn solid_rgba(width: u32, height: u32, r: u8, g: u8, b: u8, a: u8) -> Self {
        let pixel = [r, g, b, a];
        let data: Vec<u8> = pixel
            .iter()
            .copied()
            .cycle()
            .take((width * height * 4) as usize)
            .collect();
        Self {
            data,
            width,
            height,
            channels: 4,
            bytes_per_channel: 1,
            compressed_pixel_format: GpuCompressedPixelFormat::None,
            mip_positions: Vec::new(),
        }
    }

    /// Flip the image vertically (top-down ↔ bottom-up).
    pub fn vflip(&mut self) {
        let stride = self.row_stride();
        let h = self.height as usize;
        for row in 0..h / 2 {
            let top = row * stride;
            let bot = (h - 1 - row) * stride;
            // Swap row `row` with row `h - 1 - row`.
            for i in 0..stride {
                self.data.swap(top + i, bot + i);
            }
        }
    }
}
