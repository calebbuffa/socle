//! Coordinate system transforms and matrix construction helpers.

use glam::{DMat3, DMat4, DQuat, DVec3, DVec4};

/// Up-axis enumeration for coordinate system transforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    X,
    Y,
    Z,
}

/// Axis transform constants and matrix construction helpers.
pub struct Transforms;

/// Y-up to Z-up: rotate +π/2 around X axis.
pub const Y_UP_TO_Z_UP: DMat4 = DMat4::from_cols(
    DVec4::new(1.0, 0.0, 0.0, 0.0),
    DVec4::new(0.0, 0.0, 1.0, 0.0),
    DVec4::new(0.0, -1.0, 0.0, 0.0),
    DVec4::new(0.0, 0.0, 0.0, 1.0),
);

/// Z-up to Y-up: rotate -π/2 around X axis.
pub const Z_UP_TO_Y_UP: DMat4 = DMat4::from_cols(
    DVec4::new(1.0, 0.0, 0.0, 0.0),
    DVec4::new(0.0, 0.0, -1.0, 0.0),
    DVec4::new(0.0, 1.0, 0.0, 0.0),
    DVec4::new(0.0, 0.0, 0.0, 1.0),
);

/// X-up to Z-up: rotate -π/2 around Y axis.
pub const X_UP_TO_Z_UP: DMat4 = DMat4::from_cols(
    DVec4::new(0.0, 0.0, 1.0, 0.0),
    DVec4::new(0.0, 1.0, 0.0, 0.0),
    DVec4::new(-1.0, 0.0, 0.0, 0.0),
    DVec4::new(0.0, 0.0, 0.0, 1.0),
);

/// Z-up to X-up: rotate +π/2 around Y axis.
pub const Z_UP_TO_X_UP: DMat4 = DMat4::from_cols(
    DVec4::new(0.0, 0.0, -1.0, 0.0),
    DVec4::new(0.0, 1.0, 0.0, 0.0),
    DVec4::new(1.0, 0.0, 0.0, 0.0),
    DVec4::new(0.0, 0.0, 0.0, 1.0),
);

/// X-up to Y-up: rotate +π/2 around Z axis.
pub const X_UP_TO_Y_UP: DMat4 = DMat4::from_cols(
    DVec4::new(0.0, 1.0, 0.0, 0.0),
    DVec4::new(-1.0, 0.0, 0.0, 0.0),
    DVec4::new(0.0, 0.0, 1.0, 0.0),
    DVec4::new(0.0, 0.0, 0.0, 1.0),
);

/// Y-up to X-up: rotate -π/2 around Z axis.
pub const Y_UP_TO_X_UP: DMat4 = DMat4::from_cols(
    DVec4::new(0.0, -1.0, 0.0, 0.0),
    DVec4::new(1.0, 0.0, 0.0, 0.0),
    DVec4::new(0.0, 0.0, 1.0, 0.0),
    DVec4::new(0.0, 0.0, 0.0, 1.0),
);

