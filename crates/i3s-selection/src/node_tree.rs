//! Uniform node tree access, abstracting over I3S node page formats.
//!
//! Mesh-based layers (3DObject, IntegratedMesh, Point) use [`NodePage`]/[`Node`].
//! PointCloud layers use [`PointCloudNodePageDefinition`]/[`PointCloudNode`].
//! This enum provides a common interface for the selection algorithm.

use i3s::cmn::{Node, NodePage, Obb};
use i3s::pcsl::{PointCloudNode, PointCloudNodePageDefinition};

use i3s_reader::json::read_json;
use i3s_util::Result;

/// Uniform access to an I3S node tree.
pub enum NodeTree {
    /// 3DObject / IntegratedMesh / Point — uses `NodePage` with `Node`.
    Paged(PagedNodeTree),
    /// PointCloud — uses `PointCloudNodePageDefinition` with `PointCloudNode`.
    PointCloud(PointCloudNodeTree),
}

/// Node tree backed by standard `NodePage`s (3DObject, IntegratedMesh, Point).
pub struct PagedNodeTree {
    pub(crate) node_pages: Vec<Option<NodePage>>,
    pub(crate) nodes_per_page: usize,
}

/// Node tree backed by `PointCloudNodePageDefinition` pages.
pub struct PointCloudNodeTree {
    pub(crate) node_pages: Vec<Option<PointCloudNodePageDefinition>>,
    pub(crate) nodes_per_page: usize,
}

impl PagedNodeTree {
    /// Look up a `Node` by global node ID.
    pub fn node(&self, node_id: u32) -> Option<&Node> {
        let page_idx = node_id as usize / self.nodes_per_page;
        let node_within = node_id as usize % self.nodes_per_page;
        self.node_pages
            .get(page_idx)?
            .as_ref()?
            .nodes
            .get(node_within)
    }
}

impl PointCloudNodeTree {
    /// Look up a `PointCloudNode` by global node ID.
    pub fn node(&self, node_id: u32) -> Option<&PointCloudNode> {
        let page_idx = node_id as usize / self.nodes_per_page;
        let node_within = node_id as usize % self.nodes_per_page;
        self.node_pages
            .get(page_idx)?
            .as_ref()?
            .nodes
            .get(node_within)
    }
}

impl NodeTree {
    /// Nodes per page for this tree.
    pub fn nodes_per_page(&self) -> usize {
        match self {
            NodeTree::Paged(t) => t.nodes_per_page,
            NodeTree::PointCloud(t) => t.nodes_per_page,
        }
    }

    /// Total number of nodes across all loaded pages.
    pub fn node_count(&self) -> usize {
        match self {
            NodeTree::Paged(t) => t
                .node_pages
                .iter()
                .enumerate()
                .filter_map(|(i, p)| {
                    p.as_ref()
                        .map(|page| i * t.nodes_per_page + page.nodes.len())
                })
                .max()
                .unwrap_or(0),
            NodeTree::PointCloud(t) => t
                .node_pages
                .iter()
                .enumerate()
                .filter_map(|(i, p)| {
                    p.as_ref()
                        .map(|page| i * t.nodes_per_page + page.nodes.len())
                })
                .max()
                .unwrap_or(0),
        }
    }

    /// Append child node IDs for the given node.
    pub fn children_of(&self, node_id: u32, out: &mut Vec<u32>) {
        match self {
            NodeTree::Paged(t) => {
                if let Some(node) = t.node(node_id) {
                    if let Some(children) = &node.children {
                        out.extend(
                            children
                                .iter()
                                .filter_map(|&id| if id >= 0 { Some(id as u32) } else { None }),
                        );
                    }
                }
            }
            NodeTree::PointCloud(t) => {
                if let Some(node) = t.node(node_id) {
                    let first = node.first_child;
                    let count = node.child_count;
                    if first >= 0 && count > 0 {
                        for i in 0..count {
                            out.push((first + i) as u32);
                        }
                    }
                }
            }
        }
    }

    /// Get the spec OBB for a node (before CRS conversion).
    pub fn obb_of(&self, node_id: u32) -> Option<&Obb> {
        match self {
            NodeTree::Paged(t) => t.node(node_id).map(|n| &n.obb),
            NodeTree::PointCloud(t) => t.node(node_id).map(|n| &n.obb),
        }
    }

