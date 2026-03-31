use smallvec::SmallVec;
use thiserror::Error;

/// Stack-allocated warnings collection.
/// Optimized for typical case (0–8 warnings); spills to heap only when needed.
pub type Warnings = SmallVec<[Warning; 8]>;

#[derive(Debug, Error)]
pub enum GltfError {
    #[error("invalid GLB: {0}")]
    InvalidGlb(String),

    #[error("JSON parse error: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("data URI decode error: {0}")]
    DataUri(String),

    #[error("image decode error: {0}")]
    ImageDecode(String),

    #[error("Draco decode error: {0}")]
    Draco(String),

    #[error("meshopt decode error: {0}")]
    Meshopt(String),

    #[error("KTX2 decode error: {0}")]
    Ktx2(String),

    #[error("SPZ decode error: {0}")]
    Spz(String),

    #[error("buffer index {0} out of range (have {1})")]
    BufferOutOfRange(usize, usize),

    #[error("buffer view index {0} out of range")]
    BufferViewOutOfRange(usize),

    #[error("accessor index {0} out of range")]
    AccessorOutOfRange(usize),

    /// Returned when an [`orkester_io::AssetAccessor`] fetch fails (feature `async`).
    #[error("asset fetch error: {0}")]
    Fetch(String),
}

/// A non-fatal warning emitted during glTF processing.
///
/// Each warning is a human-readable message from a specific pipeline step
/// (data URI decoding, codec decompression, dequantization, …).
#[derive(Clone, Debug)]
pub struct Warning(pub String);

impl std::fmt::Display for Warning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for Warning {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for Warning {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}
