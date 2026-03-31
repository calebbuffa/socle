//! `OverlayEngine<P>` — a [`selekt::SelectionEngine`] augmented with raster overlay draping.
//!
//! `OverlayEngine` wraps a `selekt::SelectionEngine<P::Content>` and adds
//! raster overlay lifecycle management:
//!
//! - Overlay tile providers are initialized asynchronously when an overlay is added.
//! - Each frame, tiles that appear in the render set have overlay tiles fetched.
//! - [`OverlayEngine::render_nodes`] returns a fully declarative per-frame render list
//!   — each node carries its content *and* all currently-ready overlay tiles.
//!   The renderer re-binds exactly what is active this frame; no attach/detach
//!   callbacks are required.
//!
//! Geographic extents are read directly from
//! [`selekt::SpatialHierarchy::geographic_extent`] — no caller-supplied closure needed.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use egaku::PrepareRendererResources;
use glam::DMat4;
use orkester::Task;
use orkester_io::AssetAccessor;
use selekt::{FrameResult, NodeId, ViewState};

use crate::overlay::{OverlayCollection, OverlayId, RasterOverlayTile, RasterOverlayTileProvider};

/// Default texel density (texels per radian) used when choosing overlay zoom levels.
///
/// Corresponds to one 256-pixel tile covering a quarter of the globe (45°) at
/// zoom level 0: `256 / (π/4) ≈ 327 texels/radian`.
pub const DEFAULT_TARGET_TEXELS_PER_RADIAN: f64 = 256.0 / (std::f64::consts::PI / 4.0);

/// A pending or active tile provider for a single raster overlay.
enum ProviderState {
    /// Still initializing (async task running).
    Pending(Task<Box<dyn RasterOverlayTileProvider>>),
    /// Ready for use.
    Active(Arc<dyn RasterOverlayTileProvider>),
}

/// Tracks pending and attached overlay tiles for a single geometry tile.
#[derive(Default)]
struct TileOverlayState {
    /// In-flight fetch tasks: (overlay_id, tile_coords) -> task.
    pending: HashMap<(OverlayId, (u32, u32, u32)), Task<RasterOverlayTile>>,
    /// Successfully fetched overlays: overlay_id -> (uv_set_index, tile).
    attached: HashMap<OverlayId, (u32, RasterOverlayTile)>,
}

/// A single render-ready node yielded by [`OverlayEngine::render_nodes`].
///
/// Carries the geometry tile, its world transform, and the set of overlay
/// tiles currently ready for draping. The renderer re-binds this state every
/// frame — no attach/detach callbacks are needed.
pub struct OverlayRenderNode<'a, C> {
    /// The node's identity within the hierarchy.
    pub id: NodeId,
    /// Accumulated world-space transform for this node.
    pub world_transform: DMat4,
    /// Renderer-owned GPU content for this node.
    pub content: &'a C,
    /// Overlay tiles currently ready for draping.
    ///
    /// Each entry is `(uv_set_index, overlay_tile)`. `uv_set_index` is stable
    /// for the lifetime of the overlay registration (position in the
    /// `add_overlay` call order).
    pub overlays: Vec<(u32, &'a RasterOverlayTile)>,
}

/// High-level tile streaming handle with raster overlay support.
///
/// Wraps a [`selekt::SelectionEngine<P::Content>`] and drives overlay draping
/// through a declarative per-frame [`render_nodes`](Self::render_nodes) iterator.
///
/// # Type parameters
/// - `P` — content preparer implementing [`PrepareRendererResources`];
///   `P::Content` is the decoded tile content type stored by the engine.
pub struct OverlayEngine<P: PrepareRendererResources> {
    /// Inner engine — drives traversal, loading, and main-thread finalization.
    inner: selekt::SelectionEngine<P::Content>,
    /// Content preparer shared with the overlay system.
    preparer: Arc<P>,
    /// Shared asset accessor for constructing overlay tile providers.
    accessor: Arc<dyn AssetAccessor>,
    /// User-facing overlay collection. Drives provider initialization.
    collection: OverlayCollection,
    /// Provider state per overlay, keyed by `OverlayId`.
    providers: HashMap<OverlayId, ProviderState>,
    /// Overlay state per geometry tile.
    tile_state: HashMap<NodeId, TileOverlayState>,
    /// Tiles that were rendered last frame (used to detect appear/disappear).
    prev_rendered: HashSet<NodeId>,
    /// Stable ordered list of overlay ids (for UV-set index assignment).
    overlay_order: Vec<OverlayId>,
    /// Texel density hint (texels per radian) passed to
    /// [`RasterOverlayTileProvider::tiles_for_extent`].
    target_texels_per_radian: f64,
}

