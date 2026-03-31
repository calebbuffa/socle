//! GLB binary container parser.
//!
//! Reference: <https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#glb-file-format-specification>

use super::error::GltfError;
use moderu::GltfModel;

const GLB_MAGIC: u32 = 0x46546C67; // "glTF" in little-endian
const GLB_VERSION: u32 = 2;
const CHUNK_TYPE_JSON: u32 = 0x4E4F534A; // "JSON"
const CHUNK_TYPE_BIN: u32 = 0x004E4942; // "BIN\0"
const GLB_HEADER_SIZE: usize = 12;
const CHUNK_HEADER_SIZE: usize = 8;

/// Returns `true` if the first 4 bytes match the GLB magic number.
pub fn is_glb(data: &[u8]) -> bool {
    data.len() >= 4 && read_u32(data, 0) == GLB_MAGIC
}

/// Parse a GLB container into a `GltfModel` and an optional BIN chunk slice.
///
/// The returned `&[u8]` borrows directly from `data` — zero-copy for the
/// binary chunk.
pub fn parse_glb(data: &[u8]) -> Result<(GltfModel, Option<&[u8]>), GltfError> {
    if data.len() < GLB_HEADER_SIZE {
        return Err(GltfError::InvalidGlb(
            "file too short for GLB header".into(),
        ));
    }

    let magic = read_u32(data, 0);
    if magic != GLB_MAGIC {
        return Err(GltfError::InvalidGlb(format!(
            "bad magic: expected 0x{GLB_MAGIC:08X}, got 0x{magic:08X}"
        )));
    }

    let version = read_u32(data, 4);
    if version != GLB_VERSION {
        return Err(GltfError::InvalidGlb(format!(
            "unsupported version {version}, expected {GLB_VERSION}"
        )));
    }

    let total_length = read_u32(data, 8) as usize;
    if total_length > data.len() {
        return Err(GltfError::InvalidGlb(format!(
            "header length {total_length} exceeds data size {}",
            data.len()
        )));
    }

    // Parse JSON chunk (required, must be first)
    let json_offset = GLB_HEADER_SIZE;
    if json_offset + CHUNK_HEADER_SIZE > total_length {
        return Err(GltfError::InvalidGlb("missing JSON chunk header".into()));
    }

    let json_chunk_len = read_u32(data, json_offset) as usize;
    let json_chunk_type = read_u32(data, json_offset + 4);

    if json_chunk_type != CHUNK_TYPE_JSON {
        return Err(GltfError::InvalidGlb(format!(
            "first chunk type 0x{json_chunk_type:08X} is not JSON (0x{CHUNK_TYPE_JSON:08X})"
        )));
    }

    let json_data_start = json_offset + CHUNK_HEADER_SIZE;
    let json_data_end = json_data_start + json_chunk_len;
    if json_data_end > total_length {
        return Err(GltfError::InvalidGlb(
            "JSON chunk exceeds file length".into(),
        ));
    }

    let json_bytes = &data[json_data_start..json_data_end];
    let model: GltfModel = serde_json::from_slice(json_bytes)?;

    // Parse BIN chunk (optional, must be second)
    let bin_offset = json_data_end;
    let bin_chunk = if bin_offset + CHUNK_HEADER_SIZE <= total_length {
        let bin_chunk_len = read_u32(data, bin_offset) as usize;
        let bin_chunk_type = read_u32(data, bin_offset + 4);

        if bin_chunk_type == CHUNK_TYPE_BIN {
            let bin_data_start = bin_offset + CHUNK_HEADER_SIZE;
            let bin_data_end = bin_data_start + bin_chunk_len;
            if bin_data_end > total_length {
                return Err(GltfError::InvalidGlb(
                    "BIN chunk exceeds file length".into(),
                ));
            }
            Some(&data[bin_data_start..bin_data_end])
        } else {
            None
        }
    } else {
        None
    };

    Ok((model, bin_chunk))
}

#[inline(always)]
fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_glb(json: &[u8], bin: Option<&[u8]>) -> Vec<u8> {
        let json_padded_len = (json.len() + 3) & !3; // align to 4 bytes
        let bin_section_len = bin.map_or(0, |b| CHUNK_HEADER_SIZE + ((b.len() + 3) & !3));
        let total = GLB_HEADER_SIZE + CHUNK_HEADER_SIZE + json_padded_len + bin_section_len;

        let mut out = Vec::with_capacity(total);

        // Header
        out.extend_from_slice(&GLB_MAGIC.to_le_bytes());
        out.extend_from_slice(&GLB_VERSION.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());

        // JSON chunk
        out.extend_from_slice(&(json_padded_len as u32).to_le_bytes());
        out.extend_from_slice(&CHUNK_TYPE_JSON.to_le_bytes());
        out.extend_from_slice(json);
        // pad with spaces (per spec)
        out.resize(GLB_HEADER_SIZE + CHUNK_HEADER_SIZE + json_padded_len, b' ');

        // BIN chunk
        if let Some(b) = bin {
            let bin_padded_len = (b.len() + 3) & !3;
            out.extend_from_slice(&(bin_padded_len as u32).to_le_bytes());
            out.extend_from_slice(&CHUNK_TYPE_BIN.to_le_bytes());
            out.extend_from_slice(b);
            out.resize(total, 0); // pad with zeros
        }

        out
    }

    #[test]
    fn parse_minimal_glb() {
        let json = br#"{"asset":{"version":"2.0"}}"#;
        let glb = make_glb(json, None);

        assert!(is_glb(&glb));
        let (model, bin) = parse_glb(&glb).unwrap();
        assert_eq!(model.asset.version, "2.0");
        assert!(bin.is_none());
    }

    #[test]
    fn parse_glb_with_bin_chunk() {
        let json = br#"{"asset":{"version":"2.0"},"buffers":[{"byteLength":8}]}"#;
        let bin_data = &[1u8, 2, 3, 4, 5, 6, 7, 8];
        let glb = make_glb(json, Some(bin_data));

        let (model, bin) = parse_glb(&glb).unwrap();
        assert_eq!(model.buffers.len(), 1);
        assert_eq!(model.buffers[0].byte_length, 8);
        let bin = bin.unwrap();
        assert_eq!(&bin[..8], bin_data);
    }

    #[test]
    fn bad_magic_is_rejected() {
        let mut data = vec![0u8; 20];
        data[0..4].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
        assert!(!is_glb(&data));
        assert!(parse_glb(&data).is_err());
    }

    #[test]
    fn wrong_version_rejected() {
        let json = br#"{"asset":{"version":"2.0"}}"#;
        let mut glb = make_glb(json, None);
        // Overwrite version field to 1
        glb[4..8].copy_from_slice(&1u32.to_le_bytes());
        assert!(parse_glb(&glb).is_err());
    }
}
