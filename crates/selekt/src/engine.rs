use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use glam::DVec3;
use orkester::{Context, WorkQueue};

use crate::engine_state::EngineState;
use crate::format::NoopResolver;
use crate::frame::{FrameResult, PickResult};
use crate::hierarchy::{HierarchyExpansion, HierarchyExpansionError, HierarchyResolver, SpatialHierarchy};
use crate::load::{
    ContentKey, ContentLoader, HierarchyReference, LoadFailureDetails,
    LoadPassResult, LoadPriority, LoadResult, LoadedContent,
};
use crate::lod::LodEvaluator;
use crate::lod_threshold::LodThreshold;
use crate::node::{NodeId, NodeLoadState};
use crate::options::SelectionOptions;
use crate::policy::{
    FrustumVisibilityPolicy, NoOcclusion, NodeExcluder, OcclusionTester,
    Policy, ResidencyPolicy, VisibilityPolicy,
};
use crate::view::{ViewGroupHandle, ViewState, ViewUpdateResult};

// `ContentLoader` and `HierarchyResolver` each have an associated `Error` type
// that prevents them from being used as `dyn Trait` directly.  These internal
// wrappers erase the error into `Box<dyn Error>` so `SelectionEngine<C>` needs
// only a single `C` type parameter.

pub(crate) trait ErasedLoader<C: Send + 'static>: Send + Sync + 'static {
    /// Begin a load, returning a cancellation token and the load future.
    fn load_erased(
        &self,
        bg_context: &Context,
        main_context: &Context,
        node_id: NodeId,
        key: &ContentKey,
        priority: LoadPriority,
    ) -> (
        orkester::CancellationToken,
        orkester::Task<Result<LoadedContent<C>, Box<dyn std::error::Error + Send + Sync>>>,
    );
    fn free_erased(&self, content: C);
}

impl<C: Send + 'static, L: ContentLoader<C>> ErasedLoader<C> for L {
    fn load_erased(
        &self,
        bg_context: &Context,
        main_context: &Context,
        node_id: NodeId,
        key: &ContentKey,
        _priority: LoadPriority,
    ) -> (
        orkester::CancellationToken,
        orkester::Task<Result<LoadedContent<C>, Box<dyn std::error::Error + Send + Sync>>>,
    ) {
        let cancel = orkester::CancellationToken::new();
        let task = self
            .load(bg_context, main_context, node_id, key, cancel.clone())
            .map(|r| {
                r.map(|result| LoadedContent { result })
                    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })
            });
        (cancel, task)
    }

    fn free_erased(&self, content: C) {
        ContentLoader::free(self, content);
    }
}

pub(crate) trait ErasedResolver: Send + Sync + 'static {
    fn resolve_erased(
        &self,
        bg_context: &Context,
        reference: HierarchyReference,
    ) -> orkester::Task<Result<Option<HierarchyExpansion>, Box<dyn std::error::Error + Send + Sync>>>;
}

impl<R: HierarchyResolver> ErasedResolver for R {
    fn resolve_erased(
        &self,
        bg_context: &Context,
        reference: HierarchyReference,
    ) -> orkester::Task<Result<Option<HierarchyExpansion>, Box<dyn std::error::Error + Send + Sync>>>
    {
        self.resolve_reference(bg_context, reference)
            .map(|r| r.map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) }))
    }
}



struct PendingResolve {
    future: orkester::Task<
        Result<Option<HierarchyExpansion>, Box<dyn std::error::Error + Send + Sync>>,
    >,
    byte_size: usize,
}



