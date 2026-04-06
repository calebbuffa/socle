//! Zero-dependency GLB/glTF parsing and serialisation built directly into [`GltfModel`].
//!
//! Because these operations require only `serde_json` (already a dependency)
//! there is no need to pull in `moderu-io` just to load or save a file that
//! has no external references and no codec compression.
//!
//! For the full pipeline (Draco, meshopt, KTX2, image decoding, external URI
//! resolution, …) use `moderu_io::GltfReader` / `moderu_io::GltfWriter`.

use crate::GltfModel;

// ── Constants ─────────────────────────────────────────────────────────────────

const GLB_MAGIC: u32 = 0x46546C67; // "glTF" little-endian
const GLB_VERSION: u32 = 2;
const CHUNK_JSON: u32 = 0x4E4F534A; // "JSON"
const CHUNK_BIN: u32 = 0x004E4942; // "BIN\0"
const HEADER_LEN: usize = 12;
const CHUNK_HEADER_LEN: usize = 8;

// ── Error ─────────────────────────────────────────────────────────────────────

/// Error returned by [`GltfModel::from_glb`] and [`GltfModel::from_json`].
#[derive(Debug, thiserror::Error)]
pub enum GltfParseError {
    /// Binary container is malformed (too short, bad magic, etc.).
    #[error("invalid GLB: {0}")]
    InvalidGlb(String),
    /// JSON deserialisation failed.
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

// ── Parsing ───────────────────────────────────────────────────────────────────

impl GltfModel {
    /// Parse a **GLB** binary or **glTF JSON** byte slice.
    ///
    /// Detects the format automatically from the leading 4 bytes (GLB magic vs
    /// UTF-8 JSON).  For GLB, the optional BIN chunk is automatically wired
    /// into `buffers[0].data`.
    ///
    /// This covers self-contained GLB files. For files with external buffer or
    /// image URIs, or with codec compression (Draco / meshopt / …), use
    /// `moderu_io::GltfReader` instead.
    pub fn from_bytes(data: &[u8]) -> Result<Self, GltfParseError> {
        if data.get(..4) == Some(&GLB_MAGIC.to_le_bytes()) {
            Self::from_glb(data)
        } else {
            Self::from_json(data)
        }
    }

    /// Parse a **glTF JSON** byte slice.
    pub fn from_json(json: &[u8]) -> Result<Self, GltfParseError> {
        Ok(serde_json::from_slice(json)?)
    }

    /// Parse a **GLB** binary container.
    ///
    /// The BIN chunk, if present, is copied into `buffers[0].data`.
    pub fn from_glb(data: &[u8]) -> Result<Self, GltfParseError> {
        let (mut model, bin) = parse_glb(data)?;
        if let (Some(bin), Some(buf)) = (bin, model.buffers.first_mut()) {
            buf.data = bin.to_vec();
            buf.byte_length = buf.data.len();
        }
        Ok(model)
    }

    // ── Serialisation ─────────────────────────────────────────────────────────

