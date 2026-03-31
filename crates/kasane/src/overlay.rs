//! Raster overlay types and collection management.

use std::sync::Arc;

use orkester::Task;
use orkester_io::AssetAccessor;

/// A single raster overlay tile — pixel data plus UV transform for draping.
#[derive(Clone, Debug)]
pub struct RasterOverlayTile {
    /// RGBA pixel data, row-major from top-left.
    pub pixels: Arc<[u8]>,
    pub width: u32,
    pub height: u32,
    /// UV translation applied when sampling this tile onto geometry.
    pub translation: glam::Vec2,
    /// UV scale applied when sampling this tile onto geometry.
    pub scale: glam::Vec2,
}

/// Produces individual overlay tiles on demand.
///
/// Implementors fetch tiles from a URL template, WMS endpoint, etc.
pub trait RasterOverlayTileProvider: Send + Sync {
    /// Fetch the tile at the given tile coordinates.
    fn get_tile(&self, x: u32, y: u32, level: u32) -> Task<RasterOverlayTile>;
    /// Geographic coverage of this provider.
    fn bounds(&self) -> terra::GlobeRectangle;
    /// Maximum available tile zoom level.
    fn maximum_level(&self) -> u32;
    /// Minimum available tile zoom level.
    fn minimum_level(&self) -> u32 {
        0
    }

    /// Find all tile coordinates whose pixel coverage of `extent` most closely
    /// matches `target_texels_per_degree`.
    ///
    /// Returns `(x, y, level)` tuples. Returning an empty `Vec` means no tile
    /// covers the given extent (e.g., the extent is outside the provider's
    /// [`bounds()`](Self::bounds)).
    ///
    /// The default implementation uses a power-of-two Web Mercator–style
    /// scheme and is suitable for providers whose levels double in resolution.
    /// Use [`default_tiles_for_extent`] in your implementation body.
    fn tiles_for_extent(
        &self,
        extent: terra::GlobeRectangle,
        target_texels_per_radian: f64,
    ) -> Vec<(u32, u32, u32)>;
}

/// An overlay data source that can create a tile provider.
///
/// Implement this for each overlay type (URL template, Cesium ion, WMS, …).
pub trait RasterOverlay: Send + Sync {
    /// Asynchronously construct the tile provider.
    ///
    /// Called once when the overlay is added to a `Stratum`. The provider
    /// is then used for the lifetime of the overlay.
    fn create_tile_provider(
        &self,
        runtime: &orkester::Runtime,
        accessor: &Arc<dyn AssetAccessor>,
    ) -> Task<Box<dyn RasterOverlayTileProvider>>;
}

/// Opaque handle to an overlay added to an [`OverlayCollection`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OverlayId(pub u32);

/// Manages a set of raster overlays attached to a `Stratum`.
///
/// Overlays are added at runtime; tile providers are created asynchronously.
/// The collection drives attach/detach calls to the `OverlayablePreparer`.
#[derive(Default)]
pub struct OverlayCollection {
    overlays: Vec<OverlayEntry>,
    next_id: u32,
}

struct OverlayEntry {
    id: OverlayId,
    overlay: Box<dyn RasterOverlay>,
}

impl OverlayCollection {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an overlay. Returns an opaque `OverlayId` for later removal.
    pub fn add(&mut self, overlay: impl RasterOverlay + 'static) -> OverlayId {
        let id = OverlayId(self.next_id);
        self.next_id += 1;
        self.overlays.push(OverlayEntry {
            id,
            overlay: Box::new(overlay),
        });
        id
    }

    /// Remove an overlay by its id.
    pub fn remove(&mut self, id: OverlayId) {
        self.overlays.retain(|e| e.id != id);
    }

    /// Number of active overlays.
    pub fn len(&self) -> usize {
        self.overlays.len()
    }

    pub fn is_empty(&self) -> bool {
        self.overlays.is_empty()
    }

    /// Iterate over (id, overlay) pairs. Used by `OverlayStratum` to
    /// initialize tile providers when overlays are added.
    pub fn iter(&self) -> impl Iterator<Item = (OverlayId, &dyn RasterOverlay)> {
        self.overlays.iter().map(|e| (e.id, e.overlay.as_ref()))
    }
}

/// Default implementation of [`RasterOverlayTileProvider::tiles_for_extent`].
///
/// Assumes a whole-world two-dimensional grid that doubles in resolution each
/// level (Web Mercator or geographic). Returns the tiles at the coarsest level
/// whose texel density is at least `target_texels_per_radian`.
pub fn default_tiles_for_extent(
    provider: &dyn RasterOverlayTileProvider,
    extent: terra::GlobeRectangle,
    target_texels_per_radian: f64,
) -> Vec<(u32, u32, u32)> {
    let provider_bounds = provider.bounds();
    // Clamp the query extent to the provider's coverage.
    let west = extent.west.max(provider_bounds.west);
    let east = extent.east.min(provider_bounds.east);
    let south = extent.south.max(provider_bounds.south);
    let north = extent.north.min(provider_bounds.north);
    if west >= east || south >= north {
        return vec![];
    }

    // Full angular spans of the provider, used to compute per-level resolution.
    let full_lon = (provider_bounds.east - provider_bounds.west)
        .abs()
        .max(f64::EPSILON);
    let full_lat = (provider_bounds.north - provider_bounds.south)
        .abs()
        .max(f64::EPSILON);

    // Find the best level.
    let min_level = provider.minimum_level();
    let max_level = provider.maximum_level();
    // Start at the coarsest level and step up while we have sufficient resolution
    // and haven't exceeded max_level.
    let mut chosen_level = min_level;
    for level in min_level..=max_level {
        // Guard: 1u32 << level panics/wraps when level >= 32.
        // A provider that returns maximum_level() >= 32 is erroneous, but we
        // handle it gracefully by treating levels ≥ 32 as if they all have
        // maximum resolution (just use the clamped 2^31 tile count).
        let x_tiles = 1u32.checked_shl(level).unwrap_or(u32::MAX);
        let y_tiles = 1u32.checked_shl(level).unwrap_or(u32::MAX);
        let texels_per_radian_x = (x_tiles as f64 * 256.0) / full_lon;
        let texels_per_radian_y = (y_tiles as f64 * 256.0) / full_lat;
        let texels_per_radian = texels_per_radian_x.min(texels_per_radian_y);
        chosen_level = level;
        if texels_per_radian >= target_texels_per_radian {
            break;
        }
    }

    let x_tiles = 1u32.checked_shl(chosen_level).unwrap_or(u32::MAX);
    let y_tiles = 1u32.checked_shl(chosen_level).unwrap_or(u32::MAX);

    // Map the clamped extent to tile indices.
    let tile_lon = full_lon / x_tiles as f64;
    let tile_lat = full_lat / y_tiles as f64;

    let x0 = ((west - provider_bounds.west) / tile_lon).floor() as u32;
    let x1 = ((east - provider_bounds.west) / tile_lon).ceil() as u32;
    let y0 = ((south - provider_bounds.south) / tile_lat).floor() as u32;
    let y1 = ((north - provider_bounds.south) / tile_lat).ceil() as u32;

    let x0 = x0.min(x_tiles - 1);
    let x1 = x1.min(x_tiles);
    let y0 = y0.min(y_tiles - 1);
    let y1 = y1.min(y_tiles);

    let mut out = Vec::with_capacity(((x1 - x0) * (y1 - y0)) as usize);
    for y in y0..y1 {
        for x in x0..x1 {
            out.push((x, y, chosen_level));
        }
    }
    out
}
