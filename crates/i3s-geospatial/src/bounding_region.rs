//! Bounding region: a globe rectangle with height range.

use crate::cartographic::Cartographic;
use crate::ellipsoid::Ellipsoid;
use crate::globe_rectangle::GlobeRectangle;
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
}
