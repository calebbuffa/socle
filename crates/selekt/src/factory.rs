//! Content loader factory.
//!
//! A [`ContentLoaderFactory`] asynchronously creates a content loader and
//! hierarchy from a data source (URL, asset ID, SLPK path, etc.).

use std::sync::Arc;

use orkester::{AsyncSystem, Future};
use orkester_io::AssetAccessor;

use crate::hierarchy::SpatialHierarchy;
use crate::load::ContentLoader;
use crate::lod::LodEvaluator;

/// Result produced by a [`ContentLoaderFactory`].
///
/// Packages together the constructed loader, hierarchy, LOD evaluator,
/// and any errors that arose.
pub struct ContentLoaderFactoryResult<C, H, B, L>
where
    C: Send + 'static,
    H: SpatialHierarchy,
    B: LodEvaluator,
    L: ContentLoader<C>,
{
    /// The spatial hierarchy describing the tile tree.
    pub hierarchy: H,
    /// The LOD evaluator for this format.
    pub lod_evaluator: B,
    /// The content loader for fetching tile data.
    pub content_loader: L,
    /// Request headers to use for subsequent tile loads.
    pub request_headers: Vec<(String, String)>,
    /// Warning/error messages that arose during creation.
    pub errors: Vec<String>,
    /// Phantom to carry the content type.
    _phantom: std::marker::PhantomData<C>,
}

impl<C, H, B, L> ContentLoaderFactoryResult<C, H, B, L>
where
    C: Send + 'static,
    H: SpatialHierarchy,
    B: LodEvaluator,
    L: ContentLoader<C>,
{
    /// Create a successful result.
    pub fn new(hierarchy: H, lod_evaluator: B, content_loader: L) -> Self {
        Self {
            hierarchy,
            lod_evaluator,
            content_loader,
            request_headers: Vec::new(),
            errors: Vec::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Add request headers.
    pub fn with_request_headers(mut self, headers: Vec<(String, String)>) -> Self {
        self.request_headers = headers;
        self
    }
}

/// Async factory for creating a content loader and hierarchy.
///
/// Implementations parse a tileset descriptor (URL, file path, asset ID)
/// and produce the hierarchy + loader asynchronously.
///
/// # Usage
///
/// ```ignore
/// let engine = SelectionEngine::from_factory(externals, factory, resolver, renderer, policy, options);
/// // The engine starts with no root tile. Poll `root_available()` to know when
/// // the hierarchy is populated.
/// ```
pub trait ContentLoaderFactory: Send + 'static {
    /// The decoded content type (mesh, point cloud, etc.)
    type Content: Send + 'static;
    /// The spatial hierarchy type.
    type Hierarchy: SpatialHierarchy;
    /// The LOD evaluator type.
    type LodEvaluator: LodEvaluator;
    /// The content loader type.
    type ContentLoader: ContentLoader<Self::Content>;
    /// Error type for factory creation failures.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Asynchronously create the hierarchy, LOD evaluator, and content loader.
    ///
    /// The implementation should:
    /// 1. Fetch the tileset descriptor (e.g., scene layer JSON, tileset.json)
    /// 2. Parse it to extract the hierarchy structure
    /// 3. Return the assembled components
    ///
    /// The engine calls this once during construction and wires the result
    /// into the selection pipeline.
    fn create_loader(
        self,
        async_system: &AsyncSystem,
        asset_accessor: &Arc<dyn AssetAccessor>,
    ) -> Future<
        Result<
            ContentLoaderFactoryResult<
                Self::Content,
                Self::Hierarchy,
                Self::LodEvaluator,
                Self::ContentLoader,
            >,
            Self::Error,
        >,
    >;
}
