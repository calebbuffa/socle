//! Internal multi-graph composition layer.
//!
//! A [`GraphSet<C>`] holds the root [`SceneGraph`] plus any sub-scenes that are
//! attached at runtime when a `ContentLoader` resolves an external reference.
//! It implements `SceneGraph` itself, so the traversal and step machinery need
//! no knowledge of multiple-graph composition.
//!
//! # Global NodeIds
//!
//! Each graph segment is assigned a non-overlapping *base* offset. For a node
//! with local index `l` in segment with base `b`, the global [`NodeId`] index is
//! `l + b`. This means existing code that indexes into `NodeStateVec` by
//! `NodeId::index()` works without change — the vec auto-grows.
//!
//! # Children across boundaries
//!
//! When `attach(ref_node, sub_graph, sub_loader)` is called, two things happen:
//! 1. A new segment (base = previous total) is created for `sub_graph`.
//! 2. `ref_node`'s pre-computed global-children entry is overwritten with
//!    `[sub_root_global]`, so subsequent calls to `children(ref_node)` return
//!    the sub-scene root instead of an empty slice.

use glam::DMat4;
use zukei::SpatialBounds;

use crate::hierarchy::SceneGraph;
use crate::load::{ContentKey, DynContentLoader};
use crate::lod::{LodDescriptor, RefinementMode};
use crate::node::{NodeId, NodeKind};

struct GraphSegment<C: Send + 'static> {
    /// First global-index owned by this segment.
    base: usize,
    /// Number of nodes in `graph`.
    count: usize,
    graph: Box<dyn SceneGraph>,
    loader: Box<dyn DynContentLoader<C>>,
    /// `global_children[local_idx]` = pre-computed children in global NodeId space.
    ///
    /// Mutated when `attach()` targets a node in this segment.
    global_children: Vec<Vec<NodeId>>,
}

impl<C: Send + 'static> GraphSegment<C> {
    #[inline]
    fn to_local(&self, global: NodeId) -> NodeId {
        NodeId::from_index(global.index() - self.base)
    }
    #[inline]
    fn to_global(&self, local: NodeId) -> NodeId {
        NodeId::from_index(local.index() + self.base)
    }
    #[inline]
    fn owns(&self, global: NodeId) -> bool {
        let i = global.index();
        i >= self.base && i < self.base + self.count
    }
}

fn build_segment<C: Send + 'static>(
    graph: Box<dyn SceneGraph>,
    loader: Box<dyn DynContentLoader<C>>,
    base: usize,
) -> GraphSegment<C> {
    let count = graph.node_count();
    let global_children: Vec<Vec<NodeId>> = (0..count)
        .map(|local_idx| {
            let local_id = NodeId::from_index(local_idx);
            graph
                .children(local_id)
                .iter()
                .map(|&l| NodeId::from_index(l.index() + base))
                .collect()
        })
        .collect();
    GraphSegment {
        base,
        count,
        graph,
        loader,
        global_children,
    }
}

/// Engine-internal registry of all active scene graphs + their content loaders.
///
/// The root graph is always in `segments[0]` with `base = 0`.  Sub-scenes are
/// appended by [`attach`](Self::attach).
///
/// `GraphSet<C>` implements [`SceneGraph`] — the traversal and step machinery
/// call its spatial methods exactly as they would call a single `SceneGraph`.
pub(crate) struct GraphSet<C: Send + 'static> {
    segments: Vec<GraphSegment<C>>,
    /// Nodes whose `node_kind` has been overridden to `CompositeRoot` because
    /// they loaded a `SceneRef` and have a sub-scene attached under them.
    /// Such nodes have no renderable content of their own and must not appear
    /// in `nodes_to_render`.
    composite_roots: Vec<NodeId>,
}

