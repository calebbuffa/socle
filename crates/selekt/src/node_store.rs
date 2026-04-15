//! Engine-owned flat arena of spatial nodes.
//!
//! [`NodeStore`] is the single source of truth for all node structure and
//! metadata. Format adapters provide initial data via [`NodeDescriptor`]
//! arrays; the store assigns [`NodeId`]s and manages parent–child links.
//!
//! The store is format-agnostic: it knows about bounds, LOD, refinement
//! mode, and content keys — nothing about 3D Tiles, I3S, or any specific
//! format. Loaders populate it; the selection algorithm reads it.

use crate::load::ContentKey;
use crate::lod::{LodDescriptor, RefinementMode};
use crate::node::{NodeId, NodeKind};

use glam::DMat4;
use zukei::SpatialBounds;

/// Per-node data stored in the arena.
///
/// All fields are public for direct read access during traversal.
/// Mutation goes through [`NodeStore`] methods.
#[derive(Clone, Debug)]
pub struct NodeData {
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub bounds: SpatialBounds,
    pub lod: LodDescriptor,
    pub refinement: RefinementMode,
    pub kind: NodeKind,
    pub content_keys: Vec<ContentKey>,
    pub world_transform: DMat4,
    /// When `true`, the selection algorithm will call the expand callback
    /// if this node has no children. Set by implicit/procedural sources.
    pub might_have_latent_children: bool,
    /// Optional tighter bounding volume for content only (no children).
    pub content_bounds: Option<SpatialBounds>,
    /// Subtree only traversed when camera is inside this volume.
    pub viewer_request_volume: Option<SpatialBounds>,
    /// Per-node LOD metric override (replaces `lod.value` if `Some`).
    pub lod_metric_override: Option<f64>,
    /// Geographic extent in geodetic radians (if available).
    pub globe_rectangle: Option<terra::GlobeRectangle>,
    /// Always refine, never render (external tileset refs, malformed tiles).
    pub unconditionally_refined: bool,
    /// Maximum content age before re-fetch.
    pub content_max_age: Option<std::time::Duration>,
    /// Which segment (loader) owns this node.
    pub segment: u16,
}

/// Descriptor for creating a new node. Used by format adapters.
///
/// Children are described as indices into the same descriptor slice
/// (for initial tree construction) or are empty (for latent expansion
/// where children are inserted separately).
#[derive(Clone, Debug)]
pub struct NodeDescriptor {
    pub bounds: SpatialBounds,
    pub lod: LodDescriptor,
    pub refinement: RefinementMode,
    pub kind: NodeKind,
    pub content_keys: Vec<ContentKey>,
    pub world_transform: DMat4,
    pub might_have_latent_children: bool,
    /// Indices into the same `NodeDescriptor` slice for initial children.
    /// Empty for leaf nodes or latent-expansion nodes.
    pub child_indices: Vec<usize>,
    // Optional fields — default to None/false.
    pub content_bounds: Option<SpatialBounds>,
    pub viewer_request_volume: Option<SpatialBounds>,
    pub lod_metric_override: Option<f64>,
    pub globe_rectangle: Option<terra::GlobeRectangle>,
    pub unconditionally_refined: bool,
    pub content_max_age: Option<std::time::Duration>,
}

impl NodeDescriptor {
    /// Minimal leaf descriptor. Most fields default to sensible values.
    pub fn leaf(bounds: SpatialBounds, lod: LodDescriptor, content_key: ContentKey) -> Self {
        Self {
            bounds,
            lod,
            refinement: RefinementMode::Replace,
            kind: NodeKind::Renderable,
            content_keys: vec![content_key],
            world_transform: DMat4::IDENTITY,
            might_have_latent_children: false,
            child_indices: Vec::new(),
            content_bounds: None,
            viewer_request_volume: None,
            lod_metric_override: None,
            globe_rectangle: None,
            unconditionally_refined: false,
            content_max_age: None,
        }
    }

    /// Interior (empty) node with children.
    pub fn interior(
        bounds: SpatialBounds,
        lod: LodDescriptor,
        refinement: RefinementMode,
        child_indices: Vec<usize>,
    ) -> Self {
        Self {
            bounds,
            lod,
            refinement,
            kind: NodeKind::Empty,
            content_keys: Vec::new(),
            world_transform: DMat4::IDENTITY,
            might_have_latent_children: false,
            child_indices,
            content_bounds: None,
            viewer_request_volume: None,
            lod_metric_override: None,
            globe_rectangle: None,
            unconditionally_refined: false,
            content_max_age: None,
        }
    }
}

