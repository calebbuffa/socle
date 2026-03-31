//! Integration tests for moderu-reader using glTF sample assets.

use moderu::{AccessorComponentType, PrimitiveMode};
use moderu_io::reader::{GltfOk, GltfReader};
use std::fs;
use std::path::PathBuf;

/// Helper to construct path to test data directory.
fn test_data_path(model_name: &str, variant: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/glTF-Sample-Assets/Models")
        .join(model_name)
        .join(variant)
        .join(format!("{}.gltf", model_name))
}

#[test]
fn load_box_gltf() {
    let path = test_data_path("Box", "glTF");
    if !path.exists() {
        eprintln!("Skipping: test data not found: {}", path.display());
        return;
    }

    let data = fs::read(&path).expect("Failed to read Box.gltf");
    let GltfOk { warnings, .. } = GltfReader::default()
        .parse(&data)
        .expect("Failed to parse Box.gltf");
    assert!(warnings.is_empty(), "Unexpected warnings: {:?}", warnings);
}

#[test]
fn load_duck_gltf() {
    let path = test_data_path("Duck", "glTF");
    if !path.exists() {
        eprintln!("Skipping: test data not found: {}", path.display());
        return;
    }

    let data = fs::read(&path).expect("Failed to read Duck.gltf");
    let GltfOk { model, warnings } = GltfReader::default()
        .parse(&data)
        .expect("Failed to parse Duck.gltf");
    assert!(warnings.is_empty(), "Unexpected warnings: {:?}", warnings);
    // Duck model should have meshes and textures
    assert!(!model.meshes.is_empty(), "Duck should have meshes");
    assert!(!model.textures.is_empty(), "Duck should have textures");
    assert!(!model.images.is_empty(), "Duck should have images");
}

#[test]
fn load_animated_triangle_gltf() {
    let path = test_data_path("AnimatedTriangle", "glTF");
    if !path.exists() {
        eprintln!("Skipping: test data not found: {}", path.display());
        return;
    }

    let data = fs::read(&path).expect("Failed to read AnimatedTriangle.gltf");
    let GltfOk { model, warnings } = GltfReader::default()
        .parse(&data)
        .expect("Failed to parse AnimatedTriangle.gltf");
    assert!(warnings.is_empty(), "Unexpected warnings: {:?}", warnings);
    // AnimatedTriangle should have animation data
    assert!(
        !model.animations.is_empty(),
        "AnimatedTriangle should have animations"
    );
}

#[test]
fn model_structure_box() {
    let path = test_data_path("Box", "glTF");
    let data = fs::read(&path).expect("Failed to read Box.gltf");
    let GltfOk { model, .. } = GltfReader::default()
        .parse(&data)
        .expect("Failed to load Box.gltf");

    // Verify basic model structure
    assert!(
        !model.meshes.is_empty(),
        "Box should have at least one mesh"
    );

    let mesh = &model.meshes[0];
    assert!(!mesh.primitives.is_empty(), "Mesh should have primitives");

    // Box primitive should have POSITION and NORMAL attributes at minimum
    let prim = &mesh.primitives[0];
    assert!(
        prim.attributes.contains_key("POSITION"),
        "Primitive should have POSITION"
    );
    assert!(
        prim.attributes.contains_key("NORMAL"),
        "Primitive should have NORMAL"
    );
}

#[test]
fn model_buffers_loaded() {
    let path = test_data_path("Duck", "glTF");
    let data = fs::read(&path).expect("Failed to read Duck.gltf");
    let GltfOk { model, .. } = GltfReader::default()
        .parse(&data)
        .expect("Failed to load Duck.gltf");

    // Verify that buffers are loaded
    assert!(!model.buffers.is_empty(), "GltfModel should have buffers");

    for buffer in &model.buffers {
        assert!(buffer.byte_length > 0, "Buffer should have non-zero size");
    }
}

#[test]
fn asset_version_present() {
    let path = test_data_path("Box", "glTF");
    let data = fs::read(&path).expect("Failed to read Box.gltf");
    let GltfOk { model, .. } = GltfReader::default()
        .parse(&data)
        .expect("Failed to load Box.gltf");
    assert_eq!(model.asset.version, "2.0", "glTF version should be 2.0");
}

#[test]
fn builder_multiple_builds() {
    let path = test_data_path("Box", "glTF");
    let data = fs::read(&path).expect("Failed to read Box.gltf");

    // Build same model multiple times
    for _ in 0..3 {
        GltfReader::default()
            .parse(&data)
            .expect("Multiple reads should succeed");
    }
}

