//! Axis-aligned rectangle on the globe surface.

use std::f64::consts::PI;

/// A rectangle on the globe surface defined by west/south/east/north
/// boundaries in radians.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlobeRectangle {
    /// Western boundary in radians [-PI, PI].
    pub west: f64,
    /// Southern boundary in radians [-PI/2, PI/2].
    pub south: f64,
    /// Eastern boundary in radians [-PI, PI].
    pub east: f64,
    /// Northern boundary in radians [-PI/2, PI/2].
    pub north: f64,
}

impl GlobeRectangle {
    /// Create from radians.
    pub fn new(west: f64, south: f64, east: f64, north: f64) -> Self {
        Self {
            west,
            south,
            east,
            north,
        }
    }

    /// Create from degrees.
    pub fn from_degrees(west: f64, south: f64, east: f64, north: f64) -> Self {
        use i3s_util::math::to_radians;
        Self {
            west: to_radians(west),
            south: to_radians(south),
            east: to_radians(east),
            north: to_radians(north),
        }
    }

    /// Width in radians (handles antimeridian crossing).
    pub fn width(&self) -> f64 {
        let mut w = self.east - self.west;
        if w < 0.0 {
            w += 2.0 * PI;
        }
        w
    }

    /// Height in radians.
    pub fn height(&self) -> f64 {
        self.north - self.south
    }

    /// Center longitude in radians.
    pub fn center_longitude(&self) -> f64 {
        let mut center = (self.west + self.east) * 0.5;
        if self.east < self.west {
            center += PI;
            if center > PI {
                center -= 2.0 * PI;
            }
        }
        center
    }

    /// Center latitude in radians.
    pub fn center_latitude(&self) -> f64 {
        (self.south + self.north) * 0.5
    }

    /// Check if a (longitude, latitude) point in radians is inside this rectangle.
    pub fn contains(&self, longitude: f64, latitude: f64) -> bool {
        if latitude < self.south || latitude > self.north {
            return false;
        }
        if self.east >= self.west {
            longitude >= self.west && longitude <= self.east
        } else {
            // Antimeridian crossing
            longitude >= self.west || longitude <= self.east
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_and_height() {
        let r = GlobeRectangle::from_degrees(-10.0, -5.0, 10.0, 5.0);
        assert!((r.width() - i3s_util::math::to_radians(20.0)).abs() < 1e-12);
        assert!((r.height() - i3s_util::math::to_radians(10.0)).abs() < 1e-12);
    }

    #[test]
    fn contains_point() {
        let r = GlobeRectangle::from_degrees(-180.0, -90.0, 180.0, 90.0);
        assert!(r.contains(0.0, 0.0));
    }

    #[test]
    fn antimeridian_crossing() {
        let r = GlobeRectangle::from_degrees(170.0, -10.0, -170.0, 10.0);
        assert!(r.width() > 0.0);
        assert!(r.contains(i3s_util::math::to_radians(175.0), 0.0));
        assert!(r.contains(i3s_util::math::to_radians(-175.0), 0.0));
        assert!(!r.contains(0.0, 0.0));
    }
}
