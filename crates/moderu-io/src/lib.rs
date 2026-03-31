//! glTF 2.0 reader and writer with full codec pipeline.
//!
//! Provides reading and writing of GLB / glTF JSON with optional codec support:
//! Draco, meshopt, KTX2, SPZ, and image decoding/encoding.
//!
//! ## Quick start
//!
//! ```ignore
//! use moderu_io::reader::{GltfReader, GltfOk};
//! use moderu_io::writer::GltfWriter;
//!
//! let GltfOk { model, .. } = GltfReader::default().read_file("model.glb")?;
//! GltfWriter::default().write_glb(&model, "output.glb")?;
//! ```

pub mod reader;
pub mod writer;

// Top-level re-exports for convenience.
pub use reader::{GltfError, GltfOk, GltfReader, GltfReaderOptions};
pub use writer::{GltfWriter, GltfWriterOptions};

// Re-export async URL resolution helper when the "async" feature is enabled.
#[cfg(feature = "async")]
pub use reader::async_external_refs::resolve_uri;
