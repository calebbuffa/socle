/// Iterative depth-first traversal for LOD-driven tile selection.
///
/// Key invariants enforced here:
/// - Continuity lock: a `Replace`-mode parent remains selected as ancestor
///   fallback while children are not yet renderable.
/// - Hole prevention: culled `Replace` nodes force-queue their children at
///   `Normal` priority.
/// - Reference nodes: unresolved references queue their own metadata load;
///   resolved references become structural and recurse into their patched
///   children.
use std::collections::{HashMap, HashSet};

use crate::hierarchy::SpatialHierarchy;
use crate::load::{ContentKey, LoadCandidate, LoadPriority, PriorityGroup};
use crate::lod::{LodEvaluator, RefinementMode};
use crate::node::{NodeId, NodeKind, NodeLifecycleState, NodeState};
use crate::options::SelectionOptions;
use crate::policy::{OcclusionState, OcclusionTester, Policy, TileExcluder};
use crate::view::{PerViewUpdateResult, ViewState};

struct Frame {
    node_id: NodeId,
}

/// Inputs for a single traversal pass (one view group, one frame).
pub(crate) struct TraversalContext<'a, H, B, M>
where
    H: SpatialHierarchy,
    B: LodEvaluator,
    M: Policy,
{
    pub hierarchy: &'a H,
    pub lod_evaluator: &'a B,
    pub policy: &'a M,
    pub excluders: &'a [Box<dyn TileExcluder>],
    pub occlusion_tester: &'a dyn OcclusionTester,
    pub node_states: &'a mut HashMap<NodeId, NodeState>,
    pub views: &'a [ViewState],
    pub options: &'a SelectionOptions,
    pub frame_index: u64,
    pub view_group_key: u64,
    pub view_group_weight: u16,
}

/// Outputs of a traversal pass.
pub(crate) struct TraversalResult {
    pub selected: Vec<NodeId>,
    pub per_view: Vec<PerViewUpdateResult>,
    pub candidates: Vec<LoadCandidate>,
    pub visited: usize,
    pub culled: usize,
}

