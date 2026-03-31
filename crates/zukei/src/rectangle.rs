//! 2D rectangle type.

use glam::DVec2;

/// A 2D rectangle defined by minimum and maximum coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rectangle {
    /// Minimum x-coordinate.
    pub minimum_x: f64,
    /// Minimum y-coordinate.
    pub minimum_y: f64,
    /// Maximum x-coordinate.
    pub maximum_x: f64,
    /// Maximum y-coordinate.
    pub maximum_y: f64,
}

impl Rectangle {
    /// Create a new rectangle from min/max coordinates.
    pub fn new(minimum_x: f64, minimum_y: f64, maximum_x: f64, maximum_y: f64) -> Self {
        Self {
            minimum_x,
            minimum_y,
            maximum_x,
            maximum_y,
        }
    }

    /// Width of the rectangle (`maximum_x - minimum_x`).
    #[inline]
    pub fn width(&self) -> f64 {
        self.maximum_x - self.minimum_x
    }

    /// Height of the rectangle (`maximum_y - minimum_y`).
    #[inline]
    pub fn height(&self) -> f64 {
        self.maximum_y - self.minimum_y
    }

    /// Center of the rectangle.
    #[inline]
    pub fn center(&self) -> DVec2 {
        DVec2::new(
            (self.minimum_x + self.maximum_x) * 0.5,
            (self.minimum_y + self.maximum_y) * 0.5,
        )
    }

    /// Lower-left corner.
    #[inline]
    pub fn lower_left(&self) -> DVec2 {
        DVec2::new(self.minimum_x, self.minimum_y)
    }

    /// Lower-right corner.
    #[inline]
    pub fn lower_right(&self) -> DVec2 {
        DVec2::new(self.maximum_x, self.minimum_y)
    }

    /// Upper-left corner.
    #[inline]
    pub fn upper_left(&self) -> DVec2 {
        DVec2::new(self.minimum_x, self.maximum_y)
    }

    /// Upper-right corner.
    #[inline]
    pub fn upper_right(&self) -> DVec2 {
        DVec2::new(self.maximum_x, self.maximum_y)
    }

    /// Test whether a point is inside this rectangle.
    #[inline]
    pub fn contains(&self, position: DVec2) -> bool {
        position.x >= self.minimum_x
            && position.x <= self.maximum_x
            && position.y >= self.minimum_y
            && position.y <= self.maximum_y
    }

    /// Test whether this rectangle overlaps another.
    #[inline]
    pub fn overlaps(&self, other: &Rectangle) -> bool {
        self.maximum_x >= other.minimum_x
            && self.minimum_x <= other.maximum_x
            && self.maximum_y >= other.minimum_y
            && self.minimum_y <= other.maximum_y
    }

    /// Test whether this rectangle fully contains another rectangle.
    #[inline]
    pub fn fully_contains(&self, other: &Rectangle) -> bool {
        other.minimum_x >= self.minimum_x
            && other.maximum_x <= self.maximum_x
            && other.minimum_y >= self.minimum_y
            && other.maximum_y <= self.maximum_y
    }

    /// Compute signed distance from the rectangle to a position.
    ///
    /// Negative if inside, positive if outside, zero on boundary.
    pub fn signed_distance(&self, position: DVec2) -> f64 {
        let dx = (self.minimum_x - position.x)
            .max(0.0)
            .max(position.x - self.maximum_x);
        let dy = (self.minimum_y - position.y)
            .max(0.0)
            .max(position.y - self.maximum_y);

        if dx == 0.0 && dy == 0.0 {
            // Inside — return negative distance to nearest edge
            let dist_to_edge = (position.x - self.minimum_x)
                .min(self.maximum_x - position.x)
                .min(position.y - self.minimum_y)
                .min(self.maximum_y - position.y);
            -dist_to_edge
        } else {
            (dx * dx + dy * dy).sqrt()
        }
    }

    /// Compute the intersection of this rectangle with another.
    ///
    /// Returns `None` if the rectangles do not overlap.
    pub fn intersection(&self, other: &Rectangle) -> Option<Rectangle> {
        let min_x = self.minimum_x.max(other.minimum_x);
        let min_y = self.minimum_y.max(other.minimum_y);
        let max_x = self.maximum_x.min(other.maximum_x);
        let max_y = self.maximum_y.min(other.maximum_y);

        if min_x > max_x || min_y > max_y {
            None
        } else {
            Some(Rectangle::new(min_x, min_y, max_x, max_y))
        }
    }

