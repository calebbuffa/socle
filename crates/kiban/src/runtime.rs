//! The `Kiban<C>` runtime — a format-agnostic streaming spatial data layer.
//!
//! Owns a [`NodeStore`] and [`SelectionState`], calls [`selekt::select()`] each
//! frame, dispatches content loads, and coordinates overlay draping.

use std::collections::HashSet;
use std::sync::Arc;

use crate::content_cache::ContentCache;
use std::time::{Duration, Instant};

use glam::DMat4;
use orkester::{CancellationToken, Context, Handle, Resolver, Task, WorkQueue};
use orkester_io::AssetAccessor;
use selekt::{
    ContentKey, CullingOptions, DebugOptions, EMPTY_FRAME_DECISION, FrameDecision,
    FrustumVisibilityPolicy, LoadingOptions, LodRefinementOptions, NoOcclusion, NodeExcluder,
    NodeId, NodeLoadState, NodeStore, OcclusionTester, SelectionBuffers, SelectionOptions,
    SelectionState, StreamingOptions, ViewState, VisibilityPolicy,
};
use tiles3d_selekt::{GeometricErrorEvaluator, LoadResult, TilesetLoader};

use crate::event::Event;
use kasane::OverlayEvent;

#[derive(Clone, Debug)]
pub struct StratumOptions {
    pub loading: LoadingOptions,
    pub culling: CullingOptions,
    pub lod: LodRefinementOptions,
    pub streaming: StreamingOptions,
    pub debug: DebugOptions,
    pub ellipsoid: terra::Ellipsoid,
    /// Time budget for draining the main-thread finalization queue per call to
    /// `load_nodes`. Mirrors cesium-native's `mainThreadLoadingTimeLimit`.
    /// Defaults to 16 ms (one frame at 60 fps).
    pub main_thread_load_time_limit: Duration,
}

impl Default for StratumOptions {
    fn default() -> Self {
        Self {
            loading: LoadingOptions::default(),
            culling: CullingOptions::default(),
            lod: LodRefinementOptions::default(),
            streaming: StreamingOptions::default(),
            debug: DebugOptions::default(),
            ellipsoid: terra::Ellipsoid::default(),
            main_thread_load_time_limit: Duration::from_millis(16),
        }
    }
}

impl StratumOptions {
    pub fn to_selection_options(&self) -> SelectionOptions {
        SelectionOptions {
            loading: self.loading.clone(),
            culling: self.culling.clone(),
            lod: self.lod.clone(),
            streaming: self.streaming.clone(),
            debug: self.debug.clone(),
        }
    }
}

struct HierarchyBridge<'a>(&'a NodeStore);

impl kasane::OverlayHierarchy for HierarchyBridge<'_> {
    fn parent(&self, node: u64) -> Option<u64> {
        let nid = NodeId(std::num::NonZeroU64::new(node)?);
        self.0.parent(nid).map(|p| p.0.get())
    }
    fn globe_rectangle(&self, node: u64) -> Option<terra::GlobeRectangle> {
        let nid = NodeId(std::num::NonZeroU64::new(node)?);
        self.0.globe_rectangle(nid)
    }
}

#[inline]
fn node_to_u64(id: NodeId) -> u64 {
    id.0.get()
}

#[inline]
fn u64_to_node(id: u64) -> Option<NodeId> {
    std::num::NonZeroU64::new(id).map(NodeId)
}

struct InFlightLoad<C: Send + 'static> {
    node_id: NodeId,
    task: Task<Result<LoadResult<C>, tiles3d_selekt::Tiles3dError>>,
    cancel: CancellationToken,
}

/// Fade transition state of a node yielded by [`Kiban::render_nodes`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FadeState {
    /// Steady-state rendering, no active transition.
    Normal,
    /// Node just entered the render set; fade it in.
    FadingIn,
    /// Node just left the render set; content is still resident while fading out.
    FadingOut,
}

/// A single render-ready node yielded by [`Kiban::render_nodes`].
pub struct RenderNode<'a, C> {
    pub id: NodeId,
    pub world_transform: DMat4,
    pub content: &'a C,
    /// Fade transition state. Use this to drive renderer-side alpha.
    pub fade_state: FadeState,
}

