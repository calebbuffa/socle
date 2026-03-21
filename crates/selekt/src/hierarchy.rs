use crate::load::{ContentKey, HierarchyReference};
use crate::lod::{LodDescriptor, RefinementMode};
use crate::node::{NodeId, NodeKind};
use orkester::AsyncSystem;
use zukei::bounds::SpatialBounds;

/// Patch emitted by `HierarchyResolver` after resolving an external hierarchy reference.
/// The engine merges this into the active spatial hierarchy.
#[derive(Debug)]
pub struct HierarchyPatch {
    /// The node whose content turned out to be an external hierarchy.
    pub parent: NodeId,
    /// All new `NodeId`s inserted beneath `parent` after the resolve.
    pub inserted_nodes: Vec<NodeId>,
}

/// Error returned when a resolved hierarchy patch cannot be applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HierarchyPatchError {
    pub message: String,
}

impl std::fmt::Display for HierarchyPatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for HierarchyPatchError {}

/// Read-only view of a spatial hierarchy.
///
/// A hierarchy node must have stable data (bounds, LOD) after it is first observed.
/// The engine may cache returned values for the lifetime of the node.
pub trait SpatialHierarchy: Send + Sync + 'static {
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

    /// Optional tighter bounding volume covering only this node's content,
    /// excluding child contributions. Returns `None` by default, meaning
    /// `bounds()` is used for both spatial and content culling.
    fn content_bounds(&self, node: NodeId) -> Option<&SpatialBounds> {
        let _ = node;
        None
    }

    /// Stable content address for the node, if any.
    /// Returns `None` for structural/interior-only nodes.
    fn content_key(&self, node: NodeId) -> Option<&ContentKey>;

    /// Apply a resolved hierarchy patch deterministically.
    fn apply_patch(&mut self, patch: HierarchyPatch) -> Result<(), HierarchyPatchError>;
}

/// Resolves external hierarchy references produced by `Payload::Reference`.
///
/// Implementations typically fetch and parse a child tileset.json / i3s nodePages,
/// mutate a shared `SpatialHierarchy`, and return the inserted node IDs.
pub trait HierarchyResolver: Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Asynchronously resolve an external hierarchy reference.
    ///
    /// Returns `None` if the reference resolves to an empty set (e.g., outside the extent).
    fn resolve_reference(
        &self,
        async_system: &AsyncSystem,
        reference: HierarchyReference,
    ) -> orkester::Future<Result<Option<HierarchyPatch>, Self::Error>>;
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
    fn content_key(&self, node: NodeId) -> Option<&ContentKey> {
        (**self).content_key(node)
    }
    fn apply_patch(&mut self, patch: HierarchyPatch) -> Result<(), HierarchyPatchError> {
        (**self).apply_patch(patch)
    }
}

impl<E: std::error::Error + Send + Sync + 'static> HierarchyResolver
    for Box<dyn HierarchyResolver<Error = E>>
{
    type Error = E;

    fn resolve_reference(
        &self,
        async_system: &AsyncSystem,
        reference: HierarchyReference,
    ) -> orkester::Future<Result<Option<HierarchyPatch>, E>> {
        (**self).resolve_reference(async_system, reference)
    }
}
