//! [`ExternalTilesetResolver`] - retained for API compatibility.
//!
//! External tileset resolution is now handled inline by [`TilesetLoader`].

use orkester_io::AssetAccessor;
use std::sync::Arc;

/// Retained for API compatibility. No longer needed.
pub struct ExternalTilesetResolver {
    _accessor: Arc<dyn AssetAccessor>,
    _base_url: Arc<str>,
    _headers: Arc<[(String, String)]>,
}

impl ExternalTilesetResolver {
    pub fn new(
        accessor: Arc<dyn AssetAccessor>,
        base_url: impl Into<Arc<str>>,
        headers: impl Into<Arc<[(String, String)]>>,
    ) -> Self {
        Self {
            _accessor: accessor,
            _base_url: base_url.into(),
            _headers: headers.into(),
        }
    }
}
