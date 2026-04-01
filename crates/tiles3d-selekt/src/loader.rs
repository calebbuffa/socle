//! Async 3D Tiles content loading.
//!
//! Provides [`TilesetLoaderFactory`] and [`TilesetLoader`], the two halves
//! of the async fetch→decode→GPU pipeline for 3D Tiles.
//!
//! # Design
//!
//! Follows cesium-native's two-phase model:
//!
//! ```text
//! AssetAccessor::get(url)                   ← returns Task immediately
//!   .then(Context::BACKGROUND, ...)         ← worker: status check, format detect
//!   → PrepareRendererResources::prepare_in_load_thread(bytes)
//!                                           ← worker thread (parse / decompress)
//!   .then(Context::MAIN, ...)               ← main thread (GPU upload)
//!   → PrepareRendererResources::prepare_in_main_thread(worker_result)
//!   → Payload::Renderable(C)               ← stored by the engine
//! ```
//!
//! Tile format detection (b3dm, i3dm, cmpt, pnts, glb, tileset.json) is done
//! by inspecting the URL extension and the first four bytes of the response.

use std::sync::Arc;

use egaku::PrepareRendererResources;
use orkester::{CancellationToken, Context, Task};
use orkester_io::{AssetAccessor, AssetResponse, RequestPriority, resolve_url};
use selekt::{
    ContentKey, ContentLoader, NodeContent, NodeId, SceneGraph, SceneRef, SelectionEngineBuilder,
};

use crate::GeometricErrorEvaluator;
use crate::hierarchy::{
    ExplicitTilesetHierarchy, ImplicitOctreeHierarchy, ImplicitQuadtreeHierarchy,
};
use tiles3d::implicit_tiling_utilities as ImplicitTilingUtilities;
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

/// Build the URL for the root subtree file (level=0, x=0, y=0[, z=0]).
///
/// `uri_template` is the `subtrees.uri` field from an `ImplicitTiling`
/// descriptor, e.g. `"subtrees/{level}/{x}/{y}.subtree"`.
fn resolve_root_subtree_url(
    base_url: &str,
    uri_template: &str,
    scheme: SubdivisionScheme,
) -> String {
    let expanded = match scheme {
        SubdivisionScheme::Quadtree => ImplicitTilingUtilities::resolve_url_quad(
            base_url,
            uri_template,
            QuadtreeTileID::new(0, 0, 0),
        ),
        SubdivisionScheme::Octree => ImplicitTilingUtilities::resolve_url_oct(
            base_url,
            uri_template,
            OctreeTileID::new(0, 0, 0, 0),
        ),
    };
    expanded
}

/// Async factory that fetches and parses `tileset.json`, builds an
/// [`ExplicitTilesetHierarchy`], and constructs a [`TilesetLoader`].
///
/// # Type parameters
///
/// * `R` — A [`PrepareRendererResources`] implementation. The loader drives
///   the two phases automatically.
pub struct TilesetLoaderFactory<R>
where
    R: PrepareRendererResources,
{
    /// Absolute URL of `tileset.json`.
    pub tileset_url: String,
    /// HTTP headers forwarded to every request issued by the loader.
    pub headers: Vec<(String, String)>,
    /// Maximum screen-space error threshold (pixels).
    pub maximum_screen_space_error: f64,
    /// The renderer resource preparer shared with the tile loader.
    pub preparer: Arc<R>,
}

impl<R> TilesetLoaderFactory<R>
where
    R: PrepareRendererResources,
{
    /// Create a factory for the given tileset URL with default SSE (16 px).
    pub fn new(tileset_url: impl Into<String>, preparer: Arc<R>) -> Self {
        Self {
            tileset_url: tileset_url.into(),
            headers: Vec::new(),
            maximum_screen_space_error: 16.0,
            preparer,
        }
    }

    /// Set custom request headers (e.g. `Authorization`).
    pub fn with_headers(mut self, headers: Vec<(String, String)>) -> Self {
        self.headers = headers;
        self
    }

    /// Override the maximum screen-space error threshold.
    pub fn with_maximum_screen_space_error(mut self, sse: f64) -> Self {
        self.maximum_screen_space_error = sse;
        self
    }
}

