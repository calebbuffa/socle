//! High-level entry point for streaming a 3D Tiles tileset.
//!
//! [`TilesetBuilder`] is a builder that wires the async fetch → hierarchy parse →
//! LOD evaluation → GPU-upload pipeline. Calling [`TilesetBuilder::build`]
//! returns a [`Tileset<C>`] **synchronously** — for URL tilesets the
//! `tileset.json` fetch starts in the background and the handle is immediately
//! usable. For procedural sources (e.g. ellipsoid) the engine is ready
//! immediately. Per-frame calls to [`Tileset::update`] return an empty
//! [`FrameResult`] while loading is in progress and delegate to the inner
//! [`SelectionEngine`] once it is ready.
//!
//! # Typical usage
//!
//! ```ignore
//! // URL tileset — sync, background fetch begins immediately.
//! let tileset: Tileset<MyContent> = TilesetBuilder::open("https://example.com/tileset.json")
//!     .maximum_screen_space_error(16.0)
//!     .max_cached_bytes(512 << 20)
//!     .build(runtime, accessor, Arc::new(MyPreparer));
//!
//! // Ellipsoid (no network) — engine is ready immediately.
//! let tileset: Tileset<MyContent> = TilesetBuilder::ellipsoid(Ellipsoid::wgs84())
//!     .maximum_screen_space_error(16.0)
//!     .build(runtime, accessor, Arc::new(MyPreparer));
//!
//! // Per-frame — returns empty result while URL loading is in progress.
//! let result: &FrameResult = tileset.update(&views, delta_time);
//! ```

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egaku::PrepareRendererResources;
use glam::DVec3;
use orkester::{Context, Handle, Resolver, Task};
use orkester_io::AssetAccessor;
use selekt::{
    ContentKey, FrameResult, LoadFailureDetails, LoadPassResult, LodThreshold, NodeExcluder,
    NodeId, OcclusionTester, PickResult, Policy, QueryDepth, QueryShape, RenderNode,
    SelectionEngine, SelectionEngineBuilder, SelectionOptions, ViewGroupHandle, ViewState,
    ViewUpdateResult,
};
use terra::{Cartographic, Ellipsoid};

use crate::EllipsoidTilesetLoader;
use crate::ellipsoid_content_loader::EllipsoidContentLoader;
use crate::evaluator::GeometricErrorEvaluator;
use crate::height_sampler::{HeightSampler, SampleHeightResult};
use crate::hierarchy::ExplicitTilesetHierarchy;
use crate::loader::{Tiles3dError, TilesetLoader, TilesetLoaderFactory};

enum TilesetSource {
    /// Fetch and parse `tileset.json` from this URL (async).
    Uri(String),
    /// Generate an in-memory ellipsoid globe tileset (synchronous, no network).
    Ellipsoid(EllipsoidTilesetLoader),
}

// ---------------------------------------------------------------------------
// TilesetBuilder — builder
// ---------------------------------------------------------------------------

/// Builder for a streaming 3D Tiles tileset.
///
/// Create with [`TilesetBuilder::open`] (URL) or [`TilesetBuilder::ellipsoid`]
/// (procedural globe), set options, then call [`build`](TilesetBuilder::build)
/// to obtain a [`Tileset`] handle synchronously.
pub struct TilesetBuilder {
    source: TilesetSource,
    headers: Vec<(String, String)>,
    maximum_screen_space_error: f64,
    options: SelectionOptions,
    main_thread_budget: Duration,
    policy: Option<Box<dyn Policy>>,
    on_error: Option<Box<dyn Fn(&LoadFailureDetails) + Send + Sync + 'static>>,
    attribution: Option<Arc<str>>,
    main_context: Option<Context>,
}

impl TilesetBuilder {
    /// Begin configuring a tileset streamed from `uri`.
    ///
    /// The URL should point directly to a `tileset.json` file.
    pub fn open(uri: impl Into<String>) -> Self {
        Self::from_source(TilesetSource::Uri(uri.into()))
    }

