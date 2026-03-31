//! Writers for 3D Tiles objects — [`TilesetWriter`], [`SubtreeWriter`], [`SchemaWriter`].
//!
//! All writers are sync and infallible for well-formed inputs; errors are
//! captured in the result's `errors` field rather than propagated as `Err`.
//!
//! # Tileset example
//!
//! ```no_run
//! use tiles3d::{Tileset, Asset, Tile, BoundingVolume, Refine, TilesetWriter, WriteOptions};
//!
//! let tileset = Tileset {
//!     asset: Asset { version: "1.1".into(), ..Default::default() },
//!     geometric_error: 1000.0,
//!     root: Tile {
//!         bounding_volume: BoundingVolume { sphere: vec![0.0, 0.0, 0.0, 1000.0], ..Default::default() },
//!         geometric_error: 1000.0,
//!         refine: Some(Refine::Replace),
//!         ..Default::default()
//!     },
//!     ..Default::default()
//! };
//!
//! let result = TilesetWriter::write_tileset(&tileset, WriteOptions::default());
//! assert!(result.errors.is_empty());
//! std::fs::write("tileset.json", &result.bytes).unwrap();
//! ```
//!
//! # Subtree example
//!
//! ```no_run
//! use tiles3d::{Subtree, Availability, SubtreeWriter, WriteOptions};
//!
//! let subtree = Subtree {
//!     tile_availability: Availability { constant: Some(1), ..Default::default() },
//!     child_subtree_availability: Availability { constant: Some(0), ..Default::default() },
//!     ..Default::default()
//! };
//!
//! // Write JSON subtree.
//! let result = SubtreeWriter::write_subtree_json(&subtree, WriteOptions::default());
//!
//! // Write binary subtree (first buffer is the inline binary chunk).
//! let binary_payload = vec![0u8; 16];
//! let result = SubtreeWriter::write_subtree_binary(&subtree, &binary_payload, WriteOptions::default());
//! ```

use crate::generated::{Buffer, Schema, Subtree, Tileset};

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

/// Options controlling serialization behaviour.
#[derive(Debug, Clone, Default)]
pub struct WriteOptions {
    /// Emit pretty-printed (indented) JSON. Default: compact.
    pub pretty_print: bool,
}

/// Result of a [`TilesetWriter::write_tileset`] call.
#[derive(Debug)]
pub struct TilesetWriterResult {
    /// The serialized JSON bytes. Empty if a fatal error occurred.
    pub bytes: Vec<u8>,
    /// Fatal errors (e.g. non-string map keys in an extension value).
    pub errors: Vec<String>,
    /// Non-fatal warnings. Currently always empty; reserved for future use.
    pub warnings: Vec<String>,
}

/// Result of a [`SubtreeWriter`] call.
#[derive(Debug)]
pub struct SubtreeWriterResult {
    /// The serialized bytes (JSON or binary envelope). Empty on error.
    pub bytes: Vec<u8>,
    /// Fatal errors.
    pub errors: Vec<String>,
    /// Non-fatal warnings. Currently always empty; reserved for future use.
    pub warnings: Vec<String>,
}

