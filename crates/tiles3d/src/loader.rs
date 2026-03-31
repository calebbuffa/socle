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

use std::collections::HashMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

use orkester::{CancellationToken, Context, Task};
use orkester_io::{AssetAccessor, AssetResponse, RequestPriority, resolve_url};
use selekt::{
    ContentKey, ContentLoader, ContentLoaderFactory, ContentLoaderFactoryResult, DecodeOutput,
    GeometricErrorEvaluator, HierarchyReference, LoadPriority, LoadedContent, NodeId, Payload,
    PriorityGroup, RequestId, SpatialHierarchy,
};
use egaku::PrepareRendererResources;

use crate::Tileset;
use crate::availability::{OctreeAvailability, QuadtreeAvailability, SubdivisionScheme};
use crate::decoder::decode_tile;
use crate::format::Tiles3dFormat;
use crate::hierarchy::{
    ExplicitTilesetHierarchy, ImplicitOctreeHierarchy, ImplicitQuadtreeHierarchy,
};
use crate::implicit_tiling_utilities::ImplicitTilingUtilities;
use crate::subtree::parse_subtree;
use crate::tile::{OctreeTileID, QuadtreeTileID};

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
    Subtree(#[from] crate::subtree::SubtreeParseError),
    /// The `subdivisionScheme` value in `implicitTiling` was not recognised.
    #[error("unknown subdivisionScheme '{0}'")]
    UnknownSubdivisionScheme(String),
}

fn to_request_priority(p: LoadPriority) -> RequestPriority {
    let base: u8 = match p.group {
        PriorityGroup::Preload => 64,
        PriorityGroup::Normal => 128,
        PriorityGroup::Urgent => 220,
    };
    RequestPriority(base)
}

/// Binary tile format detected from the URL or magic bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileFormat {
    /// glTF binary blob (magic `glTF`).
    Glb,
    /// Batched 3D Model (magic `b3dm`).
    B3dm,
    /// Instanced 3D Model (magic `i3dm`).
    I3dm,
    /// Composite (magic `cmpt`).
    Cmpt,
    /// Point cloud (magic `pnts`).
    Pnts,
    /// External tileset (JSON).
    Json,
    /// Unknown / unrecognised.
    Unknown,
}

impl TileFormat {
    /// Detect the format from the first four bytes of the response body.
    /// Falls back to URL-based detection when the magic is not recognised.
    pub fn detect(url: &str, data: &[u8]) -> Self {
        if data.len() >= 4 {
            match &data[..4] {
                b"glTF" => return Self::Glb,
                b"b3dm" => return Self::B3dm,
                b"i3dm" => return Self::I3dm,
                b"cmpt" => return Self::Cmpt,
                b"pnts" => return Self::Pnts,
                _ => {}
            }
        }
        // Fallback to extension.
        let path = url.split('?').next().unwrap_or(url);
        match path
            .rsplit('.')
            .next()
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("glb") => Self::Glb,
            Some("b3dm") => Self::B3dm,
            Some("i3dm") => Self::I3dm,
            Some("cmpt") => Self::Cmpt,
            Some("pnts") => Self::Pnts,
            Some("json") => Self::Json,
            _ => Self::Unknown,
        }
    }
}

/// Intermediate value flowing from worker-phase to main-thread phase.
///
/// Using an explicit enum avoids boxing and keeps the type honest about
/// what the main thread will receive.

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

