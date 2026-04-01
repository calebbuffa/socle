//! Parser for 3D Tiles 1.1 `.subtree` files.
//!
//! Supports both the **binary** envelope (magic `subt`) used by most servers
//! and bare-**JSON** subtrees served without a header.
//!
//! # Binary format layout
//!
//! ```text
//!  0..4   magic    = b"subt"
//!  4..8   version  = 1  (u32 LE)
//!  8..16  json_byte_length  (u64 LE)
//! 16..24  binary_byte_length (u64 LE)
//! 24..    JSON UTF-8
//!         binary blob (buffers referenced by bufferViews)
//! ```
//!
//! The JSON payload describes three availability bitfields:
//! - `tileAvailability` — which tiles in the subtree exist
//! - `contentAvailability` — which tiles have loadable content
//! - `childSubtreeAvailability` — which leaf positions have child subtrees
//!
//! Each field is either `{ "constant": 0|1 }` (all-off / all-on) or
//! `{ "bitstream": N }` (index into `bufferViews`).

use std::collections::HashMap;

use crate::availability::{AvailabilityView, SubtreeAvailability};
use crate::generated::{Availability, Buffer, BufferView, SubdivisionScheme, Subtree};

/// Error produced while parsing a `.subtree` file.
#[derive(Debug, thiserror::Error)]
pub enum SubtreeParseError {
    /// The data was truncated before the end of the declared content.
    #[error("subtree data truncated")]
    TooShort,
    /// The 4-byte magic does not match `subt`.
    #[error("subtree: invalid magic (expected 'subt')")]
    BadMagic,
    /// Version field is not 1.
    #[error("subtree: unsupported version {0}")]
    BadVersion(u32),
    /// The JSON section could not be parsed.
    #[error("subtree JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// A `bufferView` index in the JSON is out of range.
    #[error("subtree: bufferView index {index} out of range")]
    InvalidBufferView { index: usize },
    /// A buffer view's byte range exceeds the binary blob.
    #[error("subtree: buffer view range exceeds binary payload")]
    BufferOutOfRange,
    /// A `buffer.uri` was referenced but not supplied in `external_buffers`.
    #[error("subtree: external buffer URI '{uri}' not provided")]
    MissingExternalBuffer { uri: String },
    /// A `buffer` index in a `bufferView` is out of range.
    #[error("subtree: buffer index {index} out of range")]
    InvalidBufferIndex { index: usize },
}

/// Parse a raw `.subtree` response body into a [`SubtreeAvailability`].
///
/// For subtrees whose `Buffer` entries reference external URIs via `buffer.uri`,
/// pre-fetch those URIs and pass them via [`parse_subtree_with_buffers`].
/// This variant assumes all buffer data is inline (embedded in the binary envelope).
pub fn parse_subtree(
    data: &[u8],
    scheme: SubdivisionScheme,
    subtree_levels: u32,
) -> Result<SubtreeAvailability, SubtreeParseError> {
    parse_subtree_with_buffers(data, &HashMap::new(), scheme, subtree_levels)
}

/// Like [`parse_subtree`] but accepts pre-fetched external buffer data.
///
/// `external_buffers` maps each `buffer.uri` value that appears in the subtree
/// JSON to its raw bytes. Any referenced URI absent from the map causes a
/// [`SubtreeParseError::MissingExternalBuffer`] error.
pub fn parse_subtree_with_buffers(
    data: &[u8],
    external_buffers: &HashMap<String, Vec<u8>>,
    scheme: SubdivisionScheme,
    subtree_levels: u32,
) -> Result<SubtreeAvailability, SubtreeParseError> {
    let (json_bytes, inline_binary) = split_envelope(data)?;
    let json: Subtree = serde_json::from_slice(json_bytes).map_err(SubtreeParseError::Json)?;
    build_availability(
        &json,
        inline_binary,
        external_buffers,
        scheme,
        subtree_levels,
    )
}

