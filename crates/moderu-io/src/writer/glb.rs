//! GLB (GL Transmission Format Binary) file writing.
//!
//! The GLB format consists of:
//! 1. 12-byte header (magic "glTF", version, file length)
//! 2. JSON chunk (type 0x4E4F534A "JSON")
//! 3. Binary data chunk (type 0x004E4942 "BIN\0")

use super::error::WriteResult;
use byteorder::{LittleEndian, WriteBytesExt};
use std::io::Write;

const GLB_MAGIC: u32 = 0x46546C67; // "glTF" in little-endian
const GLB_VERSION: u32 = 2;
const JSON_CHUNK_TYPE: u32 = 0x4E4F534A; // "JSON"
const BIN_CHUNK_TYPE: u32 = 0x004E4942; // "BIN\0"

/// GLB file header.
#[derive(Debug, Clone)]
pub struct GlbHeader {
    pub magic: u32,
    pub version: u32,
    pub length: u32,
}

impl GlbHeader {
    /// Create a new GLB header with the given file length.
    pub fn new(length: u32) -> Self {
        GlbHeader {
            magic: GLB_MAGIC,
            version: GLB_VERSION,
            length,
        }
    }

    /// Write the header to a writer.
    pub fn write<W: Write>(&self, writer: &mut W) -> WriteResult<()> {
        writer.write_u32::<LittleEndian>(self.magic)?;
        writer.write_u32::<LittleEndian>(self.version)?;
        writer.write_u32::<LittleEndian>(self.length)?;
        Ok(())
    }
}

/// GLB chunk header.
#[derive(Debug, Clone)]
pub struct GlbChunkHeader {
    pub length: u32,
    pub chunk_type: u32,
}

impl GlbChunkHeader {
    /// Create a new chunk header.
    pub fn new(length: u32, chunk_type: u32) -> Self {
        GlbChunkHeader { length, chunk_type }
    }

    /// Write the chunk header to a writer.
    pub fn write<W: Write>(&self, writer: &mut W) -> WriteResult<()> {
        writer.write_u32::<LittleEndian>(self.length)?;
        writer.write_u32::<LittleEndian>(self.chunk_type)?;
        Ok(())
    }
}

/// Write JSON chunk data (padded to `align`-byte boundary; GLB spec minimum is 4).
pub fn write_json_chunk<W: Write>(
    writer: &mut W,
    json_data: &[u8],
    align: usize,
) -> WriteResult<()> {
    let align = align.max(1);
    let padded_length = ((json_data.len() + align - 1) / align) * align;
    let padding = padded_length - json_data.len();

    let header = GlbChunkHeader::new(padded_length as u32, JSON_CHUNK_TYPE);
    header.write(writer)?;

    writer.write_all(json_data)?;

    // Pad with spaces (0x20 for JSON)
    for _ in 0..padding {
        writer.write_u8(0x20)?;
    }

    Ok(())
}

/// Write binary data chunk (padded to `align`-byte boundary; GLB spec minimum is 4).
pub fn write_bin_chunk<W: Write>(writer: &mut W, bin_data: &[u8], align: usize) -> WriteResult<()> {
    if bin_data.is_empty() {
        return Ok(());
    }

    let align = align.max(1);
    let padded_length = ((bin_data.len() + align - 1) / align) * align;
    let padding = padded_length - bin_data.len();

    let header = GlbChunkHeader::new(padded_length as u32, BIN_CHUNK_TYPE);
    header.write(writer)?;

    writer.write_all(bin_data)?;

    // Pad with zeros (0x00 for binary)
    for _ in 0..padding {
        writer.write_u8(0x00)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glb_header_write() {
        let header = GlbHeader::new(100);
        let mut buf = Vec::new();
        header.write(&mut buf).unwrap();
        assert_eq!(buf.len(), 12);
        assert_eq!(
            u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]),
            GLB_MAGIC
        );
    }

    #[test]
    fn test_chunk_header_write() {
        let header = GlbChunkHeader::new(50, JSON_CHUNK_TYPE);
        let mut buf = Vec::new();
        header.write(&mut buf).unwrap();
        assert_eq!(buf.len(), 8);
    }

    #[test]
    fn test_json_chunk_padding() {
        let mut buf = Vec::new();
        let json = b"{}";
        write_json_chunk(&mut buf, json, 4).unwrap();

        // 8 byte header + 4 padded bytes (2 json + 2 spaces)
        assert_eq!(buf.len(), 12);
    }
}
