//! Standalone raster overlay engine.
//!
//! [`OverlayEngine`] manages raster overlay lifecycle independently from
//! any tile selection engine. Each frame the caller passes a set of visible
//! node ids and a hierarchy reference; the engine fetches, composites, and
//! caches overlay tiles for those nodes.
//!
//! The design closely follows cesium-native's `RasterMappedTo3DTile`:
//!
//! - Each (geometry-tile, overlay) pair has at most one **ready** raster tile
//!   currently attached and optionally one **loading** higher-resolution tile.
//! - When the loading tile finishes, it replaces the ready tile (detach old,
//!   attach new).
//! - Tile overlay state is **not** destroyed the instant a geometry tile leaves
//!   the render set; it survives for one additional frame so that brief
//!   flickering in the selection engine doesn't cause jarring attach/detach
//!   cycles.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use orkester::Task;
use orkester_io::AssetAccessor;

use crate::event::OverlayEvent;
use crate::hierarchy::OverlayHierarchy;
use crate::overlay::{OverlayCollection, OverlayId, RasterOverlayTile, RasterOverlayTileProvider};

/// Default texel density (texels per radian) used when choosing overlay zoom levels.
pub const DEFAULT_TARGET_TEXELS_PER_RADIAN: f64 = 256.0 / (std::f64::consts::PI / 4.0);

/// Per-node info passed to the overlay engine each frame.
#[derive(Clone, Copy, Debug)]
pub struct OverlayNodeInfo {
    pub node_id: u64,
    /// Geometric error of this tile (metres).  Smaller = more detailed.
    pub geometric_error: f64,
}

/// Viewport / projection info needed to compute per-tile overlay resolution.
#[derive(Clone, Copy, Debug)]
pub struct OverlayViewInfo {
    /// Viewport height in pixels.
    pub viewport_height: f64,
    /// SSE denominator = `2 * tan(fov_y / 2)` for perspective.
    /// For orthographic: `2 * half_height`.
    pub sse_denominator: f64,
    /// Maximum screen-space error threshold (pixels).  Default 16.
    pub maximum_screen_space_error: f64,
}

impl Default for OverlayViewInfo {
    fn default() -> Self {
        Self {
            viewport_height: 768.0,
            sse_denominator: 2.0 * (std::f64::consts::FRAC_PI_4).tan(), // 45° fov
            maximum_screen_space_error: 16.0,
        }
    }
}

// ── Provider bookkeeping ─────────────────────────────────────────────────────

enum ProviderState {
    Pending(Task<Box<dyn RasterOverlayTileProvider>>),
    Active(Arc<dyn RasterOverlayTileProvider>),
}

// ── Per-(node, overlay) attachment state ─────────────────────────────────────

/// Mirrors cesium-native's `RasterMappedTo3DTile::AttachmentState`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum AttachmentState {
    /// No raster tile has been attached yet.
    Unattached,
    /// A coarse ancestor tile is attached while a higher-res replacement loads.
    TemporarilyAttached,
    /// The best-available raster tile is attached.
    Attached,
}

/// State for ONE overlay on ONE geometry tile.
struct MappedOverlay {
    state: AttachmentState,
    /// Currently displayed raster tile (possibly low-res).
    ready: Option<RasterOverlayTile>,
    /// Zoom level of the ready tile (so we can detect upgrades).
    ready_level: u32,
    /// In-flight fetch tasks for the target (possibly higher-res) attachment.
    loading: HashMap<(u32, u32, u32), Task<RasterOverlayTile>>,
    /// Zoom level being loaded.
    loading_level: u32,
}

/// All overlay state for a single geometry tile.
#[derive(Default)]
struct TileOverlayState {
    /// Per-overlay attachment state.
    overlays: HashMap<OverlayId, MappedOverlay>,
}

// ── OverlayEngine ────────────────────────────────────────────────────────────

pub struct OverlayEngine {
    accessor: Arc<dyn AssetAccessor>,
    ctx: orkester::Context,
    collection: OverlayCollection,
    providers: HashMap<OverlayId, ProviderState>,
    tile_state: HashMap<u64, TileOverlayState>,
    /// Tiles rendered in the *previous* frame.
    prev_rendered: HashSet<u64>,
    /// Tiles rendered two frames ago — used for deferred cleanup.
    prev_prev_rendered: HashSet<u64>,
    overlay_order: Vec<OverlayId>,
    target_texels_per_radian: f64,
    view_info: OverlayViewInfo,
    events: Vec<OverlayEvent>,
}