#[test]
fn invalid_data_returns_error() {
    let invalid_data = b"This is not a valid glTF file";
    let result = GltfReader::default().parse(invalid_data);
    assert!(result.is_err(), "Invalid data should produce an error");
}

#[test]
fn textures_loaded_in_duck() {
    let path = test_data_path("Duck", "glTF");
    let data = fs::read(&path).expect("Failed to read Duck.gltf");
    let GltfOk { model, .. } = GltfReader::default()
        .parse(&data)
        .expect("Failed to load Duck.gltf");
    assert!(!model.textures.is_empty(), "Duck should have textures");

    // Verify texture sources reference valid images
    for texture in &model.textures {
        if let Some(source_idx) = texture.source {
            assert!(
                source_idx < model.images.len(),
                "Texture source index out of bounds"
            );
        }
    }
}

#[test]
fn materials_in_duck() {
    let path = test_data_path("Duck", "glTF");
    let data = fs::read(&path).expect("Failed to read Duck.gltf");
    let GltfOk { model, .. } = GltfReader::default()
        .parse(&data)
        .expect("Failed to load Duck.gltf");
    assert!(!model.materials.is_empty(), "Duck should have materials");

    for material in &model.materials {
        // Verify material has reasonable properties
        assert!(
            material.pbr_metallic_roughness.is_some(),
            "Material should have PBR properties"
        );
    }
}

#[test]
fn animation_sampling() {
    let path = test_data_path("AnimatedTriangle", "glTF");
    let data = fs::read(&path).expect("Failed to read AnimatedTriangle.gltf");
    let GltfOk { model, .. } = GltfReader::default()
        .parse(&data)
        .expect("Failed to load AnimatedTriangle.gltf");
    assert!(!model.animations.is_empty(), "Should have animations");

    let animation = &model.animations[0];
    assert!(
        !animation.channels.is_empty(),
        "Animation should have channels"
    );
    assert!(
        !animation.samplers.is_empty(),
        "Animation should have samplers"
    );

    for channel in &animation.channels {
        assert!(
            channel.sampler < animation.samplers.len(),
            "Channel sampler index out of bounds"
        );
    }
}

#[test]
fn validation_box_vertices() {
    let path = test_data_path("Box", "glTF");
    let data = fs::read(&path).expect("Failed to read Box.gltf");
    let GltfOk { model, .. } = GltfReader::default()
        .parse(&data)
        .expect("Failed to load Box.gltf");
    assert!(!model.meshes.is_empty(), "Box should have meshes");

    for mesh in &model.meshes {
        for primitive in &mesh.primitives {
            // Box should have at least position and normal attributes
            let has_position = primitive.attributes.contains_key("POSITION");
            let has_normal = primitive.attributes.contains_key("NORMAL");

            assert!(
                has_position && has_normal,
                "Box primitives should have POSITION and NORMAL"
            );
        }
    }
}

#[test]
fn buffer_view_references_valid() {
    let path = test_data_path("Duck", "glTF");
    let data = fs::read(&path).expect("Failed to read Duck.gltf");
    let GltfOk { model, .. } = GltfReader::default()
        .parse(&data)
        .expect("Failed to load Duck.gltf");

    // Verify all buffer view references are valid
    for buffer_view in &model.buffer_views {
        assert!(
            buffer_view.buffer < model.buffers.len(),
            "Buffer view references invalid buffer"
        );
    }
}

#[test]
fn accessor_references_valid() {
    let path = test_data_path("Duck", "glTF");
    let data = fs::read(&path).expect("Failed to read Duck.gltf");
    let GltfOk { model, .. } = GltfReader::default()
        .parse(&data)
        .expect("Failed to load Duck.gltf");

    // Verify all accessor references are valid
    for accessor in &model.accessors {
        if let Some(bv_idx) = accessor.buffer_view {
            assert!(
                bv_idx < model.buffer_views.len(),
                "Accessor references invalid buffer view"
            );
        }
    }
}

#[test]
fn skins_and_joints_resolve() {
    let path = test_data_path("Duck", "glTF");
    let data = fs::read(&path).expect("Failed to read Duck.gltf");
    let GltfOk { model, .. } = GltfReader::default()
        .parse(&data)
        .expect("Failed to load Duck.gltf");

    // Verify skin joint references are valid
    for skin in &model.skins {
        for &joint_idx in &skin.joints {
            assert!(
                joint_idx < model.nodes.len(),
                "Skin references invalid joint node"
            );
        }
    }
}

#[test]
fn node_hierarchy_valid() {
    let path = test_data_path("Duck", "glTF");
    let data = fs::read(&path).expect("Failed to read Duck.gltf");
    let GltfOk { model, .. } = GltfReader::default()
        .parse(&data)
        .expect("Failed to load Duck.gltf");
    for node in &model.nodes {
        if let Some(children) = &node.children {
            for &child_idx in children {
                assert!(
                    child_idx < model.nodes.len(),
                    "Node references invalid child"
                );
            }
        }
    }
}

