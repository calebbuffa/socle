//! `selekt` — format-agnostic 3D tile selection engine.
//!
//! Abstracts cesium-native's `Cesium3DTilesSelection` into idiomatic Rust.
//! Provides the core data structures and traits for LOD-driven tile traversal,
//! async content loading, and GPU resource lifetime management.
//! Format adapters (I3S, Cesium 3D Tiles, etc.) plug in via the trait
//! interfaces.
//!
//! # Main types
//!
//! - [`SelectionEngine`] — the central engine (mirrors `Tileset`)
//! - [`SelectionEngineExternals`] — shared infrastructure (mirrors `TilesetExternals`)
//! - [`SelectionOptions`] — configuration (mirrors `TilesetOptions`)
//!
//! # Trait interfaces
//!
//! - [`SpatialHierarchy`] / [`HierarchyResolver`] — read and extend the tile tree
//! - [`ContentLoader`] — fetch tile data (mirrors `TilesetContentLoader`)
//! - [`ContentLoaderFactory`] — async factory (mirrors `TilesetContentLoaderFactory`)
//! - [`LodEvaluator`] — LOD refinement decision
//! - [`Policy`] = [`VisibilityPolicy`] + [`ResidencyPolicy`] — culling and eviction
//!
//! GPU preparation (`RendererPreparer`) and raster overlay concerns live in
//! the `belag` crate. Selekt is about "what to render", not "make it GPU-ready".

mod engine;
mod factory;
mod hierarchy;
mod load;
mod lod;
mod node;
mod options;
mod policy;
mod scheduler;
pub(crate) mod traversal;
mod view;


// Engine
pub use engine::SelectionEngine;

// Externals & options
pub use options::{SelectionError, SelectionOptions};

// Node identity and lifecycle
pub use node::{NodeId, NodeKind, NodeLifecycleState};

// LOD evaluation
pub use lod::{LodDescriptor, LodEvaluator, RefinementMode};

// Content loading
pub use load::{
    ContentHandle, ContentKey, ContentLoader, HierarchyReference, LoadCandidate, LoadPassResult,
    LoadPriority, LoadedContent, Payload, PriorityGroup, RequestId,
};

// Spatial hierarchy and resolver
pub use hierarchy::{HierarchyPatch, HierarchyPatchError, HierarchyResolver, SpatialHierarchy};

// View state and results
pub use view::{
    PerViewUpdateResult, ViewGroupHandle, ViewGroupOptions, ViewState, ViewUpdateResult,
};

// Factory (async construction)
pub use factory::{ContentLoaderFactory, ContentLoaderFactoryResult};

// Scheduling
pub use scheduler::{LoadScheduler, WeightedFairScheduler};

// Policy
pub use policy::{
    CompositeExcluder, LruResidencyPolicy, NoOcclusion, OcclusionState, OcclusionTester, Policy,
    ResidencyPolicy, TileExcluder, VisibilityPolicy,
};
#[cfg(feature = "glam")]
pub use policy::{DefaultPolicy, FrustumVisibilityPolicy};


use orkester::AsyncSystem;
use orkester_io::AssetAccessor;
use std::sync::{Arc, Mutex};

/// Shared infrastructure for multiple [`SelectionEngine`] instances.
///
/// Mirrors cesium-native's `TilesetExternals`. Create once per application,
/// then pass a reference to each engine. The shared scheduler ensures fair
/// load distribution across all engines.
#[derive(Clone)]
pub struct SelectionEngineExternals {
    /// Async runtime for spawning worker tasks and scheduling main-thread
    /// callbacks. Mirrors `TilesetExternals::asyncSystem`.
    pub async_system: AsyncSystem,
    /// Shared load queue for fair scheduling across all engines.
    pub scheduler: Arc<Mutex<WeightedFairScheduler>>,
    /// Async network I/O. Mirrors `TilesetExternals::pAssetAccessor`.
    /// Format-specific loaders use this to fetch tile data.
    pub asset_accessor: Arc<dyn AssetAccessor>,
}

impl SelectionEngineExternals {
    pub fn new(async_system: AsyncSystem, asset_accessor: Arc<dyn AssetAccessor>) -> Self {
        Self {
            async_system,
            asset_accessor,
            scheduler: Arc::new(Mutex::new(WeightedFairScheduler::new())),
        }
    }

    pub fn reset_scheduler(&mut self) {
        self.scheduler = Arc::new(Mutex::new(WeightedFairScheduler::new()));
    }
}
