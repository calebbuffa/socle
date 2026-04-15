//! Iterative depth-first selection algorithm.
//!
//! The public entry point is [`select()`] — a stateless function that reads
//! [`NodeStore`] + [`SelectionState`], writes to mutable `NodeStore` (via the
//! `expand` callback for latent children) and `SelectionState`, and returns a
//! [`FrameDecision`].
//!
//! # Two-phase DFS with TraversalDetails
//!
//! A `work_stack` holds `WorkItem` entries: `Visit` items descend into the tree,
//! and `Finalize` items combine child results bottom-up. A parallel `detail_stack`
//! carries [`TraversalDetails`] from leaves back to parents, enabling proper
//! kicking: children are removed from the render list when none were rendered
//! last frame and not all are renderable yet (cesium-native parity).
//!
//! # Zero-allocation after warmup
//!
//! All working buffers are retained in [`SelectionBuffers`] across frames.

use glam::DVec3;
use zukei::SpatialBounds;

use crate::frame_decision::{ExpandResult, FrameDecision, LoadRequest};
use crate::load::{LoadPriority, PriorityGroup};
use crate::lod::{LodDescriptor, LodEvaluator, LodFamily, RefinementMode};
use crate::node::{NodeId, NodeKind, NodeLoadState, NodeRefinementResult};
use crate::node_store::NodeStore;
use crate::options::SelectionOptions;
use crate::policy::{NodeExcluder, OcclusionState, OcclusionTester, VisibilityPolicy};
use crate::selection_state::SelectionState;
use crate::view::ViewState;

// ── StampSet ─────────────────────────────────────────────────────────────────

/// O(1) membership set backed by generation stamps.
struct StampSet {
    stamps: Vec<u64>,
    generation: u64,
}

impl StampSet {
    fn new() -> Self {
        Self {
            stamps: Vec::new(),
            generation: 1,
        }
    }

    #[inline(always)]
    fn clear(&mut self) {
        self.generation += 1;
    }

    #[inline(always)]
    fn insert(&mut self, id: NodeId) -> bool {
        let idx = id.index();
        if idx >= self.stamps.len() {
            self.stamps.resize(idx + 1, 0);
        }
        if self.stamps[idx] == self.generation {
            false
        } else {
            self.stamps[idx] = self.generation;
            true
        }
    }
}

// ── TraversalDetails ─────────────────────────────────────────────────────────

/// Bottom-up metadata for kicking decisions.
#[derive(Clone, Copy, Debug)]
struct TraversalDetails {
    all_renderable: bool,
    any_rendered_last_frame: bool,
    not_yet_renderable_count: usize,
}

impl TraversalDetails {
    fn leaf_renderable(rendered_last_frame: bool) -> Self {
        Self {
            all_renderable: true,
            any_rendered_last_frame: rendered_last_frame,
            not_yet_renderable_count: 0,
        }
    }

    fn leaf_not_renderable() -> Self {
        Self {
            all_renderable: false,
            any_rendered_last_frame: false,
            not_yet_renderable_count: 1,
        }
    }

    fn empty() -> Self {
        Self {
            all_renderable: true,
            any_rendered_last_frame: false,
            not_yet_renderable_count: 0,
        }
    }

    fn combine(&mut self, other: &TraversalDetails) {
        self.all_renderable &= other.all_renderable;
        self.any_rendered_last_frame |= other.any_rendered_last_frame;
        self.not_yet_renderable_count += other.not_yet_renderable_count;
    }
}

// ── WorkItem ─────────────────────────────────────────────────────────────────

enum WorkItem {
    Visit {
        node: NodeId,
        ancestor_rendered: bool,
        skip_depth: u32,
    },
    Finalize {
        node: NodeId,
        detail_start: usize,
        selected_start: usize,
        candidates_start: usize,
        fading_in_start: usize,
        per_view_starts: [usize; 64],
        vis: u64,
        score: i64,
        self_selected: bool,
        self_renderable: bool,
        has_content: bool,
        unconditionally_refined: bool,
        ancestor_rendered: bool,
        lifecycle: NodeLoadState,
        mode: RefinementMode,
        kind: NodeKind,
    },
}

// ── SelectionBuffers ─────────────────────────────────────────────────────────

/// Frame-persistent working buffers. Cleared each frame; capacity retained.
pub struct SelectionBuffers {
    work_stack: Vec<WorkItem>,
    detail_stack: Vec<TraversalDetails>,
    selected: Vec<NodeId>,
    selected_set: StampSet,
    candidates: Vec<LoadRequest>,
    per_view_selected: Vec<Vec<NodeId>>,
    preload_visited: StampSet,
    fading_out: Vec<NodeId>,
    fading_in: Vec<NodeId>,
}

impl SelectionBuffers {
    pub fn new() -> Self {
        Self {
            work_stack: Vec::new(),
            detail_stack: Vec::new(),
            selected: Vec::new(),
            selected_set: StampSet::new(),
            candidates: Vec::new(),
            per_view_selected: Vec::new(),
            preload_visited: StampSet::new(),
            fading_out: Vec::new(),
            fading_in: Vec::new(),
        }
    }

    fn clear(&mut self, view_count: usize) {
        self.work_stack.clear();
        self.detail_stack.clear();
        self.selected.clear();
        self.selected_set.clear();
        self.candidates.clear();
        self.per_view_selected.resize_with(view_count, Vec::new);
        for v in &mut self.per_view_selected {
            v.clear();
        }
        self.preload_visited.clear();
        self.fading_out.clear();
        self.fading_in.clear();
    }
}

