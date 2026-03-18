//! Bounding region: a globe rectangle with height range.

use crate::cartographic::Cartographic;
use crate::ellipsoid::Ellipsoid;
use crate::globe_rectangle::GlobeRectangle;
use i3s_geometry::culling::CullingResult;
use i3s_geometry::obb::OrientedBoundingBox;
use i3s_geometry::plane::Plane;
use i3s_geometry::sphere::BoundingSphere;

/// A bounding region on the globe: a geographic rectangle with minimum
/// and maximum heights above the ellipsoid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingRegion {
    /// Geographic bounds.
    pub rectangle: GlobeRectangle,
    /// Minimum height above the ellipsoid in meters.
    pub minimum_height: f64,
    /// Maximum height above the ellipsoid in meters.
    pub maximum_height: f64,
}

impl BoundingRegion {
    /// Create a bounding region.
    pub fn new(rectangle: GlobeRectangle, minimum_height: f64, maximum_height: f64) -> Self {
        Self {
            rectangle,
            minimum_height,
            maximum_height,
        }
    }

    /// Compute a bounding sphere that encloses this region on the given ellipsoid.
    pub fn to_bounding_sphere(&self, ellipsoid: &Ellipsoid) -> BoundingSphere {
        let center_carto = Cartographic::new(
            self.rectangle.center_longitude(),
            self.rectangle.center_latitude(),
            (self.minimum_height + self.maximum_height) * 0.5,
        );
        let center = ellipsoid.cartographic_to_cartesian(center_carto);

        // Sample corners at min and max height to find radius
        let corners = [
            (self.rectangle.west, self.rectangle.south),
            (self.rectangle.west, self.rectangle.north),
            (self.rectangle.east, self.rectangle.south),
            (self.rectangle.east, self.rectangle.north),
        ];

        let mut max_dist_sq = 0.0_f64;
        for &(lon, lat) in &corners {
            for &h in &[self.minimum_height, self.maximum_height] {
                let p = ellipsoid.cartographic_to_cartesian(Cartographic::new(lon, lat, h));
                max_dist_sq = max_dist_sq.max(center.distance_squared(p));
            }
        }

        BoundingSphere::new(center, max_dist_sq.sqrt())
    }

    /// Check if a cartographic point is inside this region.
    pub fn contains(&self, carto: &Cartographic) -> bool {
        carto.height >= self.minimum_height
            && carto.height <= self.maximum_height
            && self.rectangle.contains(carto.longitude, carto.latitude)
    }

    /// Compute an oriented bounding box (axis-aligned, identity rotation) that
    /// encloses this region on the given ellipsoid.
    ///
    /// The box is derived from the 8 ECEF corner points of the region (4 map
    /// corners × 2 height extremes) plus the 4 edge midpoints at maximum height
    /// to capture any bulge from the ellipsoidal surface.
    pub fn compute_bounding_box(&self, ellipsoid: &Ellipsoid) -> OrientedBoundingBox {
        let lon_mid = self.rectangle.center_longitude();
        let lat_mid = self.rectangle.center_latitude();

        // 8 corners + 4 edge midpoints at each height
        let lon_lat_samples = [
            (self.rectangle.west, self.rectangle.south),
            (self.rectangle.west, self.rectangle.north),
            (self.rectangle.east, self.rectangle.south),
            (self.rectangle.east, self.rectangle.north),
            (lon_mid, self.rectangle.south),
            (lon_mid, self.rectangle.north),
            (self.rectangle.west, lat_mid),
            (self.rectangle.east, lat_mid),
        ];

        let mut pts = Vec::with_capacity(lon_lat_samples.len() * 2);
        for &(lon, lat) in &lon_lat_samples {
            for &h in &[self.minimum_height, self.maximum_height] {
                pts.push(ellipsoid.cartographic_to_cartesian(Cartographic::new(lon, lat, h)));
            }
        }

        OrientedBoundingBox::from_corners(&pts)
    }