/// Builder for constructing a [`SelectionEngine`].
///
/// Call [`SelectionEngineBuilder::new`] with the three required components, then
/// chain optional builder methods.  Finalize by calling [`.build(bg_context, main_context)`](SelectionEngineBuilder::build).
///
/// ```ignore
/// let engine = SelectionEngineBuilder::new(hierarchy, lod, loader)
///     .with_resolver(my_resolver)
///     .with_options(SelectionOptions { max_cached_bytes: 256 << 20, ..Default::default() })
///     .on_error(|details| eprintln!("tile {:?} failed: {}", details.node_id, details.message))
///     .build(bg_context, main_context);
/// ```
pub struct SelectionEngineBuilder<C: Send + 'static> {
    pub(crate) bg_context: Context,
    pub(crate) main_context: Context,
    pub(crate) hierarchy: Box<dyn SpatialHierarchy>,
    pub(crate) lod: Box<dyn LodEvaluator>,
    pub(crate) loader: Box<dyn ErasedLoader<C>>,
    pub(crate) resolver: Box<dyn ErasedResolver>,
    pub(crate) visibility: Box<dyn VisibilityPolicy>,
    pub(crate) residency: Box<dyn ResidencyPolicy>,
    pub(crate) options: SelectionOptions,
    pub(crate) on_error: Option<Box<dyn Fn(&LoadFailureDetails) + Send + Sync>>,
}

impl<C: Send + 'static> SelectionEngineBuilder<C> {
    /// Create a builder with the three required components.
    ///
    /// `bg_context` and `main_context` are passed to
    /// [`.build()`](Self::build); supplying them here is the two-phase style
    /// needed by `ContentLoaderFactory`. For ad-hoc construction prefer the
    /// `SelectionEngineBuilder::new(hierarchy, lod, loader).build(bg, main)` form.
    ///
    /// Defaults: [`AllVisibleLruPolicy`], no resolver
    /// (correct for formats that never emit [`LoadResult::Reference`]).
    pub fn new(
        bg_context: Context,
        hierarchy: impl SpatialHierarchy + 'static,
        lod: impl LodEvaluator + 'static,
        loader: impl ContentLoader<C> + 'static,
    ) -> Self {
        let main_context = WorkQueue::default().context();
        Self {
            bg_context,
            main_context,
            hierarchy: Box::new(hierarchy),
            lod: Box::new(lod),
            loader: Box::new(loader),
            resolver: Box::new(NoopResolver),
            visibility: Box::new(FrustumVisibilityPolicy),
            residency: Box::new(crate::policy::LruResidencyPolicy),
            options: SelectionOptions::default(),
            on_error: None,
        }
    }

    /// Override the `main_context` (GPU-upload thread context).
    ///
    /// Use this when sharing a [`WorkQueue`] across multiple engines or when
    /// the caller owns the queue and drives `pump()` externally.
    pub fn with_main_context(mut self, main_context: Context) -> Self {
        self.main_context = main_context;
        self
    }

    /// Construct the engine.
    pub fn build(self) -> SelectionEngine<C> {
        SelectionEngine::new(self)
    }

    /// Set the hierarchy resolver (needed for formats that emit [`LoadResult::Reference`]).
    pub fn with_resolver(mut self, resolver: impl HierarchyResolver + 'static) -> Self {
        self.resolver = Box::new(resolver);
        self
    }

    /// Override both visibility and residency with a combined [`Policy`] implementation.
    ///
    /// Accepts any `P: Policy + 'static` — including concrete types and `Box<dyn Policy>`.
    /// Internally wraps `policy` in an `Arc` so the single value can be shared across
    /// the two policy slots without requiring `Clone`.
    pub fn with_policy<P: Policy + 'static>(mut self, policy: P) -> Self {
        struct ArcPolicy<P>(Arc<P>);
        impl<P: VisibilityPolicy> VisibilityPolicy for ArcPolicy<P> {
            fn is_visible(&self, n: NodeId, b: &zukei::SpatialBounds, v: &crate::view::ViewState) -> bool {
                self.0.is_visible(n, b, v)
            }
        }
        impl<P: ResidencyPolicy> ResidencyPolicy for ArcPolicy<P> {
            fn select_evictions(&self, nodes: &[(NodeId, usize)], budget: usize, out: &mut Vec<NodeId>) {
                self.0.select_evictions(nodes, budget, out);
            }
        }
        let shared = Arc::new(policy);
        self.visibility = Box::new(ArcPolicy(shared.clone()));
        self.residency = Box::new(ArcPolicy(shared));
        self
    }

    /// Override the visibility policy. Default: [`FrustumVisibilityPolicy`].
    pub fn with_visibility_policy(mut self, policy: impl VisibilityPolicy + 'static) -> Self {
        self.visibility = Box::new(policy);
        self
    }

    /// Override the residency (eviction) policy. Default: [`LruResidencyPolicy`].
    pub fn with_residency_policy(mut self, policy: impl ResidencyPolicy + 'static) -> Self {
        self.residency = Box::new(policy);
        self
    }

    /// Override selection options (memory budget, concurrency, etc.).
    pub fn with_options(mut self, options: SelectionOptions) -> Self {
        self.options = options;
        self
    }

    /// Callback invoked when a node load or resolve fails permanently.
    pub fn on_error(
        mut self,
        callback: impl Fn(&LoadFailureDetails) + Send + Sync + 'static,
    ) -> Self {
        self.on_error = Some(Box::new(callback));
        self
    }

}

