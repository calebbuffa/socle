//! Coordinate reference system dispatch for I3S scene layers.

use glam::{DQuat, DVec3};

use i3s::spatial::{Obb, SpatialReference};
use i3s_geometry::obb::OrientedBoundingBox;

use crate::cartographic::Cartographic;
use crate::ellipsoid::Ellipsoid;

/// Trait for converting positions from a local/projected CRS to ECEF.
pub trait CrsTransform: Send + Sync {
    /// Transform positions in-place from source CRS to ECEF.
    fn to_ecef(&self, positions: &mut [DVec3]);
}

/// Global (WKID 4326/4490) or Local/projected CRS classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneCoordinateSystem {
    Global,
    Local,
}

impl SceneCoordinateSystem {
    /// Classify a spatial reference as Global or Local.
    ///
    /// Returns `Global` if the effective WKID is 4326 (WGS84) or 4490
    /// (CGCS 2000). Per the I3S spec, these geographic CRS use ECEF for
    /// OBB quaternion/halfSize and store the center as `[lon°, lat°, elev_m]`.
    ///
    /// Returns `Local` for all other cases, including when no spatial reference
    /// is provided (assumes the layer is local/projected).
    pub fn from_spatial_reference(sr: Option<&SpatialReference>) -> Self {
        let sr = match sr {
            Some(sr) => sr,
            None => return SceneCoordinateSystem::Local,
        };

        // Use latest_wkid first (more current), then wkid
        let effective_wkid = sr.latest_wkid.or(sr.wkid);

        match effective_wkid {
            // WGS84 geographic
            Some(4326) => SceneCoordinateSystem::Global,
            // CGCS 2000 (China Geodetic Coordinate System) — same ECEF
            // semantics as WGS84 per the I3S spec
            Some(4490) => SceneCoordinateSystem::Global,
            _ => SceneCoordinateSystem::Local,
        }
    }
}

/// Convert an I3S spec OBB to a geometry OBB, applying CRS-aware transformation.
///
/// For global scenes (WKID 4326), converts the OBB center from geographic
/// coordinates `[lon°, lat°, elev_m]` to ECEF. The quaternion is already in
/// the ECEF frame per the I3S spec.
///
/// For local scenes with a [`CrsTransform`], transforms the entire OBB to
/// ECEF using the corner-transformation approach.
///
/// For local scenes without a transform, uses the OBB coordinates directly.
pub fn obb_from_spec(
    obb: &Obb,
    crs: SceneCoordinateSystem,
    crs_transform: Option<&dyn CrsTransform>,
) -> OrientedBoundingBox {
    match crs {
        SceneCoordinateSystem::Global => obb_from_spec_global(obb),
        SceneCoordinateSystem::Local => match crs_transform {
            Some(xform) => obb_from_spec_local_to_ecef(obb, xform),
            None => obb_from_spec_local(obb),
        },
    }
}

/// Convert a global-scene OBB (WKID 4326) to an ECEF OBB.
///
/// The I3S spec requires:
/// - center: `[longitude°, latitude°, elevation_m]`
/// - halfSize: meters
/// - quaternion: ECEF frame (Z+: North, Y+: East, X+: lon=lat=0)
fn obb_from_spec_global(obb: &Obb) -> OrientedBoundingBox {
    // Convert center from [lon°, lat°, elev_m] to ECEF
    let lon_deg = obb.center[0];
    let lat_deg = obb.center[1];
    let elev = obb.center[2];

    let carto = Cartographic::from_degrees(lon_deg, lat_deg, elev);
    let ecef_center = Ellipsoid::WGS84.cartographic_to_cartesian(carto);

    // Quaternion is already in ECEF frame per the I3S spec.
    // I3S quaternion order: [x, y, z, w]
    let q = DQuat::from_xyzw(
        obb.quaternion[0],
        obb.quaternion[1],
        obb.quaternion[2],
        obb.quaternion[3],
    );

    OrientedBoundingBox {
        center: ecef_center,
        half_size: DVec3::from_array(obb.half_size),
        quaternion: q,
    }
}

/// Convert a local-scene OBB to a geometry OBB (no CRS conversion).
fn obb_from_spec_local(obb: &Obb) -> OrientedBoundingBox {
    OrientedBoundingBox::from_i3s(obb.center, obb.half_size, obb.quaternion)
}

