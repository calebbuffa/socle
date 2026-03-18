//! JSON deserialization for I3S resources.
//!
//! Thin wrappers around `serde_json::from_slice` — the `i3s` crate types
//! already derive `Deserialize`, so this module provides convenience functions
//! with proper error mapping.

use i3s_util::{I3SError, Result};
use serde::de::DeserializeOwned;

/// Deserialize any I3S JSON resource from raw bytes.
///
/// This is the core deserialization function — works for `SceneLayer`,
/// `NodePage`, `Stats`, or any type from the `i3s` crate.
///
/// # Errors
///
/// Returns [`I3SError::Json`] if the bytes are not valid JSON or do not
/// match the expected type structure.
pub fn read_json<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    serde_json::from_slice(bytes).map_err(I3SError::from)
}

/// Deserialize from a JSON string.
pub fn read_json_str<T: DeserializeOwned>(s: &str) -> Result<T> {
    serde_json::from_str(s).map_err(I3SError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use i3s::cmn::NodePage;
    use i3s::cmn::Obb;

    #[test]
    fn read_obb() {
        let json = r#"{
            "center": [1.0, 2.0, 3.0],
            "halfSize": [10.0, 20.0, 30.0],
            "quaternion": [0.0, 0.0, 0.0, 1.0]
        }"#;
        let obb: Obb = read_json_str(json).unwrap();
        assert_eq!(obb.center, [1.0, 2.0, 3.0]);
        assert_eq!(obb.half_size, [10.0, 20.0, 30.0]);
        assert_eq!(obb.quaternion, [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn read_node_page() {
        let json = r#"{
            "nodes": [
                {
                    "index": 0,
                    "obb": {
                        "center": [1.0, 2.0, 3.0],
                        "halfSize": [10.0, 10.0, 10.0],
                        "quaternion": [0.0, 0.0, 0.0, 1.0]
                    },
                    "lodThreshold": 500.0,
                    "children": [1, 2, 3]
                },
                {
                    "index": 1,
                    "parentIndex": 0,
                    "obb": {
                        "center": [4.0, 5.0, 6.0],
                        "halfSize": [5.0, 5.0, 5.0],
                        "quaternion": [0.0, 0.0, 0.0, 1.0]
                    },
                    "lodThreshold": 100.0
                }
            ]
        }"#;
        let page: NodePage = read_json_str(json).unwrap();
        assert_eq!(page.nodes.len(), 2);
        assert_eq!(page.nodes[0].index, 0);
        assert_eq!(page.nodes[0].children, Some(vec![1, 2, 3]));
        assert_eq!(page.nodes[1].parent_index, Some(0));
    }

    #[test]
    fn read_json_error() {
        let result = read_json::<Obb>(b"not valid json");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, I3SError::Json(_)));
    }
}
