//! Top-level scene layer: entry point for loading and selecting I3S nodes.
//!
//! Call [`update_view`](SceneLayer::update_view) each frame for LOD selection,
//! then [`load_nodes`](SceneLayer::load_nodes) to dispatch content fetches and
//! finalize loaded content on the main thread.
//!
//! ## Async open model
//!
//! [`SceneLayer::open`] returns **immediately** (never blocks). Fetching and
//! parsing the layer document and first node page happens on the
//! [`TaskProcessor`]'s worker thread pool. Poll [`root_available`] to know
//! when the layer is ready. Until then, [`update_view`] and [`load_nodes`]
//! are no-ops.
//!
//! [`root_available`]: SceneLayer::root_available
//! [`TaskProcessor`]: i3s_async::TaskProcessor

use std::collections::HashSet;
use std::sync::Arc;

use glam::{DQuat, DVec3};

use i3s::core::{SceneLayerInfo, SceneLayerInfoPsl, SceneLayerType};
use i3s::node::NodePageDefinitionLodSelectionMetricType;
use i3s::pointcloud::PointCloudLayer;

use i3s_async::{AssetAccessor, AsyncError, ResourceUriResolver, SharedFuture};
use i3s_geometry::obb::OrientedBoundingBox;
use i3s_geospatial::crs::{self, CrsTransform, SceneCoordinateSystem};
use i3s_geospatial::ellipsoid::Ellipsoid;
use i3s_reader::codec::PassthroughDecoder;
use i3s_reader::geometry::GeometryLayout;
use i3s_reader::json::read_json;

use crate::cache::NodeCache;
use crate::excluder::is_excluded;
use crate::externals::SceneLayerExternals;
use crate::layer_info::LayerInfo;
use crate::loader::{AttributeInfo, NodeContentLoader};
use crate::node_state::{NodeLoadState, NodeState};
use crate::node_tree::{NodeTree, PagedNodeTree, PointCloudNodeTree};
use crate::options::SelectionOptions;
use crate::prepare::RendererResources;
use crate::selection::{LodMetric, select_nodes};
use crate::update_result::ViewUpdateResult;
use crate::view_state::ViewState;

/// A node selected for rendering, with its bounding volume and renderer resources.
pub struct RenderNode<'a> {
    pub node_id: u32,
    pub center: DVec3,
    pub quaternion: DQuat,
    pub half_size: DVec3,
    pub bounding_radius: f64,
    pub renderer_resources: Option<&'a RendererResources>,
}

// ---------------------------------------------------------------------------
// Ready state — everything that only exists once bootstrap completes
// ---------------------------------------------------------------------------

struct ReadyState {
    info: LayerInfo,
    node_tree: NodeTree,
    node_states: Vec<NodeState>,
    lod_metric: LodMetric,
    crs: SceneCoordinateSystem,
    crs_transform: Option<Arc<dyn CrsTransform>>,
    frame: u64,
    loader: NodeContentLoader,
    cache: NodeCache,
    page_sender: crossbeam_channel::Sender<PageFetchResult>,
    page_receiver: crossbeam_channel::Receiver<PageFetchResult>,
    pages_in_flight: HashSet<u32>,
}

// SAFETY: all fields are Send (Arc<dyn Trait> where Trait: Send+Sync, plain data)
unsafe impl Send for ReadyState {}

// ---------------------------------------------------------------------------
// SceneLayer
// ---------------------------------------------------------------------------

