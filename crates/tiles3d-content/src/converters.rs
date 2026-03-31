//! [`GltfConverters`] — a registry of tile-format-to-GltfModel converters.
//!
//! Mirrors `Cesium3DTilesContent::GltfConverters` from cesium-native.
//!
//! Built-in converters (GLB, B3DM, I3DM, CMPT, PNTS) are registered
//! automatically by [`GltfConverters::register_all`] or by calling
//! [`register_all_tile_content_types`]. Additional converters can be injected
//! at runtime via [`GltfConverters::register_magic`] and
//! [`GltfConverters::register_file_extension`].
//!
//! # Example
//!
//! ```
//! use tiles3d_content::{GltfConverters, GltfConverterResult};
//!
//! GltfConverters::register_all();
//!
//! let data: &[u8] = &[]; // real tile bytes
//! if let Some(convert) = GltfConverters::get_converter_by_magic(data) {
//!     let result = convert(data);
//!     assert!(result.model.is_some() || result.errors.len() > 0);
//! }
//! ```

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use kiban::file_extension;
use moderu::GltfModel;

// ---------------------------------------------------------------------------
// Public result type
// ---------------------------------------------------------------------------

/// Result returned by every converter function.
#[derive(Debug)]
pub struct GltfConverterResult {
    /// The decoded model. `None` if conversion failed or produced no geometry.
    pub model: Option<GltfModel>,
    /// Errors encountered during conversion.
    pub errors: Vec<String>,
    /// Non-fatal warnings.
    pub warnings: Vec<String>,
}

impl GltfConverterResult {
    /// Convenience constructor for a successful conversion.
    pub fn success(model: GltfModel) -> Self {
        Self {
            model: Some(model),
            errors: vec![],
            warnings: vec![],
        }
    }

    /// Convenience constructor for a failed conversion.
    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            model: None,
            errors: vec![error.into()],
            warnings: vec![],
        }
    }
}

/// A converter function: raw tile bytes → [`GltfConverterResult`].
pub type ConverterFn = fn(&[u8]) -> GltfConverterResult;

struct Registry {
    by_magic: HashMap<[u8; 4], ConverterFn>,
    by_extension: HashMap<String, ConverterFn>,
}

impl Registry {
    fn new() -> Self {
        Self {
            by_magic: HashMap::new(),
            by_extension: HashMap::new(),
        }
    }
}

fn global() -> &'static RwLock<Registry> {
    static REG: OnceLock<RwLock<Registry>> = OnceLock::new();
    REG.get_or_init(|| RwLock::new(Registry::new()))
}

// ---------------------------------------------------------------------------
// Built-in converters (thin wrappers over decoder.rs)
// ---------------------------------------------------------------------------

fn convert_glb(data: &[u8]) -> GltfConverterResult {
    match crate::decode_tile(data, &crate::TileFormat::Glb) {
        Some(m) => GltfConverterResult::success(m),
        None => GltfConverterResult::failure("GLB decoding failed"),
    }
}

fn convert_b3dm(data: &[u8]) -> GltfConverterResult {
    match crate::decode_tile(data, &crate::TileFormat::B3dm) {
        Some(m) => GltfConverterResult::success(m),
        None => GltfConverterResult::failure("B3DM decoding failed"),
    }
}

fn convert_i3dm(data: &[u8]) -> GltfConverterResult {
    match crate::decode_tile(data, &crate::TileFormat::I3dm) {
        Some(m) => GltfConverterResult::success(m),
        None => GltfConverterResult::failure("I3DM decoding failed"),
    }
}

fn convert_cmpt(data: &[u8]) -> GltfConverterResult {
    match crate::decode_tile(data, &crate::TileFormat::Cmpt) {
        Some(m) => GltfConverterResult::success(m),
        None => GltfConverterResult::failure("CMPT decoding failed"),
    }
}

fn convert_pnts(data: &[u8]) -> GltfConverterResult {
    match crate::decode_tile(data, &crate::TileFormat::Pnts) {
        Some(m) => GltfConverterResult::success(m),
        None => GltfConverterResult::failure("PNTS decoding failed"),
    }
}

// ---------------------------------------------------------------------------
// GltfConverters
// ---------------------------------------------------------------------------

/// Registry of tile-format converters indexed by 4-byte magic or file extension.
///
/// All methods are thread-safe via an internal global `RwLock`.
pub struct GltfConverters;