impl<P: PrepareRendererResources> OverlayEngine<P> {
    /// Create an `OverlayEngine` wrapping an existing [`selekt::SelectionEngine`].
    ///
    /// Geographic extents are read from
    /// [`selekt::SpatialHierarchy::geographic_extent`] automatically.
    /// No extra closure is required.
    pub fn new(
        engine: selekt::SelectionEngine<P::Content>,
        preparer: Arc<P>,
        accessor: Arc<dyn AssetAccessor>,
    ) -> Self {
        Self {
            inner: engine,
            preparer,
            accessor,
            collection: OverlayCollection::new(),
            providers: HashMap::new(),
            tile_state: HashMap::new(),
            prev_rendered: HashSet::new(),
            overlay_order: Vec::new(),
            target_texels_per_radian: DEFAULT_TARGET_TEXELS_PER_RADIAN,
        }
    }

    /// Set the texel density hint used when choosing overlay zoom levels.
    pub fn set_target_texels_per_radian(&mut self, v: f64) {
        self.target_texels_per_radian = v;
    }
}

impl<P: PrepareRendererResources> OverlayEngine<P> {
    /// Add a raster overlay. Returns an `OverlayId` for later removal.
    ///
    /// The overlay's tile provider is created asynchronously; draping starts
    /// as soon as the provider is ready.
    pub fn add_overlay(
        &mut self,
        overlay: impl crate::overlay::RasterOverlay + 'static,
    ) -> OverlayId {
        let runtime = self.inner.runtime();
        let id = self.collection.add(overlay);
        if let Some((_, raw)) = self.collection.iter().find(|(oid, _)| *oid == id) {
            let task = raw.create_tile_provider(runtime, &self.accessor);
            self.providers.insert(id, ProviderState::Pending(task));
        }
        self.overlay_order.push(id);
        id
    }

    /// Remove a raster overlay. Tiles that had it attached will simply stop
    /// yielding it from [`render_nodes`](Self::render_nodes) next frame.
    pub fn remove_overlay(&mut self, id: OverlayId) {
        self.collection.remove(id);
        self.providers.remove(&id);
        for state in self.tile_state.values_mut() {
            state.attached.remove(&id);
            state.pending.retain(|(oid, _), _| *oid != id);
        }
        self.overlay_order.retain(|&oid| oid != id);
    }

    /// Run one frame: traversal → load dispatch → main-thread finalization → overlay fetch drain.
    ///
    /// `delta_time` is elapsed seconds since the previous call; passed through to
    /// [`FrameResult::delta_time_seconds`].
    pub fn update(&mut self, views: &[ViewState], delta_time: f32) -> &FrameResult {
        // 1. Drain completed provider init tasks.
        self.drain_pending_providers();

        // 2. Inner engine update (traversal + load + GPU upload).
        self.inner.update(views, delta_time);

        // 3. Compute tile set delta.
        let current: HashSet<NodeId> = self
            .inner
            .last_result()
            .nodes_to_render
            .iter()
            .copied()
            .collect();

        // 4. Tiles that disappeared: drop overlay state.
        let disappeared: Vec<NodeId> = self.prev_rendered.difference(&current).copied().collect();
        for node_id in disappeared {
            self.tile_state.remove(&node_id);
        }

        // 5. Tiles that appeared or are still visible: dispatch overlay tile fetches.
        for &node_id in &current {
            self.dispatch_overlay_fetches(node_id);
        }

        // 6. Drain completed tile fetch tasks.
        self.drain_pending_tiles();

        // 7. Update previous frame set.
        self.prev_rendered = current;

        self.inner.last_result()
    }

