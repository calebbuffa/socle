//! Binary attribute buffer parsing for I3S.
//!
//! Each attribute field is stored as a separate binary resource. The layout
//! varies by type:
//!
//! - **String**: `count(u32)`, `totalBytes(u32)`, `stringSizes(u32 × count)`, `stringData(UTF-8, null-terminated)`
//! - **Double**: `count(u32)`, `padding(4 bytes)`, `values(f64 × count)`
//! - **Integer (32-bit)**: `count(u32)`, `values(i32 × count)` or `(u32 × count)`
//! - **Short (16-bit)**: `count(u32)`, `values(u16 × count)`
//!
//! All values are little-endian. Values are in the same order as features
//! in the geometry buffer (direct array indexing).

use byteorder::{LittleEndian, ReadBytesExt};
use i3s_util::{I3SError, Result};
use std::io::{Cursor, Read};

/// Decoded attribute data for a single field.
#[derive(Debug, Clone)]
pub enum AttributeData {
    Strings(Vec<String>),
    Int32(Vec<i32>),
    Uint32(Vec<u32>),
    Uint16(Vec<u16>),
    Float64(Vec<f64>),
    Float32(Vec<f32>),
}

impl AttributeData {
    /// Number of attribute values.
    pub fn len(&self) -> usize {
        match self {
            Self::Strings(v) => v.len(),
            Self::Int32(v) => v.len(),
            Self::Uint32(v) => v.len(),
            Self::Uint16(v) => v.len(),
            Self::Float64(v) => v.len(),
            Self::Float32(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Approximate byte size of the attribute data in memory.
    pub fn byte_size(&self) -> usize {
        match self {
            Self::Strings(v) => {
                v.iter().map(|s| s.len()).sum::<usize>() + v.len() * std::mem::size_of::<String>()
            }
            Self::Int32(v) => v.len() * 4,
            Self::Uint32(v) => v.len() * 4,
            Self::Uint16(v) => v.len() * 2,
            Self::Float64(v) => v.len() * 8,
            Self::Float32(v) => v.len() * 4,
        }
    }
}

/// The value type of an attribute buffer, derived from `AttributeStorageInfo.attributeValues.valueType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeValueType {
    String,
    Int32,
    Uint32,
    Uint16,
    Float64,
    Float32,
}

impl AttributeValueType {
    /// Parse from the `valueType` string in the I3S spec.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "String" | "string" => Some(Self::String),
            "Int32" => Some(Self::Int32),
            "UInt32" | "Oid32" => Some(Self::Uint32),
            "UInt16" => Some(Self::Uint16),
            "Float64" => Some(Self::Float64),
            "Float32" => Some(Self::Float32),
            _ => None,
        }
    }
}

/// Parse a binary attribute buffer.
///
/// The `value_type` determines how to interpret the binary data.
///
/// # Errors
///
/// Returns [`I3SError::Buffer`] if the buffer is truncated or malformed.
pub fn parse_attribute_buffer(
    data: &[u8],
    value_type: AttributeValueType,
) -> Result<AttributeData> {
    match value_type {
        AttributeValueType::String => parse_string_attribute(data),
        AttributeValueType::Float64 => parse_f64_attribute(data),
        AttributeValueType::Float32 => parse_f32_attribute(data),
        AttributeValueType::Int32 => parse_i32_attribute(data),
        AttributeValueType::Uint32 => parse_u32_attribute(data),
        AttributeValueType::Uint16 => parse_u16_attribute(data),
    }
}