    /// Serialise the model as a compact JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Serialise the model as pretty-printed JSON.
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Serialise the model as a self-contained **GLB** binary.
    ///
    /// `buffers[0].data` (if present) becomes the BIN chunk.  All integer
    /// buffer/bufferView byte lengths are recomputed from the live data.
    ///
    /// For codec-compressed output (Draco / meshopt / …) use
    /// `moderu_io::GltfWriter` instead.
    pub fn to_glb(&self) -> Vec<u8> {
        write_glb(self)
    }
}

// ── Internal GLB parse ────────────────────────────────────────────────────────

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    let b = data.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Parse a GLB container; returns the model and an optional BIN slice borrowing
/// from `data`.
fn parse_glb(data: &[u8]) -> Result<(GltfModel, Option<&[u8]>), GltfParseError> {
    macro_rules! bail {
        ($msg:expr) => {
            return Err(GltfParseError::InvalidGlb($msg.into()))
        };
    }

    if data.len() < HEADER_LEN {
        bail!("file too short for GLB header");
    }

    let magic = read_u32(data, 0).unwrap();
    if magic != GLB_MAGIC {
        bail!(format!(
            "bad magic: expected 0x{GLB_MAGIC:08X}, got 0x{magic:08X}"
        ));
    }
    let version = read_u32(data, 4).unwrap();
    if version != GLB_VERSION {
        bail!(format!(
            "unsupported GLB version {version}, expected {GLB_VERSION}"
        ));
    }
    let total_len = read_u32(data, 8).unwrap() as usize;
    if total_len > data.len() {
        bail!(format!(
            "header length {total_len} exceeds data size {}",
            data.len()
        ));
    }

    // JSON chunk (required, must be first).
    let json_off = HEADER_LEN;
    if json_off + CHUNK_HEADER_LEN > total_len {
        bail!("missing JSON chunk header");
    }
    let json_chunk_len = read_u32(data, json_off).unwrap() as usize;
    let json_chunk_type = read_u32(data, json_off + 4).unwrap();
    if json_chunk_type != CHUNK_JSON {
        bail!(format!(
            "first chunk type 0x{json_chunk_type:08X} is not JSON (0x{CHUNK_JSON:08X})"
        ));
    }
    let json_start = json_off + CHUNK_HEADER_LEN;
    let json_end = json_start + json_chunk_len;
    if json_end > total_len {
        bail!("JSON chunk exceeds file length");
    }

    let model: GltfModel = serde_json::from_slice(&data[json_start..json_end])?;

    // BIN chunk (optional, must be second).
    let bin_off = json_end;
    let bin_chunk = if bin_off + CHUNK_HEADER_LEN <= total_len {
        let bin_chunk_len = read_u32(data, bin_off).unwrap() as usize;
        let bin_chunk_type = read_u32(data, bin_off + 4).unwrap();
        if bin_chunk_type == CHUNK_BIN {
            let bin_start = bin_off + CHUNK_HEADER_LEN;
            let bin_end = bin_start + bin_chunk_len;
            if bin_end > total_len {
                return Err(GltfParseError::InvalidGlb(
                    "BIN chunk exceeds file length".into(),
                ));
            }
            Some(&data[bin_start..bin_end])
        } else {
            None
        }
    } else {
        None
    };

    Ok((model, bin_chunk))
}

// ── Internal GLB write ────────────────────────────────────────────────────────

fn align4(n: usize) -> usize {
    (n + 3) & !3
}

fn write_u32_le(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_glb(model: &GltfModel) -> Vec<u8> {
    // Serialise JSON.  Strip `buffers[0].data` from the JSON — it goes in the
    // BIN chunk, not inline.
    let json_bytes = serde_json::to_vec(model).unwrap_or_default();
    let json_padded = align4(json_bytes.len());

    // BIN chunk = buffers[0].data if non-empty.
    let bin_data: &[u8] = model
        .buffers
        .first()
        .map(|b| b.data.as_slice())
        .unwrap_or(&[]);
    let bin_padded = align4(bin_data.len());
    let has_bin = !bin_data.is_empty();

    let bin_section_len = if has_bin {
        CHUNK_HEADER_LEN + bin_padded
    } else {
        0
    };
    let total = HEADER_LEN + CHUNK_HEADER_LEN + json_padded + bin_section_len;

    let mut out = Vec::with_capacity(total);

    // File header.
    write_u32_le(&mut out, GLB_MAGIC);
    write_u32_le(&mut out, GLB_VERSION);
    write_u32_le(&mut out, total as u32);

    // JSON chunk.
    write_u32_le(&mut out, json_padded as u32);
    write_u32_le(&mut out, CHUNK_JSON);
    out.extend_from_slice(&json_bytes);
    out.resize(HEADER_LEN + CHUNK_HEADER_LEN + json_padded, b' '); // pad with spaces

    // BIN chunk.
    if has_bin {
        write_u32_le(&mut out, bin_padded as u32);
        write_u32_le(&mut out, CHUNK_BIN);
        out.extend_from_slice(bin_data);
        out.resize(total, 0); // pad with zeros
    }

    out
}
