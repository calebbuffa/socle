//! Error types for the i3s-native engine.

/// Errors that can occur when working with I3S data.
#[derive(Debug, thiserror::Error)]
pub enum I3sError {
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("binary buffer error: {0}")]
    Buffer(String),

    #[error("draco decode error: {0}")]
    Draco(String),

    #[error("lepcc decode error: {0}")]
    Lepcc(String),

    #[error("texture decode error: {0}")]
    Texture(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP error: {status} {url}")]
    Http { status: u16, url: String },

    #[error("network error: {0}")]
    Network(String),

    #[error("invalid data: {0}")]
    InvalidData(String),
}

/// A `Result` alias using [`I3sError`].
pub type Result<T> = std::result::Result<T, I3sError>;
