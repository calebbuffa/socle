/// Runtime buffer data that is not part of the glTF JSON but loaded separately.
///
/// Corresponds to `BufferCesium` in cesium-native — a container for the raw
/// binary payload of a glTF buffer after it has been fetched/decoded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BufferData {
    pub data: Vec<u8>,
}

/// Runtime image data that is not part of the glTF JSON but loaded separately.
///
/// Holds the decoded pixel data of a glTF image.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImageData {
    /// Raw pixel bytes.
    pub pixel_data: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Number of channels (e.g. 3 for RGB, 4 for RGBA).
    pub channels: u32,
    /// Bytes per channel (e.g. 1 for u8 pixels, 2 for u16 pixels).
    pub bytes_per_channel: u32,
}
