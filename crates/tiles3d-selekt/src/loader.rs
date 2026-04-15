//! Async 3D Tiles content loading.
//!
//! [`TilesetLoaderFactory`] fetches `tileset.json` and produces
//! [`NodeDescriptor`]s + a [`TilesetLoader`] that streams tile content.
//! Each tile is fetched, format-detected, decoded to a
//! [`GltfModel`](moderu::GltfModel), then handed to the renderer's
//! [`ContentPipeline`](egaku::ContentPipeline).

use std::sync::Arc;

use egaku::ContentPipeline;
use orkester::{CancellationToken, Context, Task};
use orkester_io::{AssetAccessor, AssetResponse, RequestPriority};
use outil::resolve_url;
use selekt::{ContentKey, ExpandResult, NodeDescriptor, NodeId};
use terra::{Cartographic, Ellipsoid, GlobeRectangle, calc_quadtree_max_geometric_error};

use crate::hierarchy::{
    ExplicitTilesetHierarchy, ImplicitOctreeHierarchy, ImplicitQuadtreeHierarchy,
};
use tiles3d::implicit_tiling_utilities;
use tiles3d::parse_subtree;
use tiles3d::{
    BoundingVolume, ImplicitTiling, OctreeAvailability, QuadtreeAvailability, SubdivisionScheme,
    Tileset,
};
use tiles3d::{OctreeTileID, QuadtreeTileID};
use tiles3d_content::{TileFormat, decode_tile};

/// Errors from the 3D Tiles loading pipeline.
#[derive(Debug, thiserror::Error)]
pub enum Tiles3dError {
    /// Network or I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON parse failure (tileset.json or tile content).
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    /// HTTP error status returned by the asset accessor.
    #[error("HTTP error {0}")]
    Http(u16),
    /// Content decoder returned an error.
    #[error("decode error: {0}")]
    Decode(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
    /// `.subtree` file could not be parsed.
    #[error("subtree error: {0}")]
    Subtree(#[from] tiles3d::SubtreeParseError),
    /// The `subdivisionScheme` value in `implicitTiling` was not recognised.
    #[error("unknown subdivisionScheme '{0}'")]
    UnknownSubdivisionScheme(String),
}

/// Result of loading content for a single tile.
pub enum LoadResult<C> {
    /// Normal renderable content.
    Renderable { content: C, byte_size: usize },
    /// External tileset reference — provides new sub-scene nodes.
    SubScene {
        descriptors: Vec<NodeDescriptor>,
        root_index: usize,
        byte_size: usize,
    },
    /// Empty — no geometry or sub-scene.
    Empty,
}

/// Build the URL for the root subtree file (level=0, x=0, y=0[, z=0]).
fn resolve_root_subtree_url(
    base_url: &str,
    uri_template: &str,
    scheme: SubdivisionScheme,
) -> String {
    match scheme {
        SubdivisionScheme::Quadtree => implicit_tiling_utilities::resolve_url_quad(
            base_url,
            uri_template,
            QuadtreeTileID::new(0, 0, 0),
        ),
        SubdivisionScheme::Octree => implicit_tiling_utilities::resolve_url_oct(
            base_url,
            uri_template,
            OctreeTileID::new(0, 0, 0, 0),
        ),
    }
}

/// Async factory that fetches and parses `tileset.json`, builds
/// [`NodeDescriptor`]s, and constructs a [`TilesetLoader`].
pub struct TilesetLoaderFactory<C: Send + 'static> {
    pub tileset_url: String,
    pub headers: Vec<(String, String)>,
    pub maximum_screen_space_error: f64,
    pub pipeline: Arc<ContentPipeline<C>>,
}

impl<C: Send + 'static> TilesetLoaderFactory<C> {
    pub fn new(tileset_url: impl Into<String>, pipeline: Arc<ContentPipeline<C>>) -> Self {
        Self {
            tileset_url: tileset_url.into(),
            headers: Vec::new(),
            maximum_screen_space_error: 16.0,
            pipeline,
        }
    }

    pub fn with_headers(mut self, headers: Vec<(String, String)>) -> Self {
        self.headers = headers;
        self
    }

    pub fn with_maximum_screen_space_error(mut self, sse: f64) -> Self {
        self.maximum_screen_space_error = sse;
        self
    }
}

/// Return type of [`TilesetLoaderFactory::create`]:
/// `(descriptors, root_index, loader, attribution)`.
type FactoryResult<C> = (
    Vec<NodeDescriptor>,
    usize,
    TilesetLoader<C>,
    Option<Arc<str>>,
);

