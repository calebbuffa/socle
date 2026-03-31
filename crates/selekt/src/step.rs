//! Pure step-function for the state-machine core of [`SelectionEngine`].
//!
//! # Types (Phase B)
//!
//! - [`StepInput`] — what the caller provides each frame: view updates and
//!   completed async loads.
//! - [`StepOutput`] — what the engine requests: loads to start, cancellations,
//!   and evicted content that must be freed.
//! - [`LoadRequest`] — a single pending load the caller must dispatch.
//! - [`CompletedLoad`] — a finished async result the caller feeds back.
//!
//! # Function (Phase C)
//!
//! [`step`] is the pure synchronous heart of the selection engine.  It takes
//! [`EngineState`](crate::engine_state::EngineState) plus all plug-in trait
//! references, processes one frame, and returns [`StepOutput`].  It contains
//! no async handles, no thread contexts, and no persistent scheduler state.
//!
//! All types are `pub(crate)` until an external use-case emerges.

use std::time::Duration;

use crate::engine_state::EngineState;
use crate::hierarchy::SpatialHierarchy;
use crate::load::{
    ContentKey, LoadFailureDetails, LoadFailureType, LoadPriority, LoadResult,
};
use crate::lod::LodEvaluator;
use crate::node::{NodeId, NodeLoadState};
use crate::options::SelectionOptions;
use crate::policy::{NodeExcluder, OcclusionTester, ResidencyPolicy, VisibilityPolicy};
use crate::traversal::traverse;
use crate::view::{ViewGroupHandle, ViewState};


/// Per-frame inputs fed into [`step`].
///
/// The caller (i.e. the `SelectionEngine` shell) is responsible for:
/// 1. Updating `view_groups` with current camera state every frame.
/// 2. Polling in-flight tasks, resolving any [`LoadResult::Reference`] items
///    (by calling `hierarchy.expand()` and synthesising a content completion),
///    and delivering all outcomes via `completed`.
pub(crate) struct StepInput<'views, C> {
    /// One entry per active view group: its handle and the current camera array.
    ///
    /// Groups are visited in order; each receives fair scheduler weight.
    pub view_groups: &'views [(ViewGroupHandle, &'views [ViewState])],

    /// Loads and resolve outcomes that finished since the last [`step`] call.
    ///
    /// Order does not matter — all entries are processed before traversal.
    /// Reference results must already be resolved by the caller; only
    /// [`LoadResult::Content`] variants should appear here.
    pub completed: Vec<CompletedLoad<C>>,
}

/// A completed async load result to be ingested by [`step`].
pub(crate) struct CompletedLoad<C> {
    /// Which node this result belongs to.
    pub node_id: NodeId,
    /// The decoded payload, or an error string if loading or resolution failed.
    pub result: Result<LoadResult<C>, String>,
}


/// Per-frame outputs produced by [`step`].
///
/// The caller (i.e. the `SelectionEngine` shell) is responsible for:
/// 1. Starting one async task per entry in `requests_to_start`.
/// 2. Cancelling in-flight tasks for every `NodeId` in `requests_to_cancel`
///    (call `CancellationToken::cancel()` and drop the task; also remove any
///    pending resolve future for that node).
/// 3. Calling `ContentLoader::free` for each item in `evicted_content`.
/// 4. Feeding task results back on the next [`step`] call via
///    [`StepInput::completed`].
pub(crate) struct StepOutput<C> {
    /// Loads the caller must start, in priority order (highest first).
    pub requests_to_start: Vec<LoadRequest>,

    /// Node IDs whose in-flight loads or resolve futures must be cancelled.
    pub requests_to_cancel: Vec<NodeId>,

    /// Decoded content evicted from the resident store this frame.
    ///
    /// The caller must call `ContentLoader::free` for each item to release
    /// GPU or other format-owned resources.
    pub evicted_content: Vec<C>,
}

/// A single load the caller must dispatch asynchronously.
pub(crate) struct LoadRequest {
    /// The node to load.
    pub node_id: NodeId,
    /// The primary content key (URL/URI) to fetch.
    pub key: ContentKey,
    /// Scheduling priority (for caller-side ordering, if desired).
    pub priority: LoadPriority,
}