/// An I3S scene layer backed by an [`AssetAccessor`].
///
/// Created via [`open`](SceneLayer::open) with an accessor, URI resolver,
/// and [`SceneLayerExternals`]. **Open returns immediately** — the layer
/// document and first node page are fetched on worker threads.
///
/// Poll [`root_available`](SceneLayer::root_available) to know when the
/// layer is usable. `update_view` and `load_nodes` silently no-op until
/// then.
///
/// Each frame once ready:
/// 1. Call [`update_view`](SceneLayer::update_view) (sync) — LOD selection
/// 2. Call [`load_nodes`](SceneLayer::load_nodes) — dispatch loads,
///    collect completed, run `prepare_in_main_thread`
pub struct SceneLayer {
    /// The asset accessor (unified HTTP + SLPK).
    accessor: Arc<dyn AssetAccessor>,
    /// URI resolver for I3S resources.
    resolver: Arc<dyn ResourceUriResolver>,
    /// External dependencies (task processor, renderer).
    externals: SceneLayerExternals,
    /// Selection options (may be mutated freely by the caller).
    pub options: SelectionOptions,
    /// State that is `None` until the async bootstrap resolves.
    ready: Option<ReadyState>,
    /// One-shot channel delivering the bootstrap result from the worker thread.
    ready_receiver: Option<crossbeam_channel::Receiver<Result<ReadyState, AsyncError>>>,
    /// Resolves (on the worker side) when the layer document and node page 0
    /// have been fetched and parsed. Mirror of `Tileset::getRootTileAvailableEvent`.
    pub root_available: SharedFuture<()>,
}

/// Result of an async node page fetch.
struct PageFetchResult {
    page_id: u32,
    data: Result<Vec<u8>, AsyncError>,
}

impl SceneLayer {
    /// Open a scene layer from an accessor and URI resolver.
    ///
    /// **Returns immediately** — the layer document and node page 0 are fetched
    /// on the worker thread pool. Poll [`root_available`](Self::root_available)
    /// to know when the layer is usable.
    ///
    /// Supports 3DObject, IntegratedMesh, Point, and PointCloud layers.
    /// For Building layers use [`BuildingSceneLayer`](crate::building::BuildingSceneLayer).
    pub fn open(
        accessor: impl AssetAccessor + 'static,
        resolver: Arc<dyn ResourceUriResolver>,
        externals: SceneLayerExternals,
        options: SelectionOptions,
    ) -> Self {
        Self::open_shared(Arc::new(accessor), resolver, externals, options, None)
    }

    /// Like [`open`](Self::open), but with a [`CrsTransform`] for local layers.
    ///
    /// When provided, OBBs from local/projected layers are transformed to ECEF,
    /// enabling a unified ECEF [`ViewState`] regardless of the layer's native CRS.
    /// For global layers (WKID 4326/4490) the transform is ignored.
    pub fn open_with_transform(
        accessor: impl AssetAccessor + 'static,
        resolver: Arc<dyn ResourceUriResolver>,
        externals: SceneLayerExternals,
        options: SelectionOptions,
        crs_transform: Arc<dyn CrsTransform>,
    ) -> Self {
        Self::open_shared(
            Arc::new(accessor),
            resolver,
            externals,
            options,
            Some(crs_transform),
        )
    }

    /// Like [`open`](Self::open), but takes a pre-shared `Arc<dyn AssetAccessor>`.
    ///
    /// Used internally by [`BuildingSceneLayer`](crate::building::BuildingSceneLayer)
    /// to share one accessor across all sublayers.
    pub fn open_shared(
        accessor: Arc<dyn AssetAccessor>,
        resolver: Arc<dyn ResourceUriResolver>,
        externals: SceneLayerExternals,
        options: SelectionOptions,
        crs_transform: Option<Arc<dyn CrsTransform>>,
    ) -> Self {
        let (ready_tx, ready_rx) = crossbeam_channel::bounded::<Result<ReadyState, AsyncError>>(1);

        // Create the promise/future pair for root_available
        let mut promise = externals.async_system.create_promise::<()>();
        let root_available = promise.future().share();

        // Spawn the bootstrap on a worker thread
        let acc = Arc::clone(&accessor);
        let res = Arc::clone(&resolver);
        let ext = externals.clone();
        let opts = options.clone();
        externals
            .async_system
            .task_processor()
            .start_task(Box::new(move || {
                let result = bootstrap_layer(&acc, &res, &ext, &opts, crs_transform);
                match result {
                    Ok(state) => {
                        // Send the state before resolving; poll_ready uses try_recv
                        let _ = ready_tx.send(Ok(state));
                        promise.resolve(());
                    }
                    Err(e) => {
                        let err = AsyncError::new(e);
                        let _ = ready_tx.send(Err(err.clone()));
                        promise.reject(err);
                    }
                }
            }));

        Self {
            accessor,
            resolver,
            externals,
            options,
            ready: None,
            ready_receiver: Some(ready_rx),
            root_available,
        }
    }

