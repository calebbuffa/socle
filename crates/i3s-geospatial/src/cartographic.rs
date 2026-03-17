//! Geographic position in radians and meters.

use i3s_util::math;

/// A geographic position: longitude, latitude (in radians), and height
/// (in meters above the ellipsoid surface).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cartographic {
    /// Longitude in radians, range [-PI, PI].
    pub longitude: f64,
    /// Latitude in radians, range [-PI/2, PI/2].
    pub latitude: f64,
    /// Height in meters above the ellipsoid.
    pub height: f64,
}

impl Cartographic {
    /// Create a new cartographic position from radians.
    pub fn new(longitude: f64, latitude: f64, height: f64) -> Self {
        Self {
            longitude,
            latitude,
            height,
        }
    }

    /// Create a cartographic position from degrees.
    pub fn from_degrees(longitude: f64, latitude: f64, height: f64) -> Self {
        Self {
            longitude: math::to_radians(longitude),
            latitude: math::to_radians(latitude),
            height,
        }
    }

    /// Longitude in degrees.
    pub fn longitude_degrees(&self) -> f64 {
        math::to_degrees(self.longitude)
    }

    /// Latitude in degrees.
    pub fn latitude_degrees(&self) -> f64 {
        math::to_degrees(self.latitude)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_degrees_roundtrip() {
        let c = Cartographic::from_degrees(-122.4194, 37.7749, 100.0);
        assert!((c.longitude_degrees() - (-122.4194)).abs() < 1e-10);
        assert!((c.latitude_degrees() - 37.7749).abs() < 1e-10);
        assert!((c.height - 100.0).abs() < 1e-12);
    }
}