    /// Iterate over render-ready nodes with their overlay tiles.
    ///
    /// Returns one [`OverlayRenderNode`] per node in the last frame's render set
    /// that has loaded content. Each node carries its world transform and the
    /// set of overlay tiles currently ready. The renderer should rebind all
    /// listed overlays every frame — binding is idempotent and avoids the need
    /// for stateful attach/detach callbacks.
    pub fn render_nodes(&self) -> impl Iterator<Item = OverlayRenderNode<'_, P::Content>> {
        self.inner
            .last_result()
            .nodes_to_render
            .iter()
            .filter_map(|&id| {
                let content = self.inner.content(id)?;
                let world_transform = self.inner.hierarchy().world_transform(id);
                let overlays = self
                    .tile_state
                    .get(&id)
                    .map(|state| {
                        state
                            .attached
                            .iter()
                            .map(|(_, (uv, tile))| (*uv, tile))
                            .collect()
                    })
                    .unwrap_or_default();
                Some(OverlayRenderNode {
                    id,
                    world_transform,
                    content,
                    overlays,
                })
            })
    }

    /// Access the underlying inner engine.
    pub fn inner(&self) -> &selekt::SelectionEngine<P::Content> {
        &self.inner
    }

    /// Mutable access to the underlying engine.
    pub fn inner_mut(&mut self) -> &mut selekt::SelectionEngine<P::Content> {
        &mut self.inner
    }

    /// Access the content preparer.
    pub fn preparer(&self) -> &Arc<P> {
        &self.preparer
    }

    /// Access the overlay collection for inspection.
    pub fn overlays(&self) -> &OverlayCollection {
        &self.collection
    }

    // ── private helpers ──────────────────────────────────────────────────────

    /// Drain completed provider-init tasks and promote them to `Active`.
    fn drain_pending_providers(&mut self) {
        let ready_ids: Vec<OverlayId> = self
            .providers
            .iter()
            .filter_map(|(&id, state)| match state {
                ProviderState::Pending(task) if task.is_ready() => Some(id),
                _ => None,
            })
            .collect();

        for id in ready_ids {
            if let Some(ProviderState::Pending(task)) = self.providers.remove(&id) {
                let provider = task.block().unwrap_or_else(|_| Box::new(DummyProvider));
                self.providers
                    .insert(id, ProviderState::Active(Arc::from(provider)));
            }
        }
    }

    /// Dispatch overlay tile fetch tasks for all active providers covering `node_id`.
    fn dispatch_overlay_fetches(&mut self, node_id: NodeId) {
        let geo_rect = match self.inner.hierarchy().geographic_extent(node_id) {
            Some(r) => r,
            None => return,
        };

        let state = self.tile_state.entry(node_id).or_default();
        let target = self.target_texels_per_radian;

        for (&overlay_id, provider_state) in &self.providers {
            let provider = match provider_state {
                ProviderState::Active(p) => Arc::clone(p),
                ProviderState::Pending(_) => continue,
            };
            if state.attached.contains_key(&overlay_id) {
                continue;
            }
            let tile_coords = provider.tiles_for_extent(geo_rect, target);
            for coords in tile_coords {
                let key = (overlay_id, coords);
                if state.pending.contains_key(&key) {
                    continue;
                }
                let (x, y, level) = coords;
                let task = provider.get_tile(x, y, level);
                state.pending.insert(key, task);
            }
        }
    }

    /// Drain completed tile fetch tasks and record them in `attached`.
    fn drain_pending_tiles(&mut self) {
        let mut completions: Vec<(NodeId, OverlayId, RasterOverlayTile)> = Vec::new();

        for (&node_id, state) in &mut self.tile_state {
            let ready_keys: Vec<(OverlayId, (u32, u32, u32))> = state
                .pending
                .iter()
                .filter_map(|(k, task)| if task.is_ready() { Some(*k) } else { None })
                .collect();
            for key in ready_keys {
                if let Some(task) = state.pending.remove(&key) {
                    if let Ok(tile) = task.block() {
                        completions.push((node_id, key.0, tile));
                    }
                }
            }
        }

        let overlay_order = &self.overlay_order;
        for (node_id, overlay_id, tile) in completions {
            let uv_index = overlay_order_index(overlay_order, overlay_id);
            if let Some(state) = self.tile_state.get_mut(&node_id) {
                // V-flip pixels from web top-down to GL bottom-up convention.
                let flipped = vflip_rgba(&tile.pixels, tile.width, tile.height);
                let flipped_tile = RasterOverlayTile { pixels: flipped, ..tile };
                state.attached.insert(overlay_id, (uv_index, flipped_tile));
            }
        }
    }
}

