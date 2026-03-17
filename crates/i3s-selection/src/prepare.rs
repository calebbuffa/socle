//! Engine integration traits for renderer resource preparation.
//!
//! Modeled after cesium-native's `IPrepareRendererResources`. The integration
//! layer (game engine, viewer, etc.) implements this trait to create GPU-ready
//! resources from the decoded I3S node content.
//!
//! The two-phase design splits work between:
//! 1. **Worker thread** (`prepare_in_load_thread`) — CPU-heavy work like
//!    vertex buffer layout conversion, normal generation, texture transcoding.
//! 2. **Main thread** (`prepare_in_main_thread`) — GPU upload, game object
//!    creation, or anything that must happen on the rendering thread.

use i3s_geospatial::crs::CrsTransform;
use std::sync::Arc;

use crate::content::NodeContent;

/// Opaque renderer-specific data attached to a loaded node.
///
/// The integration creates this in `prepare_in_load_thread` and/or
/// `prepare_in_main_thread`. The library stores it alongside `NodeContent`
/// and passes it back when the node is freed.
///
/// Use `Box<dyn std::any::Any + Send>` or a concrete type behind a type alias.
pub type RendererResources = Box<dyn std::any::Any + Send + Sync>;

/// Trait for preparing renderer resources from decoded I3S content.
///
/// This is the Rust equivalent of cesium-native's `IPrepareRendererResources`.
///
/// The library calls:
/// 1. [`prepare_in_load_thread`](Self::prepare_in_load_thread) on a worker thread
///    after node content is fetched and decoded. Do CPU-heavy work here.
/// 2. [`prepare_in_main_thread`](Self::prepare_in_main_thread) on the main thread
///    (inside [`SceneLayer::load_nodes`]). Do GPU uploads and game object creation here.
///    This must be fast — it blocks the frame.
/// 3. [`free`](Self::free) when the node is unloaded from the cache.
///
/// All methods receive the node ID and the decoded [`NodeContent`].
pub trait PrepareRendererResources: Send + Sync {
    /// Prepare resources on a worker thread (CPU-heavy work).
    ///
    /// Called from the [`TaskProcessor`](i3s_async::TaskProcessor) thread pool
    /// after the node's geometry, textures, and attributes are fetched and decoded.
    ///
    /// The `crs_transform` is provided when the layer has a local/projected CRS
    /// and the user supplied a [`CrsTransform`] at open time. Use it to convert
    /// vertex positions and normals from the layer's native CRS to ECEF:
    /// 1. Reconstruct absolute positions: `abs = obb_center + float3(vertex)`
    /// 2. Transform positions to ECEF via `crs_transform.to_ecef()`
    /// 3. For normals: transform tip positions, then re-derive unit normals
    /// 4. Pick a new ECEF origin (e.g. the transformed OBB center)
    /// 5. Re-encode vertices as Float32 offsets from the new origin
    ///
    /// Returns opaque renderer data that will be passed to `prepare_in_main_thread`.
    /// Return `None` if no worker-thread preparation is needed.
    fn prepare_in_load_thread(
        &self,
        node_id: u32,
        content: &NodeContent,
        crs_transform: Option<&Arc<dyn CrsTransform>>,
    ) -> Option<RendererResources>;

    /// Prepare resources on the main thread (GPU upload, game objects).
    ///
    /// Called synchronously during [`SceneLayer::load_nodes`] on the thread
    /// that called it (typically the main/render thread). Must complete quickly.
    ///
    /// `load_thread_result` is the output of `prepare_in_load_thread`.
    /// `crs_transform` is the same transform passed to `prepare_in_load_thread`.
    fn prepare_in_main_thread(
        &self,
        node_id: u32,
        content: &NodeContent,
        load_thread_result: Option<RendererResources>,
        crs_transform: Option<&Arc<dyn CrsTransform>>,
    ) -> Option<RendererResources>;

    /// Free renderer resources when a node is unloaded from the cache.
    ///
    /// Called on the main thread. The `resources` value is whatever was returned
    /// from `prepare_in_main_thread`.
    fn free(&self, node_id: u32, resources: Option<RendererResources>);
}

/// A no-op implementation for headless use (testing, CLI tools, servers).
///
/// Does nothing for all three methods. Useful when you only care about
/// the selection algorithm output and don't need to render anything.
pub struct NoopPrepareRendererResources;

impl PrepareRendererResources for NoopPrepareRendererResources {
    fn prepare_in_load_thread(
        &self,
        _node_id: u32,
        _content: &NodeContent,
        _crs_transform: Option<&Arc<dyn CrsTransform>>,
    ) -> Option<RendererResources> {
        None
    }

    fn prepare_in_main_thread(
        &self,
        _node_id: u32,
        _content: &NodeContent,
        _load_thread_result: Option<RendererResources>,
        _crs_transform: Option<&Arc<dyn CrsTransform>>,
    ) -> Option<RendererResources> {
        None
    }

    fn free(&self, _node_id: u32, _resources: Option<RendererResources>) {}
}
