//! Error types for glTF writing operations.

use std::io;
use thiserror::Error;

/// Result type for glTF write operations.
pub type WriteResult<T> = std::result::Result<T, WriteError>;

/// Errors that can occur during glTF writing.
#[derive(Error, Debug)]
pub enum WriteError {
    /// JSON serialization failed.
    #[error("JSON serialization failed: {0}")]
    JsonSerialization(#[from] serde_json::Error),

    /// Custom enum serialization failed.
    #[error("Failed to serialize enum value: {0}")]
    EnumSerialization(String),

    /// A codec encoder reported a fatal error.
    #[error("Codec error ({codec}): {reason}")]
    Codec { codec: &'static str, reason: String },

    /// Buffer data is missing or invalid.
    #[error("Invalid buffer: {0}")]
    InvalidBuffer(String),

    /// GLB file structure is invalid.
    #[error("Invalid GLB structure: {0}")]
    InvalidGlb(String),

    /// I/O operation failed.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Other write errors.
    #[error("Write error: {0}")]
    Other(String),
}