/// Convert a local-scene OBB to ECEF via [`CrsTransform`].
///
/// 1. Build the OBB in source CRS space
/// 2. Compute 8 corners + center (9 points)
/// 3. Transform all points to ECEF via `CrsTransform::to_ecef`
/// 4. Refit an axis-aligned OBB from the transformed corners
fn obb_from_spec_local_to_ecef(obb: &Obb, xform: &dyn CrsTransform) -> OrientedBoundingBox {
    let local_obb = OrientedBoundingBox::from_i3s(obb.center, obb.half_size, obb.quaternion);
    let mut corners = local_obb.corners();
    xform.to_ecef(&mut corners);
    OrientedBoundingBox::from_corners(&corners)
}

// WkidTransform — pure-Rust CrsTransform for common CRS families

use crate::projection::{
    TransverseMercatorParams, from_geographic_degrees, from_transverse_mercator, from_web_mercator,
};

/// The internal projection kind used by [`WkidTransform`].
#[derive(Debug, Clone, Copy)]
enum TransformKind {
    /// Web Mercator (EPSG:3857): input is (easting_m, northing_m, z_m).
    WebMercator,
    /// Geographic CRS where input is (longitude_deg, latitude_deg, z_m).
    /// Covers NAD83 (4269), ETRS89 (4258), GDA2020 (7844), etc.
    Geographic,
    /// Transverse Mercator (UTM and similar): input is (easting_m, northing_m, z_m).
    TransverseMercator(TransverseMercatorParams),
}

/// Pure-Rust [`CrsTransform`] for common WKID-based coordinate reference systems.
///
/// Converts positions from the layer's native CRS to ECEF using built-in
/// projection math — no external C libraries required.
///
/// The transform uses a configurable [`Ellipsoid`] (default [`Ellipsoid::WGS84`]).
/// Override the ellipsoid with [`with_ellipsoid`](Self::with_ellipsoid) when
/// working with layers that use a different datum.
///
/// # Supported CRS families
///
/// | EPSG code(s) | Projection | Notes |
/// |---|---|---|
/// | 3857 | Web Mercator | Spherical, equatorial |
/// | 4269 | NAD83 Geographic | ≈ WGS84 to sub-meter |
/// | 4258 | ETRS89 Geographic | European, ≈ WGS84 |
/// | 7844 | GDA2020 Geographic | Australian |
/// | 32601–32660 | UTM zones 1–60 North | WGS84 datum |
/// | 32701–32760 | UTM zones 1–60 South | WGS84 datum |
///
/// For unsupported CRS, [`from_spatial_reference`](Self::from_spatial_reference)
/// returns `None`. Provide a custom [`CrsTransform`] for those cases.
///
/// # Accuracy
///
/// - Web Mercator: exact inverse (sub-mm)
/// - Geographic CRS: treats all supported datums as the supplied ellipsoid,
///   which introduces sub-meter error for NAD83/ETRS89 vs WGS84
///   (acceptable for visualization)
/// - UTM: Redfearn series expansion, sub-mm within zone
///
/// # Example
///
/// ```
/// use i3s::spatial::SpatialReference;
/// use i3s_geospatial::crs::WkidTransform;
///
/// // Web Mercator layer
/// let sr = SpatialReference { wkid: Some(3857), ..Default::default() };
/// let xform = WkidTransform::from_spatial_reference(&sr).unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct WkidTransform {
    kind: TransformKind,
    ellipsoid: Ellipsoid,
}

impl WkidTransform {
    /// Build a transform from a layer's [`SpatialReference`], using
    /// [`Ellipsoid::WGS84`].
    ///
    /// Returns `None` if the WKID is not in the supported set.
    pub fn from_spatial_reference(sr: &SpatialReference) -> Option<Self> {
        let wkid = sr.latest_wkid.or(sr.wkid)?;
        Self::from_wkid(wkid)
    }

    /// Build a transform from an explicit EPSG code, using
    /// [`Ellipsoid::WGS84`].
    pub fn from_wkid(wkid: i64) -> Option<Self> {
        Self::from_wkid_with_ellipsoid(wkid, Ellipsoid::WGS84)
    }

    /// Build a transform with a custom [`Ellipsoid`].
    pub fn from_wkid_with_ellipsoid(wkid: i64, ellipsoid: Ellipsoid) -> Option<Self> {
        let kind = match wkid {
            // Web Mercator
            3857 | 102100 | 900913 => TransformKind::WebMercator,

            // Geographic CRS (degrees) — treat as WGS84-equivalent
            4269 | 4258 | 7844 | 4167 | 4617 => TransformKind::Geographic,

            // UTM North zones: EPSG 32601 (zone 1) through 32660 (zone 60)
            32601..=32660 => {
                let zone = (wkid - 32600) as u8;
                TransformKind::TransverseMercator(TransverseMercatorParams::utm_with_ellipsoid(
                    zone, true, ellipsoid,
                ))
            }

            // UTM South zones: EPSG 32701 (zone 1) through 32760 (zone 60)
            32701..=32760 => {
                let zone = (wkid - 32700) as u8;
                TransformKind::TransverseMercator(TransverseMercatorParams::utm_with_ellipsoid(
                    zone, false, ellipsoid,
                ))
            }

            _ => return None,
        };
        Some(Self { kind, ellipsoid })
    }