impl GltfConverters {
    /// Register all built-in converters (GLB, B3DM, I3DM, CMPT, PNTS).
    ///
    /// Safe to call multiple times — subsequent calls are idempotent.
    /// Equivalent to Cesium's `registerAllTileContentTypes()`.
    pub fn register_all() {
        Self::register_magic(*b"glTF", convert_glb);
        Self::register_magic(*b"b3dm", convert_b3dm);
        Self::register_magic(*b"i3dm", convert_i3dm);
        Self::register_magic(*b"cmpt", convert_cmpt);
        Self::register_magic(*b"pnts", convert_pnts);

        Self::register_file_extension("glb", convert_glb);
        Self::register_file_extension("b3dm", convert_b3dm);
        Self::register_file_extension("i3dm", convert_i3dm);
        Self::register_file_extension("cmpt", convert_cmpt);
        Self::register_file_extension("pnts", convert_pnts);
    }

    /// Register a converter for a 4-byte magic sequence.
    pub fn register_magic(magic: [u8; 4], converter: ConverterFn) {
        global().write().unwrap().by_magic.insert(magic, converter);
    }

    /// Register a converter for a file extension (lowercase, without `.`).
    pub fn register_file_extension(ext: impl Into<String>, converter: ConverterFn) {
        global()
            .write()
            .unwrap()
            .by_extension
            .insert(ext.into(), converter);
    }

    /// Look up a converter by the first 4 bytes of the tile data.
    ///
    /// Returns `None` if no converter is registered for that magic.
    pub fn get_converter_by_magic(data: &[u8]) -> Option<ConverterFn> {
        if data.len() < 4 {
            return None;
        }
        let magic: [u8; 4] = data[..4].try_into().unwrap();
        global().read().unwrap().by_magic.get(&magic).copied()
    }

    /// Look up a converter by file path (examines the extension).
    ///
    /// Returns `None` if no converter is registered for that extension.
    pub fn get_converter_by_file_extension(file_path: &str) -> Option<ConverterFn> {
        let ext = file_extension(file_path)?.to_ascii_lowercase();
        global().read().unwrap().by_extension.get(&ext).copied()
    }

    /// Convert tile bytes to a [`GltfConverterResult`], trying magic first
    /// then falling back to the file extension.
    ///
    /// Returns an error result if no converter is registered.
    pub fn convert(data: &[u8], file_path: &str) -> GltfConverterResult {
        let converter = Self::get_converter_by_magic(data)
            .or_else(|| Self::get_converter_by_file_extension(file_path));

        match converter {
            Some(f) => f(data),
            None => GltfConverterResult::failure(format!(
                "no converter registered for '{}' (magic {:?})",
                file_path,
                data.get(..4)
            )),
        }
    }
}

/// Register all built-in tile content type converters.
///
/// Call this once at application startup. Equivalent to Cesium's
/// `registerAllTileContentTypes()`.
pub fn register_all_tile_content_types() {
    GltfConverters::register_all();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_lookup_by_magic() {
        fn dummy(_: &[u8]) -> GltfConverterResult {
            GltfConverterResult::failure("dummy")
        }
        GltfConverters::register_magic(*b"TEST", dummy);
        let f = GltfConverters::get_converter_by_magic(b"TEST_payload");
        assert!(f.is_some());
    }

    #[test]
    fn register_and_lookup_by_extension() {
        fn dummy2(_: &[u8]) -> GltfConverterResult {
            GltfConverterResult::failure("dummy2")
        }
        GltfConverters::register_file_extension("xyz", dummy2);
        let f = GltfConverters::get_converter_by_file_extension("model.xyz");
        assert!(f.is_some());
        assert!(GltfConverters::get_converter_by_file_extension("model.abc").is_none());
    }

    #[test]
    fn convert_unknown_returns_error() {
        let result = GltfConverters::convert(b"\x00\x00\x00\x00", "unknown.bin");
        assert!(!result.errors.is_empty());
        assert!(result.model.is_none());
    }

    #[test]
    fn register_all_populates_known_formats() {
        GltfConverters::register_all();
        assert!(GltfConverters::get_converter_by_magic(b"glTF_rest").is_some());
        assert!(GltfConverters::get_converter_by_magic(b"b3dm_rest").is_some());
        assert!(GltfConverters::get_converter_by_magic(b"i3dm_rest").is_some());
        assert!(GltfConverters::get_converter_by_magic(b"cmpt_rest").is_some());
        assert!(GltfConverters::get_converter_by_magic(b"pnts_rest").is_some());
        assert!(GltfConverters::get_converter_by_file_extension("model.glb").is_some());
    }
}