/// Fully-resolved overlay attach data, ready for manager-side application.
pub struct OverlayAttachEvent {
    pub node_id: NodeId,
    pub overlay_id: kasane::OverlayId,
    pub uv_index: u32,
    pub tile: kasane::RasterOverlayTile,
    pub translation: [f64; 2],
    pub scale: [f64; 2],
}

/// Overlay mutation produced by the overlay engine for this frame.
pub enum OverlayLifecycleEvent {
    Attach(OverlayAttachEvent),
    Detach {
        node_id: NodeId,
        overlay_id: kasane::OverlayId,
    },
}

/// Main-thread events produced by a frame.
pub enum MainThreadEvent {
    Overlay(OverlayLifecycleEvent),
    ContentEvicted { node_id: NodeId },
}

// ── State machine ────────────────────────────────────────────────────────────

enum State<C: Send + 'static> {
    /// Background task is still running (async tileset.json fetch, etc.).
    Loading(
        Task<Result<tiles3d_selekt::ReadyTileset<C>, Box<dyn std::error::Error + Send + Sync>>>,
    ),
    /// Ready state.
    Ready(ReadyState<C>),
    /// Load failed.
    Failed,
    /// Transient — only during poll_loading swap.
    Consumed,
}

struct ReadyState<C: Send + 'static> {
    store: NodeStore,
    selection_state: SelectionState,
    buffers: SelectionBuffers,
    lod_evaluator: GeometricErrorEvaluator,
    visibility: Box<dyn VisibilityPolicy>,
    excluders: Vec<Box<dyn NodeExcluder>>,
    occlusion: Box<dyn OcclusionTester>,
    options: SelectionOptions,
    /// Resident tile content with memory-budget enforcement.
    cache: ContentCache<C>,
    /// Content loader.
    loader: TilesetLoader<C>,
    /// Background context for dispatching loads.
    bg_ctx: Context,
    /// Main-thread context for finalization work (used by the pipeline when two-phase
    /// prepare is split across background + main thread).
    #[allow(dead_code)]
    main_ctx: Context,
    /// In-flight loads.
    in_flight: Vec<InFlightLoad<C>>,
    /// Last frame decision.
    last_decision: FrameDecision,
}

/// A streaming spatial data layer.
///
/// `Kiban<C>` is the format-agnostic runtime that drives tile selection,
/// raster overlay draping, and lifecycle events.
pub struct Kiban<C: Send + 'static> {
    state: State<C>,
    attribution: Option<Arc<str>>,
    events: Vec<Event>,
    pending_main_thread_events: Vec<MainThreadEvent>,
    /// SSE threshold used for overlay LOD selection.
    maximum_screen_space_error: f64,
    /// Background context for async work.
    bg_ctx: Context,
    /// Main-thread work queue for time-budgeted finalization.
    main_queue: WorkQueue,
    /// Per-frame time budget for draining the main-thread finalization queue.
    main_thread_load_time_limit: Duration,
    /// Raster overlay engine.
    pub overlays: kasane::OverlayEngine,
    /// Resolves once the engine transitions to Ready or Failed.
    ready: Handle<Result<(), Arc<dyn std::error::Error + Send + Sync>>>,
    ready_resolver: Option<Resolver<Result<(), Arc<dyn std::error::Error + Send + Sync>>>>,
    on_ready_callbacks: Vec<Box<dyn FnOnce(&mut Kiban<C>) + 'static>>,
}

