use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use orkester::{AsyncSystem, SharedFuture};

use crate::SelectionEngineExternals;

use crate::hierarchy::{HierarchyPatch, HierarchyResolver, SpatialHierarchy};
use crate::load::{
    ContentHandle, ContentLoader, LoadCandidate, LoadPassResult, Payload, RequestId,
};
use crate::lod::LodEvaluator;
use crate::node::{NodeId, NodeLifecycleState, NodeState};

use crate::options::SelectionOptions;
use crate::policy::{NoOcclusion, OcclusionTester, Policy, TileExcluder};
use crate::scheduler::{LoadScheduler, WeightedFairScheduler};
use crate::traversal::{TraversalContext, traverse};
use crate::view::{ViewGroupHandle, ViewGroupOptions, ViewState, ViewUpdateResult};

struct ResidentContent<C> {
    #[allow(dead_code)]
    content_handle: ContentHandle,
    content: Option<C>,
    byte_size: usize,
}

struct ViewGroupSlot {
    generation: u32,
    weight: f64,
    active: bool,
}

struct ViewGroupTable {
    slots: Vec<ViewGroupSlot>,
    next_generation: u32,
}

impl ViewGroupTable {
    fn new() -> Self {
        Self {
            slots: Vec::new(),
            next_generation: 1,
        }
    }

    fn insert(&mut self, weight: f64) -> ViewGroupHandle {
        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1);

        if let Some((index, slot)) = self
            .slots
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| !slot.active)
        {
            slot.generation = generation;
            slot.weight = weight;
            slot.active = true;
            ViewGroupHandle {
                index: index as u32,
                generation,
            }
        } else {
            let index = self.slots.len() as u32;
            self.slots.push(ViewGroupSlot {
                generation,
                weight,
                active: true,
            });
            ViewGroupHandle { index, generation }
        }
    }

    fn remove(&mut self, handle: ViewGroupHandle) -> bool {
        if let Some(slot) = self.slots.get_mut(handle.index as usize) {
            if slot.active && slot.generation == handle.generation {
                slot.active = false;
                return true;
            }
        }
        false
    }

    fn get(&self, handle: ViewGroupHandle) -> Option<&ViewGroupSlot> {
        self.slots
            .get(handle.index as usize)
            .filter(|slot| slot.active && slot.generation == handle.generation)
    }
}

/// Format-agnostic 3D tile selection engine.
///
/// Owns a spatial hierarchy, LOD evaluator, content loader, scheduler, and
/// policy, and drives the traversal → load pipeline.
///
/// # Frame loop
///
/// The recommended frame loop splits traversal from loading:
///
/// ```ignore
/// // 1. Traversal — decide which nodes to select and request.
/// let result = engine.update_view_group(handle, &views);
///
/// // 2. Loading — dispatch queued requests across all view groups.
/// engine.load_tiles();
///
/// // 3. (Optional) Access loaded content for processing.
/// for &node_id in &result.selected {
///     if let Some(content) = engine.content(node_id) {
///         // process content
///     }
/// }
/// ```
pub struct SelectionEngine<C, H, B, X, L, S, M>
where
    C: Send + 'static,
    H: SpatialHierarchy,
    B: LodEvaluator,
    X: HierarchyResolver,
    L: ContentLoader<C>,
    S: LoadScheduler,
    M: Policy,
{
    async_system: AsyncSystem,
    hierarchy: H,
    lod_evaluator: B,
    hierarchy_resolver: X,
    content_loader: L,
    scheduler: Arc<Mutex<S>>,
    policy: M,
    options: SelectionOptions,
    view_groups: ViewGroupTable,
    node_states: HashMap<NodeId, NodeState>,
    in_flight_futures:
        HashMap<RequestId, orkester::Future<Result<crate::load::LoadedContent<C>, L::Error>>>,
    in_flight: HashMap<RequestId, NodeId>,
    resolve_futures: HashMap<
        NodeId,
        (
            orkester::Future<Result<Option<HierarchyPatch>, X::Error>>,
            usize,
            ContentHandle,
        ),
    >,
    resident: HashMap<NodeId, ResidentContent<C>>,
    total_resident_bytes: usize,
    next_content_handle: u64,
    frame_index: u64,
    excluders: Vec<Box<dyn TileExcluder>>,
    occlusion_tester: Box<dyn OcclusionTester>,
    /// Resolves when the hierarchy root is available (async factory path).
    /// `None` for synchronous construction (root is immediately available).
    root_available: Option<SharedFuture<()>>,
    _phantom: PhantomData<C>,
}