impl<C> TilesetLoaderFactory<C>
where
    C: Send + 'static,
{
    pub fn create(
        self,
        bg_ctx: Context,
        asset_accessor: &Arc<dyn AssetAccessor>,
    ) -> Task<Result<FactoryResult<C>, Tiles3dError>> {
        let url: Arc<str> = self.tileset_url.into();
        let headers: Arc<[(String, String)]> = self.headers.into();
        let _sse = self.maximum_screen_space_error;
        let pipeline = self.pipeline;
        let accessor = Arc::clone(asset_accessor);
        let bg_ctx_clone = bg_ctx.clone();

        asset_accessor
            .get(&url, &headers, RequestPriority::HIGH)
            .then(
                &bg_ctx,
                move |io_result: Result<AssetResponse, std::io::Error>| -> Task<
                    Result<FactoryResult<C>, Tiles3dError>,
                > {
                    let response = match io_result {
                        Ok(r) => r,
                        Err(e) => return orkester::resolved(Err(Tiles3dError::from(e))),
                    };
                    if let Err(code) = response.check_status() {
                        return orkester::resolved(Err(Tiles3dError::Http(code)));
                    }
                    let tileset: Tileset =
                        match serde_json::from_slice(response.decompressed_data()) {
                            Ok(t) => t,
                            Err(e) => return orkester::resolved(Err(Tiles3dError::from(e))),
                        };

                    let attribution: Option<Arc<str>> = tileset
                        .asset
                        .copyright
                        .as_deref()
                        .filter(|s| !s.is_empty())
                        .map(Arc::from);

                    let loader = TilesetLoader::http(
                        Arc::clone(&accessor),
                        Arc::clone(&url),
                        Arc::clone(&headers),
                        Arc::clone(&pipeline),
                    );

                    // Detect implicit tiling
                    if let Some(implicit) = tileset.root.implicit_tiling.clone() {
                        let scheme = implicit.subdivision_scheme;
                        let subtree_levels = implicit.subtree_levels;
                        let available_levels = implicit.available_levels as u32;
                        let root_bv = tileset.root.bounding_volume.clone();
                        let root_geometric_error = tileset.root.geometric_error;
                        let use_add = matches!(
                            tileset.root.refine.map(|r| match r {
                                tiles3d::Refine::Add => "ADD",
                                tiles3d::Refine::Replace => "REPLACE",
                            }),
                            Some("ADD")
                        );
                        let content_template = tileset
                            .root
                            .content
                            .as_ref()
                            .map(|c| c.uri.clone())
                            .or_else(|| tileset.root.contents.first().map(|c| c.uri.clone()))
                            .unwrap_or_default();
                        let subtree_url =
                            resolve_root_subtree_url(&url, &implicit.subtrees.uri, scheme);

                        fetch_implicit_subtree_and_build(
                            bg_ctx_clone,
                            accessor,
                            headers,
                            subtree_url,
                            scheme,
                            subtree_levels,
                            available_levels,
                            root_bv,
                            root_geometric_error,
                            implicit,
                            content_template,
                            use_add,
                            loader,
                            attribution,
                        )
                    } else {
                        // Explicit tileset
                        let hierarchy = ExplicitTilesetHierarchy::from_tileset(&tileset);
                        let (descriptors, root_index) = hierarchy.to_descriptors();
                        orkester::resolved(Ok((descriptors, root_index, loader, attribution)))
                    }
                },
            )
    }
}

#[allow(clippy::too_many_arguments)]
fn fetch_implicit_subtree_and_build<C>(
    bg_ctx: Context,
    loader_accessor: Arc<dyn AssetAccessor>,
    loader_headers: Arc<[(String, String)]>,
    subtree_url: String,
    scheme: SubdivisionScheme,
    subtree_levels: u32,
    available_levels: u32,
    root_bv: BoundingVolume,
    root_geometric_error: f64,
    implicit: ImplicitTiling,
    content_template: String,
    use_add: bool,
    loader: TilesetLoader<C>,
    attribution: Option<Arc<str>>,
) -> Task<Result<FactoryResult<C>, Tiles3dError>>
where
    C: Send + 'static,
{
    loader_accessor
        .get(&subtree_url, &loader_headers, RequestPriority::HIGH)
        .then(
            &bg_ctx.clone(),
            move |subtree_io: Result<AssetResponse, std::io::Error>| {
                parse_subtree_and_build(
                    subtree_io,
                    scheme,
                    subtree_levels,
                    available_levels,
                    root_bv,
                    root_geometric_error,
                    implicit,
                    content_template,
                    use_add,
                    loader,
                    attribution,
                )
            },
        )
}