impl<C: Send + 'static> Kiban<C> {
    /// Create a layer from an already-constructed tileset (no async loading).
    pub fn ready(
        ready_tileset: tiles3d_selekt::ReadyTileset<C>,
        accessor: Arc<dyn AssetAccessor>,
        bg_ctx: Context,
    ) -> Self {
        let overlays = kasane::OverlayEngine::new(Arc::clone(&accessor), bg_ctx.clone());
        let (resolver, task) =
            orkester::pair::<Result<(), Arc<dyn std::error::Error + Send + Sync>>>();
        resolver.resolve(Ok(()));

        let store =
            NodeStore::from_descriptors(&ready_tileset.descriptors, ready_tileset.root_index, 0);
        let attribution = ready_tileset.attribution.clone();
        let sse = ready_tileset.maximum_screen_space_error;
        let main_queue = WorkQueue::new();
        let main_ctx = main_queue.context();

        Self {
            state: State::Ready(ReadyState {
                store,
                selection_state: SelectionState::new(),
                buffers: SelectionBuffers::new(),
                lod_evaluator: ready_tileset.lod_evaluator,
                visibility: Box::new(FrustumVisibilityPolicy),
                excluders: Vec::new(),
                occlusion: Box::new(NoOcclusion),
                options: SelectionOptions::default(),
                cache: ContentCache::new(SelectionOptions::default().loading.max_cached_bytes),
                loader: ready_tileset.loader,
                bg_ctx: bg_ctx.clone(),
                main_ctx,
                in_flight: Vec::new(),
                last_decision: FrameDecision::default(),
            }),
            attribution,
            events: Vec::new(),
            pending_main_thread_events: Vec::new(),
            maximum_screen_space_error: sse,
            bg_ctx,
            main_queue,
            main_thread_load_time_limit: Duration::from_millis(16),
            overlays,
            ready: task.share(),
            ready_resolver: None,
            on_ready_callbacks: Vec::new(),
        }
    }

    /// Create a layer from an async task that will produce the tileset.
    pub fn from_task<E>(
        task: Task<Result<tiles3d_selekt::ReadyTileset<C>, E>>,
        accessor: Arc<dyn AssetAccessor>,
        bg_ctx: Context,
    ) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        let erased = task
            .map(|r| r.map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) }));

        let overlays = kasane::OverlayEngine::new(accessor, bg_ctx.clone());
        let (resolver, ready_task) =
            orkester::pair::<Result<(), Arc<dyn std::error::Error + Send + Sync>>>();
        Self {
            state: State::Loading(erased),
            attribution: None,
            events: Vec::new(),
            pending_main_thread_events: Vec::new(),
            maximum_screen_space_error: 16.0,
            bg_ctx,
            main_queue: WorkQueue::new(),
            main_thread_load_time_limit: Duration::from_millis(16),
            overlays,
            ready: ready_task.share(),
            ready_resolver: Some(resolver),
            on_ready_callbacks: Vec::new(),
        }
    }

    /// Returns `true` once the engine is ready.
    pub fn is_ready(&self) -> bool {
        matches!(self.state, State::Ready(_))
    }

    /// A cloneable handle that resolves when loading finishes (or fails).
    pub fn when_ready(&self) -> Handle<Result<(), Arc<dyn std::error::Error + Send + Sync>>> {
        self.ready.clone()
    }

    /// Register a callback that fires the frame the engine becomes ready.
    pub fn on_ready(&mut self, f: impl FnOnce(&mut Kiban<C>) + 'static) {
        self.on_ready_callbacks.push(Box::new(f));
    }

    /// Attribution / copyright string, if available.
    pub fn attribution(&self) -> Option<&str> {
        self.attribution.as_deref()
    }

    /// Set attribution manually.
    pub fn set_attribution(&mut self, text: impl Into<Arc<str>>) {
        self.attribution = Some(text.into());
    }

    /// Set the SSE threshold used for overlay LOD selection.
    pub fn set_maximum_screen_space_error(&mut self, value: f64) {
        self.maximum_screen_space_error = value;
    }

    /// Updates a view group, equivalent to cesium-native `Tileset::updateViewGroup`.
    pub fn update_view_group(&mut self, views: &[ViewState], _dt: f32) -> &FrameDecision {
        self.poll_loading();
        let State::Ready(s) = &mut self.state else {
            return &EMPTY_FRAME_DECISION;
        };

        // Run selection.
        s.selection_state.advance_frame();
        let loader = &s.loader;
        let decision = selekt::select(
            &mut s.store,
            &mut s.selection_state,
            &s.options,
            views,
            &s.lod_evaluator,
            s.visibility.as_ref(),
            &s.excluders,
            s.occlusion.as_ref(),
            &mut s.buffers,
            &mut |_node_id, node_data| loader.expand(node_data),
        );

        s.last_decision = decision;

        // Evict content that exceeds the memory budget.
        // Nodes currently in the render list are never evicted this frame.
        let evicted_ids = evict_excess_content(s);

        // Drive overlay engine.
        if let Some(view) = views.first() {
            let (viewport_height, sse_denom) = match view.projection {
                selekt::Projection::Perspective { fov_y, .. } => {
                    (view.viewport_px[1] as f64, 2.0 * (fov_y * 0.5).tan())
                }
                selekt::Projection::Orthographic { half_height, .. } => {
                    (view.viewport_px[1] as f64, 2.0 * half_height)
                }
            };
            self.overlays.set_view_info(kasane::OverlayViewInfo {
                viewport_height,
                sse_denominator: sse_denom,
                maximum_screen_space_error: self.maximum_screen_space_error,
            });
        }

        let State::Ready(s) = &mut self.state else {
            unreachable!();
        };

        let nodes_with_error: Vec<(u64, f64)> = s
            .last_decision
            .render
            .iter()
            .map(|&id| {
                let ge = s.store.lod_descriptor(id).value;
                (node_to_u64(id), ge)
            })
            .collect();
        self.overlays
            .update(&nodes_with_error, &HierarchyBridge(&s.store));

        // Collect overlay events.
        for e in self.overlays.drain_events() {
            match &e {
                OverlayEvent::Attached {
                    node_id,
                    overlay_id,
                    uv_index,
                    tile,
                } => {
                    if let Some(nid) = u64_to_node(*node_id) {
                        if let Some(geo_rect) = s.store.globe_rectangle(nid) {
                            let overlay_rect = &tile.rectangle;
                            let geo_w = geo_rect.east - geo_rect.west;
                            let geo_h = geo_rect.north - geo_rect.south;
                            let ov_w = (overlay_rect.east - overlay_rect.west).max(f64::EPSILON);
                            let ov_h = (overlay_rect.north - overlay_rect.south).max(f64::EPSILON);

                            let translation = [
                                (geo_rect.west - overlay_rect.west) / ov_w,
                                (overlay_rect.north - geo_rect.north) / ov_h,
                            ];
                            let scale = [geo_w / ov_w, geo_h / ov_h];

                            self.pending_main_thread_events
                                .push(MainThreadEvent::Overlay(OverlayLifecycleEvent::Attach(
                                    OverlayAttachEvent {
                                        node_id: nid,
                                        overlay_id: *overlay_id,
                                        uv_index: *uv_index,
                                        tile: tile.clone(),
                                        translation,
                                        scale,
                                    },
                                )));
                        }
                    }
                }
                OverlayEvent::Detached {
                    node_id,
                    overlay_id,
                } => {
                    if let Some(nid) = u64_to_node(*node_id) {
                        self.pending_main_thread_events
                            .push(MainThreadEvent::Overlay(OverlayLifecycleEvent::Detach {
                                node_id: nid,
                                overlay_id: *overlay_id,
                            }));
                    }
                }
                _ => {}
            }
            self.events.push(Event::Overlay(e));
        }

        for node_id in evicted_ids {
            self.events.push(Event::ContentEvicted { node_id });
            self.pending_main_thread_events
                .push(MainThreadEvent::ContentEvicted { node_id });
        }

        let State::Ready(s) = &self.state else {
            unreachable!();
        };
        &s.last_decision
    }

    /// Offline/blocking update variant, equivalent to cesium-native
    /// `Tileset::updateViewGroupOffline`.
    pub fn update_view_group_offline(&mut self, views: &[ViewState]) -> &FrameDecision {
        self.update_view_group(views, 0.0);
        while self.has_pending_work() {
            self.load_nodes();
            if views.is_empty() {
                break;
            }
            self.update_view_group(views, 0.0);
        }
        self.last_decision()
    }

    /// Processes tile loading work, equivalent to cesium-native `Tileset::loadTiles`.
    pub fn load_nodes(&mut self) {
        self.poll_loading();
        let State::Ready(s) = &mut self.state else {
            return;
        };
        poll_in_flight_loads(s, &mut self.events);
        dispatch_tile_loads(s);
        self.main_queue
            .flush_timed(self.main_thread_load_time_limit);
    }

    /// Compatibility wrapper that preserves previous behavior.
    pub fn update(&mut self, views: &[ViewState], dt: f32) -> &FrameDecision {
        self.update_view_group(views, dt);
        self.load_nodes();
        self.last_decision()
    }

    /// Drain all events accumulated since the last drain.
    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Drain main-thread events produced by the last frame(s).
    pub fn drain_main_thread_events(&mut self) -> Vec<MainThreadEvent> {
        std::mem::take(&mut self.pending_main_thread_events)
    }

    /// Drain pending overlay lifecycle events.
    ///
    /// This is a compatibility helper over [`drain_main_thread_events`](Self::drain_main_thread_events).
    pub fn drain_pending_overlay_events(&mut self) -> Vec<OverlayLifecycleEvent> {
        self.drain_main_thread_events()
            .into_iter()
            .filter_map(|e| match e {
                MainThreadEvent::Overlay(overlay) => Some(overlay),
                MainThreadEvent::ContentEvicted { .. } => None,
            })
            .collect()
    }

    /// Apply pending overlay attach/detach events to resident content.
    ///
    /// This is intentionally separate from [`update`](Self::update) so callers
    /// can own when and where renderer/content mutations happen.
    pub fn apply_pending_overlays_to_content(&mut self)
    where
        C: kasane::OverlayTarget,
    {
        let pending = self.drain_pending_overlay_events();
        let State::Ready(s) = &mut self.state else {
            return;
        };

        for e in pending {
            match e {
                OverlayLifecycleEvent::Attach(attach) => {
                    if let Some(content) = s.cache.get_mut(attach.node_id) {
                        content.attach_raster(&attach.tile, attach.translation, attach.scale);
                    }
                }
                OverlayLifecycleEvent::Detach {
                    node_id,
                    overlay_id,
                } => {
                    if let Some(content) = s.cache.get_mut(node_id) {
                        content.detach_raster(overlay_id);
                    }
                }
            }
        }
    }

    /// Stage 3: dispatch main-thread events.
    ///
    /// Compatibility shim: currently applies overlay lifecycle events to content,
    /// then drains any remaining main-thread events.
    pub fn dispatch_main_thread_events(&mut self) -> Vec<MainThreadEvent>
    where
        C: kasane::OverlayTarget,
    {
        self.apply_pending_overlays_to_content();
        self.drain_main_thread_events()
    }

    /// Frame result from the most recent [`update`] call.
    pub fn last_decision(&self) -> &FrameDecision {
        match &self.state {
            State::Ready(s) => &s.last_decision,
            _ => &EMPTY_FRAME_DECISION,
        }
    }

    /// Iterator over render-ready nodes with their content.
    ///
    /// Yields nodes in the current render set (`FadeState::Normal` or
    /// `FadeState::FadingIn`) followed by nodes that just left the render set
    /// and are fading out (`FadeState::FadingOut`). Fading-out nodes remain
    /// content-resident and have valid world transforms.
    pub fn render_nodes(&self) -> impl Iterator<Item = RenderNode<'_, C>> + '_ {
        match &self.state {
            State::Ready(s) => {
                let render_iter = s.last_decision.render.iter().filter_map(|&id| {
                    let content = s.cache.get(id)?;
                    let fade_state = if s.last_decision.fading_in.contains(&id) {
                        FadeState::FadingIn
                    } else {
                        FadeState::Normal
                    };
                    Some(RenderNode {
                        id,
                        world_transform: s.store.world_transform(id),
                        content,
                        fade_state,
                    })
                });
                let fading_out_iter = s.last_decision.fading_out.iter().filter_map(|&id| {
                    let content = s.cache.get(id)?;
                    Some(RenderNode {
                        id,
                        world_transform: s.store.world_transform(id),
                        content,
                        fade_state: FadeState::FadingOut,
                    })
                });
                Box::new(render_iter.chain(fading_out_iter))
                    as Box<dyn Iterator<Item = RenderNode<'_, C>>>
            }
            _ => Box::new(std::iter::empty()),
        }
    }

    /// Read-only access to a node's content.
    pub fn content(&self, node_id: NodeId) -> Option<&C> {
        match &self.state {
            State::Ready(s) => s.cache.get(node_id),
            _ => None,
        }
    }

    /// Mutable access to a node's content.
    pub fn content_mut(&mut self, node_id: NodeId) -> Option<&mut C> {
        match &mut self.state {
            State::Ready(s) => s.cache.get_mut(node_id),
            _ => None,
        }
    }

    /// Primary content key (URL) for a node.
    pub fn content_key(&self, node_id: NodeId) -> Option<&ContentKey> {
        match &self.state {
            State::Ready(s) => s.store.content_keys(node_id).first(),
            _ => None,
        }
    }

    /// Total bytes of resident content.
    pub fn total_data_bytes(&self) -> usize {
        match &self.state {
            State::Ready(s) => s.cache.total_bytes(),
            _ => 0,
        }
    }

    /// Number of nodes with resident content.
    pub fn resident_node_count(&self) -> usize {
        match &self.state {
            State::Ready(s) => s.cache.len(),
            _ => 0,
        }
    }

    /// The node store, if ready.
    pub fn store(&self) -> Option<&NodeStore> {
        match &self.state {
            State::Ready(s) => Some(&s.store),
            _ => None,
        }
    }

    /// Root node ID, if ready.
    pub fn root(&self) -> Option<NodeId> {
        self.store().map(|s| s.root())
    }

    /// Bounding volume of the root tile.
    pub fn root_bounds(&self) -> Option<&zukei::SpatialBounds> {
        self.store().map(|s| s.bounds(s.root()))
    }

    /// Add an excluder that skips subtrees during traversal.
    pub fn add_excluder(&mut self, excluder: impl NodeExcluder + 'static) {
        if let State::Ready(s) = &mut self.state {
            s.excluders.push(Box::new(excluder));
        }
    }

    /// Clear all excluders.
    pub fn clear_excluders(&mut self) {
        if let State::Ready(s) = &mut self.state {
            s.excluders.clear();
        }
    }

    /// Replace the occlusion tester.
    pub fn set_occlusion_tester(&mut self, tester: impl OcclusionTester + 'static) {
        if let State::Ready(s) = &mut self.state {
            s.occlusion = Box::new(tester);
        }
    }

    /// Read-only access to selection options.
    pub fn options(&self) -> Option<&SelectionOptions> {
        match &self.state {
            State::Ready(s) => Some(&s.options),
            _ => None,
        }
    }

    /// Replace selection options.
    pub fn set_options(&mut self, options: SelectionOptions) {
        if let State::Ready(s) = &mut self.state {
            s.options = options;
        }
    }

    /// Current frame index.
    pub fn frame_index(&self) -> u64 {
        match &self.state {
            State::Ready(s) => s.selection_state.frame_index,
            _ => 0,
        }
    }

    /// Block until all pending loads complete or timeout elapses.
    pub fn wait_for_all_loads(&mut self, views: &[ViewState], timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            self.update(views, 0.0);
            let State::Ready(s) = &self.state else {
                return false;
            };
            if s.in_flight.is_empty() && s.last_decision.load.is_empty() {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn poll_loading(&mut self) {
        let ready = match &self.state {
            State::Loading(task) => task.is_ready(),
            _ => return,
        };

        if !ready {
            return;
        }

        let State::Loading(task) = std::mem::replace(&mut self.state, State::Consumed) else {
            unreachable!()
        };

        let load_result = task.block();
        let (new_state, ready_result) = match load_result {
            Ok(Ok(ready_tileset)) => {
                self.attribution = ready_tileset.attribution.clone();
                self.maximum_screen_space_error = ready_tileset.maximum_screen_space_error;
                let store = NodeStore::from_descriptors(
                    &ready_tileset.descriptors,
                    ready_tileset.root_index,
                    0,
                );
                let bg_ctx = self.bg_ctx.clone();
                let main_ctx = self.main_queue.context();
                let rs = ReadyState {
                    store,
                    selection_state: SelectionState::new(),
                    buffers: SelectionBuffers::new(),
                    lod_evaluator: ready_tileset.lod_evaluator,
                    visibility: Box::new(FrustumVisibilityPolicy),
                    excluders: Vec::new(),
                    occlusion: Box::new(NoOcclusion),
                    options: SelectionOptions::default(),
                    cache: ContentCache::new(SelectionOptions::default().loading.max_cached_bytes),
                    loader: ready_tileset.loader,
                    bg_ctx,
                    main_ctx,
                    in_flight: Vec::new(),
                    last_decision: FrameDecision::default(),
                };
                (State::Ready(rs), Ok(()))
            }
            Ok(Err(e)) => {
                let arc: Arc<dyn std::error::Error + Send + Sync> = Arc::from(e);
                (State::Failed, Err(arc))
            }
            Err(e) => {
                let arc: Arc<dyn std::error::Error + Send + Sync> = Arc::new(e);
                (State::Failed, Err(arc))
            }
        };
        self.state = new_state;

        if let Some(resolver) = self.ready_resolver.take() {
            resolver.resolve(ready_result);
        }

        let callbacks = std::mem::take(&mut self.on_ready_callbacks);
        for cb in callbacks {
            cb(self);
        }
    }

    fn has_pending_work(&self) -> bool {
        match &self.state {
            State::Ready(s) => !s.in_flight.is_empty() || !s.last_decision.load.is_empty(),
            _ => false,
        }
    }
}

/// Concrete runtime for glTF-backed streamed content.
///
/// This is the concrete, non-generic runtime surface that higher-level adapters
/// (e.g. Cesium 3D Tiles) should target.
pub struct Stratum {
    inner: Kiban<moderu::GltfModel>,
    pub options: StratumOptions,
}

impl Stratum {
    pub fn ready(
        ready_tileset: tiles3d_selekt::ReadyTileset<moderu::GltfModel>,
        accessor: Arc<dyn AssetAccessor>,
        bg_ctx: Context,
        options: StratumOptions,
    ) -> Self {
        let mut inner = Kiban::ready(ready_tileset, accessor, bg_ctx);
        inner.set_options(options.to_selection_options());
        Self { inner, options }
    }

    pub fn from_task<E>(
        task: Task<Result<tiles3d_selekt::ReadyTileset<moderu::GltfModel>, E>>,
        accessor: Arc<dyn AssetAccessor>,
        bg_ctx: Context,
        options: StratumOptions,
    ) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        let mut inner = Kiban::from_task(task, accessor, bg_ctx);
        inner.set_options(options.to_selection_options());
        Self { inner, options }
    }

    pub fn is_ready(&self) -> bool {
        self.inner.is_ready()
    }

    pub fn when_ready(&self) -> Handle<Result<(), Arc<dyn std::error::Error + Send + Sync>>> {
        self.inner.when_ready()
    }

    pub fn update_view_group(&mut self, views: &[ViewState], dt_seconds: f32) -> &FrameDecision {
        self.inner.update_view_group(views, dt_seconds)
    }

    pub fn update_view_group_offline(&mut self, views: &[ViewState]) -> &FrameDecision {
        self.inner.update_view_group_offline(views)
    }

    pub fn load_nodes(&mut self) {
        self.inner.load_nodes();
    }

    pub fn dispatch_main_thread_events(&mut self) -> Vec<MainThreadEvent> {
        self.inner.drain_main_thread_events()
    }

    pub fn drain_events(&mut self) -> Vec<Event> {
        self.inner.drain_events()
    }

    pub fn last_decision(&self) -> &FrameDecision {
        self.inner.last_decision()
    }

    pub fn render_nodes(&self) -> impl Iterator<Item = RenderNode<'_, moderu::GltfModel>> + '_ {
        self.inner.render_nodes()
    }

    pub fn content(&self, node_id: NodeId) -> Option<&moderu::GltfModel> {
        self.inner.content(node_id)
    }

    pub fn content_mut(&mut self, node_id: NodeId) -> Option<&mut moderu::GltfModel> {
        self.inner.content_mut(node_id)
    }

    pub fn overlays(&self) -> &kasane::OverlayCollection {
        self.inner.overlays.collection()
    }

    pub fn overlays_mut(&mut self) -> &kasane::OverlayCollection {
        self.inner.overlays.collection()
    }

    pub fn store(&self) -> Option<&NodeStore> {
        self.inner.store()
    }

    pub fn root(&self) -> Option<NodeId> {
        self.inner.root()
    }

    pub fn apply_options(&mut self) {
        self.inner.set_options(self.options.to_selection_options());
    }

    pub fn as_kiban(&self) -> &Kiban<moderu::GltfModel> {
        &self.inner
    }

    pub fn as_kiban_mut(&mut self) -> &mut Kiban<moderu::GltfModel> {
        &mut self.inner
    }
}