    /// Check the one-shot channel; if the bootstrap result has arrived, move it
    /// into `self.ready`. Call at the start of every method that needs ready state.
    fn poll_ready(&mut self) {
        if self.ready.is_some() {
            return;
        }
        if let Some(ref rx) = self.ready_receiver {
            match rx.try_recv() {
                Ok(Ok(state)) => {
                    self.ready = Some(state);
                    self.ready_receiver = None;
                }
                Ok(Err(_)) => {
                    // Bootstrap failed — leave ready as None, discard receiver
                    self.ready_receiver = None;
                }
                Err(_) => {} // not yet available
            }
        }
    }

    /// Returns `true` once the layer document and root node page are available.
    pub fn is_ready(&mut self) -> bool {
        self.poll_ready();
        self.ready.is_some()
    }

    /// Typed layer info document — `None` until the async bootstrap completes.
    pub fn info(&mut self) -> Option<&LayerInfo> {
        self.poll_ready();
        self.ready.as_ref().map(|r| &r.info)
    }

    /// Ensure a node page is loaded. Returns `true` if the page is available.
    pub fn ensure_node_page(&mut self, page_id: u32) -> Option<bool> {
        self.poll_ready();
        let r = self.ready.as_mut()?;

        if r.node_tree.page_loaded(page_id) {
            return Some(true);
        }

        let bytes = self
            .accessor
            .get(&self.resolver.node_page_uri(page_id))
            .and_then(|resp| resp.into_data())
            .ok()?;
        r.node_tree.insert_page(page_id, &bytes).ok()?;

        // Ensure node states cover the new page
        let needed = r.node_tree.node_count();
        while r.node_states.len() < needed {
            let id = r.node_states.len() as u32;
            r.node_states.push(NodeState::new(id));
        }

        Some(true)
    }

