//! Three-dimensional geodetic bounding volume: a geodetic rectangle plus height range.

use glam::DVec3;

use crate::{Cartographic, Ellipsoid, GlobeRectangle};
use std::f64::consts::PI;

/// A 3-D geodetic bounding volume defined by a [`GlobeRectangle`] (longitude /
/// latitude extents in radians) and a height range (metres above the ellipsoid).
///
/// Directly corresponds to the `boundingVolume.region` field in 3D Tiles
/// (`[west, south, east, north, minHeight, maxHeight]`) and to I3S `fullExtent`
/// when the spatial reference is geographic.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BoundingRegion {
    /// The horizontal extent in geodetic coordinates.
    pub rectangle: GlobeRectangle,
    /// Minimum height above the ellipsoid (metres). May be negative.
    pub minimum_height: f64,
    /// Maximum height above the ellipsoid (metres).
    pub maximum_height: f64,
}

impl BoundingRegion {
    /// Construct from components.
    #[inline]
    pub const fn new(rectangle: GlobeRectangle, minimum_height: f64, maximum_height: f64) -> Self {
        Self {
            rectangle,
            minimum_height,
            maximum_height,
        }
    }

    /// Parse from a 3D Tiles `boundingVolume.region` array.
    ///
    /// Expected format: `[west_rad, south_rad, east_rad, north_rad, min_height_m, max_height_m]`.
    /// Returns `None` if the slice has fewer than 6 elements.
    pub fn from_3dtiles_region(region: &[f64]) -> Option<Self> {
        if region.len() < 6 {
            return None;
        }
        Some(Self {
            rectangle: GlobeRectangle::new(region[0], region[1], region[2], region[3]),
            minimum_height: region[4],
            maximum_height: region[5],
        })
    }

    /// Serialise to the 3D Tiles `boundingVolume.region` array format.
    pub fn to_3dtiles_region(&self) -> [f64; 6] {
        [
            self.rectangle.west,
            self.rectangle.south,
            self.rectangle.east,
            self.rectangle.north,
            self.minimum_height,
            self.maximum_height,
        ]
    }

    /// Return true if the given cartographic position lies within this region.
    pub fn contains_cartographic(&self, c: Cartographic) -> bool {
        self.rectangle.contains_cartographic(c)
            && c.height >= self.minimum_height
            && c.height <= self.maximum_height
    }

    /// Return the centre of the region as a [`Cartographic`] position.
    pub fn center_cartographic(&self) -> Cartographic {
        let centre = self.rectangle.center();
        Cartographic::new(
            centre.longitude,
            centre.latitude,
            (self.minimum_height + self.maximum_height) * 0.5,
        )
    }

    /// Height span: `maximum_height − minimum_height`.
    #[inline]
    pub fn height_range(&self) -> f64 {
        self.maximum_height - self.minimum_height
    }

    /// Return an expanded region that contains both `self` and `other`.
    pub fn union(&self, other: &Self) -> Self {
        Self {
            rectangle: GlobeRectangle::new(
                self.rectangle.west.min(other.rectangle.west),
                self.rectangle.south.min(other.rectangle.south),
                self.rectangle.east.max(other.rectangle.east),
                self.rectangle.north.max(other.rectangle.north),
            ),
            minimum_height: self.minimum_height.min(other.minimum_height),
            maximum_height: self.maximum_height.max(other.maximum_height),
        }
    }

    // ---- distance ------------------------------------------------------------

    /// Computes the distance squared (metres²) from an ECEF position to the
    /// closest point in this bounding region.
    ///
    /// Returns `0.0` if the position is inside the region or cannot be
    /// projected to a geodetic coordinate.
    ///
    /// Mirrors `CesiumGeospatial::BoundingRegion::computeDistanceSquaredToPosition`.
    pub fn compute_distance_squared_to_ecef(&self, ecef: DVec3, ellipsoid: &Ellipsoid) -> f64 {
        match ellipsoid.ecef_to_cartographic(ecef) {
            Some(carto) => self.distance_squared_impl(carto, ecef, ellipsoid),
            None => 0.0,
        }
    }