/// Flat arena of spatial nodes.
///
/// Nodes are stored in a dense `Vec<NodeData>` indexed by [`NodeId`].
/// The store grows monotonically — nodes are appended but never removed
/// (eviction is handled at the content level, not the structure level).
pub struct NodeStore {
    nodes: Vec<NodeData>,
    root: NodeId,
}

impl NodeStore {
    /// Build a `NodeStore` from a flat array of [`NodeDescriptor`]s.
    ///
    /// `root_index` is the index into `descriptors` that becomes the root node.
    /// `segment` is the loader segment ID for all nodes in this batch.
    ///
    /// `child_indices` in each descriptor refer to positions within `descriptors`.
    pub fn from_descriptors(
        descriptors: &[NodeDescriptor],
        root_index: usize,
        segment: u16,
    ) -> Self {
        let base = 0usize;
        let mut nodes = Vec::with_capacity(descriptors.len());

        // First pass: create all NodeData with empty children/parent.
        for desc in descriptors {
            nodes.push(NodeData {
                parent: None,
                children: Vec::new(),
                bounds: desc.bounds.clone(),
                lod: desc.lod,
                refinement: desc.refinement,
                kind: desc.kind,
                content_keys: desc.content_keys.clone(),
                world_transform: desc.world_transform,
                might_have_latent_children: desc.might_have_latent_children,
                content_bounds: desc.content_bounds.clone(),
                viewer_request_volume: desc.viewer_request_volume.clone(),
                lod_metric_override: desc.lod_metric_override,
                globe_rectangle: desc.globe_rectangle,
                unconditionally_refined: desc.unconditionally_refined,
                content_max_age: desc.content_max_age,
                segment,
            });
        }

        // Second pass: wire up parent/children using child_indices.
        for i in 0..descriptors.len() {
            let child_ids: Vec<NodeId> = descriptors[i]
                .child_indices
                .iter()
                .map(|&ci| NodeId::from_index(base + ci))
                .collect();
            let parent_id = NodeId::from_index(base + i);
            for &child_id in &child_ids {
                nodes[child_id.index()].parent = Some(parent_id);
            }
            nodes[i].children = child_ids;
        }

        let root = NodeId::from_index(base + root_index);
        Self { nodes, root }
    }

    /// The root node.
    #[inline]
    pub fn root(&self) -> NodeId {
        self.root
    }

    /// Total number of nodes in the store.
    #[inline]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the store is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Direct access to a node's data.
    #[inline]
    pub fn get(&self, id: NodeId) -> &NodeData {
        &self.nodes[id.index()]
    }

    /// Mutable access to a node's data.
    #[inline]
    pub fn get_mut(&mut self, id: NodeId) -> &mut NodeData {
        &mut self.nodes[id.index()]
    }

    // ── Convenience accessors (avoid verbose `store.get(id).field`) ──