fn overlay_order_index(order: &[OverlayId], id: OverlayId) -> u32 {
    order.iter().position(|&oid| oid == id).unwrap_or(0) as u32
}

/// Flip pixel rows from top-down (web convention) to bottom-up (GL convention).
fn vflip_rgba(pixels: &[u8], width: u32, height: u32) -> Arc<[u8]> {
    let stride = (width as usize) * 4;
    let mut out = vec![0u8; pixels.len()];
    for row in 0..height as usize {
        let src_start = row * stride;
        let dst_start = (height as usize - 1 - row) * stride;
        out[dst_start..dst_start + stride].copy_from_slice(&pixels[src_start..src_start + stride]);
    }
    Arc::from(out)
}

/// Minimal no-op provider used as a placeholder during `drain_pending_providers`.
struct DummyProvider;

impl RasterOverlayTileProvider for DummyProvider {
    fn get_tile(&self, _x: u32, _y: u32, _level: u32) -> Task<RasterOverlayTile> {
        unreachable!("DummyProvider::get_tile should never be called")
    }
    fn bounds(&self) -> terra::GlobeRectangle {
        terra::GlobeRectangle::new(0.0, 0.0, 0.0, 0.0)
    }
    fn maximum_level(&self) -> u32 {
        0
    }
    fn tiles_for_extent(&self, _: terra::GlobeRectangle, _: f64) -> Vec<(u32, u32, u32)> {
        vec![]
    }
}
//!
//! `OverlayEngine` wraps a `selekt::SelectionEngine<P::Content>` (where `P` implements
//! [`OverlayablePreparer`]) and adds raster overlay lifecycle management:
//!
//! - Overlay tile providers are initialized asynchronously when an overlay is added.
//! - Each frame, tiles that newly appear in the render set have overlay tiles
//!   fetched and attached via [`OverlayablePreparer::attach_raster`].
//! - Tiles that disappear from the render set have their overlays detached via
//!   [`OverlayablePreparer::detach_raster`].
//!
//! # Geographic bounds injection
//!
//! Computing which overlay tile coordinates to request requires knowing each
//! tile's geographic rectangle. Because `selekt` works in ECEF and formats
//! vary in how they store geographic extents, the caller supplies a
//! `geo_bounds` closure at construction time.
//!
//! Format-specific helper functions (e.g., `tiles3d_selekt::geo_rectangle_for_node`)
//! convert `SpatialBounds` to `GlobeRectangle` and can be used directly as
//! the closure argument.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use orkester::Task;
use orkester_io::AssetAccessor;
use selekt::{FrameResult, NodeId, ViewState};

use crate::overlay::{OverlayCollection, OverlayId, RasterOverlayTile, RasterOverlayTileProvider};
use crate::preparer::OverlayablePreparer;

/// Default texel density (texels per radian) used when choosing overlay zoom levels.
///
/// Corresponds to one 256-pixel tile covering a quarter of the globe (45°) at
/// zoom level 0: `256 / (π/4) ≈ 327 texels/radian`.
pub const DEFAULT_TARGET_TEXELS_PER_RADIAN: f64 = 256.0 / (std::f64::consts::PI / 4.0);

/// A pending or active tile provider for a single raster overlay.
enum ProviderState {
    /// Still initializing (async task running).
    Pending(Task<Box<dyn RasterOverlayTileProvider>>),
    /// Ready for use.
    Active(Arc<dyn RasterOverlayTileProvider>),
}