pub(crate) fn traverse<H, B, M>(ctx: &mut TraversalContext<'_, H, B, M>) -> TraversalResult
where
    H: SpatialHierarchy,
    B: LodEvaluator,
    M: Policy,
{
    let root = ctx.hierarchy.root();
    let view_count = ctx.views.len();

    let mut selected = Vec::new();
    let mut selected_set = HashSet::new();
    let mut per_view_sel: Vec<Vec<NodeId>> = (0..view_count).map(|_| Vec::new()).collect();
    let mut candidates = Vec::new();
    let mut visited = 0usize;
    let mut culled = 0usize;

    let mut stack = vec![Frame { node_id: root }];

    while let Some(Frame { node_id: node }) = stack.pop() {
        let bounds = ctx.hierarchy.bounds(node).clone();
        let kind = ctx.hierarchy.node_kind(node);
        let mode = ctx.hierarchy.refinement_mode(node);
        let children: Vec<NodeId> = ctx.hierarchy.children(node).to_vec();
        let content_key = ctx.hierarchy.content_key(node).cloned();
        let lod_desc = if !children.is_empty() && kind != NodeKind::Empty {
            Some(ctx.hierarchy.lod_descriptor(node).clone())
        } else {
            None
        };

        let lifecycle = node_lifecycle(ctx, node);
        if lifecycle == NodeLifecycleState::Failed {
            continue;
        }
        if lifecycle == NodeLifecycleState::RetryScheduled {
            let due = ctx
                .node_states
                .get(&node)
                .map_or(0, |state| state.next_retry_frame);
            if ctx.frame_index < due {
                continue;
            }
        }

        // If any excluder rejects this node, skip the entire subtree.
        if ctx
            .excluders
            .iter()
            .any(|e| e.should_exclude(node, &bounds))
        {
            culled += 1;
            continue;
        }

        let vis: Vec<bool> = if ctx.options.enable_frustum_culling {
            ctx.views
                .iter()
                .map(|view| ctx.policy.is_visible(node, &bounds, view))
                .collect()
        } else {
            vec![true; ctx.views.len()]
        };
        let any_visible = vis.iter().any(|&visible| visible);

        if !any_visible {
            culled += 1;
            if ctx.options.prevent_holes && mode == RefinementMode::Replace {
                for &child in &children {
                    let child_lifecycle = node_lifecycle(ctx, child);
                    if needs_load(child_lifecycle) {
                        if let Some(key) = ctx.hierarchy.content_key(child).cloned() {
                            candidates.push(make_candidate(
                                child,
                                key,
                                PriorityGroup::Normal,
                                i64::MAX,
                                ctx.view_group_key,
                                ctx.view_group_weight,
                            ));
                        }
                    }
                }
            }
            continue;
        }

        visited += 1;

        // If the renderer reports this node is occluded, defer refinement.
        let occluded = ctx.options.enable_occlusion_culling
            && ctx.occlusion_tester.occlusion_state(node) == OcclusionState::Occluded;

        let should_refine = if occluded {
            false
        } else {
            lod_desc.as_ref().map_or(false, |descriptor| {
                ctx.views.iter().enumerate().any(|(index, view)| {
                    vis[index]
                        && ctx
                            .lod_evaluator
                            .should_refine(descriptor, view, &bounds, mode)
                })
            })
        };

        // If too many children are already in a loading/queued state, stop
        // descending further to prevent unbounded in-flight load explosion.
        let loading_descendants = if should_refine && !children.is_empty() {
            children
                .iter()
                .filter(|&&child| is_pending(node_lifecycle(ctx, child)))
                .count()
        } else {
            0
        };
        let descendant_limit_exceeded = loading_descendants > ctx.options.loading_descendant_limit;
        let should_refine = should_refine && !descendant_limit_exceeded;

        let was_refined = ctx
            .node_states
            .get(&node)
            .map_or(false, |state| state.was_refined_last_frame);
        let all_children_renderable = !children.is_empty()
            && children.iter().all(|&child| {
                ctx.node_states.get(&child).map_or(false, |state| {
                    state.lifecycle == NodeLifecycleState::Renderable
                })
            });
        let must_continue_refining =
            was_refined && !all_children_renderable && mode == RefinementMode::Replace;

        match kind {
            NodeKind::Empty | NodeKind::CompositeRoot => {
                set_refined(ctx, node, !children.is_empty());
                push_children(&children, &mut stack);
            }
            NodeKind::Reference => {
                if !children.is_empty() {
                    set_refined(ctx, node, true);
                    push_children(&children, &mut stack);
                } else {
                    set_refined(ctx, node, false);
                    handle_leaf(
                        ctx,
                        node,
                        lifecycle,
                        content_key.as_ref(),
                        &vis,
                        &mut selected,
                        &mut per_view_sel,
                        &mut selected_set,
                        &mut candidates,
                    );
                }
            }
            NodeKind::Renderable => {
                if (should_refine || must_continue_refining) && !children.is_empty() {
                    handle_refine(
                        ctx,
                        node,
                        mode,
                        lifecycle,
                        was_refined,
                        all_children_renderable,
                        &children,
                        &vis,
                        &mut selected,
                        &mut per_view_sel,
                        &mut selected_set,
                        &mut candidates,
                        &mut stack,
                        content_key.as_ref(),
                    );
                } else {
                    set_refined(ctx, node, false);
                    handle_leaf(
                        ctx,
                        node,
                        lifecycle,
                        content_key.as_ref(),
                        &vis,
                        &mut selected,
                        &mut per_view_sel,
                        &mut selected_set,
                        &mut candidates,
                    );
                }
            }
        }
    }

    let per_view = ctx
        .views
        .iter()
        .zip(per_view_sel)
        .map(|(_view, sel)| PerViewUpdateResult {
            selected: sel,
            visited,
            culled,
        })
        .collect();

    // After the main selection, queue ancestors and/or siblings of selected
    // nodes at Preload priority so they are ready before the camera needs them.
    if ctx.options.preload_ancestors || ctx.options.preload_siblings {
        preload_pass(ctx, &selected, &mut candidates);
    }

    TraversalResult {
        selected,
        per_view,
        candidates,
        visited,
        culled,
    }
}

fn node_lifecycle<H: SpatialHierarchy, B: LodEvaluator, M: Policy>(
    ctx: &TraversalContext<'_, H, B, M>,
    node: NodeId,
) -> NodeLifecycleState {
    ctx.node_states
        .get(&node)
        .map_or(NodeLifecycleState::Unloaded, |state| state.lifecycle)
}

fn needs_load(lifecycle: NodeLifecycleState) -> bool {
    matches!(
        lifecycle,
        NodeLifecycleState::Unloaded
            | NodeLifecycleState::Evicted
            | NodeLifecycleState::RetryScheduled
    )
}

fn is_pending(lifecycle: NodeLifecycleState) -> bool {
    matches!(
        lifecycle,
        NodeLifecycleState::Queued | NodeLifecycleState::Loading
    )
}

fn set_refined<H: SpatialHierarchy, B: LodEvaluator, M: Policy>(
    ctx: &mut TraversalContext<'_, H, B, M>,
    node: NodeId,
    refined: bool,
) {
    ctx.node_states
        .entry(node)
        .or_insert_with(NodeState::new)
        .was_refined_last_frame = refined;
}

fn push_children(children: &[NodeId], stack: &mut Vec<Frame>) {
    for &child in children.iter().rev() {
        stack.push(Frame { node_id: child });
    }
}

fn do_select(
    node: NodeId,
    vis: &[bool],
    selected: &mut Vec<NodeId>,
    per_view_sel: &mut [Vec<NodeId>],
    selected_set: &mut HashSet<NodeId>,
) {
    if selected_set.insert(node) {
        selected.push(node);
    }
    for (index, &visible) in vis.iter().enumerate() {
        if visible {
            per_view_sel[index].push(node);
        }
    }
}

fn make_candidate(
    node: NodeId,
    key: ContentKey,
    group: PriorityGroup,
    score: i64,
    view_group_key: u64,
    view_group_weight: u16,
) -> LoadCandidate {
    LoadCandidate {
        view_group: view_group_key,
        node_id: node,
        key,
        priority: LoadPriority {
            group,
            score,
            view_group_weight,
        },
    }
}

fn handle_leaf<H: SpatialHierarchy, B: LodEvaluator, M: Policy>(
    ctx: &mut TraversalContext<'_, H, B, M>,
    node: NodeId,
    lifecycle: NodeLifecycleState,
    content_key: Option<&ContentKey>,
    vis: &[bool],
    selected: &mut Vec<NodeId>,
    per_view_sel: &mut [Vec<NodeId>],
    selected_set: &mut HashSet<NodeId>,
    candidates: &mut Vec<LoadCandidate>,
) {
    match lifecycle {
        NodeLifecycleState::Renderable => {
            do_select(node, vis, selected, per_view_sel, selected_set);
        }
        pending if needs_load(pending) => {
            if let Some(key) = content_key {
                candidates.push(make_candidate(
                    node,
                    key.clone(),
                    PriorityGroup::Normal,
                    0,
                    ctx.view_group_key,
                    ctx.view_group_weight,
                ));
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_refine<H: SpatialHierarchy, B: LodEvaluator, M: Policy>(
    ctx: &mut TraversalContext<'_, H, B, M>,
    node: NodeId,
    mode: RefinementMode,
    lifecycle: NodeLifecycleState,
    was_refined: bool,
    all_children_renderable: bool,
    children: &[NodeId],
    vis: &[bool],
    selected: &mut Vec<NodeId>,
    per_view_sel: &mut [Vec<NodeId>],
    selected_set: &mut HashSet<NodeId>,
    candidates: &mut Vec<LoadCandidate>,
    stack: &mut Vec<Frame>,
    content_key: Option<&ContentKey>,
) {
    set_refined(ctx, node, true);

    match mode {
        RefinementMode::Add => {
            match lifecycle {
                NodeLifecycleState::Renderable => {
                    do_select(node, vis, selected, per_view_sel, selected_set);
                }
                pending if needs_load(pending) => {
                    if let Some(key) = content_key {
                        candidates.push(make_candidate(
                            node,
                            key.clone(),
                            PriorityGroup::Normal,
                            0,
                            ctx.view_group_key,
                            ctx.view_group_weight,
                        ));
                    }
                }
                _ => {}
            }
            push_children(children, stack);
        }
        RefinementMode::Replace => {
            if !all_children_renderable {
                match lifecycle {
                    NodeLifecycleState::Renderable => {
                        do_select(node, vis, selected, per_view_sel, selected_set);
                    }
                    pending if needs_load(pending) => {
                        let priority_group = if was_refined {
                            PriorityGroup::Urgent
                        } else {
                            PriorityGroup::Normal
                        };
                        if let Some(key) = content_key {
                            candidates.push(make_candidate(
                                node,
                                key.clone(),
                                priority_group,
                                0,
                                ctx.view_group_key,
                                ctx.view_group_weight,
                            ));
                        }
                    }
                    pending if is_pending(pending) => {
                        if let Some(key) = content_key {
                            candidates.push(make_candidate(
                                node,
                                key.clone(),
                                PriorityGroup::Urgent,
                                0,
                                ctx.view_group_key,
                                ctx.view_group_weight,
                            ));
                        }
                    }
                    _ => {}
                }
            }
            push_children(children, stack);
        }
    }
}

/// Post-traversal preload pass: queue ancestors and/or siblings of selected
/// nodes at `Preload` priority.
fn preload_pass<H: SpatialHierarchy, B: LodEvaluator, M: Policy>(
    ctx: &mut TraversalContext<'_, H, B, M>,
    selected: &[NodeId],
    candidates: &mut Vec<LoadCandidate>,
) {
    let mut queued = HashSet::new();

    for &node in selected {
        if ctx.options.preload_ancestors {
            let mut current = node;
            while let Some(parent) = ctx.hierarchy.parent(current) {
                if !queued.insert(parent) {
                    break; // already visited this ancestor chain
                }
                let lifecycle = node_lifecycle(ctx, parent);
                if needs_load(lifecycle) {
                    if let Some(key) = ctx.hierarchy.content_key(parent).cloned() {
                        candidates.push(make_candidate(
                            parent,
                            key,
                            PriorityGroup::Preload,
                            i64::MAX,
                            ctx.view_group_key,
                            ctx.view_group_weight,
                        ));
                    }
                }
                current = parent;
            }
        }

        if ctx.options.preload_siblings {
            if let Some(parent) = ctx.hierarchy.parent(node) {
                for &sibling in ctx.hierarchy.children(parent) {
                    if sibling == node || !queued.insert(sibling) {
                        continue;
                    }
                    let lifecycle = node_lifecycle(ctx, sibling);
                    if needs_load(lifecycle) {
                        if let Some(key) = ctx.hierarchy.content_key(sibling).cloned() {
                            candidates.push(make_candidate(
                                sibling,
                                key,
                                PriorityGroup::Preload,
                                i64::MAX,
                                ctx.view_group_key,
                                ctx.view_group_weight,
                            ));
                        }
                    }
                }
            }
        }
    }
}
