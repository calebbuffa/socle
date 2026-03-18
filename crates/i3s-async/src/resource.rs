//! I3S resource addressing types.

/// Requested texture format for texture fetches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureRequestFormat {
    Jpeg,
    Png,
    Dds,
    Ktx2,
}

impl TextureRequestFormat {
    /// File extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Dds => "dds",
            Self::Ktx2 => "ktx2",
        }
    }

    /// MIME type for Accept header.
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Dds => "image/vnd-ms.dds",
            Self::Ktx2 => "image/ktx2",
        }
    }
}