    /// Get the LOD threshold for a node.
    pub fn lod_threshold_of(&self, node_id: u32) -> f64 {
        match self {
            NodeTree::Paged(t) => t.node(node_id).and_then(|n| n.lod_threshold).unwrap_or(0.0),
            NodeTree::PointCloud(t) => t.node(node_id).and_then(|n| n.lod_threshold).unwrap_or(0.0),
        }
    }

    /// Check if a given page has been loaded.
    pub fn page_loaded(&self, page_id: u32) -> bool {
        let idx = page_id as usize;
        match self {
            NodeTree::Paged(t) => idx < t.node_pages.len() && t.node_pages[idx].is_some(),
            NodeTree::PointCloud(t) => idx < t.node_pages.len() && t.node_pages[idx].is_some(),
        }
    }

    /// Insert a page from raw JSON bytes, parsing according to the tree variant.
    pub fn insert_page(&mut self, page_id: u32, bytes: &[u8]) -> Result<()> {
        match self {
            NodeTree::Paged(t) => {
                let page: NodePage = read_json(bytes)?;
                let idx = page_id as usize;
                while t.node_pages.len() <= idx {
                    t.node_pages.push(None);
                }
                t.node_pages[idx] = Some(page);
            }
            NodeTree::PointCloud(t) => {
                let page: PointCloudNodePageDefinition = read_json(bytes)?;
                let idx = page_id as usize;
                while t.node_pages.len() <= idx {
                    t.node_pages.push(None);
                }
                t.node_pages[idx] = Some(page);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use i3s::cmn::Obb;

    fn make_node(children: Option<Vec<i64>>, lod: f64) -> Node {
        Node {
            obb: Obb {
                center: [0.0, 0.0, 0.0],
                half_size: [1.0, 1.0, 1.0],
                quaternion: [0.0, 0.0, 0.0, 1.0],
            },
            lod_threshold: Some(lod),
            children,
            ..Default::default()
        }
    }

    fn make_pcnode(first_child: i64, child_count: i64, lod: f64) -> PointCloudNode {
        PointCloudNode {
            resource_id: 0,
            first_child,
            child_count,
            obb: Obb {
                center: [0.0, 0.0, 0.0],
                half_size: [1.0, 1.0, 1.0],
                quaternion: [0.0, 0.0, 0.0, 1.0],
            },
            lod_threshold: Some(lod),
            ..Default::default()
        }
    }

    #[test]
    fn paged_tree_children_and_lod() {
        let page = NodePage {
            nodes: vec![
                make_node(Some(vec![1, 2]), 100.0),
                make_node(None, 200.0),
                make_node(None, 200.0),
            ],
        };
        let tree = NodeTree::Paged(PagedNodeTree {
            node_pages: vec![Some(page)],
            nodes_per_page: 64,
        });
        let mut kids = Vec::new();
        tree.children_of(0, &mut kids);
        assert_eq!(kids, vec![1, 2]);

        assert!(tree.obb_of(0).is_some());
        assert!((tree.lod_threshold_of(0) - 100.0).abs() < 1e-6);
        assert!((tree.lod_threshold_of(1) - 200.0).abs() < 1e-6);
    }

    #[test]
    fn pointcloud_tree_contiguous_children() {
        let page = PointCloudNodePageDefinition {
            nodes: vec![
                make_pcnode(1, 3, 500.0),
                make_pcnode(-1, 0, 0.0),
                make_pcnode(-1, 0, 0.0),
                make_pcnode(-1, 0, 0.0),
            ],
        };
        let tree = NodeTree::PointCloud(PointCloudNodeTree {
            node_pages: vec![Some(page)],
            nodes_per_page: 64,
        });
        let mut kids = Vec::new();
        tree.children_of(0, &mut kids);
        assert_eq!(kids, vec![1, 2, 3]);

        assert!((tree.lod_threshold_of(0) - 500.0).abs() < 1e-6);
        assert!(tree.page_loaded(0));
        assert!(!tree.page_loaded(1));
    }

    #[test]
    fn node_count_across_pages() {
        let page0 = NodePage {
            nodes: vec![make_node(None, 0.0); 64],
        };
        let page1 = NodePage {
            nodes: vec![make_node(None, 0.0); 10],
        };
        let tree = NodeTree::Paged(PagedNodeTree {
            node_pages: vec![Some(page0), Some(page1)],
            nodes_per_page: 64,
        });
        assert_eq!(tree.node_count(), 74);
    }
}