impl Default for SelectionBuffers {
    fn default() -> Self {
        Self::new()
    }
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Run the selection algorithm.
///
/// This is the core of selekt — a pure function (aside from the `expand`
/// callback which may grow the `NodeStore`) that decides which nodes to
/// render and which to load.
///
/// # Parameters
///
/// - `store`: The node hierarchy. May be grown via `expand`.
/// - `state`: Per-node lifecycle and selection state. Updated in place.
/// - `options`: Selection tuning parameters.
/// - `views`: Camera states for this frame.
/// - `lod_evaluator`: LOD refinement decision maker.
/// - `policy`: Visibility (frustum culling) policy.
/// - `excluders`: Additional node exclusion filters.
/// - `occlusion_tester`: Occlusion culling.
/// - `buffers`: Reusable working memory.
/// - `expand`: Callback to lazily create children for latent nodes.
///   Called with `(node_id, &node_data)` → `ExpandResult`.
///   On `ExpandResult::Children(descs)`, they are inserted into `store`.
#[allow(clippy::too_many_arguments)]
pub fn select(
    store: &mut NodeStore,
    state: &mut SelectionState,
    options: &SelectionOptions,
    views: &[ViewState],
    lod_evaluator: &dyn LodEvaluator,
    policy: &dyn VisibilityPolicy,
    excluders: &[Box<dyn NodeExcluder>],
    occlusion_tester: &dyn OcclusionTester,
    buffers: &mut SelectionBuffers,
    expand: &mut dyn FnMut(NodeId, &crate::node_store::NodeData) -> ExpandResult,
) -> FrameDecision {
    let view_count = views.len();
    assert!(
        view_count <= 64,
        "selekt supports at most 64 simultaneous views"
    );
    assert!(!views.is_empty(), "select called with no views");

    buffers.clear(view_count);

    let frame_index = state.frame_index;

    // Fog culling: frame-level lod multiplier.
    let base_lod_multiplier = 1.0f64;
    let effective_lod_multiplier =
        if options.culling.enable_fog_culling && !options.culling.fog_density_table.is_empty() {
            let height = views[0].position.length();
            let density = sample_fog_density(&options.culling.fog_density_table, height);
            base_lod_multiplier / (1.0 + density * 4.0)
        } else {
            base_lod_multiplier
        };

    let progressive_multiplier = if options.lod.enable_progressive_resolution {
        options
            .lod
            .progressive_resolution_height_fraction
            .clamp(0.01, 0.99)
    } else {
        1.0
    };
    let mut any_node_loading = false;

    let mut visited = 0usize;
    let mut culled = 0usize;
    let mut occluded_count = 0usize;
    let mut kicked_count = 0usize;

    buffers.work_stack.push(WorkItem::Visit {
        node: store.root(),
        ancestor_rendered: false,
        skip_depth: 0,
    });

    while let Some(work_item) = buffers.work_stack.pop() {
        match work_item {
            WorkItem::Visit {
                node,
                ancestor_rendered,
                skip_depth,
            } => {
                visit_node(
                    node,
                    ancestor_rendered,
                    skip_depth,
                    store,
                    state,
                    lod_evaluator,
                    policy,
                    excluders,
                    occlusion_tester,
                    views,
                    options,
                    frame_index,
                    effective_lod_multiplier,
                    progressive_multiplier,
                    &mut any_node_loading,
                    buffers,
                    &mut visited,
                    &mut culled,
                    &mut occluded_count,
                    expand,
                );
            }
            WorkItem::Finalize {
                node,
                detail_start,
                selected_start,
                candidates_start,
                fading_in_start,
                per_view_starts,
                vis,
                score,
                self_selected,
                self_renderable,
                has_content,
                unconditionally_refined,
                ancestor_rendered,
                lifecycle,
                mode,
                kind,
            } => {
                let combined = finalize_node(
                    node,
                    detail_start,
                    selected_start,
                    candidates_start,
                    fading_in_start,
                    &per_view_starts,
                    vis,
                    score,
                    self_selected,
                    self_renderable,
                    has_content,
                    unconditionally_refined,
                    ancestor_rendered,
                    lifecycle,
                    mode,
                    kind,
                    frame_index,
                    options,
                    store,
                    state,
                    buffers,
                    &mut kicked_count,
                );
                buffers.detail_stack.push(combined);
            }
        }
    }

    // Preload pass.
    if options.loading.preload_ancestors || options.loading.preload_siblings {
        preload_pass(store, state, options, buffers);
    }

    // Flight destination preloading.
    if !options.streaming.flight_destinations.is_empty() {
        flight_preload_pass(store, state, lod_evaluator, policy, options, buffers);
    }

    // LOD transition tracking: record fade_in_frame when a node first enters
    // the fading_in list; clear it when the node fades out.
    if options.streaming.enable_lod_transition {
        for &node in &buffers.fading_in {
            let status = state.get_mut(node);
            if status.fade_in_frame == 0 {
                status.fade_in_frame = frame_index;
            }
        }
        for &node in &buffers.fading_out {
            state.get_mut(node).fade_in_frame = 0;
        }
    }

    FrameDecision {
        render: std::mem::take(&mut buffers.selected),
        per_view_render: std::mem::take(&mut buffers.per_view_selected),
        load: std::mem::take(&mut buffers.candidates),
        fading_in: std::mem::take(&mut buffers.fading_in),
        fading_out: std::mem::take(&mut buffers.fading_out),
        nodes_visited: visited,
        nodes_culled: culled,
        nodes_occluded: occluded_count,
        nodes_kicked: kicked_count,
    }
}

// ── visit_node ───────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn visit_node(
    node: NodeId,
    ancestor_rendered: bool,
    skip_depth: u32,
    store: &mut NodeStore,
    state: &mut SelectionState,
    lod_evaluator: &dyn LodEvaluator,
    policy: &dyn VisibilityPolicy,
    excluders: &[Box<dyn NodeExcluder>],
    occlusion_tester: &dyn OcclusionTester,
    views: &[ViewState],
    options: &SelectionOptions,
    frame_index: u64,
    effective_lod_multiplier: f64,
    progressive_multiplier: f64,
    any_node_loading: &mut bool,
    buffers: &mut SelectionBuffers,
    visited: &mut usize,
    culled: &mut usize,
    occluded_count: &mut usize,
    expand: &mut dyn FnMut(NodeId, &crate::node_store::NodeData) -> ExpandResult,
) {
    let view_count = views.len();

    // Expand latent children if needed.
    // Only expand once the node's own content is loaded (or the node has no
    // content). This prevents cascading expansion where unloaded deep nodes
    // generate children that are also unloaded, causing flickering.
    if store.might_have_latent_children(node) && store.children(node).is_empty() {
        let lifecycle = state.get(node).lifecycle;
        let has_content = !store.content_keys(node).is_empty();
        if !has_content || lifecycle == NodeLoadState::Renderable {
            let node_data = store.get(node);
            let segment = node_data.segment;
            let result = expand(node, node_data);
            match result {
                ExpandResult::Children(descs) => {
                    store.insert_children(node, &descs, segment);
                }
                ExpandResult::RetryLater | ExpandResult::None => {}
            }
        }
    }

    let bounds = store.bounds(node).clone();
    let kind = store.node_kind(node);
    let mode = store.refinement_mode(node);
    let children: Vec<NodeId> = store.children(node).to_vec();
    let has_content = !store.content_keys(node).is_empty();

    let status = state.get(node);
    let lifecycle = status.lifecycle;

    if lifecycle == NodeLoadState::Failed {
        buffers.detail_stack.push(TraversalDetails::empty());
        return;
    }
    if lifecycle == NodeLoadState::RetryScheduled && frame_index < status.next_retry_frame {
        buffers.detail_stack.push(TraversalDetails::empty());
        return;
    }

    if options.lod.enable_progressive_resolution && is_pending(lifecycle) {
        *any_node_loading = true;
    }

    if excluders.iter().any(|e| e.should_exclude(node, &bounds)) {
        *culled += 1;
        buffers.detail_stack.push(TraversalDetails::empty());
        return;
    }

    // Clipping planes.
    if !options.culling.clipping_planes.is_empty() {
        let fully_clipped = options.culling.clipping_planes.iter().any(|plane| {
            crate::evaluators::bounds_entirely_clipped(&bounds, plane.normal, plane.distance)
        });
        if fully_clipped {
            *culled += 1;
            buffers.detail_stack.push(TraversalDetails::empty());
            return;
        }
    }

    // Viewer request volume.
    if let Some(vrv) = store.viewer_request_volume(node) {
        let camera_inside = views
            .iter()
            .any(|v| crate::evaluators::point_inside_bounds(v.position, vrv));
        if !camera_inside {
            *culled += 1;
            buffers.detail_stack.push(TraversalDetails::empty());
            return;
        }
    }

    // Visibility bitmask.
    let vis: u64 = if options.culling.enable_frustum_culling {
        let mut bits = 0u64;
        for (i, view) in views.iter().enumerate() {
            if policy.is_visible(node, &bounds, view) {
                bits |= 1 << i;
            }
        }
        bits
    } else {
        (1u64 << view_count) - 1
    };

    // render_nodes_under_camera override.
    let vis = if options.culling.render_nodes_under_camera && vis == 0 {
        let mut override_bits = 0u64;
        if let Some(geo_rect) = store.globe_rectangle(node) {
            for (i, view) in views.iter().enumerate() {
                if let Some(carto) = view.position_cartographic() {
                    if geo_rect.contains_cartographic(carto) {
                        override_bits |= 1 << i;
                    }
                }
            }
        }
        override_bits
    } else {
        vis
    };

    // cesium-native: root tile (no parent) is never culled regardless of frustum.
    // Without this, a tight frustum can drop the entire tileset.
    let vis = if vis == 0 && store.parent(node).is_none() {
        (1u64 << view_count) - 1
    } else {
        vis
    };

    // cesium-native: when forbid-holes is active, culled unconditionally-refined
    // Replace tiles are force-visited so external tileset references off-screen
    // still load their children and don't leave permanent holes.
    let vis = if vis == 0
        && store.is_unconditionally_refined(node)
        && options.loading.prevent_holes
        && mode == RefinementMode::Replace
    {
        (1u64 << view_count) - 1
    } else {
        vis
    };

    if vis == 0 {
        *culled += 1;
        if state.get(node).last_result.was_rendered() {
            buffers.fading_out.push(node);
        }
        state.get_mut(node).last_result = NodeRefinementResult::Culled;

        // Culled SSE refinement.
        let should_refine_culled = options.culling.enforce_culled_screen_space_error
            && !children.is_empty()
            && kind != NodeKind::Empty
            && {
                let lod_desc = store.lod_descriptor(node);
                let mut tmp = LodDescriptor {
                    value: 0.0,
                    family: LodFamily::NONE,
                };
                let lod_desc =
                    resolve_lod_desc(lod_desc, store.lod_metric_override(node), &mut tmp);
                let culled_multiplier =
                    (options.culling.culled_screen_space_error as f32).max(f32::EPSILON);
                // Check ALL views — matches cesium-native which refines if any
                // view requires it (relevant for multi-camera / VR setups).
                views.iter().any(|view| {
                    lod_evaluator.should_refine(lod_desc, view, culled_multiplier, &bounds, mode)
                })
            };

        if should_refine_culled {
            push_children_rev(&children, ancestor_rendered, 0, &mut buffers.work_stack);
        } else if options.loading.prevent_holes && mode == RefinementMode::Replace {
            for &child in &children {
                let child_lc = state.get(child).lifecycle;
                if needs_load(child_lc) && !store.content_keys(child).is_empty() {
                    buffers.candidates.push(LoadRequest {
                        node_id: child,
                        key: store.content_keys(child)[0].clone(),
                        priority: LoadPriority {
                            group: PriorityGroup::Normal,
                            score: i64::MAX,
                            view_group_weight: 0,
                        },
                    });
                }
            }
            let is_loaded = lifecycle == NodeLoadState::Renderable;
            buffers.detail_stack.push(if is_loaded {
                TraversalDetails::leaf_renderable(state.get(node).last_result.was_rendered())
            } else {
                TraversalDetails::leaf_not_renderable()
            });
            return;
        }

        buffers.detail_stack.push(TraversalDetails::empty());
        return;
    }

    *visited += 1;

    // Importance score for cache eviction.
    {
        let lod_desc = store.lod_descriptor(node);
        let sse = store.lod_metric_override(node).unwrap_or(lod_desc.value);
        state.get_mut(node).importance = node_importance(&bounds, sse, views);
    }

    let occluded = options.culling.enable_occlusion_culling
        && occlusion_tester.occlusion_state(node) == OcclusionState::Occluded;

    let unconditionally_refined = store.is_unconditionally_refined(node);

    let should_refine = if unconditionally_refined && !children.is_empty() {
        true
    } else if (occluded && options.culling.delay_refinement_for_occlusion)
        || children.is_empty()
        || kind == NodeKind::Empty
    {
        false
    } else {
        let lod_desc = store.lod_descriptor(node);
        let mut lod_desc_tmp = LodDescriptor {
            value: 0.0,
            family: LodFamily::NONE,
        };
        let lod_desc =
            resolve_lod_desc(lod_desc, store.lod_metric_override(node), &mut lod_desc_tmp);
        views.iter().enumerate().any(|(i, view)| {
            if (vis & (1 << i)) == 0 {
                return false;
            }
            let m = view.lod_metric_multiplier
                * effective_view_multiplier(
                    view,
                    &bounds,
                    options,
                    effective_lod_multiplier,
                    *any_node_loading,
                    progressive_multiplier,
                    0.0, // camera_stationary_seconds — TODO: pass through
                );
            lod_evaluator.should_refine(lod_desc, view, m, &bounds, mode)
        })
    };

    let last_result = state.get(node).last_result;
    let was_refined =
        last_result.is_refined() || last_result == NodeRefinementResult::RenderedAndKicked;
    let self_renderable = lifecycle == NodeLoadState::Renderable && has_content;
    let must_continue_refining = was_refined
        && !self_renderable
        && mode == RefinementMode::Replace
        && !options.lod.immediately_load_desired_lod;

    let camera_velocity = DVec3::ZERO; // TODO: pass through

    match kind {
        NodeKind::Empty | NodeKind::CompositeRoot => {
            state.get_mut(node).last_result = if children.is_empty() {
                NodeRefinementResult::None
            } else {
                NodeRefinementResult::Refined
            };
            push_children_rev(&children, ancestor_rendered, 0, &mut buffers.work_stack);
        }
        NodeKind::Renderable => {
            let score = priority_score(&bounds, views, options, camera_velocity);

            if (should_refine || must_continue_refining) && !children.is_empty() {
                match mode {
                    RefinementMode::Add => {
                        let last_result = state.get(node).last_result;
                        let was_rendered = last_result.was_rendered();
                        let any_rendered_last = was_rendered || last_result.is_refined();
                        leaf_select_or_queue(
                            node,
                            lifecycle,
                            has_content,
                            vis,
                            ancestor_rendered,
                            score,
                            store,
                            state,
                            buffers,
                        );
                        let self_rendered = lifecycle == NodeLoadState::Renderable && has_content;
                        push_children_rev(
                            &children,
                            ancestor_rendered || self_rendered,
                            0,
                            &mut buffers.work_stack,
                        );
                        state.get_mut(node).last_result = NodeRefinementResult::Refined;
                        buffers.detail_stack.push(if self_rendered {
                            TraversalDetails::leaf_renderable(any_rendered_last)
                        } else {
                            TraversalDetails::leaf_not_renderable()
                        });
                    }
                    RefinementMode::Replace => {
                        let detail_start = buffers.detail_stack.len();
                        let selected_start = buffers.selected.len();
                        let candidates_start = buffers.candidates.len();
                        let fading_in_start = buffers.fading_in.len();
                        let mut per_view_starts = [0usize; 64];
                        for (i, pvs) in buffers.per_view_selected.iter().enumerate() {
                            per_view_starts[i] = pvs.len();
                        }
                        let self_renderable = lifecycle == NodeLoadState::Renderable && has_content;

                        buffers.work_stack.push(WorkItem::Finalize {
                            node,
                            detail_start,
                            selected_start,
                            candidates_start,
                            fading_in_start,
                            per_view_starts,
                            vis,
                            score,
                            self_selected: false,
                            self_renderable,
                            has_content,
                            unconditionally_refined,
                            ancestor_rendered,
                            lifecycle,
                            mode,
                            kind,
                        });
                        let self_rendered = lifecycle == NodeLoadState::Renderable && has_content;
                        push_children_rev(
                            &children,
                            ancestor_rendered || self_rendered,
                            0,
                            &mut buffers.work_stack,
                        );
                    }
                }
            } else {
                if occluded {
                    *occluded_count += 1;
                }

                // Skip-LOD logic.
                let lod_desc = store.lod_descriptor(node);
                let mut lod_skip_tmp = LodDescriptor {
                    value: 0.0,
                    family: LodFamily::NONE,
                };
                let lod_desc =
                    resolve_lod_desc(lod_desc, store.lod_metric_override(node), &mut lod_skip_tmp);

                let force_skip = options.lod.skip_level_of_detail
                    && !children.is_empty()
                    && skip_depth < options.lod.skip_levels
                    && lod_desc.value > options.lod.base_lod_metric_threshold;

                let skip_eligible = !force_skip
                    && options.lod.skip_level_of_detail
                    && !children.is_empty()
                    && lod_desc.value > options.lod.base_lod_metric_threshold
                    && views.iter().enumerate().any(|(i, view)| {
                        if (vis & (1 << i)) == 0 {
                            return false;
                        }
                        let m = view.lod_metric_multiplier
                            * effective_view_multiplier(
                                view,
                                &bounds,
                                options,
                                effective_lod_multiplier / options.lod.skip_lod_metric_factor,
                                *any_node_loading,
                                progressive_multiplier,
                                0.0,
                            );
                        lod_evaluator.should_refine(lod_desc, view, m, &bounds, mode)
                    });

                if force_skip || skip_eligible {
                    push_children_rev(
                        &children,
                        ancestor_rendered,
                        skip_depth + 1,
                        &mut buffers.work_stack,
                    );
                    state.get_mut(node).last_result = NodeRefinementResult::Refined;

                    if skip_eligible && options.streaming.load_siblings_on_skip {
                        if let Some(parent) = store.parent(node) {
                            let siblings: Vec<NodeId> = store.children(parent).to_vec();
                            for sib in siblings {
                                if sib == node {
                                    continue;
                                }
                                let sib_lc = state.get(sib).lifecycle;
                                if needs_load(sib_lc) && !store.content_keys(sib).is_empty() {
                                    let sib_score = priority_score(
                                        store.bounds(sib),
                                        views,
                                        options,
                                        camera_velocity,
                                    );
                                    buffers.candidates.push(LoadRequest {
                                        node_id: sib,
                                        key: store.content_keys(sib)[0].clone(),
                                        priority: LoadPriority {
                                            group: PriorityGroup::Preload,
                                            score: sib_score,
                                            view_group_weight: 0,
                                        },
                                    });
                                }
                            }
                        }
                    }
                } else {
                    // True leaf.
                    let last_result = state.get(node).last_result;
                    let was_rendered = last_result.was_rendered();
                    // cesium-native createTraversalDetailsForSingleTile:
                    // If the tile was *refined* last frame (children were rendering),
                    // we must treat it as if it were rendered last frame too. Without
                    // this, the parent's kick condition fires when this tile transitions
                    // from refined→rendered, causing a flash of missing geometry.
                    let any_rendered_last_frame = was_rendered || last_result.is_refined();
                    let is_renderable = lifecycle == NodeLoadState::Renderable && has_content;
                    let result = if is_renderable {
                        NodeRefinementResult::Rendered
                    } else {
                        NodeRefinementResult::None
                    };
                    leaf_select_or_queue(
                        node,
                        lifecycle,
                        has_content,
                        vis,
                        ancestor_rendered,
                        score,
                        store,
                        state,
                        buffers,
                    );
                    state.get_mut(node).last_result = result;
                    buffers.detail_stack.push(if is_renderable {
                        TraversalDetails::leaf_renderable(any_rendered_last_frame)
                    } else {
                        TraversalDetails::leaf_not_renderable()
                    });
                }
            }
        }
    }
}