/// Tracks pending and attached overlay tiles for a single geometry tile.
#[derive(Default)]
struct TileOverlayState {
    /// In-flight fetch tasks: (overlay_id, tile_coords) -> task.
    pending: HashMap<(OverlayId, (u32, u32, u32)), Task<RasterOverlayTile>>,
    /// Successfully attached overlays: overlay_id -> uv_set_index.
    attached: HashMap<OverlayId, u32>,
}

/// High-level tile streaming handle with raster overlay support.
///
/// Wraps a [`selekt::SelectionEngine<P::Content>`] and drives overlay attach/detach based on
/// which tiles enter and leave the render set each frame.
///
/// # Type parameters
/// - `P` — content preparer implementing [`OverlayablePreparer`]; `P::Content` is the
///   decoded tile content type stored by the engine.
pub struct OverlayEngine<P: OverlayablePreparer> {
    /// Inner engine — drives traversal, loading, and main-thread finalization.
    inner: selekt::SelectionEngine<P::Content>,
    /// Content preparer shared with the overlay system.
    preparer: Arc<P>,
    /// Shared asset accessor for constructing overlay tile providers.
    accessor: Arc<dyn AssetAccessor>,
    /// User-facing overlay collection. Drives provider initialization.
    collection: OverlayCollection,
    /// Provider state per overlay, keyed by `OverlayId`.
    providers: HashMap<OverlayId, ProviderState>,
    /// Overlay state per geometry tile.
    tile_state: HashMap<NodeId, TileOverlayState>,
    /// Tiles that were rendered last frame (used to detect appear/disappear).
    prev_rendered: HashSet<NodeId>,
    /// Stable ordered list of overlay ids (for UV-set index assignment).
    overlay_order: Vec<OverlayId>,
    /// Projects a tile's `NodeId` to its geographic rectangle.
    ///
    /// Returns `None` if the tile has no usable geographic extent (e.g., the root
    /// of a tileset whose bounding volume is a sphere and the caller cannot provide
    /// a tighter estimate). In that case, overlay draping is skipped for that tile.
    geo_bounds: Arc<dyn Fn(NodeId) -> Option<terra::GlobeRectangle> + Send + Sync>,
    /// Texel density hint (texels per radian) passed to
    /// [`RasterOverlayTileProvider::tiles_for_extent`].
    target_texels_per_radian: f64,
}

impl<P: OverlayablePreparer> OverlayEngine<P> {
    /// Create an `OverlayEngine` wrapping an existing [`selekt::SelectionEngine`].
    ///
    /// `geo_bounds` converts a `NodeId` to a `GlobeRectangle` (in radians).
    /// Returning `None` disables overlay draping for that tile.
    ///
    /// `target_texels_per_radian` controls the tile zoom level chosen for draping;
    /// a value of `256.0 / (PI / 4.0)` ≈ 327 corresponds to one 256-pixel tile
    /// covering a quarter of the globe at level 0.
    pub fn new(
        engine: selekt::SelectionEngine<P::Content>,
        preparer: Arc<P>,
        accessor: Arc<dyn AssetAccessor>,
        geo_bounds: impl Fn(NodeId) -> Option<terra::GlobeRectangle> + Send + Sync + 'static,
    ) -> Self {
        Self {
            inner: engine,
            preparer,
            accessor,
            collection: OverlayCollection::new(),
            providers: HashMap::new(),
            tile_state: HashMap::new(),
            prev_rendered: HashSet::new(),
            overlay_order: Vec::new(),
            geo_bounds: Arc::new(geo_bounds),
            target_texels_per_radian: DEFAULT_TARGET_TEXELS_PER_RADIAN,
        }
    }

    /// Set the texel density hint used when choosing overlay zoom levels.
    pub fn set_target_texels_per_radian(&mut self, v: f64) {
        self.target_texels_per_radian = v;
    }
}

impl<P: OverlayablePreparer> OverlayEngine<P> {
    /// Add a raster overlay. Returns an `OverlayId` for later removal.
    ///
    /// The overlay's tile provider is created asynchronously; draping starts
    /// as soon as the provider is ready.
    pub fn add_overlay(
        &mut self,
        overlay: impl crate::overlay::RasterOverlay + 'static,
    ) -> OverlayId {
        let runtime = self.inner.runtime();
        let id = self.collection.add(overlay);
        // Find the overlay we just added and kick off provider initialization.
        if let Some((_, raw)) = self.collection.iter().find(|(oid, _)| *oid == id) {
            let task = raw.create_tile_provider(runtime, &self.accessor);
            self.providers.insert(id, ProviderState::Pending(task));
        }
        self.overlay_order.push(id);
        id
    }

