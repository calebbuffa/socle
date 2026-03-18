//! `GunzipAssetAccessor` — decorator that transparently decompresses gzip
//! responses from a wrapped [`AssetAccessor`].
//!
//! Mirrors `CesiumAsync::GunzipAssetAccessor`. Because our [`AssetAccessor`]
//! is synchronous, decompression happens inline rather than in a worker thread.

use std::io::Read;
use std::sync::Arc;

use flate2::read::GzDecoder;
use i3s_util::{I3SError, Result};

use crate::accessor::AssetAccessor;
use crate::request::{AssetRequest, AssetResponse, Headers};

/// Returns `true` if `data` starts with the gzip magic bytes `[0x1F, 0x8B]`.
#[inline]
fn is_gzip(data: &[u8]) -> bool {
    data.starts_with(&[0x1f, 0x8b])
}

/// Decompress `data` using gzip. Returns the decompressed bytes.
fn gunzip(data: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = GzDecoder::new(data);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|e| I3SError::InvalidData(format!("gzip decompression failed: {e}")))?;
    Ok(out)
}

/// A decorator [`AssetAccessor`] that transparently decompresses gzip-encoded
/// responses returned by the wrapped inner accessor.
///
/// Both magic-byte detection (`\x1f\x8b` at the start of the body) and the
/// `Content-Encoding: gzip` response header are supported. The header is
/// stripped from the forwarded response so callers never see a compressed body.
///
/// # Example
/// ```rust,ignore
/// let accessor = GunzipAssetAccessor::new(RestAssetAccessor::new());
/// ```
pub struct GunzipAssetAccessor {
    inner: Arc<dyn AssetAccessor>,
}

impl GunzipAssetAccessor {
    /// Wrap any [`AssetAccessor`], automatically gunzipping its responses.
    pub fn new(inner: impl AssetAccessor + 'static) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }

    /// Same as [`new`](Self::new) but from a pre-shared `Arc`.
    pub fn from_arc(inner: Arc<dyn AssetAccessor>) -> Self {
        Self { inner }
    }
}

/// Decompress the response body if it is gzip-encoded, stripping the
/// `Content-Encoding` header so callers receive raw bytes.
fn maybe_gunzip(mut response: AssetResponse) -> Result<AssetResponse> {
    let is_gzip_header = response
        .headers
        .get("content-encoding")
        .map(|v| v.eq_ignore_ascii_case("gzip"))
        .unwrap_or(false);

    if is_gzip_header || is_gzip(&response.data) {
        response.data = gunzip(&response.data)?;
        // Strip the encoding header — body is now plain.
        response.headers.remove("content-encoding");
        response.headers.remove("Content-Encoding");
    }

    Ok(response)
}

impl AssetAccessor for GunzipAssetAccessor {
    fn get(&self, uri: &str) -> Result<AssetRequest> {
        let mut req = self.inner.get(uri)?;
        req.response = maybe_gunzip(req.response)?;
        Ok(req)
    }

    fn request(
        &self,
        verb: &str,
        uri: &str,
        headers: &Headers,
        body: &[u8],
    ) -> Result<AssetRequest> {
        let mut req = self.inner.request(verb, uri, headers, body)?;
        req.response = maybe_gunzip(req.response)?;
        Ok(req)
    }

    fn tick(&self) {
        self.inner.tick();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::request::{AssetRequest, AssetResponse};

    /// Minimal accessor that returns a pre-cooked response.
    struct FakeAccessor {
        response: AssetResponse,
    }

    impl AssetAccessor for FakeAccessor {
        fn get(&self, _uri: &str) -> Result<AssetRequest> {
            Ok(AssetRequest {
                method: "GET".into(),
                uri: "fake://test".into(),
                request_headers: HashMap::new(),
                response: self.response.clone(),
            })
        }
    }

    fn make_gzip_bytes(plain: &[u8]) -> Vec<u8> {
        use flate2::{Compression, write::GzEncoder};
        use std::io::Write;
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(plain).unwrap();
        enc.finish().unwrap()
    }

    #[test]
    fn decompresses_gzip_magic() {
        let compressed = make_gzip_bytes(b"hello world");
        let inner = FakeAccessor {
            response: AssetResponse {
                status_code: 200,
                content_type: "application/json".into(),
                headers: HashMap::new(),
                data: compressed,
            },
        };
        let accessor = GunzipAssetAccessor::new(inner);
        let req = accessor.get("fake://test").unwrap();
        assert_eq!(req.response.data, b"hello world");
    }

    #[test]
    fn decompresses_content_encoding_header() {
        let compressed = make_gzip_bytes(b"hello header");
        let mut headers = HashMap::new();
        headers.insert("content-encoding".into(), "gzip".into());
        let inner = FakeAccessor {
            response: AssetResponse {
                status_code: 200,
                content_type: "application/json".into(),
                headers,
                data: compressed,
            },
        };
        let accessor = GunzipAssetAccessor::new(inner);
        let req = accessor.get("fake://test").unwrap();
        assert_eq!(req.response.data, b"hello header");
        assert!(!req.response.headers.contains_key("content-encoding"));
    }

    #[test]
    fn passthrough_non_gzip() {
        let plain = b"plain text response";
        let inner = FakeAccessor {
            response: AssetResponse {
                status_code: 200,
                content_type: "text/plain".into(),
                headers: HashMap::new(),
                data: plain.to_vec(),
            },
        };
        let accessor = GunzipAssetAccessor::new(inner);
        let req = accessor.get("fake://test").unwrap();
        assert_eq!(req.response.data, plain);
    }

    #[test]
    fn passthrough_error_status() {
        let inner = FakeAccessor {
            response: AssetResponse {
                status_code: 404,
                content_type: String::new(),
                headers: HashMap::new(),
                data: vec![],
            },
        };
        let accessor = GunzipAssetAccessor::new(inner);
        let req = accessor.get("fake://test").unwrap();
        assert_eq!(req.response.status_code, 404);
        assert!(req.response.data.is_empty());
    }
}
