/// Opaque stable identifier for a node in the spatial hierarchy.
///
/// Internally a 1-based [`NonZeroU64`](std::num::NonZeroU64), so `Option<NodeId>`
/// is the same size as `u64` — no overhead for optional node references.
///
/// Use [`NodeId::from_index`] to construct from a 0-based array position,
/// and [`NodeId::index`] to recover that position for direct `Vec` indexing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct NodeId(pub std::num::NonZeroU64);

impl NodeId {
    /// Construct a `NodeId` from a 0-based array index.
    ///
    /// Panics if `idx` would overflow `u64::MAX`.
    #[inline]
    pub fn from_index(idx: usize) -> Self {
        Self(
            std::num::NonZeroU64::new(idx as u64 + 1)
                .expect("NodeId index must not exceed u64::MAX - 1"),
        )
    }

    /// Convert back to a 0-based array index for `Vec` / slice indexing.
    #[inline]
    pub fn index(self) -> usize {
        self.0.get() as usize - 1
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.index())
    }
}

/// Structural classification of a node in the scene graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// Has renderable content (mesh, point cloud, etc.) and participates in LOD selection.
    Renderable,
    /// Interior pass-through: no content, exists only to structure the graph.
    Empty,
    /// Root of a composite multi-layer structure (e.g., i3s building sublayers).
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
/// Note: content becomes `Renderable` as soon as the content loader delivers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeLoadState {
    /// Not yet queued for load.
    Unloaded,
    /// In the load Runtime queue; not yet dispatched.
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

/// Outcome assigned to a node at the end of each selection traversal.
///
/// Stored as `last_result` in [`NodeStatus`](crate::NodeStatus) so the next frame can enforce
/// continuity (e.g., keep refining a node whose children are still loading)
/// and count nodes that are fading out of the selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum NodeRefinementResult {
    /// Not visited this frame (unloaded, or outside the hierarchy root).
    #[default]
    None,
    /// Frustum-culled: outside every view this frame.
    Culled,
    /// Selected for rendering as a leaf (no refinement, content is resident).
    Rendered,
    /// Refined: children replace this node in the render set.
    Refined,
    /// Rendered as fallback while children are still loading, AND the
    /// `loading_descendant_limit` forced the subtree to stop loading deeper.
    RenderedAndKicked,
    /// Children are replacing this node, but some descendant load was denied
    /// by `loading_descendant_limit`.
    RefinedAndKicked,
}

impl NodeRefinementResult {
    /// Returns `true` if the node was refined (pushed children) last frame.
    /// When true the engine must continue refining even if children are not
    /// yet renderable, to avoid a pop of coarser geometry.
    #[inline]
    pub fn is_refined(self) -> bool {
        matches!(self, Self::Refined | Self::RefinedAndKicked)
    }

    /// Returns `true` if this node's own content was in the render set last frame.
    #[inline]
    pub fn was_rendered(self) -> bool {
        matches!(self, Self::Rendered | Self::RenderedAndKicked)
    }
}