/// Result of a [`SchemaWriter::write_schema`] call.
#[derive(Debug)]
pub struct SchemaWriterResult {
    /// The serialized JSON bytes. Empty if a fatal error occurred.
    pub bytes: Vec<u8>,
    /// Fatal errors.
    pub errors: Vec<String>,
    /// Non-fatal warnings. Currently always empty; reserved for future use.
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// TilesetWriter
// ---------------------------------------------------------------------------

/// Serializes a [`Tileset`] to JSON bytes.
pub struct TilesetWriter;

impl TilesetWriter {
    /// Serialize a tileset to JSON.
    pub fn write_tileset(tileset: &Tileset, opts: WriteOptions) -> TilesetWriterResult {
        match serialize(tileset, opts.pretty_print) {
            Ok(bytes) => TilesetWriterResult {
                bytes,
                errors: vec![],
                warnings: vec![],
            },
            Err(e) => TilesetWriterResult {
                bytes: vec![],
                errors: vec![e.to_string()],
                warnings: vec![],
            },
        }
    }
}

// ---------------------------------------------------------------------------
// SubtreeWriter
// ---------------------------------------------------------------------------

/// Serializes a [`Subtree`] to JSON or the binary `.subtree` envelope.
///
/// ## Binary envelope layout
///
/// ```text
///  0.. 4  magic             = b"subt"
///  4.. 8  version           = 1  (u32 LE)
///  8..16  json_byte_length  (u64 LE, padded to 8-byte alignment)
/// 16..24  binary_byte_length (u64 LE)
/// 24..    JSON UTF-8 + alignment padding (0x20)
///         binary blob
/// ```
pub struct SubtreeWriter;

impl SubtreeWriter {
    /// Serialize a subtree to JSON (`.subtree` JSON form).
    ///
    /// External buffer URIs (`buffer.uri`) must be set on the subtree before
    /// calling this; the binary payload lives in separate files.
    pub fn write_subtree_json(subtree: &Subtree, opts: WriteOptions) -> SubtreeWriterResult {
        match serialize(subtree, opts.pretty_print) {
            Ok(bytes) => SubtreeWriterResult {
                bytes,
                errors: vec![],
                warnings: vec![],
            },
            Err(e) => SubtreeWriterResult {
                bytes: vec![],
                errors: vec![e.to_string()],
                warnings: vec![],
            },
        }
    }

    /// Serialize a subtree to the binary `.subtree` envelope.
    ///
    /// `buffer_data` is the inline binary payload appended after the JSON
    /// section. The first [`Buffer`] in `subtree.buffers` must have no `uri`
    /// (it refers to this inline chunk); further buffers may be external.
    pub fn write_subtree_binary(
        subtree: &Subtree,
        buffer_data: &[u8],
        opts: WriteOptions,
    ) -> SubtreeWriterResult {
        let json_bytes = match serialize(subtree, opts.pretty_print) {
            Ok(b) => b,
            Err(e) => {
                return SubtreeWriterResult {
                    bytes: vec![],
                    errors: vec![e.to_string()],
                    warnings: vec![],
                };
            }
        };

        // Pad JSON to 8-byte alignment as required by the spec.
        let json_padded_len = (json_bytes.len() + 7) & !7;
        let json_padding = json_padded_len - json_bytes.len();
        let binary_len = buffer_data.len();

        let mut out = Vec::with_capacity(24 + json_padded_len + binary_len);
        out.extend_from_slice(b"subt");
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&(json_padded_len as u64).to_le_bytes());
        out.extend_from_slice(&(binary_len as u64).to_le_bytes());
        out.extend_from_slice(&json_bytes);
        // Pad with spaces (0x20) to keep JSON section valid UTF-8.
        out.extend(std::iter::repeat(0x20u8).take(json_padding));
        out.extend_from_slice(buffer_data);

        SubtreeWriterResult {
            bytes: out,
            errors: vec![],
            warnings: vec![],
        }
    }

    /// Build a [`Buffer`] descriptor for the inline binary chunk.
    ///
    /// The returned buffer has no `uri` (inline reference) and `byte_length`
    /// matching the provided data slice. Attach it as `subtree.buffers[0]`
    /// before calling [`write_subtree_binary`](Self::write_subtree_binary).
    pub fn inline_buffer(data: &[u8]) -> Buffer {
        Buffer {
            byte_length: data.len(),
            name: None,
            uri: None,
            data: data.to_vec(),
            ..Default::default()
        }
    }
}

// ---------------------------------------------------------------------------
// SchemaWriter
// ---------------------------------------------------------------------------

/// Serializes a [`Schema`] (3D Tiles metadata schema) to JSON bytes.
pub struct SchemaWriter;

impl SchemaWriter {
    /// Serialize a schema to JSON.
    pub fn write_schema(schema: &Schema, opts: WriteOptions) -> SchemaWriterResult {
        match serialize(schema, opts.pretty_print) {
            Ok(bytes) => SchemaWriterResult {
                bytes,
                errors: vec![],
                warnings: vec![],
            },
            Err(e) => SchemaWriterResult {
                bytes: vec![],
                errors: vec![e.to_string()],
                warnings: vec![],
            },
        }
    }
}