/// Split `data` into *(json_bytes, binary_blob)*.
///
/// If the first four bytes are `subt` the binary header is consumed;
/// otherwise the whole slice is treated as JSON with an empty binary blob.
fn split_envelope(data: &[u8]) -> Result<(&[u8], &[u8]), SubtreeParseError> {
    const MAGIC: &[u8; 4] = b"subt";
    const HEADER: usize = 24; // 4 magic + 4 version + 8 json_len + 8 bin_len

    if data.len() >= 4 && data[..4] == *MAGIC {
        if data.len() < HEADER {
            return Err(SubtreeParseError::TooShort);
        }
        let version = u32::from_le_bytes(
            data[4..8].try_into().map_err(|_| SubtreeParseError::TooShort)?,
        );
        if version != 1 {
            return Err(SubtreeParseError::BadVersion(version));
        }
        let json_len = u64::from_le_bytes(
            data[8..16].try_into().map_err(|_| SubtreeParseError::TooShort)?,
        ) as usize;
        let bin_len = u64::from_le_bytes(
            data[16..24].try_into().map_err(|_| SubtreeParseError::TooShort)?,
        ) as usize;

        let json_start = HEADER;
        let json_end = json_start.saturating_add(json_len);
        let bin_end = json_end.saturating_add(bin_len);

        if data.len() < bin_end {
            return Err(SubtreeParseError::TooShort);
        }
        Ok((&data[json_start..json_end], &data[json_end..bin_end]))
    } else {
        // Plain JSON — no binary blob.
        Ok((data, &[]))
    }
}

/// Resolve an [`Availability`] to an [`AvailabilityView`].
///
/// Bitstream specs copy bytes from either the inline binary blob (buffer
/// with no `uri`) or a pre-fetched external buffer (buffer with `uri`).
fn resolve_spec(
    spec: &Availability,
    buffer_views: &[BufferView],
    buffers: &[Buffer],
    inline_binary: &[u8],
    external_buffers: &HashMap<String, Vec<u8>>,
) -> Result<AvailabilityView, SubtreeParseError> {
    if let Some(c) = spec.constant {
        return Ok(AvailabilityView::Constant(c != 0));
    }
    if let Some(bv_idx) = spec.bitstream {
        let bv = buffer_views
            .get(bv_idx)
            .ok_or(SubtreeParseError::InvalidBufferView { index: bv_idx })?;

        // Resolve the buffer this view belongs to.
        let buffer_data: &[u8] = if buffers.is_empty() {
            // Legacy subtrees with no explicit buffers array — use inline blob.
            inline_binary
        } else {
            let buf = buffers
                .get(bv.buffer)
                .ok_or(SubtreeParseError::InvalidBufferIndex { index: bv.buffer })?;
            if let Some(uri) = &buf.uri {
                external_buffers
                    .get(uri.as_str())
                    .map(Vec::as_slice)
                    .ok_or_else(|| SubtreeParseError::MissingExternalBuffer { uri: uri.clone() })?
            } else {
                inline_binary
            }
        };

        let end = bv.byte_offset.saturating_add(bv.byte_length);
        if end > buffer_data.len() {
            return Err(SubtreeParseError::BufferOutOfRange);
        }
        return Ok(AvailabilityView::Bitstream(
            buffer_data[bv.byte_offset..end].to_vec(),
        ));
    }
    // Neither constant nor bitstream — treat as all-unavailable.
    Ok(AvailabilityView::Constant(false))
}