impl<R> ContentLoaderFactory for TilesetLoaderFactory<R>
where
    R: PrepareRendererResources + 'static,
    R::WorkerResult: Send + 'static,
    R::Content: Send + 'static,
    R::Error: std::error::Error + Send + Sync + 'static,
{
    type Format = Tiles3dFormat<R>;
    type Error = Tiles3dError;

    fn create_loader(
        self,
        bg_context: &Context,
        asset_accessor: &Arc<dyn AssetAccessor>,
    ) -> Task<Result<ContentLoaderFactoryResult<Self::Format>, Self::Error>> {
        let url: Arc<str> = self.tileset_url.into();
        let headers: Arc<[(String, String)]> = self.headers.into();
        let sse = self.maximum_screen_space_error;
        let preparer = self.preparer;
        let loader_accessor = Arc::clone(asset_accessor);
        let loader_url = Arc::clone(&url);
        let loader_headers = Arc::clone(&headers);
        // Clone bg_context so the task closure can produce ready-tasks for
        // error paths without needing an external channel.
        let bg_context = bg_context.clone();

        // Kick off the tileset.json fetch immediately.
        asset_accessor
            .get(&url, &headers, RequestPriority::HIGH)
            // Phase 1 (background): parse tileset.json; branch on implicit/explicit.
            // Returns a Task so the implicit branch can chain a second async fetch.
            .then(
                bg_context.clone(),
                move |io_result: Result<AssetResponse, std::io::Error>| -> Task<
                    Result<ContentLoaderFactoryResult<Tiles3dFormat<R>>, Tiles3dError>,
                > {
                    // ── Parse tileset.json ────────────────────────────────────
                    let response = match io_result {
                        Ok(r) => r,
                        Err(e) => return orkester::resolved(Err(Tiles3dError::from(e))),
                    };
                    if let Err(code) = response.check_status() {
                        return orkester::resolved(Err(Tiles3dError::Http(code)));
                    }
                    let tileset: Tileset = match serde_json::from_slice(response.decompressed_data()) {
                        Ok(t) => t,
                        Err(e) => return orkester::resolved(Err(Tiles3dError::from(e))),
                    };

                    let lod = GeometricErrorEvaluator::new(sse);
                    let loader = TilesetLoader::new(
                        Arc::clone(&loader_accessor),
                        Arc::clone(&loader_url),
                        Arc::clone(&loader_headers),
                        Arc::clone(&preparer),
                    );

                    // ── Detect implicit tiling ────────────────────────────────
                    if let Some(implicit) = tileset.root.implicit_tiling.clone() {
                        let scheme = implicit.subdivision_scheme;
                        let subtree_levels = implicit.subtree_levels as u32;
                        let available_levels = implicit.available_levels as u32;
                        let root_bv = tileset.root.bounding_volume.clone();
                        let root_geometric_error = tileset.root.geometric_error;
                        let use_add = matches!(
                            tileset.root.refine.as_ref().and_then(|v| v.as_str()),
                            Some("ADD")
                        );
                        // Content URI template lives on the root tile's content.
                        let content_template = tileset
                            .root
                            .content
                            .as_ref()
                            .map(|c| c.uri.clone())
                            .or_else(|| tileset.root.contents.first().map(|c| c.uri.clone()))
                            .unwrap_or_default();

                        // URL for the root subtree file: level=0, x=0, y=0 [, z=0].
                        let subtree_url =
                            resolve_root_subtree_url(&loader_url, &implicit.subtrees.uri, scheme);

                        // Phase 1b (background): fetch + parse root subtree.
                        loader_accessor.get(&subtree_url, &loader_headers, RequestPriority::HIGH).then(
                            bg_context.clone(),
                            move |subtree_io: Result<AssetResponse, std::io::Error>| -> Result<
                                ContentLoaderFactoryResult<Tiles3dFormat<R>>,
                                Tiles3dError,
                            > {
                                let sub_resp = subtree_io.map_err(Tiles3dError::from)?;
                                sub_resp.check_status().map_err(Tiles3dError::Http)?;

                                let subtree_av =
                                    parse_subtree(sub_resp.decompressed_data(), scheme, subtree_levels)
                                        .map_err(Tiles3dError::Subtree)?;;

                                let hierarchy: Box<dyn SpatialHierarchy> = match scheme {
                                    SubdivisionScheme::Quadtree => {
                                        let mut qa = QuadtreeAvailability::new(
                                            subtree_levels,
                                            available_levels,
                                        );
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
                                        let mut oa = OctreeAvailability::new(
                                            subtree_levels,
                                            available_levels,
                                        );
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

                                Ok(ContentLoaderFactoryResult::new(hierarchy, lod, loader))
                            },
                        )
                    } else {
                        // ── Explicit tileset ──────────────────────────────────
                        let hierarchy: Box<dyn SpatialHierarchy> =
                            Box::new(ExplicitTilesetHierarchy::from_tileset(&tileset));
                        orkester::resolved(Ok(ContentLoaderFactoryResult::new(hierarchy, lod, loader)))
                    }
                },
            )
    }
}

/// Async content loader for 3D Tiles tile content.
///
/// Issues one HTTP request per tile, detects the tile format, and drives
/// [`PrepareRendererResources`] through its two phases (worker then main).
///
/// The in-flight table uses `Arc<Mutex<...>>` so that cancellation tokens
/// can be reached from both `cancel()` and the task continuation cleanup.
pub struct TilesetLoader<R>
where
    R: PrepareRendererResources,
{
    accessor: Arc<dyn AssetAccessor>,
    /// Absolute base URL of the root tileset (resolves relative content keys).
    base_url: Arc<str>,
    headers: Arc<[(String, String)]>,
    preparer: Arc<R>,
    /// Active requests. `Arc` lets task closures reach the map for cleanup.
    in_flight: Arc<Mutex<HashMap<RequestId, CancellationToken>>>,
    next_id: AtomicU64,
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
            preparer,
            in_flight: Arc::new(Mutex::new(HashMap::new())),
            next_id: AtomicU64::new(1),
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

    fn request(
        &self,
        _bg_context: &Context,
        main_context: &Context,
        node_id: NodeId,
        key: &ContentKey,
        priority: LoadPriority,
    ) -> (
        RequestId,
        Task<Result<LoadedContent<R::Content>, Self::Error>>,
    ) {
        let request_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let main_context = main_context.clone();
        let token = CancellationToken::new();
        self.in_flight
            .lock()
            .unwrap()
            .insert(request_id, token.clone());

        let url: Arc<str> = resolve_url(&self.base_url, &key.0).into();
        let in_flight = Arc::clone(&self.in_flight);
        let preparer_worker = Arc::clone(&self.preparer);
        let preparer_main = Arc::clone(&self.preparer);
        let url_worker = Arc::clone(&url);

        // Phase 1: fetch (immediate).
        // `AssetAccessor::get` returns a Task immediately; no threads blocked.
        // `.with_cancellation` races the entire chain against `token` so that
        // `cancel()` immediately rejects the task without waiting for the network.
        let task = self
            .accessor
            .get(&url, &self.headers, to_request_priority(priority))
            .with_cancellation(&token)
            // Phase 2: worker — status check, format detect, decode
            .then(
                Context::BACKGROUND,
                move |io_result: Result<AssetResponse, std::io::Error>| {
                    // Clean up regardless of success/failure.
                    in_flight.lock().unwrap().remove(&request_id);

                    let response = io_result.map_err(Tiles3dError::from)?;
                    response.check_status().map_err(Tiles3dError::Http)?;

                    if response.decompressed_data().is_empty() {
                        return Ok(DecodeOutput::Empty);
                    }

                    let format = TileFormat::detect(&url_worker, response.decompressed_data());

                    if format == TileFormat::Json {
                        // External tileset — no GPU work needed.
                        let byte_size = response.decompressed_data().len();
                        return Ok(DecodeOutput::Reference(
                            HierarchyReference {
                                key: ContentKey(url_worker.to_string()),
                                source: node_id,
                                transform: None,
                            },
                            byte_size,
                        ));
                    }

                    let byte_size = response.decompressed_data().len();
                    match decode_tile(response.decompressed_data(), &format) {
                        Some(model) => {
                            let result = preparer_worker
                                .prepare_in_load_thread(model)
                                .map_err(|e| Tiles3dError::Decode(Box::new(e)))?;
                            Ok(DecodeOutput::Decoded { result, byte_size })
                        }
                        None => Ok(DecodeOutput::Empty),
                    }
                },
            )
            // Phase 3: main thread — GPU upload
            .then(
                main_context,
                move |worker_out: Result<DecodeOutput<R::WorkerResult>, Tiles3dError>| {
                    let loaded = match worker_out? {
                        DecodeOutput::Decoded { result, byte_size } => {
                            let content = preparer_main.prepare_in_main_thread(result);
                            LoadedContent {
                                payload: Payload::Renderable(content),
                                byte_size,
                            }
                        }
                        DecodeOutput::Reference(reference, byte_size) => LoadedContent {
                            payload: Payload::Reference(reference),
                            byte_size,
                        },
                        DecodeOutput::Empty => LoadedContent {
                            payload: Payload::Empty,
                            byte_size: 0,
                        },
                    };
                    Ok::<_, Tiles3dError>(loaded)
                },
            );

        (request_id, task)
    }

    fn cancel(&self, request_id: RequestId) -> bool {
        let mut guard = self.in_flight.lock().unwrap();
        if let Some(token) = guard.remove(&request_id) {
            token.cancel();
            true
        } else {
            false
        }
    }
}