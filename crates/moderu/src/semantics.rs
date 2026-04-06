/// Standard vertex attribute semantics defined by the glTF 2.0 specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VertexAttribute {
    /// Unitless XYZ vertex positions.
    Position,
    /// Normalized XYZ vertex normals.
    Normal,
    /// XYZW vertex tangents where XYZ is normalized and W is a sign value (-1 or +1)
    /// indicating handedness of the tangent basis.
    Tangent,
    /// ST texture coordinates (0-7).
    TexCoord(u8),
    /// RGB or RGBA vertex color linear multiplier (0-7).
    Color(u8),
    /// The indices of the joints from the corresponding `skin.joints` array (0-7).
    Joints(u8),
    /// The weights indicating how strongly the joint influences the vertex (0-7).
    Weights(u8),
    /// Feature IDs used in `EXT_mesh_features` (0-7).
    FeatureId(u8),
}

impl VertexAttribute {
    /// Converts enum to glTF semantic name string.
    ///
    /// Returns `"UNKNOWN"` for indexed variants with an index ≥ 8 (which are unreachable
    /// through the public API but can be constructed via the public inner `u8` field).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Position => "POSITION",
            Self::Normal => "NORMAL",
            Self::Tangent => "TANGENT",
            Self::TexCoord(i) => {
                const TEXCOORD: [&str; 8] = [
                    "TEXCOORD_0",
                    "TEXCOORD_1",
                    "TEXCOORD_2",
                    "TEXCOORD_3",
                    "TEXCOORD_4",
                    "TEXCOORD_5",
                    "TEXCOORD_6",
                    "TEXCOORD_7",
                ];
                TEXCOORD.get(*i as usize).copied().unwrap_or("UNKNOWN")
            }
            Self::Color(i) => {
                const COLOR: [&str; 8] = [
                    "COLOR_0", "COLOR_1", "COLOR_2", "COLOR_3", "COLOR_4", "COLOR_5", "COLOR_6",
                    "COLOR_7",
                ];
                COLOR.get(*i as usize).copied().unwrap_or("UNKNOWN")
            }
            Self::Joints(i) => {
                const JOINTS: [&str; 8] = [
                    "JOINTS_0", "JOINTS_1", "JOINTS_2", "JOINTS_3", "JOINTS_4", "JOINTS_5",
                    "JOINTS_6", "JOINTS_7",
                ];
                JOINTS.get(*i as usize).copied().unwrap_or("UNKNOWN")
            }
            Self::Weights(i) => {
                const WEIGHTS: [&str; 8] = [
                    "WEIGHTS_0",
                    "WEIGHTS_1",
                    "WEIGHTS_2",
                    "WEIGHTS_3",
                    "WEIGHTS_4",
                    "WEIGHTS_5",
                    "WEIGHTS_6",
                    "WEIGHTS_7",
                ];
                WEIGHTS.get(*i as usize).copied().unwrap_or("UNKNOWN")
            }
            Self::FeatureId(i) => {
                const FEATURE_ID: [&str; 8] = [
                    "_FEATURE_ID_0",
                    "_FEATURE_ID_1",
                    "_FEATURE_ID_2",
                    "_FEATURE_ID_3",
                    "_FEATURE_ID_4",
                    "_FEATURE_ID_5",
                    "_FEATURE_ID_6",
                    "_FEATURE_ID_7",
                ];
                FEATURE_ID.get(*i as usize).copied().unwrap_or("UNKNOWN")
            }
        }
    }

    /// Parses a glTF semantic name string to enum.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "POSITION" => Some(Self::Position),
            "NORMAL" => Some(Self::Normal),
            "TANGENT" => Some(Self::Tangent),
            s if s.starts_with("TEXCOORD_") => s
                .strip_prefix("TEXCOORD_")
                .and_then(|i| i.parse::<u8>().ok())
                .and_then(|i| (i < 8).then_some(Self::TexCoord(i))),
            s if s.starts_with("COLOR_") => s
                .strip_prefix("COLOR_")
                .and_then(|i| i.parse::<u8>().ok())
                .and_then(|i| (i < 8).then_some(Self::Color(i))),
            s if s.starts_with("JOINTS_") => s
                .strip_prefix("JOINTS_")
                .and_then(|i| i.parse::<u8>().ok())
                .and_then(|i| (i < 8).then_some(Self::Joints(i))),
            s if s.starts_with("WEIGHTS_") => s
                .strip_prefix("WEIGHTS_")
                .and_then(|i| i.parse::<u8>().ok())
                .and_then(|i| (i < 8).then_some(Self::Weights(i))),
            s if s.starts_with("_FEATURE_ID_") => s
                .strip_prefix("_FEATURE_ID_")
                .and_then(|i| i.parse::<u8>().ok())
                .and_then(|i| (i < 8).then_some(Self::FeatureId(i))),
            _ => None,
        }
    }
}

