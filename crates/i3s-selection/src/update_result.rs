//! Per-frame output from the selection algorithm.

/// Priority group for a node load request. Higher groups are loaded first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LoadPriority {
    /// Background preload (ancestors, siblings). Lowest priority.
    Preload = 0,
    /// Node is currently selected for rendering. Normal priority.
    Normal = 1,
    /// Node is being rendered as a fallback while its replacement loads.
    /// Highest priority — user is staring at stale data.
    Urgent = 2,
}

/// A request to load a node's content, with priority information.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadRequest {
    /// Node to load.
    pub node_id: u32,
    /// Priority group (Urgent > Normal > Preload).
    pub priority: LoadPriority,
    /// Projected screen size — larger values should load first within a group.
    pub screen_size: f64,
}

/// Traversal statistics for diagnostics and profiling.
#[derive(Debug, Clone, Default)]
pub struct TraversalStats {
    /// Nodes visited during traversal (including culled).
    pub tiles_visited: u32,
    /// Nodes fully culled (frustum or fog).
    pub tiles_culled: u32,
    /// Nodes "kicked" because the loading descendant limit was exceeded.
    pub tiles_kicked: u32,
    /// Maximum tree depth reached during traversal.
    pub max_depth_visited: u32,
}

/// Result of a per-frame view update — which nodes to render, load, and unload.
///
/// I3S uses **node-switching**: parent and children are never shown simultaneously.
/// A node is either rendered at its current LOD, or replaced entirely by its children.
#[derive(Debug, Clone, Default)]
pub struct ViewUpdateResult {
    /// Node IDs whose content is loaded and selected for rendering this frame.
    pub nodes_to_render: Vec<u32>,
    /// Nodes that need their content fetched, with priority info.
    pub load_requests: Vec<LoadRequest>,
    /// Node IDs that are no longer visible and can be evicted from memory.
    pub nodes_to_unload: Vec<u32>,
    /// Node page IDs that need to be fetched before the next frame.
    /// Traversal encountered child node IDs on pages that haven't been loaded yet.
    pub pages_needed: Vec<u32>,
    /// Traversal statistics for this frame.
    pub stats: TraversalStats,
    /// Monotonically incrementing frame counter. Useful for detecting stale results.
    pub frame_number: u64,
    /// Number of node content loads currently executing on worker threads.
    pub worker_thread_load_queue_length: u32,
    /// Number of node content loads queued but not yet dispatched.
    pub main_thread_load_queue_length: u32,
}