    /// Begin configuring a procedural ellipsoid tileset.
    ///
    /// Generates an in-memory globe hierarchy covering the full ellipsoid
    /// surface using quadtree subdivision — no network access required.
    /// The engine is ready immediately after [`build`](Self::build) returns.
    ///
    /// Pass [`Ellipsoid::wgs84()`] for the standard WGS 84 globe.
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
            main_thread_budget: Duration::from_millis(4),
            policy: None,
            on_error: None,
            attribution: None,
            main_context: None,
        }
    }

    /// HTTP headers forwarded with every tile request (e.g. `Authorization`).
    pub fn headers(mut self, headers: Vec<(String, String)>) -> Self {
        self.headers = headers;
        self
    }

    /// Maximum screen-space error threshold in pixels. Default: `16.0`.
    ///
    /// Lower values load more detail; higher values sacrifice quality for speed.
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

    /// Wall-clock budget per frame for main-thread GPU-upload finalization. Default: 4 ms.
    pub fn main_thread_budget(mut self, budget: Duration) -> Self {
        self.main_thread_budget = budget;
        self
    }

    /// Replace all [`SelectionOptions`] at once.
    pub fn options(mut self, options: SelectionOptions) -> Self {
        self.options = options;
        self
    }

    /// Override the visibility / residency policy.
    ///
    /// Default: [`selekt::AllVisibleLruPolicy`].
    pub fn policy(mut self, policy: impl Policy + 'static) -> Self {
        self.policy = Some(Box::new(policy));
        self
    }

    /// Callback invoked when a tile load or hierarchy resolve fails permanently.
    pub fn on_error(
        mut self,
        callback: impl Fn(&LoadFailureDetails) + Send + Sync + 'static,
    ) -> Self {
        self.on_error = Some(Box::new(callback));
        self
    }

    /// Attribution / copyright string (overrides the value parsed from tileset.json).
    pub fn attribution(mut self, text: impl Into<Arc<str>>) -> Self {
        self.attribution = Some(text.into());
        self
    }

    /// Set the main-thread context used to schedule GPU upload tasks
    /// (`prepare_in_main_thread`).  Pass `work_queue.context()` so the host
    /// application controls when uploads execute via `WorkQueue::pump_timed`.
    pub fn with_main_context(mut self, ctx: Context) -> Self {
        self.main_context = Some(ctx);
        self
    }

    /// Build the tileset and return a [`Tileset`] handle immediately.
    ///
    /// - **URL source**: `tileset.json` is fetched asynchronously in the
    ///   background. The returned [`Tileset`] returns empty [`FrameResult`]s
    ///   until loading completes.
    /// - **Ellipsoid source**: the engine is constructed synchronously and is
    ///   ready immediately — no network access occurs.
    pub fn build<R>(
        self,
        bg_context: Context,
        accessor: Arc<dyn AssetAccessor>,
        preparer: Arc<R>,
    ) -> Tileset<R::Content>
    where
        R: PrepareRendererResources + 'static,
        R::WorkerResult: Send + 'static,
        R::Content: Send + 'static,
        R::Error: std::error::Error + Send + Sync + 'static,
    {
        let options = self.options;
        let policy = self.policy;
        let on_error = self.on_error;
        let main_context = self.main_context;
        // Note: main_thread_budget is now caller-managed via WorkQueue::pump_timed.
        let apply_common =
            move |mut builder: SelectionEngineBuilder<R::Content>| -> SelectionEngineBuilder<R::Content> {
                builder = builder.with_options(options);
                if let Some(ctx) = main_context {
                    builder = builder.with_main_context(ctx);
                }
                if let Some(p) = policy {
                    builder = builder.with_policy(p);
                }
                if let Some(cb) = on_error {
                    builder = builder.on_error(cb);
                }
                builder
            };

        match self.source {
            TilesetSource::Uri(uri) => {
                let loader_factory = TilesetLoaderFactory::new(uri, preparer)
                    .with_headers(self.headers)
                    .with_maximum_screen_space_error(self.maximum_screen_space_error);

                let (ready_resolver, ready_task) =
                    orkester::pair::<Result<(), Arc<Tiles3dError>>>();
                let override_attr = self.attribution;
                let task =
                    loader_factory
                        .create(bg_context.clone(), &accessor)
                        .map(move |result| {
                            result.map(|(config, parsed_attr)| {
                                let final_attr = override_attr.or(parsed_attr);
                                (apply_common(config).build(), final_attr)
                            })
                        });

                Tileset {
                    state: TilesetState::Loading(task),
                    empty: FrameResult::default(),
                    last_result: FrameResult::default(),
                    default_view: None,
                    height_sampler: None,
                    attribution: None,
                    hidden_nodes: HashSet::new(),
                    ready: ready_task.share(),
                    ready_resolver: Some(ready_resolver),
                    on_ready_callbacks: Vec::new(),
                }
            }

            TilesetSource::Ellipsoid(loader) => {
                let tileset = loader.create_tileset();
                let hierarchy = ExplicitTilesetHierarchy::from_tileset(&tileset);
                let lod = GeometricErrorEvaluator::new(self.maximum_screen_space_error);
                let content_loader = EllipsoidContentLoader::new(
                    loader.ellipsoid().clone(),
                    preparer,
                );
                let config =
                    SelectionEngineBuilder::new(bg_context.clone(), hierarchy, lod, content_loader);
                let engine = apply_common(config).build();

                let (ready_resolver, ready_task) =
                    orkester::pair::<Result<(), Arc<Tiles3dError>>>();
                ready_resolver.resolve(Ok(()));
                Tileset {
                    state: TilesetState::Ready(engine),
                    empty: FrameResult::default(),
                    last_result: FrameResult::default(),
                    default_view: None,
                    height_sampler: None,
                    attribution: self.attribution,
                    hidden_nodes: HashSet::new(),
                    ready: ready_task.share(),
                    ready_resolver: None,
                    on_ready_callbacks: Vec::new(),
                }
            }
        }
    }
}