/// Convert the parsed JSON into a [`SubtreeAvailability`].
fn build_availability(
    json: &Subtree,
    inline_binary: &[u8],
    external_buffers: &HashMap<String, Vec<u8>>,
    scheme: SubdivisionScheme,
    subtree_levels: u32,
) -> Result<SubtreeAvailability, SubtreeParseError> {
    let resolve = |spec: &Availability| {
        resolve_spec(
            spec,
            &json.buffer_views,
            &json.buffers,
            inline_binary,
            external_buffers,
        )
    };

    let tile_av = resolve(&json.tile_availability)?;
    let child_subtree_av = resolve(&json.child_subtree_availability)?;

    let content_av: Result<Vec<AvailabilityView>, _> = if json.content_availability.is_empty() {
        // Spec says contentAvailability may be omitted — default to all-unavailable.
        Ok(vec![AvailabilityView::Constant(false)])
    } else {
        json.content_availability.iter().map(resolve).collect()
    };
    let content_av = content_av?;

    // SubtreeAvailability::new returns None only when content_av is empty,
    // which cannot happen here (we always supply at least one entry).
    Ok(SubtreeAvailability::new(
        scheme,
        subtree_levels,
        tile_av,
        child_subtree_av,
        content_av,
    )
    .expect("content_av is never empty"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_available_json() -> Vec<u8> {
        br#"{
            "tileAvailability": { "constant": 1 },
            "contentAvailability": [{ "constant": 1 }],
            "childSubtreeAvailability": { "constant": 0 }
        }"#
        .to_vec()
    }

    fn wrap_binary(json: &[u8], binary: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"subt");
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&(json.len() as u64).to_le_bytes());
        out.extend_from_slice(&(binary.len() as u64).to_le_bytes());
        out.extend_from_slice(json);
        out.extend_from_slice(binary);
        out
    }

    #[test]
    fn plain_json_all_available() {
        let data = all_available_json();
        let sa = parse_subtree(&data, SubdivisionScheme::Quadtree, 2).unwrap();
        assert!(sa.is_tile_available(0, 0));
        for m in 0..4 {
            assert!(sa.is_tile_available(1, m));
        }
    }

    #[test]
    fn binary_envelope_all_available() {
        let json = all_available_json();
        let data = wrap_binary(&json, &[]);
        let sa = parse_subtree(&data, SubdivisionScheme::Quadtree, 2).unwrap();
        assert!(sa.is_tile_available(0, 0));
    }

    #[test]
    fn constant_unavailable() {
        let json = br#"{
            "tileAvailability": { "constant": 0 },
            "contentAvailability": [],
            "childSubtreeAvailability": { "constant": 0 }
        }"#;
        let sa = parse_subtree(json, SubdivisionScheme::Quadtree, 2).unwrap();
        assert!(!sa.is_tile_available(0, 0));
    }

    #[test]
    fn bitstream_partial_availability() {
        // Level-2 quadtree subtree: 1 + 4 + 16 = 21 tiles → 3 bytes.
        // Mark only the root (bit 0) available.
        let bits: Vec<u8> = vec![0b0000_0001, 0x00, 0x00];
        let json = format!(
            r#"{{
                "bufferViews": [{{"buffer":0,"byteOffset":0,"byteLength":{}}}],
                "tileAvailability": {{"bitstream":0}},
                "contentAvailability": [{{"constant":0}}],
                "childSubtreeAvailability": {{"constant":0}}
            }}"#,
            bits.len()
        );
        let data = wrap_binary(json.as_bytes(), &bits);
        let sa = parse_subtree(&data, SubdivisionScheme::Quadtree, 2).unwrap();
        assert!(sa.is_tile_available(0, 0), "root must be available");
        assert!(
            !sa.is_tile_available(1, 0),
            "level-1 tiles must be unavailable"
        );
    }

    #[test]
    fn truncated_binary_is_error() {
        let json = all_available_json();
        let mut data = wrap_binary(&json, &[]);
        data.truncate(10); // cut short
        assert!(matches!(
            parse_subtree(&data, SubdivisionScheme::Quadtree, 2),
            Err(SubtreeParseError::TooShort)
        ));
    }

    #[test]
    fn bad_version_is_error() {
        let json = all_available_json();
        let mut data = wrap_binary(&json, &[]);
        // Overwrite version field (bytes 4..8) with 2.
        data[4..8].copy_from_slice(&2u32.to_le_bytes());
        assert!(matches!(
            parse_subtree(&data, SubdivisionScheme::Quadtree, 2),
            Err(SubtreeParseError::BadVersion(2))
        ));
    }
}
