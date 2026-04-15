//! `selekt` — format-agnostic spatial-hierarchy selection engine.
//!
//! # Architecture
//!
//! selekt is a pure selection library. It decides **which nodes to render**
//! and **which to load** given a spatial hierarchy and camera state. It does
//! not own content, does not perform loading, and has no generics.
//!
//! The orchestrator (e.g. `kiban`) owns a [`NodeStore`] and [`SelectionState`],
//! calls [`select()`] each frame, and acts on the returned [`FrameDecision`].
//!
//! # Quick start
//!
//! ```ignore
//! use selekt::*;
//!
//! let mut store = NodeStore::from_descriptors(&descriptors, 0, 0);
//! let mut state = SelectionState::new();
//! let mut buffers = SelectionBuffers::new();
//!
//! // Per frame:
//! state.advance_frame();
//! let decision = select(
//!     &mut store, &mut state, &options, &views,
//!     &lod_eval, &visibility, &[], &NoOcclusion,
//!     &mut buffers,
//!     &mut |_node, _data| ExpandResult::None,
//! );
//! // decision.render — nodes to draw
//! // decision.load  — content to fetch
//! ```

// Core modules (new architecture)
mod frame_decision;
mod node_store;
mod selection;
mod selection_state;

// Preserved modules
pub(crate) mod evaluators;
mod load;
mod lod;
mod lod_threshold;
mod node;
mod options;
mod policy;
mod query;
mod view;

// ── Public re-exports ────────────────────────────────────────────────────────

// Core types
pub use frame_decision::{EMPTY_FRAME_DECISION, ExpandResult, FrameDecision, LoadRequest};
pub use node_store::{NodeData, NodeDescriptor, NodeStore};
pub use selection::{SelectionBuffers, select};
pub use selection_state::{NodeStatus, SelectionState};

// Node identity and lifecycle
pub use node::{NodeId, NodeKind, NodeLoadState, NodeRefinementResult};

// LOD
pub use lod::{LodDescriptor, LodEvaluator, LodFamily, RefinementMode};
pub use lod_threshold::LodThreshold;

// Options
pub use options::{
    ClippingPlane, CullingOptions, DebugOptions, LoadingOptions, LodRefinementOptions,
    SelectionOptions, StreamingOptions,
};

// View
pub use view::{Projection, ViewState};

// Content key and load priority (kept for kiban to use)
pub use load::{ContentKey, LoadPriority, PriorityGroup};

// Spatial query
pub use query::{QueryDepth, QueryShape};

// Policy
pub use policy::{
    AllVisibleLruPolicy, CompositeExcluder, DefaultPolicy, FrustumVisibilityPolicy,
    LruResidencyPolicy, NoOcclusion, NodeExcluder, OcclusionState, OcclusionTester, Policy,
    ResidencyPolicy, VisibilityPolicy,
};
