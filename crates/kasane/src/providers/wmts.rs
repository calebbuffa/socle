//! Web Map Tile Service (WMTS) raster overlay.
//!
//! Fetches pre-rendered tiles from an OGC WMTS server using either
//! RESTful or KVP (Key-Value Pair) URL patterns.

use std::collections::HashMap;
use std::sync::Arc;

use orkester::{Context, Task};
use orkester_io::{AssetAccessor, RequestPriority};

use crate::overlay::{
    RasterOverlay, RasterOverlayTile, RasterOverlayTileProvider, default_tiles_for_extent,
};

/// Options for a WMTS overlay.
#[derive(Clone, Debug)]
pub struct WmtsOptions {
    /// Base URL of the WMTS service.
    pub url: String,
    /// The WMTS layer name.
    pub layer: String,
    /// Style name (default `"default"`).
    pub style: String,
    /// TileMatrixSet identifier.
    pub tile_matrix_set: String,
    /// Optional per-level TileMatrix labels. If empty, `level.to_string()` is used.
    pub tile_matrix_labels: Vec<String>,
    /// Image format MIME type (default `"image/png"`).
    pub format: String,
    /// Additional static dimensions (`key → value`), appended to each request.
    pub dimensions: HashMap<String, String>,
    /// Subdomain list for round-robin load balancing (e.g. `["a", "b", "c"]`).
    pub subdomains: Vec<String>,
    /// HTTP headers.
    pub headers: Vec<(String, String)>,
    /// Geographic bounds in radians.
    pub bounds: Option<terra::GlobeRectangle>,
    /// Tile width in pixels (default 256).
    pub tile_width: u32,
    /// Tile height in pixels (default 256).
    pub tile_height: u32,
    /// Minimum zoom level (default 0).
    pub minimum_level: u32,
    /// Maximum zoom level (default 18).
    pub maximum_level: u32,
}

impl Default for WmtsOptions {
    fn default() -> Self {
        Self {
            url: String::new(),
            layer: String::new(),
            style: "default".into(),
            tile_matrix_set: String::new(),
            tile_matrix_labels: Vec::new(),
            format: "image/png".into(),
            dimensions: HashMap::new(),
            subdomains: Vec::new(),
            headers: Vec::new(),
            bounds: None,
            tile_width: 256,
            tile_height: 256,
            minimum_level: 0,
            maximum_level: 18,
        }
    }
}

/// A raster overlay fetching tiles from an OGC WMTS server.
pub struct WebMapTileServiceRasterOverlay {
    options: WmtsOptions,
}

impl WebMapTileServiceRasterOverlay {
    pub fn new(options: WmtsOptions) -> Self {
        Self { options }
    }
}

impl RasterOverlay for WebMapTileServiceRasterOverlay {
    fn create_tile_provider(
        &self,
        _context: &Context,
        accessor: &Arc<dyn AssetAccessor>,
    ) -> Task<Box<dyn RasterOverlayTileProvider>> {
        let provider = WmtsTileProvider {
            options: self.options.clone(),
            accessor: Arc::clone(accessor),
            subdomain_counter: std::sync::atomic::AtomicU64::new(0),
        };
        orkester::resolved(Box::new(provider) as Box<dyn RasterOverlayTileProvider>)
    }
}

struct WmtsTileProvider {
    options: WmtsOptions,
    accessor: Arc<dyn AssetAccessor>,
    subdomain_counter: std::sync::atomic::AtomicU64,
}

impl WmtsTileProvider {
    fn tile_matrix_label(&self, level: u32) -> String {
        self.options
            .tile_matrix_labels
            .get(level as usize)
            .cloned()
            .unwrap_or_else(|| level.to_string())
    }

    fn next_subdomain(&self) -> Option<&str> {
        if self.options.subdomains.is_empty() {
            None
        } else {
            let idx = self
                .subdomain_counter
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed) as usize
                % self.options.subdomains.len();
            Some(&self.options.subdomains[idx])
        }
    }

    fn build_url(&self, x: u32, y: u32, level: u32) -> String {
        let tile_matrix = self.tile_matrix_label(level);

        // Try RESTful pattern first (URL contains `{TileMatrix}` etc.).
        let url = &self.options.url;
        if url.contains("{TileMatrix}") || url.contains("{tileMatrix}") {
            let mut result = url
                .replace("{TileMatrix}", &tile_matrix)
                .replace("{tileMatrix}", &tile_matrix)
                .replace("{TileCol}", &x.to_string())
                .replace("{tileCol}", &x.to_string())
                .replace("{TileRow}", &y.to_string())
                .replace("{tileRow}", &y.to_string())
                .replace("{Style}", &self.options.style)
                .replace("{style}", &self.options.style)
                .replace("{Layer}", &self.options.layer)
                .replace("{layer}", &self.options.layer)
                .replace("{TileMatrixSet}", &self.options.tile_matrix_set)
                .replace("{tileMatrixSet}", &self.options.tile_matrix_set);

            if let Some(sub) = self.next_subdomain() {
                result = result.replace("{s}", sub);
            }

            for (k, v) in &self.options.dimensions {
                result = result.replace(&format!("{{{k}}}"), v);
            }

            return result;
        }

        // Fall back to KVP encoding.
        let mut kvp = format!(
            "{}?SERVICE=WMTS&REQUEST=GetTile&VERSION=1.0.0\
             &LAYER={}&STYLE={}&TILEMATRIXSET={}&TILEMATRIX={}\
             &TILEROW={}&TILECOL={}&FORMAT={}",
            url,
            self.options.layer,
            self.options.style,
            self.options.tile_matrix_set,
            tile_matrix,
            y,
            x,
            self.options.format,
        );

        for (k, v) in &self.options.dimensions {
            kvp.push_str(&format!("&{k}={v}"));
        }

        kvp
    }
}

impl RasterOverlayTileProvider for WmtsTileProvider {
    fn get_tile(&self, x: u32, y: u32, level: u32) -> Task<RasterOverlayTile> {
        let url = self.build_url(x, y, level);
        let headers = self.options.headers.clone();
        let accessor = Arc::clone(&self.accessor);
        let provider_bounds = self.options.bounds.unwrap_or(terra::GlobeRectangle::MAX);
        let rect = super::url_template::compute_tile_rectangle(x, y, level, &provider_bounds);

        accessor
            .get(&url, &headers, RequestPriority::NORMAL)
            .map(move |result| {
                let resp = result.expect("WMTS tile fetch failed");
                resp.check_status()
                    .expect("WMTS tile fetch returned non-2xx status");

                let decoded = super::url_template::decode_image_to_rgba(&resp.data);

                RasterOverlayTile {
                    pixels: Arc::from(decoded.pixels),
                    width: decoded.width,
                    height: decoded.height,
                    rectangle: rect,
                }
            })
    }

    fn bounds(&self) -> terra::GlobeRectangle {
        self.options.bounds.unwrap_or(terra::GlobeRectangle::MAX)
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