    /// Computes the distance squared (metres²) from a cartographic position to
    /// the closest point in this bounding region.
    pub fn compute_distance_squared_to_cartographic(
        &self,
        pos: Cartographic,
        ellipsoid: &Ellipsoid,
    ) -> f64 {
        let ecef = ellipsoid.cartographic_to_ecef(pos);
        self.distance_squared_impl(pos, ecef, ellipsoid)
    }

    /// Computes the distance squared when both ECEF and cartographic forms of
    /// the position are already known (avoids a redundant conversion).
    pub fn compute_distance_squared(
        &self,
        carto: Cartographic,
        ecef: DVec3,
        ellipsoid: &Ellipsoid,
    ) -> f64 {
        self.distance_squared_impl(carto, ecef, ellipsoid)
    }

    /// Core distance-squared computation.
    ///
    /// Uses bounding-plane normals aligned with the geodetic frame, matching
    /// Cesium's `BoundingRegion::computeDistanceSquaredToPosition` algorithm.
    fn distance_squared_impl(
        &self,
        carto: Cartographic,
        ecef: DVec3,
        ellipsoid: &Ellipsoid,
    ) -> f64 {
        let rect = self.rectangle;
        let mut result = 0.0;

        if !rect.contains_cartographic(carto) {
            let mid_lat = (rect.south + rect.north) * 0.5;
            let west_mid =
                ellipsoid.cartographic_to_ecef(Cartographic::new(rect.west, mid_lat, 0.0));
            let east_mid =
                ellipsoid.cartographic_to_ecef(Cartographic::new(rect.east, mid_lat, 0.0));

            // West/east plane normals are in the meridian plane: N = west_mid × Ẑ (or Ẑ × east_mid).
            let west_normal = west_mid.cross(DVec3::Z).normalize_or_zero();
            let east_normal = DVec3::Z.cross(east_mid).normalize_or_zero();

            let east_west = west_mid - east_mid;
            if east_west.length_squared() > 1e-20 {
                // South plane: surface normal at south edge, crossed into east_west.
                let south_surface_normal = if rect.south > 0.0 {
                    // Entirely in northern hemisphere: use midpoint to get a more
                    // conservative (non-intersecting) plane.
                    let sc = ellipsoid.cartographic_to_ecef(Cartographic::new(
                        (rect.west + rect.east) * 0.5,
                        rect.south,
                        0.0,
                    ));
                    ellipsoid.geodetic_surface_normal(sc)
                } else {
                    let se = ellipsoid
                        .cartographic_to_ecef(Cartographic::new(rect.east, rect.south, 0.0));
                    ellipsoid.geodetic_surface_normal(se)
                };
                let south_normal = south_surface_normal.cross(east_west).normalize_or_zero();

                // North plane: surface normal at north edge, crossed with east_west.
                let north_surface_normal = if rect.north < 0.0 {
                    // Entirely in southern hemisphere: use midpoint.
                    let nc = ellipsoid.cartographic_to_ecef(Cartographic::new(
                        (rect.west + rect.east) * 0.5,
                        rect.north,
                        0.0,
                    ));
                    ellipsoid.geodetic_surface_normal(nc)
                } else {
                    let nw = ellipsoid
                        .cartographic_to_ecef(Cartographic::new(rect.west, rect.north, 0.0));
                    ellipsoid.geodetic_surface_normal(nw)
                };
                let north_normal = east_west.cross(north_surface_normal).normalize_or_zero();

                // Corner reference points for plane-distance queries.
                let sw =
                    ellipsoid.cartographic_to_ecef(Cartographic::new(rect.west, rect.south, 0.0));
                let ne =
                    ellipsoid.cartographic_to_ecef(Cartographic::new(rect.east, rect.north, 0.0));

                let from_sw = ecef - sw;
                let dist_west = from_sw.dot(west_normal);
                let dist_south = from_sw.dot(south_normal);

                let from_ne = ecef - ne;
                let dist_east = from_ne.dot(east_normal);
                let dist_north = from_ne.dot(north_normal);

                // Longitude contribution: outside west OR outside east.
                if dist_west > 0.0 {
                    result += dist_west * dist_west;
                } else if dist_east > 0.0 {
                    result += dist_east * dist_east;
                }

                // Latitude contribution: outside south OR outside north.
                if dist_south > 0.0 {
                    result += dist_south * dist_south;
                } else if dist_north > 0.0 {
                    result += dist_north * dist_north;
                }
            }
        }

        // Height contribution.
        let h = carto.height;
        if h > self.maximum_height {
            let d = h - self.maximum_height;
            result += d * d;
        } else if h < self.minimum_height {
            let d = self.minimum_height - h;
            result += d * d;
        }

        result
    }
}

