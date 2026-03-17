//! Top-level scene layer: entry point for loading and selecting I3S nodes.
//!
//! Modeled after cesium-native's `Tileset`. The main integration point for
//! the I3S engine. Call [`update_view`](SceneLayer::update_view) each frame
//! for LOD selection, then [`load_nodes`](SceneLayer::load_nodes) to dispatch
//! content fetches and finalize loaded content on the main thread.
//!
//! Supports all I3S layer types:
//! - **3DObject / IntegratedMesh / Point** — standard `NodePage` node tree
//! - **PointCloud** — `PointCloudNodePageDefinition` with density-threshold LOD
//! - **Building** — see [`BuildingSceneLayer`](crate::building::BuildingSceneLayer)

use std::collections::HashSet;
use std::sync::Arc;

use glam::{DQuat, DVec3};

use i3s::core::{SceneLayerInfo, SceneLayerInfoPsl, SceneLayerType};
use i3s::node::NodePageDefinitionLodSelectionMetricType;
use i3s::pointcloud::PointCloudLayer;

use i3s_async::{AssetAccessor, ResourceUriResolver};
use i3s_geometry::obb::OrientedBoundingBox;
use i3s_geospatial::crs::{self, CrsTransform, SceneCoordinateSystem};
use i3s_geospatial::ellipsoid::Ellipsoid;
use i3s_reader::geometry::GeometryLayout;
use i3s_reader::json::read_json;
use i3s_util::Result;

use crate::cache::NodeCache;
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

/// A loaded I3S scene layer backed by an [`AssetAccessor`].
///
/// Generic over `A`, the asset accessor type.
/// Created via [`open`](SceneLayer::open) with an accessor, URI resolver,
/// and [`SceneLayerExternals`].
///
/// Each frame:
/// 1. Call [`update_view`](SceneLayer::update_view) (sync) — LOD selection
/// 2. Call [`load_nodes`](SceneLayer::load_nodes) (async) — dispatch loads,
///    collect completed, run `prepare_in_main_thread`
/// A node selected for rendering, with its bounding volume and renderer resources.
///
/// I3S stores orientation as a quaternion, not a matrix. The integration
/// computes its own model transform from `(center, quaternion, half_size)`.
///
/// ## Coordinate system
///
/// - **Global layers** (WKID 4326/4490): Always ECEF.
/// - **Local layers with [`CrsTransform`]**: ECEF (corners transformed, AABB refit).
/// - **Local layers without transform**: Layer's native CRS.
pub struct RenderNode<'a> {
    /// The node's global ID.
    pub node_id: u32,
    /// OBB center (ECEF when a [`CrsTransform`] is provided or for global layers).
    pub center: DVec3,
    /// OBB orientation quaternion.
    pub quaternion: DQuat,
    /// OBB half-size extents along the local axes.
    pub half_size: DVec3,
    /// Radius of the OBB's bounding sphere (for draw-call culling).
    pub bounding_radius: f64,
    /// Renderer resources produced by [`PrepareRendererResources`].
    /// `None` if the integration's `prepare_in_main_thread` returned `None`.
    pub renderer_resources: Option<&'a RendererResources>,
}

pub struct SceneLayer {
    /// The typed layer info document.
    pub info: LayerInfo,
    /// The asset accessor (unified HTTP + SLPK).
    accessor: Arc<dyn AssetAccessor>,
    /// URI resolver for I3S resources.
    resolver: Arc<dyn ResourceUriResolver>,
    /// External dependencies (task processor, renderer).
    externals: SceneLayerExternals,
    /// The node tree (paged or point-cloud).
    node_tree: NodeTree,
    /// Runtime state for each node.
    node_states: Vec<NodeState>,
    /// Selection options.
    pub options: SelectionOptions,
    /// LOD metric derived from the layer's node page definition.
    lod_metric: LodMetric,
    /// Coordinate reference system classification.
    crs: SceneCoordinateSystem,
    /// Optional CRS-to-ECEF transform for local/projected layers.
    crs_transform: Option<Arc<dyn CrsTransform>>,
    /// Current frame counter.
    frame: u64,
    /// Concurrent content loader.
    loader: NodeContentLoader,
    /// Memory-budgeted content cache.
    cache: NodeCache,
    /// Channel for async node page fetch results.
    page_sender: crossbeam_channel::Sender<PageFetchResult>,
    page_receiver: crossbeam_channel::Receiver<PageFetchResult>,
    /// Page IDs currently being fetched (dedup guard).
    pages_in_flight: HashSet<u32>,
}

