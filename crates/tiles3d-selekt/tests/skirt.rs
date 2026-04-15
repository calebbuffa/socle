use tiles3d_selekt::SkirtMeshMetadata;

#[test]
fn skirt_mesh_metadata_roundtrip() {
    let meta = SkirtMeshMetadata {
        no_skirt_indices_begin: 0,
        no_skirt_indices_count: 360,
        no_skirt_vertices_begin: 0,
        no_skirt_vertices_count: 289,
        mesh_center: [1234567.8, -9876543.2, 45678.9],
        skirt_west_height: 100.5,
        skirt_south_height: 200.0,
        skirt_east_height: 150.75,
        skirt_north_height: 175.25,
    };

    let extras = meta.to_extras();

    let json = serde_json::to_string(&extras).expect("serialize extras");
    let parsed_value: serde_json::Value = serde_json::from_str(&json).expect("deserialize extras");
    let restored = SkirtMeshMetadata::parse_from_extras(&parsed_value).expect("parse_from_extras");

    assert_eq!(restored.no_skirt_indices_begin, meta.no_skirt_indices_begin);
    assert_eq!(restored.no_skirt_indices_count, meta.no_skirt_indices_count);
    assert_eq!(restored.no_skirt_vertices_begin, meta.no_skirt_vertices_begin);
    assert_eq!(restored.no_skirt_vertices_count, meta.no_skirt_vertices_count);
    assert_eq!(restored.mesh_center, meta.mesh_center);
    assert_eq!(restored.skirt_west_height, meta.skirt_west_height);
    assert_eq!(restored.skirt_south_height, meta.skirt_south_height);
    assert_eq!(restored.skirt_east_height, meta.skirt_east_height);
    assert_eq!(restored.skirt_north_height, meta.skirt_north_height);
}

#[test]
fn skirt_mesh_metadata_missing_key_returns_none() {
    let incomplete = serde_json::json!({
        "skirtMeshMetadata": {
            "noSkirtRange": [0, 100, 0, 80]
        }
    });
    assert!(
        SkirtMeshMetadata::parse_from_extras(&incomplete).is_none(),
        "parse_from_extras should return None for incomplete data"
    );
}

#[test]
fn skirt_mesh_metadata_no_key_returns_none() {
    let empty = serde_json::json!({});
    assert!(SkirtMeshMetadata::parse_from_extras(&empty).is_none());
}