/// # Usage
///
/// ```rust
/// use terra::{BoundingRegionBuilder, Cartographic, Ellipsoid};
///
/// let mut builder = BoundingRegionBuilder::new();
/// builder.expand_to_include_position(Cartographic::from_degrees(-73.9857, 40.7484, 50.0));
/// builder.expand_to_include_position(Cartographic::from_degrees(139.6917, 35.6895, 0.0));
/// let region = builder.to_region(&Ellipsoid::wgs84());
/// ```
///
/// # Longitude arithmetic
///
/// The builder keeps longitude ranges as small as possible, accounting for
/// antimeridian crossing.  When a new position falls outside the current east
/// or west edge, the algorithm extends in whichever direction requires the
/// smaller angular delta (same logic as Cesium).
#[derive(Debug, Clone)]
pub struct BoundingRegionBuilder {
    /// Accumulated south latitude (radians).  Initialised to `f64::MAX`.
    south: f64,
    /// Accumulated north latitude (radians).  Initialised to `f64::MIN`.
    north: f64,
    /// Accumulated west longitude (radians).  Only valid when `!longitude_range_is_empty`.
    west: f64,
    /// Accumulated east longitude (radians).  Only valid when `!longitude_range_is_empty`.
    east: f64,
    minimum_height: f64,
    maximum_height: f64,
    /// How close to a pole (radians) before ignoring longitude.
    pole_tolerance: f64,
    /// `true` until the first non-polar position is added.
    longitude_range_is_empty: bool,
    /// `true` until the first position (any) is added.
    lat_height_range_is_empty: bool,
}

impl Default for BoundingRegionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl BoundingRegionBuilder {
    /// The default pole proximity tolerance (matches Cesium's `EPSILON10`).
    pub const DEFAULT_POLE_TOLERANCE: f64 = 1e-10;

    /// Create a new empty builder.
    pub fn new() -> Self {
        Self {
            south: f64::MAX,
            north: f64::MIN,
            west: 0.0,
            east: 0.0,
            minimum_height: f64::MAX,
            maximum_height: f64::MIN,
            pole_tolerance: Self::DEFAULT_POLE_TOLERANCE,
            longitude_range_is_empty: true,
            lat_height_range_is_empty: true,
        }
    }

    /// The distance from a pole (radians) below which longitude is ignored.
    pub fn pole_tolerance(&self) -> f64 {
        self.pole_tolerance
    }

    /// Override the pole proximity tolerance.
    pub fn set_pole_tolerance(&mut self, tolerance: f64) {
        self.pole_tolerance = tolerance;
    }

    /// Build the final [`BoundingRegion`].
    ///
    /// If no positions were added, returns an empty region (rectangle =
    /// `GlobeRectangle::EMPTY`, min_height = 1.0, max_height = -1.0).
    pub fn to_region(&self, _ellipsoid: &Ellipsoid) -> BoundingRegion {
        if self.lat_height_range_is_empty {
            BoundingRegion::new(GlobeRectangle::EMPTY, 1.0, -1.0)
        } else {
            BoundingRegion::new(
                self.to_globe_rectangle(),
                self.minimum_height,
                self.maximum_height,
            )
        }
    }