#[allow(clippy::too_many_arguments)]
fn parse_subtree_and_build<C>(
    subtree_io: Result<AssetResponse, std::io::Error>,
    scheme: SubdivisionScheme,
    subtree_levels: u32,
    available_levels: u32,
    root_bv: BoundingVolume,
    root_geometric_error: f64,
    implicit: ImplicitTiling,
    content_template: String,
    use_add: bool,
    loader: TilesetLoader<C>,
    attribution: Option<Arc<str>>,
) -> Result<FactoryResult<C>, Tiles3dError>
where
    C: Send + 'static,
{
    let sub_resp = subtree_io.map_err(Tiles3dError::from)?;
    sub_resp.check_status().map_err(Tiles3dError::Http)?;
    let subtree_av = parse_subtree(sub_resp.decompressed_data(), scheme, subtree_levels)
        .map_err(Tiles3dError::Subtree)?;

    let (descriptors, root_index) = match scheme {
        SubdivisionScheme::Quadtree => {
            let mut qa = QuadtreeAvailability::new(subtree_levels, available_levels);
            qa.add_subtree(QuadtreeTileID::new(0, 0, 0), subtree_av);
            let h = ImplicitQuadtreeHierarchy::new(
                &root_bv,
                root_geometric_error,
                &implicit,
                qa,
                &content_template,
                use_add,
            );
            h.to_descriptors()
        }
        SubdivisionScheme::Octree => {
            let mut oa = OctreeAvailability::new(subtree_levels, available_levels);
            oa.add_subtree(OctreeTileID::new(0, 0, 0, 0), subtree_av);
            let h = ImplicitOctreeHierarchy::new(
                &root_bv,
                root_geometric_error,
                &implicit,
                oa,
                &content_template,
                use_add,
            );
            h.to_descriptors()
        }
    };

    // Attach implicit context for on-demand child-subtree loading.
    let implicit_ctx = ImplicitCtx {
        root_bv: Arc::new(root_bv),
        root_geometric_error,
        implicit_tiling: Arc::new(implicit),
        content_url_template: Arc::from(content_template.as_str()),
        use_add,
    };
    let loader = loader.with_implicit_ctx(implicit_ctx);

    Ok((descriptors, root_index, loader, attribution))
}

/// Persistent context needed to load child subtrees for an implicit tileset.
///
/// Stored inside [`LoaderInner::Http`] when the tileset uses implicit tiling.
/// It is Arc-cloned for each subtree-load task.
#[derive(Clone)]
pub(crate) struct ImplicitCtx {
    pub root_bv: Arc<BoundingVolume>,
    pub root_geometric_error: f64,
    pub implicit_tiling: Arc<ImplicitTiling>,
    pub content_url_template: Arc<str>,
    pub use_add: bool,
}

// ── Content loading ──────────────────────────────────────────────────────────

/// Unified content loader for 3D Tiles.
///
/// Handles both HTTP-fetched tiles and procedural ellipsoid content.
pub struct TilesetLoader<C: Send + 'static> {
    inner: LoaderInner<C>,
}

enum LoaderInner<C: Send + 'static> {
    Http {
        accessor: Arc<dyn AssetAccessor>,
        base_url: Arc<str>,
        headers: Arc<[(String, String)]>,
        pipeline: Arc<ContentPipeline<C>>,
        /// Present when the tileset is an implicit tileset; used to fetch
        /// child subtrees when a `__subtree__:…` synthetic key is loaded.
        implicit_ctx: Option<ImplicitCtx>,
    },
    Ellipsoid {
        ellipsoid: Ellipsoid,
        pipeline: Arc<ContentPipeline<C>>,
    },
}