    #[inline]
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.nodes[id.index()].parent
    }

    #[inline]
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        &self.nodes[id.index()].children
    }

    #[inline]
    pub fn bounds(&self, id: NodeId) -> &SpatialBounds {
        &self.nodes[id.index()].bounds
    }

    #[inline]
    pub fn lod_descriptor(&self, id: NodeId) -> &LodDescriptor {
        &self.nodes[id.index()].lod
    }

    #[inline]
    pub fn refinement_mode(&self, id: NodeId) -> RefinementMode {
        self.nodes[id.index()].refinement
    }

    #[inline]
    pub fn node_kind(&self, id: NodeId) -> NodeKind {
        self.nodes[id.index()].kind
    }

    #[inline]
    pub fn content_keys(&self, id: NodeId) -> &[ContentKey] {
        &self.nodes[id.index()].content_keys
    }

    #[inline]
    pub fn world_transform(&self, id: NodeId) -> DMat4 {
        self.nodes[id.index()].world_transform
    }

    #[inline]
    pub fn globe_rectangle(&self, id: NodeId) -> Option<terra::GlobeRectangle> {
        self.nodes[id.index()].globe_rectangle
    }

    #[inline]
    pub fn viewer_request_volume(&self, id: NodeId) -> Option<&SpatialBounds> {
        self.nodes[id.index()].viewer_request_volume.as_ref()
    }

    #[inline]
    pub fn content_bounds(&self, id: NodeId) -> Option<&SpatialBounds> {
        self.nodes[id.index()].content_bounds.as_ref()
    }

    #[inline]
    pub fn lod_metric_override(&self, id: NodeId) -> Option<f64> {
        self.nodes[id.index()].lod_metric_override
    }

    #[inline]
    pub fn is_unconditionally_refined(&self, id: NodeId) -> bool {
        self.nodes[id.index()].unconditionally_refined
    }

    #[inline]
    pub fn might_have_latent_children(&self, id: NodeId) -> bool {
        self.nodes[id.index()].might_have_latent_children
    }

    #[inline]
    pub fn segment(&self, id: NodeId) -> u16 {
        self.nodes[id.index()].segment
    }

    /// Insert children under `parent` from descriptors.
    ///
    /// Returns the `NodeId`s of the newly created nodes.
    /// Used by the expand callback to lazily grow the tree.
    pub fn insert_children(
        &mut self,
        parent: NodeId,
        descriptors: &[NodeDescriptor],
        segment: u16,
    ) -> Vec<NodeId> {
        let base = self.nodes.len();
        let mut new_ids = Vec::with_capacity(descriptors.len());

        // Append all new nodes.
        for (i, desc) in descriptors.iter().enumerate() {
            let id = NodeId::from_index(base + i);
            new_ids.push(id);
            self.nodes.push(NodeData {
                parent: Some(parent),
                children: Vec::new(),
                bounds: desc.bounds.clone(),
                lod: desc.lod,
                refinement: desc.refinement,
                kind: desc.kind,
                content_keys: desc.content_keys.clone(),
                world_transform: desc.world_transform,
                might_have_latent_children: desc.might_have_latent_children,
                content_bounds: desc.content_bounds.clone(),
                viewer_request_volume: desc.viewer_request_volume.clone(),
                lod_metric_override: desc.lod_metric_override,
                globe_rectangle: desc.globe_rectangle,
                unconditionally_refined: desc.unconditionally_refined,
                content_max_age: desc.content_max_age,
                segment,
            });
        }

        // Wire up internal children within the batch.
        for (i, desc) in descriptors.iter().enumerate() {
            let child_ids: Vec<NodeId> = desc
                .child_indices
                .iter()
                .map(|&ci| NodeId::from_index(base + ci))
                .collect();
            let node_id = NodeId::from_index(base + i);
            for &child_id in &child_ids {
                self.nodes[child_id.index()].parent = Some(node_id);
            }
            self.nodes[base + i].children = child_ids;
        }

        // Set parent's children to the top-level new nodes (those whose parent is `parent`).
        let top_level: Vec<NodeId> = new_ids
            .iter()
            .copied()
            .filter(|&id| self.nodes[id.index()].parent == Some(parent))
            .collect();
        self.nodes[parent.index()].children = top_level;
        // Mark parent as no longer latent (children have been created).
        self.nodes[parent.index()].might_have_latent_children = false;

        new_ids
    }

    /// Insert a sub-scene (e.g. external tileset) under a reference node.
    ///
    /// All nodes from `descriptors` are appended to the store. The reference
    /// node's children are set to `[sub_root]` and its kind becomes `CompositeRoot`.
    pub fn insert_sub_scene(
        &mut self,
        ref_node: NodeId,
        descriptors: &[NodeDescriptor],
        sub_root_index: usize,
        segment: u16,
    ) -> Vec<NodeId> {
        let new_ids = self.insert_children(ref_node, descriptors, segment);
        // Override: only the sub-root should be a child of ref_node.
        let sub_root = NodeId::from_index(self.nodes.len() - descriptors.len() + sub_root_index);
        self.nodes[ref_node.index()].children = vec![sub_root];
        self.nodes[ref_node.index()].kind = NodeKind::CompositeRoot;
        self.nodes[sub_root.index()].parent = Some(ref_node);
        new_ids
    }

    /// Add a new segment's initial tree. Returns `(root_node_id, all_new_ids)`.
    ///
    /// Used when adding a second tileset source to the same engine.
    pub fn add_segment(
        &mut self,
        descriptors: &[NodeDescriptor],
        root_index: usize,
        segment: u16,
    ) -> (NodeId, Vec<NodeId>) {
        let base = self.nodes.len();
        let ids = self.insert_children(self.root, descriptors, segment);
        let root = NodeId::from_index(base + root_index);
        // Undo: the insert_children set root's children to include ALL new nodes.
        // We actually want the new segment's root added alongside the existing root.
        // For now, this is handled at the kiban level by managing multiple roots.
        (root, ids)
    }
}
