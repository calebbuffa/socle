use crate::load::{ContentKey, HierarchyReference};
use crate::lod::{LodDescriptor, RefinementMode};
use crate::node::{NodeId, NodeKind};

use zukei::SpatialBounds;

/// Patch emitted by `HierarchyResolver` after resolving an external hierarchy reference.
/// The engine passes this to `SpatialHierarchy::expand` on the main thread.
///
/// `payload` carries opaque format-specific node data.  The concrete type is an
/// agreement between the `HierarchyResolver` and `SpatialHierarchy` implementations
/// for a given format.  Implicit hierarchies that only need the parent ID to
/// re-expand leave `payload` as `None`.
pub struct HierarchyExpansion {
    /// The node in the live hierarchy under which new children will be inserted.
    pub parent: NodeId,
    /// Opaque format-specific payload, if any.
    pub payload: Option<Box<dyn std::any::Any + Send>>,
    /// Monotonically increasing revision stamp of the hierarchy at the time this
    /// patch was issued.  `SpatialHierarchy::expand` implementations may
    /// reject patches whose revision is older than the hierarchy's current
    /// revision, guarding against late-arriving network responses.
    ///
    /// A value of `0` means "no revision check".
    pub revision: u64,
}

impl HierarchyExpansion {
    /// Create a patch with no payload (sufficient for implicit hierarchies that
    /// only need the parent ID to trigger a re-expand).
    pub fn new(parent: NodeId) -> Self {
        Self {
            parent,
            payload: None,
            revision: 0,
        }
    }

    /// Create a patch carrying format-specific node data.
    pub fn with_payload(parent: NodeId, data: impl std::any::Any + Send + 'static) -> Self {
        Self {
            parent,
            payload: Some(Box::new(data)),
            revision: 0,
        }
    }

    /// Extract the typed payload, consuming it from this patch.
    ///
    /// Returns `Err` if no payload is present or if the payload is not of type `T`.
    pub fn take_payload<T: std::any::Any + Send + 'static>(
        &mut self,
    ) -> Result<T, HierarchyExpansionError> {
        match self.payload.take() {
            Some(boxed) => boxed
                .downcast::<T>()
                .map(|b| *b)
                .map_err(|_| HierarchyExpansionError {
                    message: format!(
                        "payload type mismatch: expected {}",
                        std::any::type_name::<T>()
                    ),
                }),
            None => Err(HierarchyExpansionError {
                message: "patch has no payload".to_owned(),
            }),
        }
    }

    /// Returns `true` if the payload is of type `T` without consuming it.
    ///
    /// Use this to branch on payload type before calling [`take_payload`](Self::take_payload).
    pub fn payload_is<T: std::any::Any + Send + 'static>(&self) -> bool {
        self.payload
            .as_ref()
            .map(|b| b.downcast_ref::<T>().is_some())
            .unwrap_or(false)
    }
}

impl std::fmt::Debug for HierarchyExpansion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HierarchyExpansion")
            .field("parent", &self.parent)
            .field("revision", &self.revision)
            .field("payload", &self.payload.as_ref().map(|_| "<opaque>"))
            .finish()
    }
}

/// Error returned when a resolved hierarchy patch cannot be applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HierarchyExpansionError {
    pub message: String,
}

impl std::fmt::Display for HierarchyExpansionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for HierarchyExpansionError {}