    /// Return the accumulated [`GlobeRectangle`] without height information.
    ///
    /// Returns `GlobeRectangle::EMPTY` if no positions were added.
    pub fn to_globe_rectangle(&self) -> GlobeRectangle {
        if self.lat_height_range_is_empty {
            GlobeRectangle::EMPTY
        } else {
            // When only polar positions were added, west/east default to 0/0.
            let (w, e) = if self.longitude_range_is_empty {
                (0.0, 0.0)
            } else {
                (self.west, self.east)
            };
            GlobeRectangle::new(w, self.south, e, self.north)
        }
    }

    /// Expand the bounding region to include `position`.
    ///
    /// Returns `true` if the region was modified.
    pub fn expand_to_include_position(&mut self, position: Cartographic) -> bool {
        let mut modified = false;

        // Latitude and height are always updated.
        if position.latitude < self.south {
            self.south = position.latitude;
            modified = true;
        }
        if position.latitude > self.north {
            self.north = position.latitude;
            modified = true;
        }
        if position.height < self.minimum_height {
            self.minimum_height = position.height;
            modified = true;
        }
        if position.height > self.maximum_height {
            self.maximum_height = position.height;
            modified = true;
        }
        if modified {
            self.lat_height_range_is_empty = false;
        }

        // Longitude is only updated if the position is not too close to a pole.
        let is_polar = (PI / 2.0 - position.latitude.abs()) < self.pole_tolerance;
        if !is_polar {
            if self.longitude_range_is_empty {
                self.west = position.longitude;
                self.east = position.longitude;
                self.longitude_range_is_empty = false;
                modified = true;
            } else {
                // Check if position is already within [west, east].
                let contained = {
                    let tmp = GlobeRectangle::new(self.west, self.south, self.east, self.north);
                    tmp.contains_cartographic(position)
                };
                if !contained {
                    // Compute minimum angular delta to extend east vs. west.
                    let mut dist_to_west = self.west - position.longitude;
                    if dist_to_west < 0.0 {
                        // Going the long way around the antimeridian.
                        dist_to_west = (self.west - (-PI)) + (PI - position.longitude);
                    }

                    let mut dist_from_east = position.longitude - self.east;
                    if dist_from_east < 0.0 {
                        dist_from_east = (position.longitude - (-PI)) + (PI - self.east);
                    }

                    if dist_to_west < dist_from_east {
                        self.west = position.longitude;
                    } else {
                        self.east = position.longitude;
                    }
                    modified = true;
                }
            }
        }

        modified
    }

    /// Expand the bounding region to include a [`GlobeRectangle`].
    ///
    /// Returns `true` if the region was modified.
    pub fn expand_to_include_globe_rectangle(&mut self, rect: GlobeRectangle) -> bool {
        let mut modified = false;

        // Lat/height: treat the rect corners as positions with height 0 for
        // tracking south/north (heights are not carried by GlobeRectangle).
        if rect.south < self.south {
            self.south = rect.south;
            modified = true;
        }
        if rect.north > self.north {
            self.north = rect.north;
            modified = true;
        }
        if modified {
            self.lat_height_range_is_empty = false;
        }

        // Longitude: union with current range.
        if self.longitude_range_is_empty {
            self.west = rect.west;
            self.east = rect.east;
            self.longitude_range_is_empty = false;
            modified = true;
        } else {
            let current = GlobeRectangle::new(self.west, self.south, self.east, self.north);
            let unioned = globe_rectangle_union(current, rect);
            if unioned.west != self.west || unioned.east != self.east {
                self.west = unioned.west;
                self.east = unioned.east;
                modified = true;
            }
        }

        modified
    }
}