/// Parse a string attribute buffer.
///
/// Layout: `count(u32)`, `totalBytes(u32)`, `stringSizes(u32 × count)`, `stringData(null-terminated UTF-8)`
fn parse_string_attribute(data: &[u8]) -> Result<AttributeData> {
    let mut cursor = Cursor::new(data);

    let count = cursor
        .read_u32::<LittleEndian>()
        .map_err(|e| I3SError::Buffer(format!("string attr count: {e}")))? as usize;

    let _total_bytes = cursor
        .read_u32::<LittleEndian>()
        .map_err(|e| I3SError::Buffer(format!("string attr totalBytes: {e}")))?;

    // Read string byte lengths (including null terminator)
    let mut sizes = Vec::with_capacity(count);
    for _ in 0..count {
        let s = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| I3SError::Buffer(format!("string attr size: {e}")))?;
        sizes.push(s as usize);
    }

    // Read string data
    let mut strings = Vec::with_capacity(count);
    for (i, &size) in sizes.iter().enumerate() {
        if size == 0 {
            strings.push(String::new());
            continue;
        }
        let mut bytes = vec![0u8; size];
        cursor
            .read_exact(&mut bytes)
            .map_err(|e| I3SError::Buffer(format!("string attr data[{i}]: {e}")))?;
        // Remove null terminator if present
        if bytes.last() == Some(&0) {
            bytes.pop();
        }
        let s = String::from_utf8(bytes)
            .map_err(|e| I3SError::Buffer(format!("string attr UTF-8[{i}]: {e}")))?;
        strings.push(s);
    }

    Ok(AttributeData::Strings(strings))
}

/// Parse a Float64 attribute buffer.
///
/// Layout: `count(u32)`, `padding(4 bytes for 8-byte alignment)`, `values(f64 × count)`
fn parse_f64_attribute(data: &[u8]) -> Result<AttributeData> {
    let mut cursor = Cursor::new(data);

    let count = cursor
        .read_u32::<LittleEndian>()
        .map_err(|e| I3SError::Buffer(format!("f64 attr count: {e}")))? as usize;

    // 4 bytes padding for 8-byte alignment
    let _padding = cursor
        .read_u32::<LittleEndian>()
        .map_err(|e| I3SError::Buffer(format!("f64 attr padding: {e}")))?;

    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        let v = cursor
            .read_f64::<LittleEndian>()
            .map_err(|e| I3SError::Buffer(format!("f64 attr value: {e}")))?;
        values.push(v);
    }

    Ok(AttributeData::Float64(values))
}

/// Parse a Float32 attribute buffer.
///
/// Layout: `count(u32)`, `values(f32 × count)`
fn parse_f32_attribute(data: &[u8]) -> Result<AttributeData> {
    let mut cursor = Cursor::new(data);

    let count = cursor
        .read_u32::<LittleEndian>()
        .map_err(|e| I3SError::Buffer(format!("f32 attr count: {e}")))? as usize;

    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        let v = cursor
            .read_f32::<LittleEndian>()
            .map_err(|e| I3SError::Buffer(format!("f32 attr value: {e}")))?;
        values.push(v);
    }

    Ok(AttributeData::Float32(values))
}

/// Parse an Int32 attribute buffer.
///
/// Layout: `count(u32)`, `values(i32 × count)`
fn parse_i32_attribute(data: &[u8]) -> Result<AttributeData> {
    let mut cursor = Cursor::new(data);

    let count = cursor
        .read_u32::<LittleEndian>()
        .map_err(|e| I3SError::Buffer(format!("i32 attr count: {e}")))? as usize;

    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        let v = cursor
            .read_i32::<LittleEndian>()
            .map_err(|e| I3SError::Buffer(format!("i32 attr value: {e}")))?;
        values.push(v);
    }

    Ok(AttributeData::Int32(values))
}

/// Parse a UInt32 attribute buffer.
///
/// Layout: `count(u32)`, `values(u32 × count)`
fn parse_u32_attribute(data: &[u8]) -> Result<AttributeData> {
    let mut cursor = Cursor::new(data);

    let count = cursor
        .read_u32::<LittleEndian>()
        .map_err(|e| I3SError::Buffer(format!("u32 attr count: {e}")))? as usize;

    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        let v = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| I3SError::Buffer(format!("u32 attr value: {e}")))?;
        values.push(v);
    }

    Ok(AttributeData::Uint32(values))
}