/// Pure synchronous frame step.
///
/// Processes one frame of spatial selection:
/// 1. Advances the frame counter and expires age-limited content.
/// 2. Ingests load completions from `input.completed`.
/// 3. Traverses every view group in `input.view_groups`, writing frame results
///    back into `state.view_groups`.
/// 4. Drains a freshly-created local scheduler to produce `requests_to_start`.
/// 5. Evicts nodes that exceed the memory budget.
/// 6. Adjusts the LOD threshold for next frame.
///
/// # Caller contract
///
/// - All `LoadResult::Reference` items from completed loads must be resolved
///   (via `hierarchy.expand`) **before** being delivered here as
///   `LoadResult::Content { content: None, byte_size }`.
/// - The caller owns all async tokens; it must react to `StepOutput::requests_to_cancel`
///   by cancelling the associated tasks.
/// - The caller must call `ContentLoader::free` for each item in
///   `StepOutput::evicted_content`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn step<C: Send + 'static, H: SpatialHierarchy, L: LodEvaluator>(
    state: &mut EngineState<C>,
    hierarchy: &H,
    lod: &L,
    visibility: &dyn VisibilityPolicy,
    residency: &dyn ResidencyPolicy,
    excluders: &[Box<dyn NodeExcluder>],
    occlusion_tester: &dyn OcclusionTester,
    options: &SelectionOptions,
    on_load_error: Option<&dyn Fn(&LoadFailureDetails)>,
    input: StepInput<'_, C>,
) -> StepOutput<C> {
    // 1. Advance frame counter
    state.frame_index = state.frame_index.wrapping_add(1);

    // 2. Expire age-limited nodes
    let mut output = StepOutput {
        requests_to_start: Vec::new(),
        requests_to_cancel: Vec::new(),
        evicted_content: Vec::new(),
    };
    expire_stale(state, hierarchy, &mut output);

    // 3. Ingest completed loads
    for item in input.completed {
        ingest_completion(state, options, on_load_error, item);
    }

    // 4. Traverse all view groups
    state.scheduler.tick(state.frame_index);
    state.scheduler.clear();

    for &(handle, views) in input.view_groups {
        if views.is_empty() {
            continue;
        }
        let Some(slot) = state.view_groups.get(handle) else {
            continue;
        };
        let view_group_key = view_group_key(handle);
        let view_group_weight =
            (slot.weight * f64::from(u16::MAX)).clamp(1.0, f64::from(u16::MAX)) as u16;

        let (camera_stationary_seconds, camera_velocity) =
            state.view_groups.get_mut(handle).unwrap().tick_camera(views);

        state.traversal_buffers.clear(views.len());

        let stats = traverse(
            hierarchy,
            lod,
            visibility,
            excluders,
            occlusion_tester,
            &mut state.node_states,
            views,
            options,
            state.frame_index,
            view_group_key,
            view_group_weight,
            state.lod_threshold.current,
            camera_stationary_seconds,
            camera_velocity,
            &mut state.traversal_buffers,
        );

        // Enqueue candidates into the local scheduler.
        let camera_speed = camera_velocity.length();
        let cull_moving =
            options.streaming.cull_requests_while_moving && camera_speed > 0.0;
        let cull_multiplier = options.streaming.cull_requests_while_moving_multiplier;

        for candidate in &state.traversal_buffers.candidates {
            let lifecycle = state.node_states.get(candidate.node_id).lifecycle;
            if matches!(
                lifecycle,
                NodeLoadState::Loading | NodeLoadState::Renderable | NodeLoadState::Failed
            ) {
                continue;
            }
            if cull_moving && candidate.priority.group != crate::load::PriorityGroup::Urgent {
                let geometric_error = hierarchy.lod_descriptor(candidate.node_id).value;
                if camera_speed > geometric_error * cull_multiplier {
                    continue;
                }
            }
            state.scheduler.push(*candidate);
        }

        // Write frame results back into the view group slot.
        let selected = std::mem::take(&mut state.traversal_buffers.selected);
        let fading_out = std::mem::take(&mut state.traversal_buffers.fading_out);

        let slot = state.view_groups.get_mut(handle).unwrap();
        slot.result.nodes_to_render.clear();
        slot.result.nodes_fading_out.clear();
        slot.result.nodes_to_render.extend_from_slice(&selected);
        slot.result.nodes_fading_out.extend_from_slice(&fading_out);
        slot.result.nodes_visited = stats.visited;
        slot.result.nodes_culled = stats.culled;
        slot.result.nodes_occluded = stats.occluded;
        slot.result.nodes_kicked = stats.kicked;
        slot.result.frame_number = state.frame_index;
        slot.result.bytes_resident = state.resident.total_bytes;

        state.traversal_buffers.selected = selected;
        state.traversal_buffers.fading_out = fading_out;
    }

    //  5. Drain scheduler -> requests_to_start
    let max_loads = options.loading.max_simultaneous_loads;
    while output.requests_to_start.len() < max_loads {
        let Some(candidate) = state.scheduler.pop() else {
            break;
        };
        let lifecycle = state.node_states.get(candidate.node_id).lifecycle;
        if matches!(
            lifecycle,
            NodeLoadState::Loading | NodeLoadState::Renderable | NodeLoadState::Failed
        ) {
            continue;
        }
        let Some(key) = hierarchy.content_keys(candidate.node_id).first() else {
            continue;
        };
        // Mark Loading now — the caller is committed to starting this task.
        state.node_states.get_mut(candidate.node_id).lifecycle = NodeLoadState::Loading;
        output.requests_to_start.push(LoadRequest {
            node_id: candidate.node_id,
            key: key.clone(),
            priority: candidate.priority,
        });
    }

    // 6. Evict if over budget
    evict_if_needed(state, residency, options, &mut output);

    // 7. Adjust LOD threshold
    state.lod_threshold.adjust(
        state.resident.total_bytes,
        options.loading.max_cached_bytes,
    );

    output
}