/// Compute the smallest [`GlobeRectangle`] that contains both `a` and `b`.
///
/// Handles the antimeridian by choosing the union orientation that minimises
/// total width.
fn globe_rectangle_union(a: GlobeRectangle, b: GlobeRectangle) -> GlobeRectangle {
    let south = a.south.min(b.south);
    let north = a.north.max(b.north);

    // Expand west/east to cover both rectangles.
    // Try both orientations and pick the narrower one.
    let west = a.west.min(b.west);
    let east = a.east.max(b.east);

    // Handle antimeridian: if the merged rectangle would be wider than the
    // alternative wrapping version, use the wrapping version.
    let width_direct = east - west;
    let west_alt = a.west.max(b.west); // larger west
    let east_alt = a.east.min(b.east); // smaller east (might need to go via antimeridian)
    let width_alt = if east_alt < west_alt {
        // Wrapping: distance from west_alt → 180° + from −180° → east_alt
        (PI - west_alt) + (east_alt + PI)
    } else {
        width_direct // no benefit to alternative
    };

    if width_alt < width_direct && width_alt >= 0.0 {
        GlobeRectangle::new(west_alt, south, east_alt, north)
    } else {
        GlobeRectangle::new(west, south, east, north)
    }
}

/// A [`BoundingRegion`] whose height values may be very inaccurate.
///
/// This is a semantic wrapper that marks a [`BoundingRegion`] as having
/// imprecise (loose-fitting) heights, and provides conservative distance
/// metrics suitable for level-of-detail (LOD) selection.
///
/// The word *conservative* here means the computed distance may be larger
/// than the true geometric distance, which biases tile selection toward
/// loading *less* detail when heights are uncertain — preferable to loading
/// too much detail based on incorrect height proximity.
///
/// Mirrors `CesiumGeospatial::BoundingRegionWithLooseFittingHeights`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingRegionWithLooseFittingHeights {
    region: BoundingRegion,
}

impl BoundingRegionWithLooseFittingHeights {
    /// Wrap a [`BoundingRegion`] whose heights are imprecise.
    #[inline]
    pub fn new(region: BoundingRegion) -> Self {
        Self { region }
    }

    /// Returns the wrapped bounding region.
    #[inline]
    pub fn bounding_region(&self) -> &BoundingRegion {
        &self.region
    }

    /// Conservative distance squared (metres²) to an ECEF position.
    ///
    /// Delegates to [`BoundingRegion::compute_distance_squared_to_ecef`].
    #[inline]
    pub fn conservative_distance_squared_to_ecef(&self, ecef: DVec3, ellipsoid: &Ellipsoid) -> f64 {
        self.region
            .compute_distance_squared_to_ecef(ecef, ellipsoid)
    }

    /// Conservative distance squared (metres²) to a cartographic position.
    ///
    /// Delegates to [`BoundingRegion::compute_distance_squared_to_cartographic`].
    #[inline]
    pub fn conservative_distance_squared_to_cartographic(
        &self,
        pos: Cartographic,
        ellipsoid: &Ellipsoid,
    ) -> f64 {
        self.region
            .compute_distance_squared_to_cartographic(pos, ellipsoid)
    }