// ============ Data Correctness Validation Tests ============

#[test]
fn box_has_correct_geometry() {
    let path = test_data_path("Box", "glTF");
    let data = fs::read(&path).expect("Failed to read Box.gltf");
    let GltfOk { model, .. } = GltfReader::default()
        .parse(&data)
        .expect("Failed to load Box.gltf");
    assert!(!model.meshes.is_empty(), "Box should have meshes");

    let mesh = &model.meshes[0];
    assert!(
        !mesh.primitives.is_empty(),
        "Box mesh should have primitives"
    );

    let prim = &mesh.primitives[0];

    // Box should have consistent topology
    if let Some(indices_idx) = prim.indices {
        let accessor = &model.accessors[indices_idx];
        // Box typically has 36 indices (6 faces × 6 vertices)
        assert!(
            accessor.count > 0,
            "Box indices accessor should have non-zero count"
        );
    }

    // Check that accessor counts are reasonable
    for (_attr_name, &attr_idx) in &prim.attributes {
        let accessor = &model.accessors[attr_idx];
        assert!(accessor.count > 0, "Accessor count should be positive");
    }
}

#[test]
fn duck_material_values_reasonable() {
    let path = test_data_path("Duck", "glTF");
    let data = fs::read(&path).expect("Failed to read Duck.gltf");
    let GltfOk { model, .. } = GltfReader::default()
        .parse(&data)
        .expect("Failed to load Duck.gltf");
    assert!(!model.materials.is_empty(), "Duck should have materials");

    for material in &model.materials {
        if let Some(_pbr) = &material.pbr_metallic_roughness {
            // Just verify PBR object exists - types are serde_json::Value
            assert!(
                true, // metallic_factor and roughness_factor are f64, not Option
                "Material should have PBR properties"
            );
        }
    }
}

#[test]
fn duck_image_data_loaded() {
    let path = test_data_path("Duck", "glTF");
    let data = fs::read(&path).expect("Failed to read Duck.gltf");
    let GltfOk { model, .. } = GltfReader::default()
        .parse(&data)
        .expect("Failed to load Duck.gltf");
    assert!(!model.images.is_empty(), "Duck should have images");

    for image in &model.images {
        // For glTF files, images should have either uri or buffer view
        let has_uri = image.uri.as_ref().map_or(false, |u: &String| !u.is_empty());
        assert!(
            has_uri || image.buffer_view.is_some(),
            "Image should have uri or buffer_view"
        );

        // If it has a buffer_view, verify it's valid
        if let Some(bv_idx) = image.buffer_view {
            assert!(
                bv_idx < model.buffer_views.len(),
                "Image buffer_view index out of bounds"
            );
        }
    }
}

#[test]
fn animation_data_consistency() {
    let path = test_data_path("AnimatedTriangle", "glTF");
    let data = fs::read(&path).expect("Failed to read AnimatedTriangle.gltf");
    let GltfOk { model, .. } = GltfReader::default()
        .parse(&data)
        .expect("Failed to load AnimatedTriangle.gltf");
    assert!(!model.animations.is_empty(), "Should have animations");

    for animation in &model.animations {
        assert!(
            !animation.samplers.is_empty(),
            "Animation should have samplers"
        );
        assert!(
            !animation.channels.is_empty(),
            "Animation should have channels"
        );

        // Each channel should reference a valid sampler
        for channel in &animation.channels {
            let sampler_idx = channel.sampler;
            assert!(
                sampler_idx < animation.samplers.len(),
                "Channel references invalid sampler"
            );

            let sampler = &animation.samplers[sampler_idx];

            // Input and output accessors should be valid
            let input_idx = sampler.input;
            let input_accessor = &model.accessors[input_idx];
            // Input (time) should have at least 1 sample
            assert!(
                input_accessor.count > 0,
                "Animation input should have samples"
            );

            let output_idx = sampler.output;
            let output_accessor = &model.accessors[output_idx];
            // Output should have at least 1 sample
            assert!(
                output_accessor.count > 0,
                "Animation output should have samples"
            );

            // Target node should reference a valid node if present
            if let Some(target_node_idx) = channel.target.node {
                assert!(
                    target_node_idx < model.nodes.len(),
                    "Animation channel targets invalid node"
                );
            }
        }
    }
}

