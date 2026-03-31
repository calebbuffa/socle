//! `data:` URI decoding for buffer and image URIs.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use moderu::GltfModel;

/// Decode all `data:` URIs in buffers and images, replacing the URI with
/// decoded binary data. Optionally clears the URI string after decoding.
pub fn decode_data_urls(model: &mut GltfModel, clear: bool, warnings: &mut super::error::Warnings) {
    for buffer in model.buffers.iter_mut() {
        let Some(uri) = buffer.uri.as_deref() else {
            continue;
        };
        if !uri.starts_with("data:") {
            continue;
        }

        match decode_data_uri(uri) {
            Ok(data) => {
                buffer.data = data;
                if clear {
                    buffer.uri = None;
                }
            }
            Err(e) => {
                warnings.push(super::error::Warning(format!(
                    "buffer data URI decode failed: {e}"
                )));
            }
        }
    }

    for (i, image) in model.images.iter_mut().enumerate() {
        let Some(uri) = image.uri.as_deref() else {
            continue;
        };
        if !uri.starts_with("data:") {
            continue;
        }

        match decode_data_uri(uri) {
            Ok(data) => {
                // Store decoded bytes in a new buffer + bufferView so the
                // image decode pass can find it via buffer_view.
                let buf_idx = model.buffers.len();
                let bv_idx = model.buffer_views.len();
                let byte_len = data.len();

                model.buffers.push(moderu::Buffer {
                    data,
                    byte_length: byte_len,
                    ..Default::default()
                });

                model.buffer_views.push(moderu::BufferView {
                    buffer: buf_idx,
                    byte_length: byte_len,
                    ..Default::default()
                });

                image.buffer_view = Some(bv_idx);
                if clear {
                    image.uri = None;
                }
            }
            Err(e) => {
                warnings.push(super::error::Warning(format!(
                    "image[{i}] data URI decode failed: {e}"
                )));
            }
        }
    }
}

/// Decode a single `data:` URI.
///
/// Format: `data:[<mime>][;base64],<data>`
fn decode_data_uri(uri: &str) -> Result<Vec<u8>, String> {
    let rest = uri.strip_prefix("data:").ok_or("not a data: URI")?;

    let comma_pos = rest.find(',').ok_or("data: URI missing comma separator")?;

    let meta = &rest[..comma_pos];
    let encoded = &rest[comma_pos + 1..];

    if meta.ends_with(";base64") {
        STANDARD
            .decode(encoded)
            .map_err(|e| format!("base64 decode: {e}"))
    } else {
        // Percent-encoded raw data (unlikely for binary, but spec-compliant).
        Ok(percent_decode(encoded))
    }
}

fn percent_decode(input: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_digit(bytes[i + 1]), hex_digit(bytes[i + 2])) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

#[inline]
fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