// ── finalize_node ────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn finalize_node(
    node: NodeId,
    detail_start: usize,
    selected_start: usize,
    candidates_start: usize,
    fading_in_start: usize,
    per_view_starts: &[usize; 64],
    vis: u64,
    score: i64,
    _self_selected: bool,
    self_renderable: bool,
    has_content: bool,
    unconditionally_refined: bool,
    _ancestor_rendered: bool,
    lifecycle: NodeLoadState,
    _mode: RefinementMode,
    _kind: NodeKind,
    frame_index: u64,
    options: &SelectionOptions,
    store: &NodeStore,
    state: &mut SelectionState,
    buffers: &mut SelectionBuffers,
    kicked_count: &mut usize,
) -> TraversalDetails {
    let mut combined = TraversalDetails::empty();
    let detail_end = buffers.detail_stack.len();
    for i in detail_start..detail_end {
        combined.combine(&buffers.detail_stack[i]);
    }
    buffers.detail_stack.truncate(detail_start);

    let want_to_kick = !combined.all_renderable && !combined.any_rendered_last_frame;

    // cesium-native: also kick descendants while the parent tile is fading in,
    // so children don't pop in over a partially-transparent parent.
    let kick_due_to_fading_in = options.streaming.enable_lod_transition
        && options.streaming.kick_descendants_while_fading_in
        && state.get(node).last_result.was_rendered()
        && state.get(node).fade_in_frame != 0
        && state.get(node).fade_in_frame < frame_index; // fade started a prior frame

    // Only actually kick if the parent tile is renderable OR too many descendants
    // are still loading. Kicking without a renderable parent creates visible holes.
    // This matches cesium-native: `willKick = wantToKick && (notYetRenderableCount > limit || tile.isRenderable())`
    let will_kick = (want_to_kick || kick_due_to_fading_in)
        && (self_renderable
            || combined.not_yet_renderable_count > options.loading.loading_descendant_limit);

    if will_kick {
        *kicked_count += 1;

        for &removed in &buffers.selected[selected_start..] {
            if state.get(removed).last_result.was_rendered() {
                // Don't fade — will be re-added when children load.
            }
        }
        buffers.selected.truncate(selected_start);

        for (i, pvs) in buffers.per_view_selected.iter_mut().enumerate() {
            pvs.truncate(per_view_starts[i]);
        }

        buffers.fading_in.truncate(fading_in_start);

        // cesium-native: only cancel descendant load requests when the parent tile
        // was NOT rendered last frame AND we are over the loading limit. When the
        // parent is already visible (wasReallyRenderedLastFrame), children must
        // continue loading so refinement can eventually complete.
        let was_really_rendered_last_frame =
            state.get(node).last_result.was_rendered() && self_renderable;
        if !was_really_rendered_last_frame
            && !unconditionally_refined
            && combined.not_yet_renderable_count > options.loading.loading_descendant_limit
        {
            buffers.candidates.truncate(candidates_start);
        }

        if self_renderable {
            do_select(node, vis, state, buffers);
            state.get_mut(node).last_result = NodeRefinementResult::RenderedAndKicked;
            TraversalDetails::leaf_renderable(state.get(node).last_result.was_rendered())
        } else {
            if has_content && needs_load(lifecycle) {
                buffers.candidates.push(LoadRequest {
                    node_id: node,
                    key: store.content_keys(node)[0].clone(),
                    priority: LoadPriority {
                        group: PriorityGroup::Normal,
                        score,
                        view_group_weight: 0,
                    },
                });
            }
            state.get_mut(node).last_result = NodeRefinementResult::RenderedAndKicked;
            TraversalDetails::leaf_not_renderable()
        }
    } else if combined.all_renderable {
        // All children are renderable — this tile is now fully refined.
        // Add to fading_out if this tile itself was previously rendered.
        let last_result = state.get(node).last_result;
        if last_result.was_rendered() {
            buffers.fading_out.push(node);
        }
        state.get_mut(node).last_result = NodeRefinementResult::Refined;
        // Propagate anyWereRenderedLastFrame through the combined details so
        // ancestors don't incorrectly kick this subtree.
        // If the tile was previously Rendered or Refined (descendants rendered),
        // ancestors should know something was rendering here last frame.
        if !combined.any_rendered_last_frame
            && (last_result.was_rendered() || last_result.is_refined())
        {
            TraversalDetails {
                any_rendered_last_frame: true,
                ..combined
            }
        } else {
            combined
        }
    } else {
        if self_renderable {
            do_select(node, vis, state, buffers);
            state.get_mut(node).last_result = NodeRefinementResult::RenderedAndKicked;
        } else if has_content {
            match lifecycle {
                lc if needs_load(lc) || is_pending(lc) => {
                    buffers.candidates.push(LoadRequest {
                        node_id: node,
                        key: store.content_keys(node)[0].clone(),
                        priority: LoadPriority {
                            group: PriorityGroup::Urgent,
                            score,
                            view_group_weight: 0,
                        },
                    });
                }
                _ => {}
            }
            state.get_mut(node).last_result = NodeRefinementResult::Refined;
        } else {
            state.get_mut(node).last_result = NodeRefinementResult::Refined;
        }
        combined
    }
}

