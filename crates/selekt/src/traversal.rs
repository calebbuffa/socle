/// Iterative depth-first traversal for LOD-driven node selection.
///
/// # Zero-allocation design
///
/// All working buffers are retained across frames inside [`TraversalBuffers`].
/// After the first few frames the capacity stabilises and the hot path
/// performs **zero heap allocations**.
///
/// - `StampSet` replaces `HashSet` - O(1) clear via generation bump, O(1)
///   insert/contains via direct index, no hashing.
/// - `NodeStateVec` replaces `HashMap` - O(1) indexed access, no hashing,
///   cache-friendly sequential layout.
/// - `u64` bitmask replaces `Vec<bool>` for per-view visibility.
/// - All hierarchy data accessed through shared references, nothing cloned.
use glam::DVec3;
use zukei::SpatialBounds;

use crate::hierarchy::SpatialHierarchy;
use crate::load::{LoadCandidate, LoadPriority, PriorityGroup};
use crate::lod::{LodDescriptor, LodEvaluator, LodFamily, RefinementMode};
use crate::node::{NodeId, NodeKind, NodeLoadState, NodeRefinementResult, NodeStateVec};
use crate::options::SelectionOptions;
use crate::policy::{NodeExcluder, OcclusionState, OcclusionTester, VisibilityPolicy};
use crate::view::ViewState;

// StampSet: O(1) membership set using generation stamps

/// O(1) membership set backed by a flat `Vec<u64>` of generation stamps.
///
/// `clear()` is O(1) - just bumps the generation counter.
/// `insert()` is O(1) - direct index, no hashing.
/// The backing vec grows once and is reused for the lifetime of the engine.
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

    /// O(1) clear: bump generation. Old stamps become stale.
    #[inline(always)]
    fn clear(&mut self) {
        self.generation += 1;
    }

    /// Insert `id`. Returns `true` if newly inserted this generation.
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

// TraversalBuffers: frame-persistent, zero-alloc after warmup

/// Frame-persistent traversal buffers. Cleared each frame; capacity retained.
pub(crate) struct TraversalBuffers {
    /// DFS stack entries: `(node_id, ancestor_was_rendered, skip_depth)`.  The bool
    /// propagates downward so descendants of an already-rendered ancestor
    /// are treated as background (Preload) instead of Normal priority.
    /// `skip_depth` counts consecutive skip-LOD levels above this node.
    stack: Vec<(NodeId, bool, u32)>,
    pub selected: Vec<NodeId>,
    selected_set: StampSet,
    pub candidates: Vec<LoadCandidate>,
    pub per_view_selected: Vec<Vec<NodeId>>,
    preload_visited: StampSet,
    /// Nodes fading out this frame (rendered last frame, not this frame).
    pub fading_out: Vec<NodeId>,
    /// Nodes fading in this frame (in render set this frame, not last frame).
    pub fading_in: Vec<NodeId>,
}