impl<C: Send + 'static> GraphSet<C> {
    /// Create a `GraphSet` seeded with the root graph and loader.
    pub fn new(root_graph: Box<dyn SceneGraph>, root_loader: Box<dyn DynContentLoader<C>>) -> Self {
        let seg = build_segment(root_graph, root_loader, 0);
        Self {
            segments: vec![seg],
            composite_roots: Vec::new(),
        }
    }

    /// Attach a sub-scene under `ref_node`.
    ///
    /// After this call `children(ref_node)` returns `[sub_root_global]` and
    /// subsequent traversal will descend into the sub-scene.
    pub fn attach(
        &mut self,
        ref_node: NodeId,
        sub_graph: Box<dyn SceneGraph>,
        sub_loader: Box<dyn DynContentLoader<C>>,
    ) {
        // Assign a base right after the last segment.
        let base = self
            .segments
            .iter()
            .map(|s| s.base + s.count)
            .max()
            .unwrap_or(0);
        let sub_root_global = NodeId::from_index(sub_graph.root().index() + base);

        // Rewrite ref_node's children in the owning segment.
        let Some(ref_seg) = self
            .segments
            .iter_mut()
            .find(|s| s.owns(ref_node))
        else {
            debug_assert!(false, "ref_node does not belong to any known segment");
            return;
        };
        let local_idx = ref_node.index() - ref_seg.base;
        ref_seg.global_children[local_idx] = vec![sub_root_global];

        self.segments
            .push(build_segment(sub_graph, sub_loader, base));

        // Mark the reference node as CompositeRoot so the traversal treats it
        // as a structural passthrough instead of a renderable leaf.
        if !self.composite_roots.contains(&ref_node) {
            self.composite_roots.push(ref_node);
        }
    }

    /// True if `ref_node` already has a sub-scene attached (to avoid double-attach).
    pub fn has_sub_scene(&self, ref_node: NodeId) -> bool {
        if let Some(seg) = self.segments.iter().find(|s| s.owns(ref_node)) {
            let local_idx = ref_node.index() - seg.base;
            !seg.global_children[local_idx].is_empty()
                && seg.graph.children(seg.to_local(ref_node)).is_empty()
        } else {
            false
        }
    }

    /// Borrow the content loader responsible for `global`.
    pub fn loader_for(&self, global: NodeId) -> &dyn DynContentLoader<C> {
        self.seg_for(global).loader.as_ref()
    }

    #[inline]
    fn seg_for(&self, global: NodeId) -> &GraphSegment<C> {
        self.segments
            .iter()
            .find(|s| s.owns(global))
            .expect("NodeId does not belong to any known GraphSegment")
    }
}


impl<C: Send + 'static> SceneGraph for GraphSet<C> {
    fn root(&self) -> NodeId {
        let seg = &self.segments[0];
        NodeId::from_index(seg.graph.root().index() + seg.base)
    }

    fn parent(&self, global: NodeId) -> Option<NodeId> {
        let seg = self.seg_for(global);
        seg.graph
            .parent(seg.to_local(global))
            .map(|l| seg.to_global(l))
    }

    fn children(&self, global: NodeId) -> &[NodeId] {
        let seg = self.seg_for(global);
        &seg.global_children[global.index() - seg.base]
    }

    fn node_kind(&self, global: NodeId) -> NodeKind {
        if self.composite_roots.contains(&global) {
            return NodeKind::CompositeRoot;
        }
        let seg = self.seg_for(global);
        seg.graph.node_kind(seg.to_local(global))
    }

    fn bounds(&self, global: NodeId) -> &SpatialBounds {
        let seg = self.seg_for(global);
        seg.graph.bounds(seg.to_local(global))
    }

    fn lod_descriptor(&self, global: NodeId) -> &LodDescriptor {
        let seg = self.seg_for(global);
        seg.graph.lod_descriptor(seg.to_local(global))
    }

    fn refinement_mode(&self, global: NodeId) -> RefinementMode {
        let seg = self.seg_for(global);
        seg.graph.refinement_mode(seg.to_local(global))
    }

    fn content_keys(&self, global: NodeId) -> &[ContentKey] {
        let seg = self.seg_for(global);
        seg.graph.content_keys(seg.to_local(global))
    }

    fn node_count(&self) -> usize {
        self.segments.iter().map(|s| s.count).sum()
    }

    fn content_bounds(&self, global: NodeId) -> Option<&SpatialBounds> {
        let seg = self.seg_for(global);
        seg.graph.content_bounds(seg.to_local(global))
    }

    fn viewer_request_volume(&self, global: NodeId) -> Option<&SpatialBounds> {
        let seg = self.seg_for(global);
        seg.graph.viewer_request_volume(seg.to_local(global))
    }

    fn lod_metric_override(&self, global: NodeId) -> Option<f64> {
        let seg = self.seg_for(global);
        seg.graph.lod_metric_override(seg.to_local(global))
    }

    fn content_max_age(&self, global: NodeId) -> Option<std::time::Duration> {
        let seg = self.seg_for(global);
        seg.graph.content_max_age(seg.to_local(global))
    }

    fn world_transform(&self, global: NodeId) -> DMat4 {
        let seg = self.seg_for(global);
        seg.graph.world_transform(seg.to_local(global))
    }
}
