use crate::node::NodeId;

/// Classifies what kind of resource failed to load.
///
/// Passed to the `on_load_error` callback so callers can distinguish
/// network errors from format errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadFailureType {
    /// The failure occurred while fetching node content.
    NodeContent,
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

/// A self-contained sub-scene attached to a reference node.
///
/// Returned as part of [`NodeContent`] when a node's content resolves to
/// another scene (e.g. an external `tileset.json` in 3D Tiles). The engine
/// attaches the sub-scene transparently — traversal crosses the boundary
/// without any format-specific knowledge in the engine.
pub struct SceneRef<C: Send + 'static> {
    /// The spatial graph describing the sub-scene's nodes.
    pub graph: Box<dyn crate::hierarchy::SceneGraph>,
    /// Loader responsible for fetching content for nodes in `graph`.
    pub loader: Box<dyn DynContentLoader<C>>,
    /// Approximate byte footprint of the reference descriptor itself.
    pub byte_size: usize,
}

/// Unified result of a [`ContentLoader::load`] call.
///
/// A node's content is always some combination of renderable geometry and/or
/// a sub-scene reference. There is no separate "reference" variant — the engine
/// handles both independently and in combination (Add-mode nodes can carry
/// geometry *and* introduce a sub-scene simultaneously).
pub struct NodeContent<C: Send + 'static> {
    /// Decoded renderable geometry for this node, if any.
    pub renderable: Option<C>,
    /// A sub-scene attached under this node, if any.
    pub reference: Option<SceneRef<C>>,
    /// Approximate byte footprint (geometry + reference descriptor combined).
    pub byte_size: usize,
}

impl<C: Send + 'static> NodeContent<C> {
    /// Node has geometry and no sub-scene reference.
    pub fn renderable(content: C, byte_size: usize) -> Self {
        Self {
            renderable: Some(content),
            reference: None,
            byte_size,
        }
    }

    /// Node is a structural pass-through with no geometry.
    pub fn empty() -> Self {
        Self {
            renderable: None,
            reference: None,
            byte_size: 0,
        }
    }

    /// Node's content is a sub-scene reference with no local geometry.
    pub fn scene_ref(reference: SceneRef<C>) -> Self {
        let byte_size = reference.byte_size;
        Self {
            renderable: None,
            reference: Some(reference),
            byte_size,
        }
    }
}

/// Object-safe erased version of [`ContentLoader<C>`].
///
/// Required so [`SceneRef`] can hold a `Box<dyn DynContentLoader<C>>` without a
/// second generic type parameter. Implement [`ContentLoader<C>`] instead — the
/// blanket impl below covers this trait automatically.
pub trait DynContentLoader<C: Send + 'static>: Send + Sync + 'static {
    fn load_dyn(
        &self,
        bg: &orkester::Context,
        main: &orkester::Context,
        node: NodeId,
        key: &ContentKey,
        parent_world_transform: glam::DMat4,
        cancel: orkester::CancellationToken,
    ) -> orkester::Task<Result<NodeContent<C>, Box<dyn std::error::Error + Send + Sync>>>;

    fn free_dyn(&self, content: C);
}

impl<C: Send + 'static, L: ContentLoader<C>> DynContentLoader<C> for L {
    fn load_dyn(
        &self,
        bg: &orkester::Context,
        main: &orkester::Context,
        node: NodeId,
        key: &ContentKey,
        parent_world_transform: glam::DMat4,
        cancel: orkester::CancellationToken,
    ) -> orkester::Task<Result<NodeContent<C>, Box<dyn std::error::Error + Send + Sync>>> {
        self.load(bg, main, node, key, parent_world_transform, cancel)
            .map(|r| r.map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) }))
    }

    fn free_dyn(&self, content: C) {
        ContentLoader::free(self, content);
    }
}

/// Async content loading contract. Implement this for your format.
///
/// The engine calls [`load`] when it decides to fetch a node, and cancels via
/// the supplied [`CancellationToken`](orkester::CancellationToken) when the
/// node is no longer needed.
///
/// # Return type
///
/// Always return [`NodeContent<C>`]:
/// - Geometry only: `Ok(NodeContent::renderable(mesh, bytes))`
/// - Sub-scene ref: `Ok(NodeContent::scene_ref(SceneRef { graph, loader, .. }))`
/// - Both (Add-mode): construct `NodeContent` with both fields `Some`
/// - Empty node: `Ok(NodeContent::empty())`
///
/// The engine handles sub-scene attachment transparently when
/// `NodeContent::reference` is `Some`.
pub trait ContentLoader<C: Send + 'static>: Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Begin loading content for `node` at the given `key`.
    ///
    /// `bg` dispatches CPU-side work; `main` dispatches GPU-upload work that
    /// must run on the caller's thread. `cancel` is signalled when the load is
    /// no longer needed.
    fn load(
        &self,
        bg: &orkester::Context,
        main: &orkester::Context,
        node: NodeId,
        key: &ContentKey,
        parent_world_transform: glam::DMat4,
        cancel: orkester::CancellationToken,
    ) -> orkester::Task<Result<NodeContent<C>, Self::Error>>;

    /// Release content when a node is evicted from the cache.
    ///
    /// Override to release GPU resources, unbind textures, etc. Default: drops normally.
    fn free(&self, _content: C) {}
}
