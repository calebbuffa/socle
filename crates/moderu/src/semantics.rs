/// Standard vertex attribute semantics defined by the glTF 2.0 specification.
pub mod vertex {
    /// Unitless XYZ vertex positions.
    pub const POSITION: &str = "POSITION";
    /// Normalized XYZ vertex normals.
    pub const NORMAL: &str = "NORMAL";
    /// XYZW vertex tangents where XYZ is normalized and W is a sign value (-1 or +1)
    /// indicating handedness of the tangent basis.
    pub const TANGENT: &str = "TANGENT";

    /// ST texture coordinates.
    pub const TEXCOORD: [&str; 8] = [
        "TEXCOORD_0",
        "TEXCOORD_1",
        "TEXCOORD_2",
        "TEXCOORD_3",
        "TEXCOORD_4",
        "TEXCOORD_5",
        "TEXCOORD_6",
        "TEXCOORD_7",
    ];

    /// RGB or RGBA vertex color linear multiplier.
    pub const COLOR: [&str; 8] = [
        "COLOR_0", "COLOR_1", "COLOR_2", "COLOR_3", "COLOR_4", "COLOR_5", "COLOR_6", "COLOR_7",
    ];

    /// The indices of the joints from the corresponding `skin.joints` array
    /// that affect the vertex.
    pub const JOINTS: [&str; 8] = [
        "JOINTS_0", "JOINTS_1", "JOINTS_2", "JOINTS_3", "JOINTS_4", "JOINTS_5", "JOINTS_6",
        "JOINTS_7",
    ];

    /// The weights indicating how strongly the joint influences the vertex.
    pub const WEIGHTS: [&str; 8] = [
        "WEIGHTS_0",
        "WEIGHTS_1",
        "WEIGHTS_2",
        "WEIGHTS_3",
        "WEIGHTS_4",
        "WEIGHTS_5",
        "WEIGHTS_6",
        "WEIGHTS_7",
    ];

    /// Feature IDs used in `EXT_mesh_features`.
    pub const FEATURE_ID: [&str; 8] = [
        "_FEATURE_ID_0",
        "_FEATURE_ID_1",
        "_FEATURE_ID_2",
        "_FEATURE_ID_3",
        "_FEATURE_ID_4",
        "_FEATURE_ID_5",
        "_FEATURE_ID_6",
        "_FEATURE_ID_7",
    ];
}

/// Standard instance attribute semantics for `EXT_mesh_gpu_instancing`.
pub mod instance {
    /// XYZ translation vector.
    pub const TRANSLATION: &str = "TRANSLATION";
    /// XYZW rotation quaternion.
    pub const ROTATION: &str = "ROTATION";
    /// XYZ scale vector.
    pub const SCALE: &str = "SCALE";

    /// Feature IDs used in `EXT_mesh_features`.
    pub const FEATURE_ID: [&str; 8] = [
        "_FEATURE_ID_0",
        "_FEATURE_ID_1",
        "_FEATURE_ID_2",
        "_FEATURE_ID_3",
        "_FEATURE_ID_4",
        "_FEATURE_ID_5",
        "_FEATURE_ID_6",
        "_FEATURE_ID_7",
    ];
}
