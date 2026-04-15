//! Concrete raster overlay tile provider implementations.

pub(crate) mod tms;
pub(crate) mod url_template;
pub(crate) mod wms;
pub(crate) mod wmts;

pub use tms::TileMapServiceRasterOverlay;
pub use url_template::UrlTemplateRasterOverlay;
pub use wms::WebMapServiceRasterOverlay;
pub use wmts::WebMapTileServiceRasterOverlay;
