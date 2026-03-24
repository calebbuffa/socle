/// Sampler wrap mode constants as defined by the glTF specification.
pub mod wrap_mode {
    pub const CLAMP_TO_EDGE: i64 = 33071;
    pub const MIRRORED_REPEAT: i64 = 33648;
    pub const REPEAT: i64 = 10497;
}

/// Applies a sampler wrap mode to a texture coordinate.
///
/// - `REPEAT` (10497): wraps by taking the fractional part.
/// - `MIRRORED_REPEAT` (33648): like repeat, but reverses on odd integer boundaries.
/// - `CLAMP_TO_EDGE` (33071) and unrecognised: clamps to `[0.0, 1.0]`.
pub fn apply_wrap(coord: f64, mode: i64) -> f64 {
    match mode {
        wrap_mode::REPEAT => {
            let fraction = coord.fract();
            if fraction < 0.0 {
                fraction + 1.0
            } else {
                fraction
            }
        }
        wrap_mode::MIRRORED_REPEAT => {
            let integer = coord.trunc();
            let fraction = (coord - integer).abs();
            if (integer.abs() as i64) % 2 == 1 {
                1.0 - fraction
            } else {
                fraction
            }
        }
        _ => coord.clamp(0.0, 1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeat_wraps_positive() {
        assert!((apply_wrap(1.7, wrap_mode::REPEAT) - 0.7).abs() < 1e-10);
    }

    #[test]
    fn repeat_wraps_negative() {
        let result = apply_wrap(-0.3, wrap_mode::REPEAT);
        assert!((result - 0.7).abs() < 1e-10);
    }

    #[test]
    fn mirrored_repeat_odd() {
        let result = apply_wrap(1.3, wrap_mode::MIRRORED_REPEAT);
        assert!((result - 0.7).abs() < 1e-10);
    }

    #[test]
    fn mirrored_repeat_even() {
        let result = apply_wrap(2.3, wrap_mode::MIRRORED_REPEAT);
        assert!((result - 0.3).abs() < 1e-10);
    }

    #[test]
    fn clamp_to_edge() {
        assert!((apply_wrap(1.5, wrap_mode::CLAMP_TO_EDGE) - 1.0).abs() < 1e-10);
        assert!((apply_wrap(-0.5, wrap_mode::CLAMP_TO_EDGE) - 0.0).abs() < 1e-10);
        assert!((apply_wrap(0.5, wrap_mode::CLAMP_TO_EDGE) - 0.5).abs() < 1e-10);
    }
}
