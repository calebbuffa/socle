/// Position of one mip level within an image's pixel data.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MipPosition {
    pub byte_offset: usize,
    pub byte_size: usize,
}

/// Runtime image data — decoded pixel data of a glTF image.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Image {
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
