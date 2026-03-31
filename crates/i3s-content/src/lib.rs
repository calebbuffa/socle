//! `i3s-content` — I3S content processing (geometry buffers → [`GltfModel`]).

mod attributes;
mod decoder;

pub use attributes::{AttributeBuffer, AttributeDecodeError, AttributeValues, decode_attribute};
pub use decoder::{GeometryDecodeError, decode_geometry};