enum TilesetState<C: Send + 'static> {
    Loading(Task<Result<(SelectionEngine<C>, Option<Arc<str>>), Tiles3dError>>),
    Ready(SelectionEngine<C>),
    /// Load failed — the error is stored in the `ready` SharedTask cell.
    Failed,
    /// Transient state used only inside `poll_loading` while moving the task out.
    Consumed,
}

/// A streaming 3D Tiles tileset.
///
/// Obtained by calling [`TilesetBuilder::build`]. Construction is synchronous;
/// for URL sources `tileset.json` is fetched in the background, while ellipsoid
/// sources are ready immediately. All per-frame methods are safe to call
/// immediately — they return empty results while loading is in progress.
pub struct Tileset<C: Send + 'static> {
    state: TilesetState<C>,
    /// Returned by reference while the engine is not yet ready.
    empty: FrameResult,
    /// Frame result from the last `update()` call (stored here since the engine no longer owns it).
    last_result: FrameResult,
    /// Default view group created on first `update()` call.
    default_view: Option<ViewGroupHandle>,
    /// Height sampler used by [`sample_heights`](Self::sample_heights).
    ///
    /// `None` until first use, at which point it is lazily initialized to
    /// [`ApproximateHeightSampler`]. Call [`set_height_sampler`](Self::set_height_sampler)
    /// to replace it with a custom implementation.
    height_sampler: Option<Arc<dyn HeightSampler>>,
    /// Attribution / copyright string parsed from tileset.json, or the override from the builder.
    attribution: Option<Arc<str>>,
    /// Nodes hidden via [`hide_node`](Self::hide_node); filtered from render output.
    hidden_nodes: HashSet<NodeId>,
    /// Resolves once the engine transitions to `Ready` or `Failed`.
    ready: Handle<Result<(), Arc<Tiles3dError>>>,
    /// Consumed on first transition; `None` thereafter.
    ready_resolver: Option<Resolver<Result<(), Arc<Tiles3dError>>>>,
    /// Callbacks registered via [`on_ready`](Self::on_ready).
    /// Drained synchronously on the frame the engine becomes ready.
    on_ready_callbacks: Vec<Box<dyn FnOnce(&mut Tileset<C>) + 'static>>,
}