    /// Compute the union of this rectangle with another.
    pub fn union(&self, other: &Rectangle) -> Rectangle {
        Rectangle::new(
            self.minimum_x.min(other.minimum_x),
            self.minimum_y.min(other.minimum_y),
            self.maximum_x.max(other.maximum_x),
            self.maximum_y.max(other.maximum_y),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_properties() {
        let r = Rectangle::new(-1.0, -2.0, 3.0, 4.0);
        assert!((r.width() - 4.0).abs() < 1e-12);
        assert!((r.height() - 6.0).abs() < 1e-12);
        assert!((r.center() - DVec2::new(1.0, 1.0)).length() < 1e-12);
    }

    #[test]
    fn corners() {
        let r = Rectangle::new(0.0, 0.0, 10.0, 5.0);
        assert_eq!(r.lower_left(), DVec2::new(0.0, 0.0));
        assert_eq!(r.lower_right(), DVec2::new(10.0, 0.0));
        assert_eq!(r.upper_left(), DVec2::new(0.0, 5.0));
        assert_eq!(r.upper_right(), DVec2::new(10.0, 5.0));
    }

    #[test]
    fn contains_point() {
        let r = Rectangle::new(0.0, 0.0, 10.0, 10.0);
        assert!(r.contains(DVec2::new(5.0, 5.0)));
        assert!(r.contains(DVec2::new(0.0, 0.0)));
        assert!(!r.contains(DVec2::new(-1.0, 5.0)));
        assert!(!r.contains(DVec2::new(5.0, 11.0)));
    }

    #[test]
    fn overlaps_rects() {
        let a = Rectangle::new(0.0, 0.0, 5.0, 5.0);
        let b = Rectangle::new(3.0, 3.0, 8.0, 8.0);
        let c = Rectangle::new(6.0, 6.0, 10.0, 10.0);
        assert!(a.overlaps(&b));
        assert!(!a.overlaps(&c));
    }

    #[test]
    fn fully_contains_rect() {
        let big = Rectangle::new(0.0, 0.0, 10.0, 10.0);
        let small = Rectangle::new(2.0, 2.0, 8.0, 8.0);
        assert!(big.fully_contains(&small));
        assert!(!small.fully_contains(&big));
    }

    #[test]
    fn intersection_overlapping() {
        let a = Rectangle::new(0.0, 0.0, 5.0, 5.0);
        let b = Rectangle::new(3.0, 3.0, 8.0, 8.0);
        let i = a.intersection(&b).unwrap();
        assert!((i.minimum_x - 3.0).abs() < 1e-12);
        assert!((i.minimum_y - 3.0).abs() < 1e-12);
        assert!((i.maximum_x - 5.0).abs() < 1e-12);
        assert!((i.maximum_y - 5.0).abs() < 1e-12);
    }

    #[test]
    fn intersection_none() {
        let a = Rectangle::new(0.0, 0.0, 1.0, 1.0);
        let b = Rectangle::new(5.0, 5.0, 6.0, 6.0);
        assert!(a.intersection(&b).is_none());
    }

    #[test]
    fn union_rects() {
        let a = Rectangle::new(0.0, 0.0, 5.0, 5.0);
        let b = Rectangle::new(3.0, 3.0, 8.0, 8.0);
        let u = a.union(&b);
        assert!((u.minimum_x - 0.0).abs() < 1e-12);
        assert!((u.minimum_y - 0.0).abs() < 1e-12);
        assert!((u.maximum_x - 8.0).abs() < 1e-12);
        assert!((u.maximum_y - 8.0).abs() < 1e-12);
    }

    #[test]
    fn signed_distance_inside() {
        let r = Rectangle::new(0.0, 0.0, 10.0, 10.0);
        let d = r.signed_distance(DVec2::new(5.0, 5.0));
        assert!(d < 0.0);
        assert!((d - (-5.0)).abs() < 1e-12);
    }

    #[test]
    fn signed_distance_outside() {
        let r = Rectangle::new(0.0, 0.0, 10.0, 10.0);
        let d = r.signed_distance(DVec2::new(13.0, 5.0));
        assert!((d - 3.0).abs() < 1e-12);
    }
}
