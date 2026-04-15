//! Per-node selection state tracked across frames.
//!
//! [`SelectionState`] is owned by the orchestrator (kiban) and passed to
//! `selekt::select()` each frame. It tracks:
//!
//! - Load lifecycle for each node (Unloaded → Queued → Loading → Renderable)
//! - Previous-frame selection result (for kicking and fading decisions)
//! - Importance scores (for eviction ordering)
//!
//! The state grows on demand — nodes not yet seen default to `Unloaded`.

use crate::node::{NodeId, NodeLoadState, NodeRefinementResult};

/// Per-node internal tracking state.
#[derive(Clone, Debug)]
pub struct NodeStatus {
    pub lifecycle: NodeLoadState,
    /// Number of load attempts so far.
    pub retry_count: u8,
    /// Frame on which to attempt the next retry (for backoff).
    pub next_retry_frame: u64,
    /// Selection outcome from the previous frame.
    pub last_result: NodeRefinementResult,
    /// Importance score from the last traversal (higher = keep longer).
    pub importance: f32,
    /// Seconds since load epoch at which this node became Renderable.
    pub loaded_epoch_secs: u32,
    /// Frame index when this node first entered the `fading_in` list.
    /// `0` means the node is not currently fading in.
    pub fade_in_frame: u64,
}

impl NodeStatus {
    pub const DEFAULT: NodeStatus = NodeStatus {
        lifecycle: NodeLoadState::Unloaded,
        retry_count: 0,
        next_retry_frame: 0,
        last_result: NodeRefinementResult::None,
        importance: 0.0,
        loaded_epoch_secs: 0,
        fade_in_frame: 0,
    };

    pub fn new() -> Self {
        Self::DEFAULT
    }
}

impl Default for NodeStatus {
    fn default() -> Self {
        Self::new()
    }
}

/// Dense per-node selection state, indexed by `NodeId`.
///
/// O(1) access with no hashing. Grows on demand.
pub struct SelectionState {
    statuses: Vec<NodeStatus>,
    /// Monotonically increasing frame counter.
    pub frame_index: u64,
}

impl SelectionState {
    pub fn new() -> Self {
        Self {
            statuses: Vec::new(),
            frame_index: 0,
        }
    }

    /// O(1) read. Returns the static default for never-seen nodes.
    #[inline(always)]
    pub fn get(&self, id: NodeId) -> &NodeStatus {
        self.statuses
            .get(id.index())
            .unwrap_or(&NodeStatus::DEFAULT)
    }

    /// O(1) write. Grows the backing vec if needed.
    #[inline(always)]
    pub fn get_mut(&mut self, id: NodeId) -> &mut NodeStatus {
        let idx = id.index();
        if idx >= self.statuses.len() {
            self.statuses.resize_with(idx + 1, NodeStatus::new);
        }
        &mut self.statuses[idx]
    }

    /// Mark a node as renderable (content loaded).
    pub fn mark_renderable(&mut self, id: NodeId) {
        self.get_mut(id).lifecycle = NodeLoadState::Renderable;
    }

    /// Mark a node as loading.
    pub fn mark_loading(&mut self, id: NodeId) {
        self.get_mut(id).lifecycle = NodeLoadState::Loading;
    }

    /// Mark a node as queued.
    pub fn mark_queued(&mut self, id: NodeId) {
        self.get_mut(id).lifecycle = NodeLoadState::Queued;
    }

    /// Mark a node as evicted.
    pub fn mark_evicted(&mut self, id: NodeId) {
        self.get_mut(id).lifecycle = NodeLoadState::Evicted;
    }

    /// Mark a node as failed.
    pub fn mark_failed(&mut self, id: NodeId) {
        self.get_mut(id).lifecycle = NodeLoadState::Failed;
    }

    /// Schedule a retry for a node.
    pub fn mark_retry(&mut self, id: NodeId, retry_frame: u64) {
        let status = self.get_mut(id);
        status.lifecycle = NodeLoadState::RetryScheduled;
        status.retry_count += 1;
        status.next_retry_frame = retry_frame;
    }

    /// Iterate over all tracked statuses.
    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &NodeStatus)> {
        self.statuses
            .iter()
            .enumerate()
            .map(|(i, s)| (NodeId::from_index(i), s))
    }

    /// Increment frame counter.
    pub fn advance_frame(&mut self) {
        self.frame_index += 1;
    }
}

impl Default for SelectionState {
    fn default() -> Self {
        Self::new()
    }
}
