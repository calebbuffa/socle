//! glTF 2.0 Writer — save and serialize glTF models to JSON and GLB formats.
//!
//! This crate provides the ability to write glTF models back to disk in both JSON
//! and GLB (binary) formats, enabling round-trip workflows: load → modify → save → verify.

pub mod codec;
mod error;
mod glb;
mod writer;

pub use error::{WriteError, WriteResult};
pub use writer::{GltfWriter, GltfWriterOptions};

/// A convenience type alias for `Result<T, WriteError>`.
pub type Result<T> = std::result::Result<T, WriteError>;
