//! The `AssetAccessor` trait — synchronous resource fetcher.
//!
//! Equivalent to cesium-native's `IAssetAccessor`. All I/O goes through this
//! trait, which operates on arbitrary URIs. For REST endpoints, URIs are HTTP
//! URLs. For SLPK archives, URIs are archive entry paths.
//!
//! URI construction (mapping I3S resource concepts to URIs) is handled by
//! [`ResourceUriResolver`](crate::ResourceUriResolver), keeping the accessor
//! transport-agnostic.
//!
//! ## Why synchronous?
//!
//! Worker threads in the [`TaskProcessor`](crate::TaskProcessor) pool are
//! free to block — that is their purpose. Making `get` sync means the trait
//! is object-safe (`Arc<dyn AssetAccessor>` works), eliminates the
//! `SceneLayer<A>` generic monomorphization, and removes all `block_on` call
//! sites. The concurrency model is: bounded thread pool + blocking I/O per
//! thread, exactly as cesium-native does internally with libcurl.

use i3s_util::Result;

use crate::request::AssetRequest;

/// Synchronous resource accessor — equivalent to cesium-native's `IAssetAccessor`.
///
/// Fetches resources by URI. Implementations handle the transport layer:
/// - [`I3sAssetAccessor`](crate::accessor_impl::I3sAssetAccessor) — unified HTTP+SLPK accessor
///
/// Returns [`AssetRequest`] with full response metadata (status code, content
/// type, headers). Non-success HTTP status codes (404, 500, etc.) are returned
/// in the response, **not** as `Err`. `Err` is reserved for I/O-level failures
/// (network unreachable, archive corrupted, etc.).
///
/// The trait is **object-safe**: use `Arc<dyn AssetAccessor>` freely.
pub trait AssetAccessor: Send + Sync {
    /// Fetch a resource by URI (HTTP GET equivalent).
    ///
    /// Blocking — may perform network I/O or file I/O. Must only be called
    /// from a worker thread (not the main thread / frame loop).
    ///
    /// Returns the completed request-response pair. On success, the response
    /// carries status 200 and the resource data. On HTTP-level errors, the
    /// response carries the error status code (404, 500) and an empty or error body.
    ///
    /// Returns `Err` only for transport-level failures (network down, file I/O
    /// error, lock poisoned, etc.).
    fn get(&self, uri: &str) -> Result<AssetRequest>;
}
