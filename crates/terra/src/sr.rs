//! Spatial reference identified by WKID or WKT — the I3S unification bridge.
//!
//! [`SpatialReference`] stores a CRS identity in the same form that I3S scene
//! layer JSON uses (`wkid`, `latestWkid`, `vcsWkid`, `wkt`).  Call
//! [`SpatialReference::to_crs`] with a [`CrsRegistry`] to get a concrete
//! [`Crs`] implementation.  [`CrsRegistry`] comes pre-populated with the
//! common EPSG/ESRI codes and can be extended with custom factories.

use std::collections::HashMap;

use crate::{
    Ellipsoid,
    crs::{Crs, EcefCrs, GeographicCrs, WebMercatorCrs},
};

// ── CrsRegistry ──────────────────────────────────────────────────────────────

type CrsFactory = Box<dyn Fn(&Ellipsoid) -> Box<dyn Crs> + Send + Sync>;

/// A registry that maps EPSG/ESRI WKIDs to [`Crs`] factory functions.
///
/// Constructed with [`CrsRegistry::new`] (or the convenience alias
/// [`CrsRegistry::wgs84`]) and pre-populated with the built-in codes
/// (4326 / 4978 / 3857 families).  Additional mappings can be registered
/// with [`CrsRegistry::register`].
///
/// Pass a `&CrsRegistry` to [`SpatialReference::to_crs`].
pub struct CrsRegistry {
    ellipsoid: Ellipsoid,
    entries: HashMap<u32, CrsFactory>,
}

impl CrsRegistry {
    /// Create a registry for the given ellipsoid, pre-populated with
    /// built-in WKID mappings.
    pub fn new(ellipsoid: Ellipsoid) -> Self {
        let mut reg = Self {
            ellipsoid,
            entries: HashMap::new(),
        };
        reg.register(4326, Box::new(|e| Box::new(GeographicCrs::new(e.clone()))));
        reg.register(4269, Box::new(|e| Box::new(GeographicCrs::new(e.clone()))));
        reg.register(4267, Box::new(|e| Box::new(GeographicCrs::new(e.clone()))));
        reg.register(4978, Box::new(|e| Box::new(EcefCrs::new(e.clone()))));
        reg.register(3857, Box::new(|e| Box::new(WebMercatorCrs::new(e.clone()))));
        reg.register(
            102100,
            Box::new(|e| Box::new(WebMercatorCrs::new(e.clone()))),
        );
        reg.register(
            900913,
            Box::new(|e| Box::new(WebMercatorCrs::new(e.clone()))),
        );
        reg
    }

    /// Convenience constructor using the WGS 84 ellipsoid.
    pub fn wgs84() -> Self {
        Self::new(Ellipsoid::wgs84())
    }

    /// Register a custom WKID → [`Crs`] factory, overriding any existing entry.
    pub fn register(&mut self, wkid: u32, factory: CrsFactory) {
        self.entries.insert(wkid, factory);
    }

    /// Resolve a WKID to a [`Crs`] instance, or `None` if unregistered.
    pub fn resolve(&self, wkid: u32) -> Option<Box<dyn Crs>> {
        self.entries.get(&wkid).map(|f| f(&self.ellipsoid))
    }

    /// The ellipsoid this registry was built with.
    pub fn ellipsoid(&self) -> &Ellipsoid {
        &self.ellipsoid
    }
}

/// A spatial reference identified by EPSG/ESRI `wkid` and/or WKT string.
///
/// Mirrors the `spatialReference` object in I3S scene layer metadata.  The
/// two WKID fields reflect the ESRI convention where `wkid` may be an older
/// ESRI-specific code and `latestWkid` is the canonical EPSG code.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct SpatialReference {
    /// Horizontal WKID (EPSG or ESRI code).
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub wkid: Option<u32>,
    /// Preferred canonical WKID; takes priority over `wkid` when resolving.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub latest_wkid: Option<u32>,
    /// Vertical CRS WKID.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub vcs_wkid: Option<u32>,
    /// Preferred vertical CRS WKID.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub latest_vcs_wkid: Option<u32>,
    /// WKT definition string (fallback / full specification).
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub wkt: Option<String>,
}

impl SpatialReference {
    // ---- common constructors -----------------------------------------------

    /// WGS 84 geographic — EPSG:4326.  Used by most I3S global scene layers.
    pub fn wgs84() -> Self {
        Self {
            wkid: Some(4326),
            latest_wkid: Some(4326),
            ..Default::default()
        }
    }

    /// WGS 84 / Pseudo-Mercator (Web Mercator) — EPSG:3857.
    pub fn web_mercator() -> Self {
        Self {
            wkid: Some(3857),
            latest_wkid: Some(3857),
            ..Default::default()
        }
    }

    /// WGS 84 ECEF — EPSG:4978.  Used by 3D Tiles and I3S globe scene layers.
    pub fn ecef() -> Self {
        Self {
            wkid: Some(4978),
            latest_wkid: Some(4978),
            ..Default::default()
        }
    }

    /// Construct from a bare WKID.
    pub fn from_wkid(wkid: u32) -> Self {
        Self {
            wkid: Some(wkid),
            ..Default::default()
        }
    }

    // ---- resolution -------------------------------------------------------

    /// Resolve this spatial reference to a concrete [`Crs`] implementation.
    ///
    /// Returns `None` if the effective WKID is absent or not registered in
    /// `registry`.  Register custom WKIDs with [`CrsRegistry::register`].
    pub fn to_crs(&self, registry: &CrsRegistry) -> Option<Box<dyn Crs>> {
        registry.resolve(self.effective_wkid()?)
    }

    /// Resolve using a default WGS 84 registry (the common case).
    pub fn to_crs_wgs84(&self) -> Option<Box<dyn Crs>> {
        self.to_crs(&CrsRegistry::wgs84())
    }

    /// The effective horizontal WKID to use when resolving: prefers
    /// `latest_wkid`, falls back to `wkid`.
    pub fn effective_wkid(&self) -> Option<u32> {
        self.latest_wkid.or(self.wkid)
    }

    /// Return `true` if this spatial reference is geographic (degrees).
    pub fn is_geographic(&self) -> bool {
        matches!(self.effective_wkid(), Some(4326 | 4269 | 4267))
    }

    /// Return `true` if this spatial reference is WGS 84 ECEF.
    pub fn is_ecef(&self) -> bool {
        matches!(self.effective_wkid(), Some(4978))
    }

    /// Return `true` if this spatial reference is Web Mercator.
    pub fn is_web_mercator(&self) -> bool {
        matches!(self.effective_wkid(), Some(3857 | 102100 | 900913))
    }
}