/// Read-only view of a spatial hierarchy.
///
/// A hierarchy node must have stable data (bounds, LOD) after it is first observed.
/// The engine may cache returned values for the lifetime of the node.
pub trait SpatialHierarchy: Send + Sync + 'static {
    // ── Required ─────────────────────────────────────────────────────────────

    /// Returns the root node identifier.
    fn root(&self) -> NodeId;

    /// Returns the parent of `node`, or `None` if `node` is the root.
    fn parent(&self, node: NodeId) -> Option<NodeId>;

    /// Returns the children of `node`.
    fn children(&self, node: NodeId) -> &[NodeId];

    /// Structural classification of the node.
    fn node_kind(&self, node: NodeId) -> NodeKind;

    /// Bounding volume expressed in the engine's working coordinate system.
    fn bounds(&self, node: NodeId) -> &SpatialBounds;

    /// LOD descriptor used by `LodEvaluator` to decide refinement.
    fn lod_descriptor(&self, node: NodeId) -> &LodDescriptor;

    /// How children contribute to the parent's region.
    fn refinement_mode(&self, node: NodeId) -> RefinementMode;

    /// Stable content addresses for the node, if any.
    /// Returns an empty slice for structural/interior-only nodes.
    /// Returns multiple entries for formats with multi-content nodes (e.g. 3D Tiles 1.1).
    fn content_keys(&self, node: NodeId) -> &[ContentKey];

    /// Apply a resolved hierarchy patch deterministically.
    fn expand(&mut self, patch: HierarchyExpansion) -> Result<(), HierarchyExpansionError>;

    // ── Optional (defaults provided) ─────────────────────────────────────────

    /// Optional tighter bounding volume covering only this node's content,
    /// excluding child contributions. Returns `None` by default, meaning
    /// `bounds()` is used for both spatial and content culling.
    fn content_bounds(&self, node: NodeId) -> Option<&SpatialBounds> {
        let _ = node;
        None
    }

    /// Optional viewer request volume for this node.
    ///
    /// When `Some`, the entire subtree rooted at this node is only traversed
    /// (and the node itself only rendered) when the primary camera position is
    /// *inside* the returned bounding volume.  This corresponds to the 3D Tiles
    /// `viewerRequestVolume` property and is used for interior models, portals,
    /// and street-level layers that should only activate when the viewer is close.
    ///
    /// Returns `None` by default (no restriction).
    fn viewer_request_volume(&self, node: NodeId) -> Option<&SpatialBounds> {
        let _ = node;
        None
    }

    /// Per-node LOD metric override.
    ///
    /// When `Some(value)`, the traversal uses this value in place of the
    /// node's `LodDescriptor::value` when deciding whether to refine.
    /// Intended for formats (e.g. 3D Tiles 1.1 implicit tiling) that supply
    /// per-node metric overrides in metadata.
    ///
    /// Returns `None` by default (use the descriptor's built-in value).
    fn lod_metric_override(&self, node: NodeId) -> Option<f64> {
        let _ = node;
        None
    }

    /// Maximum age of content loaded for this node.
    ///
    /// When `Some(duration)`, the engine re-fetches the node's content once
    /// `duration` has elapsed since it was last loaded.
    ///
    /// Returns `None` by default (content never expires).
    fn content_max_age(&self, node: NodeId) -> Option<std::time::Duration> {
        let _ = node;
        None
    }

    /// The accumulated world-space transform for this node.
    ///
    /// Used to propagate the parent coordinate frame into external child
    /// hierarchies when a node's content resolves to an external reference
    /// (e.g. a child `tileset.json`).  Implementations that do not track
    /// per-node transforms may return [`glam::DMat4::IDENTITY`].
    fn world_transform(&self, node: NodeId) -> glam::DMat4 {
        let _ = node;
        glam::DMat4::IDENTITY
    }
}

/// Resolves external hierarchy references produced by `Payload::Reference`.
///
/// Implementations typically fetch and parse an external hierarchy document
/// (e.g. a child tileset.json or i3s nodePages), mutate a shared
/// `SpatialHierarchy`, and return the inserted node IDs.
pub trait HierarchyResolver: Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Asynchronously resolve an external hierarchy reference.
    ///
    /// Returns `None` if the reference resolves to an empty set (e.g., outside the extent).
    fn resolve_reference(
        &self,
        bg_context: &orkester::Context,
        reference: HierarchyReference,
    ) -> orkester::Task<Result<Option<HierarchyExpansion>, Self::Error>>;
}

impl SpatialHierarchy for Box<dyn SpatialHierarchy> {
    fn root(&self) -> NodeId {
        (**self).root()
    }
    fn parent(&self, node: NodeId) -> Option<NodeId> {
        (**self).parent(node)
    }
    fn children(&self, node: NodeId) -> &[NodeId] {
        (**self).children(node)
    }
    fn node_kind(&self, node: NodeId) -> NodeKind {
        (**self).node_kind(node)
    }
    fn bounds(&self, node: NodeId) -> &SpatialBounds {
        (**self).bounds(node)
    }
    fn lod_descriptor(&self, node: NodeId) -> &LodDescriptor {
        (**self).lod_descriptor(node)
    }
    fn refinement_mode(&self, node: NodeId) -> RefinementMode {
        (**self).refinement_mode(node)
    }
    fn content_bounds(&self, node: NodeId) -> Option<&SpatialBounds> {
        (**self).content_bounds(node)
    }
    fn viewer_request_volume(&self, node: NodeId) -> Option<&SpatialBounds> {
        (**self).viewer_request_volume(node)
    }
    fn lod_metric_override(&self, node: NodeId) -> Option<f64> {
        (**self).lod_metric_override(node)
    }
    fn content_max_age(&self, node: NodeId) -> Option<std::time::Duration> {
        (**self).content_max_age(node)
    }
    fn content_keys(&self, node: NodeId) -> &[ContentKey] {
        (**self).content_keys(node)
    }

    fn expand(&mut self, patch: HierarchyExpansion) -> Result<(), HierarchyExpansionError> {
        (**self).expand(patch)
    }
}

impl<E: std::error::Error + Send + Sync + 'static> HierarchyResolver
    for Box<dyn HierarchyResolver<Error = E>>
{
    type Error = E;

    fn resolve_reference(
        &self,
        bg_context: &orkester::Context,
        reference: HierarchyReference,
    ) -> orkester::Task<Result<Option<HierarchyExpansion>, E>> {
        (**self).resolve_reference(bg_context, reference)
    }
}
