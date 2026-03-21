use crate::node::NodeId;
use orkester::AsyncSystem;
use zukei::math::Mat4;

/// Opaque handle assigned to loaded content by the engine.
pub type ContentHandle = u64;
/// Opaque identifier for an in-flight load request.
pub type RequestId = u64;

/// Stable content address for a node (URI, key, or other format-defined identifier).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ContentKey(pub String);

/// Load scheduling tier. Determines which candidates are popped from the queue first.
/// Processing order: Urgent → Normal → Preload.
#[repr(C)]
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
/// Inspired by Cesium3DTilesSelection three-tier approach.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoadPriority {
    /// Scheduling tier.
    pub group: PriorityGroup,
    /// Within-group score: lower value = higher priority (e.g., max SSE across views).
    pub score: i64,
    /// View-group weight for cross-group fairness at the same tier and score.
    pub view_group_weight: u16,
}

/// A node candidate submitted to `LoadScheduler`.
#[derive(Clone, Debug)]
pub struct LoadCandidate {
    /// Stable scheduler identity for the originating view group.
    pub view_group: u64,
    pub node_id: NodeId,
    pub key: ContentKey,
    pub priority: LoadPriority,
}

/// Results from a single `load()` pass.
#[derive(Clone, Debug, Default)]
pub struct LoadPassResult {
    pub started_requests: usize,
    pub completed_main_thread_tasks: usize,
    pub pending_worker_queue: usize,
    pub pending_main_queue: usize,
}

/// Raw content returned by `ContentLoader`. Not yet GPU-prepared.
/// The engine wraps this into `Content<C>` once a `ContentHandle` is assigned.
#[derive(Clone, Debug)]
pub struct LoadedContent<C> {
    pub payload: Payload<C>,
    pub byte_size: usize,
}

/// Payload variant: what kind of data was decoded.
#[derive(Clone, Debug)]
pub enum Payload<C> {
    /// Decoded renderable content (mesh, point cloud, etc.).
    Renderable(C),
    /// This node is a reference to an external hierarchy.
    Reference(HierarchyReference),
    /// No content; node is a structural pass-through.
    Empty,
}

/// Reference from a node to an external child hierarchy.
#[derive(Clone, Debug, PartialEq)]
pub struct HierarchyReference {
    pub key: ContentKey,
    pub source: NodeId,
    pub transform: Option<Mat4>,
}

/// Async content loading contract.
///
/// Implementations issue HTTP/file/cache requests and return futures. The engine
/// calls `cancel` when a request is no longer needed (e.g., node evicted or view changed).
pub trait ContentLoader<C: Send + 'static>: Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Begin loading content for `node_id` at the given key and priority.
    /// Returns an opaque `RequestId` for cancellation and a future that resolves to the content.
    fn request(
        &self,
        async_system: &AsyncSystem,
        node_id: NodeId,
        key: &ContentKey,
        priority: LoadPriority,
    ) -> (
        RequestId,
        orkester::Future<Result<LoadedContent<C>, Self::Error>>,
    );

    /// Cancel an in-flight request. Returns `true` if the request was still pending.
    fn cancel(&self, request_id: RequestId) -> bool;
}

impl<C, E: std::error::Error + Send + Sync + 'static> ContentLoader<C>
    for Box<dyn ContentLoader<C, Error = E>>
where
    C: Send + 'static,
{
    type Error = E;

    fn request(
        &self,
        async_system: &AsyncSystem,
        node_id: NodeId,
        key: &ContentKey,
        priority: LoadPriority,
    ) -> (RequestId, orkester::Future<Result<LoadedContent<C>, E>>) {
        (**self).request(async_system, node_id, key, priority)
    }

    fn cancel(&self, request_id: RequestId) -> bool {
        (**self).cancel(request_id)
    }
}