    /// Run per-frame LOD selection (sync — pure computation, no I/O).
    ///
    /// Accepts a slice of [`ViewState`]s. Multiple views support VR (one per
    /// eye) or shadow-map cascades. A node is rendered if *any* view selects
    /// it, and the LOD screen size is the maximum across all views.
    ///
    /// Returns an empty result (no-op) until the async bootstrap resolves.
    #[must_use]
    pub fn update_view(&mut self, views: &[ViewState]) -> ViewUpdateResult {
        self.poll_ready();
        let r = match self.ready.as_mut() {
            Some(r) => r,
            None => return ViewUpdateResult::default(),
        };

        r.frame += 1;
        let frame = r.frame;
        let metric = r.lod_metric;
        let layer_crs = r.crs;
        let crs_xform = r.crs_transform.as_deref();
        let node_tree = &r.node_tree;
        let nodes_per_page = node_tree.nodes_per_page();

        // Notify excluders of a new frame (mirrors ITileExcluder::startNewFrame)
        for exc in &mut self.externals.excluders {
            Arc::get_mut(exc).map(|e| e.start_new_frame());
        }
        let excluders = &self.externals.excluders;

        let mut result = select_nodes(
            &mut r.node_states,
            |node_id, out: &mut Vec<u32>| {
                node_tree.children_of(node_id, out);
            },
            |node_id| {
                let obb = crs::obb_from_spec(node_tree.obb_of(node_id)?, layer_crs, crs_xform);
                if is_excluded(excluders, &obb) {
                    return None;
                }
                Some(obb)
            },
            |node_id| node_tree.lod_threshold_of(node_id),
            0, // root
            views,
            metric,
            &self.options,
            frame,
            nodes_per_page,
            |page_id| node_tree.page_loaded(page_id),
        );
        result.frame_number = frame;
        result.worker_thread_load_queue_length = r.loader.in_flight_count() as u32;
        result.main_thread_load_queue_length = r.loader.pending_count() as u32;
        result
    }
    /// Process content loading: dispatch fetches, collect completed, finalize.
    ///
    /// Call this after [`update_view`](SceneLayer::update_view) each frame.
    /// Fully non-blocking — never waits for I/O. No-op until bootstrap resolves.
    ///
    /// 1. Collects completed page fetches from previous frames
    /// 2. Dispatches new page fetch requests to the TaskProcessor
    /// 3. Enqueues new content load requests to the TaskProcessor
    /// 4. Collects completed loads (sync channel drain)
    /// 5. Calls `prepare_in_main_thread` for newly loaded content
    /// 6. Handles eviction, unloading, and retry
    pub fn load_nodes(&mut self, result: &ViewUpdateResult) {
        self.poll_ready();
        if self.ready.is_none() {
            return;
        }

        // Collect any completed page fetches from workers
        self.collect_page_fetches();

        // ---- Page fetch dispatch ----
        {
            let r = self.ready.as_mut().unwrap();
            for &page_id in &result.pages_needed {
                if r.node_tree.page_loaded(page_id) || r.pages_in_flight.contains(&page_id) {
                    continue;
                }
                r.pages_in_flight.insert(page_id);
                let accessor = Arc::clone(&self.accessor);
                let resolver = Arc::clone(&self.resolver);
                let sender = r.page_sender.clone();
                self.externals
                    .async_system
                    .task_processor()
                    .start_task(Box::new(move || {
                        let uri = resolver.node_page_uri(page_id);
                        let data = accessor
                            .get(&uri)
                            .and_then(|resp| resp.into_data())
                            .map_err(|e| AsyncError::new(e));
                        let _ = sender.send(PageFetchResult { page_id, data });
                    }));
            }
        }

        // ---- Load request dispatch ----
        {
            let r = self.ready.as_mut().unwrap();
            for req in &result.load_requests {
                let idx = req.node_id as usize;
                if idx < r.node_states.len()
                    && r.node_states[idx].load_state == NodeLoadState::Unloaded
                {
                    r.node_states[idx].load_state = NodeLoadState::Loading;
                    r.loader.request(req.node_id, req.priority, req.screen_size);
                }
            }
            r.loader.dispatch();
        }

        // ---- Collect completed loads ----
        // Precompute the crs_transform Arc so we can call prepare_in_main_thread
        // without holding a &mut borrow on self.ready simultaneously.
        let crs_transform = self.ready.as_ref().unwrap().crs_transform.clone();

        let completed = {
            let r = self.ready.as_mut().unwrap();
            r.loader.collect_completed()
        };

        for load in completed {
            let idx = load.node_id as usize;
            if idx >= self.ready.as_ref().unwrap().node_states.len() {
                continue;
            }
            match load.result {
                Ok(ref content) => {
                    // Phase 3: prepare_in_main_thread runs on the main thread
                    let main_resources = self
                        .externals
                        .prepare_renderer_resources
                        .prepare_in_main_thread(
                            load.node_id,
                            content,
                            load.load_thread_resources,
                            crs_transform.as_ref(),
                        );

                    let r = self.ready.as_mut().unwrap();
                    r.node_states[idx].renderer_resources = main_resources;

                    let evicted = r.cache.insert(load.node_id, content.clone());
                    r.node_states[idx].load_state = NodeLoadState::Loaded;
                    for evicted_id in evicted {
                        let ei = evicted_id as usize;
                        if ei < r.node_states.len() {
                            r.node_states[ei].load_state = NodeLoadState::Unloaded;
                            let res = r.node_states[ei].renderer_resources.take();
                            self.externals
                                .prepare_renderer_resources
                                .free(evicted_id, res);
                        }
                    }
                }
                Err(_) => {
                    let r = self.ready.as_mut().unwrap();
                    r.node_states[idx].load_state = NodeLoadState::Failed;
                    r.node_states[idx].failed_attempts += 1;
                    r.node_states[idx].last_failed_frame = r.frame;
                }
            }
        }

        // ---- Unload and retry ----
        {
            let r = self.ready.as_mut().unwrap();
            for &node_id in &result.nodes_to_unload {
                let idx = node_id as usize;
                if idx < r.node_states.len() {
                    let res = r.node_states[idx].renderer_resources.take();
                    self.externals.prepare_renderer_resources.free(node_id, res);
                    r.cache.remove(node_id);
                    r.node_states[idx].load_state = NodeLoadState::Unloaded;
                }
            }
            let frame = r.frame;
            for state in r.node_states.iter_mut() {
                if state.can_retry(frame) {
                    state.load_state = NodeLoadState::Unloaded;
                }
            }
        }
    }

