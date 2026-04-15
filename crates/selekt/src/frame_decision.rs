//! Output of the selection algorithm.
//!
//! [`FrameDecision`] is the complete result of one `select()` call: which
//! nodes to render, which to load, which to cancel, and transition state.

use crate::load::{ContentKey, LoadPriority};
use crate::node::NodeId;
use crate::node_store::NodeDescriptor;

/// A request to load content for a node.
#[derive(Clone, Debug)]
pub struct LoadRequest {
    pub node_id: NodeId,
    pub key: ContentKey,
    pub priority: LoadPriority,
}

/// Complete output of one selection pass.
#[derive(Clone, Debug, Default)]
pub struct FrameDecision {
    /// Nodes whose content should be rendered this frame.
    pub render: Vec<NodeId>,

    /// Per-view render lists (index corresponds to `views` input).
    pub per_view_render: Vec<Vec<NodeId>>,

    /// Content load requests, ordered by priority (highest first).
    pub load: Vec<LoadRequest>,

    /// Nodes fading into the render set (rendered this frame, not last).
    pub fading_in: Vec<NodeId>,

    /// Nodes fading out of the render set (rendered last frame, not this).
    pub fading_out: Vec<NodeId>,

    /// Statistics.
    pub nodes_visited: usize,
    pub nodes_culled: usize,
    pub nodes_occluded: usize,
    pub nodes_kicked: usize,
}

impl FrameDecision {
    /// Empty default constant.
    pub const EMPTY: FrameDecision = FrameDecision {
        render: Vec::new(),
        per_view_render: Vec::new(),
        load: Vec::new(),
        fading_in: Vec::new(),
        fading_out: Vec::new(),
        nodes_visited: 0,
        nodes_culled: 0,
        nodes_occluded: 0,
        nodes_kicked: 0,
    };
}

/// Static empty frame decision for use as a borrowed fallback.
pub static EMPTY_FRAME_DECISION: FrameDecision = FrameDecision::EMPTY;

/// Result of expanding latent children for a node.
#[derive(Clone, Debug)]
pub enum ExpandResult {
    /// Node has no latent children (explicit tileset).
    None,
    /// Children can't be created yet (need content loaded first, e.g. quantized mesh).
    RetryLater,
    /// Generated children. Each descriptor's `child_indices` are relative to this Vec.
    Children(Vec<NodeDescriptor>),
}