    /// Conservative distance squared when both ECEF and cartographic forms are
    /// already available (avoids a redundant coordinate conversion).
    #[inline]
    pub fn conservative_distance_squared(
        &self,
        carto: Cartographic,
        ecef: DVec3,
        ellipsoid: &Ellipsoid,
    ) -> f64 {
        self.region.compute_distance_squared(carto, ecef, ellipsoid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GlobeRectangle;

    fn e() -> Ellipsoid {
        Ellipsoid::wgs84()
    }

    /// A 10°×10° region at 0–100 m height, centred on the equator/prime-meridian.
    fn small_region() -> BoundingRegion {
        BoundingRegion::new(
            GlobeRectangle::new(0.0, 0.0, 10_f64.to_radians(), 10_f64.to_radians()),
            0.0,
            100.0,
        )
    }

    #[test]
    fn inside_cartographic_gives_zero() {
        let brlh = BoundingRegionWithLooseFittingHeights::new(small_region());
        let inside = Cartographic::new(5_f64.to_radians(), 5_f64.to_radians(), 50.0);
        let dist = brlh.conservative_distance_squared_to_cartographic(inside, &e());
        assert_eq!(dist, 0.0, "position inside region should yield distance 0");
    }

    #[test]
    fn inside_ecef_gives_zero() {
        let brlh = BoundingRegionWithLooseFittingHeights::new(small_region());
        let ell = e();
        let inside_carto = Cartographic::new(5_f64.to_radians(), 5_f64.to_radians(), 50.0);
        let inside_ecef = ell.cartographic_to_ecef(inside_carto);
        let dist = brlh.conservative_distance_squared_to_ecef(inside_ecef, &ell);
        assert_eq!(dist, 0.0, "ECEF inside region should yield distance 0");
    }

    #[test]
    fn above_region_gives_height_squared() {
        let brlh = BoundingRegionWithLooseFittingHeights::new(small_region());
        // 200 m above the surface, inside the lat/lon box → only height matters
        let pos = Cartographic::new(5_f64.to_radians(), 5_f64.to_radians(), 200.0);
        let dist = brlh.conservative_distance_squared_to_cartographic(pos, &e());
        let expected = (200.0 - 100.0_f64).powi(2); // 100² = 10 000
        assert!(
            (dist - expected).abs() < 1.0,
            "above: got {dist}, expected {expected}"
        );
    }

    #[test]
    fn below_region_gives_height_squared() {
        let brlh = BoundingRegionWithLooseFittingHeights::new(small_region());
        let pos = Cartographic::new(5_f64.to_radians(), 5_f64.to_radians(), -50.0);
        let dist = brlh.conservative_distance_squared_to_cartographic(pos, &e());
        let expected = (0.0 - (-50.0_f64)).powi(2); // 50² = 2 500
        assert!(
            (dist - expected).abs() < 1.0,
            "below: got {dist}, expected {expected}"
        );
    }

    #[test]
    fn outside_horizontally_gives_nonzero_distance() {
        let brlh = BoundingRegionWithLooseFittingHeights::new(small_region());
        // Far west of the region, at mid-height → only horizontal distance
        let pos = Cartographic::new((-10_f64).to_radians(), 5_f64.to_radians(), 50.0);
        let dist = brlh.conservative_distance_squared_to_cartographic(pos, &e());
        assert!(
            dist > 0.0,
            "position outside rectangle (west) must have nonzero distance"
        );
    }

    #[test]
    fn bounding_region_accessor_roundtrips() {
        let r = small_region();
        let brlh = BoundingRegionWithLooseFittingHeights::new(r);
        assert_eq!(*brlh.bounding_region(), r);
    }

    #[test]
    fn delegates_identically_to_bounding_region() {
        let r = small_region();
        let brlh = BoundingRegionWithLooseFittingHeights::new(r);
        let ell = e();
        // Test a position above the region.
        let pos = Cartographic::new(5_f64.to_radians(), 5_f64.to_radians(), 500.0);
        let d_brlh = brlh.conservative_distance_squared_to_cartographic(pos, &ell);
        let d_region = r.compute_distance_squared_to_cartographic(pos, &ell);
        assert_eq!(
            d_brlh, d_region,
            "BRLH must delegate exactly to BoundingRegion"
        );
    }

    #[test]
    fn combined_ecef_cartographic_method_consistent() {
        let r = small_region();
        let brlh = BoundingRegionWithLooseFittingHeights::new(r);
        let ell = e();
        let carto = Cartographic::new(5_f64.to_radians(), 5_f64.to_radians(), 200.0);
        let ecef = ell.cartographic_to_ecef(carto);
        let d1 = brlh.conservative_distance_squared_to_cartographic(carto, &ell);
        let d2 = brlh.conservative_distance_squared(carto, ecef, &ell);
        assert_eq!(d1, d2, "cartographic and combined methods must agree");
    }

    #[test]
    fn empty_builder_gives_empty_region() {
        let b = BoundingRegionBuilder::new();
        let r = b.to_region(&e());
        assert!(r.minimum_height > r.maximum_height, "empty heights");
        assert_eq!(b.to_globe_rectangle(), GlobeRectangle::EMPTY);
    }

    #[test]
    fn single_point_collapses_to_point() {
        let mut b = BoundingRegionBuilder::new();
        let c = Cartographic::from_degrees(10.0, 20.0, 100.0);
        assert!(b.expand_to_include_position(c));
        let rect = b.to_globe_rectangle();
        assert!((rect.west - c.longitude).abs() < 1e-12);
        assert!((rect.east - c.longitude).abs() < 1e-12);
        assert!((rect.south - c.latitude).abs() < 1e-12);
        assert!((rect.north - c.latitude).abs() < 1e-12);
    }

    #[test]
    fn two_points_same_hemisphere() {
        let mut b = BoundingRegionBuilder::new();
        b.expand_to_include_position(Cartographic::from_degrees(-10.0, -20.0, 0.0));
        b.expand_to_include_position(Cartographic::from_degrees(30.0, 40.0, 500.0));
        let r = b.to_region(&e());
        let rect = r.rectangle;
        assert!((rect.west - (-10.0_f64).to_radians()).abs() < 1e-10);
        assert!((rect.south - (-20.0_f64).to_radians()).abs() < 1e-10);
        assert!((rect.east - 30.0_f64.to_radians()).abs() < 1e-10);
        assert!((rect.north - 40.0_f64.to_radians()).abs() < 1e-10);
        assert!((r.minimum_height - 0.0).abs() < 1e-12);
        assert!((r.maximum_height - 500.0).abs() < 1e-12);
    }

    #[test]
    fn height_range_tracked_correctly() {
        let mut b = BoundingRegionBuilder::new();
        b.expand_to_include_position(Cartographic::new(0.0, 0.0, -50.0));
        b.expand_to_include_position(Cartographic::new(0.1, 0.1, 200.0));
        let r = b.to_region(&e());
        assert!((r.minimum_height - (-50.0)).abs() < 1e-12);
        assert!((r.maximum_height - 200.0).abs() < 1e-12);
    }

    #[test]
    fn same_point_twice_modifies_only_first_time() {
        let mut b = BoundingRegionBuilder::new();
        let c = Cartographic::from_degrees(5.0, 5.0, 0.0);
        assert!(b.expand_to_include_position(c));
        assert!(
            !b.expand_to_include_position(c),
            "second call at same position should be no-op"
        );
    }

    #[test]
    fn expand_by_globe_rectangle() {
        let mut b = BoundingRegionBuilder::new();
        let rect = GlobeRectangle::from_degrees(-45.0, -30.0, 45.0, 30.0);
        b.expand_to_include_globe_rectangle(rect);
        let got = b.to_globe_rectangle();
        assert!((got.west - rect.west).abs() < 1e-12);
        assert!((got.east - rect.east).abs() < 1e-12);
        assert!((got.south - rect.south).abs() < 1e-12);
        assert!((got.north - rect.north).abs() < 1e-12);
    }

    #[test]
    fn near_pole_position_does_not_update_longitude() {
        let mut b = BoundingRegionBuilder::new();
        // First add a normal point to set the longitude range.
        b.expand_to_include_position(Cartographic::from_degrees(0.0, 0.0, 0.0));
        let initial_west = b.to_globe_rectangle().west;

        // Add a point near the pole with a very different longitude.
        let near_pole = Cartographic::new(-PI + 0.001, PI / 2.0 - 1e-11, 0.0);
        b.expand_to_include_position(near_pole);

        // Longitude should be unchanged.
        assert!(
            (b.to_globe_rectangle().west - initial_west).abs() < 1e-12,
            "longitude should not change near pole"
        );
    }

    #[test]
    fn default_is_same_as_new() {
        let a = BoundingRegionBuilder::new();
        let b = BoundingRegionBuilder::default();
        assert_eq!(a.to_globe_rectangle(), b.to_globe_rectangle());
    }
}
