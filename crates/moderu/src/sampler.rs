/// Wrap mode for texture coordinates, as defined by glTF 2.0 / OpenGL ES.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WrapMode {
    /// Coordinates clamp to [0, 1]. (glTF code: 33071)
    ClampToEdge,
    /// Coordinate mirrors on each integer boundary. (glTF code: 33648)
    MirroredRepeat,
    /// Coordinate wraps (fractional part only). (glTF code: 10497).
    /// This is the glTF 2.0 default when no sampler is specified.
    #[default]
    Repeat,
}

impl WrapMode {
    /// Construct from a glTF integer wrap-mode code.
    /// Unknown codes fall back to `ClampToEdge`.
    pub fn from_gltf(code: u32) -> Self {
        match code {
            10497 => Self::Repeat,
            33648 => Self::MirroredRepeat,
            _ => Self::ClampToEdge,
        }
    }

    /// The glTF integer wrap-mode code for this mode.
    pub const fn to_gltf(self) -> u32 {
        match self {
            Self::ClampToEdge => 33071,
            Self::MirroredRepeat => 33648,
            Self::Repeat => 10497,
        }
    }

    /// Apply this wrap mode to a texture coordinate, returning a value in `[0, 1]`.
    pub fn apply(self, coord: f64) -> f64 {
        match self {
            Self::Repeat => {
                let f = coord.fract();
                if f < 0.0 { f + 1.0 } else { f }
            }
            Self::MirroredRepeat => {
                let abs = coord.abs();
                let trunc = abs.trunc();
                let frac = abs - trunc;
                if (trunc as u64) % 2 == 1 {
                    1.0 - frac
                } else {
                    frac
                }
            }
            Self::ClampToEdge => coord.clamp(0.0, 1.0),
        }
    }
}

/// Filter mode for texture sampling, as defined by glTF 2.0 / OpenGL ES.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FilterMode {
    /// Nearest-neighbor filtering. (glTF code: 9728)
    Nearest,
    /// Bilinear filtering. (glTF code: 9729)
    #[default]
    Linear,
}

impl FilterMode {
    /// Construct from a glTF integer filter code. Returns `None` if unknown.
    pub fn from_gltf(code: u32) -> Option<Self> {
        match code {
            9728 => Some(Self::Nearest),
            9729 => Some(Self::Linear),
            _ => None,
        }
    }

    /// The glTF integer filter code.
    pub const fn to_gltf(self) -> u32 {
        match self {
            Self::Nearest => 9728,
            Self::Linear => 9729,
        }
    }
}

impl crate::Sampler {
    /// Typed wrap mode for the S (U) axis.
    pub fn wrap_s_mode(&self) -> WrapMode {
        WrapMode::from_gltf(self.wrap_s)
    }

    /// Typed wrap mode for the T (V) axis.
    pub fn wrap_t_mode(&self) -> WrapMode {
        WrapMode::from_gltf(self.wrap_t)
    }

    /// Typed magnification filter, or `None` if unspecified.
    pub fn mag_filter_mode(&self) -> Option<FilterMode> {
        self.mag_filter.and_then(FilterMode::from_gltf)
    }

    /// Typed minification filter, or `None` if unspecified.
    pub fn min_filter_mode(&self) -> Option<FilterMode> {
        self.min_filter.and_then(FilterMode::from_gltf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeat_wraps_positive() {
        assert!((WrapMode::Repeat.apply(1.7) - 0.7).abs() < 1e-10);
    }

    #[test]
    fn repeat_wraps_negative() {
        assert!((WrapMode::Repeat.apply(-0.3) - 0.7).abs() < 1e-10);
    }

    #[test]
    fn mirrored_repeat_odd() {
        assert!((WrapMode::MirroredRepeat.apply(1.3) - 0.7).abs() < 1e-10);
    }

    #[test]
    fn mirrored_repeat_even() {
        assert!((WrapMode::MirroredRepeat.apply(2.3) - 0.3).abs() < 1e-10);
    }

    #[test]
    fn clamp_to_edge() {
        assert!((WrapMode::ClampToEdge.apply(1.5) - 1.0).abs() < 1e-10);
        assert!((WrapMode::ClampToEdge.apply(-0.5) - 0.0).abs() < 1e-10);
        assert!((WrapMode::ClampToEdge.apply(0.5) - 0.5).abs() < 1e-10);
    }

    #[test]
    fn roundtrip_wrap_mode() {
        for m in [
            WrapMode::ClampToEdge,
            WrapMode::MirroredRepeat,
            WrapMode::Repeat,
        ] {
            assert_eq!(WrapMode::from_gltf(m.to_gltf()), m);
        }
    }

    #[test]
    fn roundtrip_filter_mode() {
        for m in [FilterMode::Nearest, FilterMode::Linear] {
            assert_eq!(FilterMode::from_gltf(m.to_gltf()), Some(m));
        }
    }

    #[test]
    fn unknown_wrap_falls_back() {
        assert_eq!(WrapMode::from_gltf(9999), WrapMode::ClampToEdge);
    }

    #[test]
    fn unknown_filter_is_none() {
        assert_eq!(FilterMode::from_gltf(9999), None);
    }
}