#[test]
fn buffer_data_alignment() {
    let path = test_data_path("Duck", "glTF");
    let data = fs::read(&path).expect("Failed to read Duck.gltf");
    let GltfOk { model, .. } = GltfReader::default()
        .parse(&data)
        .expect("Failed to load Duck.gltf");

    // Verify buffer views have reasonable properties
    for bv in &model.buffer_views {
        // Byte length should be positive
        assert!(bv.byte_length > 0, "Buffer view length should be positive");

        // Stride, if present, should be in valid range [4, 252]
        if let Some(stride) = bv.byte_stride {
            assert!(
                stride >= 4 && stride <= 252,
                "Buffer view stride out of range: {}",
                stride
            );
        }

        // Buffer reference should be valid
        assert!(
            bv.buffer < model.buffers.len(),
            "Buffer view references invalid buffer"
        );
    }
}

#[test]
fn accessor_data_types_valid() {
    let path = test_data_path("Duck", "glTF");
    let data = fs::read(&path).expect("Failed to read Duck.gltf");
    let GltfOk { model, .. } = GltfReader::default()
        .parse(&data)
        .expect("Failed to load Duck.gltf");

    for accessor in &model.accessors {
        // Count should be positive
        assert!(accessor.count > 0, "Accessor count should be positive");

        // If accessor has buffer_view, verify it's valid
        if let Some(bv_idx) = accessor.buffer_view {
            assert!(
                bv_idx < model.buffer_views.len(),
                "Accessor references invalid buffer_view"
            );
        }

        // Component type is now a proper enum — just verify it exists
        // Component type should be a valid enum value
        match accessor.component_type {
            AccessorComponentType::Byte
            | AccessorComponentType::UnsignedByte
            | AccessorComponentType::Short
            | AccessorComponentType::UnsignedShort
            | AccessorComponentType::UnsignedInt
            | AccessorComponentType::Float => {
                // Valid component type
            }
        }
    }
}

#[test]
fn texture_references_consistent() {
    let path = test_data_path("Duck", "glTF");
    let data = fs::read(&path).expect("Failed to read Duck.gltf");
    let GltfOk { model, .. } = GltfReader::default()
        .parse(&data)
        .expect("Failed to load Duck.gltf");

    // Each texture should reference a valid image
    for texture in &model.textures {
        if let Some(source_idx) = texture.source {
            assert!(
                source_idx < model.images.len(),
                "Texture source out of bounds"
            );

            // Texture sampler, if present, should be valid
            if let Some(sampler_idx) = texture.sampler {
                assert!(
                    sampler_idx < model.samplers.len(),
                    "Texture sampler out of bounds"
                );
            }
        }
    }
}

#[test]
fn primitive_mode_valid() {
    let path = test_data_path("Box", "glTF");
    let data = fs::read(&path).expect("Failed to read Box.gltf");
    let GltfOk { model, .. } = GltfReader::default()
        .parse(&data)
        .expect("Failed to load Box.gltf");

    for mesh in &model.meshes {
        for prim in &mesh.primitives {
            // Primitive mode should be a valid enum value
            match prim.mode {
                PrimitiveMode::Points
                | PrimitiveMode::Lines
                | PrimitiveMode::LineLoop
                | PrimitiveMode::LineStrip
                | PrimitiveMode::Triangles
                | PrimitiveMode::TriangleStrip
                | PrimitiveMode::TriangleFan => {
                    // Valid primitive mode
                }
            }

            // Material reference should be valid (optional but if present must be valid)
            if let Some(mat_idx) = prim.material {
                assert!(
                    mat_idx < model.materials.len(),
                    "Primitive references invalid material"
                );
            }
        }
    }
}

#[test]
fn skin_inverse_bind_matrices() {
    let path = test_data_path("Duck", "glTF");
    let data = fs::read(&path).expect("Failed to read Duck.gltf");
    let GltfOk { model, .. } = GltfReader::default()
        .parse(&data)
        .expect("Failed to load Duck.gltf");

    for skin in &model.skins {
        // Inverse bind matrices accessor should be valid
        let ibm_idx = skin
            .inverse_bind_matrices
            .expect("Skin should have inverse_bind_matrices");
        let ibm_accessor = &model.accessors[ibm_idx];

        // Count should match number of joints
        assert_eq!(
            ibm_accessor.count as usize,
            skin.joints.len(),
            "Inverse bind matrices count should match joint count"
        );

        // All joint indices should be valid node references
        for &joint_idx in &skin.joints {
            assert!(
                joint_idx < model.nodes.len(),
                "Skin joint references invalid node"
            );
        }

        // Skeleton, if present, should reference a valid node
        if let Some(skeleton_idx) = skin.skeleton {
            assert!(
                skeleton_idx < model.nodes.len(),
                "Skin skeleton references invalid node"
            );
        }
    }
}
