//! Optional `glam` integration for `zukei` FFI types.
//!
//! Enable the crate feature `glam` to compile these conversion impls and use
//! this module as the Rust-side bridge to `glam` math types.

use crate::math::{Mat3, Mat4, Vec2, Vec3, Vec4};

pub use glam_dep::{DMat3, DMat4, DVec2, DVec3, DVec4};

impl From<DVec2> for Vec2 {
    fn from(value: DVec2) -> Self {
        Self {
            x: value.x,
            y: value.y,
        }
    }
}

impl From<Vec2> for DVec2 {
    fn from(value: Vec2) -> Self {
        Self::new(value.x, value.y)
    }
}

impl From<&Vec2> for DVec2 {
    fn from(value: &Vec2) -> Self {
        Self::new(value.x, value.y)
    }
}

impl From<DVec3> for Vec3 {
    fn from(value: DVec3) -> Self {
        Self {
            x: value.x,
            y: value.y,
            z: value.z,
        }
    }
}

impl From<Vec3> for DVec3 {
    fn from(value: Vec3) -> Self {
        Self::new(value.x, value.y, value.z)
    }
}

impl From<&Vec3> for DVec3 {
    fn from(value: &Vec3) -> Self {
        Self::new(value.x, value.y, value.z)
    }
}

impl From<DVec4> for Vec4 {
    fn from(value: DVec4) -> Self {
        Self {
            x: value.x,
            y: value.y,
            z: value.z,
            w: value.w,
        }
    }
}

impl From<Vec4> for DVec4 {
    fn from(value: Vec4) -> Self {
        Self::new(value.x, value.y, value.z, value.w)
    }
}

impl From<&Vec4> for DVec4 {
    fn from(value: &Vec4) -> Self {
        Self::new(value.x, value.y, value.z, value.w)
    }
}

impl From<DMat3> for Mat3 {
    fn from(value: DMat3) -> Self {
        Self {
            cols: [
                value.x_axis.into(),
                value.y_axis.into(),
                value.z_axis.into(),
            ],
        }
    }
}

impl From<Mat3> for DMat3 {
    fn from(value: Mat3) -> Self {
        Self::from_cols(
            value.cols[0].into(),
            value.cols[1].into(),
            value.cols[2].into(),
        )
    }
}

impl From<&Mat3> for DMat3 {
    fn from(value: &Mat3) -> Self {
        Self::from_cols(
            value.cols[0].into(),
            value.cols[1].into(),
            value.cols[2].into(),
        )
    }
}

impl From<DMat4> for Mat4 {
    fn from(value: DMat4) -> Self {
        Self {
            cols: [
                value.x_axis.into(),
                value.y_axis.into(),
                value.z_axis.into(),
                value.w_axis.into(),
            ],
        }
    }
}

impl From<Mat4> for DMat4 {
    fn from(value: Mat4) -> Self {
        Self::from_cols(
            value.cols[0].into(),
            value.cols[1].into(),
            value.cols[2].into(),
            value.cols[3].into(),
        )
    }
}

impl From<&Mat4> for DMat4 {
    fn from(value: &Mat4) -> Self {
        Self::from_cols(
            value.cols[0].into(),
            value.cols[1].into(),
            value.cols[2].into(),
            value.cols[3].into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec_roundtrip_owned_and_borrowed() {
        let v2 = Vec2 { x: 1.0, y: -2.0 };
        let v3 = Vec3 {
            x: 1.0,
            y: -2.0,
            z: 3.0,
        };
        let v4 = Vec4 {
            x: 1.0,
            y: -2.0,
            z: 3.0,
            w: -4.0,
        };

        let d2_owned: DVec2 = v2.into();
        let d3_owned: DVec3 = v3.into();
        let d4_owned: DVec4 = v4.into();

        let d2_borrowed: DVec2 = (&v2).into();
        let d3_borrowed: DVec3 = (&v3).into();
        let d4_borrowed: DVec4 = (&v4).into();

        assert_eq!(d2_owned, d2_borrowed);
        assert_eq!(d3_owned, d3_borrowed);
        assert_eq!(d4_owned, d4_borrowed);

        let back2: Vec2 = d2_owned.into();
        let back3: Vec3 = d3_owned.into();
        let back4: Vec4 = d4_owned.into();

        assert_eq!(back2, v2);
        assert_eq!(back3, v3);
        assert_eq!(back4, v4);
    }

    #[test]
    fn mat_roundtrip_owned_and_borrowed() {
        let m3 = Mat3 {
            cols: [
                Vec3 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                },
                Vec3 {
                    x: 0.0,
                    y: 2.0,
                    z: 0.0,
                },
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 3.0,
                },
            ],
        };

        let m4 = Mat4 {
            cols: [
                Vec4 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                    w: 0.0,
                },
                Vec4 {
                    x: 0.0,
                    y: 2.0,
                    z: 0.0,
                    w: 0.0,
                },
                Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 3.0,
                    w: 0.0,
                },
                Vec4 {
                    x: 4.0,
                    y: 5.0,
                    z: 6.0,
                    w: 1.0,
                },
            ],
        };

        let d3_owned: DMat3 = m3.into();
        let d4_owned: DMat4 = m4.into();

        let d3_borrowed: DMat3 = (&m3).into();
        let d4_borrowed: DMat4 = (&m4).into();

        assert_eq!(d3_owned, d3_borrowed);
        assert_eq!(d4_owned, d4_borrowed);

        let back3: Mat3 = d3_owned.into();
        let back4: Mat4 = d4_owned.into();

        assert_eq!(back3, m3);
        assert_eq!(back4, m4);
    }
}