impl std::fmt::Display for VertexAttribute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for VertexAttribute {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<VertexAttribute> for String {
    fn from(a: VertexAttribute) -> Self {
        a.as_str().to_owned()
    }
}

impl std::str::FromStr for VertexAttribute {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        VertexAttribute::from_str(s).ok_or(())
    }
}

/// Standard instance attribute semantics for `EXT_mesh_gpu_instancing`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InstanceAttribute {
    /// XYZ translation vector.
    Translation,
    /// XYZW rotation quaternion.
    Rotation,
    /// XYZ scale vector.
    Scale,
    /// Feature IDs used in `EXT_mesh_features` (0-7).
    FeatureId(u8),
}

impl InstanceAttribute {
    /// Converts enum to glTF semantic name string.
    ///
    /// Returns `"UNKNOWN"` for `FeatureId` variants with an index ≥ 8.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Translation => "TRANSLATION",
            Self::Rotation => "ROTATION",
            Self::Scale => "SCALE",
            Self::FeatureId(i) => {
                const FEATURE_ID: [&str; 8] = [
                    "_FEATURE_ID_0",
                    "_FEATURE_ID_1",
                    "_FEATURE_ID_2",
                    "_FEATURE_ID_3",
                    "_FEATURE_ID_4",
                    "_FEATURE_ID_5",
                    "_FEATURE_ID_6",
                    "_FEATURE_ID_7",
                ];
                FEATURE_ID.get(*i as usize).copied().unwrap_or("UNKNOWN")
            }
        }
    }

    /// Returns an iterator over all valid instance attributes (Translation, Rotation, Scale,
    /// and FeatureId 0–7).
    pub fn all() -> impl Iterator<Item = InstanceAttribute> {
        [
            InstanceAttribute::Translation,
            InstanceAttribute::Rotation,
            InstanceAttribute::Scale,
        ]
        .into_iter()
        .chain((0..8).map(InstanceAttribute::FeatureId))
    }

    /// Parses a glTF semantic name string to enum.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "TRANSLATION" => Some(Self::Translation),
            "ROTATION" => Some(Self::Rotation),
            "SCALE" => Some(Self::Scale),
            s if s.starts_with("_FEATURE_ID_") => s
                .strip_prefix("_FEATURE_ID_")
                .and_then(|i| i.parse::<u8>().ok())
                .and_then(|i| (i < 8).then_some(Self::FeatureId(i))),
            _ => None,
        }
    }
}