// ── Helper functions ─────────────────────────────────────────────────────────

#[inline(always)]
fn needs_load(lifecycle: NodeLoadState) -> bool {
    matches!(
        lifecycle,
        NodeLoadState::Unloaded | NodeLoadState::Evicted | NodeLoadState::RetryScheduled
    )
}

#[inline(always)]
fn is_pending(lifecycle: NodeLoadState) -> bool {
    matches!(lifecycle, NodeLoadState::Queued | NodeLoadState::Loading)
}

#[inline(always)]
fn push_children_rev(
    children: &[NodeId],
    ancestor_rendered: bool,
    skip_depth: u32,
    stack: &mut Vec<WorkItem>,
) {
    for &child in children.iter().rev() {
        stack.push(WorkItem::Visit {
            node: child,
            ancestor_rendered,
            skip_depth,
        });
    }
}

#[inline(always)]
fn leaf_select_or_queue(
    node: NodeId,
    lifecycle: NodeLoadState,
    has_content: bool,
    vis: u64,
    ancestor_rendered: bool,
    score: i64,
    store: &NodeStore,
    state: &SelectionState,
    buffers: &mut SelectionBuffers,
) {
    match lifecycle {
        NodeLoadState::Renderable => {
            do_select(node, vis, state, buffers);
        }
        lc if needs_load(lc) => {
            if has_content {
                let group = if ancestor_rendered {
                    PriorityGroup::Preload
                } else {
                    PriorityGroup::Normal
                };
                buffers.candidates.push(LoadRequest {
                    node_id: node,
                    key: store.content_keys(node)[0].clone(),
                    priority: LoadPriority {
                        group,
                        score,
                        view_group_weight: 0,
                    },
                });
            }
        }
        _ => {}
    }
}