impl<R> TilesetLoaderFactory<R>
where
    R: PrepareRendererResources + 'static,
    R::WorkerResult: Send + 'static,
    R::Content: Send + 'static,
    R::Error: std::error::Error + Send + Sync + 'static,
{
    pub fn create(
        self,
        bg_context: Context,
        asset_accessor: &Arc<dyn AssetAccessor>,
    ) -> Task<Result<(SelectionEngineBuilder<R::Content>, Option<Arc<str>>), Tiles3dError>> {
        let url: Arc<str> = self.tileset_url.into();
        let headers: Arc<[(String, String)]> = self.headers.into();
        let sse = self.maximum_screen_space_error;
        debug_assert!(
            sse > 0.0,
            "maximum_screen_space_error must be positive, got {sse}"
        );
        let preparer = self.preparer;
        // Single clone of accessor — captured by move into the background closure.
        // url and headers are also moved in after .get() borrows them (NLL).
        let accessor = Arc::clone(asset_accessor);
        let bg_context_clone = bg_context.clone();

        // Kick off the tileset.json fetch immediately.
        asset_accessor
            .get(&url, &headers, RequestPriority::HIGH)
            // Phase 1 (background): parse tileset.json; branch on implicit/explicit.
            // Returns a Task so the implicit branch can chain a second async fetch.
            .then(
                &bg_context,
                move |io_result: Result<AssetResponse, std::io::Error>| -> Task<
                    Result<(SelectionEngineBuilder<R::Content>, Option<Arc<str>>), Tiles3dError>,
                > {
                    // Parse tileset.json
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

                    let lod = GeometricErrorEvaluator::new(sse);
                    // Extract copyright before tileset is partially moved below.
                    let attribution: Option<Arc<str>> = tileset
                        .asset
                        .copyright
                        .as_deref()
                        .filter(|s| !s.is_empty())
                        .map(Arc::from);
                    // Each of loader and resolver needs its own Arc reference.
                    // accessor/url/headers are moved into this closure (0 extra pre-clones);
                    // one clone goes to TilesetLoader and the original is moved into
                    // fetch_implicit (implicit path) or dropped (explicit path).
                    let loader = TilesetLoader::new(
                        Arc::clone(&accessor),
                        Arc::clone(&url),
                        Arc::clone(&headers),
                        Arc::clone(&preparer),
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
                            bg_context_clone,
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
                            lod,
                            loader,
                            attribution,
                        )
                    } else {
                        // Explicit tileset
                        let hierarchy: Box<dyn SceneGraph> =
                            Box::new(ExplicitTilesetHierarchy::from_tileset(&tileset));
                        let config = SelectionEngineBuilder::new(
                            bg_context_clone.clone(),
                            hierarchy,
                            lod,
                            loader,
                        );
                        orkester::resolved(Ok((config, attribution)))
                    }
                },
            )
    }
}

/// Fetch the root subtree and build an implicit hierarchy loader result.
#[allow(clippy::too_many_arguments)]
fn fetch_implicit_subtree_and_build<R>(
    bg_context: Context,
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
    lod: GeometricErrorEvaluator,
    loader: TilesetLoader<R>,
    attribution: Option<Arc<str>>,
) -> Task<Result<(SelectionEngineBuilder<R::Content>, Option<Arc<str>>), Tiles3dError>>
where
    R: PrepareRendererResources + 'static,
    R::WorkerResult: Send + 'static,
    R::Content: Send + 'static,
    R::Error: std::error::Error + Send + Sync + 'static,
{
    loader_accessor
        .get(&subtree_url, &loader_headers, RequestPriority::HIGH)
        .then(
            &bg_context.clone(),
            move |subtree_io: Result<AssetResponse, std::io::Error>| {
                parse_subtree_and_build(
                    bg_context,
                    subtree_io,
                    scheme,
                    subtree_levels,
                    available_levels,
                    root_bv,
                    root_geometric_error,
                    implicit,
                    content_template,
                    use_add,
                    lod,
                    loader,
                    attribution,
                )
            },
        )
}

