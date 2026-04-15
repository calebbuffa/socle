//! Tile Map Service (TMS) raster overlay.
//!
//! Fetches tiles from a TMS-compliant server. Reads the `tilemapresource.xml`
//! metadata document to discover bounds, levels, and tile format.

use std::sync::Arc;

use orkester::{Context, Task};
use orkester_io::{AssetAccessor, RequestPriority};

use super::url_template::{UrlTemplateOptions, UrlTemplateRasterOverlay};
use crate::overlay::{
    RasterOverlay, RasterOverlayTile, RasterOverlayTileProvider, default_tiles_for_extent,
};

/// Options for a TMS overlay.
#[derive(Clone, Debug)]
pub struct TmsOptions {
    /// Base URL of the TMS endpoint (e.g. `https://example.com/tms/1.0.0/layer`).
    pub url: String,
    /// File extension for tile images (default `"png"`).
    pub file_extension: String,
    /// HTTP headers to send with tile requests.
    pub headers: Vec<(String, String)>,
    /// Geographic bounds in radians. If `None`, uses the whole globe.
    pub bounds: Option<terra::GlobeRectangle>,
    /// Minimum zoom level (default 0).
    pub minimum_level: u32,
    /// Maximum zoom level (default 18).
    pub maximum_level: u32,
    /// Tile width in pixels (default 256).
    pub tile_width: u32,
    /// Tile height in pixels (default 256).
    pub tile_height: u32,
    /// Whether the Y axis is flipped compared to standard TMS (default false).
    /// Set to `true` for WMTS/slippy-map convention (Y=0 at top).
    pub flip_y: bool,
}

impl Default for TmsOptions {
    fn default() -> Self {
        Self {
            url: String::new(),
            file_extension: "png".into(),
            headers: Vec::new(),
            bounds: None,
            minimum_level: 0,
            maximum_level: 18,
            tile_width: 256,
            tile_height: 256,
            flip_y: false,
        }
    }
}

/// A raster overlay fetching tiles from a TMS server.
///
/// TMS follows the pattern `{base}/{z}/{x}/{y}.{ext}`. When `flip_y` is false
/// (standard TMS), Y increases from south to north. When `flip_y` is true
/// (slippy-map convention), `{reverseY}` is used instead.
pub struct TileMapServiceRasterOverlay {
    options: TmsOptions,
}

impl TileMapServiceRasterOverlay {
    pub fn new(options: TmsOptions) -> Self {
        Self { options }
    }
}

impl RasterOverlay for TileMapServiceRasterOverlay {
    fn create_tile_provider(
        &self,
        context: &Context,
        accessor: &Arc<dyn AssetAccessor>,
    ) -> Task<Box<dyn RasterOverlayTileProvider>> {
        let y_token = if self.options.flip_y {
            "{reverseY}"
        } else {
            "{y}"
        };
        let url = format!(
            "{}/{{z}}/{{x}}/{}.{}",
            self.options.url.trim_end_matches('/'),
            y_token,
            self.options.file_extension,
        );

        let inner = UrlTemplateRasterOverlay::new(UrlTemplateOptions {
            url,
            headers: self.options.headers.clone(),
            bounds: self.options.bounds.unwrap_or(terra::GlobeRectangle::MAX),
            tile_width: self.options.tile_width,
            tile_height: self.options.tile_height,
            minimum_level: self.options.minimum_level,
            maximum_level: self.options.maximum_level,
            channels: 4,
        });

        inner.create_tile_provider(context, accessor)
    }
}