/// Format-agnostic selection engine.
///
/// Drives traversal → load scheduling → async fetch → content delivery.
///
/// `C` is the decoded content type (mesh, point cloud, etc.).
///
/// # Frame loop
///
/// ```ignore
/// let wq = WorkQueue::default();
/// let mut engine = SelectionEngineBuilder::new(bg_context, hierarchy, lod, loader)
///     .with_main_context(wq.context())
///     .build();
///
/// // Per frame:
/// let handle = engine.add_view_group(1.0);
/// engine.update_view_group(handle, &[view]);
/// engine.load();
/// wq.pump_timed(Duration::from_millis(4));  // GPU uploads, caller-driven
/// let result = engine.view_group_result(handle).unwrap();
/// ```
pub struct SelectionEngine<C: Send + 'static> {
    bg_context: Context,
    /// Context routing work to the main (GPU-upload) thread.
    main_context: Context,
    hierarchy: Box<dyn SpatialHierarchy>,
    lod: Box<dyn LodEvaluator>,
    resolver: Box<dyn ErasedResolver>,
    loader: Box<dyn ErasedLoader<C>>,
    visibility: Box<dyn VisibilityPolicy>,
    residency: Box<dyn ResidencyPolicy>,
    options: SelectionOptions,
    excluders: Vec<Box<dyn NodeExcluder>>,
    occlusion_tester: Box<dyn OcclusionTester>,
    on_load_error: Option<Box<dyn Fn(&LoadFailureDetails) + Send + Sync>>,
    /// In-flight: maps each node ID to its cancellation token and load future.
    in_flight: HashMap<
        NodeId,
        (
            orkester::CancellationToken,
            orkester::Task<Result<LoadedContent<C>, Box<dyn std::error::Error + Send + Sync>>>,
        ),
    >,
    /// Pending hierarchy-reference resolve futures.
    resolve_futures: HashMap<NodeId, PendingResolve>,
    /// Load requests produced by `step()` that have not yet been dispatched.
    /// Consumed by the next `load()` call.
    pending_requests: Vec<crate::step::LoadRequest>,
    /// Pure mutable frame-state (node lifecycles, resident content, view groups, etc.)
    state: EngineState<C>,
}