impl TraversalBuffers {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            selected: Vec::new(),
            selected_set: StampSet::new(),
            candidates: Vec::new(),
            per_view_selected: Vec::new(),
            preload_visited: StampSet::new(),
            fading_out: Vec::new(),
            fading_in: Vec::new(),
        }
    }

    /// Clear all buffers for a new frame. Retains heap capacity.
    /// `StampSet::clear()` is O(1), `Vec::clear()` is O(1).
    pub fn clear(&mut self, view_count: usize) {
        self.stack.clear();
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

/// Statistics returned from a traversal pass.
pub(crate) struct TraversalStats {
    pub visited: usize,
    pub culled: usize,
    pub occluded: usize,
    pub kicked: usize,
}

/// Run the selection traversal.
///
/// The hierarchy and node_states are intentionally separate parameters so the
/// borrow checker can verify that immutable hierarchy borrows (bounds, children,
/// lod_descriptor) never alias with mutable node_states writes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn traverse<H, B>(
    hierarchy: &H,
    lod_evaluator: &B,
    policy: &dyn VisibilityPolicy,
    excluders: &[Box<dyn NodeExcluder>],
    occlusion_tester: &dyn OcclusionTester,
    node_states: &mut NodeStateVec,
    views: &[ViewState],
    options: &SelectionOptions,
    frame_index: u64,
    view_group_key: u64,
    view_group_weight: u16,
    lod_multiplier: f64,
    camera_stationary_seconds: f32,
    camera_velocity: DVec3,
    buffers: &mut TraversalBuffers,
) -> TraversalStats
where
    H: SpatialHierarchy,
    B: LodEvaluator,
{
    let view_count = views.len();
    assert!(
        view_count <= 64,
        "selekt supports at most 64 simultaneous views"
    );
    debug_assert!(!views.is_empty(), "traverse called with no views");

    // Fog culling: compute per-frame effective lod_multiplier.
    // When fog is enabled and the density table is non-empty, attenuation is
    // derived from the primary view's height above the ellipsoid (y-axis).
    // Density is linearly interpolated from the table; larger density → smaller
    // multiplier → fewer refinements.
    let effective_lod_multiplier =
        if options.culling.enable_fog_culling && !options.culling.fog_density_table.is_empty() {
            // Use first view position's length as a rough proxy for height.
            let height = views[0].position.length();
            let density = sample_fog_density(&options.culling.fog_density_table, height);
            // Attenuation: at density ≥ 1.0 the multiplier collapses to 0.001
            // (never refine); at density 0 no change.  Mirror Cesium's formula:
            // multiplier * exp(-density * density * distance²) — but since we
            // don't have per-node distance here, we apply a global frame-level
            // knob: effective_multiplier = lod_multiplier / (1 + density * 4).
            lod_multiplier / (1.0 + density * 4.0)
        } else {
            lod_multiplier
        };

    // Progressive resolution: while any node is in-flight, shrink the effective
    // viewport height, making the LOD metric easier to satisfy. Coarser ancestors
    // render immediately while detail nodes stream in, eliminating blank-screen gaps.
    // We detect active loading lazily inside the loop using `any_node_loading`.
    let progressive_multiplier = if options.lod.enable_progressive_resolution {
        options
            .lod.progressive_resolution_height_fraction
            .clamp(0.01, 0.99)
    } else {
        1.0
    };
    // Becomes `true` the first time we see a Loading/Queued child, causing us to
    // apply `progressive_multiplier` for that frame.
    let mut any_node_loading = false;

    let mut visited = 0usize;
    let mut culled = 0usize;
    let mut occluded_count = 0usize;
    let mut kicked_count = 0usize;

    buffers.stack.push((hierarchy.root(), false, 0));

    while let Some((node, ancestor_rendered, skip_depth)) = buffers.stack.pop() {
        let bounds = hierarchy.bounds(node);
        let kind = hierarchy.node_kind(node);
        let mode = hierarchy.refinement_mode(node);
        let children = hierarchy.children(node);
        let has_content = !hierarchy.content_keys(node).is_empty();

        let state = node_states.get(node);
        let lifecycle = state.lifecycle;

        if lifecycle == NodeLoadState::Failed {
            continue;
        }
        if lifecycle == NodeLoadState::RetryScheduled && frame_index < state.next_retry_frame {
            continue;
        }

        // Track that at least one node is actively loading this frame so the
        // progressive-resolution path can relax LOD thresholds globally.
        if options.lod.enable_progressive_resolution && is_pending(lifecycle) {
            any_node_loading = true;
        }

        if excluders.iter().any(|e| e.should_exclude(node, bounds)) {
            culled += 1;
            continue;
        }

        // Clipping planes: skip nodes whose bounding volume lies entirely on
        // the clipped side of any plane.
        if !options.culling.clipping_planes.is_empty() {
            let fully_clipped = options.culling.clipping_planes.iter().any(|plane| {
                crate::evaluators::bounds_entirely_clipped(bounds, plane.normal, plane.distance)
            });
            if fully_clipped {
                culled += 1;
                continue;
            }
        }

        // Viewer request volume: skip the entire subtree unless the primary
        // camera is inside the declared request volume for this node.
        if let Some(vrv) = hierarchy.viewer_request_volume(node) {
            let camera_inside = views
                .iter()
                .any(|v| crate::evaluators::point_inside_bounds(v.position, vrv));
            if !camera_inside {
                culled += 1;
                continue;
            }
        }

        // Visibility bitmask: bit i set <=> node visible in view i.
        let vis: u64 = if options.culling.enable_frustum_culling {
            let mut bits = 0u64;
            for (i, view) in views.iter().enumerate() {
                if policy.is_visible(node, bounds, view) {
                    bits |= 1 << i;
                }
            }
            bits
        } else {
            (1u64 << view_count) - 1
        };

        // render_nodes_under_camera: even if the node is frustum-culled,
        // include it if the camera is directly over its footprint.
        let vis = if options.culling.render_nodes_under_camera && vis == 0 {
            let mut override_bits = 0u64;
            for (i, view) in views.iter().enumerate() {
                if crate::evaluators::camera_is_over_bounds(view.position, bounds) {
                    override_bits |= 1 << i;
                }
            }
            override_bits
        } else {
            vis
        };

        if vis == 0 {
            culled += 1;
            if node_states.get(node).last_result.was_rendered() {
                buffers.fading_out.push(node);
            }
            node_states.get_mut(node).last_result = NodeRefinementResult::Culled;

            // Culled SSE: even though the node is outside the frustum, we may
            // still need to refine it if its SSE exceeds the culled threshold.
            // This keeps geometry streaming near the frustum edge and avoids
            // pop-in when the camera pans.
            let should_refine_culled = options.culling.enforce_culled_screen_space_error
                && !children.is_empty()
                && kind != NodeKind::Empty
                && {
                    let lod_desc = hierarchy.lod_descriptor(node);
                    let mut tmp = LodDescriptor {
                        value: 0.0,
                        family: LodFamily::NONE,
                    };
                    let lod_desc =
                        resolve_lod_desc(lod_desc, hierarchy.lod_metric_override(node), &mut tmp);
                    let culled_multiplier =
                        (options.culling.culled_screen_space_error as f32).max(f32::EPSILON);
                    lod_evaluator.should_refine(
                        lod_desc,
                        &views[0],
                        culled_multiplier,
                        bounds,
                        mode,
                    )
                };

            if should_refine_culled {
                push_children_rev(children, ancestor_rendered, 0, &mut buffers.stack);
            } else if options.loading.prevent_holes && mode == RefinementMode::Replace {
                for &child in children {
                    let child_lc = node_states.get(child).lifecycle;
                    if needs_load(child_lc) && !hierarchy.content_keys(child).is_empty() {
                        buffers.candidates.push(LoadCandidate {
                            view_group: view_group_key,
                            node_id: child,
                            priority: LoadPriority {
                                group: PriorityGroup::Normal,
                                score: i64::MAX,
                                view_group_weight,
                            },
                        });
                    }
                }
            }
            continue;
        }

        visited += 1;

        // Importance score for cache eviction ordering.
        {
            let lod_desc = hierarchy.lod_descriptor(node);
            let sse = hierarchy
                .lod_metric_override(node)
                .unwrap_or(lod_desc.value);
            node_states.get_mut(node).importance = node_importance(bounds, sse, views);
        }

        let occluded = options.culling.enable_occlusion_culling
            && occlusion_tester.occlusion_state(node) == OcclusionState::Occluded;

        let should_refine = if (occluded && options.culling.delay_refinement_for_occlusion)
            || children.is_empty()
            || kind == NodeKind::Empty
        {
            false
        } else {
            let lod_desc = hierarchy.lod_descriptor(node);
            let mut lod_desc_tmp = LodDescriptor {
                value: 0.0,
                family: LodFamily::NONE,
            };
            let lod_desc = resolve_lod_desc(
                lod_desc,
                hierarchy.lod_metric_override(node),
                &mut lod_desc_tmp,
            );
            views.iter().enumerate().any(|(i, view)| {
                if (vis & (1 << i)) == 0 {
                    return false;
                }
                let m = view.lod_metric_multiplier
                    * effective_view_multiplier(
                        view,
                        bounds,
                        options,
                        effective_lod_multiplier,
                        any_node_loading,
                        progressive_multiplier,
                        camera_stationary_seconds,
                    );
                lod_evaluator.should_refine(lod_desc, view, m, bounds, mode)
            })
        };

        let (should_refine, was_kicked) = if should_refine {
            let loading_descendants = children
                .iter()
                .filter(|&&child| is_pending(node_states.get(child).lifecycle))
                .count();
            if loading_descendants <= options.loading.loading_descendant_limit {
                (true, false)
            } else {
                (false, true)
            }
        } else {
            (false, false)
        };

        let last_result = node_states.get(node).last_result;
        let was_refined =
            last_result.is_refined() || last_result == NodeRefinementResult::RenderedAndKicked;
        let all_children_renderable = !children.is_empty()
            && children
                .iter()
                .all(|&child| node_states.get(child).lifecycle == NodeLoadState::Renderable);
        let must_continue_refining = was_refined
            && !all_children_renderable
            && mode == RefinementMode::Replace
            && !options.lod.immediately_load_desired_lod;

        match kind {
            NodeKind::Empty | NodeKind::CompositeRoot => {
                node_states.get_mut(node).last_result = if children.is_empty() {
                    NodeRefinementResult::None
                } else {
                    NodeRefinementResult::Refined
                };
                push_children_rev(children, ancestor_rendered, 0, &mut buffers.stack);
            }
            NodeKind::Reference => {
                if !children.is_empty() {
                    node_states.get_mut(node).last_result = NodeRefinementResult::Refined;
                    push_children_rev(children, ancestor_rendered, 0, &mut buffers.stack);
                } else {
                    let score = priority_score(bounds, views, options, camera_velocity);
                    let result = if lifecycle == NodeLoadState::Renderable && has_content {
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
                        view_group_key,
                        view_group_weight,
                        score,
                        buffers,
                        node_states,
                    );
                    node_states.get_mut(node).last_result = result;
                }
            }
            NodeKind::Renderable => {
                let score = priority_score(bounds, views, options, camera_velocity);
                if (should_refine || must_continue_refining) && !children.is_empty() {
                    match mode {
                        RefinementMode::Add => {
                            // Add mode: render self AND push children.
                            leaf_select_or_queue(
                                node,
                                lifecycle,
                                has_content,
                                vis,
                                ancestor_rendered,
                                view_group_key,
                                view_group_weight,
                                score,
                                buffers,
                                node_states,
                            );
                            let self_rendered =
                                lifecycle == NodeLoadState::Renderable && has_content;
                            push_children_rev(
                                children,
                                ancestor_rendered || self_rendered,
                                0,
                                &mut buffers.stack,
                            );
                            node_states.get_mut(node).last_result = NodeRefinementResult::Refined;
                        }
                        RefinementMode::Replace => {
                            let mut self_rendered = false;
                            if !all_children_renderable {
                                match lifecycle {
                                    NodeLoadState::Renderable => {
                                        do_select(node, vis, buffers, node_states);
                                        self_rendered = true;
                                    }
                                    lc if needs_load(lc) => {
                                        if has_content {
                                            let group = if was_refined {
                                                PriorityGroup::Urgent
                                            } else if ancestor_rendered {
                                                PriorityGroup::Preload
                                            } else {
                                                PriorityGroup::Normal
                                            };
                                            buffers.candidates.push(LoadCandidate {
                                                view_group: view_group_key,
                                                node_id: node,
                                                priority: LoadPriority {
                                                    group,
                                                    score,
                                                    view_group_weight,
                                                },
                                            });
                                        }
                                    }
                                    lc if is_pending(lc) => {
                                        if has_content {
                                            buffers.candidates.push(LoadCandidate {
                                                view_group: view_group_key,
                                                node_id: node,
                                                priority: LoadPriority {
                                                    group: PriorityGroup::Urgent,
                                                    score,
                                                    view_group_weight,
                                                },
                                            });
                                        }
                                    }
                                    _ => {}
                                }
                            } else if node_states.get(node).last_result.was_rendered() {
                                // Children are now fully resident: this node fades out.
                                buffers.fading_out.push(node);
                            }
                            push_children_rev(
                                children,
                                ancestor_rendered || self_rendered,
                                0,
                                &mut buffers.stack,
                            );
                            node_states.get_mut(node).last_result = if self_rendered {
                                NodeRefinementResult::RenderedAndKicked
                            } else {
                                NodeRefinementResult::Refined
                            };
                        }
                    }
                } else {
                    // Not refining (leaf, culled children, or kicked by loading limit).
                    if was_kicked {
                        kicked_count += 1;
                    }
                    if occluded {
                        occluded_count += 1;
                    }

                    // Skip-LOD: if this node's LOD metric is large enough and the
                    // children would refine at a loosened threshold, push the children
                    // instead of rendering this node as a leaf.
                    let lod_desc = hierarchy.lod_descriptor(node);
                    let mut lod_skip_tmp = LodDescriptor {
                        value: 0.0,
                        family: LodFamily::NONE,
                    };
                    let lod_desc = resolve_lod_desc(
                        lod_desc,
                        hierarchy.lod_metric_override(node),
                        &mut lod_skip_tmp,
                    );

                    // Force descent when skip-LOD is active and this node has not yet
                    // descended skip_levels below the last rendered ancestor.  This
                    // enforces the minimum gap mandated by `options.lod.skip_levels`.
                    let force_skip = options.lod.skip_level_of_detail
                        && !was_kicked
                        && !children.is_empty()
                        && skip_depth < options.lod.skip_levels
                        && lod_desc.value > options.lod.base_lod_metric_threshold;

                    let skip_eligible = !force_skip
                        && options.lod.skip_level_of_detail
                        && !was_kicked
                        && !children.is_empty()
                        && lod_desc.value > options.lod.base_lod_metric_threshold
                        && views.iter().enumerate().any(|(i, view)| {
                            if (vis & (1 << i)) == 0 {
                                return false;
                            }
                            let m = view.lod_metric_multiplier
                                * effective_view_multiplier(
                                    view,
                                    bounds,
                                    options,
                                    effective_lod_multiplier / options.lod.skip_lod_metric_factor,
                                    any_node_loading,
                                    progressive_multiplier,
                                    camera_stationary_seconds,
                                );
                            lod_evaluator.should_refine(lod_desc, view, m, bounds, mode)
                        });

                    if force_skip {
                        push_children_rev(
                            children,
                            ancestor_rendered,
                            skip_depth + 1,
                            &mut buffers.stack,
                        );
                        node_states.get_mut(node).last_result = NodeRefinementResult::Refined;
                    } else if skip_eligible {
                        push_children_rev(
                            children,
                            ancestor_rendered,
                            skip_depth + 1,
                            &mut buffers.stack,
                        );
                        node_states.get_mut(node).last_result = NodeRefinementResult::Refined;
                        // load_siblings_on_skip: preload siblings at Preload priority so
                        // they are ready if the camera moves to an adjacent area.
                        if options.streaming.load_siblings_on_skip {
                            if let Some(parent) = hierarchy.parent(node) {
                                for &sibling in hierarchy.children(parent) {
                                    if sibling == node {
                                        continue;
                                    }
                                    let sib_lc = node_states.get(sibling).lifecycle;
                                    if needs_load(sib_lc)
                                        && !hierarchy.content_keys(sibling).is_empty()
                                    {
                                        let sib_score = priority_score(
                                            hierarchy.bounds(sibling),
                                            views,
                                            options,
                                            camera_velocity,
                                        );
                                        buffers.candidates.push(LoadCandidate {
                                            view_group: view_group_key,
                                            node_id: sibling,
                                            priority: LoadPriority {
                                                group: PriorityGroup::Preload,
                                                score: sib_score,
                                                view_group_weight,
                                            },
                                        });
                                    }
                                }
                            }
                        }
                    } else {
                        let result = if lifecycle == NodeLoadState::Renderable && has_content {
                            if was_kicked {
                                NodeRefinementResult::RenderedAndKicked
                            } else {
                                NodeRefinementResult::Rendered
                            }
                        } else {
                            NodeRefinementResult::None
                        };
                        leaf_select_or_queue(
                            node,
                            lifecycle,
                            has_content,
                            vis,
                            ancestor_rendered,
                            view_group_key,
                            view_group_weight,
                            score,
                            buffers,
                            node_states,
                        );
                        node_states.get_mut(node).last_result = result;
                    }
                }
            }
        }
    }

    if options.loading.preload_ancestors || options.loading.preload_siblings {
        preload_pass(
            hierarchy,
            node_states,
            options,
            view_group_key,
            view_group_weight,
            buffers,
        );
    }

    // Flight-destination preloading: enqueue nodes visible from declared
    // waypoints at Preload priority so they are ready when the camera arrives.
    if !options.streaming.flight_destinations.is_empty() {
        flight_preload_pass(
            hierarchy,
            lod_evaluator,
            policy,
            node_states,
            options,
            view_group_key,
            view_group_weight,
            camera_velocity,
            buffers,
        );
    }

    TraversalStats {
        visited,
        culled,
        occluded: occluded_count,
        kicked: kicked_count,
    }
}

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
    stack: &mut Vec<(NodeId, bool, u32)>,
) {
    for &child in children.iter().rev() {
        stack.push((child, ancestor_rendered, skip_depth));
    }
}