/// Parse a UInt16 attribute buffer.
///
/// Layout: `count(u32)`, `values(u16 × count)`
fn parse_u16_attribute(data: &[u8]) -> Result<AttributeData> {
    let mut cursor = Cursor::new(data);

    let count = cursor
        .read_u32::<LittleEndian>()
        .map_err(|e| I3SError::Buffer(format!("u16 attr count: {e}")))? as usize;

    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        let v = cursor
            .read_u16::<LittleEndian>()
            .map_err(|e| I3SError::Buffer(format!("u16 attr value: {e}")))?;
        values.push(v);
    }

    Ok(AttributeData::Uint16(values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use byteorder::WriteBytesExt;

    #[test]
    fn parse_string_buffer() {
        let mut buf = Vec::new();
        buf.write_u32::<LittleEndian>(2).unwrap(); // count
        buf.write_u32::<LittleEndian>(12).unwrap(); // totalBytes

        // String sizes (including null terminator)
        buf.write_u32::<LittleEndian>(6).unwrap(); // "hello\0"
        buf.write_u32::<LittleEndian>(6).unwrap(); // "world\0"

        // String data
        buf.extend_from_slice(b"hello\0");
        buf.extend_from_slice(b"world\0");

        let result = parse_attribute_buffer(&buf, AttributeValueType::String).unwrap();
        match result {
            AttributeData::Strings(v) => {
                assert_eq!(v.len(), 2);
                assert_eq!(v[0], "hello");
                assert_eq!(v[1], "world");
            }
            _ => panic!("expected Strings"),
        }
    }

    #[test]
    fn parse_f64_buffer() {
        let mut buf = Vec::new();
        buf.write_u32::<LittleEndian>(3).unwrap(); // count
        buf.write_u32::<LittleEndian>(0).unwrap(); // padding
        buf.write_f64::<LittleEndian>(1.5).unwrap();
        buf.write_f64::<LittleEndian>(2.5).unwrap();
        buf.write_f64::<LittleEndian>(3.5).unwrap();

        let result = parse_attribute_buffer(&buf, AttributeValueType::Float64).unwrap();
        match result {
            AttributeData::Float64(v) => {
                assert_eq!(v, vec![1.5, 2.5, 3.5]);
            }
            _ => panic!("expected Float64"),
        }
    }

    #[test]
    fn parse_i32_buffer() {
        let mut buf = Vec::new();
        buf.write_u32::<LittleEndian>(2).unwrap(); // count
        buf.write_i32::<LittleEndian>(-10).unwrap();
        buf.write_i32::<LittleEndian>(20).unwrap();

        let result = parse_attribute_buffer(&buf, AttributeValueType::Int32).unwrap();
        match result {
            AttributeData::Int32(v) => {
                assert_eq!(v, vec![-10, 20]);
            }
            _ => panic!("expected Int32"),
        }
    }

    #[test]
    fn parse_u32_buffer() {
        let mut buf = Vec::new();
        buf.write_u32::<LittleEndian>(2).unwrap(); // count
        buf.write_u32::<LittleEndian>(100).unwrap();
        buf.write_u32::<LittleEndian>(200).unwrap();

        let result = parse_attribute_buffer(&buf, AttributeValueType::Uint32).unwrap();
        match result {
            AttributeData::Uint32(v) => {
                assert_eq!(v, vec![100, 200]);
            }
            _ => panic!("expected Uint32"),
        }
    }

    #[test]
    fn parse_u16_buffer() {
        let mut buf = Vec::new();
        buf.write_u32::<LittleEndian>(3).unwrap(); // count
        buf.write_u16::<LittleEndian>(1).unwrap();
        buf.write_u16::<LittleEndian>(2).unwrap();
        buf.write_u16::<LittleEndian>(3).unwrap();

        let result = parse_attribute_buffer(&buf, AttributeValueType::Uint16).unwrap();
        match result {
            AttributeData::Uint16(v) => {
                assert_eq!(v, vec![1, 2, 3]);
            }
            _ => panic!("expected Uint16"),
        }
    }

    #[test]
    fn parse_empty_string_buffer() {
        let mut buf = Vec::new();
        buf.write_u32::<LittleEndian>(0).unwrap(); // count = 0
        buf.write_u32::<LittleEndian>(0).unwrap(); // totalBytes = 0

        let result = parse_attribute_buffer(&buf, AttributeValueType::String).unwrap();
        match result {
            AttributeData::Strings(v) => assert!(v.is_empty()),
            _ => panic!("expected Strings"),
        }
    }

    #[test]
    fn truncated_attribute_error() {
        let buf = vec![0u8; 2]; // too short for count
        let result = parse_attribute_buffer(&buf, AttributeValueType::Int32);
        assert!(result.is_err());
    }
}
