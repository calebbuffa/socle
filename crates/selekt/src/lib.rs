//! `selekt` — format-agnostic 3D spatial-hierarchy selection engine.
//!
//! # Quick start
//!
//! ```ignore
//! // 1. Implement the two required traits for your format:
//! //    SceneGraph + ContentLoader
//!
//! let mut engine = SelectionEngineBuilder::new(bg_context, hierarchy, lod, loader)
//!     .build();
//!
//! // 2. Per frame:
//! let handle = engine.add_view_group(1.0);
//! engine.update_view_group(handle, &[view]);
//! engine.load();
//! let result = engine.view_group_result(handle).unwrap();
//! for &node_id in &result.nodes_to_render {
//!     if let Some(content) = engine.content(node_id) { /* render */ }
//! }
//! ```
//!
//! # Trait interfaces (implement these for your format)
//!
//! - [`SceneGraph`] — describe the node hierarchy
//! - [`LodEvaluator`] — refinement decision per node
//! - [`ContentLoader`] — fetch node content asynchronously; returns [`NodeContent`]
//! - [`Policy`] = [`VisibilityPolicy`] + [`ResidencyPolicy`] — culling and eviction

mod composite;
mod engine;
mod engine_state;
pub(crate) mod evaluators;
mod format;
mod frame;
mod hierarchy;
mod load;
mod lod;
mod lod_threshold;
mod node;
mod options;
mod policy;
mod query;
mod scheduler;
pub(crate) mod step;
pub(crate) mod traversal;
mod view;

// Engine and builder
pub use engine::{SelectionEngine, SelectionEngineBuilder};

// Options
pub use options::{
    ClippingPlane, CullingOptions, DebugOptions, LoadingOptions, LodRefinementOptions,
    SelectionOptions, StreamingOptions,
};

// Node identity and lifecycle
pub use node::{NodeId, NodeKind, NodeLoadState, NodeRefinementResult};

// LOD threshold
pub use lod_threshold::LodThreshold;

// LOD evaluation
pub use lod::{LodDescriptor, LodEvaluator, LodFamily, RefinementMode};

// Spatial query
pub use query::{QueryDepth, QueryShape};

// Content loading
pub use load::{
    ContentKey, ContentLoader, DynContentLoader, LoadFailureDetails, LoadFailureType,
    LoadPassResult, LoadPriority, NodeContent, PriorityGroup, SceneRef,
};

// Spatial hierarchy
pub use hierarchy::SceneGraph;

// View state and handle
pub use view::{Projection, ViewGroupHandle, ViewState, ViewUpdateResult};

// Frame result and render node
pub use frame::{FrameResult, PickResult, RenderNode};

// Policy
pub use policy::{
    AllVisibleLruPolicy, CompositeExcluder, DefaultPolicy, FrustumVisibilityPolicy,
    LruResidencyPolicy, NoOcclusion, NodeExcluder, OcclusionState, OcclusionTester, Policy,
    ResidencyPolicy, VisibilityPolicy,
};