impl<C: Send + 'static> Tileset<C> {
    /// Returns `true` once the tileset has been parsed and the engine is ready.
    pub fn is_ready(&self) -> bool {
        matches!(self.state, TilesetState::Ready(_))
    }

    /// Returns the load error, if the tileset failed to load.
    ///
    /// Reads from the shared `when_ready()` cell — always consistent with
    /// what subscribers of [`when_ready`](Self::when_ready) observe.
    pub fn load_error(&self) -> Option<Arc<Tiles3dError>> {
        match self.ready.get()? {
            Ok(Err(e)) => Some(e),
            _ => None,
        }
    }

    /// Per-frame update: traversal, tile loading, and main-thread finalization.
    ///
    /// Returns a reference to the [`FrameResult`] valid until the next call.
    /// An empty result is returned while loading is in progress or after a
    /// failure — no panic, no unwrap required.
    pub fn update(&mut self, views: &[ViewState], _delta_time: f32) -> &FrameResult {
        self.poll_loading();
        let TilesetState::Ready(engine) = &mut self.state else {
            return &self.empty;
        };
        let handle = *self
            .default_view
            .get_or_insert_with(|| engine.add_view_group(1.0));
        engine.update_view_group(handle, views);
        engine.load();
        // Copy the result out so we can return a &FrameResult with lifetime 'self.
        if let Some(result) = engine.view_group_result(handle) {
            self.last_result.clone_from(result);
        }
        if !self.hidden_nodes.is_empty() {
            self.last_result
                .nodes_to_render
                .retain(|id| !self.hidden_nodes.contains(id));
        }
        &self.last_result
    }

    /// Frame result from the most recent [`update`](Self::update) call.
    pub fn last_result(&self) -> &FrameResult {
        if matches!(self.state, TilesetState::Ready(_)) {
            &self.last_result
        } else {
            &self.empty
        }
    }

    /// Load progress as a percentage in `[0.0, 100.0]`.
    ///
    /// Returns `100.0` when there are no tracked nodes. Corresponds to
    /// Cesium's `computeLoadProgress()`.
    pub fn compute_load_progress(&self) -> f32 {
        match &self.state {
            TilesetState::Ready(engine) => engine.compute_load_progress(),
            _ => 0.0,
        }
    }

    /// Add a view group with the given load-priority weight.
    ///
    /// Returns a handle that can be passed to [`update_view_group`](Self::update_view_group).
    /// Panics if the engine is not yet ready.
    pub fn add_view_group(&mut self, weight: f64) -> ViewGroupHandle {
        self.engine_mut()
            .expect("add_view_group called before tileset is ready")
            .add_view_group(weight)
    }

    /// Remove a view group. Returns `false` if the handle was already invalid.
    pub fn remove_view_group(&mut self, handle: ViewGroupHandle) -> bool {
        match &mut self.state {
            TilesetState::Ready(engine) => engine.remove_view_group(handle),
            _ => false,
        }
    }

    /// Returns `true` if the view group handle is still valid.
    pub fn is_view_group_active(&self, handle: ViewGroupHandle) -> bool {
        match &self.state {
            TilesetState::Ready(engine) => engine.is_view_group_active(handle),
            _ => false,
        }
    }

    /// Update a single view group and return traversal statistics.
    ///
    /// Call once per view group per frame, then call [`load`](Self::load).
    pub fn update_view_group(
        &mut self,
        handle: ViewGroupHandle,
        views: &[ViewState],
    ) -> ViewUpdateResult {
        self.poll_loading();
        match &mut self.state {
            TilesetState::Ready(engine) => engine.update_view_group(handle, views),
            _ => ViewUpdateResult::default(),
        }
    }

    /// Frame result for a specific view group. Returns `None` while loading or
    /// if the handle is invalid.
    pub fn view_group_result(&self, handle: ViewGroupHandle) -> Option<&FrameResult> {
        match &self.state {
            TilesetState::Ready(engine) => engine.view_group_result(handle),
            _ => None,
        }
    }

    /// Like [`update_view_group`](Self::update_view_group) but blocks until all
    /// LOD-required tiles are resident. Only for non-realtime capture.
    pub fn update_view_group_blocking(
        &mut self,
        handle: ViewGroupHandle,
        views: &[ViewState],
    ) -> ViewUpdateResult {
        self.poll_loading();
        match &mut self.state {
            TilesetState::Ready(engine) => engine.update_view_group_blocking(handle, views),
            _ => ViewUpdateResult::default(),
        }
    }

    /// Dispatch queued load requests. Call once per frame after all
    /// `update_view_group` calls.
    pub fn load(&mut self) -> LoadPassResult {
        match &mut self.state {
            TilesetState::Ready(engine) => engine.load(),
            _ => LoadPassResult::default(),
        }
    }

    /// Iterate over render-ready nodes from the last [`update`](Self::update) call.
    ///
    /// Each [`RenderNode`] carries the node id, its accumulated world-space transform,
    /// and a reference to the renderer-owned content. Empty while the tileset is loading.
    pub fn render_nodes(&self) -> Box<dyn Iterator<Item = RenderNode<'_, C>> + '_> {
        match &self.state {
            TilesetState::Ready(engine) => {
                let Some(handle) = self.default_view else {
                    return Box::new(std::iter::empty());
                };
                let hidden = self.hidden_nodes.clone();
                Box::new(
                    engine
                        .render_nodes(handle)
                        .filter(move |rn| !hidden.contains(&rn.id)),
                )
            }
            _ => Box::new(std::iter::empty()),
        }
    }

    /// Read-only access to loaded content for a resident node.
    pub fn content(&self, node_id: NodeId) -> Option<&C> {
        match &self.state {
            TilesetState::Ready(engine) => engine.content(node_id),
            _ => None,
        }
    }

    /// Mutable access to loaded content for a resident node.
    pub fn content_mut(&mut self, node_id: NodeId) -> Option<&mut C> {
        match &mut self.state {
            TilesetState::Ready(engine) => engine.content_mut(node_id),
            _ => None,
        }
    }

    /// Primary content key (e.g. URL) for a node.
    pub fn content_key(&self, node_id: NodeId) -> Option<&ContentKey> {
        match &self.state {
            TilesetState::Ready(engine) => engine.content_key(node_id),
            _ => None,
        }
    }

    /// All content keys for a node (multi-content tiles may have more than one).
    pub fn content_keys(&self, node_id: NodeId) -> &[ContentKey] {
        match &self.state {
            TilesetState::Ready(engine) => engine.content_keys(node_id),
            _ => &[],
        }
    }

    /// Total bytes of currently-resident tile content.
    pub fn total_data_bytes(&self) -> usize {
        match &self.state {
            TilesetState::Ready(engine) => engine.total_data_bytes(),
            _ => 0,
        }
    }

    /// Number of nodes with content currently resident in memory.
    pub fn resident_node_count(&self) -> usize {
        match &self.state {
            TilesetState::Ready(engine) => engine.resident_node_count(),
            _ => 0,
        }
    }

    /// CPU-side ray pick against the currently rendered node set.
    ///
    /// Returns hits sorted front-to-back. Returns empty while loading.
    pub fn pick(&self, ray_origin: DVec3, ray_direction: DVec3) -> Vec<PickResult> {
        match &self.state {
            TilesetState::Ready(engine) => {
                let Some(handle) = self.default_view else {
                    return Vec::new();
                };
                engine.pick(handle, ray_origin, ray_direction)
            }
            _ => Vec::new(),
        }
    }

    /// Traverse the hierarchy and return all nodes whose bounds intersect `shape`.
    pub fn query(&self, shape: &QueryShape, depth: QueryDepth) -> Vec<NodeId> {
        match &self.state {
            TilesetState::Ready(engine) => engine.query(shape, depth),
            _ => Vec::new(),
        }
    }

    /// Hide a node from render output without evicting its content.
    pub fn hide_node(&mut self, node: NodeId) {
        self.hidden_nodes.insert(node);
    }

    /// Restore a node previously hidden with [`hide_node`](Self::hide_node).
    pub fn show_node(&mut self, node: NodeId) {
        self.hidden_nodes.remove(&node);
    }

    /// Returns `true` if this node is currently hidden.
    pub fn is_node_hidden(&self, node: NodeId) -> bool {
        self.hidden_nodes.contains(&node)
    }

    /// Unhide all nodes previously hidden with [`hide_node`](Self::hide_node).
    pub fn show_all_nodes(&mut self) {
        self.hidden_nodes.clear();
    }

    /// Add an excluder that skips entire subtrees during traversal.
    pub fn add_excluder(&mut self, excluder: impl NodeExcluder + 'static) {
        if let TilesetState::Ready(engine) = &mut self.state {
            engine.add_excluder(excluder);
        }
    }

    /// Read-only access to the list of registered excluders.
    pub fn excluders(&self) -> &[Box<dyn NodeExcluder>] {
        match &self.state {
            TilesetState::Ready(engine) => engine.excluders(),
            _ => &[],
        }
    }

    /// Mutable access to the list of registered excluders.
    pub fn clear_excluders(&mut self) {
        if let TilesetState::Ready(engine) = &mut self.state {
            engine.clear_excluders();
        }
    }

    /// Replace the occlusion tester used during traversal.
    pub fn set_occlusion_tester(&mut self, tester: impl OcclusionTester + 'static) {
        if let TilesetState::Ready(engine) = &mut self.state {
            engine.set_occlusion_tester(tester);
        }
    }

    /// Read-only access to the current [`SelectionOptions`].
    pub fn options(&self) -> Option<&SelectionOptions> {
        match &self.state {
            TilesetState::Ready(engine) => Some(engine.options()),
            _ => None,
        }
    }

    /// Replace the current selection options.
    pub fn set_options(&mut self, options: SelectionOptions) {
        if let TilesetState::Ready(engine) = &mut self.state {
            engine.set_options(options);
        }
    }

    /// Read-only access to the LOD threshold controller.
    pub fn lod_threshold(&self) -> Option<&LodThreshold> {
        match &self.state {
            TilesetState::Ready(engine) => Some(engine.lod_threshold()),
            _ => None,
        }
    }

    /// Mutable access to the LOD threshold controller.
    pub fn lod_threshold_mut(&mut self) -> Option<&mut LodThreshold> {
        match &mut self.state {
            TilesetState::Ready(engine) => Some(engine.lod_threshold_mut()),
            _ => None,
        }
    }

    /// Current frame index (increments once per [`update`](Self::update) call).
    pub fn frame_index(&self) -> u64 {
        match &self.state {
            TilesetState::Ready(engine) => engine.frame_index(),
            _ => 0,
        }
    }

    /// The runtime this engine is bound to.
    pub fn bg_context(&self) -> Option<&Context> {
        match &self.state {
            TilesetState::Ready(engine) => Some(engine.bg_context()),
            _ => None,
        }
    }

    /// Attribution / copyright string for this data source, if available.
    pub fn attribution(&self) -> Option<&str> {
        self.attribution.as_deref()
    }

    /// Read-only access to the spatial hierarchy.
    pub fn hierarchy(&self) -> Option<&dyn selekt::SceneGraph> {
        match &self.state {
            TilesetState::Ready(engine) => Some(engine.hierarchy()),
            _ => None,
        }
    }

    /// Returns the root [`NodeId`] of the spatial hierarchy.
    ///
    /// Equivalent to Cesium's `getRootTile()`. Returns `None` while loading.
    pub fn root(&self) -> Option<NodeId> {
        match &self.state {
            TilesetState::Ready(engine) => Some(engine.hierarchy().root()),
            _ => None,
        }
    }

    /// Set the callback invoked whenever a node load or hierarchy resolve fails.
    pub fn set_on_load_error(
        &mut self,
        callback: impl Fn(&LoadFailureDetails) + Send + Sync + 'static,
    ) {
        if let TilesetState::Ready(engine) = &mut self.state {
            engine.set_on_load_error(callback);
        }
    }

    /// Iterate over all nodes that currently have resident content, regardless
    /// of whether they are in the current render set.
    ///
    /// Equivalent to Cesium's `loadedTiles` / `forEachLoadedTile`. Returns an
    /// empty iterator while loading is in progress.
    pub fn for_each_loaded_node(&self, mut f: impl FnMut(NodeId, &C)) {
        if let TilesetState::Ready(engine) = &self.state {
            for id in 0..engine.resident_node_count() {
                let node = NodeId::from_index(id);
                if let Some(content) = engine.content(node) {
                    f(node, content);
                }
            }
        }
    }

    /// Block the calling thread until all pending tile loads are complete or
    /// `timeout` elapses.
    ///
    /// Returns `true` if loading completed before the timeout.
    pub fn wait_for_all_loads_to_complete(
        &mut self,
        views: &[ViewState],
        timeout: Duration,
    ) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            self.update(views, 0.0);
            if self.compute_load_progress() >= 100.0 {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Replace the height sampler used by [`sample_heights`](Self::sample_heights).
    ///
    /// By default [`ApproximateHeightSampler`] is used automatically on the first
    /// call. Supply a custom implementation here if you need a different accuracy /
    /// performance trade-off (e.g. a terrain-specific sampler).
    pub fn set_height_sampler(&mut self, sampler: impl HeightSampler + 'static) {
        self.height_sampler = Some(Arc::new(sampler));
    }

    /// Sample approximate heights for a list of geographic positions.
    ///
    /// Uses the registered [`HeightSampler`] (see [`set_height_sampler`](Self::set_height_sampler)).
    /// If none has been set, lazily constructs and caches an [`ApproximateHeightSampler`]
    /// from the current hierarchy on first call.
    ///
    /// Returns an immediately-resolved [`Task`] with an empty result if the
    /// tileset is not yet ready.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let result = tileset.sample_heights(positions).block()?;
    /// ```
    pub fn sample_heights(&mut self, positions: Vec<Cartographic>) -> Task<SampleHeightResult> {
        // Lazy-init default sampler once the engine is ready.
        if self.height_sampler.is_none() {
            if let TilesetState::Ready(engine) = &self.state {
                let sampler = crate::ApproximateHeightSampler::from_hierarchy(
                    engine.hierarchy(),
                    Ellipsoid::wgs84(),
                );
                self.height_sampler = Some(Arc::new(sampler));
            }
        }

        match &self.height_sampler {
            Some(sampler) => sampler.sample_heights(positions),
            None => {
                // Engine not ready yet — return an immediately-resolved empty result.
                let n = positions.len();
                orkester::resolved(SampleHeightResult {
                    positions,
                    sample_success: vec![false; n],
                    warnings: vec!["Tileset is not yet ready".into()],
                })
            }
        }
    }

    /// Direct access to the inner [`SelectionEngine`] once loading is complete.
    ///
    /// Returns `None` while loading or after a failure.
    pub fn engine(&self) -> Option<&SelectionEngine<C>> {
        match &self.state {
            TilesetState::Ready(engine) => Some(engine),
            _ => None,
        }
    }

    /// Mutable access to the inner [`SelectionEngine`] once loading is complete.
    pub fn engine_mut(&mut self) -> Option<&mut SelectionEngine<C>> {
        match &mut self.state {
            TilesetState::Ready(engine) => Some(engine),
            _ => None,
        }
    }

    /// Returns the bounding volume of the root tile, or `None` if the engine is not yet ready.
    pub fn root_bounds(&self) -> Option<&zukei::SpatialBounds> {
        match &self.state {
            TilesetState::Ready(engine) => {
                let h = engine.hierarchy();
                Some(h.bounds(h.root()))
            }
            _ => None,
        }
    }

    /// Check whether the background loading task has completed and transition
    /// state if so. Called automatically by all per-frame methods.
    fn poll_loading(&mut self) {
        let ready = match &self.state {
            TilesetState::Loading(task) => task.is_ready(),
            _ => return,
        };

        if ready {
            let TilesetState::Loading(task) =
                std::mem::replace(&mut self.state, TilesetState::Consumed)
            else {
                unreachable!()
            };

            // Will not block — we confirmed is_ready() above.
            let load_result = task.block();
            let (new_state, ready_result) = match load_result {
                Ok(Ok((engine, attr))) => {
                    self.attribution = attr;
                    (TilesetState::Ready(engine), Ok(()))
                }
                Ok(Err(e)) => (TilesetState::Failed, Err(Arc::new(e))),
                Err(e) => (
                    TilesetState::Failed,
                    Err(Arc::new(Tiles3dError::Decode(Box::new(e)))),
                ),
            };
            self.state = new_state;

            // Signal `when_ready()` waiters with success or the load error.
            if let Some(resolver) = self.ready_resolver.take() {
                resolver.resolve(ready_result);
            }

            // Fire `on_ready` callbacks. Move the vec out first so `cb(self)`
            // can take `&mut self` without the vec being borrowed simultaneously.
            let callbacks = std::mem::take(&mut self.on_ready_callbacks);
            for cb in callbacks {
                cb(self);
            }
        }
    }

    /// Returns a cloneable [`SharedTask`] that resolves once the tileset is
    /// ready (or fails to load).
    ///
    /// Resolves `Ok(())` on success, `Err(Tiles3dError)` on failure.
    /// Useful for external coordination — awaiting multiple tilesets, async
    /// pipelines, etc.
    ///
    /// For work that needs to mutate *this* tileset on load, prefer
    /// [`on_ready`](Self::on_ready).
    ///
    /// ```ignore
    /// tileset.when_ready()
    ///     .then(Context::BACKGROUND, |result| {
    ///         result?; // propagate load error
    ///         Ok(())
    ///     });
    /// ```
    pub fn when_ready(&self) -> Handle<Result<(), Arc<Tiles3dError>>> {
        self.ready.clone()
    }

    /// Register a one-shot callback that receives `&mut Tileset<C>` the frame
    /// the tileset becomes ready.
    ///
    /// Multiple callbacks can be registered; they fire in registration order.
    /// If the tileset is already ready when this is called, the callback fires
    /// on the next [`update`](Self::update) call.
    ///
    /// No cloning or shared ownership needed:
    ///
    /// ```ignore
    /// tileset.on_ready(|t| {
    ///     camera.look_at(t.root().unwrap());
    ///     t.set_height_sampler(MyTerrainSampler::new());
    /// });
    /// ```
    pub fn on_ready(&mut self, f: impl FnOnce(&mut Tileset<C>) + 'static) {
        self.on_ready_callbacks.push(Box::new(f));
    }
}