/// Parse a fetched subtree response and assemble the loader factory result.
#[allow(clippy::too_many_arguments)]
fn parse_subtree_and_build<R>(
    bg_context: Context,
    subtree_io: Result<AssetResponse, std::io::Error>,
    scheme: SubdivisionScheme,
    subtree_levels: u32,
    available_levels: u32,
    root_bv: BoundingVolume,
    root_geometric_error: f64,
    implicit: ImplicitTiling,
    content_template: String,
    use_add: bool,
    lod: GeometricErrorEvaluator,
    loader: TilesetLoader<R>,
    attribution: Option<Arc<str>>,
) -> Result<(SelectionEngineBuilder<R::Content>, Option<Arc<str>>), Tiles3dError>
where
    R: PrepareRendererResources + 'static,
    R::WorkerResult: Send + 'static,
    R::Content: Send + 'static,
    R::Error: std::error::Error + Send + Sync + 'static,
{
    let sub_resp = subtree_io.map_err(Tiles3dError::from)?;
    sub_resp.check_status().map_err(Tiles3dError::Http)?;
    let subtree_av = parse_subtree(sub_resp.decompressed_data(), scheme, subtree_levels)
        .map_err(Tiles3dError::Subtree)?;

    let hierarchy: Box<dyn SceneGraph> = match scheme {
        SubdivisionScheme::Quadtree => {
            let mut qa = QuadtreeAvailability::new(subtree_levels, available_levels);
            qa.add_subtree(QuadtreeTileID::new(0, 0, 0), subtree_av);
            Box::new(ImplicitQuadtreeHierarchy::new(
                &root_bv,
                root_geometric_error,
                &implicit,
                qa,
                &content_template,
                use_add,
            ))
        }
        SubdivisionScheme::Octree => {
            let mut oa = OctreeAvailability::new(subtree_levels, available_levels);
            oa.add_subtree(OctreeTileID::new(0, 0, 0, 0), subtree_av);
            Box::new(ImplicitOctreeHierarchy::new(
                &root_bv,
                root_geometric_error,
                &implicit,
                oa,
                &content_template,
                use_add,
            ))
        }
    };
    Ok((
        SelectionEngineBuilder::new(bg_context, hierarchy, lod, loader),
        attribution,
    ))
}

/// Pure decode + prepare pipeline for a single 3D Tiles tile.
///
/// Owns the [`PrepareRendererResources`] implementation and exposes the two
/// pipeline phases as named methods.  Separated from [`TilesetLoader`] so that
/// the decode logic can be tested without a network accessor — pass raw bytes
/// directly to [`TileContentDecoder::worker`].
///
/// # Phases
///
/// 1. [`worker`] — called on a background thread: detects format, decodes
///    bytes, calls `PrepareRendererResources::prepare_in_load_thread`.
/// 2. [`main`] — called on the main thread: calls
///    `PrepareRendererResources::prepare_in_main_thread` and wraps the result
///    in [`LoadedContent`].
///
/// [`worker`]: TileContentDecoder::worker
/// [`main`]: TileContentDecoder::main
pub struct TileContentDecoder<R>
where
    R: PrepareRendererResources,
{
    preparer: Arc<R>,
}

/// Intermediate result of the worker-thread decode phase.
pub enum TileDecoded<W> {
    /// Tile content ready for main-thread GPU upload.
    Decoded { result: W, byte_size: usize },
    /// No renderable content (empty body, unknown format).
    Empty,
}

impl<R> TileContentDecoder<R>
where
    R: PrepareRendererResources,
    R::WorkerResult: Send + 'static,
    R::Error: std::error::Error + Send + Sync + 'static,
{
    /// Create a decoder backed by `preparer`.
    pub fn new(preparer: Arc<R>) -> Self {
        Self { preparer }
    }

    /// Worker-thread phase.
    ///
    /// Detects the tile format from `url` and `response.data`, decodes the
    /// bytes, and calls `PrepareRendererResources::prepare_in_load_thread`.
    ///
    /// JSON tiles (external tileset references) are **not** handled here —
    /// they are intercepted by [`TilesetLoader::load`] before calling this
    /// method.
    ///
    /// Returns:
    /// - `TileDecoded::Decoded` — tile content ready for main-thread upload.
    /// - `TileDecoded::Empty` — no renderable content.
    pub fn worker(
        &self,
        _node_id: NodeId,
        url: &str,
        response: AssetResponse,
    ) -> Result<TileDecoded<R::WorkerResult>, Tiles3dError> {
        if response.decompressed_data().is_empty() {
            return Ok(TileDecoded::Empty);
        }
        let format = TileFormat::detect(url, response.decompressed_data());
        let byte_size = response.decompressed_data().len();
        match decode_tile(response.decompressed_data(), &format) {
            Some(model) => {
                let result = self
                    .preparer
                    .prepare_in_load_thread(model)
                    .map_err(|e| Tiles3dError::Decode(Box::new(e)))?;
                Ok(TileDecoded::Decoded { result, byte_size })
            }
            None => Ok(TileDecoded::Empty),
        }
    }

    /// Main-thread phase.
    ///
    /// Calls `PrepareRendererResources::prepare_in_main_thread` and returns
    /// a [`NodeContent`] containing the prepared GPU resource.
    pub fn main(
        &self,
        decode_out: TileDecoded<R::WorkerResult>,
    ) -> Result<NodeContent<R::Content>, Tiles3dError>
    where
        R::Content: Send + 'static,
    {
        match decode_out {
            TileDecoded::Decoded { result, byte_size } => Ok(NodeContent::renderable(
                self.preparer.prepare_in_main_thread(result),
                byte_size,
            )),
            TileDecoded::Empty => Ok(NodeContent::empty()),
        }
    }
}