impl<C: Send + 'static> TilesetLoader<C> {
    /// Create an HTTP-based loader.
    pub(crate) fn http(
        accessor: Arc<dyn AssetAccessor>,
        base_url: impl Into<Arc<str>>,
        headers: impl Into<Arc<[(String, String)]>>,
        pipeline: Arc<ContentPipeline<C>>,
    ) -> Self {
        Self {
            inner: LoaderInner::Http {
                accessor,
                base_url: base_url.into(),
                headers: headers.into(),
                pipeline,
                implicit_ctx: None,
            },
        }
    }

    /// Attach implicit-tiling context so [`TilesetLoader::load`] can fetch
    /// child subtrees on demand.
    pub(crate) fn with_implicit_ctx(mut self, ctx: ImplicitCtx) -> Self {
        if let LoaderInner::Http {
            ref mut implicit_ctx,
            ..
        } = self.inner
        {
            *implicit_ctx = Some(ctx);
        }
        self
    }

    /// Create a procedural ellipsoid loader.
    pub(crate) fn ellipsoid(ellipsoid: Ellipsoid, pipeline: Arc<ContentPipeline<C>>) -> Self {
        Self {
            inner: LoaderInner::Ellipsoid {
                ellipsoid,
                pipeline,
            },
        }
    }

    /// Load content for the given node.
    ///
    /// Called by kiban when the selection algorithm requests a load.
    pub fn load(
        &self,
        bg_ctx: &Context,
        _node_id: NodeId,
        key: &ContentKey,
        parent_world_transform: glam::DMat4,
        cancel: CancellationToken,
    ) -> Task<Result<LoadResult<C>, Tiles3dError>> {
        match &self.inner {
            LoaderInner::Http {
                accessor,
                base_url,
                headers,
                pipeline,
                implicit_ctx,
            } => {
                // Detect synthetic subtree-fetch keys emitted by
                // ImplicitQuadtreeHierarchy::expand_node().
                if let Some(rel_url) = key.0.strip_prefix("__subtree__:") {
                    if let Some(ctx) = implicit_ctx {
                        return load_implicit_sub_subtree(
                            Arc::clone(accessor),
                            Arc::clone(base_url),
                            Arc::clone(headers),
                            ctx.clone(),
                            bg_ctx,
                            rel_url,
                            cancel,
                        );
                    }
                }
                load_http(
                    Arc::clone(accessor),
                    Arc::clone(base_url),
                    Arc::clone(headers),
                    Arc::clone(pipeline),
                    bg_ctx,
                    key,
                    parent_world_transform,
                    cancel,
                )
            }
            LoaderInner::Ellipsoid {
                ellipsoid,
                pipeline,
            } => load_ellipsoid(ellipsoid.clone(), Arc::clone(pipeline), bg_ctx, key),
        }
    }

    /// Try to expand a node's latent children (quadtree subdivision for ellipsoid tiles).
    ///
    /// Returns `ExpandResult::Children(...)` with 4 quadtree child descriptors
    /// for ellipsoid tiles, or `ExpandResult::None` for HTTP-loaded tilesets
    /// (which don't support procedural expansion).
    pub fn expand(&self, node_data: &selekt::NodeData) -> ExpandResult {
        let LoaderInner::Ellipsoid { ellipsoid, .. } = &self.inner else {
            return ExpandResult::None;
        };

        let Some(parent_rect) = node_data.globe_rectangle else {
            return ExpandResult::None;
        };

        // Compute the geometric error level from the parent's LOD value.
        // Parent error = max_err * 2\pi / (root_tiles_x * 2^level)
        // For children at level+1, error halves.
        let child_error = node_data.lod.value * 0.5;

        // Don't expand below a very small geometric error (roughly level 30).
        if child_error < 1e-7 {
            return ExpandResult::None;
        }

        let parent_transform = node_data.world_transform;
        // Parent origin in ECEF = translation column of parent's world_transform.
        let parent_ecef = glam::DVec3::new(
            parent_transform.col(3).x,
            parent_transform.col(3).y,
            parent_transform.col(3).z,
        );

        let mid_lon = (parent_rect.west + parent_rect.east) * 0.5;
        let mid_lat = (parent_rect.south + parent_rect.north) * 0.5;

        let child_rects = [
            GlobeRectangle::new(parent_rect.west, parent_rect.south, mid_lon, mid_lat), // SW
            GlobeRectangle::new(mid_lon, parent_rect.south, parent_rect.east, mid_lat), // SE
            GlobeRectangle::new(parent_rect.west, mid_lat, mid_lon, parent_rect.north), // NW
            GlobeRectangle::new(mid_lon, mid_lat, parent_rect.east, parent_rect.north), // NE
        ];

        let descriptors: Vec<NodeDescriptor> = child_rects
            .iter()
            .map(|rect| {
                let center_lon = (rect.west + rect.east) * 0.5;
                let center_lat = (rect.south + rect.north) * 0.5;
                let origin =
                    ellipsoid.cartographic_to_ecef(Cartographic::new(center_lon, center_lat, 0.0));
                let rel = origin - parent_ecef;
                let world_transform = parent_transform * glam::DMat4::from_translation(rel);

                let content_key = ContentKey(format!(
                    "{},{},{},{},{}",
                    rect.west, rect.south, rect.east, rect.north, child_error,
                ));

                NodeDescriptor {
                    bounds: crate::hierarchy::region_to_sphere_bounds(
                        rect.west, rect.south, rect.east, rect.north, 0.0, 0.0,
                    ),
                    lod: selekt::LodDescriptor {
                        value: child_error,
                        family: crate::evaluator::GEOMETRIC_ERROR_FAMILY,
                    },
                    refinement: selekt::RefinementMode::Replace,
                    kind: selekt::NodeKind::Renderable,
                    content_keys: vec![content_key],
                    world_transform,
                    might_have_latent_children: true,
                    child_indices: vec![],
                    content_bounds: None,
                    viewer_request_volume: None,
                    lod_metric_override: None,
                    globe_rectangle: Some(*rect),
                    unconditionally_refined: false,
                    content_max_age: None,
                }
            })
            .collect();

        ExpandResult::Children(descriptors)
    }
}