    /// Collect completed async page fetches and insert them into the node tree.
    ///
    /// Called at the start of [`load_nodes`](Self::load_nodes). Non-blocking —
    /// drains the page result channel without waiting.
    fn collect_page_fetches(&mut self) {
        let r = match self.ready.as_mut() {
            Some(r) => r,
            None => return,
        };

        let mut fetched = Vec::new();
        while let Ok(result) = r.page_receiver.try_recv() {
            fetched.push(result);
        }

        for fetch in fetched {
            r.pages_in_flight.remove(&fetch.page_id);
            if let Ok(bytes) = fetch.data {
                if r.node_tree.insert_page(fetch.page_id, &bytes).is_ok() {
                    let needed = r.node_tree.node_count();
                    while r.node_states.len() < needed {
                        let id = r.node_states.len() as u32;
                        r.node_states.push(NodeState::new(id));
                    }
                }
            }
            // Errors are silently dropped — page will be re-requested next frame
        }
    }

    /// Get the runtime state for a node. Returns `None` until bootstrap resolves.
    pub fn node_state(&mut self, node_id: u32) -> Option<&NodeState> {
        self.poll_ready();
        self.ready
            .as_ref()
            .and_then(|r| r.node_states.get(node_id as usize))
    }

    /// Get a mutable reference to the runtime state for a node.
    pub fn node_state_mut(&mut self, node_id: u32) -> Option<&mut NodeState> {
        self.poll_ready();
        self.ready
            .as_mut()
            .and_then(|r| r.node_states.get_mut(node_id as usize))
    }

    /// Get the coordinate reference system classification.
    /// Returns `Global` until bootstrap resolves.
    pub fn crs(&mut self) -> SceneCoordinateSystem {
        self.poll_ready();
        self.ready
            .as_ref()
            .map(|r| r.crs)
            .unwrap_or(SceneCoordinateSystem::Global)
    }

    /// Get the current frame number.
    pub fn frame(&mut self) -> u64 {
        self.poll_ready();
        self.ready.as_ref().map(|r| r.frame).unwrap_or(0)
    }

    /// Access the content cache.
    pub fn cache(&mut self) -> Option<&mut NodeCache> {
        self.poll_ready();
        self.ready.as_mut().map(|r| &mut r.cache)
    }

    /// Access the content loader.
    pub fn loader(&mut self) -> Option<&NodeContentLoader> {
        self.poll_ready();
        self.ready.as_ref().map(|r| &r.loader)
    }

    /// Access the node tree. Returns `None` until bootstrap resolves.
    pub fn node_tree(&mut self) -> Option<&NodeTree> {
        self.poll_ready();
        self.ready.as_ref().map(|r| &r.node_tree)
    }

    /// Get the ellipsoid for this layer's CRS (always WGS84 for I3S).
    pub fn ellipsoid(&self) -> Ellipsoid {
        Ellipsoid::WGS84
    }

    /// Compute load progress as a fraction `[0.0, 1.0]`.
    ///
    /// Returns `0.0` until bootstrap resolves, `1.0` when all requested nodes
    /// are loaded.
    pub fn load_progress(&mut self) -> f64 {
        self.poll_ready();
        let r = match self.ready.as_ref() {
            Some(r) => r,
            None => return 0.0,
        };
        let loaded = r
            .node_states
            .iter()
            .filter(|s| s.load_state == NodeLoadState::Loaded)
            .count();
        let loading = r.loader.in_flight_count() + r.loader.pending_count();
        let total = loaded + loading;
        if total == 0 {
            1.0
        } else {
            loaded as f64 / total as f64
        }
    }

    /// Get the OBB for a node in the working coordinate system.
    fn node_obb(&self, node_id: u32) -> Option<OrientedBoundingBox> {
        let r = self.ready.as_ref()?;
        let obb = r.node_tree.obb_of(node_id)?;
        Some(crs::obb_from_spec(obb, r.crs, r.crs_transform.as_deref()))
    }

