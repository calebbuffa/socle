//! High-level entry point for configuring a 3D Tiles tileset.
//!
//! [`TilesetBuilder`] wires the async fetch → hierarchy parse → LOD evaluation
//! → content pipeline, producing either a ready [`TilesetResult`] or an async
//! task that resolves to one. The caller (typically `kiban`) uses the result
//! to build a `NodeStore` and drive content loading.

use std::sync::Arc;

use egaku::ContentPipeline;
use orkester::Context;
use orkester_io::AssetAccessor;
use selekt::{NodeDescriptor, SelectionOptions};
use terra::Ellipsoid;

use crate::EllipsoidTilesetLoader;
use crate::evaluator::GeometricErrorEvaluator;
use crate::hierarchy::ExplicitTilesetHierarchy;
use crate::loader::{TilesetLoader, TilesetLoaderFactory};

/// Result of [`TilesetBuilder::build`].
///
/// Either a synchronously-ready tileset or an async task that will produce one.
pub enum TilesetResult<C: Send + 'static> {
    /// Tileset is ready now (e.g. procedural ellipsoid source).
    Ready(ReadyTileset<C>),
    /// Tileset is loading asynchronously (e.g. URL tileset.json fetch).
    Loading {
        task: orkester::Task<Result<ReadyTileset<C>, crate::loader::Tiles3dError>>,
        maximum_screen_space_error: f64,
    },
}

/// A fully-parsed tileset: node descriptors, content loader, and metadata.
///
/// The caller (kiban) builds a `NodeStore` from `descriptors` and uses
/// `loader` to fetch tile content.
pub struct ReadyTileset<C: Send + 'static> {
    /// Flat node descriptors for building a `NodeStore`.
    pub descriptors: Vec<NodeDescriptor>,
    /// Index of the root node in `descriptors` (always 0 for 3D Tiles).
    pub root_index: usize,
    /// Content loader for fetching tile data.
    pub loader: TilesetLoader<C>,
    /// LOD evaluator configured with the tileset's SSE threshold.
    pub lod_evaluator: GeometricErrorEvaluator,
    /// Attribution / copyright string from the tileset.
    pub attribution: Option<Arc<str>>,
    /// The maximum screen-space error threshold.
    pub maximum_screen_space_error: f64,
}

enum TilesetSource {
    /// Fetch and parse `tileset.json` from this URL (async).
    Uri(String),
    /// Generate an in-memory ellipsoid globe tileset (synchronous, no network).
    Ellipsoid(EllipsoidTilesetLoader),
}

/// Builder for a streaming 3D Tiles tileset.
pub struct TilesetBuilder {
    source: TilesetSource,
    headers: Vec<(String, String)>,
    maximum_screen_space_error: f64,
    options: SelectionOptions,
    attribution: Option<Arc<str>>,
}

impl TilesetBuilder {
    /// Begin configuring a tileset streamed from `uri`.
    pub fn open(uri: impl Into<String>) -> Self {
        Self::from_source(TilesetSource::Uri(uri.into()))
    }

    /// Begin configuring a procedural ellipsoid tileset.
    pub fn ellipsoid(ellipsoid: Ellipsoid) -> Self {
        Self::from_source(TilesetSource::Ellipsoid(EllipsoidTilesetLoader::new(
            ellipsoid,
        )))
    }

    fn from_source(source: TilesetSource) -> Self {
        Self {
            source,
            headers: Vec::new(),
            maximum_screen_space_error: 16.0,
            options: SelectionOptions::default(),
            attribution: None,
        }
    }

    /// HTTP headers forwarded with every tile request (e.g. `Authorization`).
    pub fn headers(mut self, headers: Vec<(String, String)>) -> Self {
        self.headers = headers;
        self
    }

    /// Maximum screen-space error threshold in pixels. Default: `16.0`.
    pub fn maximum_screen_space_error(mut self, sse: f64) -> Self {
        self.maximum_screen_space_error = sse;
        self
    }

    /// Memory ceiling in bytes for decoded resident tile content. Default: 512 MiB.
    pub fn max_cached_bytes(mut self, bytes: usize) -> Self {
        self.options.loading.max_cached_bytes = bytes;
        self
    }

    /// Maximum simultaneous in-flight tile loads. Default: `16`.
    pub fn max_simultaneous_loads(mut self, n: usize) -> Self {
        self.options.loading.max_simultaneous_loads = n;
        self
    }

    /// Replace all [`SelectionOptions`] at once.
    pub fn options(mut self, options: SelectionOptions) -> Self {
        self.options = options;
        self
    }

    /// Attribution / copyright string (overrides the value parsed from tileset.json).
    pub fn attribution(mut self, text: impl Into<Arc<str>>) -> Self {
        self.attribution = Some(text.into());
        self
    }

    /// Build a [`TilesetResult`] containing either a ready tileset or an
    /// async loading task.
    pub fn build<C>(
        self,
        bg_ctx: Context,
        accessor: Arc<dyn AssetAccessor>,
        pipeline: Arc<ContentPipeline<C>>,
    ) -> TilesetResult<C>
    where
        C: Send + 'static,
    {
        let sse = self.maximum_screen_space_error;

        match self.source {
            TilesetSource::Uri(uri) => {
                let loader_factory = TilesetLoaderFactory::new(uri, pipeline)
                    .with_headers(self.headers)
                    .with_maximum_screen_space_error(sse);

                let override_attr = self.attribution;
                let task = loader_factory.create(bg_ctx, &accessor).map(move |result| {
                    result.map(|(descriptors, root_index, loader, parsed_attr)| {
                        let final_attr = override_attr.or(parsed_attr);
                        ReadyTileset {
                            descriptors,
                            root_index,
                            loader,
                            lod_evaluator: GeometricErrorEvaluator::new(sse),
                            attribution: final_attr,
                            maximum_screen_space_error: sse,
                        }
                    })
                });

                TilesetResult::Loading {
                    task,
                    maximum_screen_space_error: sse,
                }
            }

            TilesetSource::Ellipsoid(loader) => {
                let tileset = loader.create_tileset();
                let hierarchy = ExplicitTilesetHierarchy::from_tileset(&tileset);
                let (mut descriptors, root_index) = hierarchy.to_descriptors();
                let lod_evaluator = GeometricErrorEvaluator::new(sse);
                let content_loader = TilesetLoader::ellipsoid(loader.ellipsoid().clone(), pipeline);

                // Mark leaf nodes (and all renderable nodes) as having latent
                // children so the expand callback can generate quadtree
                // subdivisions beyond the initial pre-generated depth.
                for desc in &mut descriptors {
                    if desc.child_indices.is_empty() && desc.globe_rectangle.is_some() {
                        desc.might_have_latent_children = true;
                    }
                }

                TilesetResult::Ready(ReadyTileset {
                    descriptors,
                    root_index,
                    loader: content_loader,
                    lod_evaluator,
                    attribution: self.attribution,
                    maximum_screen_space_error: sse,
                })
            }
        }
    }
}