fn bounds_center(bounds: &SpatialBounds) -> DVec3 {
    match bounds {
        SpatialBounds::Sphere { center, .. } => *center,
        SpatialBounds::OrientedBox { center, .. } => *center,
        SpatialBounds::AxisAlignedBox { min, max } => (*min + *max) * 0.5,
        SpatialBounds::Rectangle { min, max } => {
            DVec3::new((min.x + max.x) * 0.5, (min.y + max.y) * 0.5, 0.0)
        }
        SpatialBounds::Polygon { vertices } => {
            if vertices.is_empty() {
                DVec3::ZERO
            } else {
                let sum = vertices.iter().fold(glam::DVec2::ZERO, |a, &v| a + v);
                let c = sum / vertices.len() as f64;
                DVec3::new(c.x, c.y, 0.0)
            }
        }
        SpatialBounds::Empty => DVec3::ZERO,
    }
}

fn priority_score(
    bounds: &SpatialBounds,
    views: &[ViewState],
    options: &SelectionOptions,
    camera_velocity: DVec3,
) -> i64 {
    let center = bounds_center(bounds);
    let mut min_score = i64::MAX;
    for view in views {
        let to_node = center - view.position;
        let dist_sq = to_node.length_squared();
        if dist_sq < 1e-20 {
            return 0;
        }
        let distance = dist_sq.sqrt();
        let cos_theta = view.direction.dot(to_node / distance);
        let angle_factor = 1.0 - cos_theta;
        let mut score = (angle_factor * distance * 1_000_000.0) as i64;

        if options.streaming.enable_request_render_mode_priority {
            let speed = camera_velocity.length();
            if speed > 1e-3 {
                let vel_dir = camera_velocity / speed;
                let cos_vel = vel_dir.dot(to_node / distance);
                let vel_angle = cos_vel.clamp(-1.0, 1.0).acos();
                if vel_angle < options.streaming.request_render_mode_priority_angle {
                    let boost = (1.0
                        - vel_angle / options.streaming.request_render_mode_priority_angle)
                        * distance
                        * 500_000.0;
                    score -= boost as i64;
                }
            }
        }

        if score < min_score {
            min_score = score;
        }
    }
    min_score
}