    /// Override the reference ellipsoid.
    ///
    /// Returns a new `WkidTransform` using the given ellipsoid for both the
    /// inverse projection and the cartographic-to-ECEF conversion.
    pub fn with_ellipsoid(mut self, ellipsoid: Ellipsoid) -> Self {
        self.ellipsoid = ellipsoid;
        // Rebuild TM params with the new ellipsoid
        if let TransformKind::TransverseMercator(ref mut params) = self.kind {
            params.ellipsoid = ellipsoid;
        }
        self
    }
}

impl CrsTransform for WkidTransform {
    fn to_ecef(&self, positions: &mut [DVec3]) {
        for pos in positions.iter_mut() {
            let carto = match &self.kind {
                TransformKind::WebMercator => {
                    let mut c = from_web_mercator(pos.x, pos.y, &self.ellipsoid);
                    c.height = pos.z;
                    c
                }
                TransformKind::Geographic => {
                    let mut c = from_geographic_degrees(pos.x, pos.y);
                    c.height = pos.z;
                    c
                }
                TransformKind::TransverseMercator(params) => {
                    let mut c = from_transverse_mercator(pos.x, pos.y, params);
                    c.height = pos.z;
                    c
                }
            };
            *pos = self.ellipsoid.cartographic_to_cartesian(carto);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use i3s::spatial::SpatialReference;

    #[test]
    fn global_crs_from_wkid_4326() {
        let sr = SpatialReference {
            wkid: Some(4326),
            ..Default::default()
        };
        assert_eq!(
            SceneCoordinateSystem::from_spatial_reference(Some(&sr)),
            SceneCoordinateSystem::Global
        );
    }

    #[test]
    fn local_crs_from_wkid_3857() {
        let sr = SpatialReference {
            wkid: Some(3857),
            ..Default::default()
        };
        assert_eq!(
            SceneCoordinateSystem::from_spatial_reference(Some(&sr)),
            SceneCoordinateSystem::Local
        );
    }

    #[test]
    fn local_crs_when_no_spatial_reference() {
        assert_eq!(
            SceneCoordinateSystem::from_spatial_reference(None),
            SceneCoordinateSystem::Local
        );
    }

    #[test]
    fn latest_wkid_takes_precedence() {
        let sr = SpatialReference {
            wkid: Some(3857),
            latest_wkid: Some(4326),
            ..Default::default()
        };
        assert_eq!(
            SceneCoordinateSystem::from_spatial_reference(Some(&sr)),
            SceneCoordinateSystem::Global
        );
    }

    #[test]
    fn global_obb_center_is_ecef() {
        // Denver, CO: lon=-105, lat=39.75, elev=1596m
        let obb = Obb {
            center: [-105.01482, 39.747244, 1596.040551],
            half_size: [29.421873, 29.539055, 22.082193],
            quaternion: [0.420972, -0.055513, -0.118217, 0.897622],
        };

        let converted = obb_from_spec(&obb, SceneCoordinateSystem::Global, None);

        // ECEF center should be roughly at Earth's surface near Denver
        // ~(-1266358, -4725920, 4058016) meters
        let center = converted.center;
        assert!(
            center.length() > 6_000_000.0,
            "should be near Earth surface"
        );
        assert!(center.x < 0.0, "Denver is in western hemisphere (neg x)");
        assert!(center.y < 0.0, "Denver is in western hemisphere (neg y)");
        assert!(center.z > 0.0, "Denver is in northern hemisphere (pos z)");
    }

    #[test]
    fn local_obb_center_unchanged() {
        let obb = Obb {
            center: [1000.0, 2000.0, 50.0],
            half_size: [10.0, 10.0, 5.0],
            quaternion: [0.0, 0.0, 0.0, 1.0],
        };

        let converted = obb_from_spec(&obb, SceneCoordinateSystem::Local, None);

        assert!((converted.center.x - 1000.0).abs() < 1e-6);
        assert!((converted.center.y - 2000.0).abs() < 1e-6);
        assert!((converted.center.z - 50.0).abs() < 1e-6);
    }

    #[test]
    fn global_crs_from_wkid_4490() {
        let sr = SpatialReference {
            wkid: Some(4490),
            ..Default::default()
        };
        assert_eq!(
            SceneCoordinateSystem::from_spatial_reference(Some(&sr)),
            SceneCoordinateSystem::Global
        );
    }

    #[test]
    fn wkid_transform_web_mercator_to_ecef() {
        // Web Mercator (EPSG:3857) point near Denver
        // WM coords for lon=-105, lat=39.75: x≈-11688546, y≈4838472
        let xform = WkidTransform::from_wkid(3857).expect("should support 3857");

        let mut positions = [DVec3::new(-11_688_546.0, 4_838_472.0, 1596.0)];
        xform.to_ecef(&mut positions);

        let ecef = positions[0];
        // Should be near Earth surface
        assert!(
            ecef.length() > 6_000_000.0,
            "ECEF magnitude should be near Earth radius, got {}",
            ecef.length()
        );
        // Denver is in western hemisphere (negative x and y in ECEF)
        assert!(ecef.x < 0.0, "expected negative x for Denver ECEF");
        assert!(ecef.y < 0.0, "expected negative y for Denver ECEF");
        assert!(ecef.z > 0.0, "expected positive z for Denver ECEF");
    }

    #[test]
    fn wkid_transform_from_spatial_reference() {
        let sr = SpatialReference {
            wkid: Some(3857),
            ..Default::default()
        };
        let xform = WkidTransform::from_spatial_reference(&sr);
        assert!(xform.is_some(), "should build transform for EPSG:3857");
    }

    #[test]
    fn wkid_transform_utm_to_ecef() {
        // UTM Zone 10N point near San Francisco
        // Approximate UTM coords: E=551715, N=4179800
        let xform = WkidTransform::from_wkid(32610).expect("should support UTM zone 10N");

        let mut positions = [DVec3::new(551_715.0, 4_179_800.0, 50.0)];
        xform.to_ecef(&mut positions);

        let ecef = positions[0];
        assert!(
            ecef.length() > 6_000_000.0,
            "ECEF should be near Earth surface"
        );
        // SF is western hemisphere, northern hemisphere
        assert!(ecef.x < 0.0, "SF x should be negative");
        assert!(ecef.y < 0.0, "SF y should be negative");
        assert!(ecef.z > 0.0, "SF z should be positive");
    }

    #[test]
    fn wkid_transform_geographic_nad83_to_ecef() {
        // NAD83 (4269) — degrees input
        let xform = WkidTransform::from_wkid(4269).expect("should support NAD83");

        let mut positions = [DVec3::new(-105.0, 39.75, 1596.0)];
        xform.to_ecef(&mut positions);

        let ecef = positions[0];
        assert!(
            ecef.length() > 6_000_000.0,
            "ECEF should be near Earth surface"
        );
        assert!(ecef.x < 0.0, "Denver x should be negative");
    }

    #[test]
    fn wkid_transform_unsupported_returns_none() {
        // An obscure WKID that's not in our table
        assert!(WkidTransform::from_wkid(2227).is_none());
    }

    #[test]
    fn wkid_transform_utm_south_to_ecef() {
        // UTM Zone 56S: Sydney area
        let xform = WkidTransform::from_wkid(32756).expect("should support UTM zone 56S");

        let mut positions = [DVec3::new(334_000.0, 6_252_000.0, 10.0)];
        xform.to_ecef(&mut positions);

        let ecef = positions[0];
        assert!(
            ecef.length() > 6_000_000.0,
            "ECEF should be near Earth surface"
        );
        // Sydney is in southern hemisphere → negative z
        assert!(ecef.z < 0.0, "Sydney z should be negative");
    }

    #[test]
    fn wkid_transform_preserves_elevation() {
        // Verify that the z (elevation) is properly carried through.
        // A geographic CRS point at (0, 0, 1000) should produce ECEF
        // approximately at (WGS84_a + 1000, 0, 0).
        let xform = WkidTransform::from_wkid(4269).unwrap();

        let mut positions = [DVec3::new(0.0, 0.0, 1000.0)];
        xform.to_ecef(&mut positions);

        let ecef = positions[0];
        let expected_x = 6_378_137.0 + 1000.0; // semi_major + height
        assert!(
            (ecef.x - expected_x).abs() < 1.0,
            "x≈{expected_x}, got {}",
            ecef.x
        );
        assert!(ecef.y.abs() < 1.0, "y should be ~0");
        assert!(ecef.z.abs() < 1.0, "z should be ~0");
    }
}