#[inline(always)]
fn leaf_select_or_queue(
    node: NodeId,
    lifecycle: NodeLoadState,
    has_content: bool,
    vis: u64,
    ancestor_rendered: bool,
    view_group_key: u64,
    view_group_weight: u16,
    score: i64,
    buffers: &mut TraversalBuffers,
    node_states: &NodeStateVec,
) {
    match lifecycle {
        NodeLoadState::Renderable => {
            do_select(node, vis, buffers, node_states);
        }
        lc if needs_load(lc) => {
            if has_content {
                let group = if ancestor_rendered {
                    PriorityGroup::Preload
                } else {
                    PriorityGroup::Normal
                };
                buffers.candidates.push(LoadCandidate {
                    view_group: view_group_key,
                    node_id: node,
                    priority: LoadPriority {
                        group,
                        score,
                        view_group_weight,
                    },
                });
            }
        }
        _ => {}
    }
}

/// Returns the centroid of a [`SpatialBounds`] in 3-D world space.
#[inline(always)]
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
    }
}

/// Compute a load priority score for a node given its bounds and the active views.
///
/// Uses `(1 − \cos{\theta}) × distance` where `\theta` is the angle between the view direction
/// and the vector from camera to node centre. Score is 0 for a node at the exact
/// screen centre, and increases toward the back of the camera. Scores are compared
/// across views and the **minimum** is kept (node closest to any screen centre wins).
///
/// Lower score -> higher priority, consistent with [`LoadPriority::score`].
#[inline]
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
            return 0; // Camera is inside the node: highest priority.
        }
        let distance = dist_sq.sqrt();
        let cos_theta = view.direction.dot(to_node / distance);
        // angle_factor: 0 when on screen centre, up to 2 when directly behind.
        let angle_factor = 1.0 - cos_theta;
        let mut score = (angle_factor * distance * 1_000_000.0) as i64;

        // Request-volume priority: boost nodes in the camera movement direction.
        if options.streaming.enable_request_render_mode_priority {
            let speed = camera_velocity.length();
            if speed > 1e-3 {
                let vel_dir = camera_velocity / speed;
                let cos_vel = vel_dir.dot(to_node / distance);
                let vel_angle = cos_vel.clamp(-1.0, 1.0).acos();
                if vel_angle < options.streaming.request_render_mode_priority_angle {
                    // Reduce score (raise priority) proportionally to alignment.
                    let boost = (1.0 - vel_angle / options.streaming.request_render_mode_priority_angle)
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
fn do_select(node: NodeId, vis: u64, buffers: &mut TraversalBuffers, node_states: &NodeStateVec) {
    if buffers.selected_set.insert(node) {
        buffers.selected.push(node);
        // Track nodes newly entering the render set.
        if !node_states.get(node).last_result.was_rendered() {
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

fn preload_pass<H: SpatialHierarchy>(
    hierarchy: &H,
    node_states: &NodeStateVec,
    options: &SelectionOptions,
    view_group_key: u64,
    view_group_weight: u16,
    buffers: &mut TraversalBuffers,
) {
    let selected_count = buffers.selected.len();

    for idx in 0..selected_count {
        let node = buffers.selected[idx];

        if options.loading.preload_ancestors {
            let mut current = node;
            while let Some(parent) = hierarchy.parent(current) {
                if !buffers.preload_visited.insert(parent) {
                    break;
                }
                let lifecycle = node_states.get(parent).lifecycle;
                if needs_load(lifecycle) && !hierarchy.content_keys(parent).is_empty() {
                    buffers.candidates.push(LoadCandidate {
                        view_group: view_group_key,
                        node_id: parent,
                        priority: LoadPriority {
                            group: PriorityGroup::Preload,
                            score: i64::MAX,
                            view_group_weight,
                        },
                    });
                }
                current = parent;
            }
        }

        if options.loading.preload_siblings {
            if let Some(parent) = hierarchy.parent(node) {
                for &sibling in hierarchy.children(parent) {
                    if sibling == node || !buffers.preload_visited.insert(sibling) {
                        continue;
                    }
                    let lifecycle = node_states.get(sibling).lifecycle;
                    if needs_load(lifecycle) && !hierarchy.content_keys(sibling).is_empty() {
                        buffers.candidates.push(LoadCandidate {
                            view_group: view_group_key,
                            node_id: sibling,
                            priority: LoadPriority {
                                group: PriorityGroup::Preload,
                                score: i64::MAX,
                                view_group_weight,
                            },
                        });
                    }
                }
            }
        }
    }
}

/// Iterative DFS preload pass for declared flight-destination waypoints.
///
/// For each waypoint view-set, we do a shallow DFS: nodes that are visible
/// from the waypoint and whose LOD metric says they should refine are queued
/// at `Preload` priority. Maximum depth is capped at 8 levels to keep the
/// pass lightweight.
#[allow(clippy::too_many_arguments)]
fn flight_preload_pass<H, B>(
    hierarchy: &H,
    lod_evaluator: &B,
    policy: &dyn VisibilityPolicy,
    node_states: &NodeStateVec,
    options: &SelectionOptions,
    view_group_key: u64,
    view_group_weight: u16,
    camera_velocity: DVec3,
    buffers: &mut TraversalBuffers,
) where
    H: SpatialHierarchy,
    B: LodEvaluator,
{
    const MAX_DEPTH: usize = 8;

    for dest_views in &options.streaming.flight_destinations {
        if dest_views.is_empty() {
            continue;
        }
        // Lightweight DFS: (node_id, depth).
        let mut stack: Vec<(NodeId, usize)> = vec![(hierarchy.root(), 0)];
        while let Some((node, depth)) = stack.pop() {
            let bounds = hierarchy.bounds(node);
            let kind = hierarchy.node_kind(node);
            let mode = hierarchy.refinement_mode(node);
            let children = hierarchy.children(node);

            // Visibility check against destination views.
            let visible = dest_views
                .iter()
                .any(|v| policy.is_visible(node, bounds, v));
            if !visible {
                continue;
            }

            let lod_desc = hierarchy.lod_descriptor(node);
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
                // Leaf of the flight-destination LOD tree: queue if not loaded.
                let lc = node_states.get(node).lifecycle;
                if needs_load(lc) && !hierarchy.content_keys(node).is_empty() {
                    let score = priority_score(bounds, dest_views, options, camera_velocity);
                    buffers.candidates.push(LoadCandidate {
                        view_group: view_group_key,
                        node_id: node,
                        priority: LoadPriority {
                            group: PriorityGroup::Preload,
                            score,
                            view_group_weight,
                        },
                    });
                }
            }
        }
    }
}

/// Sample fog density from the lookup table at the given height.
///
/// The table must be sorted by ascending height. Returns 0.0 for heights
/// below the table minimum and clamps to the last value above the maximum.
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
    // Linear interpolation between surrounding entries.
    let idx = table.partition_point(|&(h, _)| h <= height);
    let (h0, d0) = table[idx - 1];
    let (h1, d1) = table[idx];
    let t = (height - h0) / (h1 - h0);
    d0 + t * (d1 - d0)
}

/// Return `lod_desc` with its value replaced by `override_val` when present.
///
/// The temporary storage `tmp` must outlive the returned reference; callers
/// declare it as `let tmp;` before calling this function.
#[inline(always)]
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

/// Compute the effective LOD metric multiplier for a single view/node combination.
///
/// Applies fog attenuation, progressive resolution, dynamic-detail reduction,
/// and foveated rendering in order. Returns the final scalar that should be
/// multiplied into `view.lod_metric_multiplier` before calling `should_refine`.
#[inline(always)]
fn effective_view_multiplier(
    view: &ViewState,
    bounds: &SpatialBounds,
    options: &SelectionOptions,
    base_multiplier: f64,
    any_node_loading: bool,
    progressive_multiplier: f64,
    camera_stationary_seconds: f32,
) -> f32 {
    debug_assert!(
        base_multiplier >= 0.0,
        "base_multiplier must be non-negative"
    );
    debug_assert!(
        progressive_multiplier > 0.0,
        "progressive_multiplier must be positive"
    );
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

/// Compute a node's importance score for cache-eviction ordering.
///
/// Higher importance = survives eviction longer. Formula:
/// `sse / (1 + dist_to_nearest_camera / approx_radius)`.
#[inline(always)]
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
            ax.max(ay).max(az)
        }
        SpatialBounds::AxisAlignedBox { min, max } => {
            ((max.x - min.x).max(max.y - min.y).max(max.z - min.z)) * 0.5
        }
        _ => 1.0,
    };
    (sse / (1.0 + min_dist / radius.max(1.0))) as f32
}
