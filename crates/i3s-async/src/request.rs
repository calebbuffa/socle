//! Asset request and response types — equivalent to cesium-native's
//! `IAssetRequest` and `IAssetResponse`.
//!
//! These types carry full HTTP-style request/response metadata, allowing
//! uniform handling across REST and SLPK data sources.

use std::collections::HashMap;

use i3s_util::{I3sError, Result};

/// HTTP-style headers (header name → value).
pub type Headers = HashMap<String, String>;

/// Response from a resource fetch — equivalent to cesium-native's `IAssetResponse`.
///
/// Carries the status code, content type, headers, and response body.
/// For SLPK, status codes follow HTTP conventions (200 = found, 404 = not found).
#[derive(Debug, Clone)]
pub struct AssetResponse {
    /// HTTP status code (200 = success, 404 = not found, 500 = error, etc.)
    pub status_code: u16,
    /// Content type (MIME type, e.g. "application/json", "application/octet-stream").
    pub content_type: String,
    /// Response headers.
    pub headers: Headers,
    /// Response body bytes.
    pub data: Vec<u8>,
}

impl AssetResponse {
    /// Whether the status code indicates success (2xx).
    #[inline]
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status_code)
    }
}

/// A completed asset request — equivalent to cesium-native's `IAssetRequest`.
///
/// Bundles the request metadata (method, URI) with the response.
/// Returned by [`AssetAccessor::get`](crate::AssetAccessor::get).
#[derive(Debug, Clone)]
pub struct AssetRequest {
    /// HTTP method (e.g. "GET").
    pub method: String,
    /// The URI that was requested.
    pub uri: String,
    /// Request headers that were sent.
    pub request_headers: Headers,
    /// The response.
    pub response: AssetResponse,
}

impl AssetRequest {
    /// Extract the response data, converting non-success status codes to errors.
    ///
    /// Returns `Ok(data)` if the status code is 2xx, otherwise returns
    /// `Err(I3sError::Http { ... })`.
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
