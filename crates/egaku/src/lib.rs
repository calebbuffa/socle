//! Two-phase renderer resource preparation for tile format loaders.
//!
//! Mirrors cesium-native's `IPrepareRendererResources`: a worker-thread
//! decode phase followed by a main-thread GPU-upload phase.  The split
//! lets the load pipeline saturate worker threads with parsing while
//! keeping GPU-API calls confined to the render thread.
//!
//! # Relationship to the load pipeline
//!
//! A format-specific `ContentLoader` drives the two phases in sequence:
//!
//! ```text
//! AssetAccessor::get(url)          ← worker thread (orkester-io)
//!   → PrepareRendererResources::prepare_in_load_thread(model)
//!                                  ← worker thread (decode / decompress)
//!   → PrepareRendererResources::prepare_in_main_thread(worker_result)
//!                                  ← main thread   (GPU upload)
//!   → Content                      ← stored in the selection engine
//! ```
//!
//! The loader owns the bytes→[`GltfModel`] step. The renderer owns
//! [`GltfModel`]→`WorkerResult`→`Content`.

use moderu::GltfModel;

/// Two-phase renderer resource preparation.
///
/// Implement this trait to bridge the tile-loading pipeline and your
/// rendering backend. The engine guarantees:
///
/// * [`prepare_in_load_thread`] is called on a worker thread — safe to do
///   blocking CPU work (parsing, decompression, mesh building).
/// * [`prepare_in_main_thread`] is called on the main thread — safe to call
///   GPU APIs. Must not block.
///
/// # Type parameters
///
/// * `WorkerResult` — CPU-side intermediate produced by the worker phase
///   and consumed by the main-thread phase.
/// * `Content` — Final render-ready value delivered to the selection engine.
///
/// [`prepare_in_load_thread`]: PrepareRendererResources::prepare_in_load_thread
/// [`prepare_in_main_thread`]: PrepareRendererResources::prepare_in_main_thread
pub trait PrepareRendererResources: Send + Sync + 'static {
    /// CPU-side decode result passed from the worker phase to the main-thread phase.
    type WorkerResult: Send + 'static;

    /// Final render-ready content stored by the engine.
    type Content: Send + 'static;

    /// Error returned by either phase.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Worker-thread phase.
    ///
    /// Decode `model` into CPU-side geometry, textures, and metadata.
    /// Do **not** call any GPU API here.
    fn prepare_in_load_thread(&self, model: GltfModel) -> Result<Self::WorkerResult, Self::Error>;

    /// Main-thread phase.
    ///
    /// Upload CPU-side data to the GPU and return the final renderable
    /// content.  Must not block — do all heavy work in
    /// [`prepare_in_load_thread`] instead.
    ///
    /// [`prepare_in_load_thread`]: PrepareRendererResources::prepare_in_load_thread
    fn prepare_in_main_thread(&self, worker_result: Self::WorkerResult) -> Self::Content;

    /// Release a worker-phase result that will never reach the main thread.
    ///
    /// Called when a tile is evicted after the worker phase finished but before
    /// [`prepare_in_main_thread`] ran (mid-pipeline eviction). The default
    /// implementation drops `worker_result` normally; override if CPU-side
    /// resources need explicit cleanup.
    ///
    /// [`prepare_in_main_thread`]: PrepareRendererResources::prepare_in_main_thread
    fn free_worker_result(&self, _worker_result: Self::WorkerResult) {}

    /// Release main-thread GPU resources for an evicted tile.
    ///
    /// Called on the main thread when the engine evicts a resident tile to stay
    /// within `max_cached_bytes`. The default implementation drops `content`
    /// normally; override to destroy GPU buffers, bind groups, etc.
    fn free(&self, _content: Self::Content) {}
}
