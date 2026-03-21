/// Opaque stable identifier for a node in the spatial hierarchy.
pub type NodeId = u64;

/// Structural classification of a node in the hierarchy.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// Has renderable content (mesh, point cloud, etc.).
    Renderable,
    /// Interior pass-through: no content, exists only to structure the hierarchy.
    Empty,
    /// Links to an external child hierarchy (triggers `HierarchyResolver`).
    Reference,
    /// Root of a composite multi-layer structure (e.g., I3S building sublayers).
    CompositeRoot,
}

/// Node lifecycle state machine.
///
/// Allowed transitions:
/// ```text
/// Unloaded ──► Queued ──► Loading ──► Renderable
///                │            │
///                └────────────┴──► RetryScheduled ──► Queued
///                                 (transient failure)
///
/// Any state ──► Failed   (permanent failure, no retry)
/// Any state ──► Evicted  (memory pressure)
/// ```
///
/// Note: GPU preparation (`Prepared` → `Renderable`) is handled by the
/// `belag` crate, not by selekt. Content becomes `Renderable` as soon as
/// the content loader delivers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeLifecycleState {
    /// Not yet queued for load.
    Unloaded,
    /// In the load scheduler queue; not yet dispatched.
    Queued,
    /// Async content load in progress on a worker thread.
    Loading,
    /// Content loaded and resident. Ready for rendering or further processing.
    Renderable,
    /// Transient load failure (e.g., HTTP 503). Will be re-queued after backoff.
    RetryScheduled,
    /// Permanent failure (malformed data, 404, etc.). Will not be retried.
    Failed,
    /// Evicted from the resident cache to free memory.
    Evicted,
}

/// Per-node internal tracking state (not part of the public API).
#[derive(Clone, Debug)]
pub(crate) struct NodeState {
    pub lifecycle: NodeLifecycleState,
    /// Number of load attempts so far.
    pub retry_count: u8,
    /// Frame on which to attempt the next retry (for backoff).
    pub next_retry_frame: u64,
    /// Whether this node was refined (replaced by children) last frame.
    /// Used to enforce continuity: if true and children still not Renderable, continue refining.
    pub was_refined_last_frame: bool,
    /// In-flight load request id (while lifecycle == Loading).
    pub request_id: Option<crate::load::RequestId>,
}

impl NodeState {
    pub fn new() -> Self {
        Self {
            lifecycle: NodeLifecycleState::Unloaded,
            retry_count: 0,
            next_retry_frame: 0,
            was_refined_last_frame: false,
            request_id: None,
        }
    }
}

impl Default for NodeState {
    fn default() -> Self {
        Self::new()
    }
}