impl Transforms {
    /// Get the transform matrix converting from one up-axis to another.
    pub fn get_up_axis_transform(from: Axis, to: Axis) -> &'static DMat4 {
        match (from, to) {
            (Axis::X, Axis::X) | (Axis::Y, Axis::Y) | (Axis::Z, Axis::Z) => &DMat4::IDENTITY,
            (Axis::Y, Axis::Z) => &Y_UP_TO_Z_UP,
            (Axis::Z, Axis::Y) => &Z_UP_TO_Y_UP,
            (Axis::X, Axis::Z) => &X_UP_TO_Z_UP,
            (Axis::Z, Axis::X) => &Z_UP_TO_X_UP,
            (Axis::X, Axis::Y) => &X_UP_TO_Y_UP,
            (Axis::Y, Axis::X) => &Y_UP_TO_X_UP,
        }
    }

    /// Create a translation-rotation-scale matrix.
    ///
    /// Equivalent to `translation * rotation * scale`, so a vector multiplied
    /// by this matrix is first scaled, then rotated, then translated.
    pub fn create_translation_rotation_scale(
        translation: DVec3,
        rotation: DQuat,
        scale: DVec3,
    ) -> DMat4 {
        let rot_scale = DMat3::from_quat(rotation)
            * DMat3::from_cols(
                DVec3::new(scale.x, 0.0, 0.0),
                DVec3::new(0.0, scale.y, 0.0),
                DVec3::new(0.0, 0.0, scale.z),
            );
        DMat4::from_cols(
            rot_scale.x_axis.extend(0.0),
            rot_scale.y_axis.extend(0.0),
            rot_scale.z_axis.extend(0.0),
            translation.extend(1.0),
        )
    }

    /// Decompose a matrix into translation, rotation, and scale.
    ///
    /// The scale may be negative (e.g. when switching handedness).
    /// Skew or other non-affine components produce undefined results.
    pub fn decompose_translation_rotation_scale(matrix: &DMat4) -> (DVec3, DQuat, DVec3) {
        let translation = matrix.w_axis.truncate();

        let rot_scale = DMat3::from_cols(
            matrix.x_axis.truncate(),
            matrix.y_axis.truncate(),
            matrix.z_axis.truncate(),
        );

        let sx = rot_scale.x_axis.length();
        let sy = rot_scale.y_axis.length();
        let sz = rot_scale.z_axis.length();

        let mut rotation_matrix = DMat3::from_cols(
            rot_scale.x_axis / sx,
            rot_scale.y_axis / sy,
            rot_scale.z_axis / sz,
        );

        let mut scale = DVec3::new(sx, sy, sz);

        // Check for reflection (negative determinant)
        let cross = rot_scale.x_axis.cross(rot_scale.y_axis);
        if cross.dot(rot_scale.z_axis) < 0.0 {
            rotation_matrix = DMat3::from_cols(
                -rotation_matrix.x_axis,
                -rotation_matrix.y_axis,
                -rotation_matrix.z_axis,
            );
            scale = -scale;
        }

        let rotation = DQuat::from_mat3(&rotation_matrix);
        (translation, rotation, scale)
    }

    /// Create a view matrix from camera pose.
    ///
    /// Similar to `glm::lookAt` but uses the camera's pose (position,
    /// direction, up) and inverts it to create the view matrix.
    pub fn create_view_matrix(position: DVec3, direction: DVec3, up: DVec3) -> DMat4 {
        let forward = -direction;
        let side = up.cross(forward).normalize();
        let pose_up = forward.cross(side).normalize();

        DMat4::from_cols(
            DVec4::new(side.x, pose_up.x, forward.x, 0.0),
            DVec4::new(side.y, pose_up.y, forward.y, 0.0),
            DVec4::new(side.z, pose_up.z, forward.z, 0.0),
            DVec4::new(
                -side.dot(position),
                -pose_up.dot(position),
                -forward.dot(position),
                1.0,
            ),
        )
    }

    /// Create a Vulkan-style perspective projection matrix with reverse-Z.
    ///
    /// Uses symmetric FOV angles. Pass `f64::INFINITY` for `z_far` to
    /// get an infinite far plane.
    pub fn create_perspective_fov(fov_x: f64, fov_y: f64, z_near: f64, z_far: f64) -> DMat4 {
        let (m22, m32) = if z_far.is_infinite() {
            (0.0, z_near)
        } else {
            (z_near / (z_far - z_near), z_near * z_far / (z_far - z_near))
        };

        DMat4::from_cols(
            DVec4::new(1.0 / (fov_x * 0.5).tan(), 0.0, 0.0, 0.0),
            DVec4::new(0.0, -1.0 / (fov_y * 0.5).tan(), 0.0, 0.0),
            DVec4::new(0.0, 0.0, m22, -1.0),
            DVec4::new(0.0, 0.0, m32, 0.0),
        )
    }

    /// Create a Vulkan-style perspective projection matrix with reverse-Z
    /// from asymmetric frustum bounds.
    ///
    /// Pass `f64::INFINITY` for `z_far` to get an infinite far plane.
    pub fn create_perspective_offcenter(
        left: f64,
        right: f64,
        bottom: f64,
        top: f64,
        z_near: f64,
        z_far: f64,
    ) -> DMat4 {
        let (m22, m32) = if z_far.is_infinite() {
            (0.0, z_near)
        } else {
            (z_near / (z_far - z_near), z_near * z_far / (z_far - z_near))
        };

        DMat4::from_cols(
            DVec4::new(2.0 * z_near / (right - left), 0.0, 0.0, 0.0),
            DVec4::new(0.0, 2.0 * z_near / (bottom - top), 0.0, 0.0),
            DVec4::new(
                (right + left) / (right - left),
                (bottom + top) / (bottom - top),
                m22,
                -1.0,
            ),
            DVec4::new(0.0, 0.0, m32, 0.0),
        )
    }

    /// Create a Vulkan-style orthographic projection matrix with reverse-Z.
    ///
    /// Pass `f64::INFINITY` for `z_far` to get an infinite far plane.
    pub fn create_orthographic(
        left: f64,
        right: f64,
        bottom: f64,
        top: f64,
        z_near: f64,
        z_far: f64,
    ) -> DMat4 {
        let (m22, m32) = if z_far.is_infinite() {
            (0.0, 1.0)
        } else {
            (1.0 / (z_far - z_near), z_far / (z_far - z_near))
        };

        DMat4::from_cols(
            DVec4::new(2.0 / (right - left), 0.0, 0.0, 0.0),
            DVec4::new(0.0, 2.0 / (bottom - top), 0.0, 0.0),
            DVec4::new(0.0, 0.0, m22, 0.0),
            DVec4::new(
                -(right + left) / (right - left),
                -(bottom + top) / (bottom - top),
                m32,
                1.0,
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    #[test]
    fn y_up_to_z_up_roundtrip() {
        let m = Y_UP_TO_Z_UP * Z_UP_TO_Y_UP;
        for i in 0..4 {
            for j in 0..4 {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!((m.col(i)[j] - expected).abs() < 1e-12, "element [{i}][{j}]");
            }
        }
    }

    #[test]
    fn x_up_to_z_up_roundtrip() {
        let m = X_UP_TO_Z_UP * Z_UP_TO_X_UP;
        for i in 0..4 {
            for j in 0..4 {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!((m.col(i)[j] - expected).abs() < 1e-12, "element [{i}][{j}]");
            }
        }
    }

    #[test]
    fn get_up_axis_identity() {
        assert_eq!(
            *Transforms::get_up_axis_transform(Axis::X, Axis::X),
            DMat4::IDENTITY
        );
        assert_eq!(
            *Transforms::get_up_axis_transform(Axis::Y, Axis::Y),
            DMat4::IDENTITY
        );
        assert_eq!(
            *Transforms::get_up_axis_transform(Axis::Z, Axis::Z),
            DMat4::IDENTITY
        );
    }

    #[test]
    fn trs_roundtrip() {
        let t = DVec3::new(1.0, 2.0, 3.0);
        let r = DQuat::from_rotation_y(FRAC_PI_2);
        let s = DVec3::new(2.0, 3.0, 4.0);

        let m = Transforms::create_translation_rotation_scale(t, r, s);
        let (t2, r2, s2) = Transforms::decompose_translation_rotation_scale(&m);

        assert!((t - t2).length() < 1e-10, "translation mismatch");
        assert!(
            (r - r2).length() < 1e-10 || (r + r2).length() < 1e-10,
            "rotation mismatch"
        );
        assert!((s - s2).length() < 1e-10, "scale mismatch");
    }

    #[test]
    fn view_matrix_origin() {
        let pos = DVec3::ZERO;
        let dir = DVec3::NEG_Z;
        let up = DVec3::Y;
        let view = Transforms::create_view_matrix(pos, dir, up);
        // At origin looking -Z, the view matrix should be identity
        for i in 0..4 {
            for j in 0..4 {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!(
                    (view.col(i)[j] - expected).abs() < 1e-10,
                    "element [{i}][{j}]: {} != {}",
                    view.col(i)[j],
                    expected
                );
            }
        }
    }

    #[test]
    fn perspective_fov_basic() {
        let m = Transforms::create_perspective_fov(FRAC_PI_2, FRAC_PI_2, 0.1, 100.0);
        // Should be a valid projection matrix (non-zero diagonal)
        assert!(m.x_axis.x.abs() > 0.0);
        assert!(m.y_axis.y.abs() > 0.0);
        // Reverse-Z: z_axis.w should be -1
        assert!((m.z_axis.w - (-1.0)).abs() < 1e-12);
    }

    #[test]
    fn orthographic_basic() {
        let m = Transforms::create_orthographic(-1.0, 1.0, -1.0, 1.0, 0.1, 100.0);
        // Should be a valid ortho matrix
        assert!((m.x_axis.x - 1.0).abs() < 1e-12); // 2/(1-(-1)) = 1
        // Reverse-Z: w_axis.w should be 1
        assert!((m.w_axis.w - 1.0).abs() < 1e-12);
    }
}
