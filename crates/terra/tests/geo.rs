//! Integration tests for `terra` — geodetic conversions and CRS resolution.

use glam::DVec3;
use terra::{
    BoundingRegion, Cartographic, Crs, CrsRegistry, EcefCrs, Ellipsoid, GeographicCrs,
    GlobeRectangle, SpatialReference, WebMercatorCrs,
};

// ── Ellipsoid ────────────────────────────────────────────────────────────────

#[test]
fn cartographic_to_ecef_equator_prime_meridian() {
    let e = Ellipsoid::wgs84();
    let c = Cartographic::from_degrees(0.0, 0.0, 0.0);
    let ecef = e.cartographic_to_ecef(c);
    // At (0°, 0°, 0m) ECEF should be (a, 0, 0).
    assert!((ecef.x - Ellipsoid::WGS84_A).abs() < 1e-4, "x = {}", ecef.x);
    assert!(ecef.y.abs() < 1e-6, "y = {}", ecef.y);
    assert!(ecef.z.abs() < 1e-6, "z = {}", ecef.z);
}

#[test]
fn cartographic_to_ecef_north_pole() {
    let e = Ellipsoid::wgs84();
    let c = Cartographic::from_degrees(0.0, 90.0, 0.0);
    let ecef = e.cartographic_to_ecef(c);
    // At the north pole ECEF should be (0, 0, b).
    assert!(ecef.x.abs() < 1e-2, "x = {}", ecef.x);
    assert!(ecef.y.abs() < 1e-2, "y = {}", ecef.y);
    assert!((ecef.z - Ellipsoid::WGS84_B).abs() < 1e-4, "z = {}", ecef.z);
}

#[test]
fn round_trip_ecef_cartographic() {
    let e = Ellipsoid::wgs84();
    let test_cases = [
        Cartographic::from_degrees(0.0, 0.0, 0.0),
        Cartographic::from_degrees(45.0, 45.0, 1000.0),
        Cartographic::from_degrees(-73.9857, 40.7484, 50.0), // Empire State Building
        Cartographic::from_degrees(139.6917, 35.6895, 0.0),  // Tokyo
        Cartographic::from_degrees(0.0, 89.9, 100.0),        // Near pole
    ];
    for c in &test_cases {
        let ecef = e.cartographic_to_ecef(*c);
        let back = e.ecef_to_cartographic(ecef).expect("should convert");
        assert!(
            (back.longitude - c.longitude).abs() < 1e-9,
            "lon mismatch for {:?}: got {}",
            c,
            back.longitude.to_degrees()
        );
        assert!(
            (back.latitude - c.latitude).abs() < 1e-9,
            "lat mismatch for {:?}: got {}",
            c,
            back.latitude.to_degrees()
        );
        assert!(
            (back.height - c.height).abs() < 1e-4,
            "height mismatch for {:?}: got {}",
            c,
            back.height
        );
    }
}

#[test]
fn scale_to_geodetic_surface_returns_surface_point() {
    let e = Ellipsoid::wgs84();
    let c = Cartographic::from_degrees(30.0, 60.0, 0.0);
    let ecef = e.cartographic_to_ecef(c);
    // Shift radially outward — scale_to_geodetic_surface should bring it back.
    let elevated = ecef * 1.05;
    let surface = e
        .scale_to_geodetic_surface(elevated)
        .expect("should project");
    // The surface point should have height ≈ 0 above the ellipsoid.
    let carto = e.ecef_to_cartographic(surface).expect("should convert");
    assert!(
        carto.height.abs() < 1e-3,
        "height above surface = {}",
        carto.height
    );
}

#[test]
fn scale_to_geodetic_surface_none_at_centre() {
    let e = Ellipsoid::wgs84();
    assert!(e.scale_to_geodetic_surface(DVec3::ZERO).is_none());
}

// ── Cartographic ─────────────────────────────────────────────────────────────

#[test]
fn cartographic_from_degrees_round_trip() {
    let c = Cartographic::from_degrees(-80.5, 43.2, 250.0);
    let (lon, lat, h) = c.to_degrees();
    assert!((lon + 80.5).abs() < 1e-10);
    assert!((lat - 43.2).abs() < 1e-10);
    assert!((h - 250.0).abs() < 1e-10);
}

// ── GlobeRectangle ───────────────────────────────────────────────────────────

#[test]
fn globe_rectangle_contains() {
    let r = GlobeRectangle::from_degrees(-10.0, -5.0, 10.0, 5.0);
    assert!(r.contains_cartographic(Cartographic::from_degrees(0.0, 0.0, 0.0)));
    assert!(!r.contains_cartographic(Cartographic::from_degrees(15.0, 0.0, 0.0)));
}

