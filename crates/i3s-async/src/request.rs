//! Asset request and response types.

use std::collections::HashMap;

use i3s_util::{I3sError, Result};

/// HTTP-style headers (header name → value).
pub type Headers = HashMap<String, String>;

/// Response from a resource fetch.
#[derive(Debug, Clone)]
pub struct AssetResponse {
    pub status_code: u16,
    pub content_type: String,
    pub headers: Headers,
    pub data: Vec<u8>,
}

impl AssetResponse {
    /// Whether the status code indicates success (2xx).
    #[inline]
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status_code)
    }
}

/// A completed asset request: method, URI, and response.
#[derive(Debug, Clone)]
pub struct AssetRequest {
    pub method: String,
    pub uri: String,
    pub request_headers: Headers,
    pub response: AssetResponse,
}

impl AssetRequest {
    /// Returns the body if status is 2xx, otherwise `Err(I3sError::Http)`.
    pub fn into_data(self) -> Result<Vec<u8>> {
        if self.response.is_success() {
            Ok(self.response.data)
        } else {
            Err(I3sError::Http {
                status: self.response.status_code,
                url: self.uri,
            })
        }
    }
}