    /// Iterate over nodes selected for rendering, with their OBB transform
    /// and renderer resources.
    ///
    /// The integration uses these to emit draw commands. Each [`RenderNode`]
    /// carries the I3S quaternion-based transform (center, quaternion, half_size)
    /// rather than a matrix — matching the I3S OBB representation.
    ///
    /// Returns an empty iterator until bootstrap resolves.
    pub fn nodes_to_render<'a>(
        &'a self,
        result: &'a ViewUpdateResult,
    ) -> impl Iterator<Item = RenderNode<'a>> + 'a {
        result.nodes_to_render.iter().filter_map(move |&node_id| {
            let obb = self.node_obb(node_id)?;
            let r = self.ready.as_ref()?;
            Some(RenderNode {
                node_id,
                center: obb.center,
                quaternion: obb.quaternion,
                half_size: obb.half_size,
                bounding_radius: obb.half_size.length(),
                renderer_resources: r
                    .node_states
                    .get(node_id as usize)
                    .and_then(|s| s.renderer_resources.as_ref()),
            })
        })
    }

    /// Run LOD selection + loading in a blocking loop until all nodes
    /// meeting the current SSE threshold are loaded.
    ///
    /// Useful for offline rendering, screenshots, or CLI tools where
    /// frame-by-frame progression isn't needed.
    ///
    /// Blocks until the async bootstrap resolves before starting the loop.
    #[must_use]
    pub fn update_view_offline(&mut self, views: &[ViewState]) -> ViewUpdateResult {
        // Block until bootstrap is complete (no main-thread dependency, so plain wait)
        if self.ready.is_none() {
            self.root_available.wait().ok();
            self.poll_ready();
        }

        loop {
            let result = self.update_view(views);
            self.load_nodes(&result);

            let (in_flight, pending, pages_in_flight) =
                self.ready.as_ref().map_or((0, 0, true), |r| {
                    (
                        r.loader.in_flight_count(),
                        r.loader.pending_count(),
                        !r.pages_in_flight.is_empty(),
                    )
                });

            if result.load_requests.is_empty() && in_flight == 0 && pending == 0 && !pages_in_flight
            {
                return result;
            }

            // Yield to let worker threads make progress
            std::thread::yield_now();
        }
    }
}

/// Detect layer type from raw JSON bytes without full deserialization.
fn probe_layer_type(bytes: &[u8]) -> i3s_util::Result<SceneLayerType> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Probe {
        layer_type: SceneLayerType,
    }
    let probe: Probe = read_json(bytes)?;
    Ok(probe.layer_type)
}

/// Bootstrap a `ReadyState` from the layer JSON and node page 0.
///
/// This function runs on a worker thread. It performs all blocking I/O and
/// returns the fully-initialized `ReadyState` on success.
fn bootstrap_layer(
    accessor: &Arc<dyn AssetAccessor>,
    resolver: &Arc<dyn ResourceUriResolver>,
    externals: &SceneLayerExternals,
    options: &SelectionOptions,
    crs_transform: Option<Arc<dyn CrsTransform>>,
) -> i3s_util::Result<ReadyState> {
    let layer_uri = resolver.layer_uri();
    let layer_bytes = accessor.get(&layer_uri)?.into_data()?;
    let layer_type = probe_layer_type(&layer_bytes)?;

    match layer_type {
        SceneLayerType::Pointcloud => bootstrap_pointcloud(
            accessor,
            resolver,
            externals,
            options,
            &layer_bytes,
            crs_transform,
        ),
        SceneLayerType::Building => Err(i3s_util::I3sError::InvalidData(
            "Building layers cannot be opened as a single SceneLayer. \
             Use BuildingSceneLayer instead."
                .into(),
        )),
        SceneLayerType::Point => bootstrap_point(
            accessor,
            resolver,
            externals,
            options,
            &layer_bytes,
            crs_transform,
        ),
        _ => {
            // 3DObject, IntegratedMesh
            bootstrap_mesh(
                accessor,
                resolver,
                externals,
                options,
                &layer_bytes,
                crs_transform,
            )
        }
    }
}