#[test]
fn globe_rectangle_intersection() {
    let a = GlobeRectangle::from_degrees(-10.0, -5.0, 10.0, 5.0);
    let b = GlobeRectangle::from_degrees(5.0, 0.0, 20.0, 10.0);
    let i = a.intersection(&b).expect("should intersect");
    assert!((i.west.to_degrees() - 5.0).abs() < 1e-10);
    assert!((i.east.to_degrees() - 10.0).abs() < 1e-10);
}

#[test]
fn globe_rectangle_intersection_none() {
    let a = GlobeRectangle::from_degrees(-20.0, -5.0, -10.0, 5.0);
    let b = GlobeRectangle::from_degrees(10.0, -5.0, 20.0, 5.0);
    assert!(a.intersection(&b).is_none());
}

#[test]
fn globe_rectangle_from_3dtiles_region() {
    use std::f64::consts::PI;
    let region = [-PI, -PI / 2.0, PI, PI / 2.0, -100.0, 500.0];
    let r = GlobeRectangle::from_3dtiles_region(&region).expect("parse");
    assert!((r.west + PI).abs() < 1e-15);
    assert!((r.north - PI / 2.0).abs() < 1e-15);
}

// ── BoundingRegion ───────────────────────────────────────────────────────────

#[test]
fn bounding_region_round_trip_3dtiles() {
    let raw = [-1.0, -0.5, 1.0, 0.5, -50.0, 200.0_f64];
    let br = BoundingRegion::from_3dtiles_region(&raw).expect("parse");
    let out = br.to_3dtiles_region();
    for (a, b) in raw.iter().zip(out.iter()) {
        assert!((a - b).abs() < 1e-15);
    }
}

#[test]
fn bounding_region_contains() {
    let br =
        BoundingRegion::from_3dtiles_region(&[-0.5, -0.5, 0.5, 0.5, 0.0, 100.0]).expect("parse");
    assert!(br.contains_cartographic(Cartographic::new(0.0, 0.0, 50.0)));
    assert!(!br.contains_cartographic(Cartographic::new(0.0, 0.0, 150.0)));
}

// ── CRS ──────────────────────────────────────────────────────────────────────

#[test]
fn geographic_crs_round_trip() {
    let crs = GeographicCrs::wgs84();
    let pos = DVec3::new(-73.9857, 40.7484, 50.0);
    let c = crs.to_cartographic(pos).expect("to_cartographic");
    let back = crs.from_cartographic(c);
    assert!((back.x - pos.x).abs() < 1e-10);
    assert!((back.y - pos.y).abs() < 1e-10);
    assert!((back.z - pos.z).abs() < 1e-10);
}

#[test]
fn ecef_crs_round_trip() {
    let crs = EcefCrs::wgs84();
    let c = Cartographic::from_degrees(10.0, 20.0, 500.0);
    let ecef = crs.from_cartographic(c);
    let back = crs.to_cartographic(ecef).expect("to_cartographic");
    assert!((back.longitude - c.longitude).abs() < 1e-9);
    assert!((back.latitude - c.latitude).abs() < 1e-9);
    assert!((back.height - c.height).abs() < 1e-4);
}

#[test]
fn web_mercator_crs_round_trip() {
    let crs = WebMercatorCrs::wgs84();
    let c = Cartographic::from_degrees(10.0, 50.0, 0.0);
    let projected = crs.from_cartographic(c);
    let back = crs.to_cartographic(projected).expect("to_cartographic");
    assert!((back.longitude - c.longitude).abs() < 1e-10);
    assert!((back.latitude - c.latitude).abs() < 1e-10);
}

// ── SpatialReference ─────────────────────────────────────────────────────────

#[test]
fn spatial_reference_resolves_known_wkids() {
    let reg = CrsRegistry::wgs84();
    assert!(SpatialReference::wgs84().to_crs(&reg).is_some());
    assert!(SpatialReference::web_mercator().to_crs(&reg).is_some());
    assert!(SpatialReference::ecef().to_crs(&reg).is_some());
}

#[test]
fn spatial_reference_unknown_wkid_returns_none() {
    let sr = SpatialReference::from_wkid(32633); // UTM zone 33N — not built-in.
    assert!(sr.to_crs_wgs84().is_none());
}

#[test]
fn spatial_reference_latest_wkid_takes_priority() {
    let sr = SpatialReference {
        wkid: Some(900913),      // Old Google code
        latest_wkid: Some(3857), // Canonical EPSG
        ..Default::default()
    };
    assert_eq!(sr.effective_wkid(), Some(3857));
    assert!(sr.to_crs_wgs84().is_some());
}

#[test]
fn spatial_reference_predicates() {
    assert!(SpatialReference::wgs84().is_geographic());
    assert!(SpatialReference::ecef().is_ecef());
    assert!(SpatialReference::web_mercator().is_web_mercator());
}
