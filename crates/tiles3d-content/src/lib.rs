//! `tiles3d-content` — 3D Tiles content processing.
//!
//! Provides binary tile format detection and decoding from B3DM, I3DM, GLB,
//! CMPT, and PNTS into [`moderu::GltfModel`], plus a [`GltfConverters`]
//! plugin registry matching the Cesium3DTilesContent API.

mod converters;
mod decoder;

pub use decoder::decode_tile;
pub use converters::{
    ConverterFn, GltfConverterResult, GltfConverters, register_all_tile_content_types,
};

/// Binary tile format detected from the URL or magic bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileFormat {
    /// glTF binary blob (magic `glTF`).
    Glb,
    /// Batched 3D Model (magic `b3dm`).
    B3dm,
    /// Instanced 3D Model (magic `i3dm`).
    I3dm,
    /// Composite (magic `cmpt`).
    Cmpt,
    /// Point cloud (magic `pnts`).
    Pnts,
    /// External tileset (JSON).
    Json,
    /// Unknown / unrecognised.
    Unknown,
}

impl TileFormat {
    /// Detect the format from the first four bytes of the response body.
    /// Falls back to URL-based detection when the magic is not recognised.
    pub fn detect(url: &str, data: &[u8]) -> Self {
        if data.len() >= 4 {
            match &data[..4] {
                b"glTF" => return Self::Glb,
                b"b3dm" => return Self::B3dm,
                b"i3dm" => return Self::I3dm,
                b"cmpt" => return Self::Cmpt,
                b"pnts" => return Self::Pnts,
                _ => {}
            }
        }
        // Fallback to extension.
        let path = url.split('?').next().unwrap_or(url);
        match path
            .rsplit('.')
            .next()
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("glb") => Self::Glb,
            Some("b3dm") => Self::B3dm,
            Some("i3dm") => Self::I3dm,
            Some("cmpt") => Self::Cmpt,
            Some("pnts") => Self::Pnts,
            Some("json") => Self::Json,
            _ => Self::Unknown,
        }
    }
}
