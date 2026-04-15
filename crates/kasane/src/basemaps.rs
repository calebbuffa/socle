//! Built-in free basemap presets.
//!
//! Each variant produces a [`UrlTemplateRasterOverlay`] ready to add to an
//! [`OverlayEngine`](crate::OverlayEngine).
//!
//! # Example
//!
//! ```rust,ignore
//! use kasane::basemaps::Basemap;
//!
//! engine.add_overlay(Basemap::Osm.into_overlay());
//! ```

use crate::UrlTemplateOptions;
use crate::UrlTemplateRasterOverlay;

/// Built-in free basemap presets.
///
/// Each variant carries the URL template and sensible defaults. Call
/// [`into_overlay()`](Basemap::into_overlay) to get something you can pass
/// straight to [`OverlayEngine::add_overlay`](crate::OverlayEngine::add_overlay).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Basemap {
    /// OpenStreetMap standard street map.
    Osm,
    /// CartoDB Positron — clean light-grey street map.
    CartoLight,
    /// CartoDB Dark Matter — dark-theme street map.
    CartoDark,
    /// CartoDB Positron without labels.
    CartoLightNoLabels,
    /// OpenTopoMap — topographic map derived from OSM + SRTM.
    OpenTopoMap,
    /// ESRI World Imagery — high-resolution satellite/aerial imagery.
    EsriSatellite,
}

impl Basemap {
    /// URL template for this basemap.
    pub fn url(self) -> &'static str {
        match self {
            Self::Osm => "https://tile.openstreetmap.org/{z}/{x}/{reverseY}.png",
            Self::CartoLight => "https://basemaps.cartocdn.com/light_all/{z}/{x}/{reverseY}.png",
            Self::CartoDark => "https://basemaps.cartocdn.com/dark_all/{z}/{x}/{reverseY}.png",
            Self::CartoLightNoLabels => {
                "https://basemaps.cartocdn.com/light_nolabels/{z}/{x}/{reverseY}.png"
            }
            Self::OpenTopoMap => "https://tile.opentopomap.org/{z}/{x}/{reverseY}.png",
            Self::EsriSatellite => {
                "https://services.arcgisonline.com/arcgis/rest/services/World_Imagery/MapServer/tile/{z}/{reverseY}/{x}"
            }
        }
    }

    /// Maximum zoom level for this basemap.
    pub fn maximum_level(self) -> u32 {
        match self {
            Self::Osm => 19,
            Self::CartoLight | Self::CartoDark | Self::CartoLightNoLabels => 20,
            Self::OpenTopoMap => 17,
            Self::EsriSatellite => 19,
        }
    }

    /// Convert to a ready-to-use [`UrlTemplateRasterOverlay`].
    pub fn into_overlay(self) -> UrlTemplateRasterOverlay {
        UrlTemplateRasterOverlay::new(UrlTemplateOptions {
            url: self.url().into(),
            maximum_level: self.maximum_level(),
            ..Default::default()
        })
    }
}