fn bootstrap_mesh(
    accessor: &Arc<dyn AssetAccessor>,
    resolver: &Arc<dyn ResourceUriResolver>,
    externals: &SceneLayerExternals,
    options: &SelectionOptions,
    layer_bytes: &[u8],
    crs_transform: Option<Arc<dyn CrsTransform>>,
) -> i3s_util::Result<ReadyState> {
    let info: SceneLayerInfo = read_json(layer_bytes)?;

    let lod_metric = match info.node_pages.as_ref() {
        Some(npd) => match npd.lod_selection_metric_type {
            NodePageDefinitionLodSelectionMetricType::Maxscreenthreshold => {
                LodMetric::MaxScreenThreshold
            }
            NodePageDefinitionLodSelectionMetricType::Maxscreenthresholdsq => {
                LodMetric::MaxScreenThresholdSQ
            }
        },
        None => LodMetric::MaxScreenThreshold,
    };

    let crs = SceneCoordinateSystem::from_spatial_reference(info.spatial_reference.as_ref());
    let nodes_per_page = info
        .node_pages
        .as_ref()
        .map(|npd| npd.nodes_per_page as usize)
        .unwrap_or(64);

    let page0_bytes = accessor.get(&resolver.node_page_uri(0))?.into_data()?;
    let mut node_tree = NodeTree::Paged(PagedNodeTree {
        node_pages: Vec::new(),
        nodes_per_page,
    });
    node_tree.insert_page(0, &page0_bytes)?;

    let node_count = node_tree.node_count();
    let node_states = (0..node_count).map(|i| NodeState::new(i as u32)).collect();

    let attribute_infos = build_mesh_attribute_infos(&info);
    let loader = NodeContentLoader::new(
        Arc::clone(accessor),
        Arc::clone(resolver),
        externals.async_system.task_processor().clone(),
        Arc::clone(&externals.prepare_renderer_resources),
        crs_transform.clone(),
        options.max_simultaneous_loads,
        GeometryLayout::default(),
        attribute_infos,
        Arc::new(PassthroughDecoder),
    );
    let cache = NodeCache::new(options.maximum_cached_bytes);
    let (page_sender, page_receiver) = crossbeam_channel::unbounded();

    Ok(ReadyState {
        info: LayerInfo::Mesh(info),
        node_tree,
        node_states,
        lod_metric,
        crs,
        crs_transform,
        frame: 0,
        loader,
        cache,
        page_sender,
        page_receiver,
        pages_in_flight: HashSet::new(),
    })
}

fn bootstrap_point(
    accessor: &Arc<dyn AssetAccessor>,
    resolver: &Arc<dyn ResourceUriResolver>,
    externals: &SceneLayerExternals,
    options: &SelectionOptions,
    layer_bytes: &[u8],
    crs_transform: Option<Arc<dyn CrsTransform>>,
) -> i3s_util::Result<ReadyState> {
    let info: SceneLayerInfoPsl = read_json(layer_bytes)?;

    let lod_metric = match info.point_node_pages.as_ref() {
        Some(npd) => match npd.lod_selection_metric_type {
            NodePageDefinitionLodSelectionMetricType::Maxscreenthreshold => {
                LodMetric::MaxScreenThreshold
            }
            NodePageDefinitionLodSelectionMetricType::Maxscreenthresholdsq => {
                LodMetric::MaxScreenThresholdSQ
            }
        },
        None => LodMetric::MaxScreenThreshold,
    };

    let crs = SceneCoordinateSystem::from_spatial_reference(info.spatial_reference.as_ref());
    let nodes_per_page = info
        .point_node_pages
        .as_ref()
        .map(|npd| npd.nodes_per_page as usize)
        .unwrap_or(64);

    let page0_bytes = accessor.get(&resolver.node_page_uri(0))?.into_data()?;
    let mut node_tree = NodeTree::Paged(PagedNodeTree {
        node_pages: Vec::new(),
        nodes_per_page,
    });
    node_tree.insert_page(0, &page0_bytes)?;

    let node_count = node_tree.node_count();
    let node_states = (0..node_count).map(|i| NodeState::new(i as u32)).collect();

    let attribute_infos = build_psl_attribute_infos(&info);
    let loader = NodeContentLoader::new(
        Arc::clone(accessor),
        Arc::clone(resolver),
        externals.async_system.task_processor().clone(),
        Arc::clone(&externals.prepare_renderer_resources),
        crs_transform.clone(),
        options.max_simultaneous_loads,
        GeometryLayout::default(),
        attribute_infos,
        Arc::new(PassthroughDecoder),
    );
    let cache = NodeCache::new(options.maximum_cached_bytes);
    let (page_sender, page_receiver) = crossbeam_channel::unbounded();

    Ok(ReadyState {
        info: LayerInfo::Point(info),
        node_tree,
        node_states,
        lod_metric,
        crs,
        crs_transform,
        frame: 0,
        loader,
        cache,
        page_sender,
        page_receiver,
        pages_in_flight: HashSet::new(),
    })
}