impl std::fmt::Display for InstanceAttribute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for InstanceAttribute {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<InstanceAttribute> for String {
    fn from(a: InstanceAttribute) -> Self {
        a.as_str().to_owned()
    }
}

impl std::str::FromStr for InstanceAttribute {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        InstanceAttribute::from_str(s).ok_or(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_basic_types_to_string() {
        assert_eq!(VertexAttribute::Position.as_str(), "POSITION");
        assert_eq!(VertexAttribute::Normal.as_str(), "NORMAL");
        assert_eq!(VertexAttribute::Tangent.as_str(), "TANGENT");
    }

    #[test]
    fn vertex_indexed_types_to_string() {
        assert_eq!(VertexAttribute::TexCoord(0).as_str(), "TEXCOORD_0");
        assert_eq!(VertexAttribute::TexCoord(7).as_str(), "TEXCOORD_7");
        assert_eq!(VertexAttribute::Color(3).as_str(), "COLOR_3");
        assert_eq!(VertexAttribute::Joints(1).as_str(), "JOINTS_1");
        assert_eq!(VertexAttribute::Weights(5).as_str(), "WEIGHTS_5");
        assert_eq!(VertexAttribute::FeatureId(2).as_str(), "_FEATURE_ID_2");
    }

    #[test]
    fn vertex_from_string_basic() {
        assert_eq!(
            VertexAttribute::from_str("POSITION"),
            Some(VertexAttribute::Position)
        );
        assert_eq!(
            VertexAttribute::from_str("NORMAL"),
            Some(VertexAttribute::Normal)
        );
        assert_eq!(
            VertexAttribute::from_str("TANGENT"),
            Some(VertexAttribute::Tangent)
        );
    }

    #[test]
    fn vertex_from_string_indexed() {
        assert_eq!(
            VertexAttribute::from_str("TEXCOORD_0"),
            Some(VertexAttribute::TexCoord(0))
        );
        assert_eq!(
            VertexAttribute::from_str("TEXCOORD_7"),
            Some(VertexAttribute::TexCoord(7))
        );
        assert_eq!(
            VertexAttribute::from_str("COLOR_3"),
            Some(VertexAttribute::Color(3))
        );
        assert_eq!(
            VertexAttribute::from_str("JOINTS_1"),
            Some(VertexAttribute::Joints(1))
        );
        assert_eq!(
            VertexAttribute::from_str("WEIGHTS_5"),
            Some(VertexAttribute::Weights(5))
        );
        assert_eq!(
            VertexAttribute::from_str("_FEATURE_ID_2"),
            Some(VertexAttribute::FeatureId(2))
        );
    }

    #[test]
    fn vertex_from_string_invalid() {
        assert!(VertexAttribute::from_str("INVALID").is_none());
        assert!(VertexAttribute::from_str("TEXCOORD_8").is_none()); // Out of range
        assert!(VertexAttribute::from_str("COLOR_9").is_none()); // Out of range
        assert!(VertexAttribute::from_str("TEXCOORD").is_none()); // Missing index
        assert!(VertexAttribute::from_str("").is_none());
    }

    #[test]
    fn vertex_roundtrip_conversion() {
        let attrs = [
            VertexAttribute::Position,
            VertexAttribute::Normal,
            VertexAttribute::Tangent,
            VertexAttribute::TexCoord(0),
            VertexAttribute::TexCoord(7),
            VertexAttribute::Color(4),
            VertexAttribute::Joints(2),
            VertexAttribute::Weights(6),
            VertexAttribute::FeatureId(3),
        ];

        for attr in attrs {
            let s = attr.as_str();
            let parsed = VertexAttribute::from_str(s).expect("Failed to parse");
            assert_eq!(attr, parsed, "Roundtrip failed for {:?}", attr);
        }
    }

    #[test]
    fn vertex_display_trait() {
        assert_eq!(format!("{}", VertexAttribute::Position), "POSITION");
        assert_eq!(format!("{}", VertexAttribute::TexCoord(3)), "TEXCOORD_3");
        assert_eq!(
            format!("{}", VertexAttribute::FeatureId(0)),
            "_FEATURE_ID_0"
        );
    }

    #[test]
    fn vertex_as_ref_trait() {
        let attr = VertexAttribute::Normal;
        let s: &str = attr.as_ref();
        assert_eq!(s, "NORMAL");
    }

    #[test]
    fn vertex_equality_and_hash() {
        let attr1 = VertexAttribute::TexCoord(2);
        let attr2 = VertexAttribute::TexCoord(2);
        let attr3 = VertexAttribute::TexCoord(3);

        assert_eq!(attr1, attr2);
        assert_ne!(attr1, attr3);

        // Test that it can be used in HashMap
        use std::collections::HashMap;
        let mut map = HashMap::new();
        map.insert(attr1, "value");
        assert_eq!(map.get(&attr2), Some(&"value"));
    }

    #[test]
    fn instance_basic_types_to_string() {
        assert_eq!(InstanceAttribute::Translation.as_str(), "TRANSLATION");
        assert_eq!(InstanceAttribute::Rotation.as_str(), "ROTATION");
        assert_eq!(InstanceAttribute::Scale.as_str(), "SCALE");
    }

    #[test]
    fn instance_indexed_types_to_string() {
        assert_eq!(InstanceAttribute::FeatureId(0).as_str(), "_FEATURE_ID_0");
        assert_eq!(InstanceAttribute::FeatureId(7).as_str(), "_FEATURE_ID_7");
    }

    #[test]
    fn instance_from_string_basic() {
        assert_eq!(
            InstanceAttribute::from_str("TRANSLATION"),
            Some(InstanceAttribute::Translation)
        );
        assert_eq!(
            InstanceAttribute::from_str("ROTATION"),
            Some(InstanceAttribute::Rotation)
        );
        assert_eq!(
            InstanceAttribute::from_str("SCALE"),
            Some(InstanceAttribute::Scale)
        );
    }

    #[test]
    fn instance_from_string_indexed() {
        assert_eq!(
            InstanceAttribute::from_str("_FEATURE_ID_0"),
            Some(InstanceAttribute::FeatureId(0))
        );
        assert_eq!(
            InstanceAttribute::from_str("_FEATURE_ID_7"),
            Some(InstanceAttribute::FeatureId(7))
        );
    }

    #[test]
    fn instance_from_string_invalid() {
        assert!(InstanceAttribute::from_str("INVALID").is_none());
        assert!(InstanceAttribute::from_str("_FEATURE_ID_8").is_none()); // Out of range
        assert!(InstanceAttribute::from_str("TRANSLATION ").is_none()); // Trailing space
    }

    #[test]
    fn instance_roundtrip_conversion() {
        let attrs = [
            InstanceAttribute::Translation,
            InstanceAttribute::Rotation,
            InstanceAttribute::Scale,
            InstanceAttribute::FeatureId(0),
            InstanceAttribute::FeatureId(7),
        ];

        for attr in attrs {
            let s = attr.as_str();
            let parsed = InstanceAttribute::from_str(s).expect("Failed to parse");
            assert_eq!(attr, parsed, "Roundtrip failed for {:?}", attr);
        }
    }

    #[test]
    fn instance_display_trait() {
        assert_eq!(format!("{}", InstanceAttribute::Translation), "TRANSLATION");
        assert_eq!(
            format!("{}", InstanceAttribute::FeatureId(3)),
            "_FEATURE_ID_3"
        );
    }

    #[test]
    fn instance_as_ref_trait() {
        let attr = InstanceAttribute::Rotation;
        let s: &str = attr.as_ref();
        assert_eq!(s, "ROTATION");
    }

    #[test]
    fn instance_all_iterator() {
        let all_attrs: Vec<_> = InstanceAttribute::all().collect();

        // Should have 3 basic + 8 indexed = 11 total
        assert_eq!(all_attrs.len(), 11);

        // Verify some key attributes are present
        assert!(all_attrs.contains(&InstanceAttribute::Translation));
        assert!(all_attrs.contains(&InstanceAttribute::Rotation));
        assert!(all_attrs.contains(&InstanceAttribute::Scale));
        assert!(all_attrs.contains(&InstanceAttribute::FeatureId(7)));
    }

    #[test]
    fn instance_equality_and_hash() {
        let attr1 = InstanceAttribute::FeatureId(2);
        let attr2 = InstanceAttribute::FeatureId(2);
        let attr3 = InstanceAttribute::FeatureId(3);

        assert_eq!(attr1, attr2);
        assert_ne!(attr1, attr3);

        // Test that it can be used in HashMap
        use std::collections::HashMap;
        let mut map = HashMap::new();
        map.insert(attr1, "value");
        assert_eq!(map.get(&attr2), Some(&"value"));
    }

    #[test]
    fn instance_constants_unchanged() {
        assert_eq!(InstanceAttribute::Translation.as_str(), "TRANSLATION");
        assert_eq!(InstanceAttribute::Rotation.as_str(), "ROTATION");
        assert_eq!(InstanceAttribute::Scale.as_str(), "SCALE");
        assert_eq!(InstanceAttribute::FeatureId(0).as_str(), "_FEATURE_ID_0");
    }
}