    /// Determine on which side of a plane the bounding region lies.
    ///
    /// Returns [`CullingResult::Inside`] if the region is entirely on the
    /// positive (normal) side, [`CullingResult::Outside`] if entirely on the
    /// negative side, and [`CullingResult::Intersecting`] otherwise.
    pub fn intersect_plane(&self, plane: &Plane, ellipsoid: &Ellipsoid) -> CullingResult {
        self.compute_bounding_box(ellipsoid).intersect_plane(plane)
    }

    /// Compute the squared distance from an ECEF position to the closest point
    /// on the axis-aligned bounding box of this region.
    pub fn distance_squared_to(&self, position: glam::DVec3, ellipsoid: &Ellipsoid) -> f64 {
        self.compute_bounding_box(ellipsoid)
            .distance_squared_to(position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounding_sphere_encloses_corners() {
        let r = GlobeRectangle::from_degrees(-1.0, -1.0, 1.0, 1.0);
        let br = BoundingRegion::new(r, 0.0, 1000.0);
        let sphere = br.to_bounding_sphere(&Ellipsoid::WGS84);
        // All corners should be inside the sphere
        let corners = [
            (r.west, r.south),
            (r.west, r.north),
            (r.east, r.south),
            (r.east, r.north),
        ];
        for &(lon, lat) in &corners {
            for &h in &[0.0, 1000.0] {
                let p = Ellipsoid::WGS84.cartographic_to_cartesian(Cartographic::new(lon, lat, h));
                assert!(
                    sphere.contains(p),
                    "corner ({}, {}, {}) should be inside sphere",
                    lon,
                    lat,
                    h
                );
            }
        }
    }

    #[test]
    fn contains_cartographic() {
        let r = GlobeRectangle::from_degrees(-10.0, -10.0, 10.0, 10.0);
        let br = BoundingRegion::new(r, 0.0, 100.0);
        assert!(br.contains(&Cartographic::from_degrees(0.0, 0.0, 50.0)));
        assert!(!br.contains(&Cartographic::from_degrees(0.0, 0.0, 200.0)));
        assert!(!br.contains(&Cartographic::from_degrees(20.0, 0.0, 50.0)));
    }

    #[test]
    fn bounding_box_encloses_corners() {
        let r = GlobeRectangle::from_degrees(-5.0, -5.0, 5.0, 5.0);
        let br = BoundingRegion::new(r, 0.0, 500.0);
        let obb = br.compute_bounding_box(&Ellipsoid::WGS84);
        let corners = [(r.west, r.south), (r.west, r.north), (r.east, r.south), (r.east, r.north)];
        for &(lon, lat) in &corners {
            for &h in &[0.0, 500.0] {
                let p = Ellipsoid::WGS84.cartographic_to_cartesian(Cartographic::new(lon, lat, h));
                assert!(obb.contains(p), "corner ({lon}, {lat}, {h}) not contained in OBB");
            }
        }
    }

    #[test]
    fn intersect_plane_inside() {
        use glam::DVec3;
        // Small tile near (0°, 0°): ECEF center is near (6.378e6, 0, 0).
        // A plane with normal=+X and distance=-5e6 sits at x=5e6.
        // The region center (x≈6.378e6) is on the positive (+X) side,
        // far enough that the whole OBB should be Inside.
        let r = GlobeRectangle::from_degrees(-1.0, -1.0, 1.0, 1.0);
        let br = BoundingRegion::new(r, 0.0, 1000.0);
        let plane = Plane::new(DVec3::X, -5.0e6);
        let result = br.intersect_plane(&plane, &Ellipsoid::WGS84);
        assert_eq!(result, CullingResult::Inside);
    }

    #[test]
    fn distance_squared_point_outside() {
        use glam::DVec3;
        let r = GlobeRectangle::from_degrees(-1.0, -1.0, 1.0, 1.0);
        let br = BoundingRegion::new(r, 0.0, 1000.0);
        // A point far away should have non-zero distance
        let far_point = DVec3::new(1e10, 0.0, 0.0);
        let d2 = br.distance_squared_to(far_point, &Ellipsoid::WGS84);
        assert!(d2 > 0.0);
    }
}