fn bootstrap_pointcloud(
    accessor: &Arc<dyn AssetAccessor>,
    resolver: &Arc<dyn ResourceUriResolver>,
    externals: &SceneLayerExternals,
    options: &SelectionOptions,
    layer_bytes: &[u8],
    crs_transform: Option<Arc<dyn CrsTransform>>,
) -> i3s_util::Result<ReadyState> {
    let info: PointCloudLayer = read_json(layer_bytes)?;
    let nodes_per_page = info.store.index.nodes_per_page as usize;
    let crs = SceneCoordinateSystem::from_spatial_reference(Some(&info.spatial_reference));

    let page0_bytes = accessor.get(&resolver.node_page_uri(0))?.into_data()?;
    let mut node_tree = NodeTree::PointCloud(PointCloudNodeTree {
        node_pages: Vec::new(),
        nodes_per_page,
    });
    node_tree.insert_page(0, &page0_bytes)?;

    let node_count = node_tree.node_count();
    let node_states = (0..node_count).map(|i| NodeState::new(i as u32)).collect();

    let loader = NodeContentLoader::new(
        Arc::clone(accessor),
        Arc::clone(resolver),
        externals.async_system.task_processor().clone(),
        Arc::clone(&externals.prepare_renderer_resources),
        crs_transform.clone(),
        options.max_simultaneous_loads,
        GeometryLayout::default(),
        Vec::new(),
        Arc::new(PassthroughDecoder),
    );
    let cache = NodeCache::new(options.maximum_cached_bytes);
    let (page_sender, page_receiver) = crossbeam_channel::unbounded();

    Ok(ReadyState {
        info: LayerInfo::PointCloud(info),
        node_tree,
        node_states,
        lod_metric: LodMetric::DensityThreshold,
        crs,
        crs_transform,
        frame: 0,
        loader,
        cache,
        page_sender,
        page_receiver,
        pages_in_flight: HashSet::new(),
    })
}

/// Build attribute info list from a 3DObject/IntegratedMesh layer's `attributeStorageInfo`.
fn build_mesh_attribute_infos(info: &SceneLayerInfo) -> Vec<AttributeInfo> {
    use i3s_reader::attribute::AttributeValueType;

    let storage = match info.attribute_storage_info.as_ref() {
        Some(s) => s,
        None => return Vec::new(),
    };

    storage
        .iter()
        .enumerate()
        .filter_map(|(idx, asi)| {
            let value_type_str = asi.attribute_values.as_ref()?.value_type.as_str();
            let value_type = AttributeValueType::from_str(value_type_str)?;
            Some(AttributeInfo {
                attribute_id: idx as u32,
                value_type,
            })
        })
        .collect()
}

/// Build attribute info list from a Point (PSL) layer's `attributeStorageInfo`.
fn build_psl_attribute_infos(info: &SceneLayerInfoPsl) -> Vec<AttributeInfo> {
    use i3s_reader::attribute::AttributeValueType;

    let storage = match info.attribute_storage_info.as_ref() {
        Some(s) => s,
        None => return Vec::new(),
    };

    storage
        .iter()
        .enumerate()
        .filter_map(|(idx, asi)| {
            let value_type_str = asi.attribute_values.as_ref()?.value_type.as_str();
            let value_type = AttributeValueType::from_str(value_type_str)?;
            Some(AttributeInfo {
                attribute_id: idx as u32,
                value_type,
            })
        })
        .collect()
}
