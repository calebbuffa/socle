//! Async network I/O traits for orkester.
//!
//! Defines the [`AssetAccessor`] trait and supporting types
//! for asynchronous HTTP/network operations.

use orkester::{AsyncSystem, Future};
use std::collections::HashMap;

/// HTTP headers as key-value pairs.
pub type HttpHeaders = HashMap<String, String>;

/// A completed HTTP response.
pub trait AssetResponse: Send + Sync {
    /// HTTP status code (e.g. 200, 404). Returns 0 for non-HTTP responses.
    fn status_code(&self) -> u16;

    /// Response content type (e.g. `"application/json"`).
    fn content_type(&self) -> &str;

    /// Response headers.
    fn headers(&self) -> &HttpHeaders;

    /// Response body data.
    fn data(&self) -> &[u8];
}

/// A completed asset request, containing the original request metadata
/// and the response.
pub trait AssetRequest: Send + Sync {
    /// HTTP method used (e.g. `"GET"`, `"POST"`).
    fn method(&self) -> &str;

    /// The URL that was requested.
    fn url(&self) -> &str;

    /// Request headers that were sent.
    fn request_headers(&self) -> &HttpHeaders;

    /// The response, or `None` if the request failed without producing a response.
    fn response(&self) -> Option<&dyn AssetResponse>;
}

/// Asynchronous asset accessor.
///
/// Implementors provide HTTP (or other
/// protocol) access to remote assets. The engine invokes these methods from
/// any thread; implementations must be `Send + Sync`.
pub trait AssetAccessor: Send + Sync {
    /// Perform an HTTP GET for the given URL.
    fn get(
        &self,
        async_system: &AsyncSystem,
        url: &str,
        headers: &[(String, String)],
    ) -> Future<Box<dyn AssetRequest>>;

    /// Perform an HTTP request with an arbitrary verb and optional body.
    fn request(
        &self,
        async_system: &AsyncSystem,
        verb: &str,
        url: &str,
        headers: &[(String, String)],
        body: &[u8],
    ) -> Future<Box<dyn AssetRequest>>;

    /// Tick the accessor during main-thread blocking waits.
    ///
    /// Some implementations (e.g., browser-based) need the main thread to
    /// pump their event loop. Implementations that don't need this can
    /// leave it empty.
    fn tick(&self) {}
}

// Allow `Box<dyn AssetAccessor>` to be used directly.
impl AssetAccessor for Box<dyn AssetAccessor> {
    fn get(
        &self,
        async_system: &AsyncSystem,
        url: &str,
        headers: &[(String, String)],
    ) -> Future<Box<dyn AssetRequest>> {
        (**self).get(async_system, url, headers)
    }

    fn request(
        &self,
        async_system: &AsyncSystem,
        verb: &str,
        url: &str,
        headers: &[(String, String)],
        body: &[u8],
    ) -> Future<Box<dyn AssetRequest>> {
        (**self).request(async_system, verb, url, headers, body)
    }

    fn tick(&self) {
        (**self).tick()
    }
}