impl<C, H, B, X, L, S, M> SelectionEngine<C, H, B, X, L, S, M>
where
    C: Send + 'static,
    H: SpatialHierarchy,
    B: LodEvaluator,
    X: HierarchyResolver,
    L: ContentLoader<C>,
    S: LoadScheduler,
    M: Policy,
{
    /// Create a new engine from shared externals.
    ///
    /// The async system and scheduler are taken from the shared externals,
    /// ensuring fair load distribution when multiple engines share the same
    /// externals.
    pub fn with_externals(
        externals: &SelectionEngineExternals,
        hierarchy: H,
        lod_evaluator: B,
        hierarchy_resolver: X,
        content_loader: L,
        policy: M,
        options: SelectionOptions,
    ) -> SelectionEngine<C, H, B, X, L, WeightedFairScheduler, M> {
        SelectionEngine::new(
            externals.async_system.clone(),
            hierarchy,
            lod_evaluator,
            hierarchy_resolver,
            content_loader,
            Arc::clone(&externals.scheduler),
            policy,
            options,
        )
    }

    /// Low-level constructor with explicit async system and scheduler.
    ///
    /// Prefer [`with_externals`](Self::with_externals) for typical usage.
    pub fn new(
        async_system: AsyncSystem,
        hierarchy: H,
        lod_evaluator: B,
        hierarchy_resolver: X,
        content_loader: L,
        scheduler: Arc<Mutex<S>>,
        policy: M,
        options: SelectionOptions,
    ) -> Self {
        Self {
            async_system,
            hierarchy,
            lod_evaluator,
            hierarchy_resolver,
            content_loader,
            scheduler,
            policy,
            options,
            view_groups: ViewGroupTable::new(),
            node_states: HashMap::new(),
            in_flight_futures: HashMap::new(),
            in_flight: HashMap::new(),
            resolve_futures: HashMap::new(),
            resident: HashMap::new(),
            total_resident_bytes: 0,
            next_content_handle: 1,
            frame_index: 0,
            excluders: Vec::new(),
            occlusion_tester: Box::new(NoOcclusion),
            root_available: None,
            _phantom: PhantomData,
        }
    }

    pub fn add_view_group(&mut self, opts: ViewGroupOptions) -> ViewGroupHandle {
        self.view_groups.insert(opts.weight)
    }

    pub fn remove_view_group(&mut self, handle: ViewGroupHandle) -> bool {
        self.view_groups.remove(handle)
    }

    pub fn is_view_group_active(&self, handle: ViewGroupHandle) -> bool {
        self.view_groups.get(handle).is_some()
    }

    /// Update a single view group, returning traversal results without loading.
    ///
    /// Call once per view group per frame, then call [`load`](Self::load) to
    /// dispatch the queued requests.
    pub fn update_view_group(
        &mut self,
        handle: ViewGroupHandle,
        views: &[ViewState],
    ) -> ViewUpdateResult {
        self.frame_index += 1;
        self.begin_frame();
        self.process_ready_pipeline();

        let Some(slot) = self.view_groups.get(handle) else {
            return ViewUpdateResult::default();
        };

        let view_group_key = Self::view_group_key(handle);
        let view_group_weight =
            (slot.weight * f64::from(u16::MAX)).clamp(1.0, f64::from(u16::MAX)) as u16;

        let mut ctx = TraversalContext {
            hierarchy: &self.hierarchy,
            lod_evaluator: &self.lod_evaluator,
            policy: &self.policy,
            excluders: &self.excluders,
            occlusion_tester: &*self.occlusion_tester,
            node_states: &mut self.node_states,
            views,
            options: &self.options,
            frame_index: self.frame_index,
            view_group_key,
            view_group_weight,
        };

        let traversal = traverse(&mut ctx);
        let queued_requests = traversal.candidates.len();

        for candidate in traversal.candidates {
            self.enqueue_candidate(candidate);
        }

        ViewUpdateResult {
            selected: traversal.selected,
            visited: traversal.visited,
            culled: traversal.culled,
            queued_requests,
            worker_thread_load_queue_length: self.pending_worker_queue_len(),
            main_thread_load_queue_length: 0,
            frame_number: self.frame_index,
            per_view: traversal.per_view,
        }
    }

    /// Update a view group and block until all tiles meeting the LOD threshold
    /// are loaded and ready to render.
    ///
    /// Significantly slower than [`update_view_group`](Self::update_view_group)
    /// — only use for movie capture or other non-realtime situations.
    pub fn update_view_group_offline(
        &mut self,
        handle: ViewGroupHandle,
        views: &[ViewState],
    ) -> ViewUpdateResult {
        loop {
            let result = self.update_view_group(handle, views);
            let load = self.load();
            self.dispatch_main_thread_tasks();

            if load.pending_worker_queue == 0 && load.pending_main_queue == 0 {
                return result;
            }

            // Yield to let async work complete before retrying.
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// Load tiles deemed most important across all view groups.
    ///
    /// Call frequently (e.g. once per render frame) after updating view groups.
    /// Returns quickly when there is nothing to do.
    pub fn load(&mut self) -> LoadPassResult {
        let result = self.run_load_pass();
        let _ = self.evict_if_needed();
        result
    }

    /// Process pipeline completions without issuing new content requests.
    pub fn dispatch_main_thread_tasks(&mut self) -> LoadPassResult {
        self.process_ready_pipeline();
        let _ = self.evict_if_needed();

        LoadPassResult {
            started_requests: 0,
            completed_main_thread_tasks: 0,
            pending_worker_queue: self.pending_worker_queue_len(),
            pending_main_queue: 0,
        }
    }

    pub fn frame_index(&self) -> u64 {
        self.frame_index
    }

    pub fn async_system(&self) -> &AsyncSystem {
        &self.async_system
    }

    /// Total bytes of currently-resident content.
    pub fn total_data_bytes(&self) -> usize {
        self.total_resident_bytes
    }

    /// Number of nodes with content currently loaded in memory.
    pub fn number_of_tiles_loaded(&self) -> usize {
        self.resident.len()
    }

    /// Returns the load progress as a percentage in the range `[0.0, 100.0]`.
    ///
    /// Progress is computed as the fraction of tracked nodes that have
    /// reached the `Renderable` state. Returns `100.0` when there are
    /// no tracked nodes (nothing to load).
    ///
    pub fn compute_load_progress(&self) -> f32 {
        if self.node_states.is_empty() {
            return 100.0;
        }
        let renderable = self
            .node_states
            .values()
            .filter(|s| s.lifecycle == NodeLifecycleState::Renderable)
            .count();
        (renderable as f32 / self.node_states.len() as f32) * 100.0
    }

    /// Read-only access to the options.
    pub fn options(&self) -> &SelectionOptions {
        &self.options
    }

    /// Mutable access to the options.
    pub fn options_mut(&mut self) -> &mut SelectionOptions {
        &mut self.options
    }

    /// Read-only access to the spatial hierarchy.
    pub fn hierarchy(&self) -> &H {
        &self.hierarchy
    }

    /// Read-only access to loaded content for a resident node.
    ///
    /// Returns `None` if the node is not resident or has no renderable content
    /// (e.g., `Payload::Empty` or `Payload::Reference`).
    pub fn content(&self, node_id: NodeId) -> Option<&C> {
        self.resident.get(&node_id).and_then(|r| r.content.as_ref())
    }

    /// Mutable access to loaded content for a resident node.
    pub fn content_mut(&mut self, node_id: NodeId) -> Option<&mut C> {
        self.resident
            .get_mut(&node_id)
            .and_then(|r| r.content.as_mut())
    }

    /// Add a tile excluder that will skip entire subtrees during traversal.
    pub fn add_excluder(&mut self, excluder: Box<dyn TileExcluder>) {
        self.excluders.push(excluder);
    }

    /// Read-only access to the current set of tile excluders.
    pub fn excluders(&self) -> &[Box<dyn TileExcluder>] {
        &self.excluders
    }

    /// Mutable access to the current set of tile excluders.
    pub fn excluders_mut(&mut self) -> &mut Vec<Box<dyn TileExcluder>> {
        &mut self.excluders
    }

    /// Replace the occlusion tester used during traversal.
    pub fn set_occlusion_tester(&mut self, tester: Box<dyn OcclusionTester>) {
        self.occlusion_tester = tester;
    }

    /// Returns a future that resolves when the hierarchy root is available.
    ///
    /// For engines created via [`from_factory`](Self::from_factory), this
    /// resolves once the factory's async initialization completes.
    /// For engines created synchronously, this returns `None` (root is
    /// immediately available).
    ///
    pub fn root_available(&self) -> Option<&SharedFuture<()>> {
        self.root_available.as_ref()
    }

    /// Returns `true` if the hierarchy root is ready for traversal.
    pub fn is_root_available(&self) -> bool {
        self.root_available.as_ref().map_or(true, |f| f.is_ready())
    }

    fn view_group_key(handle: ViewGroupHandle) -> u64 {
        (u64::from(handle.generation) << 32) | u64::from(handle.index)
    }

    fn begin_frame(&mut self) {
        for state in self.node_states.values_mut() {
            if state.lifecycle == NodeLifecycleState::Queued && state.request_id.is_none() {
                state.lifecycle = NodeLifecycleState::Unloaded;
            }
        }

        let mut scheduler = self.scheduler.lock().unwrap();
        scheduler.clear();
        scheduler.tick(self.frame_index);
    }

    fn enqueue_candidate(&mut self, candidate: LoadCandidate) {
        let state = self
            .node_states
            .entry(candidate.node_id)
            .or_insert_with(NodeState::new);

        if matches!(
            state.lifecycle,
            NodeLifecycleState::Loading
                | NodeLifecycleState::Renderable
                | NodeLifecycleState::Failed
        ) {
            return;
        }

        state.lifecycle = NodeLifecycleState::Queued;
        let mut scheduler = self.scheduler.lock().unwrap();
        scheduler.push(candidate);
    }

    fn process_ready_pipeline(&mut self) {
        self.process_load_completions();
        self.process_resolve_completions();
    }

    fn run_load_pass(&mut self) -> LoadPassResult {
        self.process_ready_pipeline();
        let started_requests = self.drain_scheduler(self.options.max_simultaneous_tile_loads);
        self.process_ready_pipeline();

        LoadPassResult {
            started_requests,
            completed_main_thread_tasks: 0,
            pending_worker_queue: self.pending_worker_queue_len(),
            pending_main_queue: 0,
        }
    }

    fn process_load_completions(&mut self) {
        let ready: Vec<RequestId> = self
            .in_flight_futures
            .iter()
            .filter(|(_, future)| future.is_ready())
            .map(|(&request_id, _)| request_id)
            .collect();

        for request_id in ready {
            let Some(future) = self.in_flight_futures.remove(&request_id) else {
                continue;
            };
            let Some(node_id) = self.in_flight.remove(&request_id) else {
                continue;
            };

            match future.block() {
                Ok(Ok(loaded)) => {
                    let content_handle = self.next_content_handle;
                    self.next_content_handle += 1;

                    if let Some(state) = self.node_states.get_mut(&node_id) {
                        state.request_id = None;
                    }

                    match loaded.payload {
                        Payload::Renderable(content) => {
                            self.insert_resident(
                                node_id,
                                content_handle,
                                Some(content),
                                loaded.byte_size,
                            );
                            self.mark_node_renderable(node_id);
                        }
                        Payload::Reference(reference) => {
                            let resolve_future = self
                                .hierarchy_resolver
                                .resolve_reference(&self.async_system, reference);
                            self.resolve_futures.insert(
                                node_id,
                                (resolve_future, loaded.byte_size, content_handle),
                            );
                        }
                        Payload::Empty => {
                            self.insert_resident(node_id, content_handle, None, loaded.byte_size);
                            self.mark_node_renderable(node_id);
                        }
                    }
                }
                Ok(Err(_)) | Err(_) => {
                    self.handle_load_failure(node_id);
                }
            }
        }
    }

    fn process_resolve_completions(&mut self) {
        let ready: Vec<NodeId> = self
            .resolve_futures
            .iter()
            .filter(|(_, (future, _, _))| future.is_ready())
            .map(|(&node_id, _)| node_id)
            .collect();

        for node_id in ready {
            let Some((future, byte_size, content_handle)) = self.resolve_futures.remove(&node_id)
            else {
                continue;
            };

            match future.block() {
                Ok(Ok(Some(patch))) => {
                    if self.hierarchy.apply_patch(patch).is_ok() {
                        self.insert_resident(node_id, content_handle, None, byte_size);
                        self.mark_node_renderable(node_id);
                    } else if let Some(state) = self.node_states.get_mut(&node_id) {
                        state.request_id = None;
                        state.lifecycle = NodeLifecycleState::Failed;
                    }
                }
                Ok(Ok(None)) => {
                    self.insert_resident(node_id, content_handle, None, byte_size);
                    self.mark_node_renderable(node_id);
                }
                Ok(Err(_)) | Err(_) => {
                    self.handle_load_failure(node_id);
                }
            }
        }
    }

    fn insert_resident(
        &mut self,
        node_id: NodeId,
        content_handle: ContentHandle,
        content: Option<C>,
        byte_size: usize,
    ) {
        if let Some(previous) = self.resident.insert(
            node_id,
            ResidentContent {
                content_handle,
                content,
                byte_size,
            },
        ) {
            self.total_resident_bytes =
                self.total_resident_bytes.saturating_sub(previous.byte_size);
        }

        self.total_resident_bytes = self.total_resident_bytes.saturating_add(byte_size);
    }

    fn mark_node_renderable(&mut self, node_id: NodeId) {
        let state = self
            .node_states
            .entry(node_id)
            .or_insert_with(NodeState::new);
        state.lifecycle = NodeLifecycleState::Renderable;
        state.request_id = None;
        state.retry_count = 0;
        state.next_retry_frame = 0;
    }

    fn handle_load_failure(&mut self, node_id: NodeId) {
        let state = self
            .node_states
            .entry(node_id)
            .or_insert_with(NodeState::new);
        state.request_id = None;
        state.retry_count = state.retry_count.saturating_add(1);

        if state.retry_count >= self.options.retry_limit {
            state.lifecycle = NodeLifecycleState::Failed;
        } else {
            state.lifecycle = NodeLifecycleState::RetryScheduled;
            state.next_retry_frame = self
                .frame_index
                .saturating_add(u64::from(self.options.retry_backoff_frames));
        }
    }

    fn drain_scheduler(&mut self, max_requests: usize) -> usize {
        let mut started = 0;
        let mut scheduler = self.scheduler.lock().unwrap();

        while started < max_requests {
            let Some(candidate) = scheduler.pop() else {
                break;
            };

            let lifecycle = self
                .node_states
                .get(&candidate.node_id)
                .map_or(NodeLifecycleState::Unloaded, |state| state.lifecycle);

            if matches!(
                lifecycle,
                NodeLifecycleState::Loading
                    | NodeLifecycleState::Renderable
                    | NodeLifecycleState::Failed
            ) {
                continue;
            }

            let (request_id, future) = self.content_loader.request(
                &self.async_system,
                candidate.node_id,
                &candidate.key,
                candidate.priority,
            );

            self.in_flight.insert(request_id, candidate.node_id);
            self.in_flight_futures.insert(request_id, future);

            let state = self
                .node_states
                .entry(candidate.node_id)
                .or_insert_with(NodeState::new);
            state.lifecycle = NodeLifecycleState::Loading;
            state.request_id = Some(request_id);
            started += 1;
        }

        started
    }

    fn pending_worker_queue_len(&self) -> usize {
        let scheduler = self.scheduler.lock().unwrap();
        scheduler.len() + self.in_flight_futures.len() + self.resolve_futures.len()
    }

    fn evict_if_needed(&mut self) -> Vec<NodeId> {
        let budget_bytes = self.options.max_cached_bytes;
        if self.total_resident_bytes <= budget_bytes {
            return Vec::new();
        }

        let resident_nodes: Vec<(NodeId, usize)> = self
            .resident
            .iter()
            .map(|(&node_id, resident)| (node_id, resident.byte_size))
            .collect();
        let mut to_evict = Vec::new();
        self.policy
            .select_evictions(&resident_nodes, budget_bytes, &mut to_evict);

        for &node_id in &to_evict {
            if let Some(resident) = self.resident.remove(&node_id) {
                self.total_resident_bytes =
                    self.total_resident_bytes.saturating_sub(resident.byte_size);
            }

            if let Some(state) = self.node_states.get_mut(&node_id) {
                state.lifecycle = NodeLifecycleState::Evicted;
                state.request_id = None;
            }
        }

        to_evict
    }
}