fn serialize<T: serde::Serialize>(value: &T, pretty: bool) -> Result<Vec<u8>, serde_json::Error> {
    if pretty {
        serde_json::to_vec_pretty(value)
    } else {
        serde_json::to_vec(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::{Asset, Availability, BoundingVolume, Class, Refine, Tile};
    use crate::reader::TilesetReader;
    use std::collections::HashMap;

    fn make_tileset() -> Tileset {
        Tileset {
            asset: Asset {
                version: "1.1".into(),
                ..Default::default()
            },
            geometric_error: 500.0,
            root: Tile {
                bounding_volume: BoundingVolume {
                    sphere: vec![0.0, 0.0, 0.0, 1000.0],
                    ..Default::default()
                },
                geometric_error: 500.0,
                refine: Some(Refine::Replace),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn tileset_round_trip() {
        let ts = make_tileset();
        let r = TilesetWriter::write_tileset(&ts, WriteOptions::default());
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        let result = TilesetReader::read_from_slice(&r.bytes);
        assert!(result.ok(), "errors: {:?}", result.errors);
        let ts2 = result.tileset.unwrap();
        assert_eq!(ts2.asset.version, "1.1");
        assert_eq!(ts2.geometric_error, 500.0);
        assert_eq!(ts2.root.refine, Some(Refine::Replace));
    }

    #[test]
    fn tileset_pretty_print() {
        let ts = make_tileset();
        let r = TilesetWriter::write_tileset(&ts, WriteOptions { pretty_print: true });
        assert!(r.errors.is_empty());
        assert!(r.bytes.contains(&b'\n'));
    }

    #[test]
    fn subtree_json_round_trip() {
        let subtree = Subtree {
            tile_availability: Availability {
                constant: Some(1),
                ..Default::default()
            },
            child_subtree_availability: Availability {
                constant: Some(0),
                ..Default::default()
            },
            ..Default::default()
        };
        let r = SubtreeWriter::write_subtree_json(&subtree, WriteOptions::default());
        assert!(r.errors.is_empty());
        let parsed: Subtree = serde_json::from_slice(&r.bytes).unwrap();
        assert_eq!(parsed.tile_availability.constant, Some(1));
        assert_eq!(parsed.child_subtree_availability.constant, Some(0));
    }

    #[test]
    fn subtree_binary_envelope_round_trip() {
        let subtree = Subtree {
            tile_availability: Availability {
                constant: Some(1),
                ..Default::default()
            },
            child_subtree_availability: Availability {
                constant: Some(0),
                ..Default::default()
            },
            ..Default::default()
        };
        let payload = vec![0xAAu8, 0xBB, 0xCC, 0xDD];
        let r = SubtreeWriter::write_subtree_binary(&subtree, &payload, WriteOptions::default());
        assert!(r.errors.is_empty());
        assert_eq!(&r.bytes[0..4], b"subt");
        assert_eq!(u32::from_le_bytes(r.bytes[4..8].try_into().unwrap()), 1);
        use crate::generated::SubdivisionScheme;
        let av = crate::subtree::parse_subtree(&r.bytes, SubdivisionScheme::Quadtree, 2)
            .expect("should parse");
        assert!(av.is_tile_available(0, 0));
    }

    #[test]
    fn schema_round_trip() {
        let schema = Schema {
            id: "test-schema".into(),
            classes: {
                let mut m = HashMap::new();
                m.insert(
                    "Building".into(),
                    Class {
                        name: Some("Building".into()),
                        ..Default::default()
                    },
                );
                m
            },
            ..Default::default()
        };
        let r = SchemaWriter::write_schema(&schema, WriteOptions::default());
        assert!(r.errors.is_empty());
        let parsed: Schema = serde_json::from_slice(&r.bytes).unwrap();
        assert_eq!(parsed.id, "test-schema");
        assert!(parsed.classes.contains_key("Building"));
    }
}
