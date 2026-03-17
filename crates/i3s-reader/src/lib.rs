//! I3S content reader: JSON deserialization and binary buffer parsing.
//!
//! This crate provides functions to deserialize all I3S resource types:
//!
//! - **JSON resources**: scene layer documents, node pages, statistics
//! - **Binary geometry buffers**: uncompressed vertex/feature arrays
//! - **Binary attribute buffers**: per-field typed arrays (string, int, float)
//!
//! # Examples
//!
//! ```no_run
//! use i3s_reader::json;
//! use i3s::core::SceneLayerInfo;
//!
//! let bytes = std::fs::read("layer.json").unwrap();
//! let layer: SceneLayerInfo = json::read_json(&bytes).unwrap();
//! println!("Layer: {}", layer.name.unwrap_or_default());
//! ```

pub mod attribute;
pub mod geometry;
pub mod json;

#[cfg(feature = "draco")]
pub mod draco;

#[cfg(feature = "textures")]
pub mod texture;
