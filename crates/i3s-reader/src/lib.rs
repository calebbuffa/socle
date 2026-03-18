//! I3S content reader: JSON deserialization and binary buffer parsing.
//!
//! This crate provides functions to deserialize all I3S resource types:
//!
//! - **JSON resources**: scene layer documents, node pages, statistics
//! - **Binary geometry buffers**: uncompressed vertex/feature arrays
//! - **Binary attribute buffers**: per-field typed arrays (string, int, float)

pub mod attribute;
pub mod attribute_compression;
pub mod codec;
pub mod geometry;
pub mod json;

#[cfg(feature = "draco")]
pub mod draco;

#[cfg(feature = "textures")]
pub mod texture;

#[cfg(feature = "lepcc")]
pub mod lepcc;