fn load_http<C: Send + 'static>(
    accessor: Arc<dyn AssetAccessor>,
    base_url: Arc<str>,
    headers: Arc<[(String, String)]>,
    pipeline: Arc<ContentPipeline<C>>,
    bg_ctx: &Context,
    key: &ContentKey,
    parent_world_transform: glam::DMat4,
    cancel: CancellationToken,
) -> Task<Result<LoadResult<C>, Tiles3dError>> {
    let url: Arc<str> = resolve_url(&base_url, &key.0).into();
    let accessor_clone = Arc::clone(&accessor);
    let headers_clone = Arc::clone(&headers);
    let pipeline_clone = Arc::clone(&pipeline);
    let priority = RequestPriority(128);
    accessor
        .get(&url, &headers, priority)
        .with_cancellation(&cancel)
        .then(
            bg_ctx,
            move |io_result: Result<AssetResponse, std::io::Error>| -> Task<Result<LoadResult<C>, Tiles3dError>> {
                let response = match io_result {
                    Ok(r) => r,
                    Err(e) => return orkester::resolved(Err(Tiles3dError::from(e))),
                };
                if let Err(code) = response.check_status() {
                    return orkester::resolved(Err(Tiles3dError::Http(code)));
                }
                let data = response.decompressed_data();
                if data.is_empty() {
                    return orkester::resolved(Ok(LoadResult::Empty));
                }

                // Detect external tileset reference (JSON content).
                if TileFormat::detect(&url, data) == TileFormat::Json {
                    let byte_size = data.len();
                    let tileset: Tileset = match serde_json::from_slice(data) {
                        Ok(t) => t,
                        Err(e) => return orkester::resolved(Err(Tiles3dError::from(e))),
                    };
                    let mut child_hierarchy = ExplicitTilesetHierarchy::from_tileset_with_root_transform(
                        &tileset,
                        parent_world_transform,
                    );
                    if child_hierarchy.node_count() == 0 {
                        return orkester::resolved(Ok(LoadResult::Empty));
                    }
                    // Resolve relative content keys against the external tileset URL.
                    child_hierarchy.resolve_content_keys(&url);
                    let (descriptors, root_index) = child_hierarchy.to_descriptors();
                    return orkester::resolved(Ok(LoadResult::SubScene {
                        descriptors,
                        root_index,
                        byte_size,
                    }));
                }

                // Normal tile: detect format, decode to GltfModel, hand to pipeline.
                let byte_size = data.len();
                let format = TileFormat::detect(&url, data);
                match decode_tile(response.decompressed_data(), &format) {
                    Some(model) => {
                        pipeline_clone.run(model).map(move |result| {
                            match result {
                                Ok(content) => Ok(LoadResult::Renderable { content, byte_size }),
                                Err(e) => Err(Tiles3dError::Decode(e)),
                            }
                        })
                    }
                    None => orkester::resolved(Ok(LoadResult::Empty)),
                }
            },
        )
}