/// Poll in-flight loads and promote completed ones.
fn poll_in_flight_loads<C: Send + 'static>(s: &mut ReadyState<C>, _events: &mut Vec<Event>) {
    let mut i = 0;
    while i < s.in_flight.len() {
        if !s.in_flight[i].task.is_ready() {
            i += 1;
            continue;
        }
        let load = s.in_flight.swap_remove(i);
        let result = load.task.block();
        match result {
            Ok(Ok(LoadResult::Renderable { content, byte_size })) => {
                s.cache.insert(load.node_id, content, byte_size);
                s.selection_state.mark_renderable(load.node_id);
            }
            Ok(Ok(LoadResult::SubScene {
                descriptors,
                root_index: _,
                byte_size: _,
            })) => {
                // Insert sub-scene nodes under the parent node.
                let segment = s.store.segment(load.node_id);
                s.store.insert_children(load.node_id, &descriptors, segment);
                // Mark as renderable even though it has no geometry —
                // the traversal will visit the new children next frame.
                s.selection_state.mark_renderable(load.node_id);
                // Mark node as unconditionally refined (it's a reference, not geometry).
                s.store.get_mut(load.node_id).unconditionally_refined = true;
            }
            Ok(Ok(LoadResult::Empty)) => {
                s.selection_state.mark_renderable(load.node_id);
            }
            Ok(Err(_e)) => {
                s.selection_state.mark_failed(load.node_id);
            }
            Err(_cancelled) => {
                // Task was cancelled — revert to unloaded.
                s.selection_state.get_mut(load.node_id).lifecycle = NodeLoadState::Unloaded;
            }
        }
        // Don't increment i — swap_remove moved the last element to position i.
    }
}