    /// Remove a raster overlay and detach it from all currently-rendered tiles.
    pub fn remove_overlay(&mut self, id: OverlayId) {
        self.collection.remove(id);
        self.providers.remove(&id);
        // Detach from all currently-rendered tiles.
        let uv_index = self
            .overlay_order
            .iter()
            .position(|&oid| oid == id)
            .unwrap_or(0) as u32;
        let preparer = Arc::clone(&self.preparer);
        let engine = &mut self.inner;
        for node_id in self.prev_rendered.iter().copied() {
            if let Some(state) = self.tile_state.get_mut(&node_id) {
                if state.attached.remove(&id).is_some() {
                    if let Some(content) = engine.content_mut(node_id) {
                        preparer.detach_raster(node_id, uv_index, content);
                    }
                }
                state.pending.retain(|(oid, _), _| *oid != id);
            }
        }
        self.overlay_order.retain(|&oid| oid != id);
    }

    /// Run one frame: traversal → load dispatch → main-thread finalization → overlay draping.
    ///
    /// `delta_time` is elapsed seconds since the previous call; passed through to [`FrameResult::delta_time_seconds`].
    pub fn update(&mut self, views: &[ViewState], delta_time: f32) -> &FrameResult {
        // 1. Drain completed provider init tasks.
        self.drain_pending_providers();

        // 2. Inner stratum update (traversal + load + GPU upload).
        self.inner.update(views, delta_time);

        // 3. Compute tile set delta.
        let current: HashSet<NodeId> = self
            .inner
            .last_result()
            .nodes_to_render
            .iter()
            .copied()
            .collect();

        // 4. Tiles that disappeared: detach overlays and drop state.
        let disappeared: Vec<NodeId> = self.prev_rendered.difference(&current).copied().collect();
        for node_id in disappeared {
            self.detach_all(node_id);
            self.tile_state.remove(&node_id);
        }

        // 5. Tiles that appeared: dispatch overlay tile fetches.
        let appeared: Vec<NodeId> = current.difference(&self.prev_rendered).copied().collect();
        for node_id in appeared {
            self.dispatch_overlay_fetches(node_id);
        }

        // 6. Drain completed tile fetch tasks and call attach_raster.
        self.drain_pending_tiles();

        // 7. Update previous frame set.
        self.prev_rendered = current;

        self.inner.last_result()
    }

    /// Access the underlying inner stratum.
    pub fn inner(&self) -> &selekt::SelectionEngine<P::Content> {
        &self.inner
    }

    /// Mutable access to the underlying engine.
    pub fn inner_mut(&mut self) -> &mut selekt::SelectionEngine<P::Content> {
        &mut self.inner
    }

    /// Access the content preparer.
    pub fn preparer(&self) -> &Arc<P> {
        &self.preparer
    }

    /// Access the overlay collection for inspection.
    pub fn overlays(&self) -> &OverlayCollection {
        &self.collection
    }

    // ── private helpers ──────────────────────────────────────────────────────

    /// Drain completed provider-init tasks and promote them to `Active`.
    fn drain_pending_providers(&mut self) {
        let ready_ids: Vec<OverlayId> = self
            .providers
            .iter()
            .filter_map(|(&id, state)| match state {
                ProviderState::Pending(task) if task.is_ready() => Some(id),
                _ => None,
            })
            .collect();

        for id in ready_ids {
            if let Some(ProviderState::Pending(task)) = self.providers.remove(&id) {
                let provider = task.block().unwrap_or_else(|_| Box::new(DummyProvider));
                self.providers
                    .insert(id, ProviderState::Active(Arc::from(provider)));
            }
        }
    }

