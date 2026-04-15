//! `kiban` — streaming spatial data platform.
//!
//! Composes [`selekt`] (selection engine), [`kasane`] (overlay engine), and
//! format adapters (e.g. [`tiles3d_selekt`]) into a single runtime.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │  Format adapter (3D Tiles, i3s, custom) │
//! │  Produces: NodeDescriptors, Loader      │
//! ├─────────────────────────────────────────┤
//! │  kiban                                  │
//! │  ├─ selekt::NodeStore + SelectionState  │
//! │  ├─ selekt::select() (pure function)    │
//! │  ├─ kasane::OverlayEngine               │
//! │  └─ Content loading & lifecycle         │
//! ├─────────────────────────────────────────┤
//! │  Your renderer (handle events, draw)    │
//! └─────────────────────────────────────────┘
//! ```

mod async_runtime;
mod content;
mod content_cache;
mod event;
mod runtime;

pub use async_runtime::AsyncRuntime;
pub use content::{
    ContentAddress, ContentKind, ContentLoadRequest, ContentLoadResult, ContentLoadResultState,
    ContentLoader, ContentManager, ContentOptions, LodErrorDescriptor, Node, NodeLoadStatus,
    NodeRefine, NodeTransform, SlabIndex, UnloadContentResult,
};
pub use event::Event;
pub use runtime::{
    FadeState, Kiban, MainThreadEvent, OverlayAttachEvent, OverlayLifecycleEvent, RenderNode,
    Stratum, StratumOptions,
};

// Re-export URI utilities from outil for convenience.
pub use outil::{Uri, file_extension, resolve_url};

// Re-export key types from sub-crates so users can depend on kiban alone.
pub use egaku::{ContentPipeline, PipelineError};
pub use kasane::{OverlayEngine, OverlayEvent, OverlayId, RasterOverlay, RasterOverlayTile};
pub use selekt::{LodEvaluator, NodeId, NodeStore, SelectionOptions, ViewState};
