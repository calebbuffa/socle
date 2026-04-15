//! Web Map Service (WMS) raster overlay.
//!
//! Fetches map images from an OGC WMS server via GetMap requests.

use std::sync::Arc;

use orkester::{Context, Task};
use orkester_io::{AssetAccessor, RequestPriority};

use crate::overlay::{
    RasterOverlay, RasterOverlayTile, RasterOverlayTileProvider, default_tiles_for_extent,
};

/// Options for a WMS overlay.
#[derive(Clone, Debug)]
pub struct WmsOptions {
    /// Base URL of the WMS service (e.g. `https://example.com/wms`).
    pub url: String,
    /// Comma-separated layer names.
    pub layers: String,
    /// WMS version string (default `"1.3.0"`). Controls coordinate axis order.
    pub version: String,
    /// Image format MIME type (default `"image/png"`).
    pub format: String,
    /// Coordinate reference system (default `"EPSG:4326"`).
    pub crs: String,
    /// HTTP headers sent with each request.
    pub headers: Vec<(String, String)>,
    /// Geographic bounds in radians. Defaults to the whole globe.
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

impl Default for WmsOptions {
    fn default() -> Self {
        Self {
            url: String::new(),
            layers: String::new(),
            version: "1.3.0".into(),
            format: "image/png".into(),
            crs: "EPSG:4326".into(),
            headers: Vec::new(),
            bounds: None,
            tile_width: 256,
            tile_height: 256,
            minimum_level: 0,
            maximum_level: 18,
        }
    }
}

/// A raster overlay fetching images from an OGC WMS server.
///
/// Each tile request issues a GetMap call with the tile's geographic extent
/// as the BBOX parameter. WMS 1.3.0+ swaps axis order for geographic CRS
/// (lat,lon instead of lon,lat).
pub struct WebMapServiceRasterOverlay {
    options: WmsOptions,
}

impl WebMapServiceRasterOverlay {
    pub fn new(options: WmsOptions) -> Self {
        Self { options }
    }
}

impl RasterOverlay for WebMapServiceRasterOverlay {
    fn create_tile_provider(
        &self,
        _context: &Context,
        accessor: &Arc<dyn AssetAccessor>,
    ) -> Task<Box<dyn RasterOverlayTileProvider>> {
        let provider = WmsTileProvider {
            options: self.options.clone(),
            accessor: Arc::clone(accessor),
        };
        orkester::resolved(Box::new(provider) as Box<dyn RasterOverlayTileProvider>)
    }
}

struct WmsTileProvider {
    options: WmsOptions,
    accessor: Arc<dyn AssetAccessor>,
}

impl WmsTileProvider {
    fn build_url(&self, x: u32, y: u32, level: u32) -> String {
        let bounds = self.options.bounds.unwrap_or(terra::GlobeRectangle::MAX);

        // Compute the geographic extent of this tile.
        let x_tiles = 1u64 << level;
        let y_tiles = 1u64 << level;
        let lon_span = bounds.east - bounds.west;
        let lat_span = bounds.north - bounds.south;
        let tile_lon = lon_span / x_tiles as f64;
        let tile_lat = lat_span / y_tiles as f64;

        let west = bounds.west + x as f64 * tile_lon;
        let south = bounds.south + y as f64 * tile_lat;
        let east = west + tile_lon;
        let north = south + tile_lat;

        // Convert to degrees for the BBOX.
        let to_deg = |r: f64| r.to_degrees();

        // WMS 1.3.0+ with geographic CRS uses lat,lon ordering.
        let is_130_plus = self.options.version.starts_with("1.3");
        let is_geographic = self.options.crs == "EPSG:4326" || self.options.crs == "CRS:84";

        let bbox = if is_130_plus && is_geographic && self.options.crs != "CRS:84" {
            // EPSG:4326 in WMS 1.3.0: lat,lon
            format!(
                "{},{},{},{}",
                to_deg(south),
                to_deg(west),
                to_deg(north),
                to_deg(east)
            )
        } else {
            // Pre-1.3.0 or CRS:84: lon,lat
            format!(
                "{},{},{},{}",
                to_deg(west),
                to_deg(south),
                to_deg(east),
                to_deg(north)
            )
        };

        let crs_key = if is_130_plus { "CRS" } else { "SRS" };

        format!(
            "{}?SERVICE=WMS&VERSION={}&REQUEST=GetMap&LAYERS={}&{}={}&BBOX={}&WIDTH={}&HEIGHT={}&FORMAT={}",
            self.options.url,
            self.options.version,
            self.options.layers,
            crs_key,
            self.options.crs,
            bbox,
            self.options.tile_width,
            self.options.tile_height,
            self.options.format,
        )
    }
}

impl RasterOverlayTileProvider for WmsTileProvider {
    fn get_tile(&self, x: u32, y: u32, level: u32) -> Task<RasterOverlayTile> {
        let url = self.build_url(x, y, level);
        let headers = self.options.headers.clone();
        let accessor = Arc::clone(&self.accessor);
        let provider_bounds = self.options.bounds.unwrap_or(terra::GlobeRectangle::MAX);
        let rect = super::url_template::compute_tile_rectangle(x, y, level, &provider_bounds);

        accessor
            .get(&url, &headers, RequestPriority::NORMAL)
            .map(move |result| {
                let resp = result.expect("WMS tile fetch failed");
                resp.check_status()
                    .expect("WMS tile fetch returned non-2xx status");

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