impl OverlayEngine {
    pub fn new(accessor: Arc<dyn AssetAccessor>, ctx: orkester::Context) -> Self {
        Self {
            accessor,
            ctx,
            collection: OverlayCollection::new(),
            providers: HashMap::new(),
            tile_state: HashMap::new(),
            prev_rendered: HashSet::new(),
            prev_prev_rendered: HashSet::new(),
            overlay_order: Vec::new(),
            target_texels_per_radian: DEFAULT_TARGET_TEXELS_PER_RADIAN,
            view_info: OverlayViewInfo::default(),
            events: Vec::new(),
        }
    }

    pub fn set_target_texels_per_radian(&mut self, v: f64) {
        self.target_texels_per_radian = v;
    }

    pub fn set_view_info(&mut self, info: OverlayViewInfo) {
        self.view_info = info;
    }

    pub fn add(&mut self, overlay: impl crate::overlay::RasterOverlay + 'static) -> OverlayId {
        let id = self.collection.add(overlay);
        if let Some((_, raw)) = self.collection.iter().find(|(oid, _)| *oid == id) {
            let task = raw.create_tile_provider(&self.ctx, &self.accessor);
            self.providers.insert(id, ProviderState::Pending(task));
        }
        self.overlay_order.push(id);
        id
    }

    pub fn remove(&mut self, id: OverlayId) {
        self.collection.remove(id);
        self.providers.remove(&id);
        for (&node_id, state) in &mut self.tile_state {
            if state.overlays.remove(&id).is_some() {
                self.events.push(OverlayEvent::Detached {
                    node_id,
                    overlay_id: id,
                });
            }
        }
        self.overlay_order.retain(|&oid| oid != id);
    }