/// Async content loader for 3D Tiles tile content.
///
/// Issues one HTTP request per tile and drives [`TileContentDecoder`] through
/// its two phases (worker then main).  All decode and prepare logic lives in
/// [`TileContentDecoder`]; this struct is responsible only for request
/// lifecycle: URL resolution, fetch, and cancellation.
pub struct TilesetLoader<R>
where
    R: PrepareRendererResources,
{
    accessor: Arc<dyn AssetAccessor>,
    /// Absolute base URL of the root tileset (resolves relative content keys).
    base_url: Arc<str>,
    headers: Arc<[(String, String)]>,
    decoder: Arc<TileContentDecoder<R>>,
}

impl<R> TilesetLoader<R>
where
    R: PrepareRendererResources,
{
    pub(crate) fn new(
        accessor: Arc<dyn AssetAccessor>,
        base_url: impl Into<Arc<str>>,
        headers: impl Into<Arc<[(String, String)]>>,
        preparer: Arc<R>,
    ) -> Self {
        Self {
            accessor,
            base_url: base_url.into(),
            headers: headers.into(),
            decoder: Arc::new(TileContentDecoder::new(preparer)),
        }
    }
}

impl<R> ContentLoader<R::Content> for TilesetLoader<R>
where
    R: PrepareRendererResources + 'static,
    R::WorkerResult: Send + 'static,
    R::Content: Send + 'static,
    R::Error: std::error::Error + Send + Sync + 'static,
{
    type Error = Tiles3dError;

    fn load(
        &self,
        bg_context: &Context,
        main_context: &Context,
        node_id: NodeId,
        key: &ContentKey,
        parent_world_transform: glam::DMat4,
        cancel: CancellationToken,
    ) -> Task<Result<NodeContent<R::Content>, Self::Error>> {
        let main_context = main_context.clone();
        let url: Arc<str> = resolve_url(&self.base_url, &key.0).into();
        let decoder = Arc::clone(&self.decoder);
        let accessor_clone = Arc::clone(&self.accessor);
        let headers_clone = Arc::clone(&self.headers);
        let priority = RequestPriority(128);
        self.accessor
            .get(&url, &self.headers, priority)
            .with_cancellation(&cancel)
            .then(
                bg_context,
                move |io_result: Result<AssetResponse, std::io::Error>| -> Task<Result<NodeContent<R::Content>, Tiles3dError>> {
                    let response = match io_result {
                        Ok(r) => r,
                        Err(e) => return orkester::resolved(Err(Tiles3dError::from(e))),
                    };
                    if let Err(code) = response.check_status() {
                        return orkester::resolved(Err(Tiles3dError::Http(code)));
                    }
                    let data = response.decompressed_data();
                    if data.is_empty() {
                        return orkester::resolved(Ok(NodeContent::empty()));
                    }

                    // Detect external tileset reference (JSON content).
                    if TileFormat::detect(&url, data) == TileFormat::Json {
                        let byte_size = data.len();
                        let tileset: Tileset = match serde_json::from_slice(data) {
                            Ok(t) => t,
                            Err(e) => return orkester::resolved(Err(Tiles3dError::from(e))),
                        };
                        // Build a sub-scene: the child hierarchy + a new loader
                        // with the child's URL as the base for relative content keys.
                        // Pass the reference node's world transform so that sub-tileset
                        // tile transforms are accumulated on top of it (mirrors cesium-native
                        // passing tileTransform to parseTilesetJson for external tilesets).
                        let child_hierarchy = ExplicitTilesetHierarchy::from_tileset_with_root_transform(&tileset, parent_world_transform);
                        if child_hierarchy.node_count() == 0 {
                            return orkester::resolved(Ok(NodeContent::empty()));
                        }
                        let child_loader = TilesetLoader {
                            accessor: accessor_clone,
                            base_url: Arc::clone(&url),
                            headers: headers_clone,
                            decoder: Arc::clone(&decoder),
                        };
                        let scene_ref = SceneRef {
                            graph: Box::new(child_hierarchy),
                            loader: Box::new(child_loader),
                            byte_size,
                        };
                        return orkester::resolved(Ok(NodeContent::scene_ref(scene_ref)));
                    }

                    // Normal tile: decode on worker, upload on main thread.
                    let out = match decoder.worker(node_id, &url, response) {
                        Ok(o) => o,
                        Err(e) => return orkester::resolved(Err(e)),
                    };
                    orkester::resolved(Ok(out))
                        .then(&main_context, move |worker_out: Result<TileDecoded<R::WorkerResult>, Tiles3dError>| {
                            decoder.main(worker_out?)
                        })
                },
            )
    }
}
