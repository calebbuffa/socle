//! URL template raster overlay — the most common overlay source.
//!
//! Fetches tiles from a server using a URL pattern with `{x}`, `{y}`, `{z}`
//! (and variants) substituted per tile request.

use std::sync::Arc;

use orkester::{Context, Task};
use orkester_io::{AssetAccessor, RequestPriority};

use crate::overlay::{
    RasterOverlay, RasterOverlayTile, RasterOverlayTileProvider, default_tiles_for_extent,
};

/// Options for constructing a [`UrlTemplateRasterOverlay`].
#[derive(Clone, Debug)]
pub struct UrlTemplateOptions {
    /// URL template with substitution tokens. Supported tokens:
    /// - `{x}` — tile column
    /// - `{y}` — tile row
    /// - `{z}` — zoom level
    /// - `{reverseY}` — `(2^z - 1 - y)`, for TMS-style Y-axis
    /// - `{reverseX}` — `(2^z - 1 - x)`
    pub url: String,
    /// HTTP headers to send with each tile request.
    pub headers: Vec<(String, String)>,
    /// Geographic bounds of the overlay in radians.
    pub bounds: terra::GlobeRectangle,
    /// Tile width in pixels (default 256).
    pub tile_width: u32,
    /// Tile height in pixels (default 256).
    pub tile_height: u32,
    /// Minimum zoom level (default 0).
    pub minimum_level: u32,
    /// Maximum zoom level (default 18).
    pub maximum_level: u32,
    /// Number of color channels in the decoded tiles (default 4 = RGBA).
    pub channels: u32,
}

impl Default for UrlTemplateOptions {
    fn default() -> Self {
        Self {
            url: String::new(),
            headers: Vec::new(),
            bounds: terra::GlobeRectangle::MAX,
            tile_width: 256,
            tile_height: 256,
            minimum_level: 0,
            maximum_level: 18,
            channels: 4,
        }
    }
}

/// A raster overlay that fetches tiles from a URL template.
pub struct UrlTemplateRasterOverlay {
    options: UrlTemplateOptions,
}

impl UrlTemplateRasterOverlay {
    pub fn new(options: UrlTemplateOptions) -> Self {
        Self { options }
    }
}

impl RasterOverlay for UrlTemplateRasterOverlay {
    fn create_tile_provider(
        &self,
        _context: &Context,
        accessor: &Arc<dyn AssetAccessor>,
    ) -> Task<Box<dyn RasterOverlayTileProvider>> {
        let provider = UrlTemplateTileProvider {
            options: self.options.clone(),
            accessor: Arc::clone(accessor),
        };
        orkester::resolved(Box::new(provider) as Box<dyn RasterOverlayTileProvider>)
    }
}

struct UrlTemplateTileProvider {
    options: UrlTemplateOptions,
    accessor: Arc<dyn AssetAccessor>,
}

impl UrlTemplateTileProvider {
    fn build_url(&self, x: u32, y: u32, level: u32) -> String {
        let reverse_y = (1u64 << level).saturating_sub(1).saturating_sub(y as u64);
        let reverse_x = (1u64 << level).saturating_sub(1).saturating_sub(x as u64);

        self.options
            .url
            .replace("{x}", &x.to_string())
            .replace("{y}", &y.to_string())
            .replace("{z}", &level.to_string())
            .replace("{reverseY}", &reverse_y.to_string())
            .replace("{reverseX}", &reverse_x.to_string())
    }
}

impl RasterOverlayTileProvider for UrlTemplateTileProvider {
    fn get_tile(&self, x: u32, y: u32, level: u32) -> Task<RasterOverlayTile> {
        let url = self.build_url(x, y, level);
        let headers = self.options.headers.clone();
        let rect = compute_tile_rectangle(x, y, level, &self.options.bounds);

        self.accessor
            .get(&url, &headers, RequestPriority::NORMAL)
            .map(move |result| {
                let resp = result.expect("tile fetch failed");
                resp.check_status()
                    .expect("tile fetch returned non-2xx status");

                let decoded = decode_image_to_rgba(&resp.data);

                RasterOverlayTile {
                    pixels: Arc::from(decoded.pixels),
                    width: decoded.width,
                    height: decoded.height,
                    rectangle: rect,
                }
            })
    }

    fn bounds(&self) -> terra::GlobeRectangle {
        self.options.bounds
    }

    fn maximum_level(&self) -> u32 {
        self.options.maximum_level
    }

    fn minimum_level(&self) -> u32 {
        self.options.minimum_level
    }

    fn tiles_for_extent(
        &self,
        extent: terra::GlobeRectangle,
        target_texels_per_radian: f64,
    ) -> Vec<(u32, u32, u32)> {
        default_tiles_for_extent(self, extent, target_texels_per_radian)
    }
}

pub(crate) struct DecodedImage {
    pub(crate) pixels: Vec<u8>,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

/// Decode a PNG/JPEG/WebP byte buffer into raw RGBA pixel data.
pub(crate) fn decode_image_to_rgba(data: &[u8]) -> DecodedImage {
    let img = image::load_from_memory(data).expect("failed to decode overlay tile image");
    let rgba = img.into_rgba8();
    DecodedImage {
        width: rgba.width(),
        height: rgba.height(),
        pixels: rgba.into_raw(),
    }
}

/// Compute the geographic rectangle for a tile at `(x, y, level)` within the
/// provider's bounds, using a simple equirectangular grid.
pub(crate) fn compute_tile_rectangle(
    x: u32,
    y: u32,
    level: u32,
    provider_bounds: &terra::GlobeRectangle,
) -> terra::GlobeRectangle {
    let tiles = (1u64 << level) as f64;
    let full_lon = provider_bounds.east - provider_bounds.west;
    let full_lat = provider_bounds.north - provider_bounds.south;
    let tile_lon = full_lon / tiles;
    let tile_lat = full_lat / tiles;
    terra::GlobeRectangle::new(
        provider_bounds.west + x as f64 * tile_lon,
        provider_bounds.south + y as f64 * tile_lat,
        provider_bounds.west + (x + 1) as f64 * tile_lon,
        provider_bounds.south + (y + 1) as f64 * tile_lat,
    )
}
