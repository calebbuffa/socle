//! The `AssetAccessor` trait — generic async resource fetcher.
//!
//! Equivalent to cesium-native's `IAssetAccessor`. All I/O goes through this
//! trait, which operates on arbitrary URIs. For REST endpoints, URIs are HTTP
//! URLs. For SLPK archives, URIs are archive entry paths.
//!
//! URI construction (mapping I3S resource concepts to URIs) is handled by
//! [`ResourceUriResolver`](crate::ResourceUriResolver), keeping the accessor
//! transport-agnostic.
//!
//! Uses native Rust `async fn` in traits (edition 2024) — no proc-macro needed.
//! The trait uses RPITIT so consumers must be generic: `fn foo<A: AssetAccessor>(a: &A)`.

use i3s_util::Result;

use crate::request::AssetRequest;

/// Async resource accessor — equivalent to cesium-native's `IAssetAccessor`.
///
/// Fetches resources by URI. Implementations handle the transport layer:
/// - [`RestAssetAccessor`](crate::rest::RestAssetAccessor) — HTTP GET via reqwest
/// - [`SlpkAssetAccessor`](crate::slpk::SlpkAssetAccessor) — ZIP archive via the `zip` crate
///
/// Returns [`AssetRequest`] with full response metadata (status code, content
/// type, headers). Non-success HTTP status codes (404, 500, etc.) are returned
/// in the response, **not** as `Err`. `Err` is reserved for I/O-level failures
/// (network unreachable, archive lock poisoned, etc.).
///
/// Because this trait uses RPITIT (`async fn`), it is **not** object-safe.
/// Consumers should be generic: `fn foo<A: AssetAccessor>(a: &A)`.
pub trait AssetAccessor: Send + Sync {
    /// Fetch a resource by URI (HTTP GET equivalent).
    ///
    /// Returns the completed request-response pair. On success, the response
    /// carries status 200 and the resource data. On HTTP-level errors, the
    /// response carries the error status code (404, 500) and an empty or error body.
    ///
    /// Returns `Err` only for transport-level failures (network down, file I/O
    /// error, lock poisoned, etc.).
    #[allow(async_fn_in_trait)]
    async fn get(&self, uri: &str) -> Result<AssetRequest>;
}
