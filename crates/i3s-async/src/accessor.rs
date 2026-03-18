//! The `AssetAccessor` trait — synchronous resource fetcher.

use i3s_util::Result;

use crate::request::{AssetRequest, AssetResponse, Headers};

/// Synchronous resource accessor. Non-success HTTP status codes are in the response, not `Err`.
pub trait AssetAccessor: Send + Sync {
    /// Fetch a resource via GET. Non-success HTTP status codes are returned in
    /// the response, not as `Err`. `Err` is reserved for network/IO failures.
    fn get(&self, uri: &str) -> Result<AssetRequest>;

    /// Fetch a resource using an arbitrary HTTP verb with an optional body.
    ///
    /// The default implementation returns a 405 Method Not Allowed response.
    /// Override this for accessors that need to serve POST/PATCH/etc.
    fn request(
        &self,
        verb: &str,
        uri: &str,
        headers: &Headers,
        body: &[u8],
    ) -> Result<AssetRequest> {
        Ok(AssetRequest {
            method: verb.to_string(),
            uri: uri.to_string(),
            request_headers: Headers::default(),
            response: AssetResponse {
                status_code: 405,
                content_type: String::new(),
                headers: Headers::default(),
                data: Vec::new(),
            },
        })
    }

    /// Called each frame on the main thread.
    ///
    /// Accessors that need to pump an event loop (e.g., cURL multi) should
    /// do so here. The default implementation is a no-op.
    fn tick(&self) {}
}