/// Result of an async node page fetch.
struct PageFetchResult {
    page_id: u32,
    data: std::result::Result<Vec<u8>, i3s_util::I3sError>,
}

impl SceneLayer {
    /// Create a scene layer from an accessor and URI resolver.
    /// Fetches the layer info document and the first node page to bootstrap the tree.
    ///
    /// Automatically detects the layer type from the JSON document and
    /// constructs the appropriate node tree variant. Supports 3DObject,
    /// IntegratedMesh, Point, and PointCloud layers.
    ///
    /// For Building layers, use [`BuildingSceneLayer`](crate::building::BuildingSceneLayer).
    pub fn open(
        accessor: impl AssetAccessor + 'static,
        resolver: Arc<dyn ResourceUriResolver>,
        externals: SceneLayerExternals,
        options: SelectionOptions,
    ) -> Result<Self> {
        Self::open_shared(Arc::new(accessor), resolver, externals, options, None)
    }

    /// Like [`open`](Self::open), but with a [`CrsTransform`] for local layers.
    ///
    /// When provided, OBBs from local/projected layers are transformed to ECEF,
    /// enabling a unified ECEF [`ViewState`] regardless of the layer's native CRS.
    /// For global layers (WKID 4326/4490), the transform is ignored.
    pub fn open_with_transform(
        accessor: impl AssetAccessor + 'static,
        resolver: Arc<dyn ResourceUriResolver>,
        externals: SceneLayerExternals,
        options: SelectionOptions,
        crs_transform: Arc<dyn CrsTransform>,
    ) -> Result<Self> {
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
    ) -> Result<Self> {
        // Fetch the layer JSON document
        let layer_uri = resolver.layer_uri();
        let layer_bytes = accessor.get(&layer_uri)?.into_data()?;

        // Probe the layer type from the raw JSON
        let layer_type = probe_layer_type(&layer_bytes)?;

        match layer_type {
            SceneLayerType::Pointcloud => {
                Self::open_pointcloud(
                    accessor,
                    resolver,
                    externals,
                    options,
                    &layer_bytes,
                    crs_transform,
                )
            }
            SceneLayerType::Building => Err(i3s_util::I3sError::InvalidData(
                "Building layers cannot be opened as a single SceneLayer. \
                     Use BuildingSceneLayer instead."
                    .into(),
            )),
            SceneLayerType::Point => {
                Self::open_point(
                    accessor,
                    resolver,
                    externals,
                    options,
                    &layer_bytes,
                    crs_transform,
                )
            }
            _ => {
                // 3DObject, IntegratedMesh
                Self::open_mesh(
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

    /// Open a 3DObject or IntegratedMesh layer.
    fn open_mesh(
        accessor: Arc<dyn AssetAccessor>,
        resolver: Arc<dyn ResourceUriResolver>,
        externals: SceneLayerExternals,
        options: SelectionOptions,
        layer_bytes: &[u8],
        crs_transform: Option<Arc<dyn CrsTransform>>,
    ) -> Result<Self> {
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

        let page0_uri = resolver.node_page_uri(0);
        let page0_bytes = accessor.get(&page0_uri)?.into_data()?;

        let mut node_tree = NodeTree::Paged(PagedNodeTree {
            node_pages: Vec::new(),
            nodes_per_page,
        });
        node_tree.insert_page(0, &page0_bytes)?;

        let node_count = node_tree.node_count();
        let node_states: Vec<NodeState> =
            (0..node_count).map(|i| NodeState::new(i as u32)).collect();

        let attribute_infos = build_mesh_attribute_infos(&info);
        let loader = NodeContentLoader::new(
            Arc::clone(&accessor),
            Arc::clone(&resolver),
            externals.async_system.task_processor().clone(),
            Arc::clone(&externals.prepare_renderer_resources),
            crs_transform.clone(),
            options.max_simultaneous_loads,
            GeometryLayout::default(),
            attribute_infos,
        );
        let cache = NodeCache::new(options.maximum_cached_bytes);
        let (page_sender, page_receiver) = crossbeam_channel::unbounded();

        Ok(Self {
            info: LayerInfo::Mesh(info),
            accessor,
            resolver,
            externals,
            node_tree,
            node_states,
            options,
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

    /// Open a Point scene layer (PSL profile).
    fn open_point(
        accessor: Arc<dyn AssetAccessor>,
        resolver: Arc<dyn ResourceUriResolver>,
        externals: SceneLayerExternals,
        options: SelectionOptions,
        layer_bytes: &[u8],
        crs_transform: Option<Arc<dyn CrsTransform>>,
    ) -> Result<Self> {
        let info: SceneLayerInfoPsl = read_json(layer_bytes)?;

        // PSL uses pointNodePages instead of nodePages
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

        let page0_uri = resolver.node_page_uri(0);
        let page0_bytes = accessor.get(&page0_uri)?.into_data()?;

        let mut node_tree = NodeTree::Paged(PagedNodeTree {
            node_pages: Vec::new(),
            nodes_per_page,
        });
        node_tree.insert_page(0, &page0_bytes)?;

        let node_count = node_tree.node_count();
        let node_states: Vec<NodeState> =
            (0..node_count).map(|i| NodeState::new(i as u32)).collect();

        let attribute_infos = build_psl_attribute_infos(&info);
        let loader = NodeContentLoader::new(
            Arc::clone(&accessor),
            Arc::clone(&resolver),
            externals.async_system.task_processor().clone(),
            Arc::clone(&externals.prepare_renderer_resources),
            crs_transform.clone(),
            options.max_simultaneous_loads,
            GeometryLayout::default(),
            attribute_infos,
        );
        let cache = NodeCache::new(options.maximum_cached_bytes);
        let (page_sender, page_receiver) = crossbeam_channel::unbounded();

        Ok(Self {
            info: LayerInfo::Point(info),
            accessor,
            resolver,
            externals,
            node_tree,
            node_states,
            options,
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

    /// Open a PointCloud scene layer (PCSL profile).
    fn open_pointcloud(
        accessor: Arc<dyn AssetAccessor>,
        resolver: Arc<dyn ResourceUriResolver>,
        externals: SceneLayerExternals,
        options: SelectionOptions,
        layer_bytes: &[u8],
        crs_transform: Option<Arc<dyn CrsTransform>>,
    ) -> Result<Self> {
        let info: PointCloudLayer = read_json(layer_bytes)?;

        // PointCloud uses density-threshold LOD
        let lod_metric = LodMetric::DensityThreshold;

        let crs = SceneCoordinateSystem::from_spatial_reference(Some(&info.spatial_reference));

        let nodes_per_page = info.store.index.nodes_per_page as usize;

        let page0_uri = resolver.node_page_uri(0);
        let page0_bytes = accessor.get(&page0_uri)?.into_data()?;

        let mut node_tree = NodeTree::PointCloud(PointCloudNodeTree {
            node_pages: Vec::new(),
            nodes_per_page,
        });
        node_tree.insert_page(0, &page0_bytes)?;

        let node_count = node_tree.node_count();
        let node_states: Vec<NodeState> =
            (0..node_count).map(|i| NodeState::new(i as u32)).collect();

        // PointCloud doesn't use the same attribute system — no attributeStorageInfo
        let loader = NodeContentLoader::new(
            Arc::clone(&accessor),
            Arc::clone(&resolver),
            externals.async_system.task_processor().clone(),
            Arc::clone(&externals.prepare_renderer_resources),
            crs_transform.clone(),
            options.max_simultaneous_loads,
            GeometryLayout::default(),
            Vec::new(),
        );
        let cache = NodeCache::new(options.maximum_cached_bytes);
        let (page_sender, page_receiver) = crossbeam_channel::unbounded();

        Ok(Self {
            info: LayerInfo::PointCloud(info),
            accessor,
            resolver,
            externals,
            node_tree,
            node_states,
            options,
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

    /// Ensure a node page is loaded. Returns `true` if the page is available.
    pub fn ensure_node_page(&mut self, page_id: u32) -> Result<bool> {
        if self.node_tree.page_loaded(page_id) {
            return Ok(true);
        }

        let bytes =
            self.accessor.get(&self.resolver.node_page_uri(page_id))?.into_data()?;
        self.node_tree.insert_page(page_id, &bytes)?;

        // Ensure node states cover the new page
        let needed = self.node_tree.node_count();
        while self.node_states.len() < needed {
            let id = self.node_states.len() as u32;
            self.node_states.push(NodeState::new(id));
        }

        Ok(true)
    }

    /// Run per-frame LOD selection (sync — pure computation, no I/O).
    ///
    /// Accepts a slice of [`ViewState`]s. Multiple views support VR (one per
    /// eye) or shadow-map cascades. A node is rendered if *any* view selects
    /// it, and the LOD screen size is the maximum across all views.
    pub fn update_view(&mut self, views: &[ViewState]) -> ViewUpdateResult {
        self.frame += 1;
        let frame = self.frame;
        let metric = self.lod_metric;
        let layer_crs = self.crs;
        let crs_xform = self.crs_transform.as_deref();
        let node_tree = &self.node_tree;
        let nodes_per_page = node_tree.nodes_per_page();

        select_nodes(
            &mut self.node_states,
            |node_id, out: &mut Vec<u32>| {
                node_tree.children_of(node_id, out);
            },
            |node_id| {
                let obb = node_tree.obb_of(node_id)?;
                Some(crs::obb_from_spec(obb, layer_crs, crs_xform))
            },
            |node_id| node_tree.lod_threshold_of(node_id),
            0, // root
            views,
            metric,
            &self.options,
            frame,
            nodes_per_page,
            |page_id| node_tree.page_loaded(page_id),
        )
    }

    /// Process content loading: dispatch fetches, collect completed, finalize.
    ///
    /// Call this after [`update_view`](SceneLayer::update_view) each frame.
    /// Fully non-blocking — never waits for I/O. Node pages and content are
    /// fetched asynchronously on worker threads and collected when ready.
    ///
    /// 1. Collects completed page fetches from previous frames
    /// 2. Dispatches new page fetch requests to the TaskProcessor
    /// 3. Enqueues new content load requests to the TaskProcessor
    /// 4. Collects completed loads (sync channel drain)
    /// 5. Calls `prepare_in_main_thread` for newly loaded content
    /// 6. Handles eviction, unloading, and retry
    pub fn load_nodes(&mut self, result: &ViewUpdateResult) {
        // Collect any completed page fetches from workers
        self.collect_page_fetches();

        // Dispatch async page fetches for pages needed by traversal
        for &page_id in &result.pages_needed {
            if self.node_tree.page_loaded(page_id) || self.pages_in_flight.contains(&page_id) {
                continue;
            }
            self.pages_in_flight.insert(page_id);
            let accessor = Arc::clone(&self.accessor);
            let resolver = Arc::clone(&self.resolver);
            let sender = self.page_sender.clone();
            self.externals
                .async_system
                .task_processor()
                .start_task(Box::new(move || {
                    let uri = resolver.node_page_uri(page_id);
                    let data = accessor.get(&uri).and_then(|r| r.into_data());
                    let _ = sender.send(PageFetchResult { page_id, data });
                }));
        }

        // Enqueue new load requests from selection
        for req in &result.load_requests {
            let idx = req.node_id as usize;
            if idx < self.node_states.len()
                && self.node_states[idx].load_state == NodeLoadState::Unloaded
            {
                self.node_states[idx].load_state = NodeLoadState::Loading;
                self.loader
                    .request(req.node_id, req.priority, req.screen_size);
            }
        }

        // Dispatch up to the concurrency limit
        self.loader.dispatch();

        // Collect completed loads (sync — drains channel)
        let completed = self.loader.collect_completed();
        for load in completed {
            let idx = load.node_id as usize;
            if idx >= self.node_states.len() {
                continue;
            }
            match load.result {
                Ok(ref content) => {
                    // Phase 3: prepare_in_main_thread (runs here, on the main thread)
                    let main_resources = self
                        .externals
                        .prepare_renderer_resources
                        .prepare_in_main_thread(
                            load.node_id,
                            content,
                            load.load_thread_resources,
                            self.crs_transform.as_ref(),
                        );

                    // Store renderer resources on the node (cesium-native: Tile owns its renderResources)
                    self.node_states[idx].renderer_resources = main_resources;

                    let evicted = self.cache.insert(load.node_id, content.clone());
                    self.node_states[idx].load_state = NodeLoadState::Loaded;
                    // Mark evicted nodes as unloaded and free their renderer resources
                    for evicted_id in evicted {
                        let ei = evicted_id as usize;
                        if ei < self.node_states.len() {
                            self.node_states[ei].load_state = NodeLoadState::Unloaded;
                            let res = self.node_states[ei].renderer_resources.take();
                            self.externals
                                .prepare_renderer_resources
                                .free(evicted_id, res);
                        }
                    }
                }
                Err(_) => {
                    self.node_states[idx].load_state = NodeLoadState::Failed;
                    self.node_states[idx].failed_attempts += 1;
                    self.node_states[idx].last_failed_frame = self.frame;
                }
            }
        }

        // Unload nodes that are no longer visible
        for &node_id in &result.nodes_to_unload {
            let idx = node_id as usize;
            if idx < self.node_states.len() {
                // Return renderer resources to the integration for cleanup
                let res = self.node_states[idx].renderer_resources.take();
                self.externals.prepare_renderer_resources.free(node_id, res);
                self.cache.remove(node_id);
                self.node_states[idx].load_state = NodeLoadState::Unloaded;
            }
        }

        // Retry failed nodes that have cooled down
        let frame = self.frame;
        for state in self.node_states.iter_mut() {
            if state.can_retry(frame) {
                state.load_state = NodeLoadState::Unloaded;
            }
        }
    }

    /// Collect completed async page fetches and insert them into the node tree.
    ///
    /// Called at the start of [`load_nodes`](Self::load_nodes). Non-blocking —
    /// drains the page result channel without waiting.
    fn collect_page_fetches(&mut self) {
        let mut fetched = Vec::new();
        while let Ok(result) = self.page_receiver.try_recv() {
            fetched.push(result);
        }

        for fetch in fetched {
            self.pages_in_flight.remove(&fetch.page_id);
            if let Ok(bytes) = fetch.data {
                if self.node_tree.insert_page(fetch.page_id, &bytes).is_ok() {
                    // Ensure node states cover the new page
                    let needed = self.node_tree.node_count();
                    while self.node_states.len() < needed {
                        let id = self.node_states.len() as u32;
                        self.node_states.push(NodeState::new(id));
                    }
                }
            }
            // Errors are silently dropped — page will be re-requested next frame
        }
    }

    /// Get the runtime state for a node.
    pub fn node_state(&self, node_id: u32) -> Option<&NodeState> {
        self.node_states.get(node_id as usize)
    }

    /// Get a mutable reference to the runtime state for a node.
    pub fn node_state_mut(&mut self, node_id: u32) -> Option<&mut NodeState> {
        self.node_states.get_mut(node_id as usize)
    }

    /// Get the coordinate reference system classification.
    pub fn crs(&self) -> SceneCoordinateSystem {
        self.crs
    }

    /// Get the current frame number.
    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// Access the content cache.
    pub fn cache(&mut self) -> &mut NodeCache {
        &mut self.cache
    }

    /// Access the content loader.
    pub fn loader(&self) -> &NodeContentLoader {
        &self.loader
    }

    /// Access the node tree.
    pub fn node_tree(&self) -> &NodeTree {
        &self.node_tree
    }

    /// Get the ellipsoid for this layer's CRS.
    ///
    /// Returns WGS84 for global layers (WKID 4326), which is the standard
    /// ellipsoid for I3S geographic scenes.
    pub fn ellipsoid(&self) -> Ellipsoid {
        Ellipsoid::WGS84
    }

    /// Compute load progress as a fraction `[0.0, 1.0]`.
    ///
    /// `1.0` means all nodes that the selection algorithm wants are loaded.
    /// `0.0` means nothing is loaded yet (or no nodes are requested).
    pub fn load_progress(&self) -> f64 {
        let loaded = self
            .node_states
            .iter()
            .filter(|s| s.load_state == NodeLoadState::Loaded)
            .count();
        let loading = self.loader.in_flight_count() + self.loader.pending_count();
        let total = loaded + loading;
        if total == 0 {
            1.0
        } else {
            loaded as f64 / total as f64
        }
    }

    /// Get the OBB for a node in the working coordinate system.
    fn node_obb(&self, node_id: u32) -> Option<OrientedBoundingBox> {
        let obb = self.node_tree.obb_of(node_id)?;
        Some(crs::obb_from_spec(
            obb,
            self.crs,
            self.crs_transform.as_deref(),
        ))
    }

    /// Iterate over nodes selected for rendering, with their OBB transform
    /// and renderer resources.
    ///
    /// The integration uses these to emit draw commands. Each [`RenderNode`]
    /// carries the I3S quaternion-based transform (center, quaternion, half_size)
    /// rather than a matrix — matching the I3S OBB representation.
    pub fn nodes_to_render<'a>(
        &'a self,
        result: &'a ViewUpdateResult,
    ) -> impl Iterator<Item = RenderNode<'a>> + 'a {
        result.nodes_to_render.iter().filter_map(move |&node_id| {
            let obb = self.node_obb(node_id)?;
            Some(RenderNode {
                node_id,
                center: obb.center,
                quaternion: obb.quaternion,
                half_size: obb.half_size,
                bounding_radius: obb.half_size.length(),
                renderer_resources: self
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
    pub fn update_view_offline(&mut self, views: &[ViewState]) -> ViewUpdateResult {
        loop {
            let result = self.update_view(views);
            self.load_nodes(&result);

            // Done when nothing is pending or in-flight
            if result.load_requests.is_empty()
                && self.loader.in_flight_count() == 0
                && self.loader.pending_count() == 0
                && self.pages_in_flight.is_empty()
            {
                return result;
            }

            // Yield to let worker threads make progress
            std::thread::yield_now();
        }
    }
}

/// Detect layer type from raw JSON bytes without full deserialization.
fn probe_layer_type(bytes: &[u8]) -> Result<SceneLayerType> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Probe {
        layer_type: SceneLayerType,
    }
    let probe: Probe = read_json(bytes)?;
    Ok(probe.layer_type)
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