    /// Dispatch overlay tile fetch tasks for all active providers covering `node_id`.
    fn dispatch_overlay_fetches(&mut self, node_id: NodeId) {
        let geo_rect = match (self.geo_bounds)(node_id) {
            Some(r) => r,
            None => return,
        };

        let state = self.tile_state.entry(node_id).or_default();
        let target = self.target_texels_per_radian;

        for (&overlay_id, provider_state) in &self.providers {
            let provider = match provider_state {
                ProviderState::Active(p) => Arc::clone(p),
                ProviderState::Pending(_) => continue, // will retry when provider is ready
            };
            if state.attached.contains_key(&overlay_id) {
                continue; // already attached
            }
            let tile_coords = provider.tiles_for_extent(geo_rect, target);
            // For each required overlay tile, dispatch an async fetch task.
            for coords in tile_coords {
                let key = (overlay_id, coords);
                if state.pending.contains_key(&key) {
                    continue; // already in flight
                }
                let (x, y, level) = coords;
                let task = provider.get_tile(x, y, level);
                state.pending.insert(key, task);
            }
        }
    }

    /// Drain completed tile fetch tasks and call `attach_raster` on finished ones.
    fn drain_pending_tiles(&mut self) {
        // Phase 1: collect completed tasks into a flat buffer (no engine/preparer access).
        let mut completions: Vec<(NodeId, OverlayId, RasterOverlayTile)> = Vec::new();

        for (&node_id, state) in &mut self.tile_state {
            let ready_keys: Vec<(OverlayId, (u32, u32, u32))> = state
                .pending
                .iter()
                .filter_map(|(k, task)| if task.is_ready() { Some(*k) } else { None })
                .collect();
            for key in ready_keys {
                if let Some(task) = state.pending.remove(&key) {
                    if let Ok(tile) = task.block() {
                        completions.push((node_id, key.0, tile));
                    }
                }
            }
        }

        // Phase 2: apply attach_raster and update attached state.
        let overlay_order = self.overlay_order.clone();
        let preparer = Arc::clone(&self.preparer);
        let engine = &mut self.inner;

        for (node_id, overlay_id, tile) in completions {
            if let Some(content) = engine.content_mut(node_id) {
                let uv_index = overlay_order_index(&overlay_order, overlay_id);
                preparer.attach_raster(node_id, uv_index, &tile, content);
            }
            // Record the attachment regardless of whether content was present
            // (content might be transiently absent mid-frame).
            if let Some(state) = self.tile_state.get_mut(&node_id) {
                let uv_index = overlay_order_index(&overlay_order, overlay_id);
                state.attached.insert(overlay_id, uv_index);
            }
        }
    }

    /// Call `detach_raster` for every overlay attached to `node_id`.
    fn detach_all(&mut self, node_id: NodeId) {
        let Some(state) = self.tile_state.get(&node_id) else {
            return;
        };
        if state.attached.is_empty() {
            return;
        }
        let attached: Vec<(OverlayId, u32)> =
            state.attached.iter().map(|(&k, &v)| (k, v)).collect();
        let preparer = Arc::clone(&self.preparer);
        let engine = &mut self.inner;
        if let Some(content) = engine.content_mut(node_id) {
            for (overlay_id, uv_index) in attached {
                preparer.detach_raster(node_id, uv_index, content);
                let _ = overlay_id; // id already used via uv_index
            }
        }
    }
}

fn overlay_order_index(order: &[OverlayId], id: OverlayId) -> u32 {
    order.iter().position(|&oid| oid == id).unwrap_or(0) as u32
}

/// Minimal no-op provider used as a placeholder during `drain_pending_providers`.
struct DummyProvider;

impl RasterOverlayTileProvider for DummyProvider {
    fn get_tile(&self, _x: u32, _y: u32, _level: u32) -> Task<RasterOverlayTile> {
        unreachable!("DummyProvider::get_tile should never be called")
    }
    fn bounds(&self) -> terra::GlobeRectangle {
        terra::GlobeRectangle::new(0.0, 0.0, 0.0, 0.0)
    }
    fn maximum_level(&self) -> u32 {
        0
    }
    fn tiles_for_extent(&self, _: terra::GlobeRectangle, _: f64) -> Vec<(u32, u32, u32)> {
        vec![]
    }
}