impl<C: Send + 'static> SelectionEngine<C> {
    /// Construct a new engine from a [`SelectionEngineBuilder`].
    pub(crate) fn new(config: SelectionEngineBuilder<C>) -> Self {
        Self {
            bg_context: config.bg_context,
            main_context: config.main_context,
            hierarchy: config.hierarchy,
            lod: config.lod,
            resolver: config.resolver,
            loader: config.loader,
            visibility: config.visibility,
            residency: config.residency,
            options: config.options,
            excluders: Vec::new(),
            occlusion_tester: Box::new(NoOcclusion),
            on_load_error: config.on_error,
            in_flight: HashMap::new(),
            resolve_futures: HashMap::new(),
            pending_requests: Vec::new(),
            state: EngineState::new(),
        }
    }

    /// Returns the frame result for a specific view group from its most recent
    /// [`update_view_group`](Self::update_view_group) call.
    ///
    /// Returns `None` if `handle` is no longer valid.
    pub fn view_group_result(&self, handle: ViewGroupHandle) -> Option<&FrameResult> {
        self.state.view_groups.get(handle).map(|slot| &slot.result)
    }

    /// Iterate over render-ready nodes for the given view group.
    ///
    /// Each [`RenderNode`] carries the node id, its accumulated world-space transform
    /// (ready to use as a GPU model-matrix uniform), and a reference to the
    /// renderer-owned content. Nodes without loaded content are skipped.
    ///
    /// Returns an empty iterator if `handle` is invalid.
    pub fn render_nodes(
        &self,
        handle: ViewGroupHandle,
    ) -> impl Iterator<Item = crate::frame::RenderNode<'_, C>> {
        let ids: &[NodeId] = self
            .state.view_groups
            .get(handle)
            .map(|slot| slot.result.nodes_to_render.as_slice())
            .unwrap_or(&[]);
        ids.iter().filter_map(|&id| {
            self.content(id).map(|content| crate::frame::RenderNode {
                id,
                world_transform: self.hierarchy.world_transform(id),
                content,
            })
        })
    }

    /// CPU-side ray pick against the given view group's current render set.
    ///
    /// Tests `ray_origin + t * ray_direction` against each rendered node's
    /// bounding volume. Returns hits sorted by ascending distance (front-to-back).
    /// Only currently-selected nodes are tested — no re-traversal occurs.
    ///
    /// All coordinates must be in the same CRS as the engine's hierarchy.
    pub fn pick(
        &self,
        handle: ViewGroupHandle,
        ray_origin: DVec3,
        ray_direction: DVec3,
    ) -> Vec<PickResult> {
        let ids: &[NodeId] = self
            .state.view_groups
            .get(handle)
            .map(|slot| slot.result.nodes_to_render.as_slice())
            .unwrap_or(&[]);
        let mut hits: Vec<PickResult> = ids
            .iter()
            .filter_map(|&id| {
                let bounds = self.hierarchy.bounds(id);
                crate::evaluators::ray_vs_bounds(ray_origin, ray_direction, bounds).map(
                    |distance| PickResult {
                        node_id: id,
                        distance,
                    },
                )
            })
            .collect();
        hits.sort_unstable_by(|a, b| {
            a.distance
                .partial_cmp(&b.distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits
    }

    /// Traverse the spatial hierarchy and return all nodes whose bounds intersect
    /// `shape`, independent of any camera or LOD evaluation.
    ///
    /// Traversal prunes entire branches that are entirely outside `shape`.
    /// Returns the deepest nodes reached given `depth` — internal nodes are only
    /// included when they have no children in the hierarchy.
    ///
    /// This method does **not** trigger any loading and is independent of previous
    /// [`update`] calls. All coordinates in `shape` must be in the same CRS as
    /// the engine's hierarchy.
    pub fn query(
        &self,
        shape: &crate::query::QueryShape,
        depth: crate::query::QueryDepth,
    ) -> Vec<NodeId> {
        use crate::query::{QueryDepth, shape_intersects_bounds};
        let mut result = Vec::new();
        // DFS stack: (node, current_level)
        let mut stack = vec![(self.hierarchy.root(), 0u32)];
        while let Some((node, level)) = stack.pop() {
            let bounds = self.hierarchy.bounds(node);
            if !shape_intersects_bounds(shape, bounds) {
                continue;
            }
            let children = self.hierarchy.children(node);
            let at_depth_limit = matches!(depth, QueryDepth::Level(n) if level >= n);
            if children.is_empty() || at_depth_limit {
                result.push(node);
            } else {
                for &child in children {
                    stack.push((child, level + 1));
                }
            }
        }
        result
    }

    /// Add a view group. `weight` controls fair-share load priority relative
    /// to other groups — use `1.0` for equal weight.
    ///
    /// For most cases the built-in default view group (used by [`update`](Self::update))
    /// is sufficient.  Call this only for multi-view setups.
    pub fn add_view_group(&mut self, weight: f64) -> ViewGroupHandle {
        self.state.view_groups.insert(weight)
    }

    pub fn remove_view_group(&mut self, handle: ViewGroupHandle) -> bool {
        self.state.view_groups.remove(handle)
    }

    pub fn is_view_group_active(&self, handle: ViewGroupHandle) -> bool {
        self.state.view_groups.get(handle).is_some()
    }

    /// Update a single view group, returning traversal statistics.
    ///
    /// Call once per view group per frame, then call [`load`](Self::load) to
    /// dispatch the queued requests.
    pub fn update_view_group(
        &mut self,
        handle: ViewGroupHandle,
        views: &[ViewState],
    ) -> ViewUpdateResult {
        debug_assert!(
            !views.is_empty(),
            "update_view_group called with zero views"
        );

        if self.state.view_groups.get(handle).is_none() {
            return ViewUpdateResult::default();
        }

        let completed = self.collect_completions();
        let view_pairs: &[(ViewGroupHandle, &[ViewState])] = &[(handle, views)];
        let output = crate::step::step(
            &mut self.state,
            &self.hierarchy,
            &self.lod,
            &*self.visibility,
            &*self.residency,
            &self.excluders,
            &*self.occlusion_tester,
            &self.options,
            self.on_load_error.as_ref().map(|f| f.as_ref() as &dyn Fn(&LoadFailureDetails)),
            crate::step::StepInput { view_groups: view_pairs, completed },
        );
        // Stash scheduled requests — they are dispatched by the next load() call.
        let queued = self.state.traversal_buffers.candidates.len();
        self.pending_requests.extend(output.requests_to_start);
        // Still apply cancellations and evictions immediately.
        for node_id in output.requests_to_cancel {
            if let Some((token, _)) = self.in_flight.remove(&node_id) {
                token.cancel();
            }
            self.resolve_futures.remove(&node_id);
        }
        for content in output.evicted_content {
            self.loader.free_erased(content);
        }

        let slot = self.state.view_groups.get(handle).unwrap();
        let result = &slot.result;
        ViewUpdateResult {
            nodes_occluded: result.nodes_occluded,
            nodes_kicked: result.nodes_kicked,
            nodes_visited: result.nodes_visited,
            nodes_culled: result.nodes_culled,
            queued_requests: queued,
            worker_thread_load_queue_length: self.in_flight.len() + self.resolve_futures.len(),
            frame_number: result.frame_number,
            nodes_fading_out: result.nodes_fading_out.len(),
            nodes_newly_renderable: result.nodes_newly_renderable,
        }
    }

    /// Update a view group and block until all nodes meeting the LOD threshold
    /// are loaded and ready to render.
    ///
    /// Significantly slower than [`update_view_group`](Self::update_view_group)
    /// — only use for movie capture or other non-realtime situations.
    ///
    /// The caller is responsible for pumping the main-thread work queue between
    /// iterations if GPU-upload tasks are in use.
    pub fn update_view_group_blocking(
        &mut self,
        handle: ViewGroupHandle,
        views: &[ViewState],
    ) -> ViewUpdateResult {
        const MAX_ITERATIONS: u32 = 10_000;
        for _ in 0..MAX_ITERATIONS {
            let result = self.update_view_group(handle, views);
            let load = self.load();
            if load.pending_worker_queue == 0 {
                return result;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        self.update_view_group(handle, views)
    }

    /// Dispatch queued load requests across all view groups.
    ///
    /// Call once per frame after all `update_view_group` calls.
    pub fn load(&mut self) -> LoadPassResult {
        // Dispatch any requests staged by update_view_group.
        let mut started = self.pending_requests.len();
        let pending = std::mem::take(&mut self.pending_requests);
        for req in pending {
            let (token, future) = self.loader.load_erased(
                &self.bg_context,
                &self.main_context,
                req.node_id,
                &req.key,
                req.priority,
            );
            self.in_flight.insert(req.node_id, (token, future));
        }

        // Also flush any late-arriving completions (resolve futures, etc.)
        let completed = self.collect_completions();
        if !completed.is_empty() {
            let output = crate::step::step(
                &mut self.state,
                &self.hierarchy,
                &self.lod,
                &*self.visibility,
                &*self.residency,
                &self.excluders,
                &*self.occlusion_tester,
                &self.options,
                self.on_load_error.as_ref().map(|f| f.as_ref() as &dyn Fn(&LoadFailureDetails)),
                crate::step::StepInput { view_groups: &[], completed },
            );
            started += output.requests_to_start.len();
            self.apply_step_output(output);
        }

        LoadPassResult {
            started_requests: started,
            completed_main_thread_tasks: 0,
            pending_worker_queue: self.in_flight.len() + self.resolve_futures.len(),
            nodes_newly_renderable: 0,
        }
    }

    pub fn frame_index(&self) -> u64 {
        self.state.frame_index
    }

    pub fn bg_context(&self) -> &Context {
        &self.bg_context
    }

    /// Returns a context that routes work to the main (GPU-upload) thread.
    pub fn main_context(&self) -> &orkester::Context {
        &self.main_context
    }

    /// Total bytes of currently-resident content.
    pub fn total_data_bytes(&self) -> usize {
        self.state.resident.total_bytes
    }

    /// Read-only access to the LOD threshold controller.
    pub fn lod_threshold(&self) -> &LodThreshold {
        &self.state.lod_threshold
    }

    /// Mutable access to the LOD threshold controller.
    ///
    /// Use this to override the base multiplier or reset the hysteresis state
    /// (e.g. after a camera teleport).
    pub fn lod_threshold_mut(&mut self) -> &mut LodThreshold {
        &mut self.state.lod_threshold
    }

    /// Number of nodes with content currently resident in memory.
    pub fn resident_node_count(&self) -> usize {
        self.state.resident.map.len()
    }

    /// Load progress as a percentage in `[0.0, 100.0]`.
    ///
    /// Returns `100.0` when there are no tracked nodes (nothing to load).
    /// Corresponds to Cesium's `computeLoadProgress()`.
    pub fn compute_load_progress(&self) -> f32 {
        let (tracked, renderable) = self.state.node_states.iter().fold((0usize, 0usize), |(t, r), s| {
            if s.lifecycle != NodeLoadState::Unloaded {
                (t + 1, r + usize::from(s.lifecycle == NodeLoadState::Renderable))
            } else {
                (t, r)
            }
        });
        if tracked == 0 { 100.0 } else { (renderable as f32 / tracked as f32) * 100.0 }
    }

    pub fn options(&self) -> &SelectionOptions {
        &self.options
    }

    /// Replace the current selection options.
    pub fn set_options(&mut self, options: SelectionOptions) {
        self.options = options;
    }

    /// Read-only access to the spatial hierarchy.
    pub fn hierarchy(&self) -> &dyn SpatialHierarchy {
        &*self.hierarchy
    }

    /// Expand the hierarchy with an externally-resolved patch.
    ///
    /// Call this when you receive a [`HierarchyExpansion`] from outside the engine
    /// (e.g. after manually resolving an external tileset reference).
    pub fn expand_hierarchy(&mut self, patch: HierarchyExpansion) -> Result<(), HierarchyExpansionError> {
        self.hierarchy.expand(patch)
    }

    /// Read-only access to loaded content for a resident node.
    ///
    /// Returns `None` if the node is not resident or has no renderable content.
    pub fn content(&self, node_id: NodeId) -> Option<&C> {
        self.state.resident.content(node_id)
    }

    /// Mutable access to loaded content for a resident node.
    pub fn content_mut(&mut self, node_id: NodeId) -> Option<&mut C> {
        self.state.resident.content_mut(node_id)
    }

    /// The primary content key (e.g. URL) for a node, as recorded in the hierarchy.
    ///
    /// Useful for debugging, logging, and cache-control identification.
    /// Returns `None` for structural nodes with no content.
    pub fn content_key(&self, node_id: NodeId) -> Option<&ContentKey> {
        self.hierarchy.content_keys(node_id).first()
    }

    /// All content keys for a node (multi-content nodes may have more than one).
    pub fn content_keys(&self, node_id: NodeId) -> &[ContentKey] {
        self.hierarchy.content_keys(node_id)
    }

    /// Add an excluder that will skip entire subtrees during traversal.
    pub fn add_excluder(&mut self, excluder: impl NodeExcluder + 'static) {
        self.excluders.push(Box::new(excluder));
    }

    pub fn excluders(&self) -> &[Box<dyn NodeExcluder>] {
        &self.excluders
    }

    pub fn clear_excluders(&mut self) {
        self.excluders.clear();
    }

    /// Replace the occlusion tester used during traversal.
    pub fn set_occlusion_tester(&mut self, tester: impl OcclusionTester + 'static) {
        self.occlusion_tester = Box::new(tester);
    }

    /// Set a callback invoked whenever a node load or hierarchy resolve fails.
    pub fn set_on_load_error(
        &mut self,
        callback: impl Fn(&LoadFailureDetails) + Send + Sync + 'static,
    ) {
        self.on_load_error = Some(Box::new(callback));
    }

    fn collect_completions(&mut self) -> Vec<crate::step::CompletedLoad<C>> {
        use crate::step::CompletedLoad;
        let ok = |node_id, content, byte_size| CompletedLoad {
            node_id,
            result: Ok(LoadResult::Content { content, byte_size }),
        };
        let err = |node_id, msg: String| CompletedLoad { node_id, result: Err(msg) };

        let mut completions: Vec<CompletedLoad<C>> = Vec::new();

        // Poll in-flight content loads.
        let ready_ids: Vec<NodeId> = self.in_flight
            .iter()
            .filter_map(|(&id, (_, f))| f.is_ready().then_some(id))
            .collect();
        for node_id in ready_ids {
            let Some((_token, future)) = self.in_flight.remove(&node_id) else { continue };
            match future.block() {
                Ok(Ok(loaded)) => match loaded.result {
                    LoadResult::Content { content, byte_size } => {
                        completions.push(ok(node_id, content, byte_size));
                    }
                    LoadResult::Reference { mut reference, byte_size } => {
                        reference.transform = Some(self.hierarchy.world_transform(node_id));
                        let resolve_future = self.resolver.resolve_erased(&self.bg_context, reference);
                        self.resolve_futures.insert(node_id, PendingResolve { future: resolve_future, byte_size });
                    }
                },
                Ok(Err(e)) => completions.push(err(node_id, e.to_string())),
                Err(e) => completions.push(err(node_id, format!("{e:?}"))),
            }
        }

        // Poll pending resolve futures.
        let ready_resolves: Vec<NodeId> = self.resolve_futures
            .iter()
            .filter_map(|(&id, p)| p.future.is_ready().then_some(id))
            .collect();
        for node_id in ready_resolves {
            let Some(PendingResolve { future, byte_size }) = self.resolve_futures.remove(&node_id) else { continue };
            match future.block() {
                Ok(Ok(Some(patch))) => match self.hierarchy.expand(patch) {
                    Ok(()) => completions.push(ok(node_id, None, byte_size)),
                    Err(e) => completions.push(err(node_id, e.to_string())),
                },
                Ok(Ok(None)) => completions.push(ok(node_id, None, byte_size)),
                Ok(Err(e)) => completions.push(err(node_id, e.to_string())),
                Err(e) => completions.push(err(node_id, format!("{e:?}"))),
            }
        }

        completions
    }

    /// React to the output of `step()`: start new loads, cancel old ones, free evicted content.
    fn apply_step_output(&mut self, output: crate::step::StepOutput<C>) {
        // Start new loads.
        for req in output.requests_to_start {
            let (token, future) = self.loader.load_erased(
                &self.bg_context,
                &self.main_context,
                req.node_id,
                &req.key,
                req.priority,
            );
            self.in_flight.insert(req.node_id, (token, future));
        }
        // Cancel evicted in-flight loads and resolve futures.
        for node_id in output.requests_to_cancel {
            if let Some((token, _)) = self.in_flight.remove(&node_id) {
                token.cancel();
            }
            self.resolve_futures.remove(&node_id);
        }
        // Free evicted content.
        for content in output.evicted_content {
            self.loader.free_erased(content);
        }
    }
}
