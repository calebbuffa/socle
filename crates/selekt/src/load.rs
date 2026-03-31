use crate::node::NodeId;
use glam::DMat4 as Mat4;

/// Classifies what kind of resource failed to load.
///
/// Passed to the `on_load_error` callback so callers can distinguish
/// network errors from format errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadFailureType {
    /// The failure occurred while fetching node content (the typical case).
    NodeContent,
    /// The failure occurred while resolving an external hierarchy reference.
    HierarchyReference,
    /// Unknown / unclassified failure.
    Unknown,
}

/// Detailed information passed to the `on_load_error` callback.
#[derive(Clone, Debug)]
pub struct LoadFailureDetails {
    /// Which node encountered the failure.
    pub node_id: NodeId,
    /// What kind of resource was being loaded.
    pub failure_type: LoadFailureType,
    /// HTTP status code if the failure was an HTTP error (e.g. 404, 503).
    /// `None` for non-HTTP failures (e.g. parse errors, I/O errors).
    pub http_status_code: Option<u16>,
    /// Human-readable description of the error.
    pub message: String,
}

/// Stable content address for a node (URI, key, or other format-defined identifier).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ContentKey(pub String);

/// Load scheduling tier. Determines which candidates are popped from the queue first.
/// Processing order: Urgent → Normal → Preload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PriorityGroup {
    /// Speculative: siblings of culled nodes, pre-loaded for smooth panning.
    Preload = 0,
    /// Normal: nodes required for current-frame LOD.
    Normal = 1,
    /// Urgent: nodes whose absence causes kicked ancestors (visible detail loss).
    Urgent = 2,
}

/// Full load priority for a candidate. Ordering: group tier first, then score within tier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LoadPriority {
    /// Scheduling tier.
    pub group: PriorityGroup,
    /// Within-group score: lower value = higher priority (e.g., max SSE across views).
    pub score: i64,
    /// View-group weight for cross-group fairness at the same tier and score.
    pub view_group_weight: u16,
}

/// A node candidate submitted to the internal load scheduler.
///
/// Does not carry the content key — the engine looks it up from the
/// hierarchy at dispatch time, avoiding per-candidate `String` allocation.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LoadCandidate {
    pub(crate) view_group: u64,
    pub(crate) node_id: NodeId,
    pub(crate) priority: LoadPriority,
}

/// Results from a single `load()` pass.
#[derive(Clone, Debug, Default)]
pub struct LoadPassResult {
    pub started_requests: usize,
    pub completed_main_thread_tasks: usize,
    pub pending_worker_queue: usize,
    /// Nodes that transitioned to `Renderable` during this pass.
    pub nodes_newly_renderable: usize,
}

/// Intermediate value flowing from the worker-thread decode phase to the
/// main-thread GPU-upload phase.
///
/// Produced inside [`ContentLoader::load`] implementations that have a
/// two-phase worker→main pipeline (e.g. CPU-decode then GPU-upload). Using
/// a shared enum avoids re-defining the three-case pattern in every format crate.
pub enum DecodeOutput<W: Send + 'static> {
    /// Decoded node data ready for main-thread GPU upload.
    Decoded { result: W, byte_size: usize },
    /// Pointer to an external child hierarchy — no GPU work needed.
    Reference(HierarchyReference, usize),
    /// Node carries no renderable content.
    Empty,
}

/// Reference from a node to an external child hierarchy.
#[derive(Clone, Debug, PartialEq)]
pub struct HierarchyReference {
    pub key: ContentKey,
    pub source: NodeId,
    pub transform: Option<Mat4>,
}

/// Result of a completed [`ContentLoader::load`] call.
///
/// The two variants reflect the two outcomes the engine must handle differently:
/// - [`Content`](LoadResult::Content): decoded (or empty) node data ready for rendering.
/// - [`Reference`](LoadResult::Reference): pointer to an external child hierarchy.
///
/// `byte_size` tracks the in-memory footprint for LRU eviction accounting.
pub enum LoadResult<C> {
    /// Decoded renderable data for this node.
    ///
    /// `content` is `None` when the node exists in the hierarchy but carries no
    /// renderable geometry (structural nodes, empty tiles, etc.).
    Content {
        content: Option<C>,
        /// Approximate byte footprint of `content` (used for memory-budget tracking).
        byte_size: usize,
    },
    /// This node's content file turned out to be a pointer to a child hierarchy.
    ///
    /// The engine will use the configured [`HierarchyResolver`](crate::hierarchy::HierarchyResolver)
    /// to fetch and expand the referenced hierarchy.
    Reference {
        reference: HierarchyReference,
        /// Byte size of the reference descriptor itself (usually small).
        byte_size: usize,
    },
}

// Internal types (used by engine, not part of ContentLoader trait)

/// Internal wrapper carrying both the result and the cancellation token.
/// The cancellation token is threaded through so eviction can cancel mid-flight.
pub(crate) struct LoadedContent<C> {
    pub result: LoadResult<C>,
}

/// Async content loading contract.
///
/// Implement this for your format. The engine calls [`load`](ContentLoader::load)
/// when it decides to fetch a node and cancels via the supplied
/// [`CancellationToken`](orkester::CancellationToken) when the node is no longer needed.
///
/// # Required method
///
/// ```rust,ignore
/// fn load(
///     &self,
///     bg: &Context,
///     main: &Context,
///     node: NodeId,
///     key: &ContentKey,
///     cancel: CancellationToken,
/// ) -> Task<Result<Option<C>, Self::Error>>;
/// ```
///
/// Return `Ok(LoadResult::Content { .. })` when decoded, `Ok(LoadResult::Reference { .. })`\n/// for nodes that reference a child hierarchy, or `Err(e)` on failure (triggers retry).
///
/// # Optional method
///
/// Override [`free`](ContentLoader::free) to release GPU or other external
/// resources when a node is evicted. The default implementation drops `content`.
pub trait ContentLoader<C: Send + 'static>: Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Begin loading content for `node_id` at the given key.
    ///
    /// `bg` dispatches CPU-side work (decoding); `main` dispatches GPU-upload
    /// work that must run on the caller's thread. `cancel` is signalled by the
    /// engine when the load is no longer needed — check it in long-running
    /// async chains.
    ///
    /// Returns `Ok(LoadResult::Content { .. })` on success, `Ok(LoadResult::Reference { .. })`
    /// for nodes that reference a child hierarchy, or `Err(e)` on failure.
    fn load(
        &self,
        bg: &orkester::Context,
        main: &orkester::Context,
        node: NodeId,
        key: &ContentKey,
        cancel: orkester::CancellationToken,
    ) -> orkester::Task<Result<LoadResult<C>, Self::Error>>;

    /// Release content when a node is evicted from the cache.
    ///
    /// Called on the main thread before the content is dropped. Override to
    /// release GPU resources, unbind textures, etc. Default: drops normally.
    fn free(&self, _content: C) {}
}