#[inline(always)]
fn do_select(node: NodeId, vis: u64, state: &SelectionState, buffers: &mut SelectionBuffers) {
    if buffers.selected_set.insert(node) {
        buffers.selected.push(node);
        if !state.get(node).last_result.was_rendered() {
            buffers.fading_in.push(node);
        }
    }
    let mut bits = vis;
    while bits != 0 {
        let i = bits.trailing_zeros() as usize;
        buffers.per_view_selected[i].push(node);
        bits &= bits - 1;
    }
}

fn preload_pass(
    store: &NodeStore,
    state: &SelectionState,
    options: &SelectionOptions,
    buffers: &mut SelectionBuffers,
) {
    let selected_count = buffers.selected.len();

    for idx in 0..selected_count {
        let node = buffers.selected[idx];

        if options.loading.preload_ancestors {
            let mut current = node;
            while let Some(parent) = store.parent(current) {
                if !buffers.preload_visited.insert(parent) {
                    break;
                }
                let lifecycle = state.get(parent).lifecycle;
                if needs_load(lifecycle) && !store.content_keys(parent).is_empty() {
                    buffers.candidates.push(LoadRequest {
                        node_id: parent,
                        key: store.content_keys(parent)[0].clone(),
                        priority: LoadPriority {
                            group: PriorityGroup::Preload,
                            score: i64::MAX,
                            view_group_weight: 0,
                        },
                    });
                }
                current = parent;
            }
        }

        if options.loading.preload_siblings {
            if let Some(parent) = store.parent(node) {
                let siblings: Vec<NodeId> = store.children(parent).to_vec();
                for sibling in siblings {
                    if sibling == node || !buffers.preload_visited.insert(sibling) {
                        continue;
                    }
                    let lifecycle = state.get(sibling).lifecycle;
                    if needs_load(lifecycle) && !store.content_keys(sibling).is_empty() {
                        buffers.candidates.push(LoadRequest {
                            node_id: sibling,
                            key: store.content_keys(sibling)[0].clone(),
                            priority: LoadPriority {
                                group: PriorityGroup::Preload,
                                score: i64::MAX,
                                view_group_weight: 0,
                            },
                        });
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn flight_preload_pass(
    store: &NodeStore,
    state: &SelectionState,
    lod_evaluator: &dyn LodEvaluator,
    policy: &dyn VisibilityPolicy,
    options: &SelectionOptions,
    buffers: &mut SelectionBuffers,
) {
    const MAX_DEPTH: usize = 8;
    let camera_velocity = DVec3::ZERO;

    for dest_views in &options.streaming.flight_destinations {
        if dest_views.is_empty() {
            continue;
        }
        let mut stack: Vec<(NodeId, usize)> = vec![(store.root(), 0)];
        while let Some((node, depth)) = stack.pop() {
            let bounds = store.bounds(node);
            let kind = store.node_kind(node);
            let mode = store.refinement_mode(node);
            let children = store.children(node);

            let visible = dest_views
                .iter()
                .any(|v| policy.is_visible(node, bounds, v));
            if !visible {
                continue;
            }

            let lod_desc = store.lod_descriptor(node);
            let should_preload_refine = kind != NodeKind::Empty
                && !children.is_empty()
                && depth < MAX_DEPTH
                && dest_views.iter().any(|v| {
                    lod_evaluator.should_refine(lod_desc, v, v.lod_metric_multiplier, bounds, mode)
                });

            if should_preload_refine {
                for &child in children {
                    stack.push((child, depth + 1));
                }
            } else {
                let lc = state.get(node).lifecycle;
                if needs_load(lc) && !store.content_keys(node).is_empty() {
                    let score = priority_score(bounds, dest_views, options, camera_velocity);
                    buffers.candidates.push(LoadRequest {
                        node_id: node,
                        key: store.content_keys(node)[0].clone(),
                        priority: LoadPriority {
                            group: PriorityGroup::Preload,
                            score,
                            view_group_weight: 0,
                        },
                    });
                }
            }
        }
    }
}

fn sample_fog_density(table: &[(f64, f64)], height: f64) -> f64 {
    if table.is_empty() {
        return 0.0;
    }
    if height <= table[0].0 {
        return table[0].1;
    }
    if height >= table[table.len() - 1].0 {
        return table[table.len() - 1].1;
    }
    let idx = table.partition_point(|&(h, _)| h <= height);
    let (h0, d0) = table[idx - 1];
    let (h1, d1) = table[idx];
    let t = (height - h0) / (h1 - h0);
    d0 + t * (d1 - d0)
}

fn resolve_lod_desc<'a>(
    lod_desc: &'a LodDescriptor,
    override_val: Option<f64>,
    tmp: &'a mut LodDescriptor,
) -> &'a LodDescriptor {
    match override_val {
        Some(v) => {
            *tmp = LodDescriptor {
                value: v,
                ..*lod_desc
            };
            tmp
        }
        None => lod_desc,
    }
}

fn effective_view_multiplier(
    view: &ViewState,
    bounds: &SpatialBounds,
    options: &SelectionOptions,
    base_multiplier: f64,
    any_node_loading: bool,
    progressive_multiplier: f64,
    camera_stationary_seconds: f32,
) -> f32 {
    let mut m = base_multiplier;

    if options.lod.enable_progressive_resolution && any_node_loading {
        m *= progressive_multiplier;
    }

    let center = bounds_center(bounds);
    let to_node = center - view.position;
    let dist = to_node.length();

    if options.lod.enable_dynamic_detail_reduction && dist > 1e-10 {
        let cos_theta = view.direction.dot(to_node / dist).clamp(-1.0, 1.0) as f64;
        let angle_factor = (1.0 - cos_theta) * 0.5;
        let reduction = 1.0
            / (1.0
                + options.lod.dynamic_detail_reduction_density
                    * dist
                    * angle_factor
                    * options.lod.dynamic_detail_reduction_factor);
        m *= reduction;
    }

    if options.lod.enable_foveated_rendering {
        let cos_theta = if dist > 1e-10 {
            view.direction.dot(to_node / dist).clamp(-1.0, 1.0) as f64
        } else {
            1.0
        };
        let half_fov = match &view.projection {
            crate::view::Projection::Perspective { fov_y, fov_x } => fov_x.max(*fov_y) * 0.5,
            crate::view::Projection::Orthographic { .. } => std::f64::consts::FRAC_PI_2,
        };
        let node_angle = cos_theta.clamp(-1.0, 1.0).acos();
        let cone_angle = options.lod.foveated_cone_size * half_fov;
        if node_angle > cone_angle {
            let ring_fraction =
                ((node_angle - cone_angle) / (half_fov - cone_angle).max(1e-10)).clamp(0.0, 1.0);
            let time_ratio = if options.lod.foveated_time_delay > 0.0 {
                (camera_stationary_seconds as f64 / options.lod.foveated_time_delay as f64)
                    .clamp(0.0, 1.0)
            } else {
                1.0
            };
            let relaxation = options.lod.foveated_min_lod_metric_relaxation
                + (1.0 - options.lod.foveated_min_lod_metric_relaxation) * time_ratio;
            m *= 1.0 - ring_fraction * (1.0 - relaxation);
        }
    }

    m as f32
}

fn node_importance(bounds: &SpatialBounds, sse: f64, views: &[ViewState]) -> f32 {
    let center = bounds_center(bounds);
    let min_dist = views
        .iter()
        .map(|v| (center - v.position).length())
        .fold(f64::MAX, f64::min);
    let radius = match bounds {
        SpatialBounds::Sphere { radius, .. } => *radius,
        SpatialBounds::OrientedBox { half_axes, .. } => {
            let ax = half_axes.x_axis.length();
            let ay = half_axes.y_axis.length();
            let az = half_axes.z_axis.length();
            (ax * ax + ay * ay + az * az).sqrt()
        }
        SpatialBounds::AxisAlignedBox { min, max } => (*max - *min).length() * 0.5,
        _ => 1.0,
    };
    let ratio = radius / (1.0 + min_dist);
    (sse * ratio) as f32
}