    pub fn len(&self) -> usize {
        self.overlay_order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.overlay_order.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (OverlayId, &dyn crate::overlay::RasterOverlay)> {
        self.collection.iter()
    }

    pub fn for_node(&self, node_id: u64) -> Vec<(u32, &RasterOverlayTile)> {
        self.tile_state
            .get(&node_id)
            .map(|state| {
                state
                    .overlays
                    .iter()
                    .filter_map(|(_, mapped)| {
                        let uv = overlay_order_index(&self.overlay_order, OverlayId(0)); // unused in this path
                        mapped.ready.as_ref().map(|t| (0u32, t))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Returns `true` if all active overlays have been attached to this node.
    ///
    /// When overlays are registered, tiles should not be rendered until their
    /// overlays are ready — the parent tile should stay visible instead.
    /// This mirrors cesium-native's `RasterMappedTo3DTile::update()` returning
    /// `MoreDetailAvailable` to keep parents rendering.
    pub fn is_node_ready(&self, node_id: u64) -> bool {
        // If no overlays registered, everything is ready.
        if self.overlay_order.is_empty() {
            return true;
        }
        // If no active providers yet, nothing can be ready.
        let active_count = self
            .providers
            .values()
            .filter(|p| matches!(p, ProviderState::Active(_)))
            .count();
        if active_count == 0 {
            return false;
        }
        let state = match self.tile_state.get(&node_id) {
            Some(s) => s,
            None => return false,
        };
        // Check that every active overlay has an attached tile for this node.
        for (&overlay_id, provider_state) in &self.providers {
            if !matches!(provider_state, ProviderState::Active(_)) {
                continue;
            }
            match state.overlays.get(&overlay_id) {
                Some(mapped) if mapped.ready.is_some() => {}
                _ => return false,
            }
        }
        true
    }

    /// Run one frame of overlay processing.
    ///
    /// Each entry in `nodes` is `(node_id, geometric_error)`. The geometric
    /// error is used together with `maximum_screen_space_error` to compute
    /// per-tile overlay texel density — matching cesium-native's
    /// `computeDesiredScreenPixels()` formula.
    pub fn update(&mut self, nodes: &[(u64, f64)], hierarchy: &dyn OverlayHierarchy) {
        // 1. Promote pending providers.
        self.drain_pending_providers();

        let current: HashSet<u64> = nodes.iter().map(|(id, _)| *id).collect();

        // 2. Deferred cleanup: tiles gone for TWO consecutive frames get purged.
        //    This prevents flicker from one-frame selection jitter.
        let stale: Vec<u64> = self
            .tile_state
            .keys()
            .copied()
            .filter(|id| !current.contains(id) && !self.prev_rendered.contains(id))
            .collect();
        for node_id in stale {
            if let Some(state) = self.tile_state.remove(&node_id) {
                for (&overlay_id, mapped) in &state.overlays {
                    if mapped.state != AttachmentState::Unattached {
                        self.events.push(OverlayEvent::Detached {
                            node_id,
                            overlay_id,
                        });
                    }
                }
            }
        }

        // 3. For visible tiles, dispatch fetches and process completions.
        for &(node_id, geometric_error) in nodes {
            self.dispatch_for_node(node_id, geometric_error, hierarchy);
        }
        self.process_completions();

        // 4. Rotate frame sets.
        self.prev_prev_rendered = std::mem::take(&mut self.prev_rendered);
        self.prev_rendered = current;
    }

    pub fn collection(&self) -> &OverlayCollection {
        &self.collection
    }

    pub fn drain_events(&mut self) -> Vec<OverlayEvent> {
        std::mem::take(&mut self.events)
    }

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

    /// For a single geometry node, ensure each active overlay has fetches in flight.
    /// If the node has no overlay yet but a parent does, immediately inherit the
    /// parent's overlay (CesiumJS "upsampledFromParent" pattern) so tiles are
    /// never rendered without some texture.
    fn dispatch_for_node(
        &mut self,
        node_id: u64,
        geometric_error: f64,
        hierarchy: &dyn OverlayHierarchy,
    ) {
        let geo_rect = match hierarchy.globe_rectangle(node_id) {
            Some(r) => r,
            None => return,
        };

        // Before mutating tile_state, collect parent overlay data for any
        // overlay that this node doesn't have yet.
        let mut parent_overlays: HashMap<OverlayId, RasterOverlayTile> = HashMap::new();
        if !self.tile_state.contains_key(&node_id)
            || self.tile_state.get(&node_id).map_or(false, |s| {
                self.overlay_order
                    .iter()
                    .any(|oid| !s.overlays.contains_key(oid))
            })
        {
            // Walk up hierarchy to find closest ancestor with ready overlays.
            let mut ancestor = hierarchy.parent(node_id);
            while let Some(pid) = ancestor {
                if let Some(parent_state) = self.tile_state.get(&pid) {
                    for oid in &self.overlay_order {
                        if !parent_overlays.contains_key(oid) {
                            if let Some(pm) = parent_state.overlays.get(oid) {
                                if let Some(ref tile) = pm.ready {
                                    parent_overlays.insert(*oid, tile.clone());
                                }
                            }
                        }
                    }
                    // If we found all overlays, stop walking.
                    if parent_overlays.len() == self.overlay_order.len() {
                        break;
                    }
                }
                ancestor = hierarchy.parent(pid);
            }
        }

        // Compute per-node target texel density.
        //
        // The geometry tile covers `rect_width_rad` radians and should have an
        // overlay tile that provides at least one full 256-texel tile across
        // it. So target_texels_per_radian = 256 / rect_width_rad  (one overlay
        // tile per geometry tile at minimum). The `tiles_for_extent` function
        // then picks the coarsest level meeting that density.
        //
        // As tiles refine (smaller rect_width), this value increases and the
        // overlay level rises to match — exactly matching the geometry LOD.
        let rect_width_rad = (geo_rect.east - geo_rect.west).abs().max(f64::EPSILON);
        let target = 256.0 / rect_width_rad;
        let state = self.tile_state.entry(node_id).or_default();

        for (&overlay_id, provider_state) in &self.providers {
            let provider = match provider_state {
                ProviderState::Active(p) => Arc::clone(p),
                ProviderState::Pending(_) => continue,
            };

            let is_new = !state.overlays.contains_key(&overlay_id);

            let mapped = state
                .overlays
                .entry(overlay_id)
                .or_insert_with(|| MappedOverlay {
                    state: AttachmentState::Unattached,
                    ready: None,
                    ready_level: 0,
                    loading: HashMap::new(),
                    loading_level: 0,
                });

            // Inherit parent's overlay immediately if this is a new entry.
            if is_new && mapped.state == AttachmentState::Unattached {
                if let Some(parent_tile) = parent_overlays.remove(&overlay_id) {
                    mapped.ready = Some(parent_tile.clone());
                    mapped.ready_level = 0; // parent level — will be upgraded
                    mapped.state = AttachmentState::TemporarilyAttached;

                    let uv_index = overlay_order_index(&self.overlay_order, overlay_id);
                    self.events.push(OverlayEvent::Attached {
                        node_id,
                        overlay_id,
                        uv_index,
                        tile: parent_tile,
                    });
                }
            }

            // If already fully attached and no loading in progress, check if
            // higher resolution is available.
            let tile_coords = provider.tiles_for_extent(geo_rect, target);
            if tile_coords.is_empty() {
                continue;
            }
            let target_level = tile_coords[0].2;

            // Already loading this level or better? Skip.
            if !mapped.loading.is_empty() && mapped.loading_level >= target_level {
                continue;
            }
            // Already attached at this level? Skip.
            if mapped.state == AttachmentState::Attached && mapped.ready_level >= target_level {
                continue;
            }

            // Dispatch fetches for all coords at the target level.
            // Clear any existing loading state (we're upgrading).
            log::debug!(
                "overlay node={:?} ge={:.1} target_tpr={:.1} → level {} ({} tiles)",
                node_id,
                geometric_error,
                target,
                target_level,
                tile_coords.len(),
            );
            mapped.loading.clear();
            mapped.loading_level = target_level;
            for (x, y, level) in tile_coords {
                let task = provider.get_tile(x, y, level);
                mapped.loading.insert((x, y, level), task);
            }

            // If we already have a ready tile displayed, mark as temporarily
            // attached while the higher-res version loads (cesium-native pattern).
            if mapped.state == AttachmentState::Attached {
                mapped.state = AttachmentState::TemporarilyAttached;
            }
        }
    }

    /// Check all in-flight fetch groups. When ALL fetches for a (node, overlay)
    /// are done, composite into one tile and emit an Attached event.
    fn process_completions(&mut self) {
        let overlay_order = &self.overlay_order;

        // Collect (node, overlay) pairs whose loading group is fully ready.
        let ready_pairs: Vec<(u64, OverlayId)> = self
            .tile_state
            .iter()
            .flat_map(|(&node_id, state)| {
                state
                    .overlays
                    .iter()
                    .filter(|(_, mapped)| {
                        !mapped.loading.is_empty() && mapped.loading.values().all(|t| t.is_ready())
                    })
                    .map(move |(&overlay_id, _)| (node_id, overlay_id))
            })
            .collect();

        for (node_id, overlay_id) in ready_pairs {
            let state = match self.tile_state.get_mut(&node_id) {
                Some(s) => s,
                None => continue,
            };
            let mapped = match state.overlays.get_mut(&overlay_id) {
                Some(m) => m,
                None => continue,
            };

            // Drain all loading tasks.
            let tasks: HashMap<(u32, u32, u32), Task<RasterOverlayTile>> =
                std::mem::take(&mut mapped.loading);
            let mut tiles: Vec<RasterOverlayTile> = Vec::new();
            for (_, task) in tasks {
                if let Ok(tile) = task.block() {
                    tiles.push(tile);
                }
            }
            if tiles.is_empty() {
                continue;
            }

            // Composite into a single tile.
            let composite = if tiles.len() == 1 {
                tiles.into_iter().next().unwrap()
            } else {
                let mut west = f64::MAX;
                let mut south = f64::MAX;
                let mut east = f64::MIN;
                let mut north = f64::MIN;
                for t in &tiles {
                    west = west.min(t.rectangle.west);
                    south = south.min(t.rectangle.south);
                    east = east.max(t.rectangle.east);
                    north = north.max(t.rectangle.north);
                }
                let target_rect = terra::GlobeRectangle::new(west, south, east, north);
                let cols = ((east - west) / (tiles[0].rectangle.east - tiles[0].rectangle.west))
                    .ceil() as u32;
                let rows = ((north - south) / (tiles[0].rectangle.north - tiles[0].rectangle.south))
                    .ceil() as u32;
                let target_w = (cols * tiles[0].width).min(2048);
                let target_h = (rows * tiles[0].height).min(2048);
                crate::compositing::composite_overlay_tiles(&tiles, target_w, target_h, target_rect)
            };

            let uv_index = overlay_order_index(overlay_order, overlay_id);

            // If there was a previous ready tile, detach it first.
            if mapped.state != AttachmentState::Unattached && mapped.ready.is_some() {
                self.events.push(OverlayEvent::Detached {
                    node_id,
                    overlay_id,
                });
            }

            // Attach the new composite.
            mapped.ready = Some(composite.clone());
            mapped.ready_level = mapped.loading_level;
            mapped.state = AttachmentState::Attached;

            self.events.push(OverlayEvent::Attached {
                node_id,
                overlay_id,
                uv_index,
                tile: composite,
            });
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
