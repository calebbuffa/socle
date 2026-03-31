//! [`ExternalTilesetResolver`] — resolves `Payload::Reference` from explicit tilesets.
//!
//! When the tile loader encounters a tile whose content is a JSON file (another
//! `tileset.json`), it returns `Payload::Reference`.  The engine then calls
//! [`HierarchyResolver::resolve_reference`] with that reference.  This module
//! fetches the child tileset, flattens it into an [`ExplicitTilesetHierarchy`],
//! and returns a [`HierarchyExpansion`] that the engine merges into the live hierarchy.

use std::sync::Arc;

use orkester::{Context, Task};
use orkester_io::{AssetAccessor, RequestPriority, resolve_url};
use selekt::{HierarchyExpansion, HierarchyReference, HierarchyResolver};

use crate::hierarchy::ExplicitTilesetHierarchy;
use crate::loader::Tiles3dError;
use tiles3d::Tileset;

/// Resolves external tileset references for explicit 3D Tiles hierarchies.
///
/// When a tile's content is another `tileset.json`, the selection engine
/// returns `Payload::Reference`.  This resolver fetches the child tileset on a
/// background thread, builds an [`ExplicitTilesetHierarchy`] from it, and
/// returns the inserted node IDs as a [`HierarchyExpansion`].
///
/// The patch is applied by the engine to the live [`ExplicitTilesetHierarchy`]
/// via [`SpatialHierarchy::expand`].
pub struct ExternalTilesetResolver {
    accessor: Arc<dyn AssetAccessor>,
    /// Base URL for resolving relative references produced by the root tileset.
    base_url: Arc<str>,
    headers: Arc<[(String, String)]>,
}

impl ExternalTilesetResolver {
    /// Create a new resolver.
    ///
    /// * `accessor` — shared asset accessor for HTTP / file fetches.
    /// * `base_url` — the URL of the root `tileset.json` (used to resolve
    ///   relative child URLs).
    /// * `headers` — request headers forwarded to every fetch.
    pub fn new(
        accessor: Arc<dyn AssetAccessor>,
        base_url: impl Into<Arc<str>>,
        headers: impl Into<Arc<[(String, String)]>>,
    ) -> Self {
        Self {
            accessor,
            base_url: base_url.into(),
            headers: headers.into(),
        }
    }
}

impl HierarchyResolver for ExternalTilesetResolver {
    type Error = Tiles3dError;

    fn resolve_reference(
        &self,
        bg_context: &Context,
        reference: HierarchyReference,
    ) -> Task<Result<Option<HierarchyExpansion>, Self::Error>> {
        let url: Arc<str> = resolve_url(&self.base_url, &reference.key.0).into();
        let headers = Arc::clone(&self.headers);
        let parent = reference.source;
        // Carry the accumulated parent transform so the child tileset's
        // bounding volumes are expressed in the same world space.
        let parent_transform = reference.transform.unwrap_or(glam::DMat4::IDENTITY);

        self.accessor
            .get(&url, &headers, RequestPriority::NORMAL)
            .then(
                bg_context,
                move |io_result: Result<orkester_io::AssetResponse, std::io::Error>| {
                    let response = io_result.map_err(Tiles3dError::from)?;
                    response.check_status().map_err(Tiles3dError::Http)?;

                    let child_tileset: Tileset =
                        serde_json::from_slice(response.decompressed_data()).map_err(Tiles3dError::from)?;

                    // Build the child hierarchy, propagating the parent world transform
                    // so all child bounding volumes are in the same coordinate space.
                    let child_hierarchy =
                        ExplicitTilesetHierarchy::from_tileset_with_root_transform(
                            &child_tileset,
                            parent_transform,
                        );

                    if child_hierarchy.node_count() == 0 {
                        return Ok(None);
                    }

                    Ok(Some(HierarchyExpansion::with_payload(
                        parent,
                        child_hierarchy,
                    )))
                },
            )
    }
}