/// Load a child subtree file and return its tiles as a `LoadResult::SubScene`.
///
/// `rel_url` is the relative subtree URL (everything after `"__subtree__:"`).
/// It is resolved against `base_url` to obtain the absolute fetch URL.
fn load_implicit_sub_subtree<C: Send + 'static>(
    accessor: Arc<dyn AssetAccessor>,
    base_url: Arc<str>,
    headers: Arc<[(String, String)]>,
    ctx: ImplicitCtx,
    bg_ctx: &Context,
    rel_url: &str,
    cancel: CancellationToken,
) -> Task<Result<LoadResult<C>, Tiles3dError>> {
    let subtree_root = parse_subtree_tile_from_url(&ctx.implicit_tiling.subtrees.uri, rel_url);
    let subtree_url: Arc<str> = resolve_url(&base_url, rel_url).into();

    accessor
        .get(&subtree_url, &headers, RequestPriority::HIGH)
        .with_cancellation(&cancel)
        .then(
            bg_ctx,
            move |io_result: Result<AssetResponse, std::io::Error>| {
                let response = match io_result {
                    Ok(r) => r,
                    Err(e) => return orkester::resolved(Err(Tiles3dError::from(e))),
                };
                if let Err(code) = response.check_status() {
                    return orkester::resolved(Err(Tiles3dError::Http(code)));
                }

                let Some(subtree_root) = subtree_root else {
                    return orkester::resolved(Ok(LoadResult::Empty));
                };

                let subtree_levels = ctx.implicit_tiling.subtree_levels;
                let subtree_av = match parse_subtree(
                    response.decompressed_data(),
                    tiles3d::SubdivisionScheme::Quadtree,
                    subtree_levels,
                ) {
                    Ok(av) => av,
                    Err(e) => return orkester::resolved(Err(Tiles3dError::Subtree(e))),
                };

                let byte_size = response.decompressed_data().len();
                let h = ImplicitQuadtreeHierarchy::new_for_subtree(
                    subtree_root,
                    &ctx.root_bv,
                    ctx.root_geometric_error,
                    &ctx.implicit_tiling,
                    subtree_av,
                    ctx.content_url_template.as_ref(),
                    ctx.use_add,
                );
                let (descriptors, root_index) = h.to_descriptors();
                orkester::resolved(Ok(LoadResult::SubScene {
                    descriptors,
                    root_index,
                    byte_size,
                }))
            },
        )
}

/// Recover a [`QuadtreeTileID`] from a resolved subtree URL by matching
/// against the URI template's `{level}`, `{x}`, `{y}` placeholders.
///
/// Returns `None` when the URL cannot be parsed.
fn parse_subtree_tile_from_url(template: &str, rel_url: &str) -> Option<QuadtreeTileID> {
    let mut level: Option<u32> = None;
    let mut x_coord: Option<u32> = None;
    let mut y_coord: Option<u32> = None;

    let mut url_rest = rel_url;
    let mut tmpl_rest = template;
    for (placeholder, dest) in [
        ("{level}", &mut level),
        ("{x}", &mut x_coord),
        ("{y}", &mut y_coord),
    ] {
        let Some(ph_idx) = tmpl_rest.find(placeholder) else {
            break;
        };
        let literal_prefix = &tmpl_rest[..ph_idx];
        let Some(stripped) = url_rest.strip_prefix(literal_prefix) else {
            return None;
        };
        url_rest = stripped;
        let end = url_rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(url_rest.len());
        *dest = url_rest[..end].parse().ok();
        url_rest = &url_rest[end..];
        tmpl_rest = &tmpl_rest[ph_idx + placeholder.len()..];
    }

    Some(QuadtreeTileID::new(level?, x_coord?, y_coord?))
}

fn load_ellipsoid<C: Send + 'static>(
    ellipsoid: Ellipsoid,
    pipeline: Arc<ContentPipeline<C>>,
    bg_ctx: &Context,
    key: &ContentKey,
) -> Task<Result<LoadResult<C>, Tiles3dError>> {
    // Parse "west,south,east,north[,geometric_error]" from the content key.
    let parts: Vec<f64> = key.0.split(',').filter_map(|s| s.parse().ok()).collect();
    if parts.len() < 4 {
        return orkester::resolved(Ok(LoadResult::Empty));
    }
    let (west, south, east, north) = (parts[0], parts[1], parts[2], parts[3]);
    let skirt_height = if parts.len() >= 5 {
        parts[4] * 5.0
    } else {
        0.0
    };
    let byte_size = crate::ellipsoid_content_loader::total_byte_size();

    bg_ctx.run(move || -> Task<Result<LoadResult<C>, Tiles3dError>> {
        let model = crate::ellipsoid_content_loader::build_model(
            &ellipsoid,
            west,
            south,
            east,
            north,
            skirt_height,
        );
        pipeline.run(model).map(move |result| match result {
            Ok(content) => Ok(LoadResult::Renderable { content, byte_size }),
            Err(e) => Err(Tiles3dError::Decode(e)),
        })
    })
}