fn view_group_key(handle: ViewGroupHandle) -> u64 {
    (u64::from(handle.generation) << 32) | u64::from(handle.index)
}

fn ingest_completion<C: Send + 'static>(
    state: &mut EngineState<C>,
    options: &SelectionOptions,
    on_load_error: Option<&dyn Fn(&LoadFailureDetails)>,
    item: CompletedLoad<C>,
) {
    let node_id = item.node_id;
    match item.result {
        Ok(LoadResult::Content { content, byte_size }) => {
            state.resident.insert(node_id, content, byte_size);
            mark_renderable(state, node_id);
        }
        Ok(LoadResult::Reference { .. }) => {
            // The shell must resolve references before delivering completions.
            // Treat as an unexpected no-content success — mark renderable with zero size.
            state.resident.insert(node_id, None, 0);
            mark_renderable(state, node_id);
        }
        Err(msg) => {
            handle_failure(state, options, on_load_error, node_id, LoadFailureType::NodeContent, &msg);
        }
    }
}

fn mark_renderable<C: Send + 'static>(state: &mut EngineState<C>, node_id: NodeId) {
    let s = state.node_states.get_mut(node_id);
    s.lifecycle = NodeLoadState::Renderable;
    s.retry_count = 0;
    s.next_retry_frame = 0;
    let secs = state.load_epoch.elapsed().as_secs();
    s.loaded_epoch_secs = (secs.min(u32::MAX as u64) as u32).max(1);
}

fn handle_failure<C: Send + 'static>(
    state: &mut EngineState<C>,
    options: &SelectionOptions,
    on_load_error: Option<&dyn Fn(&LoadFailureDetails)>,
    node_id: NodeId,
    failure_type: LoadFailureType,
    error_msg: &str,
) {
    fn extract_http_status(msg: &str) -> Option<u16> {
        msg.strip_prefix("HTTP error ").and_then(|s| s.parse().ok())
    }
    let http_status = extract_http_status(error_msg);
    if let Some(cb) = on_load_error {
        cb(&LoadFailureDetails {
            node_id,
            failure_type,
            http_status_code: http_status,
            message: error_msg.to_owned(),
        });
    }
    let is_permanent = matches!(http_status, Some(s) if s >= 400 && s < 500 && s != 429);
    let s = state.node_states.get_mut(node_id);
    if is_permanent {
        s.lifecycle = NodeLoadState::Failed;
    } else {
        s.retry_count = s.retry_count.saturating_add(1);
        if s.retry_count >= options.loading.retry_limit {
            s.lifecycle = NodeLoadState::Failed;
        } else {
            s.lifecycle = NodeLoadState::RetryScheduled;
            s.next_retry_frame = state
                .frame_index
                .saturating_add(u64::from(options.loading.retry_backoff_frames));
        }
    }
}

fn expire_stale<C: Send + 'static, H: SpatialHierarchy>(
    state: &mut EngineState<C>,
    hierarchy: &H,
    output: &mut StepOutput<C>,
) {
    let now_secs = state.load_epoch.elapsed().as_secs() as u32;
    let expired: Vec<NodeId> = state.resident.map.keys().copied().filter(|&id| {
        if let Some(max_age) = hierarchy.content_max_age(id) {
            let loaded = state.node_states.get(id).loaded_epoch_secs;
            loaded != 0
                && Duration::from_secs(now_secs.saturating_sub(loaded) as u64) >= max_age
        } else {
            false
        }
    }).collect();
    apply_evictions(state, &expired, output);
}

fn evict_if_needed<C: Send + 'static>(
    state: &mut EngineState<C>,
    residency: &dyn ResidencyPolicy,
    options: &SelectionOptions,
    output: &mut StepOutput<C>,
) {
    let budget = options.loading.max_cached_bytes;
    if state.resident.total_bytes <= budget {
        return;
    }
    let mut resident_nodes: Vec<(NodeId, usize)> = state
        .resident
        .map
        .iter()
        .map(|(&id, r)| (id, r.byte_size))
        .collect();
    resident_nodes.sort_unstable_by(|a, b| {
        let ia = state.node_states.get(a.0).importance;
        let ib = state.node_states.get(b.0).importance;
        ia.partial_cmp(&ib).unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut to_evict = Vec::new();
    residency.select_evictions(&resident_nodes, budget, &mut to_evict);
    apply_evictions(state, &to_evict, output);
}

fn apply_evictions<C: Send + 'static>(
    state: &mut EngineState<C>,
    to_evict: &[NodeId],
    output: &mut StepOutput<C>,
) {
    for &node_id in to_evict {
        if let Some(resident) = state.resident.remove(node_id) {
            if let Some(content) = resident.content {
                output.evicted_content.push(content);
            }
        }
        // The node may be in-flight — tell the caller to cancel it.
        let lifecycle = state.node_states.get(node_id).lifecycle;
        if lifecycle == NodeLoadState::Loading {
            output.requests_to_cancel.push(node_id);
        }
        state.node_states.get_mut(node_id).lifecycle = NodeLoadState::Evicted;
    }
}
