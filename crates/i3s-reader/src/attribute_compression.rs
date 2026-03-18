//! Attribute compression and decompression utilities.
//!
//! Mirrors [`CesiumUtility::AttributeCompression`]:
//!
//! * **Oct-encoding** — compact unit-vector representation using 2 unsigned
//!   integers that together span a +/-1 octahedral map.
//! * **RGB565** — 16-bit packed RGB color decoding.

use glam::DVec3;
use i3s_util::math::{from_snorm, sign_not_zero};

/// Decode a unit-length normal stored in oct encoding with a custom SNORM
/// range.
///
/// `x` and `y` are the two unsigned integer components (both must be in
/// `[0, range_max]`). `range_max` is the largest representable value, i.e.
/// `2^bits - 1`.
///
/// Returns a normalized [`DVec3`].
pub fn oct_decode_in_range<T>(x: T, y: T, range_max: T) -> DVec3
where
    T: Copy + Into<f64>,
{
    let range_max_f = range_max.into();
    let rx = from_snorm(x.into(), range_max_f);
    let ry = from_snorm(y.into(), range_max_f);
    let rz = 1.0 - (rx.abs() + ry.abs());

    let (final_x, final_y) = if rz < 0.0 {
        (
            (1.0 - ry.abs()) * sign_not_zero(rx),
            (1.0 - rx.abs()) * sign_not_zero(ry),
        )
    } else {
        (rx, ry)
    };

    DVec3::new(final_x, final_y, rz).normalize()
}

/// Decode a unit-length normal stored in the standard 2-byte oct encoding
/// (each component in `[0, 255]`).
pub fn oct_decode(x: u8, y: u8) -> DVec3 {
    oct_decode_in_range(x, y, u8::MAX)
}

/// Decode a 16-bit RGB565-packed color into normalized `(r, g, b)` values in
/// `[0.0, 1.0]`.
///
/// Bit layout: `[RRRRR GGGGGG BBBBB]` (5 red, 6 green, 5 blue).
pub fn decode_rgb565(value: u16) -> (f64, f64, f64) {
    let red = (value >> 11) as f64 / 31.0;
    let green = ((value >> 5) & 0x3F) as f64 / 63.0;
    let blue = (value & 0x1F) as f64 / 31.0;
    (red, green, blue)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oct_decode_plus_z() {
        let n = oct_decode(127, 127);
        assert!(n.z > 0.9, "z should be close to 1, got {}", n.z);
        assert!(n.length().abs() - 1.0 < 1e-10, "result must be unit length");
    }

    #[test]
    fn oct_decode_plus_x() {
        let n = oct_decode(255, 127);
        assert!(n.x > 0.9, "x should be close to 1, got {}", n.x);
        assert!((n.length() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn oct_decode_in_range_u16() {
        let n = oct_decode_in_range(u16::MAX / 2, u16::MAX / 2, u16::MAX);
        assert!(n.z > 0.9, "z should be close to 1 for centre of 16-bit oct");
        assert!((n.length() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn decode_rgb565_white() {
        let (r, g, b) = decode_rgb565(0xFFFF);
        assert!((r - 1.0).abs() < 1e-10);
        assert!((g - 1.0).abs() < 1e-10);
        assert!((b - 1.0).abs() < 1e-10);
    }

    #[test]
    fn decode_rgb565_black() {
        let (r, g, b) = decode_rgb565(0x0000);
        assert_eq!(r, 0.0);
        assert_eq!(g, 0.0);
        assert_eq!(b, 0.0);
    }

    #[test]
    fn decode_rgb565_pure_red() {
        let (r, g, b) = decode_rgb565(0xF800);
        assert!((r - 1.0).abs() < 1e-10, "r={r}");
        assert_eq!(g, 0.0);
        assert_eq!(b, 0.0);
    }

    #[test]
    fn decode_rgb565_pure_green() {
        let (r, g, b) = decode_rgb565(0x07E0);
        assert_eq!(r, 0.0);
        assert!((g - 1.0).abs() < 1e-10, "g={g}");
        assert_eq!(b, 0.0);
    }
}
