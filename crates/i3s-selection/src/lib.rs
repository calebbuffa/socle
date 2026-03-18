//! LOD traversal and view-dependent node selection for I3S scene layers.
//!
//! Given a camera/viewport state, traverses the I3S node tree, evaluates
//! `lodThreshold` (maxScreenThreshold / maxScreenThresholdSQ), frustum-culls
//! invisible nodes, and produces a set of nodes to render, load, or unload.
//!
//! I3S uses **node-switching**: a parent is replaced by its children when the
//! projected screen size of its bounding volume exceeds `lodThreshold`.
//! Parent and children are **never shown simultaneously**.

pub mod building;
pub mod cache;
pub mod content;
pub mod excluder;
pub mod externals;
pub mod layer_info;
pub mod loader;
pub mod node_state;
pub mod node_tree;
pub mod options;
pub mod prepare;
pub mod scene_layer;
pub mod selection;
pub mod update_result;
pub mod view_state;

pub use building::BuildingSceneLayer;
pub use cache::NodeCache;
pub use content::NodeContent;
pub use excluder::NodeExcluder;
pub use externals::SceneLayerExternals;
pub use layer_info::LayerInfo;
pub use loader::{AttributeInfo, LoadResult, NodeContentLoader};
pub use node_state::{NodeLoadState, NodeState};
pub use node_tree::NodeTree;
pub use options::SelectionOptions;
pub use prepare::{NoopPrepareRendererResources, PrepareRendererResources, RendererResources};
pub use scene_layer::{RenderNode, SceneLayer};
pub use selection::LodMetric;
pub use update_result::{LoadPriority, LoadRequest, TraversalStats, ViewUpdateResult};
pub use view_state::ViewState;