fn dispatch_tile_loads<C: Send + 'static>(s: &mut ReadyState<C>) {
    let max_in_flight = s.options.loading.max_simultaneous_loads;
    for req in &s.last_decision.load {
        if s.in_flight.len() >= max_in_flight {
            break;
        }
        let lifecycle = s.selection_state.get(req.node_id).lifecycle;
        if lifecycle == NodeLoadState::Queued || lifecycle == NodeLoadState::Loading {
            continue;
        }
        if s.cache.contains(req.node_id) {
            continue;
        }
        let world_transform = s.store.world_transform(req.node_id);
        let cancel = CancellationToken::new();
        let task = s.loader.load(
            &s.bg_ctx,
            req.node_id,
            &req.key,
            world_transform,
            cancel.clone(),
        );
        s.selection_state.mark_loading(req.node_id);
        s.in_flight.push(InFlightLoad {
            node_id: req.node_id,
            task,
            cancel,
        });
    }
}

/// Evict the least-important content nodes to stay within the memory budget.
/// Returns the IDs of evicted nodes.
fn evict_excess_content<C: Send + 'static>(s: &mut ReadyState<C>) -> Vec<NodeId> {
    s.cache.set_max_bytes(s.options.loading.max_cached_bytes);
    if !s.cache.is_over_budget() {
        return Vec::new();
    }
    // Pin both the current render set AND nodes that are still fading out —
    // fading-out nodes are still being displayed by the renderer and must not
    // be evicted mid-transition.
    let mut pinned: HashSet<NodeId> = s.last_decision.render.iter().copied().collect();
    pinned.extend(s.last_decision.fading_out.iter().copied());
    let evicted = s
        .cache
        .evict(&pinned, |id| s.selection_state.get(id).importance);
    for &id in &evicted {
        s.selection_state.mark_evicted(id);
    }
    evicted
}
